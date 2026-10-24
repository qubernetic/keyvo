//! The key renderer (spec §9): one snapshot per primitive, the JPEG frame
//! header, and the tile of a whole demo profile page.

use image::{ImageFormat, Rgb, RgbImage};
use keyvo_core::profile::Color;
use keyvo_core::render::{KeySpec, Renderer, TileSize};

/// The key size the Keypad publishes (spec §5.4).
const KEY: TileSize = TileSize {
    width: 118,
    height: 118,
};

const ORANGE: Color = Color::rgb(0xff, 0x80, 0x00);

fn decoded(jpeg: &[u8]) -> RgbImage {
    image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg)
        .unwrap()
        .to_rgb8()
}

fn close(a: Rgb<u8>, b: Rgb<u8>) -> bool {
    a.0.iter().zip(b.0).all(|(x, y)| x.abs_diff(y) <= 8)
}

#[test]
fn a_background_fills_the_tile_with_the_key_color() {
    let spec = KeySpec {
        background: Some(ORANGE),
        ..KeySpec::default()
    };
    let tile = Renderer::new().tile(&spec, KEY).unwrap();
    let picture = decoded(&tile.jpeg().unwrap());
    assert_eq!(picture.dimensions(), (118, 118));
    assert!(close(*picture.get_pixel(59, 59), Rgb([0xff, 0x80, 0x00])));
    insta::assert_binary_snapshot!("background.png", tile.png().unwrap());
}

#[test]
fn the_accent_is_a_bar_at_the_bottom_center() {
    let spec = KeySpec {
        accent: Some(Color::rgb(0x4e, 0x8b, 0xd6)),
        ..KeySpec::default()
    };
    let tile = Renderer::new().tile(&spec, KEY).unwrap();
    let picture = decoded(&tile.jpeg().unwrap());
    assert!(close(*picture.get_pixel(59, 107), Rgb([0x4e, 0x8b, 0xd6])));
    assert!(!close(*picture.get_pixel(10, 107), Rgb([0x4e, 0x8b, 0xd6])));
    assert!(!close(*picture.get_pixel(59, 59), Rgb([0x4e, 0x8b, 0xd6])));
    insta::assert_binary_snapshot!("accent.png", tile.png().unwrap());
}
