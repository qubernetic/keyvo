//! The daemon end to end: a configuration directory on disk, `Pad`s built
//! on `FakeHidTransport` and a `FakeInputInjector`. Profile edits go
//! through the real inotify path, `exec` actions through `sh`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::path::Path;
use std::time::{Duration, SystemTime};

use keyvo_client::Editor;
use keyvo_core::chord::Chord;
use keyvo_core::feedback::Look;
use keyvo_core::focus::{FakeWindowWatcher, WindowContext};
use keyvo_core::inject::FakeInputInjector;
use keyvo_core::press::PressKind;
use keyvo_core::profile::{KeySlot, Profile};
use keyvo_core::render::{KeySpec, Renderer, TileSize};
use keyvo_core::socket::{Command, Context, ContextSource, DeviceStatus, Edge, Status};
use keyvo_core::state::{Mode, View};
use keyvo_core::trace::{Outcome, TraceRecord};
use keyvo_daemon::daemon::{self, Answer, Event, Request};
use keyvo_hid::display;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use support::{Image, Pad, Pads, startup};

/// How long a test waits for one event before it fails.
const PATIENCE: Duration = Duration::from_secs(10);

const CONFIG: &str = "\
[timing]
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
    requests: mpsc::UnboundedSender<Request>,
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
        Self::start_on(CONFIG, profiles, pads, injector, watcher)
    }

    /// A daemon on `config` and the profiles `(id, text)`.
    fn start_on(
        config: &str,
        profiles: &[(&str, &str)],
        pads: Vec<Pad>,
        injector: FakeInputInjector,
        watcher: FakeWindowWatcher,
    ) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), config).unwrap();
        std::fs::create_dir(dir.path().join("profiles")).unwrap();
        for (id, text) in profiles {
            std::fs::write(dir.path().join(format!("profiles/{id}.toml")), text).unwrap();
        }
        let (sender, events) = broadcast::channel(256);
        let (requests, inbox) = mpsc::unbounded_channel();
        let (stop, stopped) = oneshot::channel::<()>();
        let config_dir = dir.path().to_path_buf();
        let task = tokio::spawn(async move {
            daemon::run(
                &config_dir,
                Pads::new(pads),
                injector,
                watcher,
                sender,
                inbox,
                async {
                    let _ = stopped.await;
                },
            )
            .await
        });
        Self {
            dir,
            events,
            requests,
            stop,
            task,
        }
    }

    /// What the controller answers to `command`.
    async fn ask(&self, command: Command) -> Answer {
        let (reply, answer) = oneshot::channel();
        self.requests.send(Request { command, reply }).unwrap();
        tokio::time::timeout(PATIENCE, answer)
            .await
            .expect("an answer")
            .expect("a running controller")
    }

    async fn status(&self) -> Status {
        match self.ask(Command::Status {}).await {
            Answer::Status(status) => status,
            other => panic!("not a status: {other:?}"),
        }
    }

    /// The trace entry a client `press` returns.
    async fn press(&self, key: u8, dry_run: bool) -> TraceRecord {
        let command = Command::Press {
            key: KeySlot::new(key).unwrap(),
            kind: PressKind::Short,
            dry_run,
        };
        match self.ask(command).await {
            Answer::Pressed(record) => record,
            other => panic!("not a press: {other:?}"),
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
    let renderer = Renderer::new();
    let under = renderer.panel_color(profile.profile.accent);
    Image {
        rect: (rect.x, rect.y, rect.width, rect.height),
        jpeg: renderer.tile(&spec, size).unwrap().jpeg_on(under).unwrap(),
    }
}

/// The panel background of `text` on the captured geometry: the bounding
/// box of the keys.
fn backdrop_of(text: &str) -> Image {
    let size = TileSize {
        width: 435,
        height: 434,
    };
    let backdrop = Renderer::new()
        .backdrop(profile(text).profile.accent, size)
        .unwrap();
    Image {
        rect: (23, 6, 435, 434),
        jpeg: backdrop.jpeg().unwrap(),
    }
}

async fn started(global: &str) -> (Daemon, Pad, FakeInputInjector) {
    let pad = Pad::new(&startup());
    let injector = FakeInputInjector::new();
    let mut daemon = Daemon::start(global, vec![pad.clone()], injector.clone());
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    assert!(pad.script_done());
    // The panel background, then the nine keys.
    assert_eq!(pad.images().len(), 10);
    (daemon, pad, injector)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_profile_change_repaints_only_the_changed_key() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    assert_eq!(pad.images()[0], backdrop_of(GLOBAL));
    assert_eq!(pad.images()[2], image_of(GLOBAL, 2, Look::Idle));

    let edited = GLOBAL.replace("\"Copy\"", "\"Paste\"");
    daemon.edit(&edited);
    assert_eq!(daemon.painted().await, vec![2]);
    let images = pad.images();
    assert_eq!(images.len(), 11);
    assert_eq!(images[10], image_of(&edited, 2, Look::Idle));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_editor_write_repaints_the_key_through_the_reload() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    let state = tempfile::tempdir().unwrap();
    let editor = Editor::new(daemon.dir.path().to_path_buf(), state.path());
    let key = KeySlot::new(2).unwrap();
    let mut fields = editor.key("global", 1, key).unwrap().fields;
    fields.label = Some("Paste".into());
    let edited = editor
        .set_key("global", 1, key, &fields, SystemTime::now())
        .unwrap();
    assert!(edited.snapshot.is_some());

    assert_eq!(daemon.painted().await, vec![2]);
    let written = std::fs::read_to_string(edited.path).unwrap();
    assert_eq!(written, GLOBAL.replace("\"Copy\"", "\"Paste\""));
    let images = pad.images();
    assert_eq!(images.len(), 11);
    assert_eq!(images[10], image_of(&written, 2, Look::Idle));
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
    assert_eq!(images.len(), 11);
    assert_eq!(images[10], image_of(&fixed, 3, Look::Idle));
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
    assert_eq!(pad.images().len(), 10);
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
async fn a_keep_alive_expiry_is_reclaimed_and_the_panel_and_every_key_repainted() {
    let pad = Pad::new(&(startup() + &support::fixture("display-reclaim.txt")));
    let mut daemon = Daemon::start(GLOBAL, vec![pad.clone()], FakeInputInjector::new());
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    pad.send(&support::reports(&support::fixture("claim-lost.txt")));
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    assert!(pad.script_done());
    let images = pad.images();
    assert_eq!(images.len(), 20);
    // On the claim and on the reclaim: the panel background first.
    assert_eq!(images[0], backdrop_of(GLOBAL));
    assert_eq!(images[10], backdrop_of(GLOBAL));
    for key in 1..=9 {
        assert_eq!(
            images[10 + usize::from(key)],
            image_of(GLOBAL, key, Look::Idle)
        );
    }
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
    assert_eq!(second.images()[0], backdrop_of(GLOBAL));
    assert_eq!(second.images()[1], image_of(GLOBAL, 1, Look::Idle));
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
    let builtin = "schema_version = 1\n[profile]\nid = \"global\"\nname = \"G\"\n[[pages]]\n";
    assert_eq!(pad.images()[0], backdrop_of(builtin));
    assert_eq!(pad.images()[1], image_of(builtin, 1, Look::Idle));
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
    assert_eq!(pad.images()[0], backdrop_of(GLOBAL));
    assert_eq!(pad.images()[1], image_of(GLOBAL, 1, Look::Idle));

    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~ : bash")));
    assert_eq!(daemon.switched().await, view("terminal", 1, Mode::Auto));
    // The switch paints the terminal's panel background before its keys,
    // then every key on it.
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    let images = pad.images();
    assert_eq!(images.len(), 20);
    assert_eq!(images[10], backdrop_of(TERMINAL));
    assert_eq!(images[11], image_of(TERMINAL, 1, Look::Idle));
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

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_the_device_the_view_and_the_trace_size() {
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start(GLOBAL, vec![pad.clone()], FakeInputInjector::new());
    daemon.painted().await;
    let status = daemon.status().await;
    assert_eq!(status.version, keyvo_core::VERSION);
    assert_eq!(
        status.device,
        DeviceStatus {
            connected: true,
            keys: Some(9),
            error: None
        }
    );
    assert_eq!(status.view, view("global", 1, Mode::Auto));
    assert_eq!(status.context, None);
    assert_eq!(status.trace, 0);

    pad.unplug();
    daemon
        .until(|event| matches!(event, Event::Disconnected { .. }).then_some(()))
        .await;
    assert_eq!(
        daemon.status().await.device,
        DeviceStatus {
            connected: false,
            keys: None,
            error: Some("device disconnected".into())
        }
    );
    daemon.stop().await;
}

/// A global profile whose key 1 touches `marker` and key 2 sends a chord.
fn marking(marker: &Path) -> String {
    format!(
        "schema_version = 1
[profile]
id = \"global\"
name = \"Global\"

[[pages]]
[pages.keys.1]
label = \"Mark\"
action = {{ type = \"exec\", cmd = \"touch '{}'\" }}

[pages.keys.2]
label = \"Copy\"
action = {{ type = \"chord\", keys = \"ctrl+c\" }}
",
        marker.display()
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dry_run_press_is_traced_as_simulated_and_executes_nothing() {
    let marks = tempfile::tempdir().unwrap();
    let marker = marks.path().join("ran");
    let (mut daemon, pad, injector) = started(&marking(&marker)).await;

    let record = daemon.press(1, true).await;
    assert!(record.trigger.simulated);
    assert_eq!(record.result, Outcome::DryRun);
    assert_eq!(record.key, KeySlot::new(1).unwrap());
    assert!(record.action.is_some());
    // The same entry goes to the subscribers and the ring buffer.
    assert_eq!(daemon.trace().await, record);
    let record = daemon.press(2, true).await;
    assert_eq!(record.result, Outcome::DryRun);
    assert_eq!(daemon.trace().await, record);
    assert_eq!(daemon.status().await.trace, 2);

    // A real press of the same key runs, so the dry run would have shown.
    let real = daemon.press(1, false).await;
    assert!(!real.trigger.simulated);
    assert_eq!(real.result, Outcome::Pending);
    let done = daemon.trace().await;
    assert!(
        matches!(done.result, Outcome::Exited { code: Some(0), .. }),
        "{done:?}"
    );
    assert!(marker.exists());
    // The real press paints its success look; the dry runs sent no chord.
    assert!(injector.sent().is_empty());
    daemon
        .shows(&pad, &image_of(&marking(&marker), 1, Look::Success))
        .await;
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dry_run_leaves_the_marker_absent_and_the_pad_unpainted() {
    let marks = tempfile::tempdir().unwrap();
    let marker = marks.path().join("ran");
    let (mut daemon, pad, _) = started(&marking(&marker)).await;
    daemon.press(1, true).await;
    daemon.trace().await;
    // Longer than the exec and a coalesced paint would take.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!marker.exists());
    assert_eq!(pad.images().len(), 10);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_press_runs_a_chord_like_the_device() {
    let (mut daemon, _, injector) = started(GLOBAL).await;
    let record = daemon.press(2, false).await;
    let events = Chord::parse("ctrl+c").unwrap().events();
    assert_eq!(
        record.result,
        Outcome::Sent {
            events: events.clone()
        }
    );
    assert_eq!(daemon.trace().await, record);
    assert_eq!(injector.sent(), events);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reload_answers_whether_the_directory_loaded() {
    let (mut daemon, _, _) = started(GLOBAL).await;
    let broken = daemon.dir.path().join("profiles/broken.toml");
    std::fs::write(&broken, "not toml").unwrap();
    match daemon.ask(Command::Reload {}).await {
        Answer::Reloaded(Err(error)) => assert!(error.contains("broken.toml"), "{error}"),
        other => panic!("{other:?}"),
    }
    std::fs::remove_file(&broken).unwrap();
    assert!(matches!(
        daemon.ask(Command::Reload {}).await,
        Answer::Reloaded(Ok(()))
    ));
    daemon
        .until(|event| matches!(event, Event::Reloaded).then_some(()))
        .await;
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn notify_context_switches_like_the_watcher_and_is_reported() {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_with(
        &[("global", GLOBAL), ("terminal", TERMINAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    daemon.painted().await;
    let command = Command::NotifyContext {
        class: "org.kde.konsole".into(),
        title: "~ : bash".into(),
    };
    assert!(matches!(daemon.ask(command).await, Answer::Noted));
    let notified = Context {
        class: "org.kde.konsole".into(),
        title: "~ : bash".into(),
        pid: None,
        source: ContextSource::NotifyContext,
    };
    let reported = daemon
        .until(|event| match event {
            Event::Context(context) => Some(context.clone()),
            _ => None,
        })
        .await;
    assert_eq!(reported, notified);
    assert_eq!(daemon.switched().await, view("terminal", 1, Mode::Auto));
    assert_eq!(daemon.status().await.context, Some(notified));

    // The watcher's reports are context events too.
    assert!(watcher.focus(WindowContext::new("org.kde.dolphin", "Home").with_pid(7)));
    let reported = daemon
        .until(|event| match event {
            Event::Context(context) => Some(context.clone()),
            _ => None,
        })
        .await;
    assert_eq!(reported.source, ContextSource::Watcher);
    assert_eq!(reported.pid, Some(7));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn device_keys_are_reported_as_key_events() {
    let (mut daemon, pad, _) = started(GLOBAL).await;
    pad.hold(&[5]);
    pad.hold(&[]);
    let mut edges = Vec::new();
    while edges.len() < 2 {
        let event = daemon
            .until(|event| match event {
                Event::Key { key, edge } => Some((key.get(), *edge)),
                _ => None,
            })
            .await;
        edges.push(event);
    }
    assert_eq!(edges, [(5, Edge::Press), (5, Edge::Release)]);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribe_replays_the_newest_ring_entries_then_streams() {
    let config = format!("{CONFIG}[trace]\nring = 2\n");
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start_on(
        &config,
        &[("global", GLOBAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        FakeWindowWatcher::failing("no focus source"),
    );
    daemon.painted().await;
    let mut pressed = Vec::new();
    for key in [4, 5, 6] {
        pressed.push(daemon.press(key, true).await);
    }
    // The ring keeps the newest two.
    assert_eq!(daemon.status().await.trace, 2);
    let command = Command::Subscribe {
        events: Vec::new(),
        replay: 5,
    };
    let Answer::Subscribed {
        view: now,
        backlog,
        mut events,
    } = daemon.ask(command).await
    else {
        panic!("not subscribed");
    };
    assert_eq!(now, view("global", 1, Mode::Auto));
    assert_eq!(backlog, pressed[1..]);
    let later = daemon.press(7, true).await;
    let streamed = tokio::time::timeout(PATIENCE, async {
        loop {
            if let Event::Trace(record) = events.recv().await.unwrap() {
                return record;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(streamed, later);

    let Answer::Subscribed { backlog, .. } = daemon
        .ask(Command::Subscribe {
            events: Vec::new(),
            replay: 1,
        })
        .await
    else {
        panic!("not subscribed");
    };
    assert_eq!(backlog, [later]);
    daemon.stop().await;
}
