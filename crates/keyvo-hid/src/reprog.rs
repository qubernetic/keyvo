//! Classic HID++ `0x1B04 REPROG_CONTROLS_V4` (spec §5.6): the control walk
//! and the divert of the Keypad's page buttons, whose presses then arrive
//! as notifications ([`crate::input::Input::with_page_buttons`]).
//!
//! The divert is volatile: the caller sends it on every open and reconnect,
//! after the keep-alive has started.

use std::time::Duration;

use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::feature;

/// CID of the left page button (measured).
pub const PAGE_LEFT: u16 = 0x01A1;
/// CID of the right page button (measured).
pub const PAGE_RIGHT: u16 = 0x01A2;
/// The page buttons, in the order keyvo diverts them.
const PAGE_BUTTONS: [u16; 2] = [PAGE_LEFT, PAGE_RIGHT];

/// Flag of [`ControlInfo::flags`]: the control can be diverted.
pub const FLAG_DIVERTABLE: u8 = 0x20;

/// `getCount`.
const FN_GET_COUNT: u8 = 0;
/// `getCidInfo(index)`.
const FN_GET_CID_INFO: u8 = 1;
/// `getCidReporting(cid)`.
const FN_GET_CID_REPORTING: u8 = 2;
/// `setCidReporting(cid, flags)`.
const FN_SET_CID_REPORTING: u8 = 3;
/// Reporting flag: the control is diverted (set and read back).
const REPORTING_DIVERT: u8 = 0x01;
/// Reporting flag: the divert bit of this request is valid (set only).
const REPORTING_DIVERT_VALID: u8 = 0x02;

/// Default of `[timing].divert_settle` (spec §6.1): Julusian PR #22 waits
/// this long after open before the first divert request.
pub const DEFAULT_DIVERT_SETTLE: Duration = Duration::from_millis(100);
/// Default of `[timing].divert_spacing` (spec §6.1): Julusian PR #22 leaves
/// this much between two divert requests.
pub const DEFAULT_DIVERT_SPACING: Duration = Duration::from_millis(20);

/// The `[timing]` values of the divert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DivertTiming {
    /// `[timing].divert_settle`: the wait before the first request.
    pub settle: Duration,
    /// `[timing].divert_spacing`: the wait between two `0x1B04` requests.
    pub spacing: Duration,
}

impl Default for DivertTiming {
    fn default() -> Self {
        Self {
            settle: DEFAULT_DIVERT_SETTLE,
            spacing: DEFAULT_DIVERT_SPACING,
        }
    }
}

/// One control from `getCidInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlInfo {
    /// Control id.
    pub cid: u16,
    /// Task id.
    pub task_id: u16,
    /// Capability flags; [`FLAG_DIVERTABLE`] among them.
    pub flags: u8,
    /// Position byte.
    pub position: u8,
    /// Group.
    pub group: u8,
    /// Group mask.
    pub group_mask: u8,
}

impl ControlInfo {
    /// True when the control can be diverted.
    #[must_use]
    pub const fn divertable(&self) -> bool {
        self.flags & FLAG_DIVERTABLE != 0
    }
}

/// What the divert found and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diverted {
    /// Index of `0x1B04` on the classic plane, for
    /// [`crate::input::Input::with_page_buttons`].
    pub reprog_index: u8,
    /// Every control the device lists, in its order.
    pub controls: Vec<ControlInfo>,
}

/// Errors of [`divert_page_buttons`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DivertError {
    /// A request failed.
    #[error(transparent)]
    Hidpp(#[from] HidppError),
    /// The device does not list a page button.
    #[error("control 0x{cid:04x} is not in the 0x1B04 control list")]
    MissingControl {
        /// The CID.
        cid: u16,
    },
    /// The device lists a page button without the divertable flag.
    #[error("control 0x{cid:04x} is not divertable (flags 0x{flags:02x})")]
    NotDivertable {
        /// The CID.
        cid: u16,
        /// Its flags.
        flags: u8,
    },
    /// `getCidReporting` does not show the divert after the set.
    #[error("control 0x{cid:04x} reports flags 0x{flags:02x} after the divert")]
    NotConfirmed {
        /// The CID.
        cid: u16,
        /// The reporting flags read back.
        flags: u8,
    },
}

/// Diverts both page buttons (spec §5.6): waits `timing.settle`, resolves
/// the `0x1B04` index, walks the controls with `getCount` and
/// `getCidInfo`, refuses unless both page buttons are listed and
/// divertable, then per button `setCidReporting(cid, 0x03)` verified with
/// `getCidReporting`. Every further `0x1B04` request waits
/// `timing.spacing` first; `sleep` does the waiting, so the caller decides
/// how (a thread sleep in the device actor, a recorder in tests).
///
/// # Errors
///
/// [`DivertError::MissingControl`] and [`DivertError::NotDivertable`]
/// before anything is diverted, [`DivertError::NotConfirmed`], and the
/// request errors.
pub fn divert_page_buttons<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    timing: &DivertTiming,
    mut sleep: impl FnMut(Duration),
) -> Result<Diverted, DivertError> {
    sleep(timing.settle);
    let index = hidpp.feature_index(Plane::Classic, feature::REPROG_CONTROLS_V4)?;
    let reply = hidpp.request(Plane::Classic, index, FN_GET_COUNT, &[])?;
    let count = reply.params.first().copied().unwrap_or_default();
    let mut controls = Vec::with_capacity(usize::from(count));
    for position in 0..count {
        sleep(timing.spacing);
        let reply = hidpp.request(Plane::Classic, index, FN_GET_CID_INFO, &[position])?;
        controls.push(control_info(&reply.params)?);
    }
    for cid in PAGE_BUTTONS {
        let control = controls
            .iter()
            .find(|control| control.cid == cid)
            .ok_or(DivertError::MissingControl { cid })?;
        if !control.divertable() {
            return Err(DivertError::NotDivertable {
                cid,
                flags: control.flags,
            });
        }
    }
    for cid in PAGE_BUTTONS {
        let [hi, lo] = cid.to_be_bytes();
        sleep(timing.spacing);
        let reply = hidpp.request(
            Plane::Classic,
            index,
            FN_SET_CID_REPORTING,
            &[hi, lo, REPORTING_DIVERT | REPORTING_DIVERT_VALID],
        )?;
        if reply.params.get(..2) != Some(&[hi, lo]) {
            return Err(HidppError::Malformed(format!(
                "setCidReporting(0x{cid:04x}) answered for another control"
            ))
            .into());
        }
        sleep(timing.spacing);
        let reply = hidpp.request(Plane::Classic, index, FN_GET_CID_REPORTING, &[hi, lo])?;
        let [r_hi, r_lo, flags, ..] = reply.params[..] else {
            return Err(HidppError::Malformed("short getCidReporting reply".into()).into());
        };
        if [r_hi, r_lo] != [hi, lo] || flags & REPORTING_DIVERT == 0 {
            return Err(DivertError::NotConfirmed { cid, flags });
        }
    }
    Ok(Diverted {
        reprog_index: index,
        controls,
    })
}

/// Decodes a `getCidInfo` reply: cid (u16), task id (u16), flags,
/// position, group, group mask.
fn control_info(params: &[u8]) -> Result<ControlInfo, HidppError> {
    let [c0, c1, t0, t1, flags, position, group, group_mask, ..] = params[..] else {
        return Err(HidppError::Malformed("short getCidInfo reply".into()));
    };
    Ok(ControlInfo {
        cid: u16::from_be_bytes([c0, c1]),
        task_id: u16::from_be_bytes([t0, t1]),
        flags,
        position,
        group,
        group_mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidpp::{DEVICE_DIRECT, SoftwareId};
    use keyvo_core::hid::FakeHidTransport;

    /// One long report, swid 1, zero-padded.
    fn long(function: u8, params: &[u8]) -> String {
        let mut bytes = vec![0x11, 0xff, 0x0b, (function << 4) | 0x1];
        bytes.extend(params);
        bytes.resize(20, 0);
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// getFeature(0x1B04) = 0x0b, then getCount and getCidInfo for `infos`.
    fn walk(infos: &[[u8; 5]]) -> String {
        let count = u8::try_from(infos.len()).unwrap();
        let mut lines = vec![
            "> 11 ff 00 01 1b 04 00 00 00 00 00 00 00 00 00 00 00 00 00 00".to_owned(),
            "< 11 ff 00 01 0b 00 06 00 00 00 00 00 00 00 00 00 00 00 00 00".to_owned(),
            format!("> {}", long(FN_GET_COUNT, &[])),
            format!("< {}", long(FN_GET_COUNT, &[count])),
        ];
        for (position, info) in (0..).zip(infos) {
            lines.push(format!("> {}", long(FN_GET_CID_INFO, &[position])));
            lines.push(format!("< {}", long(FN_GET_CID_INFO, info)));
        }
        lines.join("\n") + "\n"
    }

    const LEFT: [u8; 5] = [0x01, 0xa1, 0x00, 0x00, 0x20];
    const RIGHT: [u8; 5] = [0x01, 0xa2, 0x00, 0x00, 0x20];

    fn divert(script: &str) -> (Result<Diverted, DivertError>, FakeHidTransport) {
        let mut hidpp = Hidpp::new(
            FakeHidTransport::from_script(script).unwrap(),
            DEVICE_DIRECT,
            SoftwareId::new(0x1).unwrap(),
            Duration::from_secs(1),
        );
        let result = divert_page_buttons(&mut hidpp, &DivertTiming::default(), |_| {});
        (result, std::mem::take(hidpp.transport_mut()))
    }

    #[test]
    fn a_missing_page_button_is_refused_before_anything_is_diverted() {
        let (result, transport) = divert(&walk(&[LEFT]));
        assert_eq!(result, Err(DivertError::MissingControl { cid: PAGE_RIGHT }));
        assert!(transport.is_done());
    }

    #[test]
    fn a_page_button_that_is_not_divertable_is_refused() {
        let fixed = [0x01, 0xa2, 0x00, 0x00, 0x10];
        let (result, transport) = divert(&walk(&[LEFT, fixed]));
        assert_eq!(
            result,
            Err(DivertError::NotDivertable {
                cid: PAGE_RIGHT,
                flags: 0x10,
            })
        );
        assert!(transport.is_done());
    }

    #[test]
    fn a_divert_that_does_not_read_back_is_not_confirmed() {
        let script = walk(&[LEFT, RIGHT])
            + &format!(
                "> {}\n< {}\n> {}\n< {}\n",
                long(FN_SET_CID_REPORTING, &[0x01, 0xa1, 0x03]),
                long(FN_SET_CID_REPORTING, &[0x01, 0xa1, 0x03]),
                long(FN_GET_CID_REPORTING, &[0x01, 0xa1]),
                long(FN_GET_CID_REPORTING, &[0x01, 0xa1, 0x00]),
            );
        let (result, _) = divert(&script);
        assert_eq!(
            result,
            Err(DivertError::NotConfirmed {
                cid: PAGE_LEFT,
                flags: 0x00,
            })
        );
    }

    #[test]
    fn a_set_answered_for_another_control_is_malformed() {
        let script = walk(&[LEFT, RIGHT])
            + &format!(
                "> {}\n< {}\n",
                long(FN_SET_CID_REPORTING, &[0x01, 0xa1, 0x03]),
                long(FN_SET_CID_REPORTING, &[0x01, 0xa2, 0x03]),
            );
        let (result, _) = divert(&script);
        assert!(matches!(
            result,
            Err(DivertError::Hidpp(HidppError::Malformed(_)))
        ));
    }

    #[test]
    fn a_short_cid_info_reply_is_malformed() {
        assert!(matches!(
            control_info(&[0x01, 0xa1]),
            Err(HidppError::Malformed(_))
        ));
    }

    #[test]
    fn default_timing_matches_spec_6_1() {
        assert_eq!(
            DivertTiming::default(),
            DivertTiming {
                settle: Duration::from_millis(100),
                spacing: Duration::from_millis(20),
            }
        );
    }

    #[test]
    fn divert_errors_name_the_control() {
        assert_eq!(
            DivertError::NotDivertable {
                cid: PAGE_LEFT,
                flags: 0x10
            }
            .to_string(),
            "control 0x01a1 is not divertable (flags 0x10)"
        );
    }
}
