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
//!   result comes back as the key's `success` or `error` look.
//! - The [`WindowWatcher`] reports the focused window and the page buttons
//!   arrive as diverted controls; both go to the context state machine,
//!   which the controller wakes at its next deadline (focus hysteresis,
//!   page-button long press). A changed profile, page or mode repaints and
//!   is reported as [`Event::Switched`]. Without a watcher the daemon runs
//!   on, without focus switching.
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
//! `focus_hysteresis` and `page_grace` are read at start; the others and
//! `[trace].ring` apply from the next reload.

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::SystemTime;

use keyvo_core::config::Config;
use keyvo_core::focus::{WindowContext, WindowWatcher};
use keyvo_core::inject::InputInjector;
use keyvo_core::layout::{self, Loaded, Profiles};
use keyvo_core::press::{PressDetector, PressKind};
use keyvo_core::profile::{Action, KeySlot};
use keyvo_core::socket::{
    Command as Wire, Context, ContextSource, DeviceStatus, Edge as KeyEdge, Geometry, Status,
};
use keyvo_core::state::{PageButton, View};
use keyvo_core::trace::{Outcome, TraceRecord, Trigger};
use keyvo_hid::input::{Control, Edge};
use keyvo_hid::reprog::{PAGE_LEFT, PAGE_RIGHT};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::Instant;

use crate::device::{self, Command, Connector, Frame};
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
    /// `reload`: whether the directory loaded, else every problem.
    Reloaded(Result<(), String>),
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

/// Runs the daemon on the configuration in `paths.config_dir` until
/// `shutdown` completes. `watcher` reports the focused window; one that does not
/// start is logged and the daemon runs without focus switching. Socket
/// clients' commands arrive on `requests`.
///
/// # Errors
///
/// When the device actor's thread cannot be started or ends in a panic.
pub async fn run<C, I, W>(
    paths: &Paths,
    connector: C,
    injector: I,
    mut watcher: W,
    events: broadcast::Sender<Event>,
    mut requests: mpsc::UnboundedReceiver<Request>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()>
where
    C: Connector,
    I: InputInjector + Send + 'static,
    W: WindowWatcher,
{
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

    let settings = device::Settings::from_timing(&timing);
    let (commands, inbox) = std_mpsc::channel();
    let (outbox, mut device_events) = mpsc::unbounded_channel();
    let actor = std::thread::Builder::new()
        .name("keyvo-device".into())
        .spawn(move || {
            device::run(connector, &settings, &inbox, |event| {
                let _ = outbox.send(event);
            });
        })?;

    let (focused, mut focus) = mpsc::unbounded_channel::<WindowContext>();
    if let Err(error) = watcher.start(Box::new(move |window| {
        let _ = focused.send(window);
    })) {
        tracing::warn!("focus switching off: {error}");
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
        context: None,
        ring: VecDeque::new(),
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
            Some(window) = focus.recv() => controller.focus(window, ContextSource::Watcher),
            Some(request) = requests.recv() => controller.request(request),
            Some(()) = changed(&mut changes) => {
                controller.reload_at = Some(now + controller.engine.config().timing.reload_settle);
            }
            Some(record) = finished.recv() => controller.finished(record),
            () = sleep(controller.reload_at) => { let _ = controller.reload(); }
            () = sleep(controller.flush_at) => controller.flush(),
            () = sleep(look_at) => controller.dirty(),
            () = sleep(long_at) => controller.long_presses(),
            () = sleep(state_at) => controller.poll_state(),
        }
    }
    drop(watcher);

    let _ = commands.send(Command::Stop);
    tokio::task::spawn_blocking(move || actor.join())
        .await
        .map_err(io::Error::other)?
        .map_err(|_| io::Error::other("the device actor panicked"))
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

struct Controller<'a, I> {
    config_dir: &'a Path,
    socket: &'a Path,
    engine: Engine,
    injector: I,
    events: broadcast::Sender<Event>,
    commands: std_mpsc::Sender<Command>,
    presses: PressDetector<u8>,
    flush_at: Option<Instant>,
    reload_at: Option<Instant>,
    done: mpsc::UnboundedSender<TraceRecord>,
    device: DeviceStatus,
    /// The last focus report.
    context: Option<Context>,
    /// The newest `[trace].ring` trace entries, oldest first.
    ring: VecDeque<TraceRecord>,
}

impl<I: InputInjector> Controller<'_, I> {
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
                match (input.control, input.edge) {
                    (Control::Key(key), edge) => {
                        if let Some(slot) = KeySlot::new(key) {
                            let edge = match edge {
                                Edge::Press => KeyEdge::Press,
                                Edge::Release => KeyEdge::Release,
                            };
                            self.emit(Event::Key { key: slot, edge });
                        }
                        match edge {
                            Edge::Press => self.presses.press(key, now),
                            Edge::Release => {
                                if let Some(kind) = self.presses.release(key, now) {
                                    self.pressed(key, kind);
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
        self.engine.focus(window, Instant::now().into_std());
        self.switched(&before);
    }

    /// A pending focus change or a page-button long press is due.
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
                "showing profile {} page {} in {} mode",
                view.profile,
                view.page,
                view.mode.as_str()
            );
            self.dirty();
            self.emit(Event::Switched(view));
        }
    }

    fn long_presses(&mut self) {
        for key in self.presses.poll(Instant::now().into_std()) {
            self.pressed(key, PressKind::Long);
        }
    }

    /// Resolves and runs a device press of `key`.
    fn pressed(&mut self, key: u8, press: PressKind) {
        if let Some(slot) = KeySlot::new(key) {
            self.press(
                slot,
                Trigger {
                    press,
                    simulated: false,
                },
            );
        }
    }

    /// Resolves a press of `slot` and, unless it is simulated, runs its
    /// action. The entry as resolved: a chord's result is in, an `exec`'s
    /// follows as a trace event.
    fn press(&mut self, slot: KeySlot, trigger: Trigger) -> TraceRecord {
        let mut record = self.engine.press(slot, trigger, SystemTime::now());
        if trigger.simulated {
            self.emit(Event::Trace(record.clone()));
            return record;
        }
        // A press ends the key's error or success look.
        self.dirty();
        match record.action.clone() {
            None => self.emit(Event::Trace(record.clone())),
            Some(Action::Chord { keys }) => {
                let events = keys.events();
                let outcome = match self.injector.send(&events) {
                    Ok(()) => Outcome::Sent { events },
                    Err(error) => Outcome::Failed {
                        error: error.to_string(),
                    },
                };
                record.finish(outcome);
                self.emit(Event::Trace(record.clone()));
            }
            Some(Action::Exec { cmd }) => {
                self.engine.started(&record, Instant::now().into_std());
                let env = exec::env(&record, self.socket);
                let timeout = self.engine.config().timing.exec_timeout;
                let done = self.done.clone();
                let mut running = record.clone();
                tokio::spawn(async move {
                    running.finish(exec::run(&cmd, &env, timeout).await);
                    let _ = done.send(running);
                });
            }
        }
        record
    }

    fn finished(&mut self, record: TraceRecord) {
        self.engine.finished(&record, Instant::now().into_std());
        self.dirty();
        self.emit(Event::Trace(record));
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

    fn request(&mut self, request: Request) {
        let answer = match request.command {
            Wire::Status {} => Answer::Status(Status {
                version: keyvo_core::VERSION.to_owned(),
                config_dir: self.config_dir.to_string_lossy().into_owned(),
                device: self.device.clone(),
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
            Wire::Press { key, kind, dry_run } => Answer::Pressed(self.press(
                key,
                Trigger {
                    press: kind,
                    simulated: dry_run,
                },
            )),
            Wire::Reload {} => Answer::Reloaded(self.reload()),
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
