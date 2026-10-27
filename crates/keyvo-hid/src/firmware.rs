//! Classic HID++ `0x0003 DEVICE_FW_VERSION`: the firmware entities of a
//! device (application, bootloader, hardware, ...) for `keyvo probe` and the
//! hardware notes, which record the firmware with every measurement.

use std::fmt;

use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::feature;

/// `getEntityCount`.
const FN_GET_ENTITY_COUNT: u8 = 0;
/// `getFwInfo(entity)`.
const FN_GET_FW_INFO: u8 = 1;

/// What a firmware entity is (the type byte of `getFwInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareKind {
    /// `0`: the main application.
    Application,
    /// `1`: the bootloader.
    Bootloader,
    /// `2`: the hardware revision.
    Hardware,
    /// Any other type byte (`0x0b` on the Keypad, meaning unknown).
    Other(u8),
}

impl From<u8> for FirmwareKind {
    fn from(byte: u8) -> Self {
        match byte {
            0 => Self::Application,
            1 => Self::Bootloader,
            2 => Self::Hardware,
            other => Self::Other(other),
        }
    }
}

impl fmt::Display for FirmwareKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => f.write_str("application"),
            Self::Bootloader => f.write_str("bootloader"),
            Self::Hardware => f.write_str("hardware"),
            Self::Other(byte) => write!(f, "type 0x{byte:02x}"),
        }
    }
}

/// One `getFwInfo` answer. Displays as Solaar prints it: `U1 66.00.B0017`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareEntity {
    /// What the entity is.
    pub kind: FirmwareKind,
    /// The three-character prefix without trailing NULs and spaces (`U1`).
    pub name: String,
    /// Version number, BCD (`0x66` reads 66).
    pub version: u8,
    /// Revision, BCD.
    pub revision: u8,
    /// Build, BCD.
    pub build: u16,
    /// True for the entity that is running.
    pub active: bool,
    /// The product id the entity reports.
    pub transport_pid: u16,
}

impl fmt::Display for FirmwareEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:02X}.{:02X}.B{:04X}",
            self.name, self.version, self.revision, self.build
        )
    }
}

/// Every firmware entity of the device, resolving the `0x0003` index first.
///
/// # Errors
///
/// [`HidppError::FeatureNotPresent`] without the feature,
/// [`HidppError::Malformed`] for a short reply, plus the request errors.
pub fn entities<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<Vec<FirmwareEntity>, HidppError> {
    let index = hidpp.feature_index(Plane::Classic, feature::DEVICE_FW_VERSION)?;
    let reply = hidpp.request(Plane::Classic, index, FN_GET_ENTITY_COUNT, &[])?;
    let count = *reply
        .params
        .first()
        .ok_or_else(|| HidppError::Malformed("empty getEntityCount reply".into()))?;
    (0..count)
        .map(|entity| {
            let reply = hidpp.request(Plane::Classic, index, FN_GET_FW_INFO, &[entity])?;
            decode_entity(&reply.params)
        })
        .collect()
}

/// The running application entity, the firmware version to record.
#[must_use]
pub fn active_application(entities: &[FirmwareEntity]) -> Option<&FirmwareEntity> {
    entities
        .iter()
        .find(|entity| entity.kind == FirmwareKind::Application && entity.active)
}

fn decode_entity(params: &[u8]) -> Result<FirmwareEntity, HidppError> {
    let [
        kind,
        p0,
        p1,
        p2,
        version,
        revision,
        b0,
        b1,
        active,
        pid0,
        pid1,
        ..,
    ] = params[..]
    else {
        return Err(HidppError::Malformed("short getFwInfo reply".into()));
    };
    let name = String::from_utf8_lossy(&[p0, p1, p2])
        .trim_end_matches(['\0', ' '])
        .to_owned();
    Ok(FirmwareEntity {
        kind: FirmwareKind::from(kind),
        name,
        version,
        revision,
        build: u16::from_be_bytes([b0, b1]),
        active: active & 0x01 != 0,
        transport_pid: u16::from_be_bytes([pid0, pid1]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidpp::{DEVICE_DIRECT, SoftwareId};
    use keyvo_core::hid::FakeHidTransport;
    use std::time::Duration;

    fn long(head: &[u8]) -> Vec<u8> {
        let mut bytes = head.to_vec();
        bytes.resize(20, 0);
        bytes
    }

    #[test]
    fn a_device_without_entities_has_none() {
        let fake = FakeHidTransport::new()
            .expect(
                &long(&[0x11, 0xff, 0x00, 0x0e, 0x00, 0x03]),
                &[&long(&[0x11, 0xff, 0x00, 0x0e, 0x02])],
            )
            .expect(
                &long(&[0x11, 0xff, 0x02, 0x0e]),
                &[&long(&[0x11, 0xff, 0x02, 0x0e, 0x00])],
            );
        let mut hidpp = Hidpp::new(
            fake,
            DEVICE_DIRECT,
            SoftwareId::new(0xe).unwrap(),
            Duration::from_secs(1),
        );
        assert_eq!(entities(&mut hidpp), Ok(vec![]));
        assert_eq!(active_application(&[]), None);
    }

    #[test]
    fn a_short_fw_info_reply_is_malformed() {
        assert!(matches!(
            decode_entity(&[0, 0x55, 0x31]),
            Err(HidppError::Malformed(_))
        ));
        assert_eq!(FirmwareKind::Other(0x0b).to_string(), "type 0x0b");
        assert_eq!(FirmwareKind::Application.to_string(), "application");
    }
}
