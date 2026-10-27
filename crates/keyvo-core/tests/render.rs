//! The key renderer (spec §9): one snapshot per primitive, the JPEG frame
//! header, and the tile of a whole demo profile page.

use std::path::Path;

use image::{ImageFormat, Rgb, RgbImage};
use keyvo_core::profile::{Color, Profile};
use keyvo_core::render::{self, Canvas, KeyPlace, KeySpec, Panel, Renderer, TileSize};
use keyvo_core::stack::Stack;

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

/// The whole lucide set is bundled, its metadata aliases with it (#142).
#[test]
fn a_lucide_icon_outside_the_old_subset_and_its_alias_draw_the_same_tile() {
    let house = icon("lucide:house", None);
    assert_ne!(house, icon("lucide:circle-question-mark", None));
    assert_eq!(icon("lucide:home", None), house);
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

/// The nine specs of page `page` of the demo profile alone.
fn demo_page(page: usize) -> Vec<KeySpec> {
    render::page(&Stack::alone(&demo(), page).unwrap(), 9)
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
    let specs = demo_page(1);
    assert_eq!(specs.len(), 9);
    assert_eq!(
        specs[0],
        KeySpec {
            background: None,
            accent,
            icon: Some("lucide:hammer".to_owned()),
            label: Some("Build".to_owned()),
            ..KeySpec::default()
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
    let general = demo_page(2);
    assert_eq!(general[4].label.as_deref(), Some("Settings"));
    assert!(general[0].icon.is_none());
}

#[test]
fn a_page_past_the_last_is_an_error() {
    let error = Stack::alone(&demo(), 3).unwrap_err();
    assert_eq!(error.to_string(), "profile `demo` has no page 3; it has 2");
    assert!(Stack::alone(&demo(), 0).is_err());
}

#[test]
fn the_demo_page_renders_on_the_captured_panel() {
    let renderer = Renderer::new();
    let specs = demo_page(1);
    let panel = renderer.panel(&specs, &captured_panel()).unwrap();
    assert_eq!(
        panel.size(),
        TileSize {
            width: 480,
            height: 480
        }
    );
    let rgb = panel.rgb();
    // Between the keys the panel shows the profile's panel background.
    let at = |x: usize, y: usize| &rgb[(y * 480 + x) * 3..(y * 480 + x) * 3 + 3];
    let background = renderer.panel_color(demo().profile.accent);
    assert_eq!(background, Color::rgb(16, 28, 43));
    assert_eq!(at(160, 160), &[background.r, background.g, background.b]);
    // The middle of key 5 is the key's own tile.
    let key5 = renderer.tile(&specs[4], KEY).unwrap().rgb();
    assert_eq!(
        at(181 + 10, 163 + 59),
        &key5[(59 * 118 + 10) * 3..(59 * 118 + 10) * 3 + 3]
    );
    insta::assert_binary_snapshot!("demo-page-1.png", panel.png().unwrap());
}

/// The union of the captured key rectangles: 435x434 at 23,6.
const UNION: KeyPlace = KeyPlace {
    x: 23,
    y: 6,
    size: TileSize {
        width: 435,
        height: 434,
    },
};

/// The RGB pixel at `x`, `y` of an RGB buffer `width` pixels wide.
fn pixel(rgb: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
    let at = (y * width + x) * 3;
    [rgb[at], rgb[at + 1], rgb[at + 2]]
}

/// A canvas of the demo page with every key drawn.
fn demo_canvas(renderer: &Renderer) -> Canvas {
    let specs = demo_page(1);
    let mut canvas = renderer
        .canvas(demo().profile.accent, &captured_panel())
        .unwrap();
    for (key, spec) in specs.iter().enumerate() {
        renderer.draw(&mut canvas, key, spec).unwrap();
    }
    canvas
}

#[test]
fn the_bounds_of_dirty_keys_cover_exactly_their_rectangles() {
    let panel = captured_panel();
    assert_eq!(panel.bounds(0..9), Some(UNION));
    assert_eq!(panel.bounds([4]), Some(panel.keys[4]));
    assert_eq!(
        panel.bounds([0, 4]),
        Some(KeyPlace {
            x: 24,
            y: 6,
            size: TileSize {
                width: 275,
                height: 275
            }
        })
    );
    // Keys the panel does not have are ignored.
    assert_eq!(panel.bounds([4, 20]), Some(panel.keys[4]));
    assert_eq!(panel.bounds([]), None);
}

#[test]
fn the_canvas_composites_the_page_and_crops_the_union() {
    let renderer = Renderer::new();
    let canvas = demo_canvas(&renderer);
    let union = canvas.crop(UNION).unwrap();
    assert_eq!(union.size(), UNION.size);
    assert_eq!(sof0(&union.jpeg().unwrap()), Some((435, 434)));
    // The crop is the composite panel the preview shows, cut to the union.
    let specs = demo_page(1);
    let whole = renderer.panel(&specs, &captured_panel()).unwrap().rgb();
    let crop = union.rgb();
    for (x, y) in [(0, 0), (1, 0), (137, 150), (200, 200), (434, 433)] {
        assert_eq!(
            pixel(&crop, 435, x, y),
            pixel(&whole, 480, x + 23, y + 6),
            "{x},{y}"
        );
    }
    // Key 1 starts at 24,6: the crop's first column is the panel background.
    let background = renderer.panel_color(demo().profile.accent);
    assert_eq!(
        pixel(&crop, 435, 0, 59),
        [background.r, background.g, background.b]
    );
    insta::assert_binary_snapshot!("canvas-union.png", union.png().unwrap());
}

#[test]
fn a_redrawn_key_replaces_its_old_tile_on_the_canvas() {
    let renderer = Renderer::new();
    let mut canvas = demo_canvas(&renderer);
    let pressed = KeySpec {
        background: Some(ORANGE),
        label: Some("Run".to_owned()),
        ..KeySpec::default()
    };
    renderer.draw(&mut canvas, 4, &pressed).unwrap();
    let key5 = captured_panel().keys[4];
    let crop = canvas.crop(key5).unwrap();
    // The same key drawn on a fresh canvas: nothing of the old tile stays,
    // not even in the rounded corners.
    let mut fresh = renderer
        .canvas(demo().profile.accent, &captured_panel())
        .unwrap();
    renderer.draw(&mut fresh, 4, &pressed).unwrap();
    assert_eq!(crop.rgb(), fresh.crop(key5).unwrap().rgb());
    // The other keys stay as they were.
    let other = demo_canvas(&renderer);
    let key1 = captured_panel().keys[0];
    assert_eq!(
        canvas.crop(key1).unwrap().rgb(),
        other.crop(key1).unwrap().rgb()
    );
    insta::assert_binary_snapshot!("canvas-key-5.png", crop.png().unwrap());
}

#[test]
fn a_crop_or_key_outside_the_canvas_is_an_error() {
    let renderer = Renderer::new();
    let mut canvas = renderer.canvas(None, &captured_panel()).unwrap();
    let outside = KeyPlace {
        x: 400,
        y: 400,
        size: KEY,
    };
    assert_eq!(
        canvas.crop(outside).unwrap_err().to_string(),
        "118x118 at 400,400 is outside the 480x480 canvas"
    );
    assert_eq!(
        renderer
            .draw(&mut canvas, 9, &KeySpec::default())
            .unwrap_err()
            .to_string(),
        "no key 10 on a panel of 9 keys"
    );
}

#[test]
fn tiles_are_encoded_at_quality_100() {
    let jpeg = Renderer::new()
        .tile(&KeySpec::default(), KEY)
        .unwrap()
        .jpeg()
        .unwrap();
    // Every DQT segment holds 8-bit tables of ones.
    let mut at = 2;
    let mut tables = 0;
    while let [0xff, marker, len0, len1, ..] = jpeg[at..] {
        let len = usize::from(u16::from_be_bytes([len0, len1]));
        if marker == 0xdb {
            for table in jpeg[at + 4..at + 2 + len].chunks(65) {
                assert_eq!(table[0] >> 4, 0, "8-bit precision");
                assert!(table[1..].iter().all(|&q| q == 1));
                tables += 1;
            }
        }
        if marker == 0xda {
            break;
        }
        at += 2 + len;
    }
    assert_eq!(tables, 2);
}

#[test]
fn every_key_tile_is_a_baseline_jpeg_of_the_tile_size() {
    let renderer = Renderer::new();
    for spec in demo_page(1) {
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

mod looks {
    use image::Rgb;
    use keyvo_core::feedback::Look;
    use keyvo_core::profile::{Color, Key, KeyState};
    use keyvo_core::render::{KeySpec, Renderer, Theme};

    use super::{KEY, count, decoded};

    const ACCENT: Color = Color::rgb(0x4e, 0x8b, 0xd6);

    /// Demo key 1: icon, label and the profile accent.
    fn build_key() -> Key {
        Key {
            icon: Some("lucide:hammer".to_owned()),
            label: Some("Build".to_owned()),
            ..Key::default()
        }
    }

    fn spec(look: Look) -> KeySpec {
        KeySpec::from_key(Some(&build_key()), Some(ACCENT)).with_look(Some(&build_key()), look)
    }

    fn rgb(color: Color) -> Rgb<u8> {
        Rgb([color.r, color.g, color.b])
    }

    /// Pixels close to `color` in the badge corner (top right).
    fn in_corner(spec: &KeySpec, color: Color) -> usize {
        let picture = decoded(&Renderer::new().tile(spec, KEY).unwrap().jpeg().unwrap());
        picture
            .enumerate_pixels()
            .filter(|(x, y, p)| *x >= 80 && *y < 38 && super::close(**p, rgb(color)))
            .count()
    }

    #[test]
    fn idle_is_the_key_itself() {
        let idle = spec(Look::Idle);
        assert_eq!(idle, KeySpec::from_key(Some(&build_key()), Some(ACCENT)));
        assert_eq!(idle.look, Look::Idle);
    }

    #[test]
    fn running_dims_the_icon_and_draws_an_accent_arc() {
        let renderer = Renderer::new();
        let idle = decoded(
            &renderer
                .tile(&spec(Look::Idle), KEY)
                .unwrap()
                .jpeg()
                .unwrap(),
        );
        let running_tile = renderer.tile(&spec(Look::Running), KEY).unwrap();
        let running = decoded(&running_tile.jpeg().unwrap());
        // The icon square (centered at 59, 42, 47 wide) is white when idle
        // and has no white left when running; the label stays white.
        let white = Rgb([0xff, 0xff, 0xff]);
        let white_icon = |picture: &image::RgbImage| {
            picture
                .enumerate_pixels()
                .filter(|(x, y, p)| {
                    (35..84).contains(x) && (18..66).contains(y) && super::close(**p, white)
                })
                .count()
        };
        assert!(white_icon(&idle) > 200, "{}", white_icon(&idle));
        assert_eq!(white_icon(&running), 0);
        assert!(count(&running, white) > 100, "label");
        // The arc starts at the top and turns clockwise: accent pixels in
        // the upper right quarter around the icon, none in the upper left.
        let accent_in = |x: std::ops::Range<u32>, y: std::ops::Range<u32>| {
            running
                .enumerate_pixels()
                .filter(|(px, py, p)| {
                    x.contains(px) && y.contains(py) && super::close(**p, rgb(ACCENT))
                })
                .count()
        };
        assert!(accent_in(60..100, 5..45) > 30);
        assert_eq!(accent_in(15..55, 5..45), 0);
        insta::assert_binary_snapshot!("running.png", running_tile.png().unwrap());
    }

    #[test]
    fn the_spinner_angle_turns_the_arc() {
        let renderer = Renderer::new();
        let start = spec(Look::Running);
        let turned = KeySpec {
            spinner: 180,
            ..start.clone()
        };
        assert_ne!(
            renderer.tile(&start, KEY).unwrap(),
            renderer.tile(&turned, KEY).unwrap()
        );
    }

    #[test]
    fn a_long_run_shows_a_static_badge_instead_of_the_arc() {
        let theme = Theme::default();
        let long = spec(Look::RunningLong);
        assert!(in_corner(&long, theme.running) > 100);
        let tile = Renderer::new().tile(&long, KEY).unwrap();
        insta::assert_binary_snapshot!("running-long.png", tile.png().unwrap());
    }

    #[test]
    fn success_shows_a_green_badge() {
        let theme = Theme::default();
        let success = spec(Look::Success);
        assert!(in_corner(&success, theme.success) > 100);
        assert_eq!(in_corner(&success, theme.error), 0);
        assert_eq!(in_corner(&spec(Look::Idle), theme.success), 0);
        let tile = Renderer::new().tile(&success, KEY).unwrap();
        insta::assert_binary_snapshot!("success.png", tile.png().unwrap());
    }

    #[test]
    fn error_shows_a_red_badge() {
        let theme = Theme::default();
        let error = spec(Look::Error);
        assert!(in_corner(&error, theme.error) > 100);
        assert_eq!(in_corner(&error, theme.success), 0);
        let tile = Renderer::new().tile(&error, KEY).unwrap();
        insta::assert_binary_snapshot!("error.png", tile.png().unwrap());
    }

    #[test]
    fn a_key_state_overrides_the_built_in_look() {
        let mut key = build_key();
        key.states.insert(
            "error".to_owned(),
            KeyState {
                label: Some("Failed".to_owned()),
                color: Some(Color::rgb(0x5a, 0x1e, 0x1e)),
                badge: Some("!".to_owned()),
                icon: Some("lucide:bug".to_owned()),
            },
        );
        let error = KeySpec::from_key(Some(&key), Some(ACCENT)).with_look(Some(&key), Look::Error);
        assert_eq!(error.label.as_deref(), Some("Failed"));
        assert_eq!(error.icon.as_deref(), Some("lucide:bug"));
        assert_eq!(error.background, Some(Color::rgb(0x5a, 0x1e, 0x1e)));
        assert_eq!(error.badge.as_deref(), Some("!"));
        // The badge keeps the look's color; only its content changes.
        assert!(in_corner(&error, Theme::default().error) > 100);
        // Other looks of the same key are untouched.
        let success =
            KeySpec::from_key(Some(&key), Some(ACCENT)).with_look(Some(&key), Look::Success);
        assert_eq!(success.label.as_deref(), Some("Build"));
        let tile = Renderer::new().tile(&error, KEY).unwrap();
        insta::assert_binary_snapshot!("error-override.png", tile.png().unwrap());
    }

    #[test]
    fn a_custom_state_badge_is_drawn_on_an_idle_key() {
        let state = KeyState {
            badge: Some("3".to_owned()),
            ..KeyState::default()
        };
        let mut dirty = KeySpec::from_key(Some(&build_key()), Some(ACCENT));
        dirty.apply(&state);
        assert_eq!(dirty.badge.as_deref(), Some("3"));
        assert!(in_corner(&dirty, Theme::default().badge) > 100);
    }

    #[test]
    fn a_running_key_without_an_icon_or_accent_still_spins() {
        let label_only = Key {
            label: Some("Test".to_owned()),
            ..Key::default()
        };
        let running =
            KeySpec::from_key(Some(&label_only), None).with_look(Some(&label_only), Look::Running);
        let idle = KeySpec::from_key(Some(&label_only), None);
        let renderer = Renderer::new();
        assert_ne!(
            renderer.tile(&running, KEY).unwrap(),
            renderer.tile(&idle, KEY).unwrap()
        );
        let blank = KeySpec::default().with_look(None, Look::Error);
        assert!(in_corner(&blank, Theme::default().error) > 100);
    }
}
