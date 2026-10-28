//! Golden tests: `keyvo-hid` replays the hcooper capture frames in
//! `tests/protocol-fixtures/hid/` through `FakeHidTransport` and must write
//! exactly the captured requests and decode the captured replies.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Write as _;
use std::time::Duration;

use keyvo_core::clock::FakeClock;
use keyvo_core::hid::FakeHidTransport;
use keyvo_hid::dialpad::{
    self, Dialpad, DialpadError, DialpadEvent, DialpadTiming, Link, Slot, Slots,
};
use keyvo_hid::display::{self, DisplayMode};
use keyvo_hid::firmware::{self, FirmwareEntity, FirmwareKind};
use keyvo_hid::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, Plane, ReceiverError, SoftwareId};
use keyvo_hid::image::{
    self, ImageConfig, ImageError, ImageHeader, IndexBase, Rect, Target, Written,
};
use keyvo_hid::input::{Control, Input, InputEvent};
use keyvo_hid::keep_alive::{self, KeepAlive, KeepAliveTiming, TimeoutRange};
use keyvo_hid::multiroller::Roller;
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
    assert_eq!((geometry.keys.len(), geometry.visible_areas), (9, 0));
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
fn hcooper_capabilities_decode_every_field() {
    let mut hidpp = replay(fixture!("display-capabilities.txt"));
    assert_eq!(
        display::capabilities(&mut hidpp).unwrap(),
        display::Capabilities {
            screen_count: 1,
            max_image_size: 124_928,
            max_image_fps: 30,
            deferrable_display_update: true,
            rgb565: true,
            rgb888: false,
            jpeg: true,
            calibrated: true,
            origin: 0,
        }
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

/// The `[timing]` defaults of the plug-in splash wait (spec §6.1).
const SPLASH: display::SplashWait = display::SplashWait {
    retry: Duration::from_millis(500),
    timeout: Duration::from_millis(10_000),
};

/// The claim without a keep-alive to poll while it waits.
fn claim(hidpp: &mut Hidpp<FakeHidTransport>) -> Result<display::Claim, display::ClaimError> {
    display::claim(hidpp, SPLASH, |_| Ok(()))
}

#[test]
fn hcooper_claim_takes_the_display_from_logo_to_streaming_mode() {
    let mut hidpp = replay(fixture!("display-claim.txt"));
    let claim = claim(&mut hidpp).unwrap();
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
    assert_eq!(claim.splash_retries, 0);
}

/// `display-claim.txt` with its switch to `a0` (steps 6 and 7) replaced by
/// `display-splash.txt`, built from the #52 daemon log.
fn claim_through_splash() -> String {
    let head = fixture!("display-claim.txt").split("# 6.").next().unwrap();
    format!("{head}{}", fixture!("display-splash.txt"))
}

#[test]
fn the_claim_waits_out_the_plug_in_splash() {
    let mut hidpp = replay(&claim_through_splash());
    let mut polls = 0;
    let claim = display::claim(&mut hidpp, SPLASH, |_| {
        polls += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(claim.splash_retries, 2);
    assert!(claim.mode_event);
    // Between two retries the caller's keep-alive gets its turn.
    assert_eq!(polls, 2);
    let set_modes = hidpp
        .transport_mut()
        .written()
        .iter()
        .filter(|report| report.starts_with(&[0x13, 0xff, 0x02, 0x4e]))
        .count();
    assert_eq!(set_modes, 3);
    assert_eq!(hidpp.take_unmatched(), None);
    assert!(hidpp.transport_mut().is_done());
}

/// The `KEEP_ALIVE` index in the fixtures.
const KEEP_ALIVE_INDEX: u8 = 0x04;

/// The lines of `claim-lost.txt`, in the order the device sends them: the
/// `KEEP_ALIVE` expiry event, then the mode-change event to `a1`.
const EXPIRY_EVENT: usize = 0;
const MODE_EVENT: usize = 1;

/// The claim, then `lost` (lines of `claim-lost.txt`) answering its last
/// request, then the reclaim exchange `reclaims` times.
fn claim_then_lost(lost: &[usize], reclaims: usize) -> String {
    let events: Vec<_> = fixture!("claim-lost.txt")
        .lines()
        .filter(|line| line.starts_with('<'))
        .collect();
    let mut script = fixture!("display-claim.txt").to_owned();
    for &n in lost {
        script.push_str(events[n]);
        script.push('\n');
    }
    script + &fixture!("display-reclaim.txt").repeat(reclaims)
}

/// Claims, then reads every inbound frame through the claim watch; the
/// number of reclaims.
fn reclaims_after(script: &str) -> usize {
    let mut hidpp = replay(script);
    let claim = claim(&mut hidpp).unwrap();
    let watch = display::ClaimWatch::new(DEVICE_DIRECT, claim.display_index)
        .with_keep_alive(KEEP_ALIVE_INDEX);
    let mut reclaims = 0;
    while let Some(frame) = hidpp.next_frame(Duration::ZERO).unwrap() {
        if watch.reclaim_on(&mut hidpp, &frame).unwrap() {
            reclaims += 1;
        }
    }
    assert!(hidpp.transport_mut().is_done());
    reclaims
}

#[test]
fn the_mode_and_expiry_events_of_one_expiry_give_one_reclaim() {
    assert_eq!(
        reclaims_after(&claim_then_lost(&[EXPIRY_EVENT, MODE_EVENT], 1)),
        1
    );
}

#[test]
fn the_mode_event_alone_gives_one_reclaim() {
    assert_eq!(reclaims_after(&claim_then_lost(&[MODE_EVENT], 1)), 1);
}

#[test]
fn the_expiry_event_alone_gives_one_reclaim() {
    assert_eq!(reclaims_after(&claim_then_lost(&[EXPIRY_EVENT], 1)), 1);
}

#[test]
fn the_claim_watch_tells_lost_claims_from_other_reports() {
    let watch = display::ClaimWatch::new(DEVICE_DIRECT, 2).with_keep_alive(KEEP_ALIVE_INDEX);
    let frame = |hex: &str| {
        let mut bytes = decode_hex(&hex.replace(' ', ""));
        bytes.resize(if bytes[0] == 0x13 { 32 } else { 20 }, 0);
        keyvo_hid::frame::Frame::parse(&bytes).unwrap()
    };
    for lost in ["13 ff 02 10 cb a1", "13 ff 02 10 c3 a2", "11 ff 04 00"] {
        assert!(watch.is_loss(&frame(lost)), "{lost}");
    }
    for other in [
        "13 ff 02 10 c6 a0",    // the mode-change event of the claim
        "13 ff 02 00 c0 01 05", // a key report
        "13 ff 02 4e c0 a1",    // a setMode reply
        "11 ff 04 1e 0b b8",    // a keepAlive reply
        "11 ff 0b 00 01 a1",    // a page-button notification
        "13 01 02 10 cb a1",    // another device
    ] {
        assert!(!watch.is_loss(&frame(other)), "{other}");
    }
    // Without the keep-alive index only the mode event counts.
    assert!(!display::ClaimWatch::new(DEVICE_DIRECT, 2).is_loss(&frame("11 ff 04 00")));
}

#[test]
fn a_reclaim_the_display_does_not_confirm_is_an_error() {
    let mut script = claim_then_lost(&[MODE_EVENT], 0);
    let refused =
        fixture!("display-reclaim.txt").replace("< 13 ff 02 5e c0 a0", "< 13 ff 02 5e c0 a1");
    script.push_str(&refused);
    let mut hidpp = replay(&script);
    let claim = claim(&mut hidpp).unwrap();
    let watch = display::ClaimWatch::new(DEVICE_DIRECT, claim.display_index);
    let frame = hidpp.next_frame(Duration::ZERO).unwrap().unwrap();
    assert_eq!(
        watch.reclaim_on(&mut hidpp, &frame),
        Err(display::ClaimError::ModeNotConfirmed { mode: 0xa1 })
    );
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
fn the_set_image_header_of_a_union_paint_is_hcooper_frame_521() {
    // The union of the captured key rectangles, with the header fields
    // Options+ sends: display 1, no deferred update, one JPEG image.
    let union = Target::Panel.rect(&display::captured_geometry()).unwrap();
    let header = ImageHeader::jpeg(union, 1460);
    assert_eq!(header.display_index, image::DISPLAY_INDEX);
    assert!(!header.defer_display_update);
    assert_eq!(header.count, 1);
    assert_eq!(header.format, image::FORMAT_JPEG);
    // Frame 521 after the VLP header (14 ff 02 2d e0): bytes 5 to 19.
    assert_eq!(header.encode()[..], binding_paint("521")[5..20]);
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

#[test]
fn first_light_acks_of_both_index_bases_and_both_software_ids_count() {
    // The ACK pairs of `image-acks.txt`, in the fixture's order.
    let acks: Vec<_> = fixture!("image-acks.txt")
        .lines()
        .filter(|line| line.starts_with('<'))
        .collect();
    let settings = [
        (IndexBase::Zero, 0xd),
        (IndexBase::One, 0xd),
        (IndexBase::Zero, 0xb),
        (IndexBase::One, 0xb),
    ];
    assert_eq!(acks.len(), 2 * settings.len());
    for ((index_base, swid), pair) in settings.into_iter().zip(acks.chunks(2)) {
        let config = ImageConfig {
            swid: SoftwareId::new(swid).unwrap(),
            index_base,
            ..ImageConfig::default()
        };
        let jpeg = startup_jpeg();
        let fragments = image::fragments(DEVICE_DIRECT, 2, &config, CAPTURED_PANEL, &jpeg).unwrap();
        assert_eq!(fragments.len(), 2);
        // The writer resolves the display index first, then sends each
        // fragment after the ACK of the one before.
        let mut script = fixture!("vlp-get-feature-display.txt").to_owned();
        for (fragment, ack) in fragments.iter().zip(pair) {
            writeln!(script, "> {}\n{ack}", to_hex(fragment)).unwrap();
        }
        let mut hidpp = replay(&script);
        let written = image::write(&mut hidpp, &config, CAPTURED_PANEL, &jpeg).unwrap();
        assert_eq!(
            written,
            Written {
                fragments: 2,
                acked: 2
            },
            "{index_base:?}, swid {swid:#x}"
        );
        assert_eq!(hidpp.take_unmatched(), None);
        assert!(hidpp.transport_mut().is_done());
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
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
            InputEvent::press(Control::Key(2)),
            InputEvent::press(Control::Key(1)),
            InputEvent::release(Control::Key(1)),
            InputEvent::release(Control::Key(2)),
            InputEvent::press(Control::Key(1)),
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
            InputEvent::press(right),
            InputEvent::press(left),
            InputEvent::release(right),
            InputEvent::release(left),
            InputEvent::press(left),
            InputEvent::release(left),
            InputEvent::press(right),
            InputEvent::release(right),
        ]
    );
}

#[test]
fn first_light_divert_walks_the_controls_and_diverts_both_page_buttons() {
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

/// The Dialpad's slot on the receiver in the receiver fixtures.
const DIALPAD_SLOT: u8 = 3;

/// The Dialpad's CIDs, buttons 1 to 4 (measured).
const DIALPAD_BUTTONS: [u16; 4] = [0x0053, 0x0056, 0x0059, 0x005a];

/// The search, the control walk, the roller capabilities and the diverts:
/// everything `Dialpad::open` sends on the receiver.
fn receiver_open() -> String {
    [
        fixture!("receiver-slot-probe.txt"),
        fixture!("receiver-controls.txt"),
        fixture!("receiver-multiroller.txt"),
        fixture!("receiver-divert.txt"),
    ]
    .concat()
}

/// `Dialpad::open` on `script`, the waits it asked for in ms.
fn open_dialpad(script: &str, link: Link) -> (Hidpp<FakeHidTransport>, Dialpad, Vec<u128>) {
    let mut hidpp = replay(script);
    let mut slept = Vec::new();
    let dialpad = Dialpad::open(&mut hidpp, link, &DialpadTiming::default(), |pause| {
        slept.push(pause.as_millis());
    })
    .unwrap();
    (hidpp, dialpad, slept)
}

/// Every event until the script runs dry.
fn dialpad_events(hidpp: &mut Hidpp<FakeHidTransport>, dialpad: &mut Dialpad) -> Vec<DialpadEvent> {
    std::iter::from_fn(|| dialpad.next_event(hidpp, REPLY_TIMEOUT, |_| {}).unwrap()).collect()
}

#[test]
fn receiver_open_finds_the_dialpad_on_its_slot_and_diverts_it() {
    let (mut hidpp, dialpad, slept) = open_dialpad(&receiver_open(), Link::Receiver);
    assert_eq!(dialpad.device_index(), DIALPAD_SLOT);
    assert_eq!(hidpp.device_index(), DIALPAD_SLOT);
    assert_eq!(
        dialpad.slots(),
        &Slots(vec![
            (1, Slot::Empty(ReceiverError::UnknownDevice)),
            (2, Slot::Silent),
            (3, Slot::Dialpad),
        ])
    );
    assert_eq!(dialpad.reprog_index(), 0x0a);
    assert_eq!(dialpad.roller_index(), 0x0d);
    assert_eq!(dialpad.buttons(), DIALPAD_BUTTONS);
    assert_eq!(dialpad.button(0x0059), Some(3));
    assert_eq!(dialpad.button(0x0001), None);
    assert_eq!(
        dialpad.rollers(),
        [
            Roller {
                id: 0,
                increments: 40,
                ratchets: 0,
            },
            Roller {
                id: 1,
                increments: 180,
                ratchets: 0,
            },
        ]
    );
    // The settle before the walk, the spacing before every further 0x1B04
    // request (four getCidInfo, set and verify per button) and every
    // setMode.
    let mut expected = vec![100];
    expected.extend([20; 4 + 8 + 2]);
    assert_eq!(slept, expected);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn receiver_input_gives_turns_and_button_edges_of_the_dialpad_only() {
    let script = receiver_open() + fixture!("receiver-input.txt");
    let (mut hidpp, mut dialpad, _) = open_dialpad(&script, Link::Receiver);
    let turn = |roller, delta| DialpadEvent::Input(InputEvent::Rotate { roller, delta });
    let button = |cid| Control::Page(cid);
    assert_eq!(
        dialpad_events(&mut hidpp, &mut dialpad),
        [
            turn(1, 3),
            turn(1, -2),
            turn(0, 127),
            turn(0, -128),
            turn(0, -1),
            DialpadEvent::Input(InputEvent::press(button(0x0053))),
            DialpadEvent::Input(InputEvent::release(button(0x0053))),
            DialpadEvent::Input(InputEvent::press(button(0x0059))),
            DialpadEvent::Input(InputEvent::press(button(0x005a))),
            DialpadEvent::Input(InputEvent::release(button(0x0059))),
            DialpadEvent::Input(InputEvent::release(button(0x005a))),
        ]
    );
    // The kernel's DJ report is not a frame.
    assert_eq!(hidpp.dropped_reports(), 1);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn receiver_restore_gives_every_divert_back() {
    let script = receiver_open() + fixture!("receiver-restore.txt");
    let (mut hidpp, dialpad, _) = open_dialpad(&script, Link::Receiver);
    let mut slept = Vec::new();
    dialpad
        .restore(&mut hidpp, |pause| slept.push(pause.as_millis()))
        .unwrap();
    assert_eq!(slept, [20; 6]);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn receiver_arrival_after_a_lost_link_rediverts_the_dialpad() {
    // Button 1 held when the link goes; the arrival re-sends every divert,
    // and the stop restores them.
    let held = "< 11 03 0a 00 00 53 00 00 00 00 00 00 00 00 00 00 00 00 00 00\n";
    let script = [
        receiver_open().as_str(),
        held,
        fixture!("receiver-arrival.txt"),
        fixture!("receiver-divert.txt"),
        held,
        fixture!("receiver-restore.txt"),
    ]
    .concat();
    let (mut hidpp, mut dialpad, _) = open_dialpad(&script, Link::Receiver);
    let mut slept = Vec::new();
    let mut next = || {
        dialpad
            .next_event(&mut hidpp, REPLY_TIMEOUT, |pause| {
                slept.push(pause.as_millis());
            })
            .unwrap()
    };
    let press = DialpadEvent::Input(InputEvent::press(Control::Page(0x0053)));
    assert_eq!(next(), Some(press));
    assert_eq!(next(), Some(DialpadEvent::LinkLost));
    assert_eq!(next(), Some(DialpadEvent::Rediverted));
    // Nothing was held after the lost link: the button presses again.
    assert_eq!(next(), Some(press));
    assert_eq!(next(), None);
    let mut expected = vec![100];
    expected.extend([20; 8 + 2]);
    assert_eq!(slept, expected);
    dialpad.restore(&mut hidpp, |_| {}).unwrap();
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn receiver_search_without_an_awake_dialpad_waits_for_its_arrival() {
    let mut hidpp = replay(fixture!("receiver-asleep.txt"));
    let timing = DialpadTiming::default();
    let Err(DialpadError::NotFound(slots)) =
        Dialpad::open(&mut hidpp, Link::Receiver, &timing, |_| {})
    else {
        panic!("found a Dialpad in the asleep fixture");
    };
    let empty = Slot::Empty(ReceiverError::UnknownDevice);
    assert_eq!(
        slots,
        Slots(vec![
            (1, empty),
            (2, empty),
            (3, Slot::Silent),
            (4, empty),
            (5, empty),
            (6, empty),
        ])
    );
    assert_eq!(slots.silent(), [3]);
    let slot = dialpad::wait_for_arrival(&mut hidpp, &slots.silent(), REPLY_TIMEOUT).unwrap();
    assert_eq!(slot, Some(DIALPAD_SLOT));
    assert!(hidpp.transport_mut().is_done());

    // The slot alone is asked again.
    let open = receiver_open();
    let from_slot_3 = format!("#{}", open.split("# 3. slot 3").nth(1).unwrap());
    let mut hidpp = replay(&from_slot_3);
    let dialpad = Dialpad::open(&mut hidpp, Link::Slot(DIALPAD_SLOT), &timing, |_| {}).unwrap();
    assert_eq!(dialpad.slots(), &Slots(vec![(3, Slot::Dialpad)]));
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn a_bluetooth_dialpad_is_the_same_driver_on_device_index_0xff() {
    let open = receiver_open();
    let from_slot_3 = format!("#{}", open.split("# 3. slot 3").nth(1).unwrap());
    let script = from_slot_3.replace("11 03 ", "11 ff ")
        + &fixture!("receiver-input.txt")
            .lines()
            .filter(|line| line.starts_with("< 11 03 0d"))
            .map(|line| line.replace("11 03 ", "11 ff ") + "\n")
            .collect::<String>();
    let (mut hidpp, mut dialpad, _) = open_dialpad(&script, Link::Direct);
    assert_eq!(dialpad.device_index(), DEVICE_DIRECT);
    assert_eq!(dialpad.slots().to_string(), "direct dialpad");
    assert_eq!(dialpad_events(&mut hidpp, &mut dialpad).len(), 5);
    assert!(hidpp.transport_mut().is_done());
}

#[test]
fn a_device_index_whose_model_is_not_the_dialpad_is_skipped() {
    // Slot 3 answers with the MX Master 4's WPID B042 in its model id.
    let script = fixture!("receiver-slot-probe.txt").replace("29 00 04 bc 00", "29 00 04 b0 42")
        + &[4, 5, 6]
            .map(|slot| {
                format!(
                    "> 11 0{slot} 00 0e 46 10{}\n< 10 0{slot} 8f 00 0e 08 00\n",
                    " 00".repeat(14)
                )
            })
            .concat();
    let mut hidpp = replay(&script);
    let error = Dialpad::find(
        &mut hidpp,
        Link::Receiver,
        &DialpadTiming::default(),
        |_| {},
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "no MX Creative Dialpad found (slots: 1 empty, 2 silent, 3 model b0 42 00 00 00 00, \
         4 empty, 5 empty, 6 empty)"
    );
    assert!(hidpp.transport_mut().is_done());
}
