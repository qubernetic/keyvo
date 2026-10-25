//! `keyvo fill`: paints a solid color or a PNG on one key, on every key or
//! on the whole panel, at the rectangles the device publishes (spec §5.4,
//! §5.5). The image is scaled to each rectangle and sent as a baseline JPEG.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ImageFormat, Rgb, RgbImage};
use keyvo_core::clock::Clock;
use keyvo_core::hid::HidTransport;
use keyvo_hid::display::Geometry;
use keyvo_hid::image::{self as lcd, ImageConfig, Rect, Target};

use crate::session::{Inbound, Session};

/// JPEG quality of a fill, inside the 85 to 95 of spec §5.5.
const JPEG_QUALITY: u8 = 90;

/// What to paint.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// One color everywhere.
    Color(Rgb<u8>),
    /// A picture, scaled to each target.
    Picture(RgbImage),
}

impl Source {
    /// `#rrggbb` or `rrggbb` is a color; anything else is the path of a PNG.
    ///
    /// # Errors
    ///
    /// When the PNG cannot be read or decoded.
    pub fn parse(argument: &str) -> anyhow::Result<Self> {
        if let Some(color) = parse_color(argument) {
            return Ok(Self::Color(color));
        }
        let path = Path::new(argument);
        let bytes = std::fs::read(path).with_context(|| {
            format!("`{argument}` is neither a #rrggbb color nor a readable PNG")
        })?;
        Self::png(&bytes).with_context(|| format!("cannot decode {}", path.display()))
    }

    /// A picture from PNG bytes; an alpha channel is dropped.
    ///
    /// # Errors
    ///
    /// When the bytes are not a PNG.
    pub fn png(bytes: &[u8]) -> anyhow::Result<Self> {
        let picture = image::load_from_memory_with_format(bytes, ImageFormat::Png)?;
        Ok(Self::Picture(picture.to_rgb8()))
    }

    /// The baseline JPEG of this source at `width` x `height`.
    ///
    /// # Errors
    ///
    /// The encoder errors.
    pub fn jpeg(&self, width: u16, height: u16) -> anyhow::Result<Vec<u8>> {
        let (width, height) = (u32::from(width), u32::from(height));
        let picture = match self {
            Self::Color(color) => RgbImage::from_pixel(width, height, *color),
            Self::Picture(picture) => {
                image::imageops::resize(picture, width, height, FilterType::Triangle)
            }
        };
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY).encode_image(&picture)?;
        Ok(jpeg)
    }
}

fn parse_color(text: &str) -> Option<Rgb<u8>> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    Some(Rgb([channel(0)?, channel(2)?, channel(4)?]))
}

/// Where to paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// One key, numbered from 1 in the geometry's order.
    Key(u8),
    /// Every key of the geometry, one write each.
    All,
    /// The whole panel in one write.
    Panel,
}

/// `rect` grown by `margin` pixels on all four sides and clamped to the
/// panel of `geometry` (`0..width`, `0..height`) on each side separately.
/// The clamp never cuts into `rect` itself, so a margin of 0 is the
/// published rectangle whatever the panel size.
#[must_use]
pub fn grow(rect: Rect, margin: u16, geometry: &Geometry) -> Rect {
    let right = rect.x.saturating_add(rect.width);
    let bottom = rect.y.saturating_add(rect.height);
    let x = rect.x.saturating_sub(margin);
    let y = rect.y.saturating_sub(margin);
    let right = right.saturating_add(margin).min(geometry.width.max(right));
    let bottom = bottom
        .saturating_add(margin)
        .min(geometry.height.max(bottom));
    Rect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

/// Paints `source` at `placement` and reports each write on `out`; the
/// keep-alive runs between the writes. Every key rectangle is first grown
/// by `margin` ([`grow`]); the panel is painted as published.
///
/// # Errors
///
/// [`lcd::ImageError`] for a key the geometry does not list or an image
/// the writer refuses, the transport errors and write errors on `out`.
pub fn fill<T: HidTransport, C: Clock>(
    session: &mut Session<T, C>,
    source: &Source,
    placement: Placement,
    margin: u16,
    config: &ImageConfig,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let geometry = session.claim.geometry.clone();
    let targets: Vec<_> = match placement {
        Placement::Key(key) => vec![Target::Key(key)],
        Placement::All => geometry.keys.iter().map(|k| Target::Key(k.index)).collect(),
        Placement::Panel => vec![Target::Panel],
    };
    for target in targets {
        let rect = match target {
            Target::Key(_) => grow(target.rect(&geometry)?, margin, &geometry),
            Target::Panel => target.rect(&geometry)?,
        };
        let jpeg = source.jpeg(rect.width, rect.height)?;
        let written = lcd::write(&mut session.hidpp, config, rect, &jpeg)?;
        let label = match target {
            Target::Key(key) => format!("key {key}"),
            Target::Panel => "panel".to_owned(),
        };
        let plural = if written.fragments == 1 { "" } else { "s" };
        writeln!(
            out,
            "{label}  {}x{} at {},{}  {} bytes, {} fragment{plural}, {} acked",
            rect.width,
            rect.height,
            rect.x,
            rect.y,
            jpeg.len(),
            written.fragments,
            written.acked
        )?;
        session.tick()?;
    }
    Ok(())
}

/// Keeps the claim for `duration` after a fill: pings when due and sleeps
/// in between, reading the time from `clock`. After every ping it reads
/// what the device sent; a lost claim is taken back and `repaint` paints
/// the image again (spec §5.3), so the panel is back within one period.
///
/// # Errors
///
/// The ping's request errors, the reclaim's errors and those of `repaint`.
pub fn hold<T: HidTransport, C: Clock>(
    session: &mut Session<T, C>,
    clock: &impl Clock,
    duration: Duration,
    mut sleep: impl FnMut(Duration),
    mut repaint: impl FnMut(&mut Session<T, C>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let start = clock.now();
    loop {
        session.tick()?;
        let mut reclaimed = false;
        while let Some(inbound) = session.next_frame(Duration::ZERO)? {
            reclaimed |= inbound == Inbound::Reclaimed;
        }
        if reclaimed {
            repaint(session)?;
        }
        let elapsed = clock.now().saturating_duration_since(start);
        let Some(left) = duration.checked_sub(elapsed).filter(|left| !left.is_zero()) else {
            return Ok(());
        };
        sleep(session.until_due().min(left));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tests::{exchanges, fixture, replay};
    use keyvo_core::clock::FakeClock;
    use keyvo_core::hid::to_hex;
    use keyvo_hid::hidpp::DEVICE_DIRECT;
    use keyvo_hid::image::ImageError;
    use keyvo_hid::keep_alive::{DEFAULT_PERIOD, KeepAliveTiming};
    use std::fmt::Write as _;

    const ORANGE: Rgb<u8> = Rgb([0xff, 0x80, 0x00]);
    /// The display's VLP index in the captures.
    const DISPLAY: u8 = 2;

    pub(crate) fn geometry() -> Geometry {
        Session::open(
            replay(&fixture("display-claim.txt")),
            FakeClock::new(),
            None,
        )
        .unwrap()
        .claim
        .geometry
    }

    /// For every target the fragments of `source`'s JPEG with their ACKs.
    pub(crate) fn paints(source: &Source, targets: &[Target], config: &ImageConfig) -> String {
        let geometry = geometry();
        let rects: Vec<_> = targets.iter().map(|t| t.rect(&geometry).unwrap()).collect();
        paints_at(source, &rects, config)
    }

    /// For every rectangle the fragments of `source`'s JPEG, each answered
    /// by its ACK in the device's layout
    /// (`13 ff 02 <fn|swid> c<index> 00 <01 on the last fragment, else 00>`,
    /// `image-acks.txt`).
    pub(crate) fn paints_at(source: &Source, rects: &[Rect], config: &ImageConfig) -> String {
        let mut script = String::new();
        for &rect in rects {
            let jpeg = source.jpeg(rect.width, rect.height).unwrap();
            for fragment in lcd::fragments(DEVICE_DIRECT, DISPLAY, config, rect, &jpeg).unwrap() {
                let index = fragment[4] & 0x0f;
                let last = u8::from(fragment[4] & 0x40 != 0);
                let mut ack = vec![0x13, 0xff, DISPLAY, fragment[3], 0xc0 | index, 0x00, last];
                ack.resize(32, 0);
                writeln!(script, "> {}\n< {}", to_hex(&fragment), to_hex(&ack)).unwrap();
            }
        }
        script
    }

    fn run(source: &Source, placement: Placement, targets: &[Target]) -> anyhow::Result<String> {
        let geometry = geometry();
        let rects: Vec<_> = targets.iter().map(|t| t.rect(&geometry).unwrap()).collect();
        run_at(source, placement, 0, &rects)
    }

    /// `fill` with `margin`, against a script that expects `rects`.
    fn run_at(
        source: &Source,
        placement: Placement,
        margin: u16,
        rects: &[Rect],
    ) -> anyhow::Result<String> {
        let config = ImageConfig::default();
        let script = fixture("display-claim.txt") + &paints_at(source, rects, &config);
        let mut session = Session::open(replay(&script), FakeClock::new(), None)?;
        let mut out = Vec::new();
        fill(&mut session, source, placement, margin, &config, &mut out)?;
        assert!(session.hidpp.transport_mut().is_done());
        Ok(String::from_utf8(out)?)
    }

    #[test]
    fn a_margin_grows_every_key_on_all_four_sides() {
        let geometry = geometry();
        let grown: Vec<_> = geometry
            .keys
            .iter()
            .map(|k| Rect {
                x: k.x - 4,
                y: k.y - 4,
                width: k.width + 8,
                height: k.height + 8,
            })
            .collect();
        let text = run_at(&Source::Color(ORANGE), Placement::All, 4, &grown).unwrap();
        assert!(text.starts_with("key 1  126x126 at 20,2  "), "{text}");
        assert_eq!(text.lines().count(), 9);
    }

    #[test]
    fn a_margin_is_clamped_to_the_panel_on_that_side_only() {
        let source = Source::Color(ORANGE);
        // Key 1 at 24,6: the top edge stops at 0, the others grow by 8.
        let top = Rect {
            x: 16,
            y: 0,
            width: 134,
            height: 132,
        };
        let text = run_at(&source, Placement::Key(1), 8, &[top]).unwrap();
        assert!(text.starts_with("key 1  134x132 at 16,0  "), "{text}");
        // Key 9 at 340,321: right and bottom stop at 480, left and top grow.
        let corner = Rect {
            x: 295,
            y: 276,
            width: 185,
            height: 204,
        };
        let text = run_at(&source, Placement::Key(9), 45, &[corner]).unwrap();
        assert!(text.starts_with("key 9  185x204 at 295,276  "), "{text}");
    }

    #[test]
    fn the_panel_ignores_the_margin() {
        let geometry = geometry();
        let panel = Target::Panel.rect(&geometry).unwrap();
        let text = run_at(&Source::Color(ORANGE), Placement::Panel, 4, &[panel]).unwrap();
        assert!(text.starts_with("panel  435x434 at 23,6  "), "{text}");
    }

    #[test]
    fn grow_never_shrinks_a_key_past_a_too_small_panel() {
        let geometry = Geometry {
            width: 100,
            height: 100,
            ..geometry()
        };
        let key = Target::Key(9).rect(&geometry).unwrap();
        assert_eq!(
            grow(key, 4, &geometry),
            Rect {
                x: 336,
                y: 317,
                width: 122,
                height: 122,
            }
        );
    }

    fn decoded(jpeg: &[u8]) -> RgbImage {
        image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg)
            .unwrap()
            .to_rgb8()
    }

    fn close(a: Rgb<u8>, b: Rgb<u8>) -> bool {
        a.0.iter().zip(b.0).all(|(x, y)| x.abs_diff(y) <= 8)
    }

    #[test]
    fn colors_parse_with_or_without_the_hash() {
        for text in ["#ff8000", "ff8000", "FF8000"] {
            assert_eq!(Source::parse(text).unwrap(), Source::Color(ORANGE));
        }
        let error = Source::parse("#ff80").unwrap_err();
        assert!(
            error.to_string().contains("neither a #rrggbb color"),
            "{error}"
        );
    }

    #[test]
    fn a_color_becomes_a_jpeg_of_the_rectangle() {
        let jpeg = Source::Color(ORANGE).jpeg(118, 118).unwrap();
        assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
        let picture = decoded(&jpeg);
        assert_eq!(picture.dimensions(), (118, 118));
        assert!(close(*picture.get_pixel(59, 59), ORANGE));
    }

    #[test]
    fn a_png_is_scaled_to_the_rectangle() {
        let mut png = Vec::new();
        RgbImage::from_pixel(4, 2, Rgb([0, 0, 0xff]))
            .write_to(&mut std::io::Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), &png).unwrap();
        let source = Source::parse(file.path().to_str().unwrap()).unwrap();
        let picture = decoded(&source.jpeg(435, 434).unwrap());
        assert_eq!(picture.dimensions(), (435, 434));
        assert!(close(*picture.get_pixel(200, 200), Rgb([0, 0, 0xff])));

        std::fs::write(file.path(), b"not a png").unwrap();
        let error = Source::parse(file.path().to_str().unwrap()).unwrap_err();
        assert!(error.to_string().starts_with("cannot decode"), "{error}");
    }

    #[test]
    fn fill_paints_one_key_at_its_published_rectangle() {
        let source = Source::Color(ORANGE);
        let text = run(&source, Placement::Key(5), &[Target::Key(5)]).unwrap();
        let len = source.jpeg(118, 118).unwrap().len();
        assert_eq!(
            text,
            format!("key 5  118x118 at 181,163  {len} bytes, 1 fragment, 1 acked\n")
        );
    }

    #[test]
    fn fill_all_paints_every_key_of_the_geometry_in_order() {
        let targets: Vec<_> = (1..=9).map(Target::Key).collect();
        let text = run(&Source::Color(ORANGE), Placement::All, &targets).unwrap();
        let keys: Vec<_> = text
            .lines()
            .map(|l| l.split("  ").next().unwrap())
            .collect();
        assert_eq!(
            keys,
            (1..=9).map(|k| format!("key {k}")).collect::<Vec<_>>()
        );
        assert!(text.lines().all(|l| l.ends_with("1 acked")), "{text}");
    }

    #[test]
    fn fill_panel_paints_the_bounding_box_of_the_keys() {
        let text = run(&Source::Color(ORANGE), Placement::Panel, &[Target::Panel]).unwrap();
        assert!(text.starts_with("panel  435x434 at 23,6  "), "{text}");
    }

    #[test]
    fn a_key_outside_the_geometry_is_refused() {
        let error = run(&Source::Color(ORANGE), Placement::Key(10), &[]).unwrap_err();
        assert_eq!(
            error.downcast_ref::<ImageError>(),
            Some(&ImageError::NoSuchKey { key: 10, count: 9 })
        );
    }

    #[test]
    fn hold_keeps_pinging_until_the_time_is_up() {
        let pings = fixture("keep-alive-ping.txt");
        let script = format!(
            "{}{}{}",
            exchanges(&pings, 0..2),
            fixture("display-claim.txt"),
            exchanges(&pings, 2..4),
        );
        let clock = FakeClock::new();
        let mut session =
            Session::open(replay(&script), &clock, Some(KeepAliveTiming::default())).unwrap();
        let mut sleeps = Vec::new();
        hold(
            &mut session,
            &clock,
            2 * DEFAULT_PERIOD,
            |wait| {
                sleeps.push(wait);
                clock.advance(wait);
            },
            |_| panic!("nothing was lost"),
        )
        .unwrap();
        assert_eq!(sleeps, [DEFAULT_PERIOD, DEFAULT_PERIOD]);
        assert!(session.hidpp.transport_mut().is_done());
    }

    #[test]
    fn hold_repaints_the_image_after_a_reclaim() {
        let config = ImageConfig::default();
        let source = Source::Color(ORANGE);
        let panel = paints(&source, &[Target::Panel], &config);
        // The paint, the expiry's two events after its last ACK, the
        // reclaim, the same paint again.
        let script = format!(
            "{}{panel}{}{}{panel}",
            fixture("display-claim.txt"),
            fixture("claim-lost.txt"),
            fixture("display-reclaim.txt"),
        );
        let clock = FakeClock::new();
        let mut session = Session::open(replay(&script), &clock, None).unwrap();
        let mut out = Vec::new();
        fill(
            &mut session,
            &source,
            Placement::Panel,
            0,
            &config,
            &mut out,
        )
        .unwrap();
        let mut repaints = 0;
        hold(
            &mut session,
            &clock,
            DEFAULT_PERIOD,
            |wait| clock.advance(wait),
            |session| {
                repaints += 1;
                fill(session, &source, Placement::Panel, 0, &config, &mut out)
            },
        )
        .unwrap();
        assert_eq!(repaints, 1);
        assert!(session.hidpp.transport_mut().is_done());
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().filter(|l| l.starts_with("panel")).count(), 2);
    }
}
