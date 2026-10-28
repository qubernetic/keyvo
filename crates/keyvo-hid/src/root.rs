//! The ROOT feature on both planes: classic HID++ `0x0000` on report `0x11`
//! and the VLP root (`0x0102`) on report `0x13`. Both sit at index 0 of their
//! plane, the one index the protocol fixes; every other index is resolved
//! through `getFeature` and cached per connection (spec §5.2).

use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};

/// Feature ids keyvo uses. Ids are stable; their indices are not.
pub mod feature {
    /// Classic ROOT.
    pub const ROOT: u16 = 0x0000;
    /// Classic `DEVICE_FW_VERSION`.
    pub const DEVICE_FW_VERSION: u16 = 0x0003;
    /// Classic `KEEP_ALIVE` (Keypad only).
    pub const KEEP_ALIVE: u16 = 0x0008;
    /// Classic `REPROG_CONTROLS_V4` (page buttons, Dialpad buttons).
    pub const REPROG_CONTROLS_V4: u16 = 0x1B04;
    /// Classic `MULTI_ROLLER` (the Dialpad's dial and roller).
    pub const MULTI_ROLLER: u16 = 0x4610;
    /// Classic `BRIGHTNESS_CONTROL`.
    pub const BRIGHTNESS_CONTROL: u16 = 0x8040;
    /// VLP contextual display (LCD keys).
    pub const VLP_DISPLAY: u16 = 0x19A1;
}

/// Index of the root feature on either plane, fixed by the protocol.
const ROOT_INDEX: u8 = 0;
/// ROOT `getFeature`, on both planes.
const FN_GET_FEATURE: u8 = 0;
/// Classic ROOT `ping` (`getProtocolVersion`).
const FN_PING: u8 = 1;
/// VLP root `ProtocolCapabilities`.
const FN_VLP_PROTOCOL: u8 = 1;

/// A VLP root `ProtocolCapabilities` answer. Field names come from the
/// Options+ inventory; the widths are the capture decode (medium confidence,
/// hardware check in M0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlpProtocol {
    /// VLP protocol major version.
    pub major: u8,
    /// VLP protocol minor version.
    pub minor: u8,
    /// Memory available to the VLP features in bytes, big-endian u32.
    pub available_memory: u32,
}

/// The HID++ protocol version a classic ping reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// Major version (4 on the Keypad).
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

/// A classic `getFeature` answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureInfo {
    /// Feature index on the classic plane.
    pub index: u8,
    /// Feature type flags (obsolete, hidden, engineering).
    pub flags: u8,
    /// Feature version.
    pub version: u8,
}

/// A VLP root `getFeature` answer. Field names come from the Options+ log
/// strings (`GetFeature(id) -> id, index, hidden, version, maxMemory`); the
/// widths are the capture decode (medium confidence, hardware check in M0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlpFeatureInfo {
    /// Feature index on the VLP plane.
    pub index: u8,
    /// Hidden flag byte.
    pub hidden: u8,
    /// Feature version.
    pub version: u8,
    /// Maximum memory the feature accepts, big-endian u32.
    pub max_memory: u32,
}

impl<T: HidTransport> Hidpp<T> {
    /// Classic ROOT ping: returns the protocol version; the device must echo
    /// `data`.
    ///
    /// # Errors
    ///
    /// [`HidppError::Malformed`] when the echo differs, plus the request errors.
    pub fn ping(&mut self, data: u8) -> Result<ProtocolVersion, HidppError> {
        let reply = self.request(Plane::Classic, ROOT_INDEX, FN_PING, &[0, 0, data])?;
        let [major, minor, echo, ..] = reply.params[..] else {
            return Err(HidppError::Malformed("short ping reply".into()));
        };
        if echo != data {
            return Err(HidppError::Malformed(format!(
                "ping echoed 0x{echo:02x} instead of 0x{data:02x}"
            )));
        }
        Ok(ProtocolVersion { major, minor })
    }

    /// Classic ROOT `getFeature(id)`; `None` when the device does not have it.
    /// A present feature's index is cached for [`Hidpp::feature_index`].
    ///
    /// # Errors
    ///
    /// The request errors.
    pub fn get_feature(&mut self, id: u16) -> Result<Option<FeatureInfo>, HidppError> {
        let reply = self.request(
            Plane::Classic,
            ROOT_INDEX,
            FN_GET_FEATURE,
            &id.to_be_bytes(),
        )?;
        let [index, flags, version, ..] = reply.params[..] else {
            return Err(HidppError::Malformed("short getFeature reply".into()));
        };
        let present = index != ROOT_INDEX || id == feature::ROOT;
        if present {
            self.features.insert((Plane::Classic, id), index);
        }
        Ok(present.then_some(FeatureInfo {
            index,
            flags,
            version,
        }))
    }

    /// VLP root `ProtocolCapabilities`, the first request of the display
    /// claim (spec §5.4).
    ///
    /// # Errors
    ///
    /// [`HidppError::Malformed`] for a short reply, plus the request errors.
    pub fn vlp_protocol(&mut self) -> Result<VlpProtocol, HidppError> {
        let reply = self.request(Plane::Vlp, ROOT_INDEX, FN_VLP_PROTOCOL, &[])?;
        let [major, minor, m0, m1, m2, m3, ..] = reply.params[..] else {
            return Err(HidppError::Malformed("short VLP protocol reply".into()));
        };
        Ok(VlpProtocol {
            major,
            minor,
            available_memory: u32::from_be_bytes([m0, m1, m2, m3]),
        })
    }

    /// VLP root `getFeature(id)`; `None` when the device does not have it.
    /// A present feature's index is cached for [`Hidpp::feature_index`].
    ///
    /// # Errors
    ///
    /// [`HidppError::Malformed`] when the reply does not echo `id`, plus the
    /// request errors.
    pub fn vlp_get_feature(&mut self, id: u16) -> Result<Option<VlpFeatureInfo>, HidppError> {
        let reply = self.request(Plane::Vlp, ROOT_INDEX, FN_GET_FEATURE, &id.to_be_bytes())?;
        let [hi, lo, index, hidden, version, m0, m1, m2, m3, ..] = reply.params[..] else {
            return Err(HidppError::Malformed("short VLP getFeature reply".into()));
        };
        let echoed = u16::from_be_bytes([hi, lo]);
        if echoed != id {
            return Err(HidppError::Malformed(format!(
                "VLP getFeature(0x{id:04x}) answered for 0x{echoed:04x}"
            )));
        }
        if index != ROOT_INDEX {
            self.features.insert((Plane::Vlp, id), index);
        }
        Ok((index != ROOT_INDEX).then_some(VlpFeatureInfo {
            index,
            hidden,
            version,
            max_memory: u32::from_be_bytes([m0, m1, m2, m3]),
        }))
    }

    /// The index of feature `id` on `plane`, asked once per connection and
    /// cached.
    ///
    /// # Errors
    ///
    /// [`HidppError::FeatureNotPresent`] when the device does not have it,
    /// plus the request errors.
    pub fn feature_index(&mut self, plane: Plane, id: u16) -> Result<u8, HidppError> {
        if let Some(index) = self.features.get(&(plane, id)) {
            return Ok(*index);
        }
        let index = match plane {
            Plane::Classic => self.get_feature(id)?.map(|f| f.index),
            Plane::Vlp => self.vlp_get_feature(id)?.map(|f| f.index),
        }
        .ok_or(HidppError::FeatureNotPresent { plane, feature: id })?;
        self.features.insert((plane, id), index);
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidpp::{DEVICE_DIRECT, SoftwareId};
    use keyvo_core::hid::FakeHidTransport;
    use std::time::Duration;

    fn hidpp(request: &[u8], reply: &[u8]) -> Hidpp<FakeHidTransport> {
        Hidpp::new(
            FakeHidTransport::new().expect(request, &[reply]),
            DEVICE_DIRECT,
            SoftwareId::new(0x1).unwrap(),
            Duration::from_secs(1),
        )
    }

    fn report(len: usize, head: &[u8]) -> Vec<u8> {
        let mut bytes = head.to_vec();
        bytes.resize(len, 0);
        bytes
    }

    #[test]
    fn ping_with_a_wrong_echo_is_malformed() {
        let mut hidpp = hidpp(
            &report(20, &[0x11, 0xff, 0x00, 0x11, 0x00, 0x00, 0x39]),
            &report(20, &[0x11, 0xff, 0x00, 0x11, 0x04, 0x05, 0x3a]),
        );
        assert!(matches!(hidpp.ping(0x39), Err(HidppError::Malformed(_))));
    }

    #[test]
    fn vlp_get_feature_for_another_id_is_malformed() {
        let mut hidpp = hidpp(
            &report(32, &[0x13, 0xff, 0x00, 0x01, 0xe0, 0x19, 0xa1]),
            &report(32, &[0x13, 0xff, 0x00, 0x01, 0xc0, 0x19, 0xa2, 0x02]),
        );
        assert!(matches!(
            hidpp.vlp_get_feature(feature::VLP_DISPLAY),
            Err(HidppError::Malformed(_))
        ));
    }

    #[test]
    fn vlp_get_feature_index_0_means_absent() {
        let mut hidpp = hidpp(
            &report(32, &[0x13, 0xff, 0x00, 0x01, 0xe0, 0x19, 0xa1]),
            &report(32, &[0x13, 0xff, 0x00, 0x01, 0xc0, 0x19, 0xa1, 0x00]),
        );
        assert_eq!(
            hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY),
            Err(HidppError::FeatureNotPresent {
                plane: Plane::Vlp,
                feature: feature::VLP_DISPLAY,
            })
        );
    }

    #[test]
    fn root_itself_is_present_at_index_0() {
        let mut hidpp = hidpp(
            &report(20, &[0x11, 0xff, 0x00, 0x01, 0x00, 0x00]),
            &report(20, &[0x11, 0xff, 0x00, 0x01, 0x00, 0x00, 0x02]),
        );
        assert_eq!(
            hidpp.get_feature(feature::ROOT).unwrap(),
            Some(FeatureInfo {
                index: 0,
                flags: 0,
                version: 2,
            })
        );
    }
}
