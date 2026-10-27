//! The context state machine (spec §6.1, §6.3): which profile and page the
//! pad shows, from the focused window and the two page buttons.
//!
//! - The page list: for the shown profile the pad walks one ordered list.
//!   Index 0 is the global profile's page named `general`, the global pages
//!   after it in file order take -1, -2, ...; index 1 is the shown
//!   profile's context page, its other pages follow from 2 in file order.
//!   A short press of the left button shows index - 1, of the right one
//!   index + 1; the list does not wrap.
//! - `auto` mode: the focused window selects the profile
//!   ([`Profiles::select`]) once its report has stood for
//!   `[timing].focus_hysteresis`; an app switch (another window class)
//!   shows index 1 of the new profile, unless a page button was used within
//!   `[timing].page_grace`: then the index stays, clamped to the new list.
//!   A title change in the same app keeps the index.
//! - `manual` mode is a focus lock: the shown profile and index stay
//!   whatever the focus does, the buttons walk the same list. Leaving it
//!   shows index 1 of the profile the focused window selects.
//! - A page button held for `[timing].long_press` toggles the mode; both
//!   held together escape to the global profile's context page in auto
//!   mode.
//! - A window whose class is in `[focus].hold_classes` (the keyvo app)
//!   holds the context: its report cancels a pending one and the window
//!   applied before stays, so the profile and page stay too.
//!
//! Synchronous and clock-free: every call takes `now`, and the caller
//! wakes the machine with [`State::poll`] at [`State::next_deadline`].

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::{Config, Focus};
use crate::focus::WindowContext;
use crate::layout::Profiles;
use crate::press::{PressDetector, PressKind};
use crate::profile::{GLOBAL_ID, Profile};

/// Who chooses the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// The focused window chooses.
    Auto,
    /// The profile is locked; focus changes are recorded, not shown.
    Manual,
}

impl Mode {
    /// The name in the spec and the socket protocol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

/// One of the Keypad's two page buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageButton {
    /// The left button (CID `0x01A1`).
    Left,
    /// The right button (CID `0x01A2`).
    Right,
}

/// What the pad shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// The id of the profile whose page is painted: the global one at index
    /// 0 and below.
    pub profile: String,
    /// The page, counted from 1 inside `profile`.
    pub page: usize,
    /// The mode.
    pub mode: Mode,
    /// The position in the shown profile's page list: 1 its context page,
    /// 0 the global `general` page.
    pub index: isize,
}

/// The mode and page state of the pad.
#[derive(Debug, Clone)]
pub struct State {
    hysteresis: Duration,
    page_grace: Duration,
    long_press: Duration,
    mode: Mode,
    /// The profile the focused window selects.
    selected: String,
    /// The profile whose page list the pad walks: the selected one in auto
    /// mode, the locked one in manual mode.
    shown: String,
    /// The position in the shown profile's page list.
    index: isize,
    /// The focused window as last applied.
    window: Option<WindowContext>,
    /// A newer window and when it applies.
    pending: Option<(WindowContext, Instant)>,
    /// The classes that hold the context.
    hold: Focus,
    /// The focused window while it holds the context.
    in_front: Option<WindowContext>,
    /// The last page-button press or release.
    page_used: Option<Instant>,
    held: BTreeSet<PageButton>,
    buttons: PressDetector<PageButton>,
}

impl State {
    /// Auto mode on the global profile's context page, before any focus
    /// report, with the `[timing]` and `[focus]` of `config`.
    #[must_use]
    pub fn new(config: &Config, _profiles: &Profiles) -> Self {
        let timing = &config.timing;
        Self {
            hysteresis: timing.focus_hysteresis,
            page_grace: timing.page_grace,
            long_press: timing.long_press,
            mode: Mode::Auto,
            selected: GLOBAL_ID.to_owned(),
            shown: GLOBAL_ID.to_owned(),
            index: 1,
            window: None,
            pending: None,
            hold: config.focus.clone(),
            in_front: None,
            page_used: None,
            held: BTreeSet::new(),
            buttons: PressDetector::new(timing.long_press),
        }
    }

    /// The mode.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// What the pad shows with `profiles`. A profile that no longer exists
    /// shows as the global one.
    #[must_use]
    pub fn view(&self, profiles: &Profiles) -> View {
        let shown = self.shown(profiles);
        let list = PageList::of(profiles, shown);
        let index = list.clamp(self.index);
        let (profile, page) = list.get(index).unwrap_or((shown, 1));
        View {
            profile: profile.profile.id.clone(),
            page,
            mode: self.mode,
            index,
        }
    }

    /// Whether a window of `class` holds the context.
    #[must_use]
    pub fn holds(&self, class: &str) -> bool {
        self.hold.holds(class)
    }

    /// The focused window, while it is one that holds the context.
    #[must_use]
    pub const fn held(&self) -> Option<&WindowContext> {
        self.in_front.as_ref()
    }

    /// The focus source reports `window` at `now`. It applies once no other
    /// report came for `[timing].focus_hysteresis`; a report of the window
    /// already applied cancels a pending one, and so does a report of a
    /// window that holds the context, which never applies.
    pub fn focus(&mut self, window: WindowContext, now: Instant, profiles: &Profiles) {
        if self.holds(&window.class) {
            self.pending = None;
            self.in_front = Some(window);
        } else {
            self.in_front = None;
            if self.window.as_ref() == Some(&window) {
                self.pending = None;
            } else {
                self.pending = Some((window, now + self.hysteresis));
            }
        }
        self.poll(now, profiles);
    }

    /// `button` went down at `now`. The second button held escapes.
    pub fn press(&mut self, button: PageButton, now: Instant, _profiles: &Profiles) {
        self.page_used = Some(now);
        self.held.insert(button);
        if self.held.len() > 1 {
            self.escape();
        } else {
            self.buttons.press(button, now);
        }
    }

    /// `button` came up at `now`: a short press pages, a long one not yet
    /// fired toggles the mode.
    pub fn release(&mut self, button: PageButton, now: Instant, profiles: &Profiles) {
        self.page_used = Some(now);
        self.held.remove(&button);
        match self.buttons.release(button, now) {
            Some(PressKind::Short) => self.step(button, profiles),
            Some(PressKind::Long) => self.toggle_mode(),
            None => {}
        }
    }

    /// Applies what is due at `now`: a pending window, a long press.
    pub fn poll(&mut self, now: Instant, profiles: &Profiles) {
        if self.pending.as_ref().is_some_and(|(_, at)| *at <= now)
            && let Some((window, _)) = self.pending.take()
        {
            self.apply(window, now, profiles);
        }
        for _ in self.buttons.poll(now) {
            self.toggle_mode();
        }
    }

    /// When [`State::poll`] has something to do next.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let focus = self.pending.as_ref().map(|(_, at)| *at);
        match (focus, self.buttons.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The profiles were reloaded: the focused window (while one that holds
    /// the context is in front, the one before it) selects again, as its
    /// rules may have changed; manual mode keeps its profile unless it is
    /// gone (then the global context page). An index the list no longer has
    /// becomes the nearest end.
    pub fn reload(&mut self, profiles: &Profiles) {
        self.selected = self.window.as_ref().map_or_else(
            || GLOBAL_ID.to_owned(),
            |window| profiles.select(window).profile.id.clone(),
        );
        match self.mode {
            Mode::Auto => self.shown.clone_from(&self.selected),
            Mode::Manual => {
                if profiles.get(&self.shown).is_none() {
                    GLOBAL_ID.clone_into(&mut self.shown);
                    self.index = 1;
                }
            }
        }
        self.index = PageList::of(profiles, self.shown(profiles)).clamp(self.index);
    }

    /// The shown profile in `profiles`, the global one when it is gone.
    fn shown<'a>(&self, profiles: &'a Profiles) -> &'a Profile {
        profiles
            .get(&self.shown)
            .unwrap_or_else(|| profiles.global())
    }

    fn apply(&mut self, window: WindowContext, now: Instant, profiles: &Profiles) {
        let app_switch = self
            .window
            .as_ref()
            .is_none_or(|old| old.class != window.class);
        let in_grace = self
            .page_used
            .is_some_and(|used| now.saturating_duration_since(used) < self.page_grace);
        profiles
            .select(&window)
            .profile
            .id
            .clone_into(&mut self.selected);
        self.window = Some(window);
        if self.mode == Mode::Auto {
            self.shown.clone_from(&self.selected);
            self.index = if app_switch && !in_grace {
                1
            } else {
                PageList::of(profiles, self.shown(profiles)).clamp(self.index)
            };
        }
    }

    /// A short press: the next index towards `button`, nothing past an end.
    fn step(&mut self, button: PageButton, profiles: &Profiles) {
        let list = PageList::of(profiles, self.shown(profiles));
        let next = match button {
            PageButton::Left => self.index - 1,
            PageButton::Right => self.index + 1,
        };
        if list.clamp(next) == next {
            self.index = next;
        }
    }

    /// A long press: auto mode locks the shown profile and index; the lock
    /// released shows index 1 of the profile the focused window selects.
    fn toggle_mode(&mut self) {
        match self.mode {
            Mode::Auto => self.mode = Mode::Manual,
            Mode::Manual => {
                self.mode = Mode::Auto;
                self.shown.clone_from(&self.selected);
                self.index = 1;
            }
        }
    }

    /// Both page buttons: the global context page in auto mode; the held
    /// buttons do nothing more until they are pressed again.
    fn escape(&mut self) {
        self.mode = Mode::Auto;
        GLOBAL_ID.clone_into(&mut self.selected);
        GLOBAL_ID.clone_into(&mut self.shown);
        self.index = 1;
        self.buttons = PressDetector::new(self.long_press);
    }
}

/// The page list of one shown profile (spec §6.3), lowest index first.
struct PageList<'a> {
    /// The index of the first entry: 1 minus the global pages from
    /// `general` on, 1 when the global profile has no `general` page.
    lowest: isize,
    /// The profile and page (counted from 1) of each index.
    pages: Vec<(&'a Profile, usize)>,
}

impl<'a> PageList<'a> {
    fn of(profiles: &'a Profiles, shown: &'a Profile) -> Self {
        let global = profiles.global();
        let general = named_page(global, "general");
        // The global pages from the last one down to `general`: indices
        // ..., -1, 0.
        let mut pages: Vec<_> = general
            .map(|general| (general..=global.pages.len()).rev())
            .into_iter()
            .flatten()
            .map(|page| (global, page))
            .collect();
        let lowest = 1 - isize::try_from(pages.len()).unwrap_or(0);
        let context = context_page(shown);
        let is_global = shown.profile.id == GLOBAL_ID;
        pages.push((shown, context));
        pages.extend(
            (1..=shown.pages.len())
                .filter(|&page| page != context)
                // The global profile's pages from `general` on are left of
                // its context page already.
                .filter(|&page| !is_global || general.is_none_or(|general| page < general))
                .map(|page| (shown, page)),
        );
        Self { lowest, pages }
    }

    /// The highest index.
    fn highest(&self) -> isize {
        self.lowest + isize::try_from(self.pages.len()).unwrap_or(1) - 1
    }

    /// `index` moved into the list: the nearest end when it is outside.
    fn clamp(&self, index: isize) -> isize {
        index.clamp(self.lowest, self.highest())
    }

    /// The profile and page at `index`.
    fn get(&self, index: isize) -> Option<(&'a Profile, usize)> {
        let at = usize::try_from(index - self.lowest).ok()?;
        self.pages.get(at).copied()
    }
}

/// The page named `context`, else the first; counted from 1.
pub(crate) fn context_page(profile: &Profile) -> usize {
    named_page(profile, "context").unwrap_or(1)
}

fn named_page(profile: &Profile, name: &str) -> Option<usize> {
    profile
        .pages
        .iter()
        .position(|page| page.name.as_deref() == Some(name))
        .map(|index| index + 1)
}
