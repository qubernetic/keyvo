//! The VLP contextual display feature `0x19A1` (spec §5.4): the key geometry
//! the device publishes. Its index is resolved through the VLP root on every
//! connection; nothing about the grid is hard-coded.

use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::feature;

/// `getGeometry(screen)`.
const FN_GET_GEOMETRY: u8 = 1;

/// The screen keyvo asks about; the Keypad has one (captured request `01`).
const SCREEN: u8 = 1;
/// Bytes before the first key record: shape, dimension (u16), width (u16),
/// height (u16), key count, and a second count (0 in the capture, meaning
/// unconfirmed).
const GEOMETRY_HEADER_LEN: usize = 9;
/// Bytes of one key record: shape, x, y, w, h (u16 each).
const KEY_RECORD_LEN: usize = 9;

/// One key's rectangle on the panel, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRect {
    /// 1-based position in the table (row-major on the Keypad).
    pub index: u8,
    /// Shape byte of the record (1 in the capture).
    pub shape: u8,
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

/// The display geometry from display fn1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    /// Display shape byte (1 in the capture).
    pub shape: u8,
    /// The `dimension` field (0x0060 in the capture, meaning unconfirmed).
    pub dimension: u16,
    /// Panel width in pixels.
    pub width: u16,
    /// Panel height in pixels.
    pub height: u16,
    /// The keys in table order.
    pub keys: Vec<KeyRect>,
}

/// Asks the display for its geometry table, resolving the display index
/// first.
///
/// # Errors
///
/// [`HidppError::Malformed`] when the table is shorter than its key count
/// says or a key lies outside the panel, plus the request errors.
pub fn geometry<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<Geometry, HidppError> {
    let index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let reply = hidpp.request(Plane::Vlp, index, FN_GET_GEOMETRY, &[SCREEN])?;
    decode_geometry(&reply.params)
}

fn decode_geometry(params: &[u8]) -> Result<Geometry, HidppError> {
    let Some((header, records)) = params.split_first_chunk::<GEOMETRY_HEADER_LEN>() else {
        return Err(HidppError::Malformed("short geometry header".into()));
    };
    let [shape, d0, d1, w0, w1, h0, h1, count, _] = *header;
    let width = u16::from_be_bytes([w0, w1]);
    let height = u16::from_be_bytes([h0, h1]);
    let needed = usize::from(count) * KEY_RECORD_LEN;
    let Some(records) = records.get(..needed) else {
        return Err(HidppError::Malformed(format!(
            "geometry announces {count} keys but carries {} record bytes",
            records.len()
        )));
    };
    let keys = (1..=count)
        .zip(records.as_chunks::<KEY_RECORD_LEN>().0)
        .map(|(index, record)| key_rect(index, record, width, height))
        .collect::<Result<_, _>>()?;
    Ok(Geometry {
        shape,
        dimension: u16::from_be_bytes([d0, d1]),
        width,
        height,
        keys,
    })
}

fn key_rect(
    index: u8,
    record: &[u8; KEY_RECORD_LEN],
    width: u16,
    height: u16,
) -> Result<KeyRect, HidppError> {
    let [shape, x0, x1, y0, y1, w0, w1, h0, h1] = *record;
    let key = KeyRect {
        index,
        shape,
        x: u16::from_be_bytes([x0, x1]),
        y: u16::from_be_bytes([y0, y1]),
        width: u16::from_be_bytes([w0, w1]),
        height: u16::from_be_bytes([h0, h1]),
    };
    let right = u32::from(key.x) + u32::from(key.width);
    let bottom = u32::from(key.y) + u32::from(key.height);
    if right > u32::from(width) || bottom > u32::from(height) {
        return Err(HidppError::Malformed(format!(
            "key {index} at ({}, {}) {}x{} lies outside the {width}x{height} panel",
            key.x, key.y, key.width, key.height
        )));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(count: u8, records: &[[u8; KEY_RECORD_LEN]]) -> Vec<u8> {
        let mut bytes = vec![0x01, 0x00, 0x60, 0x01, 0xe0, 0x01, 0xe0, count, 0x00];
        bytes.extend(records.iter().flatten());
        bytes
    }

    const KEY_1: [u8; KEY_RECORD_LEN] = [0x01, 0x00, 0x18, 0x00, 0x06, 0x00, 0x76, 0x00, 0x76];

    #[test]
    fn padding_after_the_records_is_ignored() {
        let mut params = table(1, &[KEY_1]);
        params.resize(54, 0);
        let geometry = decode_geometry(&params).unwrap();
        assert_eq!(geometry.keys.len(), 1);
        assert_eq!((geometry.shape, geometry.dimension), (1, 0x60));
    }

    #[test]
    fn fewer_records_than_announced_is_malformed() {
        assert!(matches!(
            decode_geometry(&table(2, &[KEY_1])),
            Err(HidppError::Malformed(_))
        ));
    }

    #[test]
    fn a_short_header_is_malformed() {
        assert!(matches!(
            decode_geometry(&[0x01, 0x00]),
            Err(HidppError::Malformed(_))
        ));
    }

    #[test]
    fn a_key_outside_the_panel_is_malformed() {
        let off_panel = [0x01, 0x01, 0x80, 0x00, 0x06, 0x00, 0x76, 0x00, 0x76];
        assert!(matches!(
            decode_geometry(&table(1, &[off_panel])),
            Err(HidppError::Malformed(_))
        ));
    }
}
