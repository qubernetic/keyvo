//! The device actor (spec §5.1, §5.5): one thread owns the Keypad's
//! transport. It starts the keep-alive, claims the display, diverts the
//! page buttons, then paints the [`Frame`] it is sent and reports input.
//! Only keys whose spec differs from what the pad shows are rendered and
//! written, so a paint request that changes one key writes one key.
//!
//! The panel background goes first: after a claim, and whenever the
//! active profile's panel color changes, the actor writes the whole panel
//! in that color and then every key on it, so nothing of an earlier panel
//! paint (`keyvo fill --panel`, the logo) stays beside a key.
//!
//! When the device is gone, or the claim fails, the actor waits
//! `[timing].reconnect_interval` and opens it again: a new connection
//! claims, diverts and repaints the panel and every key. A claim lost on a
//! live connection (keep-alive expiry, spec §5.3) is taken back with
//! `setMode(a0)` and the panel and every key are repainted.
//!
//! HID I/O is synchronous (`HidTransport::read` blocks up to its timeout),
//! so the actor is a plain thread and talks to the tokio controller through
//! channels. It reads at most `[timing].render_coalesce` at a time, so a
//! paint request waits no longer than that.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use keyvo_core::clock::SystemClock;
use keyvo_core::config::Timing;
use keyvo_core::hid::{HidTransport, TransportError};
use keyvo_core::profile::Color;
use keyvo_core::render::{KeySpec, RenderError, Renderer, TileSize};
use keyvo_hid::display::{self, ClaimError, ClaimWatch, Geometry};
use keyvo_hid::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, Plane, SoftwareId};
use keyvo_hid::image::{self, ImageConfig, ImageError, Rect, Target};
use keyvo_hid::input::{Input, InputEvent};
use keyvo_hid::keep_alive::{KeepAlive, KeepAliveTiming};
use keyvo_hid::reprog::{self, DivertError, DivertTiming};
use keyvo_hid::root::feature;

/// Software id of the actor's requests, as `keyvo probe` uses it; image
/// writes carry [`ImageConfig::swid`].
const REQUEST_SWID: u8 = 0xe;

/// Opens the Keypad's transport, once per connection attempt.
pub trait Connector: Send + 'static {
    /// The transport of one connection.
    type Transport: HidTransport + Send + 'static;

    /// Opens the device.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when no device can be opened now; the actor tries
    /// again after `[timing].reconnect_interval`.
    fn connect(&mut self) -> Result<Self::Transport, TransportError>;
}

/// The `[timing]` values the actor uses, read once at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Keep-alive period and timeout.
    pub keep_alive: KeepAliveTiming,
    /// Page-button divert settle and spacing.
    pub divert: DivertTiming,
    /// Image write settings; `ack_timeout` from `[timing]`.
    pub image: ImageConfig,
    /// `[timing].reply_timeout`.
    pub reply_timeout: Duration,
    /// `[timing].reconnect_interval`.
    pub reconnect_interval: Duration,
    /// `[timing].render_coalesce`: the longest read between two looks at
    /// the paint requests.
    pub poll: Duration,
}

impl Settings {
    /// The settings for `timing`, with the image writer's default software
    /// id and index base.
    #[must_use]
    pub fn from_timing(timing: &Timing) -> Self {
        Self {
            keep_alive: KeepAliveTiming {
                period: timing.keep_alive_period,
                timeout: timing.keep_alive_timeout,
            },
            divert: DivertTiming {
                settle: timing.divert_settle,
                spacing: timing.divert_spacing,
            },
            image: ImageConfig {
                ack_timeout: timing.ack_timeout,
                ..ImageConfig::default()
            },
            reply_timeout: timing.reply_timeout,
            reconnect_interval: timing.reconnect_interval,
            poll: timing.render_coalesce,
        }
    }
}

/// What the pad shows: the panel background and the keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    /// The active profile's accent; the panel background is drawn from it
    /// ([`Renderer::panel_color`]).
    pub accent: Option<Color>,
    /// The specs of keys 1 to n.
    pub keys: Vec<KeySpec>,
}

/// A request to the actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// What the pad should show; it replaces any earlier request.
    Paint(Frame),
    /// End the actor.
    Stop,
}

/// What the actor reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A Keypad is claimed; it has `keys` keys.
    Connected {
        /// The key count of the geometry.
        keys: u8,
    },
    /// The device is gone or could not be claimed. Reported once per
    /// change of the error, not for every attempt.
    Disconnected {
        /// Why.
        error: String,
    },
    /// These keys (1-based) were written.
    Painted {
        /// The keys, in geometry order.
        keys: Vec<u8>,
    },
    /// A key or page-button press or release.
    Input(InputEvent),
    /// The claim was lost (keep-alive expiry, spec §5.3) and is taken
    /// back; every key is painted again.
    Reclaimed,
}

/// Why a connection ended.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    /// The device could not be opened.
    #[error("cannot open the keypad: {0}")]
    Open(TransportError),
    /// A request, the keep-alive or the transport failed.
    #[error(transparent)]
    Hidpp(#[from] HidppError),
    /// The display claim failed.
    #[error(transparent)]
    Claim(#[from] ClaimError),
    /// The page-button divert failed.
    #[error(transparent)]
    Divert(#[from] DivertError),
    /// More keys than a key number holds.
    #[error("the geometry lists {0} keys")]
    TooManyKeys(usize),
}

/// Runs the actor until [`Command::Stop`] or until the command channel
/// closes, reporting through `emit`.
pub fn run<C: Connector>(
    mut connector: C,
    settings: &Settings,
    commands: &Receiver<Command>,
    mut emit: impl FnMut(Event),
) {
    let renderer = Renderer::new();
    let mut wanted = None;
    let mut reported = None;
    loop {
        let error = match connector.connect() {
            Ok(transport) => {
                reported = None;
                match serve(
                    transport,
                    settings,
                    &renderer,
                    &mut wanted,
                    commands,
                    &mut emit,
                ) {
                    Ok(()) => return,
                    Err(error) => error,
                }
            }
            Err(error) => DeviceError::Open(error),
        };
        let message = error.to_string();
        if reported.as_ref() != Some(&message) {
            tracing::warn!("keypad: {message}");
            emit(Event::Disconnected {
                error: message.clone(),
            });
            reported = Some(message);
        }
        if !wait(settings.reconnect_interval, &mut wanted, commands) {
            return;
        }
    }
}

/// Waits `interval`, taking paint requests; `false` when told to stop.
fn wait(interval: Duration, wanted: &mut Option<Frame>, commands: &Receiver<Command>) -> bool {
    let until = Instant::now() + interval;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        match commands.recv_timeout(left) {
            Ok(Command::Paint(frame)) => *wanted = Some(frame),
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return false,
            Err(RecvTimeoutError::Timeout) => return true,
        }
    }
}

/// Takes every queued request; `false` when told to stop.
fn drain(wanted: &mut Option<Frame>, commands: &Receiver<Command>) -> bool {
    loop {
        match commands.try_recv() {
            Ok(Command::Paint(frame)) => *wanted = Some(frame),
            Ok(Command::Stop) | Err(TryRecvError::Disconnected) => return false,
            Err(TryRecvError::Empty) => return true,
        }
    }
}

/// One connection: claim, then paint and read until stopped (`Ok`) or
/// until the device fails (`Err`).
fn serve<T: HidTransport>(
    transport: T,
    settings: &Settings,
    renderer: &Renderer,
    wanted: &mut Option<Frame>,
    commands: &Receiver<Command>,
    emit: &mut impl FnMut(Event),
) -> Result<(), DeviceError> {
    let swid = SoftwareId::new(REQUEST_SWID)?;
    let mut hidpp = Hidpp::new(transport, DEVICE_DIRECT, swid, settings.reply_timeout);
    let mut keep_alive = KeepAlive::new(SystemClock, settings.keep_alive);
    keep_alive.poll(&mut hidpp)?;
    let claim = display::claim(&mut hidpp)?;
    let diverted = reprog::divert_page_buttons(&mut hidpp, &settings.divert, std::thread::sleep)?;
    let geometry = claim.geometry;
    let count = geometry.keys.len();
    let keys = u8::try_from(count).map_err(|_| DeviceError::TooManyKeys(count))?;
    let mut input = Input::new(hidpp.device_index(), claim.display_index, keys)
        .with_page_buttons(diverted.reprog_index);
    let watch = ClaimWatch::new(hidpp.device_index(), claim.display_index)
        .with_keep_alive(hidpp.feature_index(Plane::Classic, feature::KEEP_ALIVE)?);
    // The panel color and the key specs the pad shows.
    let mut panel: Option<Color> = None;
    let mut painted: Vec<Option<KeySpec>> = vec![None; count];
    emit(Event::Connected { keys });
    loop {
        if !drain(wanted, commands) {
            return Ok(());
        }
        let mut written = Vec::new();
        if let Some(frame) = wanted.as_ref() {
            let under = renderer.panel_color(frame.accent);
            if panel != Some(under) {
                // The whole panel first, so nothing of an earlier panel
                // paint stays beside a key; then every key on it.
                paint_panel(&mut hidpp, settings, renderer, &geometry, frame.accent)?;
                panel = Some(under);
                painted.fill(None);
                keep_alive.poll(&mut hidpp)?;
            }
            for (key, shown) in geometry.keys.iter().zip(&mut painted) {
                let Some(spec) = frame.keys.get(usize::from(key.index).wrapping_sub(1)) else {
                    continue;
                };
                if shown.as_ref() == Some(spec) {
                    continue;
                }
                let rect = Rect {
                    x: key.x,
                    y: key.y,
                    width: key.width,
                    height: key.height,
                };
                let size = TileSize {
                    width: rect.width,
                    height: rect.height,
                };
                match renderer
                    .tile(spec, size)
                    .and_then(|tile| tile.jpeg_on(under))
                {
                    Ok(jpeg) => match image::write(&mut hidpp, &settings.image, rect, &jpeg) {
                        Ok(_) => written.push(key.index),
                        Err(ImageError::Hidpp(error)) => return Err(error.into()),
                        Err(error) => tracing::warn!("key {}: not painted: {error}", key.index),
                    },
                    Err(error) => warn_render(key.index, &error),
                }
                // A key that cannot be painted is not tried again until its
                // spec changes.
                *shown = Some(spec.clone());
                keep_alive.poll(&mut hidpp)?;
            }
        }
        if !written.is_empty() {
            emit(Event::Painted { keys: written });
        }
        keep_alive.poll(&mut hidpp)?;
        let wait = keep_alive.until_due().min(settings.poll);
        if let Some(frame) = hidpp.next_frame(wait)? {
            if watch.reclaim_on(&mut hidpp, &frame)? {
                panel = None;
                painted.fill(None);
                emit(Event::Reclaimed);
                continue;
            }
            for event in input.feed(&frame) {
                emit(Event::Input(event));
            }
        }
    }
}

/// Writes the panel background of a profile with `accent` to the bounding
/// box of the keys. A background that cannot be painted is logged; only a
/// failing device ends the connection.
fn paint_panel<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    settings: &Settings,
    renderer: &Renderer,
    geometry: &Geometry,
    accent: Option<Color>,
) -> Result<(), DeviceError> {
    let rect = match Target::Panel.rect(geometry) {
        Ok(rect) => rect,
        Err(error) => {
            tracing::warn!("panel: not painted: {error}");
            return Ok(());
        }
    };
    let size = TileSize {
        width: rect.width,
        height: rect.height,
    };
    match renderer.backdrop(accent, size).and_then(|tile| tile.jpeg()) {
        Ok(jpeg) => match image::write(hidpp, &settings.image, rect, &jpeg) {
            Ok(_) => {}
            Err(ImageError::Hidpp(error)) => return Err(error.into()),
            Err(error) => tracing::warn!("panel: not painted: {error}"),
        },
        Err(error) => tracing::warn!("panel: not rendered: {error}"),
    }
    Ok(())
}

fn warn_render(key: u8, error: &RenderError) {
    tracing::warn!("key {key}: not rendered: {error}");
}
