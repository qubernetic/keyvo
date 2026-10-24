//! Golden tests: `keyvo-hid` replays the hcooper capture frames in
//! `tests/protocol-fixtures/hid/` through `FakeHidTransport` and must write
//! exactly the captured requests and decode the captured replies.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use keyvo_core::clock::FakeClock;
use keyvo_core::hid::FakeHidTransport;
use keyvo_hid::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, Plane, SoftwareId};
use keyvo_hid::keep_alive::{self, KeepAlive, KeepAliveTiming, TimeoutRange};
use keyvo_hid::root::{FeatureInfo, ProtocolVersion, VlpFeatureInfo, feature};

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
