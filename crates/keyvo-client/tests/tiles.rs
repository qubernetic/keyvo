//! The GUI's tiles are the daemon's tiles: the same `keyvo-core` renderer
//! on the same profile gives the same pixels.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use keyvo_client::tiles::{self, KeySummary, Outline, TileError, Tiles};
use keyvo_core::layout::{self, Profiles};
use keyvo_core::render::{self, Renderer, TileSize};
use keyvo_core::stack::{Layer, Stack};
use keyvo_core::state::{Mode, View};

const SIZE: TileSize = TileSize {
    width: 118,
    height: 118,
};

fn profiles() -> Profiles {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../keyvo-core/tests/fixtures/dirs/full");
    layout::load(&dir).unwrap().profiles
}

fn view(profile: &str, page: usize) -> View {
    View {
        profile: profile.into(),
        page,
        mode: Mode::Auto,
        index: 1,
        locked: false,
    }
}

#[test]
fn a_page_renders_like_the_daemon_paints_it() {
    let profiles = profiles();
    let paged: Vec<_> = profiles.iter().filter(|p| !p.pages.is_empty()).collect();
    assert!(!paged.is_empty());
    for profile in paged {
        let id = &profile.profile.id;
        let tiles = Tiles::new().page(&profiles, &view(id, 1), 9, SIZE).unwrap();
        let specs = render::page(&Stack::new(&profiles, profile, 1).unwrap(), 9);
        assert_eq!(tiles.len(), 9, "{id}");
        for (tile, spec) in tiles.iter().zip(&specs) {
            let painted = Renderer::new().tile(spec, SIZE).unwrap();
            assert_eq!(tile.size(), SIZE);
            assert_eq!(tile.rgb(), painted.rgb(), "{id}");
        }
    }
}

#[test]
fn a_locked_view_renders_the_lock_page_and_names_no_key() {
    let profiles = profiles();
    let locked = View {
        locked: true,
        ..view("global", 1)
    };
    let accent = profiles.get("global").unwrap().profile.accent;
    let tiles = Tiles::new().page(&profiles, &locked, 9, SIZE).unwrap();
    let specs = render::lock_page(accent, 9);
    assert_eq!(tiles.len(), 9);
    for (tile, spec) in tiles.iter().zip(&specs) {
        assert_eq!(tile.rgb(), Renderer::new().tile(spec, SIZE).unwrap().rgb());
    }
    let keys = tiles::keys(&profiles, &locked, 9).unwrap();
    assert_eq!(keys, vec![KeySummary::default(); 9]);
}

#[test]
fn the_keys_of_a_page_carry_label_and_action_in_key_order() {
    let keys = tiles::keys(&profiles(), &view("global", 1), 9).unwrap();
    assert_eq!(keys.len(), 9);
    assert_eq!(keys[4].label.as_deref(), Some("Mute"));
    assert_eq!(
        serde_json::to_value(&keys[4]).unwrap(),
        serde_json::json!({
            "label": "Mute",
            "action": {"type": "exec", "cmd": "wpctl set-mute @DEFAULT_SINK@ toggle"},
            "layer": "global",
        })
    );
    assert_eq!(
        serde_json::to_value(&keys[0]).unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn a_key_the_profile_lacks_shows_the_global_key_and_names_its_layer() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../keyvo-core/tests/fixtures/dirs/fallback");
    let profiles = layout::load(&dir).unwrap().profiles;
    let konsole = view("konsole", 1);
    let keys = tiles::keys(&profiles, &konsole, 9).unwrap();
    assert_eq!(
        (keys[0].label.as_deref(), keys[0].layer),
        (Some("New tab"), Some(Layer::App))
    );
    assert_eq!(
        (keys[2].label.as_deref(), keys[2].layer),
        (Some("Type"), Some(Layer::Global))
    );
    assert_eq!(keys[3], KeySummary::default());
    let tiles = Tiles::new().page(&profiles, &konsole, 9, SIZE).unwrap();
    // Key 3 is drawn, not blank like key 4, which no layer defines.
    assert_ne!(tiles[2].rgb(), tiles[3].rgb());
}

#[test]
fn the_keys_of_an_unknown_profile_or_page_are_an_error() {
    assert!(matches!(
        tiles::keys(&profiles(), &view("nowhere", 1), 9),
        Err(TileError::UnknownProfile(_))
    ));
    assert!(matches!(
        tiles::keys(&profiles(), &view("global", 99), 9),
        Err(TileError::Render(_))
    ));
}

#[test]
fn a_profile_the_gui_does_not_have_is_named() {
    let error = Tiles::new()
        .page(&profiles(), &view("nowhere", 1), 9, SIZE)
        .unwrap_err();
    assert!(matches!(error, TileError::UnknownProfile(ref id) if id == "nowhere"));
    assert!(error.to_string().contains("nowhere"), "{error}");
}

#[test]
fn the_outline_of_a_profile_has_its_page_count_and_accent() {
    let profiles = profiles();
    assert_eq!(
        tiles::outline(&profiles, "konsole").unwrap(),
        Outline {
            pages: 0,
            accent: Some("#2ecc71".into())
        }
    );
    assert_eq!(
        tiles::outline(&profiles, "global").unwrap(),
        Outline {
            pages: 1,
            accent: None
        }
    );
    assert_eq!(
        serde_json::to_value(tiles::outline(&profiles, "konsole").unwrap()).unwrap(),
        serde_json::json!({"pages": 0, "accent": "#2ecc71"})
    );
    assert_eq!(
        serde_json::to_value(tiles::outline(&profiles, "global").unwrap()).unwrap(),
        serde_json::json!({"pages": 1})
    );
    assert!(matches!(
        tiles::outline(&profiles, "nowhere"),
        Err(TileError::UnknownProfile(ref id)) if id == "nowhere"
    ));
}

#[test]
fn a_page_the_profile_does_not_have_is_a_render_error() {
    let error = Tiles::new()
        .page(&profiles(), &view("global", 99), 9, SIZE)
        .unwrap_err();
    assert!(matches!(error, TileError::Render(_)), "{error:?}");
}
