//! Golden tests: `keyvo-hid` replays the hcooper capture frames in
//! `tests/protocol-fixtures/hid/` through `FakeHidTransport` and must write
//! exactly the captured requests and decode the captured replies.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use keyvo_core::clock::FakeClock;
use keyvo_core::hid::FakeHidTransport;
use keyvo_hid::display::{self, DisplayMode};
use keyvo_hid::firmware::{self, FirmwareEntity, FirmwareKind};
use keyvo_hid::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, Plane, SoftwareId};
use keyvo_hid::image::{self, ImageConfig, ImageError, Rect, Target, Written};
use keyvo_hid::input::{Control, Input, InputEvent};
use keyvo_hid::keep_alive::{self, KeepAlive, KeepAliveTiming, TimeoutRange};
use keyvo_hid::reprog::{self, ControlInfo, DivertTiming};
use keyvo_hid::root::{FeatureInfo, ProtocolVersion, VlpFeatureInfo, VlpProtocol, feature};

/// Options+ tags the captured requests with software id 0xe.
const CAPTURE_SWID: u8 = 0xe;
const REPLY_TIMEOUT: Duration = Duration::from_secs(1);

fn replay(script: &str) -> Hidpp<FakeHidTransport> {
    Hidpp::new(
        FakeHidTransport::from_script(script).unwrap(),
        DEVICE_DIRECT,
        SoftwareId::new(CAPTURE_SWID).unwrap(),
        REPLY_TIMEOUT,
    )
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../../tests/protocol-fixtures/hid/", $name))
    };
}

#[test]
fn hcooper_root_ping_reports_hidpp_4_5() {
    let mut hidpp = replay(fixture!("root-ping.txt"));
    assert_eq!(
        hidpp.ping(0x39).unwrap(),
        ProtocolVersion { major: 4, minor: 5 }
    );
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn hcooper_vlp_get_feature_finds_the_display_at_index_2() {
    let mut hidpp = replay(fixture!("vlp-get-feature-display.txt"));
    assert_eq!(
        hidpp.vlp_get_feature(feature::VLP_DISPLAY).unwrap(),
        Some(VlpFeatureInfo {
            index: 2,
            hidden: 0,
            version: 0,
            max_memory: 0x0001_e803,
        })
    );
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn hcooper_root_get_feature_finds_reprog_controls() {
    let mut hidpp = replay(fixture!("root-get-feature-reprog.txt"));
    assert_eq!(
        hidpp.get_feature(feature::REPROG_CONTROLS_V4).unwrap(),
        Some(FeatureInfo {
            index: 0x0b,
            flags: 0,
            version: 6,
        })
    );
}

#[test]
fn hcooper_root_get_feature_index_0_means_absent() {
    let mut hidpp = replay(fixture!("root-get-feature-absent.txt"));
    assert_eq!(hidpp.get_feature(0x0020).unwrap(), None);
}

#[test]
fn feature_index_resolves_through_get_feature_once() {
    let mut hidpp = replay(fixture!("vlp-get-feature-display.txt"));
    assert_eq!(
        hidpp
            .feature_index(Plane::Vlp, feature::VLP_DISPLAY)
            .unwrap(),
        2
    );
    // Cached: a second lookup writes nothing (the script is exhausted).
    assert_eq!(
        hidpp
            .feature_index(Plane::Vlp, feature::VLP_DISPLAY)
            .unwrap(),
        2
    );
}

#[test]
fn a_get_feature_answer_is_cached_for_feature_index() {
    let mut hidpp = replay(fixture!("root-get-feature-reprog.txt"));
    hidpp.get_feature(feature::REPROG_CONTROLS_V4).unwrap();
    // The script is exhausted: a second getFeature would fail.
    assert_eq!(
        hidpp
            .feature_index(Plane::Classic, feature::REPROG_CONTROLS_V4)
            .unwrap(),
        0x0b
    );
    let mut vlp = replay(fixture!("vlp-get-feature-display.txt"));
    vlp.vlp_get_feature(feature::VLP_DISPLAY).unwrap();
    assert_eq!(
        vlp.feature_index(Plane::Vlp, feature::VLP_DISPLAY).unwrap(),
        2
    );
}

#[test]
fn hcooper_firmware_walk_gives_the_four_entities() {
    let mut hidpp = replay(fixture!("firmware-version.txt"));
    let entities = firmware::entities(&mut hidpp).unwrap();
    assert!(hidpp.transport_mut().is_done());
    assert_eq!(
        entities[1],
        FirmwareEntity {
            kind: FirmwareKind::Application,
            name: "U1".into(),
            version: 0x66,
            revision: 0x00,
            build: 0x0017,
            active: true,
            transport_pid: 0xc354,
        }
    );
    let shown: Vec<_> = entities
        .iter()
        .map(|e| (e.kind, e.to_string(), e.active))
        .collect();
    assert_eq!(
        shown,
        [
            (
                FirmwareKind::Bootloader,
                "BL2 20.00.B0017".to_owned(),
                false
            ),
            (FirmwareKind::Application, "U1 66.00.B0017".to_owned(), true),
            (
                FirmwareKind::Other(0x0b),
                "IM 01.00.B0017".to_owned(),
                false
            ),
            (FirmwareKind::Hardware, "HW 00.00.B0000".to_owned(), false),
        ]
    );
    assert_eq!(
        firmware::active_application(&entities).map(ToString::to_string),
        Some("U1 66.00.B0017".to_owned())
    );
}

#[test]
fn feature_index_of_an_absent_feature_is_an_error() {
    let mut hidpp = replay(fixture!("root-get-feature-absent.txt"));
    assert_eq!(
        hidpp.feature_index(Plane::Classic, 0x0020),
        Err(HidppError::FeatureNotPresent {
            plane: Plane::Classic,
            feature: 0x0020,
        })
    );
}

#[test]
fn hcooper_keep_alive_range_is_500_to_10000_ms() {
    let mut hidpp = replay(fixture!("keep-alive-range.txt"));
    assert_eq!(
        keep_alive::timeout_range(&mut hidpp).unwrap(),
        TimeoutRange {
            min: Duration::from_millis(500),
            max: Duration::from_millis(10_000),
        }
    );
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn hcooper_keep_alive_pings_once_per_period_with_the_default_timing() {
    let mut hidpp = replay(fixture!("keep-alive-ping.txt"));
    let clock = FakeClock::new();
    let mut keep_alive = KeepAlive::new(&clock, KeepAliveTiming::default());
    let granted = Some(Duration::from_millis(3000));

    // Due at once: the first ping goes out before anything else is read.
    assert_eq!(keep_alive.poll(&mut hidpp).unwrap(), granted);
    clock.advance(Duration::from_millis(999));
    assert_eq!(keep_alive.poll(&mut hidpp).unwrap(), None);
    assert_eq!(keep_alive.until_due(), Duration::from_millis(1));
    clock.advance(Duration::from_millis(1));
    assert_eq!(keep_alive.poll(&mut hidpp).unwrap(), granted);
    clock.advance(Duration::from_millis(1000));
    assert_eq!(keep_alive.poll(&mut hidpp).unwrap(), granted);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn hcooper_geometry_is_a_480_panel_with_nine_118_pixel_keys() {
    let mut hidpp = replay(fixture!("display-geometry.txt"));
    let geometry = display::geometry(&mut hidpp).unwrap();
    assert_eq!((geometry.width, geometry.height), (480, 480));
    let origins: Vec<_> = geometry.keys.iter().map(|k| (k.index, k.x, k.y)).collect();
    assert_eq!(
        origins,
        [
            (1, 24, 6),
            (2, 182, 7),
            (3, 340, 6),
            (4, 23, 164),
            (5, 181, 163),
            (6, 339, 164),
            (7, 23, 322),
            (8, 181, 321),
            (9, 340, 321),
        ]
    );
    assert!(
        geometry
            .keys
            .iter()
            .all(|k| (k.width, k.height) == (118, 118))
    );
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn the_captured_geometry_is_the_hcooper_reply() {
    let mut hidpp = replay(fixture!("display-geometry.txt"));
    assert_eq!(
        display::captured_geometry(),
        display::geometry(&mut hidpp).unwrap()
    );
}

#[test]
fn hcooper_claim_takes_the_display_from_logo_to_streaming_mode() {
    let mut hidpp = replay(fixture!("display-claim.txt"));
    let claim = display::claim(&mut hidpp).unwrap();
    assert_eq!(
        claim.protocol,
        VlpProtocol {
            major: 1,
            minor: 0,
            available_memory: 0x0001_e820,
        }
    );
    assert_eq!(claim.display_index, 2);
    assert_eq!(claim.modes.len(), 13);
    assert!(claim.modes.contains(&DisplayMode::STREAMING));
    assert_eq!(claim.boot_mode, DisplayMode::STANDBY);
    assert_eq!(claim.geometry.keys.len(), 9);
    assert!(claim.mode_event);
    // The mode-change event is consumed, not left for the input path.
    assert_eq!(hidpp.take_unmatched(), None);
    assert!(hidpp.transport_mut().is_done());
}

/// The panel the Options+ captures paint: the bounding box of the nine keys.
const CAPTURED_PANEL: Rect = Rect {
    x: 23,
    y: 6,
    width: 435,
    height: 434,
};

/// One 4095-byte LCD write of `02-binding-paint.pcap` by frame number, from
/// `captures/external/hcooper/derived/binding-paint-frames.tsv`.
fn binding_paint(frame: &str) -> Vec<u8> {
    let tsv = include_str!("../../../captures/external/hcooper/derived/binding-paint-frames.tsv");
    let hex = tsv
        .lines()
        .filter_map(|line| line.split('\t').collect::<Vec<_>>().try_into().ok())
        .find_map(|[number, _, hex]: [&str; 3]| (number == frame).then_some(hex))
        .unwrap();
    decode_hex(hex)
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

/// The JPEG a captured write carries: the 24-bit length in the first
/// fragment's header, the bytes after the 20-byte first and the 5-byte
/// continuation headers.
fn captured_jpeg(fragments: &[Vec<u8>]) -> Vec<u8> {
    let len = usize::from(fragments[0][17]) << 16
        | usize::from(fragments[0][18]) << 8
        | usize::from(fragments[0][19]);
    let mut jpeg = fragments[0][20..].to_vec();
    for fragment in &fragments[1..] {
        jpeg.extend_from_slice(&fragment[5..]);
    }
    jpeg.truncate(len);
    jpeg
}

#[test]
fn hcooper_geometry_places_keys_and_the_captured_panel() {
    let mut hidpp = replay(fixture!("display-geometry.txt"));
    let geometry = display::geometry(&mut hidpp).unwrap();
    // The panel Options+ paints is the bounding box of the published keys.
    assert_eq!(Target::Panel.rect(&geometry), Ok(CAPTURED_PANEL));
    assert_eq!(
        Target::Key(5).rect(&geometry),
        Ok(Rect {
            x: 181,
            y: 163,
            width: 118,
            height: 118,
        })
    );
    assert_eq!(
        Target::Key(10).rect(&geometry),
        Err(ImageError::NoSuchKey { key: 10, count: 9 })
    );
    assert_eq!(
        Target::Key(0).rect(&geometry),
        Err(ImageError::NoSuchKey { key: 0, count: 9 })
    );
}

#[test]
fn hcooper_single_fragment_panel_write_is_reproduced() {
    let captured = vec![binding_paint("521")];
    let jpeg = captured_jpeg(&captured);
    assert_eq!(jpeg.len(), 1460);
    let fragments = image::fragments(
        DEVICE_DIRECT,
        2,
        &ImageConfig::default(),
        CAPTURED_PANEL,
        &jpeg,
    )
    .unwrap();
    assert_eq!(fragments, captured);
}

#[test]
fn hcooper_two_fragment_panel_write_is_reproduced() {
    let captured = vec![binding_paint("639"), binding_paint("640")];
    let jpeg = captured_jpeg(&captured);
    assert_eq!(jpeg.len(), 0x110b);
    let fragments = image::fragments(
        DEVICE_DIRECT,
        2,
        &ImageConfig::default(),
        CAPTURED_PANEL,
        &jpeg,
    )
    .unwrap();
    // Sequence bytes a0 (first, index 0) and 61 (last, index 1).
    assert_eq!(fragments, captured);
}

/// The ACK lines of `image-write-panel.txt`: `13 ff 02 2e c<index> ...`.
const ACK_PREFIX: &str = "< 13 ff 02 2e";

/// The startup capture's write settings: swid 0xe, index base 0.
fn startup_write_config() -> ImageConfig {
    ImageConfig {
        swid: SoftwareId::new(CAPTURE_SWID).unwrap(),
        ..ImageConfig::default()
    }
}

/// The JPEG the fixture's `0x14` writes carry.
fn startup_jpeg() -> Vec<u8> {
    let writes: Vec<_> = fixture!("image-write-panel.txt")
        .lines()
        .filter_map(|line| line.strip_prefix("> 14 "))
        .map(|hex| decode_hex(&format!("14{}", hex.replace(' ', ""))))
        .collect();
    captured_jpeg(&writes)
}

#[test]
fn hcooper_panel_write_sends_each_fragment_after_the_ack_of_the_one_before() {
    let mut hidpp = replay(fixture!("image-write-panel.txt"));
    let jpeg = startup_jpeg();
    assert_eq!(jpeg.len(), 0x110a);
    let written = image::write(&mut hidpp, &startup_write_config(), CAPTURED_PANEL, &jpeg).unwrap();
    assert_eq!(
        written,
        Written {
            fragments: 2,
            acked: 2
        }
    );
    // The ACKs are consumed; none reaches the input path as a key 1 press.
    assert_eq!(hidpp.take_unmatched(), None);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn a_missing_ack_times_out_and_the_next_fragment_goes_out_anyway() {
    let without_acks = fixture!("image-write-panel.txt")
        .lines()
        .filter(|line| !line.starts_with(ACK_PREFIX))
        .collect::<Vec<_>>()
        .join("\n");
    let mut hidpp = replay(&without_acks);
    let written = image::write(
        &mut hidpp,
        &startup_write_config(),
        CAPTURED_PANEL,
        &startup_jpeg(),
    )
    .unwrap();
    assert_eq!(
        written,
        Written {
            fragments: 2,
            acked: 0
        }
    );
    assert!(hidpp.transport_mut().is_done());
}

/// Every event the fixture's reports cause, read through `Hidpp::next_frame`.
fn events(script: &str, mut input: Input) -> Vec<InputEvent> {
    let mut hidpp = replay(script);
    let mut events = Vec::new();
    while let Some(frame) = hidpp.next_frame(Duration::ZERO).unwrap() {
        events.extend(input.feed(&frame));
    }
    assert!(hidpp.transport_mut().is_done());
    events
}

#[test]
fn key_input_fixture_gives_the_exact_press_release_list() {
    let input = Input::new(DEVICE_DIRECT, 2, 9);
    assert_eq!(
        events(fixture!("key-input.txt"), input),
        [
            InputEvent::press(Control::Key(5)),
            InputEvent::press(Control::Key(1)),
            InputEvent::release(Control::Key(5)),
            InputEvent::release(Control::Key(1)),
        ]
    );
}

#[test]
fn page_button_fixture_gives_the_exact_press_release_list() {
    let input = Input::new(DEVICE_DIRECT, 2, 9).with_page_buttons(0x0b);
    let (left, right) = (
        Control::Page(reprog::PAGE_LEFT),
        Control::Page(reprog::PAGE_RIGHT),
    );
    assert_eq!(
        events(fixture!("page-buttons.txt"), input),
        [
            InputEvent::press(left),
            InputEvent::press(right),
            InputEvent::release(left),
            InputEvent::release(right),
        ]
    );
}

#[test]
fn hcooper_divert_walks_the_controls_and_diverts_both_page_buttons() {
    let mut hidpp = replay(fixture!("reprog-divert.txt"));
    let mut slept = Vec::new();
    let diverted = reprog::divert_page_buttons(&mut hidpp, &DivertTiming::default(), |pause| {
        slept.push(pause.as_millis());
    })
    .unwrap();
    assert_eq!(diverted.reprog_index, 0x0b);
    assert_eq!(
        diverted.controls,
        [
            ControlInfo {
                cid: reprog::PAGE_LEFT,
                task_id: 0,
                flags: reprog::FLAG_DIVERTABLE,
                position: 0,
                group: 1,
                group_mask: 0,
            },
            ControlInfo {
                cid: reprog::PAGE_RIGHT,
                task_id: 0,
                flags: reprog::FLAG_DIVERTABLE,
                position: 0,
                group: 1,
                group_mask: 0,
            },
        ]
    );
    // The settle once, then the spacing before each further 0x1B04 request:
    // getCidInfo twice, set and verify per button.
    assert_eq!(slept, [100, 20, 20, 20, 20, 20, 20]);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn a_key_report_while_waiting_for_an_ack_is_kept_for_the_input_path() {
    // Key 1 held (function 0, spec §5.6) arrives before the first ACK.
    let key_press = format!("< 13 ff 02 00 c0 01 01{}", " 00".repeat(25));
    let script = fixture!("image-write-panel.txt")
        .lines()
        .flat_map(|line| {
            let before = line
                .starts_with("< 13 ff 02 2e c0")
                .then(|| key_press.clone());
            before.into_iter().chain([line.to_owned()])
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut hidpp = replay(&script);
    let written = image::write(
        &mut hidpp,
        &startup_write_config(),
        CAPTURED_PANEL,
        &startup_jpeg(),
    )
    .unwrap();
    assert_eq!(written.acked, 2);
    let kept = hidpp.take_unmatched().unwrap();
    assert_eq!(kept[..7], [0x13, 0xff, 0x02, 0x00, 0xc0, 0x01, 0x01]);
    assert_eq!(hidpp.take_unmatched(), None);
}
