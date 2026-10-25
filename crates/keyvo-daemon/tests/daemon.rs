//! The daemon end to end: a configuration directory on disk, `Pad`s built
//! on `FakeHidTransport` and a `FakeInputInjector`. Profile edits go
//! through the real inotify path, `exec` actions through `sh`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::path::Path;
use std::time::Duration;

use keyvo_core::chord::Chord;
use keyvo_core::feedback::Look;
use keyvo_core::focus::{FakeWindowWatcher, WindowContext};
use keyvo_core::inject::FakeInputInjector;
use keyvo_core::press::PressKind;
use keyvo_core::profile::{KeySlot, Profile};
use keyvo_core::render::{KeySpec, Renderer, TileSize};
use keyvo_core::state::{Mode, View};
use keyvo_core::trace::{Outcome, TraceRecord};
use keyvo_daemon::daemon::{self, Event};
use keyvo_hid::display;
use tokio::sync::{broadcast, oneshot};
use tokio::task::JoinHandle;

use support::{Image, Pad, Pads, startup};

/// How long a test waits for one event before it fails.
const PATIENCE: Duration = Duration::from_secs(10);

const CONFIG: &str = "\
[timing]
keep_alive_period = 3600000
divert_settle = 0
divert_spacing = 0
render_coalesce = 5
reconnect_interval = 20
reload_settle = 20
long_press = 500
exec_timeout = 1000
focus_hysteresis = 10
";

/// A profile for the terminal, its key 1 differs from the global one.
const TERMINAL: &str = r##"
schema_version = 1

[profile]
id = "terminal"
name = "Terminal"
match = { class = ["org.kde.konsole"] }
accent = "#2ecc71"

[[pages]]
name = "context"
[pages.keys.1]
icon = "lucide:terminal"
label = "Shell"
"##;

const GLOBAL: &str = r##"
schema_version = 1

[profile]
id = "global"
name = "Global"
accent = "#4e8bd6"

[[pages]]
[pages.keys.1]
icon = "lucide:hammer"
label = "Build"
action = { type = "exec", cmd = "sleep 0.1; exit 1" }

[pages.keys.2]
label = "Copy"
action = { type = "chord", keys = "ctrl+c" }

[pages.keys.3]
label = "Slow"
action = { type = "exec", cmd = "sleep 5" }
"##;

struct Daemon {
    dir: tempfile::TempDir,
    events: broadcast::Receiver<Event>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<std::io::Result<()>>,
}

impl Daemon {
    /// A daemon on `global` alone, without a focus source.
    fn start(global: &str, pads: Vec<Pad>, injector: FakeInputInjector) -> Self {
        Self::start_with(
            &[("global", global)],
            pads,
            injector,
            FakeWindowWatcher::failing("no focus source"),
        )
    }

    /// A daemon on the profiles `(id, text)`, focus reported by `watcher`.
    fn start_with(
        profiles: &[(&str, &str)],
        pads: Vec<Pad>,
        injector: FakeInputInjector,
        watcher: FakeWindowWatcher,
    ) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), CONFIG).unwrap();
        std::fs::create_dir(dir.path().join("profiles")).unwrap();
        for (id, text) in profiles {
            std::fs::write(dir.path().join(format!("profiles/{id}.toml")), text).unwrap();
        }
        let (sender, events) = broadcast::channel(256);
        let (stop, stopped) = oneshot::channel::<()>();
        let config_dir = dir.path().to_path_buf();
        let task = tokio::spawn(async move {
            daemon::run(
                &config_dir,
                Pads::new(pads),
                injector,
                watcher,
                sender,
                async {
                    let _ = stopped.await;
                },
            )
            .await
        });
        Self {
            dir,
            events,
            stop,
            task,
        }
    }

    /// The next event that `wanted` accepts.
    async fn until<T>(&mut self, mut wanted: impl FnMut(&Event) -> Option<T>) -> T {
        tokio::time::timeout(PATIENCE, async {
            loop {
                let event = self.events.recv().await.expect("an open event stream");
                if let Some(found) = wanted(&event) {
                    return found;
                }
            }
        })
        .await
        .expect("the awaited event")
    }

    async fn painted(&mut self) -> Vec<u8> {
        self.until(|event| match event {
            Event::Painted { keys } => Some(keys.clone()),
            _ => None,
        })
        .await
    }

    async fn switched(&mut self) -> View {
        self.until(|event| match event {
            Event::Switched(view) => Some(view.clone()),
            _ => None,
        })
        .await
    }

    /// Waits until the last image `pad` received for the key of `image`
    /// is `image`.
    async fn shows(&mut self, pad: &Pad, image: &Image) {
        let last = |pad: &Pad| {
            pad.images()
                .into_iter()
                .rev()
                .find(|i| i.rect == image.rect)
        };
        while last(pad).as_ref() != Some(image) {
            self.painted().await;
        }
    }

    async fn trace(&mut self) -> TraceRecord {
        self.until(|event| match event {
            Event::Trace(record) => Some(record.clone()),
            _ => None,
        })
        .await
    }

    /// Writes the global profile the way editors do: a temporary file
    /// renamed over the old one.
    fn edit(&self, text: &str) {
        let temp = self.dir.path().join("profiles/.global.toml.swp");
        std::fs::write(&temp, text).unwrap();
        std::fs::rename(&temp, self.dir.path().join("profiles/global.toml")).unwrap();
    }

    async fn stop(self) {
        self.stop.send(()).unwrap();
        self.task.await.unwrap().unwrap();
    }
}

fn profile(text: &str) -> Profile {
    Profile::from_toml(text, Path::new("global.toml")).unwrap()
}

/// The image of key `key` of `text`'s first page in `look`, on the
/// captured geometry.
fn image_of(text: &str, key: u8, look: Look) -> Image {
    image_on(text, 1, key, look)
}

/// The image of key `key` of page `page` (counted from 1) of `text`.
fn image_on(text: &str, page: usize, key: u8, look: Look) -> Image {
    let profile = profile(text);
    let slot = KeySlot::new(key).unwrap();
    let bound = profile.pages[page - 1].keys.get(&slot);
    let mut spec = KeySpec::from_key(bound, profile.profile.accent);
    if look != Look::Idle {
        spec = spec.with_look(bound, look);
    }
    let rect = display::captured_geometry().keys[usize::from(key - 1)];
    let size = TileSize {
        width: rect.width,
        height: rect.height,
    };
    Image {
        rect: (rect.x, rect.y, rect.width, rect.height),
        jpeg: Renderer::new().tile(&spec, size).unwrap().jpeg().unwrap(),
    }
}

async fn started(global: &str) -> (Daemon, Pad, FakeInputInjector) {
    let pad = Pad::new(&startup());
    let injector = FakeInputInjector::new();
    let mut daemon = Daemon::start(global, vec![pad.clone()], injector.clone());
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    assert!(pad.script_done());
    (daemon, pad, injector)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_profile_change_repaints_only_the_changed_key() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    assert_eq!(pad.images()[1], image_of(GLOBAL, 2, Look::Idle));

    let edited = GLOBAL.replace("\"Copy\"", "\"Paste\"");
    daemon.edit(&edited);
    assert_eq!(daemon.painted().await, vec![2]);
    let images = pad.images();
    assert_eq!(images.len(), 10);
    assert_eq!(images[9], image_of(&edited, 2, Look::Idle));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_broken_profile_keeps_the_last_good_state() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    daemon.edit(&GLOBAL.replace("label = \"Copy\"", "label = 7"));
    let error = daemon
        .until(|event| match event {
            Event::ReloadFailed { error } => Some(error.clone()),
            _ => None,
        })
        .await;
    assert!(error.contains("global.toml"), "{error}");
    // Nothing was repainted; the next good edit applies.
    let fixed = GLOBAL.replace("\"Slow\"", "\"Later\"");
    daemon.edit(&fixed);
    daemon
        .until(|event| matches!(event, Event::Reloaded).then_some(()))
        .await;
    assert_eq!(daemon.painted().await, vec![3]);
    let images = pad.images();
    assert_eq!(images.len(), 10);
    assert_eq!(images[9], image_of(&fixed, 3, Look::Idle));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_exec_that_exits_1_paints_running_then_the_error_look() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    pad.hold(&[1]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert_eq!(record.trigger.press, PressKind::Short);
    assert!(
        matches!(record.result, Outcome::Exited { code: Some(1), .. }),
        "{record:?}"
    );
    let error = image_of(GLOBAL, 1, Look::Error);
    while pad.images().last() != Some(&error) {
        assert!(daemon.painted().await.contains(&1));
    }
    assert!(pad.images().contains(&image_of(GLOBAL, 1, Look::Running)));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_exec_past_the_timeout_fails_with_the_error_look() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    pad.hold(&[3]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert_eq!(
        record.result,
        Outcome::Failed {
            error: "timed out after 1000 ms".into()
        }
    );
    let error = image_of(GLOBAL, 3, Look::Error);
    while pad.images().last() != Some(&error) {
        daemon.painted().await;
    }
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chord_key_sends_its_events_without_a_look() {
    let (mut daemon, pad, injector) = started(GLOBAL).await;
    pad.hold(&[2]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    let events = Chord::parse("ctrl+c").unwrap().events();
    assert_eq!(
        record.result,
        Outcome::Sent {
            events: events.clone()
        }
    );
    assert_eq!(injector.sent(), events);
    assert_eq!(pad.images().len(), 9);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_injection_is_traced() {
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start(
        GLOBAL,
        vec![pad.clone()],
        FakeInputInjector::failing("no uinput"),
    );
    daemon.painted().await;
    pad.hold(&[2]);
    pad.hold(&[]);
    assert_eq!(
        daemon.trace().await.result,
        Outcome::Failed {
            error: "input injection failed: no uinput".into()
        }
    );
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_key_fires_its_long_press_before_the_release() {
    let (mut daemon, pad, injector) = started(GLOBAL).await;
    pad.hold(&[2]);
    let record = daemon.trace().await;
    assert_eq!(record.trigger.press, PressKind::Long);
    assert_eq!(injector.sent().len(), 4);
    pad.hold(&[]);
    // The release after a long press runs nothing more.
    pad.hold(&[5]);
    pad.hold(&[]);
    assert_eq!(daemon.trace().await.result, Outcome::Unbound);
    assert_eq!(injector.sent().len(), 4);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_keypad_is_reclaimed_and_repainted() {
    let first = Pad::new(&startup());
    let second = Pad::new(&startup());
    let mut daemon = Daemon::start(
        GLOBAL,
        vec![first.clone(), second.clone()],
        FakeInputInjector::new(),
    );
    assert_eq!(daemon.painted().await.len(), 9);
    first.unplug();
    let error = daemon
        .until(|event| match event {
            Event::Disconnected { error } => Some(error.clone()),
            _ => None,
        })
        .await;
    assert_eq!(error, "device disconnected");
    daemon
        .until(|event| matches!(event, Event::Connected { keys: 9 }).then_some(()))
        .await;
    assert_eq!(daemon.painted().await.len(), 9);
    assert_eq!(second.images()[0], image_of(GLOBAL, 1, Look::Idle));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_broken_profile_at_start_paints_the_builtin_global_profile() {
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start("not toml", vec![pad.clone()], FakeInputInjector::new());
    daemon
        .until(|event| matches!(event, Event::ReloadFailed { .. }).then_some(()))
        .await;
    assert_eq!(daemon.painted().await.len(), 9);
    let blank = image_of(
        "schema_version = 1\n[profile]\nid = \"global\"\nname = \"G\"\n[[pages]]\n",
        1,
        Look::Idle,
    );
    assert_eq!(pad.images()[0], blank);
    daemon.stop().await;
}

/// The global profile with a `general` page after its first one.
fn with_general() -> String {
    format!(
        "{GLOBAL}
[[pages]]
name = \"general\"
[pages.keys.1]
icon = \"lucide:play\"
label = \"Play\"
"
    )
}

fn view(profile: &str, page: usize, mode: Mode) -> View {
    View {
        profile: profile.to_owned(),
        page,
        mode,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_painted_profile_follows_the_focused_window() {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_with(
        &[("global", GLOBAL), ("terminal", TERMINAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    assert_eq!(daemon.painted().await.len(), 9);
    assert_eq!(pad.images()[0], image_of(GLOBAL, 1, Look::Idle));

    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~ : bash")));
    assert_eq!(daemon.switched().await, view("terminal", 1, Mode::Auto));
    daemon.shows(&pad, &image_of(TERMINAL, 1, Look::Idle)).await;
    // A key press resolves on the terminal profile now.
    pad.hold(&[2]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert_eq!(record.profile, "terminal");
    assert_eq!(record.result, Outcome::Unbound);

    // Another class without a profile of its own shows the global one.
    assert!(watcher.focus(WindowContext::new("org.kde.dolphin", "Home")));
    assert_eq!(daemon.switched().await, view("global", 1, Mode::Auto));
    daemon.shows(&pad, &image_of(GLOBAL, 1, Look::Idle)).await;
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_button_toggles_the_general_page() {
    let global = with_general();
    let (mut daemon, pad, _) = started(&global).await;
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, Mode::Auto));
    daemon
        .shows(&pad, &image_on(&global, 2, 1, Look::Idle))
        .await;
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 1, Mode::Auto));
    daemon
        .shows(&pad, &image_on(&global, 1, 1, Look::Idle))
        .await;
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_page_button_switches_the_mode_and_both_escape() {
    let (mut daemon, pad, _) = started(&with_general()).await;
    // Another diverted control is ignored.
    pad.buttons(&[0x00c4]);
    pad.buttons(&[]);
    pad.buttons(&[0x01a2]);
    assert_eq!(daemon.switched().await, view("global", 1, Mode::Manual));
    pad.buttons(&[]);
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, Mode::Manual));
    pad.buttons(&[0x01a1]);
    pad.buttons(&[0x01a1, 0x01a2]);
    assert_eq!(daemon.switched().await, view("global", 1, Mode::Auto));
    pad.buttons(&[]);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reload_selects_the_profile_for_the_focused_window_again() {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_with(
        &[("global", GLOBAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    daemon.painted().await;
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~")));
    // No profile matches yet: nothing switches. A new profile file does.
    std::fs::write(daemon.dir.path().join("profiles/terminal.toml"), TERMINAL).unwrap();
    assert_eq!(daemon.switched().await, view("terminal", 1, Mode::Auto));
    daemon.shows(&pad, &image_of(TERMINAL, 1, Look::Idle)).await;
    daemon.stop().await;
}
