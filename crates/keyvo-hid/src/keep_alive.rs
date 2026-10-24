//! Classic HID++ `0x0008 KEEP_ALIVE` (spec §5.3). This slice reads the
//! timeout range; the keep-alive loop follows with the claim sequence.

use std::time::Duration;

use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::feature;

/// `getTimeoutRange`.
const FN_GET_TIMEOUT_RANGE: u8 = 0;

/// The keep-alive timeouts the device accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutRange {
    /// Shortest accepted timeout.
    pub min: Duration,
    /// Longest accepted timeout.
    pub max: Duration,
}

/// Asks the device for its keep-alive timeout range, resolving the
/// `KEEP_ALIVE` index first.
///
/// # Errors
///
/// [`HidppError::FeatureNotPresent`] on a device without keep-alive (the
/// Dialpad), plus the request errors.
pub fn timeout_range<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<TimeoutRange, HidppError> {
    let index = hidpp.feature_index(Plane::Classic, feature::KEEP_ALIVE)?;
    let reply = hidpp.request(Plane::Classic, index, FN_GET_TIMEOUT_RANGE, &[])?;
    let [min_hi, min_lo, max_hi, max_lo, ..] = reply.params[..] else {
        return Err(HidppError::Malformed("short getTimeoutRange reply".into()));
    };
    Ok(TimeoutRange {
        min: Duration::from_millis(u16::from_be_bytes([min_hi, min_lo]).into()),
        max: Duration::from_millis(u16::from_be_bytes([max_hi, max_lo]).into()),
    })
}
