//! The Linux bindings of the daemon: the Keypad on hidraw (spec §5.1) and
//! chord injection on uinput through `evdev` (spec §4.1).

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType};
use keyvo_core::chord::{KeyCode, KeyEvent};
use keyvo_core::focus::WindowWatcher;
use keyvo_core::hid::TransportError;
use keyvo_core::inject::{InjectError, InputInjector};
use keyvo_core::lineage::Lineage;
use keyvo_core::session::SessionWatcher;
use keyvo_hid::dialpad::Link;
use keyvo_hid::hidraw::HidrawTransport;
use keyvo_hid::sysfs;
use tokio::sync::{broadcast, mpsc};

use crate::daemon::{self, Devices, Event, Sources};
use crate::device::Connector;
use crate::dialpad::DialpadConnector;
use crate::socket::{Limits, Server};

/// Name of the virtual keyboard keyvo creates.
const UINPUT_NAME: &str = "keyvo";
/// Daemon events the log keeps up with before it drops the oldest.
const EVENT_BACKLOG: usize = 256;

/// Runs the daemon on this machine until `shutdown`: the Keypad at
/// `devices.keypad` or found under sysfs, the Dialpad at `devices.dialpad`
/// or on the Bolt receiver found under sysfs, chords on uinput, the focused window and
/// the session lock and the process lineage from `sources` (the binary passes
/// [`crate::kwin::KWinWatcher`], [`crate::logind::LogindSessionWatcher`] and
/// [`crate::lineage::ProcLineage`]),
/// the configuration in `config_dir`, the control socket at `socket`. Every
/// trace record is logged.
///
/// # Errors
///
/// When the socket cannot be bound (another daemon runs, or its directory
/// is not private), and the errors of [`daemon::run`].
pub async fn serve(
    config_dir: &Path,
    devices: Devices<Option<PathBuf>, Option<PathBuf>>,
    socket: &Path,
    sources: Sources<impl WindowWatcher, impl SessionWatcher, impl Lineage>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()> {
    let server = Server::bind(socket)?;
    tracing::info!("control socket at {}", server.path().display());
    // Read once at start; a reload does not change the socket's limits.
    let limits = Limits::from_config(&daemon::config(config_dir));
    let (requests, inbox) = mpsc::unbounded_channel();
    let server = tokio::spawn(async move { server.serve(requests, limits).await });
    let (events, mut log) = broadcast::channel(EVENT_BACKLOG);
    tokio::spawn(async move {
        loop {
            match log.recv().await {
                Ok(Event::Trace(record)) => tracing::info!("press: {record:?}"),
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let paths = daemon::Paths {
        config_dir: config_dir.to_path_buf(),
        socket: socket.to_path_buf(),
    };
    let result = daemon::run(
        &paths,
        Devices {
            keypad: HidrawConnector::new(devices.keypad),
            dialpad: ReceiverConnector::new(devices.dialpad),
        },
        Injector::open(),
        sources,
        events,
        inbox,
        shutdown,
    )
    .await;
    // Dropping the server removes the socket file.
    server.abort();
    let _ = server.await;
    result
}

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

/// Opens the Dialpad's node: the given one, else the Bolt receiver's HID++
/// node searched under sysfs on every attempt. A given node sysfs knows as
/// a Bluetooth Dialpad is reached directly, any other as a receiver.
#[derive(Debug, Clone)]
pub struct ReceiverConnector {
    device: Option<PathBuf>,
    class_dir: PathBuf,
    dev_dir: PathBuf,
}

impl ReceiverConnector {
    /// Opens `device`, or searches the system's sysfs without one.
    #[must_use]
    pub fn new(device: Option<PathBuf>) -> Self {
        Self::with_dirs(
            device,
            Path::new(sysfs::SYSFS_HIDRAW),
            Path::new(sysfs::DEV_DIR),
        )
    }

    /// As [`ReceiverConnector::new`], searching the hidraw class directory
    /// `class_dir` with device files in `dev_dir`.
    #[must_use]
    pub fn with_dirs(device: Option<PathBuf>, class_dir: &Path, dev_dir: &Path) -> Self {
        Self {
            device,
            class_dir: class_dir.to_path_buf(),
            dev_dir: dev_dir.to_path_buf(),
        }
    }

    /// The node to open and how the Dialpad is reached on it.
    fn node(&self) -> Result<(PathBuf, Link), TransportError> {
        let io = |error: sysfs::SysfsError| TransportError::Io(error.to_string());
        if let Some(path) = &self.device {
            let bluetooth = sysfs::node_at(&self.class_dir, &self.dev_dir, path)
                .map_err(io)?
                .is_some_and(|node| node.is_bluetooth_dialpad());
            let link = if bluetooth {
                Link::Direct
            } else {
                Link::Receiver
            };
            return Ok((path.clone(), link));
        }
        let node = sysfs::find_receiver(&self.class_dir, &self.dev_dir)
            .map_err(io)?
            .ok_or_else(|| {
                TransportError::Io("no Bolt receiver node (046d:c548 with reports 10 11)".into())
            })?;
        Ok((node.path, Link::Receiver))
    }
}

impl DialpadConnector for ReceiverConnector {
    type Transport = HidrawTransport;

    fn connect(&mut self) -> Result<(HidrawTransport, Link), TransportError> {
        let (path, link) = self.node()?;
        let transport = HidrawTransport::open(&path).map_err(|error| {
            TransportError::Io(format!("cannot open {}: {error}", path.display()))
        })?;
        Ok((transport, link))
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
    use keyvo_core::focus::{FakeWindowWatcher, WindowContext};
    use keyvo_core::lineage::NoLineage;
    use keyvo_core::session::FakeSessionWatcher;

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
    fn the_dialpad_connector_finds_the_receiver_or_reaches_a_bluetooth_dialpad() {
        let root = tempfile::tempdir().unwrap();
        for (node, id) in [
            ("hidraw3", "0005:0000046D:0000BC00"),
            ("hidraw9", "0003:0000046D:0000C548"),
        ] {
            let device = root.path().join("class").join(node).join("device");
            std::fs::create_dir_all(&device).unwrap();
            std::fs::write(device.join("uevent"), format!("HID_ID={id}\n")).unwrap();
            std::fs::write(device.join("report_descriptor"), [0x85, 0x10, 0x85, 0x11]).unwrap();
        }
        let class = root.path().join("class");
        let dev = Path::new("/dev");
        let node = |device: Option<&str>| {
            ReceiverConnector::with_dirs(device.map(PathBuf::from), &class, dev).node()
        };
        assert_eq!(
            node(None),
            Ok((PathBuf::from("/dev/hidraw9"), Link::Receiver))
        );
        assert_eq!(
            node(Some("/dev/hidraw3")),
            Ok((PathBuf::from("/dev/hidraw3"), Link::Direct))
        );
        assert_eq!(
            node(Some("/dev/hidraw7")),
            Ok((PathBuf::from("/dev/hidraw7"), Link::Receiver))
        );
        let error = ReceiverConnector::with_dirs(None, Path::new("/absent"), dev)
            .connect()
            .unwrap_err();
        assert_eq!(
            error,
            TransportError::Io("no Bolt receiver node (046d:c548 with reports 10 11)".into())
        );
    }

    #[tokio::test]
    async fn serve_runs_until_shutdown_without_a_keypad() {
        let dir = tempfile::tempdir().unwrap();
        let absent = dir.path().join("hidraw-absent");
        let socket = dir.path().join("run/keyvo.sock");
        let watcher = FakeWindowWatcher::new();
        let client = {
            let socket = socket.clone();
            async move {
                // The socket answers while the daemon runs.
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                let mut stream = std::os::unix::net::UnixStream::connect(&socket).unwrap();
                std::io::Write::write_all(&mut stream, b"{\"v\":1,\"cmd\":\"status\"}\n").unwrap();
                let mut line = String::new();
                tokio::task::spawn_blocking(move || {
                    std::io::BufRead::read_line(&mut std::io::BufReader::new(stream), &mut line)
                        .unwrap();
                    line
                })
                .await
                .unwrap()
            }
        };
        let (line, served) = tokio::join!(
            client,
            serve(
                dir.path(),
                Devices {
                    keypad: Some(absent.clone()),
                    dialpad: Some(absent),
                },
                &socket,
                Sources {
                    window: watcher.clone(),
                    session: FakeSessionWatcher::new(),
                    lineage: NoLineage,
                },
                tokio::time::sleep(std::time::Duration::from_millis(200)),
            )
        );
        served.unwrap();
        assert!(line.contains("\"ok\":true"), "{line}");
        assert!(line.contains("\"connected\":false"), "{line}");
        assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~")));
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn serve_refuses_a_socket_another_daemon_serves() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("run/keyvo.sock");
        let _first = Server::bind(&socket).unwrap();
        let error = serve(
            dir.path(),
            Devices {
                keypad: None,
                dialpad: None,
            },
            &socket,
            Sources {
                window: FakeWindowWatcher::new(),
                session: FakeSessionWatcher::failing("no logind"),
                lineage: NoLineage,
            },
            std::future::ready(()),
        )
        .await
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
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
