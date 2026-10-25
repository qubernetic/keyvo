//! The pad's keys for the GUI, rendered like the daemon renders them for
//! the LCD (spec §10, device mirror).

use keyvo_core::layout::Profiles;
use keyvo_core::render::{self, RenderError, Renderer, Tile, TileSize};
use keyvo_core::state::View;

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
        let profile = profiles
            .get(&view.profile)
            .ok_or_else(|| TileError::UnknownProfile(view.profile.clone()))?;
        render::page(profile, view.page, keys)?
            .iter()
            .map(|spec| Ok(self.renderer.tile(spec, size)?))
            .collect()
    }
}
