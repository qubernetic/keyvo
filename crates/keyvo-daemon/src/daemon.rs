//! The controller: one tokio task that owns the [`Engine`] and ties the
//! device actor, the presses, the `exec` processes and the hot-reload
//! together.
//!
//! - State changes mark the pad dirty; the specs go to the actor
//!   `[timing].render_coalesce` after the first change, so a burst becomes
//!   one paint, and the actor writes only the keys that differ.
//! - A key press starts the long-press timer; the release or the timer
//!   makes it a short or long press, resolved on the active page. `chord`
//!   goes through the [`InputInjector`]; `exec` runs in its own task and its
//!   result comes back as the key's `success` or `error` look.
//! - A change in the configuration directory reloads it after
//!   `[timing].reload_settle` of quiet. A directory that does not load
//!   keeps the last good state.
//!
//! The `[timing]` values of the device actor and `long_press` are read at
//! start; the others apply from the next reload.

use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::mpsc as std_mpsc;
use std::time::SystemTime;

use keyvo_core::config::Config;
use keyvo_core::inject::InputInjector;
use keyvo_core::layout::{self, Loaded, Profiles};
use keyvo_core::press::{PressDetector, PressKind};
use keyvo_core::profile::{Action, KeySlot};
use keyvo_core::trace::{Outcome, TraceRecord, Trigger};
use keyvo_hid::input::{Control, Edge};
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;

use crate::device::{self, Command, Connector};
use crate::engine::Engine;
use crate::exec;

/// What the daemon reports, for logs, tests and later the socket's
/// `subscribe` (slice 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A Keypad is claimed.
    Connected {
        /// Its key count.
        keys: u8,
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
    /// A press was resolved and, for an action, executed.
    Trace(TraceRecord),
    /// The configuration directory was reloaded.
    Reloaded,
    /// The configuration directory did not load; the last good state stays.
    ReloadFailed {
        /// Every problem, one per line.
        error: String,
    },
}

/// Runs the daemon on the configuration in `config_dir` until `shutdown`
/// completes.
///
/// # Errors
///
/// When the device actor's thread cannot be started or ends in a panic.
pub async fn run<C, I>(
    config_dir: &Path,
    connector: C,
    injector: I,
    events: broadcast::Sender<Event>,
    shutdown: impl Future<Output = ()>,
) -> io::Result<()>
where
    C: Connector,
    I: InputInjector + Send + 'static,
{
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

    let mut changes = watch(config_dir);
    let (done, mut finished) = mpsc::unbounded_channel();
    let mut controller = Controller {
        config_dir,
        engine,
        injector,
        events,
        commands: commands.clone(),
        presses: PressDetector::new(timing.long_press),
        flush_at: None,
        reload_at: None,
        done,
    };

    tokio::pin!(shutdown);
    loop {
        let now = Instant::now();
        let look_at = controller
            .engine
            .next_change(now.into_std())
            .map(Instant::from_std);
        let long_at = controller.presses.next_deadline().map(Instant::from_std);
        tokio::select! {
            () = &mut shutdown => break,
            Some(event) = device_events.recv() => controller.device(event),
            Some(()) = changed(&mut changes) => {
                controller.reload_at = Some(now + controller.engine.config().timing.reload_settle);
            }
            Some((slot, record)) = finished.recv() => controller.finished(slot, record),
            () = sleep(controller.reload_at) => controller.reload(),
            () = sleep(controller.flush_at) => controller.flush(),
            () = sleep(look_at) => controller.dirty(),
            () = sleep(long_at) => controller.long_presses(),
        }
    }

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
    let path = config_dir.join("config.toml");
    let config = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| Config::from_toml(&text, &path).ok())
        .unwrap_or_default();
    Loaded {
        config,
        profiles: Profiles::new(std::collections::BTreeMap::new()),
    }
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
    engine: Engine,
    injector: I,
    events: broadcast::Sender<Event>,
    commands: std_mpsc::Sender<Command>,
    presses: PressDetector<u8>,
    flush_at: Option<Instant>,
    reload_at: Option<Instant>,
    done: mpsc::UnboundedSender<(KeySlot, TraceRecord)>,
}

impl<I: InputInjector> Controller<'_, I> {
    fn emit(&self, event: Event) {
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
        let specs = self.engine.specs(Instant::now().into_std());
        let _ = self.commands.send(Command::Paint(specs));
    }

    fn device(&mut self, event: device::Event) {
        match event {
            device::Event::Connected { keys } => {
                self.engine.set_key_count(keys);
                self.dirty();
                self.emit(Event::Connected { keys });
            }
            device::Event::Disconnected { error } => self.emit(Event::Disconnected { error }),
            device::Event::Painted { keys } => self.emit(Event::Painted { keys }),
            device::Event::Input(input) => {
                let now = Instant::now().into_std();
                match (input.control, input.edge) {
                    (Control::Key(key), Edge::Press) => self.presses.press(key, now),
                    (Control::Key(key), Edge::Release) => {
                        if let Some(kind) = self.presses.release(key, now) {
                            self.pressed(key, kind);
                        }
                    }
                    // The page buttons switch pages with slice 11.
                    (Control::Page(cid), _) => tracing::debug!("page button 0x{cid:04x}"),
                }
            }
        }
    }

    fn long_presses(&mut self) {
        for key in self.presses.poll(Instant::now().into_std()) {
            self.pressed(key, PressKind::Long);
        }
    }

    /// Resolves and runs a press of `key`.
    fn pressed(&mut self, key: u8, press: PressKind) {
        let Some(slot) = KeySlot::new(key) else {
            return;
        };
        let trigger = Trigger {
            press,
            simulated: false,
        };
        let mut record = self.engine.press(slot, trigger, SystemTime::now());
        // A press ends the key's error or success look.
        self.dirty();
        match record.action.clone() {
            None => self.emit(Event::Trace(record)),
            Some(Action::Chord { keys }) => {
                let events = keys.events();
                let outcome = match self.injector.send(&events) {
                    Ok(()) => Outcome::Sent { events },
                    Err(error) => Outcome::Failed {
                        error: error.to_string(),
                    },
                };
                record.finish(outcome);
                self.emit(Event::Trace(record));
            }
            Some(Action::Exec { cmd }) => {
                self.engine.started(slot, Instant::now().into_std());
                let env = exec::env(&record);
                let timeout = self.engine.config().timing.exec_timeout;
                let done = self.done.clone();
                tokio::spawn(async move {
                    record.finish(exec::run(&cmd, &env, timeout).await);
                    let _ = done.send((slot, record));
                });
            }
        }
    }

    fn finished(&mut self, slot: KeySlot, record: TraceRecord) {
        self.engine
            .finished(slot, &record.result, Instant::now().into_std());
        self.dirty();
        self.emit(Event::Trace(record));
    }

    fn reload(&mut self) {
        self.reload_at = None;
        match layout::load(self.config_dir) {
            Ok(loaded) => {
                self.engine.reload(loaded);
                tracing::info!("configuration reloaded");
                self.dirty();
                self.emit(Event::Reloaded);
            }
            Err(errors) => {
                tracing::warn!("configuration not reloaded, keeping the last good one:\n{errors}");
                self.emit(Event::ReloadFailed {
                    error: errors.to_string(),
                });
            }
        }
    }
}
