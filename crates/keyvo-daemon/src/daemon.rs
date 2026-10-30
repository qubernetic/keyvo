//! The controller: one tokio task that owns the [`Engine`] and ties the
//! device actor, the presses, the `exec` processes, the hot-reload and the
//! socket clients together.
//!
//! - State changes mark the pad dirty; the specs go to the actor
//!   `[timing].render_coalesce` after the first change, so a burst becomes
//!   one paint, and the actor writes only the keys that differ.
//! - A key press starts the long-press timer; the release or the timer
//!   makes it a short or long press, resolved on the active page. `chord`
//!   goes through the [`InputInjector`]; `exec` runs in its own task and its
//!   result comes back as the key's `success` or `error` look. An action
//!   that fails runs the key's `on_fail` once, traced as a second record
//!   of the press; it leaves the `error` look and is never retried.
//! - The [`WindowWatcher`] reports the focused window and the page buttons
//!   arrive as diverted controls; both go to the context state machine,
//!   which the controller wakes at its next deadline (focus hysteresis,
//!   page-button long press, page return; a key press restarts the page
//!   return). A changed profile, page or mode repaints and is reported as
//!   [`Event::Switched`]. Without a watcher the daemon runs on, without
//!   focus switching. A `launch` key that waits for its window opens a
//!   pending launch with its shell's pid; a failed launch drops it, and
//!   the [`Lineage`] tells the state machine which launch a focused
//!   window's process belongs to.
//! - The [`SessionWatcher`] reports the session lock. While locked the pad
//!   shows the lock page, every press (a socket `press` and its dry run
//!   included) is traced `blocked` and runs nothing, and the page buttons
//!   do nothing; the lock and unlock are reported as [`Event::Switched`]
//!   with `locked` set. Without a session source the daemon runs on,
//!   without lock detection.
//! - A change in the configuration directory reloads it after
//!   `[timing].reload_settle` of quiet. A directory that does not load
//!   keeps the last good state.
//! - Socket clients send [`Request`]s (`keyvo_core::socket`): `status`,
//!   `subscribe`, `press` (a dry run resolves and traces, executes nothing
//!   and leaves the key's look alone), `reload` and `notify_context` (a
//!   focus report like the watcher's). Every trace entry goes into an
//!   in-memory ring buffer of `[trace].ring` entries.
//!
//! The `[timing]` values of the device actor, `long_press`,
//! `focus_hysteresis` and `[focus].hold_classes` are read at start; the
//! others and `[trace].ring` apply from the next reload.

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::SystemTime;

use keyvo_core::chord::Chord;
use keyvo_core::config::Config;
use keyvo_core::dialpad::{ButtonSlot, Control as DialControl, Roller, Turn};
use keyvo_core::focus::{WindowContext, WindowWatcher};
use keyvo_core::inject::InputInjector;
use keyvo_core::layout::{self, Loaded, Profiles};
use keyvo_core::lineage::Lineage;
use keyvo_core::press::{PressDetector, PressKind};
use keyvo_core::profile::{Action, KeySlot};
use keyvo_core::recipe;
use keyvo_core::session::{Session, SessionWatcher};
use keyvo_core::socket::{
    Command as Wire, Context, ContextSource, DeviceStatus, DialpadStatus, Edge as KeyEdge,
    Geometry, Recipes, RollerStatus, Status,
};
use keyvo_core::state::{LaunchId, PageButton, View};
use keyvo_core::trace::{Outcome, TraceRecord, Trigger};
use keyvo_hid::input::{Control, Edge, InputEvent};
use keyvo_hid::reprog::{PAGE_LEFT, PAGE_RIGHT};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::Instant;

use crate::device::{self, Command, Connector, Frame};
use crate::dialpad::{self, DialpadConnector};
use crate::engine::Engine;
use crate::exec;

/// What the daemon reports, for logs, tests and the socket's `subscribe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A Keypad is claimed.
    Connected {
        /// Its key count.
        keys: u8,
        /// Its display as it publishes it.
        geometry: Geometry,
    },
    /// The Keypad is gone or cannot be claimed.
    Disconnected {
        /// Why.
        error: String,
    },
    /// These keys (1-based) were written to the pad.
    Painted {
        /// The keys.
        keys: Vec<u8>,
    },
    /// A key on the device went down or up.
    Key {
        /// The key.
        key: KeySlot,
        /// Which way.
        edge: KeyEdge,
    },
    /// The Dialpad connected or is gone.
    Dialpad(DialpadStatus),
    /// A Dialpad button went down or up.
    Button {
        /// The button.
        button: ButtonSlot,
        /// Which way.
        edge: KeyEdge,
    },
    /// The Dialpad's dial or roller turned.
    Dial {
        /// Which one.
        roller: Roller,
        /// Detents, clockwise positive.
        delta: i32,
        /// How often its action ran.
        actions: u32,
    },
    /// A focus source reported a window.
    Context(Context),
    /// A press was resolved and, for an action, executed.
    Trace(TraceRecord),
    /// The pad shows another profile, page or mode.
    Switched(View),
    /// The configuration directory was reloaded.
    Reloaded,
    /// The configuration directory did not load; the last good state stays.
    ReloadFailed {
        /// Every problem, one per line.
        error: String,
    },
}

/// A socket client's command, answered on `reply`.
#[derive(Debug)]
pub struct Request {
    /// The command.
    pub command: Wire,
    /// Where the answer goes.
    pub reply: oneshot::Sender<Answer>,
}

/// The controller's answer to a [`Request`].
#[derive(Debug)]
pub enum Answer {
    /// `status`.
    Status(Status),
    /// `subscribe`: what the pad shows now, the newest `replay` trace
    /// entries, oldest first, and every event from here on.
    Subscribed {
        /// What the pad shows now.
        view: View,
        /// The trace entries to send first.
        backlog: Vec<TraceRecord>,
        /// Every later event.
        events: broadcast::Receiver<Event>,
    },
    /// `press`: the entry as resolved; an `exec` is still pending.
    Pressed(TraceRecord),
    /// `press` of the dial or the roller: the entry of one run and how
    /// often it runs.
    Turned {
        /// The entry of one run.
        entry: TraceRecord,
        /// How often the action runs.
        runs: u32,
    },
    /// `reload`: whether the directory loaded, else every problem.
    Reloaded(Result<(), String>),
    /// `recipes`: the catalogue as read now.
    Recipes(Recipes),
    /// A command whose fields break [`Wire::check`].
    Invalid(String),
    /// `notify_context`: taken.
    Noted,
}

/// Where the daemon's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// The configuration directory (spec §7.1), reported in `status`.
    pub config_dir: PathBuf,
    /// The control socket (spec §8), handed to `exec` children as
    /// `KEYVO_SOCKET`.
    pub socket: PathBuf,
}

/// The daemon's view of the desktop: the focused window, the session lock
/// (spec §6.2) and the process tree behind a window (§6.3).
#[derive(Debug, Clone)]
pub struct Sources<W, S, L> {
    /// Reports the focused window.
    pub window: W,
    /// Reports the session lock.
    pub session: S,
    /// Tells whether a window's process belongs to a launch.
    pub lineage: L,
}

/// The devices the daemon drives, each by its own actor: the Keypad and the
/// Dialpad.
#[derive(Debug, Clone)]
pub struct Devices<C, D> {
    /// Opens the Keypad.
    pub keypad: C,
    /// Opens the Dialpad's receiver or node.
    pub dialpad: D,
}

/// Runs the daemon on the configuration in `paths.config_dir` until
/// `shutdown` completes, the Keypad and the Dialpad of `devices` each on
/// its own thread. `sources.window` reports the focused window; one
/// that does not start is logged and the daemon runs without focus
/// switching. `sources.session` reports the session lock; one that does
/// not start is logged once and the daemon runs without lock detection.
/// `sources.lineage` tells which launch a focused window belongs to.
/// Socket clients' commands arrive on `requests`.
///
/// # Errors
///
/// When a device actor's thread cannot be started or ends in a panic.
pub async fn run<C, D, I, W, S, L>(
    paths: &Paths,
    devices: Devices<C, D>,
    injector: I,
    sources: Sources<W, S, L>,
    events: broadcast::Sender<Event>,
    mut requests: mpsc::UnboundedReceiver<Request>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()>
where
    C: Connector,
    D: DialpadConnector,
    I: InputInjector + Send + 'static,
    W: WindowWatcher,
    S: SessionWatcher,
    L: Lineage,
{
    let Sources {
        window: mut watcher,
        session: mut session_watcher,
        lineage,
    } = sources;
    let config_dir = paths.config_dir.as_path();
    let loaded = layout::load(config_dir).unwrap_or_else(|errors| {
        tracing::warn!("configuration not loaded:\n{errors}");
        let _ = events.send(Event::ReloadFailed {
            error: errors.to_string(),
        });
        fallback(config_dir)
    });
    let engine = Engine::new(loaded);
    let timing = engine.config().timing;

    let (commands, mut device_events, actor) = start_keypad(devices.keypad, &timing)?;
    let (dialpad_stop, mut dialpad_events, dialpad_actor) =
        start_dialpad(devices.dialpad, &timing)?;

    let (focused, mut focus) = mpsc::unbounded_channel::<WindowContext>();
    if let Err(error) = watcher.start(Box::new(move |window| {
        let _ = focused.send(window);
    })) {
        tracing::warn!("focus switching off: {error}");
    }
    let (reported, mut sessions) = mpsc::unbounded_channel::<Session>();
    if let Err(error) = session_watcher.start(Box::new(move |session| {
        let _ = reported.send(session);
    })) {
        tracing::warn!("lock detection off, the pad stays live under a lock: {error}");
    }

    let mut changes = watch(config_dir);
    let (done, mut finished) = mpsc::unbounded_channel();
    let mut controller = Controller {
        config_dir,
        socket: &paths.socket,
        engine,
        injector,
        events,
        commands: commands.clone(),
        presses: PressDetector::new(timing.long_press),
        flush_at: None,
        reload_at: None,
        done,
        device: DeviceStatus {
            connected: false,
            keys: None,
            geometry: None,
            error: None,
        },
        dialpad: DialpadStatus::default(),
        context: None,
        ring: VecDeque::new(),
        lineage,
    };

    tokio::pin!(shutdown);
    loop {
        let now = Instant::now();
        let look_at = controller
            .engine
            .next_change(now.into_std())
            .map(Instant::from_std);
        let long_at = controller.presses.next_deadline().map(Instant::from_std);
        let state_at = controller.engine.next_deadline().map(Instant::from_std);
        tokio::select! {
            () = &mut shutdown => break,
            Some(event) = device_events.recv() => controller.device(event),
            Some(event) = dialpad_events.recv() => controller.dialpad(event),
            Some(window) = focus.recv() => controller.focus(window, ContextSource::Watcher),
            Some(session) = sessions.recv() => controller.session(session),
            Some(request) = requests.recv() => controller.request(request),
            Some(()) = changed(&mut changes) => {
                controller.reload_at = Some(now + controller.engine.config().timing.reload_settle);
            }
            Some(done) = finished.recv() => controller.finished(done),
            () = sleep(controller.reload_at) => { let _ = controller.reload(); }
            () = sleep(controller.flush_at) => controller.flush(),
            () = sleep(look_at) => controller.dirty(),
            () = sleep(long_at) => controller.long_presses(),
            () = sleep(state_at) => controller.poll_state(),
        }
    }
    drop(watcher);
    drop(session_watcher);

    let _ = commands.send(Command::Stop);
    drop(dialpad_stop);
    tokio::task::spawn_blocking(move || actor.join())
        .await
        .map_err(io::Error::other)?
        .map_err(|_| io::Error::other("the device actor panicked"))?;
    tokio::task::spawn_blocking(move || dialpad_actor.join())
        .await
        .map_err(io::Error::other)?
        .map_err(|_| io::Error::other("the Dialpad actor panicked"))
}

/// The Keypad's device actor on its own thread: its commands, its events,
/// its thread.
fn start_keypad<C: Connector>(
    connector: C,
    timing: &keyvo_core::config::Timing,
) -> io::Result<(
    std_mpsc::Sender<Command>,
    mpsc::UnboundedReceiver<device::Event>,
    std::thread::JoinHandle<()>,
)> {
    let settings = device::Settings::from_timing(timing);
    let (commands, inbox) = std_mpsc::channel();
    let (outbox, events) = mpsc::unbounded_channel();
    let actor = std::thread::Builder::new()
        .name("keyvo-device".into())
        .spawn(move || {
            device::run(connector, &settings, &inbox, |event| {
                let _ = outbox.send(event);
            });
        })?;
    Ok((commands, events, actor))
}

/// The Dialpad actor on its own thread: the sender whose drop stops it, its
/// events, its thread.
fn start_dialpad<D: DialpadConnector>(
    connector: D,
    timing: &keyvo_core::config::Timing,
) -> io::Result<(
    std_mpsc::Sender<()>,
    mpsc::UnboundedReceiver<dialpad::Event>,
    std::thread::JoinHandle<()>,
)> {
    let settings = dialpad::Settings::from_timing(timing);
    let (stop, stopped) = std_mpsc::channel::<()>();
    let (outbox, events) = mpsc::unbounded_channel();
    let actor = std::thread::Builder::new()
        .name("keyvo-dialpad".into())
        .spawn(move || {
            dialpad::run(connector, &settings, &stopped, |event| {
                let _ = outbox.send(event);
            });
        })?;
    Ok((stop, events, actor))
}

/// What a directory that does not load leaves: its `config.toml` when that
/// one is valid, so the timing survives a broken profile, and the built-in
/// global profile.
fn fallback(config_dir: &Path) -> Loaded {
    Loaded {
        config: config(config_dir),
        profiles: Profiles::new(std::collections::BTreeMap::new()),
    }
}

/// The `config.toml` in `config_dir` when it is there and valid, else the
/// defaults; [`run`] reports why it did not load.
#[must_use]
pub fn config(config_dir: &Path) -> Config {
    let path = config_dir.join("config.toml");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| Config::from_toml(&text, &path).ok())
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn watch(config_dir: &Path) -> Option<mpsc::UnboundedReceiver<()>> {
    crate::watch::watch(config_dir)
        .inspect_err(|error| {
            tracing::warn!(
                "hot-reload off, cannot watch {}: {error}",
                config_dir.display()
            );
        })
        .ok()
}

#[cfg(not(target_os = "linux"))]
fn watch(_config_dir: &Path) -> Option<mpsc::UnboundedReceiver<()>> {
    None
}

/// The next change, or never without a watcher.
async fn changed(changes: &mut Option<mpsc::UnboundedReceiver<()>>) -> Option<()> {
    match changes {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

/// Completes at `deadline`, or never without one.
async fn sleep(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// A finished `exec` or `launch`.
struct Done {
    /// Its record, the result in.
    record: TraceRecord,
    /// The pending launch it opened.
    launch: Option<LaunchId>,
    /// The key's `on_fail`, run when the result is an error; `None` for an
    /// `on_fail` run itself.
    on_fail: Option<Action>,
}

/// What can be held for a long press: a Keypad key or a Dialpad button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pressable {
    /// A key, 1-based.
    Key(u8),
    /// A Dialpad button.
    Button(ButtonSlot),
}

struct Controller<'a, I, L> {
    config_dir: &'a Path,
    socket: &'a Path,
    engine: Engine,
    injector: I,
    events: broadcast::Sender<Event>,
    commands: std_mpsc::Sender<Command>,
    presses: PressDetector<Pressable>,
    flush_at: Option<Instant>,
    reload_at: Option<Instant>,
    /// Where a finished `exec` or `launch` reports.
    done: mpsc::UnboundedSender<Done>,
    device: DeviceStatus,
    dialpad: DialpadStatus,
    /// The last focus report.
    context: Option<Context>,
    /// The newest `[trace].ring` trace entries, oldest first.
    ring: VecDeque<TraceRecord>,
    lineage: L,
}

impl<I: InputInjector, L: Lineage> Controller<'_, I, L> {
    fn emit(&mut self, event: Event) {
        if let Event::Trace(record) = &event {
            let ring = self.engine.config().trace.ring;
            self.ring.push_back(record.clone());
            while self.ring.len() > ring {
                self.ring.pop_front();
            }
        }
        // Nobody listening is fine.
        let _ = self.events.send(event);
    }

    /// Something the keys show changed: paint after the coalescing window.
    fn dirty(&mut self) {
        let coalesce = self.engine.config().timing.render_coalesce;
        self.flush_at
            .get_or_insert_with(|| Instant::now() + coalesce);
    }

    fn flush(&mut self) {
        self.flush_at = None;
        let frame = Frame {
            view: Some(self.engine.view()),
            accent: self.engine.active().profile.accent,
            keys: self.engine.specs(Instant::now().into_std()),
        };
        let _ = self.commands.send(Command::Paint(frame));
    }

    fn device(&mut self, event: device::Event) {
        match event {
            device::Event::Connected { keys, panel } => {
                self.engine.set_key_count(keys);
                let geometry = Geometry::from(&panel);
                self.device = DeviceStatus {
                    connected: true,
                    keys: Some(keys),
                    geometry: Some(geometry.clone()),
                    error: None,
                };
                self.dirty();
                self.emit(Event::Connected { keys, geometry });
            }
            device::Event::Disconnected { error } => {
                self.device = DeviceStatus {
                    connected: false,
                    keys: None,
                    geometry: None,
                    error: Some(error.clone()),
                };
                self.emit(Event::Disconnected { error });
            }
            device::Event::Painted { keys } => self.emit(Event::Painted { keys }),
            // The actor repaints the keys it holds by itself.
            device::Event::Reclaimed => tracing::warn!("keypad: claim lost, display reclaimed"),
            device::Event::Input(input) => {
                let now = Instant::now().into_std();
                // The Keypad has no rollers; the Dialpad is not the
                // daemon's yet (roadmap M3).
                let InputEvent::Button { control, edge } = input else {
                    return;
                };
                match (control, edge) {
                    (Control::Key(key), edge) => {
                        if let Some(slot) = KeySlot::new(key) {
                            let edge = match edge {
                                Edge::Press => KeyEdge::Press,
                                Edge::Release => KeyEdge::Release,
                            };
                            self.emit(Event::Key { key: slot, edge });
                        }
                        match edge {
                            Edge::Press => {
                                self.engine.touch(now);
                                self.presses.press(Pressable::Key(key), now);
                            }
                            Edge::Release => {
                                if let Some(kind) = self.presses.release(Pressable::Key(key), now) {
                                    self.pressed(Pressable::Key(key), kind);
                                }
                            }
                        }
                    }
                    (Control::Page(cid), edge) => {
                        let button = match cid {
                            PAGE_LEFT => PageButton::Left,
                            PAGE_RIGHT => PageButton::Right,
                            _ => {
                                tracing::debug!("diverted control 0x{cid:04x} ignored");
                                return;
                            }
                        };
                        let before = self.engine.view();
                        match edge {
                            Edge::Press => self.engine.page_press(button, now),
                            Edge::Release => self.engine.page_release(button, now),
                        }
                        self.switched(&before);
                    }
                }
            }
        }
    }

    fn dialpad(&mut self, event: dialpad::Event) {
        match event {
            dialpad::Event::Connected { slot, rollers } => {
                self.dialpad = DialpadStatus {
                    connected: true,
                    slot,
                    rollers: rollers
                        .iter()
                        .filter_map(|roller| {
                            Some(RollerStatus {
                                roller: Roller::from_id(roller.id)?,
                                increments: roller.increments,
                            })
                        })
                        .collect(),
                    error: None,
                };
                self.emit(Event::Dialpad(self.dialpad.clone()));
            }
            dialpad::Event::Disconnected { error } => {
                self.dialpad = DialpadStatus {
                    error: Some(error),
                    ..DialpadStatus::default()
                };
                self.emit(Event::Dialpad(self.dialpad.clone()));
            }
            dialpad::Event::Rotate { roller, delta } => {
                if let Some(roller) = Roller::from_id(roller) {
                    self.turn(roller, i32::from(delta), false);
                }
            }
            dialpad::Event::Button { button, edge } => {
                let Some(button) = ButtonSlot::new(button) else {
                    return;
                };
                let now = Instant::now().into_std();
                let pressable = Pressable::Button(button);
                match edge {
                    Edge::Press => {
                        self.emit(Event::Button {
                            button,
                            edge: KeyEdge::Press,
                        });
                        // A button is pad activity; a turn is not.
                        self.engine.touch(now);
                        self.presses.press(pressable, now);
                    }
                    Edge::Release => {
                        self.emit(Event::Button {
                            button,
                            edge: KeyEdge::Release,
                        });
                        if let Some(kind) = self.presses.release(pressable, now) {
                            self.pressed(pressable, kind);
                        }
                    }
                }
            }
        }
    }

    /// A turn of `roller` by `delta`: resolved on the active profile, its
    /// action run once per `step`, unless `simulated`; reported as a
    /// `dial` event. The entry of one run and how often it ran.
    fn turn(&mut self, roller: Roller, delta: i32, simulated: bool) -> (TraceRecord, u32) {
        let Turn { record, runs } = self
            .engine
            .turn(roller, delta, simulated, SystemTime::now());
        let ran = match (&record.result, simulated) {
            // Below one step, or nothing bound: no trace at the rate of
            // the turn reports.
            (Outcome::Unbound | Outcome::Pending, false) if runs == 0 => 0,
            (Outcome::Pending, false) => {
                for _ in 0..runs {
                    self.run(record.clone(), None);
                }
                runs
            }
            _ => {
                self.emit(Event::Trace(record.clone()));
                0
            }
        };
        // A client's turn has no device event, as a client's press has
        // none.
        if !simulated {
            self.emit(Event::Dial {
                roller,
                delta,
                actions: ran,
            });
        }
        (record, runs)
    }

    /// `source` reports the focused `window`.
    fn focus(&mut self, window: WindowContext, source: ContextSource) {
        tracing::debug!("focus from {source:?}: {window:?}");
        let held = self.engine.holds(&window.class);
        if held {
            tracing::debug!(
                "{} holds the context: the view stays {:?}",
                window.class,
                self.engine.view()
            );
        }
        let context = Context {
            class: window.class.clone(),
            title: window.title.clone(),
            pid: window.pid,
            source,
            held,
        };
        self.context = Some(context.clone());
        self.emit(Event::Context(context));
        let before = self.engine.view();
        self.engine
            .focus(window, Instant::now().into_std(), &self.lineage);
        self.switched(&before);
    }

    /// The session source reports `session`: a lock paints the lock page
    /// and blocks every press, an unlock paints what the state shows.
    fn session(&mut self, session: Session) {
        let before = self.engine.view();
        if self.engine.session(session) {
            tracing::info!(
                "session {}",
                if session.is_locked() {
                    "locked: the pad shows the lock and blocks every press"
                } else {
                    "unlocked"
                }
            );
        }
        self.switched(&before);
    }

    /// A pending focus change, a page-button long press or the page return
    /// is due.
    fn poll_state(&mut self) {
        let before = self.engine.view();
        self.engine.poll(Instant::now().into_std());
        self.switched(&before);
    }

    /// Repaints and reports when the pad shows something else than
    /// `before`.
    fn switched(&mut self, before: &View) {
        let view = self.engine.view();
        if view != *before {
            tracing::info!(
                "showing profile {} index {} (page {}) in {} mode{}",
                view.profile,
                view.index,
                view.page,
                view.mode.as_str(),
                if view.locked { " under the lock" } else { "" }
            );
            self.dirty();
            self.emit(Event::Switched(view));
        }
    }

    fn long_presses(&mut self) {
        for held in self.presses.poll(Instant::now().into_std()) {
            self.pressed(held, PressKind::Long);
        }
    }

    /// Resolves and runs a device press of a key or a Dialpad button.
    fn pressed(&mut self, held: Pressable, press: PressKind) {
        let trigger = Trigger {
            press,
            simulated: false,
        };
        match held {
            Pressable::Key(key) => {
                if let Some(slot) = KeySlot::new(key) {
                    self.press(slot, trigger);
                }
            }
            Pressable::Button(button) => {
                self.button(button, trigger);
            }
        }
    }

    /// Resolves a press of `slot` and, unless it is simulated, runs its
    /// action. The entry as resolved: a chord's result is in, an `exec`'s
    /// follows as a trace event. An action that fails runs the key's
    /// `on_fail` once.
    fn press(&mut self, slot: KeySlot, trigger: Trigger) -> TraceRecord {
        let record = self.engine.press(slot, trigger, SystemTime::now());
        let on_fail = self.engine.on_fail(slot);
        self.run(record, on_fail)
    }

    /// Resolves a press of Dialpad button `button` like a key's and runs
    /// it the same way.
    fn button(&mut self, button: ButtonSlot, trigger: Trigger) -> TraceRecord {
        let record = self.engine.button(button, trigger, SystemTime::now());
        let on_fail = self.engine.button_on_fail(button);
        self.run(record, on_fail)
    }

    /// Runs the action `record` resolved to, unless it is simulated or
    /// blocked, with `on_fail` for when it fails; the entry as resolved.
    fn run(&mut self, mut record: TraceRecord, on_fail: Option<Action>) -> TraceRecord {
        if record.trigger.simulated || matches!(record.result, Outcome::Blocked { .. }) {
            self.emit(Event::Trace(record.clone()));
            return record;
        }
        // A press ends the key's error or success look.
        if record.key.is_some() {
            self.dirty();
        }
        match record.action.clone() {
            None => self.emit(Event::Trace(record.clone())),
            Some(Action::Chord { keys }) => {
                record.finish(self.chord(&keys));
                self.emit(Event::Trace(record.clone()));
                if let Some(action) = on_fail.filter(|_| record.result.is_error()) {
                    self.on_fail(&record, action);
                }
            }
            Some(Action::Exec { cmd }) => {
                let env = exec::env(&record, self.socket);
                self.exec(&record, on_fail, cmd, env);
            }
            Some(action @ Action::Launch { .. }) => self.launch(&record, on_fail, &action),
        }
        record
    }

    /// Sends the events of the chord `keys` through the injector.
    fn chord(&mut self, keys: &Chord) -> Outcome {
        let events = keys.events();
        match self.injector.send(&events) {
            Ok(()) => Outcome::Sent { events },
            Err(error) => Outcome::Failed {
                error: error.to_string(),
            },
        }
    }

    /// Runs the `exec` command `cmd` of `record` with `env`.
    fn exec(
        &mut self,
        record: &TraceRecord,
        on_fail: Option<Action>,
        cmd: String,
        env: Vec<(&'static str, String)>,
    ) {
        let timeout = self.engine.config().timing.exec_timeout;
        self.spawn(record, None, on_fail, async move {
            exec::run(&cmd, &env, timeout).await
        });
    }

    /// Runs the key's `on_fail` `action` once after the press `failed`,
    /// with the key's `KEYVO_*` variables and `KEYVO_FAILED_CODE`, traced
    /// as a second record of the press. It changes no look and has no
    /// `on_fail` of its own; a `launch` opens no pending launch.
    fn on_fail(&mut self, failed: &TraceRecord, action: Action) {
        tracing::info!(
            "{} failed, running its on_fail: {}",
            failed.key.map_or_else(
                || failed.control.map_or_else(String::new, |c| c.to_string()),
                |key| format!("key {}", key.get())
            ),
            action.summary()
        );
        let mut record = failed.on_fail(action.clone());
        let env = exec::on_fail_env(failed, &failed.result, self.socket);
        match action {
            Action::Chord { keys } => {
                record.finish(self.chord(&keys));
                self.emit(Event::Trace(record));
            }
            Action::Exec { cmd } => self.exec(&record, None, cmd, env),
            Action::Launch { cmd, .. } => {
                let settle = self.engine.config().timing.launch_settle;
                self.spawn(&record, None, None, async move {
                    exec::launch(&cmd, &env, settle).await
                });
            }
        }
    }

    /// Starts the `launch` `action` of `record`. One that waits for its
    /// window opens a pending launch first, which learns the shell's pid
    /// at the spawn.
    fn launch(&mut self, record: &TraceRecord, on_fail: Option<Action>, action: &Action) {
        let Action::Launch { cmd, .. } = action else {
            return;
        };
        let pending = self.engine.launched(action, Instant::now().into_std());
        let env = exec::env(record, self.socket);
        let settle = self.engine.config().timing.launch_settle;
        match exec::Launch::spawn(cmd, &env) {
            Ok(launch) => {
                if let (Some(id), Some(pid)) = (pending, launch.pid()) {
                    self.engine.launch_started(id, pid);
                }
                self.spawn(record, pending, on_fail, launch.settle(settle));
            }
            Err(outcome) => self.spawn(record, pending, on_fail, std::future::ready(outcome)),
        }
    }

    /// Runs `action`, the command of `record`, without blocking; its result
    /// comes back through `done` with the pending `launch` it opened and
    /// the key's `on_fail`. The key's own action shows the running look,
    /// an `on_fail` run none.
    fn spawn(
        &mut self,
        record: &TraceRecord,
        launch: Option<LaunchId>,
        on_fail: Option<Action>,
        action: impl Future<Output = Outcome> + Send + 'static,
    ) {
        if record.branch.is_none() {
            self.engine.started(record, Instant::now().into_std());
        }
        let done = self.done.clone();
        let mut running = record.clone();
        tokio::spawn(async move {
            running.finish(action.await);
            let _ = done.send(Done {
                record: running,
                launch,
                on_fail,
            });
        });
    }

    /// The command of `done.record` finished. A `launch` that failed to
    /// start or exited with an error expects no window any more; one that
    /// exited 0 within the settle handed over to a running instance, whose
    /// window still comes. A failed action runs the key's `on_fail`; an
    /// `on_fail` run is only reported.
    fn finished(&mut self, done: Done) {
        let Done {
            record,
            launch,
            on_fail,
        } = done;
        if record.branch.is_some() {
            self.emit(Event::Trace(record));
            return;
        }
        if let Some(id) = launch
            && record.result.is_error()
        {
            self.engine.launch_failed(id);
        }
        self.engine.finished(&record, Instant::now().into_std());
        self.dirty();
        self.emit(Event::Trace(record.clone()));
        if let Some(action) = on_fail.filter(|_| record.result.is_error()) {
            self.on_fail(&record, action);
        }
    }

    /// Loads the configuration directory again; on failure the last good
    /// state stays and the problems come back.
    fn reload(&mut self) -> Result<(), String> {
        self.reload_at = None;
        match layout::load(self.config_dir) {
            Ok(loaded) => {
                let before = self.engine.view();
                self.engine.reload(loaded);
                tracing::info!("configuration reloaded");
                self.dirty();
                self.emit(Event::Reloaded);
                self.switched(&before);
                Ok(())
            }
            Err(errors) => {
                let error = errors.to_string();
                tracing::warn!("configuration not reloaded, keeping the last good one:\n{error}");
                self.emit(Event::ReloadFailed {
                    error: error.clone(),
                });
                Err(error)
            }
        }
    }

    /// The bundled recipe packs and the user's, read on every request (a
    /// few small files) so an edited pack shows without a reload. Reading
    /// runs nothing; a pack's command runs only from a key it was applied
    /// to.
    fn recipes(&self) -> Recipes {
        let catalogue = recipe::catalogue(self.config_dir);
        for problem in &catalogue.problems {
            tracing::warn!("recipe pack left out: {problem}");
        }
        catalogue.into()
    }

    fn request(&mut self, request: Request) {
        let answer = match request.command {
            Wire::Status {} => Answer::Status(Status {
                version: keyvo_core::VERSION.to_owned(),
                config_dir: self.config_dir.to_string_lossy().into_owned(),
                device: self.device.clone(),
                dialpad: self.dialpad.clone(),
                view: self.engine.view(),
                context: self.context.clone(),
                trace: self.ring.len(),
            }),
            Wire::Subscribe { replay, .. } => Answer::Subscribed {
                view: self.engine.view(),
                backlog: self
                    .ring
                    .iter()
                    .skip(self.ring.len().saturating_sub(replay))
                    .cloned()
                    .collect(),
                events: self.events.subscribe(),
            },
            Wire::Press {
                key,
                control,
                delta,
                kind,
                dry_run,
            } => {
                let trigger = Trigger {
                    press: kind,
                    simulated: dry_run,
                };
                // `Request::parse` checked the fields (`Command::check`).
                match (key, control, delta) {
                    (Some(key), ..) => Answer::Pressed(self.press(key, trigger)),
                    (None, Some(DialControl::Button(button)), _) => {
                        Answer::Pressed(self.button(button, trigger))
                    }
                    (None, Some(control), Some(delta)) => {
                        let roller = control.roller().unwrap_or(Roller::Dial);
                        let (entry, runs) = self.turn(roller, delta, dry_run);
                        Answer::Turned { entry, runs }
                    }
                    _ => Answer::Invalid(
                        Wire::Press {
                            key,
                            control,
                            delta,
                            kind,
                            dry_run,
                        }
                        .check()
                        .err()
                        .unwrap_or_default(),
                    ),
                }
            }
            Wire::Reload {} => Answer::Reloaded(self.reload()),
            Wire::Recipes {} => Answer::Recipes(self.recipes()),
            Wire::NotifyContext { class, title } => {
                self.focus(
                    WindowContext::new(&class, &title),
                    ContextSource::NotifyContext,
                );
                Answer::Noted
            }
        };
        // A client that went away does not need its answer.
        let _ = request.reply.send(answer);
    }
}
