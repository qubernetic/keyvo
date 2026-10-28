//! Finding the Keypad's hidraw node through sysfs (spec §5.1): every
//! `/sys/class/hidraw/<node>` names its HID device's bus, vendor and product
//! in `device/uevent`, and its report descriptor in
//! `device/report_descriptor`. keyvo picks the node whose descriptor carries
//! the bulk report `0x14`, never the first node that matches on VID/PID.
//! The Bolt receiver's node, the Dialpad's path, is the receiver node that
//! carries the HID++ reports `0x10` and `0x11`.

use std::fmt;
use std::path::{Path, PathBuf};

/// The sysfs class directory of the hidraw nodes.
pub const SYSFS_HIDRAW: &str = "/sys/class/hidraw";
/// Where the device nodes live.
pub const DEV_DIR: &str = "/dev";
/// Logitech's USB vendor id.
pub const LOGITECH: u16 = 0x046d;
/// USB product id of the MX Creative Keypad.
pub const MX_CREATIVE_KEYPAD: u16 = 0xc354;
/// The report a node must carry to be the Keypad's: the VLP bulk report of
/// the LCD image fragments.
pub const BULK_REPORT: u8 = 0x14;
/// USB product id of the Logi Bolt receiver.
pub const BOLT_RECEIVER: u16 = 0xc548;
/// Product id of the MX Creative Dialpad (its Bolt WPID too).
pub const MX_CREATIVE_DIALPAD: u16 = 0xbc00;
/// The reports a receiver node must carry to be the one keyvo talks
/// HID++ on: the short and the long HID++ report.
pub const RECEIVER_REPORTS: [u8; 2] = [0x10, 0x11];
/// Bus type of a Bluetooth HID device.
pub const BUS_BLUETOOTH: u16 = 0x0005;

/// Bus, vendor and product of a HID device (`HID_ID` in its uevent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidId {
    /// Bus type (`0003` = USB, `0005` = Bluetooth).
    pub bus: u16,
    /// Vendor id.
    pub vendor: u16,
    /// Product id.
    pub product: u16,
}

impl fmt::Display for HidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.product)
    }
}

/// One hidraw node and what its HID device says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Node name, `hidraw2`.
    pub name: String,
    /// Device path, `/dev/hidraw2`.
    pub path: PathBuf,
    /// Bus, vendor and product.
    pub id: HidId,
    /// `HID_NAME`, empty when the uevent has none.
    pub hid_name: String,
    /// The report ids the descriptor declares, ascending.
    pub report_ids: Vec<u8>,
}

/// Errors of the node search.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SysfsError {
    /// A sysfs file or directory could not be read.
    #[error("cannot read {path}: {message}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The OS message.
        message: String,
    },
    /// A report descriptor ends inside an item.
    #[error("report descriptor of {node} is truncated at byte {offset}")]
    TruncatedDescriptor {
        /// The node.
        node: String,
        /// Offset of the item that does not fit.
        offset: usize,
    },
    /// No node of the device.
    #[error("no hidraw node for {vendor:04x}:{product:04x}")]
    NotFound {
        /// Vendor id searched for.
        vendor: u16,
        /// Product id searched for.
        product: u16,
    },
    /// Nodes of the device, but none with the wanted report.
    #[error(
        "no hidraw node of {vendor:04x}:{product:04x} carries report 0x{report:02x}; found {found}"
    )]
    NoNodeWithReport {
        /// Vendor id searched for.
        vendor: u16,
        /// Product id searched for.
        product: u16,
        /// The wanted report id.
        report: u8,
        /// The nodes found and their reports, `hidraw4 (04 05)`.
        found: String,
    },
}

/// The report ids a HID report descriptor declares (its Report ID global
/// items), ascending and without repeats; empty for a device without report
/// ids.
///
/// # Errors
///
/// The offset of an item that runs past the end.
pub fn report_ids(descriptor: &[u8]) -> Result<Vec<u8>, usize> {
    let mut ids = Vec::new();
    let mut offset = 0;
    while let Some(&prefix) = descriptor.get(offset) {
        let (header, size) = if prefix == LONG_ITEM {
            // `fe <data size> <long tag> <data>`.
            let size = descriptor.get(offset + 1).ok_or(offset)?;
            (3, usize::from(*size))
        } else {
            // Size code 3 means four bytes.
            (1, [0, 1, 2, 4][usize::from(prefix & 0x03)])
        };
        let data = descriptor
            .get(offset + header..offset + header + size)
            .ok_or(offset)?;
        if prefix & !0x03 == REPORT_ID_ITEM
            && let Some(&id) = data.first()
        {
            ids.push(id);
        }
        offset += header + size;
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// Prefix of a long item.
const LONG_ITEM: u8 = 0xfe;
/// Report ID: global item (type 1), tag 8, size bits cleared.
const REPORT_ID_ITEM: u8 = 0x84;

/// Every hidraw node under `class_dir` whose uevent names a HID device,
/// ordered by node number; device paths are under `dev_dir`. A missing
/// `class_dir` (no hidraw support loaded) is no node.
///
/// # Errors
///
/// [`SysfsError::Io`] when a directory or file cannot be read and
/// [`SysfsError::TruncatedDescriptor`].
pub fn enumerate(class_dir: &Path, dev_dir: &Path) -> Result<Vec<Node>, SysfsError> {
    let entries = match std::fs::read_dir(class_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(class_dir, &error)),
    };
    let mut nodes = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io_error(class_dir, &error))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let device = entry.path().join("device");
        let uevent_path = device.join("uevent");
        let uevent =
            std::fs::read_to_string(&uevent_path).map_err(|e| io_error(&uevent_path, &e))?;
        let Some(id) = uevent_value(&uevent, "HID_ID").and_then(parse_hid_id) else {
            continue;
        };
        let descriptor_path = device.join("report_descriptor");
        let descriptor =
            std::fs::read(&descriptor_path).map_err(|e| io_error(&descriptor_path, &e))?;
        let report_ids =
            report_ids(&descriptor).map_err(|offset| SysfsError::TruncatedDescriptor {
                node: name.clone(),
                offset,
            })?;
        nodes.push(Node {
            path: dev_dir.join(&name),
            hid_name: uevent_value(&uevent, "HID_NAME")
                .unwrap_or_default()
                .to_owned(),
            name,
            id,
            report_ids,
        });
    }
    nodes.sort_by_key(|node| node_number(&node.name));
    Ok(nodes)
}

/// The node of `vendor:product` that carries `report`.
///
/// # Errors
///
/// [`SysfsError::NotFound`] without any node of the device,
/// [`SysfsError::NoNodeWithReport`] when none of them carries `report`.
pub fn select(nodes: &[Node], vendor: u16, product: u16, report: u8) -> Result<&Node, SysfsError> {
    let candidates: Vec<_> = nodes
        .iter()
        .filter(|node| node.id.vendor == vendor && node.id.product == product)
        .collect();
    if candidates.is_empty() {
        return Err(SysfsError::NotFound { vendor, product });
    }
    candidates
        .iter()
        .find(|node| node.report_ids.contains(&report))
        .copied()
        .ok_or_else(|| SysfsError::NoNodeWithReport {
            vendor,
            product,
            report,
            found: candidates
                .iter()
                .map(|node| {
                    format!(
                        "{} ({})",
                        node.name,
                        keyvo_core::hid::to_hex(&node.report_ids)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        })
}

fn io_error(path: &Path, error: &std::io::Error) -> SysfsError {
    SysfsError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

/// The value of `KEY=value` in a uevent.
fn uevent_value<'a>(uevent: &'a str, key: &str) -> Option<&'a str> {
    uevent.lines().find_map(|line| {
        line.split_once('=')
            .filter(|(name, _)| *name == key)
            .map(|(_, value)| value)
    })
}

/// `0003:0000046D:0000C354`: bus, vendor and product in hex.
fn parse_hid_id(value: &str) -> Option<HidId> {
    let mut fields = value
        .split(':')
        .map(|field| u32::from_str_radix(field, 16).ok());
    let (Some(Some(bus)), Some(Some(vendor)), Some(Some(product)), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    Some(HidId {
        bus: u16::try_from(bus).ok()?,
        vendor: u16::try_from(vendor).ok()?,
        product: u16::try_from(product).ok()?,
    })
}

/// `hidraw10` sorts after `hidraw9`; other names sort last, by name.
fn node_number(name: &str) -> (u32, String) {
    let number = name
        .strip_prefix("hidraw")
        .and_then(|n| n.parse().ok())
        .unwrap_or(u32::MAX);
    (number, name.to_owned())
}

/// The Keypad's node: [`enumerate`] then [`select`] for
/// [`MX_CREATIVE_KEYPAD`] and [`BULK_REPORT`].
///
/// # Errors
///
/// The errors of both.
pub fn find_keypad(class_dir: &Path, dev_dir: &Path) -> Result<Node, SysfsError> {
    let nodes = enumerate(class_dir, dev_dir)?;
    select(&nodes, LOGITECH, MX_CREATIVE_KEYPAD, BULK_REPORT).cloned()
}

/// The Bolt receiver's HID++ node: among the nodes of
/// [`BOLT_RECEIVER`] the first that carries both [`RECEIVER_REPORTS`]
/// (the kernel's `hid-logitech-dj` gives the receiver's keyboard and mouse
/// interfaces nodes of their own). `None` without one.
///
/// # Errors
///
/// The errors of [`enumerate`].
pub fn find_receiver(class_dir: &Path, dev_dir: &Path) -> Result<Option<Node>, SysfsError> {
    Ok(enumerate(class_dir, dev_dir)?.into_iter().find(|node| {
        node.id.vendor == LOGITECH
            && node.id.product == BOLT_RECEIVER
            && RECEIVER_REPORTS
                .iter()
                .all(|report| node.report_ids.contains(report))
    }))
}

/// The node behind a `--device` path, by its node name (`hidraw3` of
/// `/dev/hidraw3`); `None` when sysfs has no node of that name.
///
/// # Errors
///
/// The errors of [`enumerate`].
pub fn node_at(class_dir: &Path, dev_dir: &Path, path: &Path) -> Result<Option<Node>, SysfsError> {
    let Some(name) = path.file_name() else {
        return Ok(None);
    };
    Ok(enumerate(class_dir, dev_dir)?
        .into_iter()
        .find(|node| std::ffi::OsStr::new(&node.name) == name))
}

impl Node {
    /// True for a Dialpad paired over Bluetooth, which answers on device
    /// index `0xff` like a USB device. Over the receiver the kernel gives
    /// the Dialpad a child node on the USB bus; keyvo talks to the
    /// receiver's node instead.
    #[must_use]
    pub const fn is_bluetooth_dialpad(&self) -> bool {
        self.id.bus == BUS_BLUETOOTH
            && self.id.vendor == LOGITECH
            && self.id.product == MX_CREATIVE_DIALPAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sysfs");

    /// A copy of the fixture tree `scenario` with each `report_descriptor.hex`
    /// turned into the binary `report_descriptor` sysfs has.
    fn sysfs(scenario: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        copy(&Path::new(FIXTURES).join(scenario), root.path());
        root
    }

    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let source = entry.path();
            if source.is_dir() {
                copy(&source, &to.join(&name));
            } else if name == "report_descriptor.hex" {
                let text = fs::read_to_string(&source).unwrap();
                fs::write(to.join("report_descriptor"), hex(&text)).unwrap();
            } else {
                fs::copy(&source, to.join(&name)).unwrap();
            }
        }
    }

    fn hex(text: &str) -> Vec<u8> {
        text.lines()
            .filter(|line| !line.starts_with('#'))
            .flat_map(str::split_whitespace)
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }

    fn descriptor(scenario: &str, node: &str) -> Vec<u8> {
        let path = Path::new(FIXTURES)
            .join(scenario)
            .join("class/hidraw")
            .join(node)
            .join("device/report_descriptor.hex");
        hex(&fs::read_to_string(path).unwrap())
    }

    fn find(scenario: &str) -> Result<Node, SysfsError> {
        let root = sysfs(scenario);
        find_keypad(&root.path().join("class/hidraw"), Path::new(DEV_DIR))
    }

    #[test]
    fn the_keypad_descriptor_declares_its_five_reports() {
        let keypad = descriptor("keypad-and-receiver", "hidraw2");
        assert_eq!(report_ids(&keypad), Ok(vec![0x04, 0x05, 0x11, 0x13, 0x14]));
        let receiver = descriptor("keypad-and-receiver", "hidraw9");
        assert_eq!(report_ids(&receiver), Ok(vec![0x10, 0x11]));
    }

    #[test]
    fn descriptor_items_of_every_size_and_long_items_are_skipped() {
        // Logical Maximum with four data bytes that hold 0x85 0x99, a long
        // item (fe, size 2, tag) whose data hold 0x85 0x98, then Report ID 7.
        let descriptor = [
            0x27, 0x85, 0x99, 0x00, 0x00, 0xfe, 0x02, 0x10, 0x85, 0x98, 0x85, 0x07, 0x85, 0x07,
        ];
        assert_eq!(report_ids(&descriptor), Ok(vec![0x07]));
        assert_eq!(report_ids(&[]), Ok(vec![]));
    }

    #[test]
    fn a_truncated_descriptor_names_the_item_offset() {
        assert_eq!(report_ids(&[0x75, 0x08, 0x26, 0xff]), Err(2));
        assert_eq!(report_ids(&[0xfe, 0x04, 0x10, 0x00]), Err(0));
        assert_eq!(report_ids(&[0xfe]), Err(0));
    }

    #[test]
    fn the_keypad_node_is_found_beside_the_receiver() {
        let node = find("keypad-and-receiver").unwrap();
        assert_eq!(node.name, "hidraw2");
        assert_eq!(node.path, Path::new("/dev/hidraw2"));
        assert_eq!(
            node.id,
            HidId {
                bus: 0x0003,
                vendor: LOGITECH,
                product: MX_CREATIVE_KEYPAD
            }
        );
        assert_eq!(node.hid_name, "Logitech MX Creative Keypad");
        assert_eq!(node.id.to_string(), "046d:c354");
    }

    #[test]
    fn the_node_with_the_bulk_report_wins_over_the_first_match() {
        let root = sysfs("keypad-split");
        let nodes = enumerate(&root.path().join("class/hidraw"), Path::new(DEV_DIR)).unwrap();
        let names: Vec<_> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["hidraw4", "hidraw5"]);
        assert_eq!(nodes[0].report_ids, [0x04, 0x05]);
        let node = select(&nodes, LOGITECH, MX_CREATIVE_KEYPAD, BULK_REPORT).unwrap();
        assert_eq!(node.name, "hidraw5");
    }

    #[test]
    fn a_keypad_without_the_bulk_report_is_refused_with_what_was_found() {
        assert_eq!(
            find("keypad-without-bulk"),
            Err(SysfsError::NoNodeWithReport {
                vendor: LOGITECH,
                product: MX_CREATIVE_KEYPAD,
                report: BULK_REPORT,
                found: "hidraw2 (11 13)".into(),
            })
        );
    }

    #[test]
    fn no_keypad_is_not_found() {
        assert_eq!(
            find("receiver-only"),
            Err(SysfsError::NotFound {
                vendor: LOGITECH,
                product: MX_CREATIVE_KEYPAD
            })
        );
        let empty = tempfile::tempdir().unwrap();
        assert!(matches!(
            find_keypad(empty.path(), Path::new(DEV_DIR)),
            Err(SysfsError::NotFound { .. })
        ));
    }

    fn receiver(scenario: &str) -> Option<Node> {
        let root = sysfs(scenario);
        find_receiver(&root.path().join("class/hidraw"), Path::new(DEV_DIR)).unwrap()
    }

    #[test]
    fn the_receiver_node_is_found_beside_the_keypad_and_alone() {
        for scenario in ["keypad-and-receiver", "receiver-only"] {
            let node = receiver(scenario).unwrap();
            assert_eq!(node.name, "hidraw9", "{scenario}");
            assert_eq!(node.id.to_string(), "046d:c548");
            assert_eq!(node.report_ids, [0x10, 0x11]);
            assert!(!node.is_bluetooth_dialpad());
        }
        assert_eq!(receiver("keypad-split"), None);
    }

    /// A class directory with one node per `(name, HID_ID, descriptor)`.
    fn class_dir(nodes: &[(&str, &str, &[u8])]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for (node, id, descriptor) in nodes {
            let device = root.path().join(node).join("device");
            fs::create_dir_all(&device).unwrap();
            fs::write(device.join("uevent"), format!("HID_ID={id}\n")).unwrap();
            fs::write(device.join("report_descriptor"), descriptor).unwrap();
        }
        root
    }

    #[test]
    fn only_the_receiver_node_with_both_hidpp_reports_is_taken() {
        // The receiver's keyboard and mouse interfaces have nodes of their
        // own; the HID++ one carries 0x10 and 0x11.
        let receiver = "0003:0000046D:0000C548";
        let root = class_dir(&[
            ("hidraw7", receiver, &[0x85, 0x01]),
            ("hidraw8", receiver, &[0x85, 0x10]),
            ("hidraw9", receiver, &[0x85, 0x10, 0x85, 0x11, 0x85, 0x20]),
        ]);
        let node = find_receiver(root.path(), Path::new(DEV_DIR)).unwrap();
        assert_eq!(node.map(|n| n.name), Some("hidraw9".to_owned()));
        let root = class_dir(&[("hidraw7", receiver, &[0x85, 0x01])]);
        assert_eq!(find_receiver(root.path(), Path::new(DEV_DIR)), Ok(None));
    }

    #[test]
    fn a_device_path_is_looked_up_by_its_node_name() {
        let root = class_dir(&[
            ("hidraw3", "0005:0000046D:0000BC00", &[0x85, 0x11]),
            (
                "hidraw9",
                "0003:0000046D:0000C548",
                &[0x85, 0x10, 0x85, 0x11],
            ),
        ]);
        let dialpad = node_at(root.path(), Path::new(DEV_DIR), Path::new("/dev/hidraw3"))
            .unwrap()
            .unwrap();
        assert!(dialpad.is_bluetooth_dialpad());
        let receiver = node_at(root.path(), Path::new(DEV_DIR), Path::new("hidraw9")).unwrap();
        assert!(!receiver.unwrap().is_bluetooth_dialpad());
        assert_eq!(
            node_at(root.path(), Path::new(DEV_DIR), Path::new("/tmp/fake")),
            Ok(None)
        );
        // The Dialpad as a child of the receiver's DJ driver is not the
        // Bluetooth one.
        let root = class_dir(&[("hidraw4", "0003:0000046D:0000BC00", &[0x85, 0x11])]);
        let child = node_at(root.path(), Path::new(DEV_DIR), Path::new("/dev/hidraw4"));
        assert!(!child.unwrap().unwrap().is_bluetooth_dialpad());
    }

    #[test]
    fn nodes_sort_by_number_not_by_name() {
        let root = tempfile::tempdir().unwrap();
        let class = root.path().join("class/hidraw");
        for (node, product) in [("hidraw10", "0000C354"), ("hidraw9", "0000C548")] {
            let device = class.join(node).join("device");
            fs::create_dir_all(&device).unwrap();
            fs::write(
                device.join("uevent"),
                format!("HID_ID=0003:0000046D:{product}\n"),
            )
            .unwrap();
            fs::write(device.join("report_descriptor"), [0x85, 0x14]).unwrap();
        }
        // A node without a HID uevent is skipped, not an error.
        fs::create_dir_all(class.join("hidraw3/device")).unwrap();
        fs::write(class.join("hidraw3/device/uevent"), "DRIVER=x\n").unwrap();
        let nodes = enumerate(&class, Path::new("/devices")).unwrap();
        let names: Vec<_> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["hidraw9", "hidraw10"]);
        assert_eq!(nodes[1].path, Path::new("/devices/hidraw10"));
        assert_eq!(nodes[1].hid_name, "");
    }

    #[test]
    fn a_missing_class_directory_is_no_device_and_an_unreadable_descriptor_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            enumerate(&root.path().join("absent"), Path::new(DEV_DIR)),
            Ok(vec![])
        );
        let device = root.path().join("hidraw0/device");
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("uevent"), "HID_ID=0003:0000046D:0000C354\n").unwrap();
        assert!(matches!(
            enumerate(root.path(), Path::new(DEV_DIR)),
            Err(SysfsError::Io { .. })
        ));
        fs::write(device.join("report_descriptor"), [0x26, 0xff]).unwrap();
        assert_eq!(
            enumerate(root.path(), Path::new(DEV_DIR)),
            Err(SysfsError::TruncatedDescriptor {
                node: "hidraw0".into(),
                offset: 0
            })
        );
    }
}
