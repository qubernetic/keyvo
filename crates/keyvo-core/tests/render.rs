//! The key renderer (spec §9): one snapshot per primitive, the JPEG frame
//! header, and the tile of a whole demo profile page.

use std::path::Path;

use image::{ImageFormat, Rgb, RgbImage};
use keyvo_core::profile::{Color, Profile};
use keyvo_core::render::{self, KeyPlace, KeySpec, Panel, Renderer, TileSize};

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

fn labeled(label: &str, icon: Option<&str>) -> keyvo_core::render::Tile {
    let spec = KeySpec {
        label: Some(label.to_owned()),
        icon: icon.map(str::to_owned),
        ..KeySpec::default()
    };
    Renderer::new().tile(&spec, KEY).unwrap()
}

/// White pixels in the rows `rows` of `tile`.
fn white_in_rows(tile: &keyvo_core::render::Tile, rows: std::ops::Range<u32>) -> usize {
    let picture = decoded(&tile.jpeg().unwrap());
    picture
        .enumerate_pixels()
        .filter(|(_, y, p)| rows.contains(y) && close(**p, Rgb([0xff, 0xff, 0xff])))
        .count()
}

#[test]
fn a_label_alone_is_centered() {
    let tile = labeled("Build", None);
    assert!(white_in_rows(&tile, 40..65) > 100);
    assert_eq!(white_in_rows(&tile, 0..30), 0);
    assert_eq!(white_in_rows(&tile, 80..118), 0);
    insta::assert_binary_snapshot!("label.png", tile.png().unwrap());
}

#[test]
fn a_label_goes_below_the_icon() {
    let tile = labeled("Commit", Some("lucide:git-commit"));
    assert!(white_in_rows(&tile, 20..60) > 100, "icon");
    assert!(white_in_rows(&tile, 80..100) > 100, "label");
    insta::assert_binary_snapshot!("icon-label.png", tile.png().unwrap());
}

#[test]
fn a_long_label_shrinks_then_is_cut_with_an_ellipsis() {
    let renderer = Renderer::new();
    let short = renderer.fit_label("Build", KEY).unwrap();
    assert_eq!(short.text, "Build");
    let longer = renderer.fit_label("Deployment", KEY).unwrap();
    assert_eq!(longer.text, "Deployment");
    assert!(longer.font_size < short.font_size, "{longer:?} {short:?}");
    let cut = renderer
        .fit_label("Run every integration test", KEY)
        .unwrap();
    assert!(cut.text.starts_with("Run every"), "{cut:?}");
    assert!(cut.text.ends_with('\u{2026}'), "{cut:?}");
    assert!(renderer.fit_label("  ", KEY).is_none());
    insta::assert_binary_snapshot!(
        "long-label.png",
        labeled("Run every integration test", None).png().unwrap()
    );
}

#[test]
fn markup_in_a_label_is_text() {
    let renderer = Renderer::new();
    let fitted = renderer.fit_label("a<b & c>", KEY).unwrap();
    assert_eq!(fitted.text, "a<b & c>");
    assert!(white_in_rows(&labeled("a<b & c>", None), 40..65) > 50);
}

fn demo() -> Profile {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles/valid/demo.toml");
    Profile::from_toml(&std::fs::read_to_string(&path).unwrap(), &path).unwrap()
}

/// The Keypad's panel as captured (spec §5.4): 480x480, nine 118x118 keys.
fn captured_panel() -> Panel {
    let origins = [
        (24, 6),
        (182, 7),
        (340, 6),
        (23, 164),
        (181, 163),
        (339, 164),
        (23, 322),
        (181, 321),
        (340, 321),
    ];
    Panel {
        size: TileSize {
            width: 480,
            height: 480,
        },
        keys: origins
            .iter()
            .map(|&(x, y)| KeyPlace { x, y, size: KEY })
            .collect(),
    }
}

#[test]
fn a_page_gives_one_spec_per_key_with_the_profile_accent() {
    let accent = Some(Color::rgb(0x4e, 0x8b, 0xd6));
    let specs = render::page(&demo(), 1, 9).unwrap();
    assert_eq!(specs.len(), 9);
    assert_eq!(
        specs[0],
        KeySpec {
            background: None,
            accent,
            icon: Some("lucide:hammer".to_owned()),
            label: Some("Build".to_owned()),
        }
    );
    assert_eq!(specs[1].background, Some(Color::rgb(0x2f, 0x6f, 0x3e)));
    assert_eq!(
        specs[6],
        KeySpec {
            accent,
            ..KeySpec::default()
        }
    );
    let general = render::page(&demo(), 2, 9).unwrap();
    assert_eq!(general[4].label.as_deref(), Some("Settings"));
    assert!(general[0].icon.is_none());
}

#[test]
fn a_page_past_the_last_is_an_error() {
    let error = render::page(&demo(), 3, 9).unwrap_err();
    assert_eq!(error.to_string(), "profile `demo` has no page 3; it has 2");
    assert!(render::page(&demo(), 0, 9).is_err());
}

#[test]
fn the_demo_page_renders_on_the_captured_panel() {
    let renderer = Renderer::new();
    let specs = render::page(&demo(), 1, 9).unwrap();
    let panel = renderer.panel(&specs, &captured_panel()).unwrap();
    assert_eq!(
        panel.size(),
        TileSize {
            width: 480,
            height: 480
        }
    );
    let rgb = panel.rgb();
    // Between the keys the panel stays black.
    let at = |x: usize, y: usize| &rgb[(y * 480 + x) * 3..(y * 480 + x) * 3 + 3];
    assert_eq!(at(160, 160), &[0, 0, 0]);
    // The middle of key 5 is the key's own tile.
    let key5 = renderer.tile(&specs[4], KEY).unwrap().rgb();
    assert_eq!(
        at(181 + 10, 163 + 59),
        &key5[(59 * 118 + 10) * 3..(59 * 118 + 10) * 3 + 3]
    );
    insta::assert_binary_snapshot!("demo-page-1.png", panel.png().unwrap());
}

#[test]
fn every_key_tile_is_a_baseline_jpeg_of_the_tile_size() {
    let renderer = Renderer::new();
    for spec in render::page(&demo(), 1, 9).unwrap() {
        let jpeg = renderer.tile(&spec, KEY).unwrap().jpeg().unwrap();
        assert_eq!(sof0(&jpeg), Some((118, 118)), "{spec:?}");
    }
    let panel = TileSize {
        width: 435,
        height: 434,
    };
    let jpeg = renderer
        .tile(&KeySpec::default(), panel)
        .unwrap()
        .jpeg()
        .unwrap();
    assert_eq!(sof0(&jpeg), Some((435, 434)));
}

/// Width and height of the baseline frame header (SOF0), walking the
/// marker segments; `None` without one.
fn sof0(jpeg: &[u8]) -> Option<(u16, u16)> {
    let mut at = 2;
    while let [0xff, marker, len0, len1, ..] = *jpeg.get(at..)? {
        if marker == 0xc0 {
            let header = jpeg.get(at + 5..at + 9)?;
            return Some((
                u16::from_be_bytes([header[2], header[3]]),
                u16::from_be_bytes([header[0], header[1]]),
            ));
        }
        at += 2 + usize::from(u16::from_be_bytes([len0, len1]));
    }
    None
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
