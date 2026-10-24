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

/// Pixels of `picture` within 8 of `color` per channel.
fn count(picture: &RgbImage, color: Rgb<u8>) -> usize {
    picture.pixels().filter(|&&p| close(p, color)).count()
}

fn icon(reference: &str, background: Option<Color>) -> keyvo_core::render::Tile {
    let spec = KeySpec {
        icon: Some(reference.to_owned()),
        background,
        ..KeySpec::default()
    };
    Renderer::new().tile(&spec, KEY).unwrap()
}

#[test]
fn an_icon_is_drawn_in_white_on_a_dark_key() {
    let tile = icon("lucide:hammer", None);
    let picture = decoded(&tile.jpeg().unwrap());
    let white = count(&picture, Rgb([0xff, 0xff, 0xff]));
    assert!(white > 300, "{white} white pixels");
    insta::assert_binary_snapshot!("icon.png", tile.png().unwrap());
}

#[test]
fn an_icon_is_drawn_in_black_on_a_light_key() {
    let tile = icon("lucide:hammer", Some(Color::rgb(0xf0, 0xe0, 0x60)));
    let picture = decoded(&tile.jpeg().unwrap());
    assert!(count(&picture, Rgb([0x11, 0x11, 0x11])) > 300);
    assert_eq!(count(&picture, Rgb([0xff, 0xff, 0xff])), 0);
}

#[test]
fn an_unknown_icon_draws_the_help_icon() {
    let help = icon("lucide:circle-question-mark", None);
    for reference in ["lucide:no-such-icon", "phosphor:house", "hammer"] {
        assert_eq!(icon(reference, None), help, "{reference}");
    }
    assert_ne!(icon("lucide:hammer", None), help);
}

#[test]
fn deprecated_lucide_names_resolve_to_the_current_icon() {
    assert_eq!(
        icon("lucide:git-commit", None),
        icon("lucide:git-commit-horizontal", None)
    );
}

#[test]
fn icon_lookup_explains_what_is_missing() {
    use keyvo_core::render::icon::{self, IconError};
    assert!(icon::lookup("lucide:hammer").unwrap().contains("<svg"));
    assert_eq!(
        icon::lookup("lucide:nope").unwrap_err(),
        IconError::UnknownIcon("lucide:nope".to_owned())
    );
    assert_eq!(
        icon::lookup("file:icons/x.png").unwrap_err(),
        IconError::UnsupportedSource("file".to_owned())
    );
    assert_eq!(
        icon::lookup("hammer").unwrap_err(),
        IconError::NoSource("hammer".to_owned())
    );
    assert_eq!(
        icon::lookup("lucide:nope").unwrap_err().to_string(),
        "no bundled icon `lucide:nope`"
    );
    assert!(icon::names().any(|name| name == "lucide:terminal"));
}
