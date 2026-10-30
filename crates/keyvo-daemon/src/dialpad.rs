//! The Dialpad actor (spec §5.8): one thread owns the Dialpad's transport,
//! the Bolt receiver's HID++ node or a Bluetooth Dialpad's own node. It
//! finds the Dialpad, diverts its buttons and rollers, and reports their
//! turns and presses; the controller resolves them on the profiles.
//!
//! The Keypad's actor runs beside it on its own thread and transport, so
//! neither device waits for the other: either may be absent. Without a
//! receiver, or when the transport fails, the actor reports why once and
//! opens it again after `[timing].reconnect_interval`. A receiver whose
//! Dialpad sleeps is reported once too; the actor then waits for the
//! receiver's arrival notification, or asks every slot again after
//! `[timing].reconnect_interval`. On a clean stop the diverts are given
//! back.
//!
//! It reads at most `[timing].render_coalesce` at a time, so a stop waits
//! no longer than that.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use keyvo_core::config::Timing;
use keyvo_core::hid::{HidTransport, TransportError};
use keyvo_hid::dialpad::{self, Dialpad, DialpadError, DialpadEvent, DialpadTiming, Link};
use keyvo_hid::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, SoftwareId};
use keyvo_hid::input::{Control, Edge, InputEvent};
use keyvo_hid::multiroller::Roller;
use keyvo_hid::reprog::DivertTiming;

/// Software id of the actor's requests; the slot search takes the three
/// after it.
const REQUEST_SWID: u8 = 0xe;

/// Opens the transport the Dialpad is reached on, once per connection
/// attempt, and says how.
pub trait DialpadConnector: Send + 'static {
    /// The transport of one connection.
    type Transport: HidTransport + Send + 'static;

    /// Opens the receiver's node or the Dialpad's own.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when there is none now; the actor tries again
    /// after `[timing].reconnect_interval`.
    fn connect(&mut self) -> Result<(Self::Transport, Link), TransportError>;
}

/// A connector that never finds a Dialpad, for a daemon without one.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoDialpad;

/// The transport of [`NoDialpad`], never opened.
#[derive(Debug)]
pub enum NoTransport {}

impl HidTransport for NoTransport {
    fn write(&mut self, _: &[u8]) -> Result<(), TransportError> {
        match *self {}
    }

    fn read(&mut self, _: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        match *self {}
    }
}

impl DialpadConnector for NoDialpad {
    type Transport = NoTransport;

    fn connect(&mut self) -> Result<(NoTransport, Link), TransportError> {
        Err(TransportError::Io("no Dialpad".into()))
    }
}

/// The `[timing]` values the actor uses, read once at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// `[timing].slot_probe`, `divert_settle` and `divert_spacing`.
    pub timing: DialpadTiming,
    /// `[timing].reply_timeout`.
    pub reply_timeout: Duration,
    /// `[timing].reconnect_interval`.
    pub reconnect_interval: Duration,
    /// `[timing].render_coalesce`: the longest read between two looks at
    /// the stop.
    pub poll: Duration,
}

impl Settings {
    /// The settings for `timing`.
    #[must_use]
    pub const fn from_timing(timing: &Timing) -> Self {
        Self {
            timing: DialpadTiming {
                slot_probe: timing.slot_probe,
                divert: DivertTiming {
                    settle: timing.divert_settle,
                    spacing: timing.divert_spacing,
                },
            },
            reply_timeout: timing.reply_timeout,
            reconnect_interval: timing.reconnect_interval,
            poll: timing.render_coalesce,
        }
    }
}

/// What the actor reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The Dialpad answers and its controls are diverted.
    Connected {
        /// Its receiver slot; `None` over Bluetooth.
        slot: Option<u8>,
        /// Its rollers.
        rollers: Vec<Roller>,
    },
    /// There is no Dialpad, or the connection failed. Reported once per
    /// change of the error, not for every attempt.
    Disconnected {
        /// Why.
        error: String,
    },
    /// A roller turned.
    Rotate {
        /// The device's roller id: 0 the roller, 1 the dial.
        roller: u8,
        /// Signed detents, clockwise positive.
        delta: i8,
    },
    /// A button went down or up.
    Button {
        /// The button, 1 to 4 in CID order.
        button: u8,
        /// Which way.
        edge: Edge,
    },
}

/// Runs the actor until `stop`'s sender is dropped, reporting through
/// `emit`.
pub fn run<C: DialpadConnector>(
    mut connector: C,
    settings: &Settings,
    stop: &Receiver<()>,
    mut emit: impl FnMut(Event),
) {
    let mut reported = None;
    loop {
        let error = match connector.connect() {
            Ok((transport, link)) => {
                match serve(transport, link, settings, stop, &mut emit, &mut reported) {
                    Ok(()) => return,
                    Err(error) => error.to_string(),
                }
            }
            // The connector's own words: no receiver node found.
            Err(TransportError::Io(message)) => message,
            Err(error) => format!("cannot open the Dialpad's node: {error}"),
        };
        report(&mut reported, error, &mut emit);
        if !wait(settings.reconnect_interval, stop) {
            return;
        }
    }
}

/// Reports `error` unless it is the one reported last.
fn report(reported: &mut Option<String>, error: String, emit: &mut impl FnMut(Event)) {
    if reported.as_ref() != Some(&error) {
        tracing::warn!("dialpad: {error}");
        emit(Event::Disconnected {
            error: error.clone(),
        });
        *reported = Some(error);
    }
}

/// Whether the daemon has let the actor go.
fn stopped(stop: &Receiver<()>) -> bool {
    !matches!(stop.try_recv(), Err(TryRecvError::Empty))
}

/// Waits `interval`; `false` when told to stop.
fn wait(interval: Duration, stop: &Receiver<()>) -> bool {
    match stop.recv_timeout(interval) {
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => true,
        Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => false,
    }
}

/// One connection: find, divert, then report until stopped (`Ok`) or until
/// the transport fails (`Err`).
fn serve<T: HidTransport>(
    transport: T,
    link: Link,
    settings: &Settings,
    stop: &Receiver<()>,
    emit: &mut impl FnMut(Event),
    reported: &mut Option<String>,
) -> Result<(), DialpadError> {
    let swid = SoftwareId::new(REQUEST_SWID)?;
    let mut hidpp = Hidpp::new(transport, DEVICE_DIRECT, swid, settings.reply_timeout);
    let mut target = link;
    let mut dialpad = loop {
        if stopped(stop) {
            return Ok(());
        }
        match Dialpad::open(&mut hidpp, target, &settings.timing, std::thread::sleep) {
            Ok(dialpad) => break dialpad,
            Err(DialpadError::NotFound(slots)) if link != Link::Direct => {
                report(
                    reported,
                    format!("no Dialpad answers ({slots}); waiting for it"),
                    emit,
                );
                target =
                    arrival(&mut hidpp, &slots.silent(), settings, stop)?.map_or(link, Link::Slot);
            }
            Err(error) => return Err(error),
        }
    };
    *reported = None;
    let slot = (dialpad.link() != Link::Direct).then(|| dialpad.device_index());
    tracing::info!(
        "dialpad: connected{}, {} buttons",
        slot.map_or_else(String::new, |slot| format!(" on slot {slot}")),
        dialpad.buttons().len()
    );
    emit(Event::Connected {
        slot,
        rollers: dialpad.rollers().to_vec(),
    });
    loop {
        if stopped(stop) {
            if let Err(error) = dialpad.restore(&mut hidpp, std::thread::sleep) {
                tracing::warn!("dialpad: diverts not given back: {error}");
            }
            return Ok(());
        }
        match dialpad.next_event(&mut hidpp, settings.poll, std::thread::sleep)? {
            Some(DialpadEvent::Input(InputEvent::Rotate { roller, delta })) => {
                emit(Event::Rotate { roller, delta });
            }
            Some(DialpadEvent::Input(InputEvent::Button {
                control: Control::Page(cid),
                edge,
            })) => {
                if let Some(button) = dialpad.button(cid).and_then(|n| u8::try_from(n).ok()) {
                    emit(Event::Button { button, edge });
                } else {
                    tracing::debug!("dialpad: control 0x{cid:04x} ignored");
                }
            }
            Some(DialpadEvent::Input(InputEvent::Button { .. })) | None => {}
            Some(DialpadEvent::LinkLost) => {
                tracing::info!("dialpad: link lost (asleep or out of range)");
            }
            Some(DialpadEvent::Rediverted) => tracing::info!("dialpad: back, diverts sent again"),
        }
    }
}

/// Waits at most `[timing].reconnect_interval` for the receiver to announce
/// the Dialpad or a device on a `silent` slot, looking at the stop between
/// reads; the slot, or `None` to search every slot again.
fn arrival<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    silent: &[u8],
    settings: &Settings,
    stop: &Receiver<()>,
) -> Result<Option<u8>, HidppError> {
    let until = Instant::now() + settings.reconnect_interval;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || stopped(stop) {
            return Ok(None);
        }
        if let Some(slot) = dialpad::wait_for_arrival(hidpp, silent, left.min(settings.poll))? {
            return Ok(Some(slot));
        }
    }
}
