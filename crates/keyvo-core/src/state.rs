//! The context state machine (spec §6.1, §6.3): which profile and page the
//! pad shows, from the focused window and the two page buttons.
//!
//! - `auto` mode: the focused window selects the profile
//!   ([`Profiles::select`]) once its report has stood for
//!   `[timing].focus_hysteresis`; the pad shows that profile's context page
//!   or the global profile's general page, and a short page-button press
//!   toggles between them. An app switch (another window class) on the
//!   general page returns to the context page, unless a page button was
//!   used within `[timing].page_grace`.
//! - `manual` mode: the profile and page stay whatever the focus does;
//!   right pages forward, left back, both wrap around.
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

/// Who chooses the profile and page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// The focused window chooses.
    Auto,
    /// The user pages through one profile; focus changes are ignored.
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
    /// The id of the profile whose page is painted.
    pub profile: String,
    /// The page, counted from 1.
    pub page: usize,
    /// The mode.
    pub mode: Mode,
}

/// The page auto mode shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutoPage {
    Context,
    General,
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
    auto_page: AutoPage,
    /// The profile and page manual mode stays on.
    manual: (String, usize),
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
    pub fn new(config: &Config, profiles: &Profiles) -> Self {
        let timing = &config.timing;
        Self {
            hysteresis: timing.focus_hysteresis,
            page_grace: timing.page_grace,
            long_press: timing.long_press,
            mode: Mode::Auto,
            selected: GLOBAL_ID.to_owned(),
            auto_page: AutoPage::Context,
            manual: (GLOBAL_ID.to_owned(), context_page(profiles.global())),
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
        let (profile, page) = match self.mode {
            Mode::Manual => {
                let profile = profiles
                    .get(&self.manual.0)
                    .unwrap_or_else(|| profiles.global());
                (profile, self.manual.1)
            }
            Mode::Auto => {
                if let (AutoPage::General, Some(page)) = (self.auto_page, general_page(profiles)) {
                    (profiles.global(), page)
                } else {
                    let profile = profiles
                        .get(&self.selected)
                        .unwrap_or_else(|| profiles.global());
                    (profile, context_page(profile))
                }
            }
        };
        View {
            profile: profile.profile.id.clone(),
            page,
            mode: self.mode,
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
    pub fn press(&mut self, button: PageButton, now: Instant, profiles: &Profiles) {
        self.page_used = Some(now);
        self.held.insert(button);
        if self.held.len() > 1 {
            self.escape(profiles);
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
            Some(PressKind::Long) => self.toggle_mode(profiles),
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
            self.toggle_mode(profiles);
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
    /// rules may have changed; manual mode stays unless its profile is gone
    /// (then the global context page) or its page is (then page 1).
    pub fn reload(&mut self, profiles: &Profiles) {
        self.selected = self.window.as_ref().map_or_else(
            || GLOBAL_ID.to_owned(),
            |window| profiles.select(window).profile.id.clone(),
        );
        if general_page(profiles).is_none() {
            self.auto_page = AutoPage::Context;
        }
        match profiles.get(&self.manual.0) {
            None => {
                self.manual = (GLOBAL_ID.to_owned(), context_page(profiles.global()));
            }
            Some(profile) if self.manual.1 > profile.pages.len() => self.manual.1 = 1,
            Some(_) => {}
        }
    }

    fn apply(&mut self, window: WindowContext, now: Instant, profiles: &Profiles) {
        let app_switch = self
            .window
            .as_ref()
            .is_none_or(|old| old.class != window.class);
        let in_grace = self
            .page_used
            .is_some_and(|used| now.saturating_duration_since(used) < self.page_grace);
        if self.auto_page == AutoPage::General && app_switch && !in_grace {
            self.auto_page = AutoPage::Context;
        }
        profiles
            .select(&window)
            .profile
            .id
            .clone_into(&mut self.selected);
        self.window = Some(window);
    }

    fn step(&mut self, button: PageButton, profiles: &Profiles) {
        match self.mode {
            Mode::Auto => {
                self.auto_page =
                    if self.auto_page == AutoPage::Context && general_page(profiles).is_some() {
                        AutoPage::General
                    } else {
                        AutoPage::Context
                    };
            }
            Mode::Manual => {
                let pages = profiles
                    .get(&self.manual.0)
                    .map_or(0, |profile| profile.pages.len());
                if pages > 0 {
                    let index = self.manual.1.clamp(1, pages) - 1;
                    let next = match button {
                        PageButton::Right => (index + 1) % pages,
                        PageButton::Left => (index + pages - 1) % pages,
                    };
                    self.manual.1 = next + 1;
                }
            }
        }
    }

    fn toggle_mode(&mut self, profiles: &Profiles) {
        match self.mode {
            Mode::Auto => {
                let view = self.view(profiles);
                self.manual = (view.profile, view.page);
                self.mode = Mode::Manual;
            }
            Mode::Manual => {
                self.auto_page = AutoPage::Context;
                self.mode = Mode::Auto;
            }
        }
    }

    /// Both page buttons: the global context page in auto mode; the held
    /// buttons do nothing more until they are pressed again.
    fn escape(&mut self, profiles: &Profiles) {
        self.mode = Mode::Auto;
        self.auto_page = AutoPage::Context;
        GLOBAL_ID.clone_into(&mut self.selected);
        self.manual = (GLOBAL_ID.to_owned(), context_page(profiles.global()));
        self.buttons = PressDetector::new(self.long_press);
    }
}

/// The page named `context`, else the first; counted from 1.
pub(crate) fn context_page(profile: &Profile) -> usize {
    named_page(profile, "context").unwrap_or(1)
}

/// The global profile's page named `general`, counted from 1.
fn general_page(profiles: &Profiles) -> Option<usize> {
    named_page(profiles.global(), "general")
}

fn named_page(profile: &Profile, name: &str) -> Option<usize> {
    profile
        .pages
        .iter()
        .position(|page| page.name.as_deref() == Some(name))
        .map(|index| index + 1)
}
