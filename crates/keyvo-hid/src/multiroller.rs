//! Classic HID++ `0x4610 MULTI_ROLLER` (spec §5.8): the Dialpad's dial and
//! roller. `fn0 getCapabilities` gives the roller count, `fn1
//! getRollerCapabilities(r)` each roller's increments per rotation, and
//! `fn3 setMode(r, mode)` diverts a roller, whose turns then arrive as
//! rotation notifications (event 0) instead of scroll input.
//!
//! Layouts from the static pass (`docs/research/2026-10-28-dialpad-static-pass.md`
//! §1 to §5): roller ids are `0..count`, always masked `& 0x0F`; the
//! reporting mode is one bit.

use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::feature;

/// `getCapabilities`.
const FN_GET_CAPABILITIES: u8 = 0;
/// `getRollerCapabilities(roller)`.
const FN_GET_ROLLER_CAPABILITIES: u8 = 1;
/// `setMode(roller, mode)`.
const FN_SET_MODE: u8 = 3;
/// The event number of a rotation notification (the function nibble).
pub const ROTATION_EVENT: u8 = 0;
/// Roller ids and the roller count are four bits wide.
const ROLLER_MASK: u8 = 0x0f;

/// How a roller reports its turns (`ReportingMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `0`: the device's own scroll input.
    Native,
    /// `1`: rotation notifications to the host.
    Diverted,
}

impl Mode {
    const fn byte(self) -> u8 {
        match self {
            Self::Native => 0,
            Self::Diverted => 1,
        }
    }
}

/// One roller from `getRollerCapabilities`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roller {
    /// Roller id, `0..count`.
    pub id: u8,
    /// Increments per full rotation (byte 0).
    pub increments: u8,
    /// Ratchet increments (byte 1); 0 on the Dialpad, which has no ratchet.
    pub ratchets: u8,
}

/// One rotation notification: `<roller> <delta:i8> [<timestamp:u32 BE>]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rotation {
    /// Roller id (low nibble of byte 0).
    pub roller: u8,
    /// Signed increments since the last notification.
    pub delta: i8,
    /// The device's timestamp; 0 when the event carries none. Unit and wrap
    /// are open (M3-2).
    pub timestamp: u32,
}

/// Every roller of the device: `getCapabilities`, then
/// `getRollerCapabilities` per roller, resolving the `0x4610` index first.
///
/// # Errors
///
/// [`HidppError::FeatureNotPresent`] without the feature,
/// [`HidppError::Malformed`] for a short reply, plus the request errors.
pub fn rollers<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<Vec<Roller>, HidppError> {
    let index = hidpp.feature_index(Plane::Classic, feature::MULTI_ROLLER)?;
    let reply = hidpp.request(Plane::Classic, index, FN_GET_CAPABILITIES, &[])?;
    let count = reply
        .params
        .first()
        .ok_or_else(|| HidppError::Malformed("empty getCapabilities reply".into()))?
        & ROLLER_MASK;
    (0..count)
        .map(|id| {
            let reply = hidpp.request(Plane::Classic, index, FN_GET_ROLLER_CAPABILITIES, &[id])?;
            let [increments, ratchets, ..] = reply.params[..] else {
                return Err(HidppError::Malformed(
                    "short getRollerCapabilities reply".into(),
                ));
            };
            Ok(Roller {
                id,
                increments,
                ratchets,
            })
        })
        .collect()
}

/// `setMode(roller, mode)`; the reply carries nothing keyvo reads.
///
/// # Errors
///
/// [`HidppError::FeatureNotPresent`] without the feature, plus the request
/// errors.
pub fn set_mode<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    roller: u8,
    mode: Mode,
) -> Result<(), HidppError> {
    let index = hidpp.feature_index(Plane::Classic, feature::MULTI_ROLLER)?;
    hidpp.request(
        Plane::Classic,
        index,
        FN_SET_MODE,
        &[roller & ROLLER_MASK, mode.byte()],
    )?;
    Ok(())
}

/// The rotation a notification's parameters carry; `None` for fewer than
/// two bytes. A payload without the timestamp bytes reads timestamp 0.
#[must_use]
pub fn rotation(params: &[u8]) -> Option<Rotation> {
    let [roller, delta, rest @ ..] = params else {
        return None;
    };
    let timestamp = match rest {
        [a, b, c, d, ..] => u32::from_be_bytes([*a, *b, *c, *d]),
        _ => 0,
    };
    Some(Rotation {
        roller: roller & ROLLER_MASK,
        delta: i8::from_be_bytes([*delta]),
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_reads_the_delta_as_a_signed_byte_at_its_edges() {
        let delta = |byte| rotation(&[0x01, byte]).map(|r| r.delta);
        assert_eq!(delta(0x7f), Some(127));
        assert_eq!(delta(0x80), Some(-128));
        assert_eq!(delta(0xff), Some(-1));
        assert_eq!(delta(0x03), Some(3));
    }

    #[test]
    fn rotation_masks_the_roller_and_reads_an_optional_timestamp() {
        assert_eq!(
            rotation(&[0xf1, 0x02, 0x00, 0x01, 0x02, 0x03]),
            Some(Rotation {
                roller: 1,
                delta: 2,
                timestamp: 0x0001_0203,
            })
        );
        assert_eq!(rotation(&[0x00, 0x02, 0x09]).map(|r| r.timestamp), Some(0));
        assert_eq!(rotation(&[0x00]), None);
        assert_eq!(rotation(&[]), None);
    }
}
