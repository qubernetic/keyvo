//! The Linux bindings of the daemon: the Keypad on hidraw (spec §5.1) and
//! chord injection on uinput through `evdev` (spec §4.1).

use std::io;
use std::path::{Path, PathBuf};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType};
use keyvo_core::chord::{KeyCode, KeyEvent};
use keyvo_core::hid::TransportError;
use keyvo_core::inject::{InjectError, InputInjector};
use keyvo_hid::hidraw::HidrawTransport;
use keyvo_hid::sysfs;

use crate::device::Connector;

/// Name of the virtual keyboard keyvo creates.
const UINPUT_NAME: &str = "keyvo";

/// Opens the given hidraw node, or searches the Keypad's node under sysfs
/// on every attempt, so a node number that changes after a replug is found.
#[derive(Debug, Clone)]
pub struct HidrawConnector {
    device: Option<PathBuf>,
    class_dir: PathBuf,
    dev_dir: PathBuf,
}

impl HidrawConnector {
    /// Opens `device`, or searches the system's sysfs without one.
    #[must_use]
    pub fn new(device: Option<PathBuf>) -> Self {
        Self::with_dirs(
            device,
            Path::new(sysfs::SYSFS_HIDRAW),
            Path::new(sysfs::DEV_DIR),
        )
    }

    /// As [`HidrawConnector::new`], searching the hidraw class directory
    /// `class_dir` with device files in `dev_dir`.
    #[must_use]
    pub fn with_dirs(device: Option<PathBuf>, class_dir: &Path, dev_dir: &Path) -> Self {
        Self {
            device,
            class_dir: class_dir.to_path_buf(),
            dev_dir: dev_dir.to_path_buf(),
        }
    }
}

impl Connector for HidrawConnector {
    type Transport = HidrawTransport;

    fn connect(&mut self) -> Result<HidrawTransport, TransportError> {
        let path = match &self.device {
            Some(path) => path.clone(),
            None => {
                sysfs::find_keypad(&self.class_dir, &self.dev_dir)
                    .map_err(|error| TransportError::Io(error.to_string()))?
                    .path
            }
        };
        HidrawTransport::open(&path)
    }
}

/// The chord injector: the uinput keyboard, or the reason there is none.
/// Without uinput the daemon still paints and runs `exec`; chords fail and
/// say why.
pub enum Injector {
    /// A virtual keyboard that declares every key a chord can press.
    Uinput(VirtualDevice),
    /// `/dev/uinput` could not be used.
    Unavailable(String),
}

impl Injector {
    /// Creates the virtual keyboard.
    #[must_use]
    pub fn open() -> Self {
        match uinput_keyboard() {
            Ok(device) => Self::Uinput(device),
            Err(error) => {
                let reason = format!("cannot create the uinput keyboard: {error}");
                tracing::warn!("{reason}; chord keys will fail");
                Self::Unavailable(reason)
            }
        }
    }
}

fn uinput_keyboard() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<evdev::KeyCode>::new();
    for code in KeyCode::all() {
        keys.insert(evdev::KeyCode(code.get()));
    }
    VirtualDevice::builder()?
        .name(UINPUT_NAME)
        .with_keys(&keys)?
        .build()
}

impl InputInjector for Injector {
    fn send(&mut self, events: &[KeyEvent]) -> Result<(), InjectError> {
        match self {
            Self::Uinput(device) => {
                for event in events {
                    // Each edge is its own report; `emit` adds the SYN.
                    let value = i32::from(event.down);
                    device
                        .emit(&[evdev::InputEvent::new(
                            EventType::KEY.0,
                            event.code.get(),
                            value,
                        )])
                        .map_err(|error| InjectError(error.to_string()))?;
                }
                Ok(())
            }
            Self::Unavailable(reason) => Err(InjectError(reason.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use keyvo_core::chord::Chord;

    use super::*;

    #[test]
    fn the_connector_finds_the_keypad_node_or_opens_the_given_one() {
        let root = tempfile::tempdir().unwrap();
        let device = root.path().join("class/hidraw2/device");
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(
            device.join("uevent"),
            "HID_ID=0003:0000046D:0000C354\nHID_NAME=Logitech MX Creative Keypad\n",
        )
        .unwrap();
        std::fs::write(device.join("report_descriptor"), [0x85, 0x11, 0x85, 0x14]).unwrap();
        let dev = root.path().join("dev");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("hidraw2"), []).unwrap();

        let class = root.path().join("class");
        assert!(
            HidrawConnector::with_dirs(None, &class, &dev)
                .connect()
                .is_ok()
        );
        let given = Some(dev.join("hidraw2"));
        assert!(
            HidrawConnector::with_dirs(given, Path::new("/absent"), &dev)
                .connect()
                .is_ok()
        );
        let error = HidrawConnector::with_dirs(None, Path::new("/absent"), &dev)
            .connect()
            .unwrap_err();
        assert_eq!(
            error,
            TransportError::Io("no hidraw node for 046d:c354".into())
        );
    }

    #[test]
    fn without_uinput_a_chord_fails_with_the_reason() {
        let mut injector = Injector::Unavailable("no /dev/uinput".into());
        let events = Chord::parse("ctrl+c").unwrap().events();
        assert_eq!(
            injector.send(&events),
            Err(InjectError("no /dev/uinput".into()))
        );
    }
}
