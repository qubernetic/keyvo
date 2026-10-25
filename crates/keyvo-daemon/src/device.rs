//! The device actor (spec §5.1, §5.5): one thread owns the Keypad's
//! transport. It starts the keep-alive, claims the display, diverts the
//! page buttons, then paints the [`Frame`] it is sent and reports input.
//!
//! The pad gets one composite image per change (spec §5.5), as Options+
//! paints it: the actor keeps a [`Canvas`] of the whole display, the panel
//! background with every key drawn on it. After a claim, on a new view
//! (profile, page or mode) and on a new panel color the canvas is drawn
//! anew and the device gets one image of the union of the key rectangles;
//! otherwise only keys whose spec differs from what the pad shows are drawn
//! and the device gets the bounding box of those keys, cropped from the
//! same canvas. A key and the background beside it therefore always come
//! from one image: nothing of an earlier panel paint (`keyvo fill
//! --panel`, the logo) stays beside a key, and a page does not appear key
//! by key.
//!
//! Areas wait in a [`Queue`] of at most two and are encoded when they are
//! sent, from the canvas as it is then; the next image goes after the
//! device's response to the one before, and a crop above the device's
//! `maxImageSize` goes as horizontal bands.
//!
//! When the device is gone, or the claim fails, the actor waits
//! `[timing].reconnect_interval` and opens it again: a new connection
//! claims, diverts and repaints the union. A claim lost on a live
//! connection (keep-alive expiry, spec §5.3) is taken back with
//! `setMode(a0)` and the union is repainted.
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
use keyvo_core::render::{Canvas, KeyPlace, KeySpec, Panel, RenderError, Renderer};
use keyvo_core::state::View;
use keyvo_hid::display::{self, ClaimError, ClaimWatch};
use keyvo_hid::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, Plane, SoftwareId};
use keyvo_hid::image::{ImageConfig, ImageError, Queue, Rect};
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

/// What the pad shows: the view, the panel background and the keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    /// The profile, page and mode the keys belong to; a new view repaints
    /// the union of the keys.
    pub view: Option<View>,
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
        /// The panel and the key rectangles of the geometry.
        panel: Panel,
    },
    /// The device is gone or could not be claimed. Reported once per
    /// change of the error, not for every attempt.
    Disconnected {
        /// Why.
        error: String,
    },
    /// An image carrying the change of these keys (1-based) was written.
    Painted {
        /// The keys, in geometry order.
        keys: Vec<u8>,
    },
    /// A key or page-button press or release.
    Input(InputEvent),
    /// The claim was lost (keep-alive expiry, spec §5.3) and is taken
    /// back; the union of the keys is painted again.
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
    let image = image_config(&mut hidpp, settings.image)?;
    let diverted = reprog::divert_page_buttons(&mut hidpp, &settings.divert, std::thread::sleep)?;
    let panel = claim.geometry.panel();
    let count = panel.keys.len();
    let keys = u8::try_from(count).map_err(|_| DeviceError::TooManyKeys(count))?;
    let mut input = Input::new(hidpp.device_index(), claim.display_index, keys)
        .with_page_buttons(diverted.reprog_index);
    let watch = ClaimWatch::new(hidpp.device_index(), claim.display_index)
        .with_keep_alive(hidpp.feature_index(Plane::Classic, feature::KEEP_ALIVE)?);
    let mut shown = Shown::new(count);
    let mut queue = Queue::new();
    emit(Event::Connected {
        keys,
        panel: panel.clone(),
    });
    loop {
        if !drain(wanted, commands) {
            return Ok(());
        }
        if let Some(frame) = wanted.as_ref() {
            shown.update(renderer, &panel, frame, &mut queue);
        }
        if let Some(canvas) = &shown.canvas {
            let encode = |rect: Rect| -> Result<Vec<u8>, PaintError> {
                Ok(canvas.crop(place(rect))?.jpeg()?)
            };
            match queue.send_next(&mut hidpp, &image, encode) {
                Ok(Some(sent)) if !sent.keys.is_empty() => {
                    emit(Event::Painted { keys: sent.keys });
                }
                Ok(_) => {}
                Err(PaintError::Image(ImageError::Hidpp(error))) => return Err(error.into()),
                Err(error) => tracing::warn!("panel: not painted: {error}"),
            }
        }
        keep_alive.poll(&mut hidpp)?;
        // With areas waiting, only look at what the device sent.
        let wait = if queue.is_empty() {
            keep_alive.until_due().min(settings.poll)
        } else {
            Duration::ZERO
        };
        if let Some(frame) = hidpp.next_frame(wait)? {
            if watch.reclaim_on(&mut hidpp, &frame)? {
                shown = Shown::new(count);
                queue.clear();
                emit(Event::Reclaimed);
                continue;
            }
            for event in input.feed(&frame) {
                emit(Event::Input(event));
            }
        }
    }
}

/// The image settings with the display's `maxImageSize` (fn0). A display
/// that refuses fn0 is painted within the fragment index limit alone; only
/// a failing transport ends the connection.
fn image_config<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    config: ImageConfig,
) -> Result<ImageConfig, DeviceError> {
    match display::capabilities(hidpp) {
        Ok(capabilities) => Ok(ImageConfig {
            max_image_size: usize::try_from(capabilities.max_image_size).ok(),
            ..config
        }),
        Err(HidppError::Transport(error)) => Err(HidppError::Transport(error).into()),
        Err(error) => {
            tracing::warn!("display capabilities: {error}; no maxImageSize limit");
            Ok(config)
        }
    }
}

/// What the pad shows: the canvas the images are cropped from, the view
/// and the key specs drawn on it. `None` until drawn, so a new connection
/// and a reclaim paint the union.
struct Shown {
    canvas: Option<Canvas>,
    view: Option<View>,
    keys: Vec<Option<KeySpec>>,
}

impl Shown {
    fn new(count: usize) -> Self {
        Self {
            canvas: None,
            view: None,
            keys: vec![None; count],
        }
    }

    /// Draws what `frame` changes and queues the area to send: the union of
    /// the keys when the canvas is new (claim, view, panel color), else the
    /// bounding box of the changed keys.
    fn update(&mut self, renderer: &Renderer, panel: &Panel, frame: &Frame, queue: &mut Queue) {
        let background = renderer.panel_color(frame.accent);
        let new = self.view != frame.view
            || self
                .canvas
                .as_ref()
                .is_none_or(|canvas| canvas.background() != background);
        if new {
            match renderer.canvas(frame.accent, panel) {
                Ok(canvas) => self.canvas = Some(canvas),
                Err(error) => {
                    tracing::warn!("panel: not rendered: {error}");
                    return;
                }
            }
            self.view.clone_from(&frame.view);
            self.keys.fill(None);
        }
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let mut changed = Vec::new();
        for (index, shown) in self.keys.iter_mut().enumerate() {
            let Some(spec) = frame.keys.get(index) else {
                continue;
            };
            if shown.as_ref() == Some(spec) {
                continue;
            }
            match renderer.draw(canvas, index, spec) {
                Ok(()) => changed.push(index),
                Err(error) => warn_render(index + 1, &error),
            }
            // A key that cannot be drawn is not tried again until its spec
            // changes.
            *shown = Some(spec.clone());
        }
        let area = if new {
            panel.bounds(0..panel.keys.len())
        } else {
            panel.bounds(changed.iter().copied())
        };
        if let Some(area) = area {
            let keys: Vec<u8> = changed
                .iter()
                .filter_map(|&index| u8::try_from(index + 1).ok())
                .collect();
            queue.push(rect(area), &keys);
        }
    }
}

/// Why an area could not be painted.
#[derive(Debug, thiserror::Error)]
enum PaintError {
    /// The crop or its encoding failed.
    #[error(transparent)]
    Render(#[from] RenderError),
    /// The image write failed.
    #[error(transparent)]
    Image(#[from] ImageError),
}

/// The image rectangle of a canvas area.
const fn rect(area: KeyPlace) -> Rect {
    Rect {
        x: area.x,
        y: area.y,
        width: area.size.width,
        height: area.size.height,
    }
}

/// The canvas area of an image rectangle.
const fn place(rect: Rect) -> KeyPlace {
    KeyPlace {
        x: rect.x,
        y: rect.y,
        size: keyvo_core::render::TileSize {
            width: rect.width,
            height: rect.height,
        },
    }
}

fn warn_render(key: usize, error: &RenderError) {
    tracing::warn!("key {key}: not rendered: {error}");
}
