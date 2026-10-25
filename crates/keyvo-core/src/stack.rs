//! The layer stack (spec §6.1): the keys of an app profile's page sit on
//! the global layer, so a key the app profile does not define shows and
//! runs the same key of the global profile.
//!
//! As built (pre-alpha): two layers. Under every page of an app profile
//! lies the global profile's context page; the global profile has nothing
//! under it. The project layer and the overlays come with their slices.

use serde::{Deserialize, Serialize};

use crate::layout::Profiles;
use crate::profile::{GLOBAL_ID, Key, KeySlot, Page, Profile};
use crate::render::RenderError;
use crate::state;

/// The layer of the stack that supplied a key; the names are the ones of
/// `KEYVO_LAYER` (spec §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// The global profile.
    Global,
    /// The active app profile.
    App,
}

impl Layer {
    /// The name in the spec, the trace and `KEYVO_LAYER`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::App => "app",
        }
    }

    /// The layer `profile` forms.
    #[must_use]
    pub fn of(profile: &Profile) -> Self {
        if profile.profile.id == GLOBAL_ID {
            Self::Global
        } else {
            Self::App
        }
    }
}

/// One page of a profile with the layer under it.
#[derive(Debug, Clone, Copy)]
pub struct Stack<'a> {
    profile: &'a Profile,
    page: usize,
    top: &'a Page,
    global: Option<&'a Page>,
}

impl<'a> Stack<'a> {
    /// Page `page` (counted from 1) of `profile`; for an app profile the
    /// context page of `profiles`' global profile lies under it.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoSuchPage`] when `profile` has no page `page`.
    pub fn new(
        profiles: &'a Profiles,
        profile: &'a Profile,
        page: usize,
    ) -> Result<Self, RenderError> {
        let mut stack = Self::alone(profile, page)?;
        if Layer::of(profile) == Layer::App {
            let global = profiles.global();
            stack.global = state::context_page(global)
                .checked_sub(1)
                .and_then(|index| global.pages.get(index));
        }
        Ok(stack)
    }

    /// Page `page` (counted from 1) of `profile` with nothing under it, as
    /// `keyvo preview` renders one profile file.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoSuchPage`] when `profile` has no page `page`.
    pub fn alone(profile: &'a Profile, page: usize) -> Result<Self, RenderError> {
        let top = page
            .checked_sub(1)
            .and_then(|index| profile.pages.get(index))
            .ok_or_else(|| RenderError::NoSuchPage {
                profile: profile.profile.id.clone(),
                page,
                pages: profile.pages.len(),
            })?;
        Ok(Self {
            profile,
            page,
            top,
            global: None,
        })
    }

    /// The profile on top.
    #[must_use]
    pub const fn profile(&self) -> &'a Profile {
        self.profile
    }

    /// The page of the profile on top, counted from 1.
    #[must_use]
    pub const fn page(&self) -> usize {
        self.page
    }

    /// The key at `slot` and the layer it comes from: the profile's own,
    /// else the global one; `None` when no layer defines it. A key the
    /// profile defines hides the global one, even without an action.
    #[must_use]
    pub fn key(&self, slot: KeySlot) -> Option<(&'a Key, Layer)> {
        self.top
            .keys
            .get(&slot)
            .map(|key| (key, Layer::of(self.profile)))
            .or_else(|| {
                self.global
                    .and_then(|page| page.keys.get(&slot))
                    .map(|key| (key, Layer::Global))
            })
    }

    /// The layer the key at `slot` comes from, if any defines it.
    #[must_use]
    pub fn layer(&self, slot: KeySlot) -> Option<Layer> {
        self.key(slot).map(|(_, layer)| layer)
    }
}
