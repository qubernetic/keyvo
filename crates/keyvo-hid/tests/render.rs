//! The renderer's tiles against the image writer: every tile of the demo
//! profile, rendered at the captured geometry, passes the writer's frame
//! header check (baseline SOF0 at the rectangle's size, spec §5.5).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use keyvo_core::profile::Profile;
use keyvo_core::render::{self, KeySpec, Renderer, TileSize};
use keyvo_hid::display;
use keyvo_hid::hidpp::DEVICE_DIRECT;
use keyvo_hid::image::{self, ImageConfig, Target};

/// The display's VLP index in the captures.
const DISPLAY: u8 = 2;

fn demo() -> Profile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../keyvo-core/tests/fixtures/profiles/valid/demo.toml");
    Profile::from_toml(&std::fs::read_to_string(&path).unwrap(), &path).unwrap()
}

#[test]
fn every_rendered_key_passes_the_writer_frame_check() {
    let geometry = display::captured_geometry();
    let panel = geometry.panel();
    let renderer = Renderer::new();
    let specs = render::page(&demo(), 1, u8::try_from(panel.keys.len()).unwrap()).unwrap();
    for (spec, key) in specs.iter().zip(&geometry.keys) {
        let rect = Target::Key(key.index).rect(&geometry).unwrap();
        let size = TileSize {
            width: rect.width,
            height: rect.height,
        };
        let jpeg = renderer.tile(spec, size).unwrap().jpeg().unwrap();
        let fragments =
            image::fragments(DEVICE_DIRECT, DISPLAY, &ImageConfig::default(), rect, &jpeg);
        assert!(fragments.is_ok(), "key {}: {fragments:?}", key.index);
    }
}

#[test]
fn a_panel_sized_tile_passes_the_writer_frame_check() {
    let geometry = display::captured_geometry();
    let rect = Target::Panel.rect(&geometry).unwrap();
    let size = TileSize {
        width: rect.width,
        height: rect.height,
    };
    let jpeg = Renderer::new()
        .tile(&KeySpec::default(), size)
        .unwrap()
        .jpeg()
        .unwrap();
    assert!(image::fragments(DEVICE_DIRECT, DISPLAY, &ImageConfig::default(), rect, &jpeg).is_ok());
}
