//! The pad's keys for the GUI, rendered like the daemon renders them for
//! the LCD (spec §10, device mirror): a key the shown profile does not
//! define comes from the global profile (spec §6.1), and each key names the
//! layer that supplied it.

use keyvo_core::layout::Profiles;
use keyvo_core::profile::{Action, KeySlot};
use keyvo_core::render::{self, RenderError, Renderer, Tile, TileSize};
use keyvo_core::stack::{Layer, Stack};
use keyvo_core::state::View;
use serde::Serialize;

/// Why a page has no tiles.
#[derive(Debug, thiserror::Error)]
pub enum TileError {
    /// The daemon shows a profile the GUI's configuration does not have.
    #[error("no profile `{0}` in the configuration directory")]
    UnknownProfile(String),
    /// The page does not render.
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// What a key of the mirror says besides its pixels: its label and its
/// action as in the profile, and the layer they come from, for the GUI to
/// announce.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct KeySummary {
    /// The key's label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The key's action; none for an unbound key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// The layer that supplied the key; none when no layer defines it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
}

/// The page `view` shows over the global layer.
fn stack<'a>(profiles: &'a Profiles, view: &View) -> Result<Stack<'a>, TileError> {
    let profile = profiles
        .get(&view.profile)
        .ok_or_else(|| TileError::UnknownProfile(view.profile.clone()))?;
    Ok(Stack::new(profiles, profile, view.page)?)
}

/// The `keys` keys of the page `view` shows, in key order.
///
/// # Errors
///
/// As [`Tiles::page`].
pub fn keys(profiles: &Profiles, view: &View, keys: u8) -> Result<Vec<KeySummary>, TileError> {
    let stack = stack(profiles, view)?;
    Ok((1..=keys)
        .map(|n| {
            KeySlot::new(n)
                .and_then(|slot| stack.key(slot))
                .map_or_else(KeySummary::default, |(key, layer)| KeySummary {
                    label: key.label.clone(),
                    action: key.action.clone(),
                    layer: Some(layer),
                })
        })
        .collect())
}

/// Renders pages with the `keyvo-core` renderer.
#[derive(Debug)]
pub struct Tiles {
    renderer: Renderer,
}

impl Default for Tiles {
    fn default() -> Self {
        Self::new()
    }
}

impl Tiles {
    /// A renderer with the built-in theme and fonts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            renderer: Renderer::new(),
        }
    }

    /// The `keys` keys of the page `view` shows, at `size`, in key order.
    ///
    /// # Errors
    ///
    /// When `profiles` has no profile of `view`'s id, or the page does not
    /// render.
    pub fn page(
        &self,
        profiles: &Profiles,
        view: &View,
        keys: u8,
        size: TileSize,
    ) -> Result<Vec<Tile>, TileError> {
        render::page(&stack(profiles, view)?, keys)
            .iter()
            .map(|spec| Ok(self.renderer.tile(spec, size)?))
            .collect()
    }
}
