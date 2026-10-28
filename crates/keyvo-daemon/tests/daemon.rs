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
use keyvo_core::lineage::{Lineage, NoLineage};
use keyvo_core::press::PressKind;
use keyvo_core::profile::{KeySlot, Profile};
use keyvo_core::render::{self, Canvas, KeyPlace, KeySpec, Renderer};
use keyvo_core::session::{FakeSessionWatcher, Session};
use keyvo_core::socket::{Command, Context, ContextSource, DeviceStatus, Edge, Geometry, Status};
use keyvo_core::stack::Layer;
use keyvo_core::state::{Mode, View};
use keyvo_core::trace::{Outcome, TraceRecord};
use keyvo_daemon::daemon::{self, Answer, Event, Request, Sources};
use keyvo_hid::display;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use support::{Image, Pad, Pads, startup};

/// How long a test waits for one event before it fails.
const PATIENCE: Duration = Duration::from_secs(10);

/// The control socket the daemon is told about, inside its configuration
/// directory; these tests talk to the controller directly.
const SOCKET: &str = "run/keyvo.sock";

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
    /// Locks and unlocks the daemon's session.
    session: FakeSessionWatcher,
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

    /// A daemon on `config` and the profiles `(id, text)`, its session
    /// unlocked until the test says otherwise through `session`.
    fn start_on(
        config: &str,
        profiles: &[(&str, &str)],
        pads: Vec<Pad>,
        injector: FakeInputInjector,
        watcher: FakeWindowWatcher,
    ) -> Self {
        Self::start_in(
            config,
            profiles,
            pads,
            injector,
            Sources {
                window: watcher,
                session: FakeSessionWatcher::new(),
                lineage: NoLineage,
            },
        )
    }

    /// A daemon on `config` and the profiles `(id, text)`, focus and
    /// session reported by `sources`.
    fn start_in(
        config: &str,
        profiles: &[(&str, &str)],
        pads: Vec<Pad>,
        injector: FakeInputInjector,
        sources: Sources<FakeWindowWatcher, FakeSessionWatcher, impl Lineage + Send + 'static>,
    ) -> Self {
        let session = sources.session.clone();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), config).unwrap();
        std::fs::create_dir(dir.path().join("profiles")).unwrap();
        for (id, text) in profiles {
            std::fs::write(dir.path().join(format!("profiles/{id}.toml")), text).unwrap();
        }
        let (sender, events) = broadcast::channel(256);
        let (requests, inbox) = mpsc::unbounded_channel();
        let (stop, stopped) = oneshot::channel::<()>();
        let paths = daemon::Paths {
            config_dir: dir.path().to_path_buf(),
            socket: dir.path().join(SOCKET),
        };
        let task = tokio::spawn(async move {
            daemon::run(
                &paths,
                Pads::new(pads),
                injector,
                sources,
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
            session,
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

/// The spec of key `key` of page `page` (counted from 1) of `profile`.
fn spec_on(profile: &Profile, page: usize, key: u8, look: Look) -> KeySpec {
    let slot = KeySlot::new(key).unwrap();
    let bound = profile.pages[page - 1].keys.get(&slot);
    let spec = KeySpec::from_key(bound, profile.profile.accent);
    if look == Look::Idle {
        spec
    } else {
        spec.with_look(bound, look)
    }
}

/// The image of `place` cropped from `canvas`.
fn crop(canvas: &Canvas, place: KeyPlace) -> Image {
    Image {
        rect: (place.x, place.y, place.size.width, place.size.height),
        jpeg: canvas.crop(place).unwrap().jpeg().unwrap(),
    }
}

/// The image of key `key` of page `page` (counted from 1) of `text`: the
/// key's rectangle cropped from the canvas, which a one-key change sends.
fn image_on(text: &str, page: usize, key: u8, look: Look) -> Image {
    let profile = profile(text);
    let renderer = Renderer::new();
    let mut canvas = renderer
        .canvas(
            profile.profile.accent,
            &display::captured_geometry().panel(),
        )
        .unwrap();
    let index = usize::from(key - 1);
    renderer
        .draw(&mut canvas, index, &spec_on(&profile, page, key, look))
        .unwrap();
    let place = canvas.panel().keys[index];
    crop(&canvas, place)
}

/// The one image of page `page` of `text` with every key idle: the union
/// of the key rectangles, which a new profile, page or mode sends.
fn union_on(text: &str, page: usize) -> Image {
    union_of_profile(&profile(text), page)
}

/// [`union_of`] the app profile `text` over the global profile `global`,
/// whose first page is its context page: a key `text` lacks shows the
/// global key, with the accent of `text` (spec §6.1).
fn union_over(text: &str, global: &str) -> Image {
    union_over_on(text, 1, global)
}

/// [`union_over`] for page `page` of `text`: every page of an app profile
/// lies over the global context page.
fn union_over_on(text: &str, page: usize, global: &str) -> Image {
    let mut stacked = profile(text);
    for (slot, key) in &profile(global).pages[0].keys {
        stacked.pages[page - 1]
            .keys
            .entry(*slot)
            .or_insert_with(|| key.clone());
    }
    union_of_profile(&stacked, page)
}

/// [`union_on`] for a parsed `profile`.
fn union_of_profile(profile: &Profile, page: usize) -> Image {
    let renderer = Renderer::new();
    let mut canvas = renderer
        .canvas(
            profile.profile.accent,
            &display::captured_geometry().panel(),
        )
        .unwrap();
    for key in 1..=9 {
        let spec = spec_on(profile, page, key, Look::Idle);
        renderer
            .draw(&mut canvas, usize::from(key - 1), &spec)
            .unwrap();
    }
    let union = canvas.panel().bounds(0..9).unwrap();
    assert_eq!(
        (union.x, union.y, union.size.width, union.size.height),
        (23, 6, 435, 434)
    );
    crop(&canvas, union)
}

/// [`union_on`] the first page.
fn union_of(text: &str) -> Image {
    union_on(text, 1)
}

async fn started(global: &str) -> (Daemon, Pad, FakeInputInjector) {
    let pad = Pad::new(&startup());
    let injector = FakeInputInjector::new();
    let mut daemon = Daemon::start(global, vec![pad.clone()], injector.clone());
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    assert!(pad.script_done());
    // One image: the panel background and the nine keys.
    assert_eq!(pad.images(), [union_of(global)]);
    (daemon, pad, injector)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_profile_change_repaints_only_the_changed_key() {
    let (mut daemon, pad, _) = started(GLOBAL).await;

    let edited = GLOBAL.replace("\"Copy\"", "\"Paste\"");
    daemon.edit(&edited);
    assert_eq!(daemon.painted().await, vec![2]);
    let images = pad.images();
    assert_eq!(images.len(), 2);
    assert_eq!(images[1], image_of(&edited, 2, Look::Idle));
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
    assert_eq!(images.len(), 2);
    assert_eq!(images[1], image_of(&written, 2, Look::Idle));
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
    assert_eq!(images.len(), 2);
    assert_eq!(images[1], image_of(&fixed, 3, Look::Idle));
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
    // Every feedback change writes one image: key 1's rectangle, cropped
    // from the canvas.
    let key1 = error.rect;
    assert_eq!(key1, (24, 6, 118, 118));
    assert!(pad.images()[1..].iter().all(|image| image.rect == key1));
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

/// A short `exec_timeout` under a shorter `launch_settle`, so a launched
/// child visibly outlives the timeout.
const LAUNCH_CONFIG: &str = "\
[timing]
divert_settle = 0
divert_spacing = 0
render_coalesce = 5
reconnect_interval = 20
reload_settle = 20
exec_timeout = 300
launch_settle = 100
";

const LAUNCHER: &str = r#"
schema_version = 1

[profile]
id = "global"
name = "Global"

[[pages]]
[pages.keys.1]
label = "App"
action = { type = "launch", cmd = "sleep 5" }

[pages.keys.2]
label = "Broken"
action = { type = "launch", cmd = "exit 3" }

[pages.keys.3]
label = "Handed over"
action = { type = "launch", cmd = "true" }
"#;

/// Whether process `pid` runs: it exists and is not a zombie.
fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .map(|(_, rest)| !rest.starts_with('Z'))
        })
        .unwrap_or(false)
}

async fn launcher() -> (Daemon, Pad) {
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start_on(
        LAUNCH_CONFIG,
        &[("global", LAUNCHER)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        FakeWindowWatcher::failing("no focus source"),
    );
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    (daemon, pad)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launch_still_running_after_the_settle_has_started_and_lives_on() {
    let (mut daemon, pad) = launcher().await;
    let pressed = std::time::Instant::now();
    pad.hold(&[1]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert!(
        pressed.elapsed() < Duration::from_secs(1),
        "{:?}",
        pressed.elapsed()
    );
    let Outcome::Started { pid } = record.result else {
        panic!("not started: {record:?}");
    };
    let success = image_of(LAUNCHER, 1, Look::Success);
    while pad.images().last() != Some(&success) {
        daemon.painted().await;
    }
    // Past `exec_timeout` the child still runs: nothing killed it.
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(alive(pid));
    // Its own group: the shell and whatever it started go together.
    let killed = std::process::Command::new("sh")
        .args(["-c", &format!("kill -9 -{pid}")])
        .status()
        .unwrap();
    assert!(killed.success());
    let deadline = std::time::Instant::now() + PATIENCE;
    while alive(pid) {
        assert!(std::time::Instant::now() < deadline, "pid {pid} still runs");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launch_that_exits_within_the_settle_reports_its_code() {
    let (mut daemon, pad) = launcher().await;
    pad.hold(&[2]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert!(
        matches!(record.result, Outcome::Exited { code: Some(3), .. }),
        "{record:?}"
    );
    let error = image_of(LAUNCHER, 2, Look::Error);
    while pad.images().last() != Some(&error) {
        daemon.painted().await;
    }
    pad.hold(&[3]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert!(
        matches!(record.result, Outcome::Exited { code: Some(0), .. }),
        "{record:?}"
    );
    let success = image_of(LAUNCHER, 3, Look::Success);
    while pad.images().last() != Some(&success) {
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
    assert_eq!(pad.images().len(), 1);
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
async fn a_keep_alive_expiry_is_reclaimed_and_the_union_repainted() {
    let pad = Pad::new(&(startup() + &support::fixture("display-reclaim.txt")));
    let mut daemon = Daemon::start(GLOBAL, vec![pad.clone()], FakeInputInjector::new());
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    pad.send(&support::reports(&support::fixture("claim-lost.txt")));
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    assert!(pad.script_done());
    // On the claim and on the reclaim: one image of the whole page.
    assert_eq!(pad.images(), [union_of(GLOBAL), union_of(GLOBAL)]);
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
        .until(|event| matches!(event, Event::Connected { keys: 9, .. }).then_some(()))
        .await;
    assert_eq!(daemon.painted().await.len(), 9);
    assert_eq!(second.images(), [union_of(GLOBAL)]);
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
    assert_eq!(pad.images(), [union_of(builtin)]);
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

/// The view of `page` of `profile` at list position `index`.
fn view(profile: &str, page: usize, index: isize, mode: Mode) -> View {
    View {
        profile: profile.to_owned(),
        page,
        mode,
        index,
        locked: false,
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
    assert_eq!(pad.images(), [union_of(GLOBAL)]);

    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~ : bash")));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    // The switch writes exactly one image: the union of the keys, with the
    // terminal's panel background and every key on it.
    assert_eq!(daemon.painted().await, (1..=9).collect::<Vec<_>>());
    // The terminal lacks keys 2 to 9: the global keys show under it.
    assert_eq!(
        pad.images(),
        [union_of(GLOBAL), union_over(TERMINAL, GLOBAL)]
    );
    // A key press resolves on the terminal profile now, a key it lacks on
    // the global layer: key 2 sends the global chord.
    pad.hold(&[2]);
    pad.hold(&[]);
    let record = daemon.trace().await;
    assert_eq!(record.profile, "terminal");
    assert_eq!(record.layer, Some(Layer::Global));
    assert!(matches!(record.result, Outcome::Sent { .. }));
    pad.hold(&[1]);
    pad.hold(&[]);
    let own = daemon.trace().await;
    assert_eq!(
        (own.layer, own.result),
        (Some(Layer::App), Outcome::Unbound)
    );

    // Another class without a profile of its own shows the global one.
    assert!(watcher.focus(WindowContext::new("org.kde.dolphin", "Home")));
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_of(GLOBAL)).await;
    daemon.stop().await;
}

/// The one image of the lock page over the panel of `text`'s profile: its
/// accent tints the panel and underlines the lock.
fn lock_union(text: &str) -> Image {
    let accent = profile(text).profile.accent;
    let renderer = Renderer::new();
    let mut canvas = renderer
        .canvas(accent, &display::captured_geometry().panel())
        .unwrap();
    for (index, spec) in render::lock_page(accent, 9).iter().enumerate() {
        renderer.draw(&mut canvas, index, spec).unwrap();
    }
    let union = canvas.panel().bounds(0..9).unwrap();
    crop(&canvas, union)
}

/// `view` under the session lock.
fn locked(view: View) -> View {
    View {
        locked: true,
        ..view
    }
}

fn blocked() -> Outcome {
    Outcome::Blocked {
        reason: "session locked".into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_locked_session_paints_the_lock_and_blocks_every_press() {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let injector = FakeInputInjector::new();
    let mut daemon = Daemon::start_with(
        &[("global", GLOBAL), ("terminal", TERMINAL)],
        vec![pad.clone()],
        injector.clone(),
        watcher.clone(),
    );
    daemon.painted().await;
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~ : bash")));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_over(TERMINAL, GLOBAL)).await;

    // The lock keeps the view and paints the lock page in its accent.
    assert!(daemon.session.report(Session::Locked));
    assert_eq!(
        daemon.switched().await,
        locked(view("terminal", 1, 1, Mode::Auto))
    );
    daemon.shows(&pad, &lock_union(TERMINAL)).await;
    assert!(daemon.status().await.view.locked);

    // Every key is blocked: neither the exec of key 1 nor the global chord
    // of key 2 runs, and no look changes.
    let images = pad.images().len();
    for key in [1, 2, 9] {
        pad.hold(&[key]);
        pad.hold(&[]);
        let record = daemon.trace().await;
        assert_eq!((record.key.get(), record.result), (key, blocked()));
        assert_eq!(record.action, None);
    }
    // The page buttons do nothing: no switch comes before the next trace.
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    pad.hold(&[5]);
    pad.hold(&[]);
    let mut seen = Vec::new();
    let record = daemon
        .until(|event| {
            seen.push(event.clone());
            match event {
                Event::Trace(record) => Some(record.clone()),
                _ => None,
            }
        })
        .await;
    assert_eq!(record.result, blocked());
    assert!(!seen.iter().any(|event| matches!(event, Event::Switched(_))));
    // A client press is blocked the same way, a dry run included.
    for dry_run in [false, true] {
        let record = daemon.press(2, dry_run).await;
        assert_eq!(
            (record.result, record.trigger.simulated),
            (blocked(), dry_run)
        );
    }
    assert!(injector.sent().is_empty());
    assert_eq!(pad.images().len(), images);

    // A focus change under the lock applies: the lock shows over the new
    // profile, and the unlock shows that profile.
    assert!(watcher.focus(WindowContext::new("org.kde.dolphin", "Home")));
    assert_eq!(
        daemon.switched().await,
        locked(view("global", 1, 1, Mode::Auto))
    );
    daemon.shows(&pad, &lock_union(GLOBAL)).await;
    assert!(daemon.session.report(Session::Unlocked));
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_of(GLOBAL)).await;
    pad.hold(&[2]);
    pad.hold(&[]);
    assert!(matches!(daemon.trace().await.result, Outcome::Sent { .. }));
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daemon_started_under_the_lock_paints_the_lock_page() {
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start_in(
        CONFIG,
        &[("global", GLOBAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        Sources {
            window: FakeWindowWatcher::failing("no focus source"),
            session: FakeSessionWatcher::starting(Session::Locked),
            lineage: NoLineage,
        },
    );
    daemon.shows(&pad, &lock_union(GLOBAL)).await;
    assert!(daemon.status().await.view.locked);
    assert!(daemon.session.report(Session::Unlocked));
    daemon.shows(&pad, &union_of(GLOBAL)).await;
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_session_source_the_daemon_runs_unlocked() {
    let pad = Pad::new(&startup());
    let injector = FakeInputInjector::new();
    let mut daemon = Daemon::start_in(
        CONFIG,
        &[("global", GLOBAL)],
        vec![pad.clone()],
        injector.clone(),
        Sources {
            window: FakeWindowWatcher::failing("no focus source"),
            session: FakeSessionWatcher::failing("no logind"),
            lineage: NoLineage,
        },
    );
    assert_eq!(daemon.painted().await.len(), 9);
    assert_eq!(pad.images(), [union_of(GLOBAL)]);
    assert!(!daemon.session.report(Session::Locked));
    pad.hold(&[2]);
    pad.hold(&[]);
    assert!(matches!(daemon.trace().await.result, Outcome::Sent { .. }));
    assert!(!daemon.status().await.view.locked);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_button_walks_to_the_general_page() {
    let global = with_general();
    let (mut daemon, pad, _) = started(&global).await;
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Auto));
    daemon.shows(&pad, &union_on(&global, 2)).await;
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_on(&global, 1)).await;
    daemon.stop().await;
}

/// The terminal profile with a second page.
fn terminal_with_two_pages() -> String {
    format!(
        "{TERMINAL}
[[pages]]
name = \"git\"
[pages.keys.1]
icon = \"lucide:git-branch\"
label = \"Branch\"
"
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn the_page_buttons_walk_the_page_list_of_a_two_page_profile() {
    let global = with_general();
    let terminal = terminal_with_two_pages();
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_with(
        &[("global", &global), ("terminal", &terminal)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    daemon.painted().await;
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~")));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_over(&terminal, &global)).await;
    // Right: the terminal's own page 2, still over the global context page.
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("terminal", 2, 2, Mode::Auto));
    daemon
        .shows(&pad, &union_over_on(&terminal, 2, &global))
        .await;
    // Right again is past the end: no event, no image.
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    // Left: the context page; left: the global general page as it is.
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_over(&terminal, &global)).await;
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Auto));
    daemon.shows(&pad, &union_on(&global, 2)).await;
    // Left past the end: nothing, so the next event is the right press's.
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    daemon.stop().await;
}

/// The `page_return_global` of [`a_global_page_returns_after_the_last_key_press`].
const PAGE_RETURN: Duration = Duration::from_millis(1000);

#[tokio::test(flavor = "multi_thread")]
async fn a_global_page_returns_after_the_last_key_press() {
    let global = with_general();
    let terminal = terminal_with_two_pages();
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_on(
        &format!("{CONFIG}page_return_global = {}\n", PAGE_RETURN.as_millis()),
        &[("global", &global), ("terminal", &terminal)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    daemon.painted().await;
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~")));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    pad.buttons(&[0x01a1]);
    let released = tokio::time::Instant::now();
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Auto));
    daemon.shows(&pad, &union_on(&global, 2)).await;
    // A key press half way restarts the clock: the return comes the full
    // time after it, with the event and the repaint. Half way from the
    // release, however long the repaint took to arrive.
    tokio::time::sleep_until(released + PAGE_RETURN / 2).await;
    let pressed = std::time::Instant::now();
    pad.hold(&[2]);
    pad.hold(&[]);
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    assert!(pressed.elapsed() >= PAGE_RETURN, "{:?}", pressed.elapsed());
    daemon.shows(&pad, &union_over(&terminal, &global)).await;
    // The own pages do not return with `page_return_own` at its default 0.
    pad.buttons(&[0x01a2]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("terminal", 2, 2, Mode::Auto));
    let quiet = tokio::time::timeout(PAGE_RETURN * 3, daemon.switched()).await;
    assert!(quiet.is_err(), "{quiet:?}");
    assert_eq!(
        daemon.status().await.view,
        view("terminal", 2, 2, Mode::Auto)
    );
    daemon.stop().await;
}

/// A global profile whose `general` page opens applications: key 1 waits
/// for a window of its class, key 2 does not, key 3 hands over to a
/// running Konsole (exits 0 at once), key 4 fails.
const OPENER: &str = r##"
schema_version = 1

[profile]
id = "global"
name = "Global"
accent = "#4e8bd6"

[[pages]]
name = "context"
[pages.keys.1]
label = "Build"

[[pages]]
name = "general"
[pages.keys.1]
label = "App"
action = { type = "launch", cmd = "sleep 5", class = "org.example.Launched" }

[pages.keys.2]
label = "Shy"
action = { type = "launch", cmd = "sleep 5", class = "org.example.Launched", wait_focus = false }

[pages.keys.3]
label = "Handed over"
action = { type = "launch", cmd = "true", class = "org.kde.konsole" }

[pages.keys.4]
label = "Broken"
action = { type = "launch", cmd = "exit 3", class = "org.kde.konsole" }
"##;

/// A daemon on [`OPENER`] and [`TERMINAL`] with the `/proc` lineage,
/// `page_return_global` of [`PAGE_RETURN`] and a `launch_settle` of
/// 100 ms, showing the global `general` page.
async fn opener() -> (Daemon, Pad, FakeWindowWatcher) {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_in(
        &format!(
            "{CONFIG}page_return_global = {}\nlaunch_settle = 100\n",
            PAGE_RETURN.as_millis()
        ),
        &[("global", OPENER), ("terminal", TERMINAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        Sources {
            window: watcher.clone(),
            session: FakeSessionWatcher::new(),
            lineage: keyvo_daemon::lineage::ProcLineage::new(),
        },
    );
    daemon.painted().await;
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Auto));
    (daemon, pad, watcher)
}

/// Presses `key` and returns its trace entry once the launch settled.
async fn open(daemon: &mut Daemon, pad: &Pad, key: u8) -> Outcome {
    pad.hold(&[key]);
    pad.hold(&[]);
    daemon.trace().await.result
}

/// Ends the launch led by `pid` with everything it started.
fn end(pid: u32) {
    let killed = std::process::Command::new("sh")
        .args(["-c", &format!("kill -9 -{pid}")])
        .status()
        .unwrap();
    assert!(killed.success());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launched_window_keeps_the_global_page_until_the_return() {
    let (mut daemon, pad, watcher) = opener().await;
    let Outcome::Started { pid } = open(&mut daemon, &pad, 1).await else {
        panic!("not started");
    };
    // Half the return time on: the return the key press started would come
    // well before `page_return_global` after the window.
    tokio::time::sleep(PAGE_RETURN / 2).await;
    // Konsole's window from the launch's session: not the launch's class,
    // so the lineage alone makes it the launch's.
    let arrived = std::time::Instant::now();
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~").with_pid(pid)));
    // The page stays under the terminal profile; the return comes
    // `page_return_global` after the window, to its context page.
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    assert!(arrived.elapsed() >= PAGE_RETURN, "{:?}", arrived.elapsed());
    assert_eq!(daemon.status().await.context.unwrap().pid, Some(pid));
    end(pid);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_window_or_a_launch_that_does_not_wait_shows_the_context_page() {
    let (mut daemon, pad, watcher) = opener().await;
    let Outcome::Started { pid } = open(&mut daemon, &pad, 1).await else {
        panic!("not started");
    };
    // Konsole, clicked with the mouse: another class, another process.
    let clicked = std::time::Instant::now();
    let foreign = WindowContext::new("org.kde.konsole", "~").with_pid(std::process::id());
    assert!(watcher.focus(foreign));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    assert!(
        clicked.elapsed() < PAGE_RETURN / 2,
        "{:?}",
        clicked.elapsed()
    );
    // `wait_focus = false`: its own window shows the context page at once.
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Auto));
    let Outcome::Started { pid: shy } = open(&mut daemon, &pad, 2).await else {
        panic!("not started");
    };
    let shown = std::time::Instant::now();
    assert!(watcher.focus(WindowContext::new("org.example.Shy", "x").with_pid(shy)));
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
    assert!(shown.elapsed() < PAGE_RETURN / 2, "{:?}", shown.elapsed());
    end(pid);
    end(shy);
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_handover_keeps_the_page_for_its_class_and_a_failed_launch_does_not() {
    let (mut daemon, pad, watcher) = opener().await;
    let handed = open(&mut daemon, &pad, 3).await;
    assert!(
        matches!(handed, Outcome::Exited { code: Some(0), .. }),
        "{handed:?}"
    );
    // The running Konsole's window, outside the launch's session: its
    // class matches the launch.
    let arrived = std::time::Instant::now();
    let konsole = WindowContext::new("org.kde.konsole", "~").with_pid(std::process::id());
    assert!(watcher.focus(konsole.clone()));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    assert!(arrived.elapsed() >= PAGE_RETURN, "{:?}", arrived.elapsed());
    // Another window, back to the `general` page, the failing launch.
    assert!(watcher.focus(WindowContext::new("org.example.Other", "x")));
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Auto));
    let broken = open(&mut daemon, &pad, 4).await;
    assert!(
        matches!(broken, Outcome::Exited { code: Some(3), .. }),
        "{broken:?}"
    );
    let shown = std::time::Instant::now();
    assert!(watcher.focus(konsole));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    assert!(shown.elapsed() < PAGE_RETURN / 2, "{:?}", shown.elapsed());
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_held_page_button_switches_the_mode_and_both_escape() {
    let (mut daemon, pad, _) = started(&with_general()).await;
    // Another diverted control is ignored.
    pad.buttons(&[0x00c4]);
    pad.buttons(&[]);
    pad.buttons(&[0x01a2]);
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Manual));
    pad.buttons(&[]);
    pad.buttons(&[0x01a1]);
    pad.buttons(&[]);
    assert_eq!(daemon.switched().await, view("global", 2, 0, Mode::Manual));
    pad.buttons(&[0x01a1]);
    pad.buttons(&[0x01a1, 0x01a2]);
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
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
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    daemon.shows(&pad, &union_over(TERMINAL, GLOBAL)).await;
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_the_device_the_view_and_the_trace_size() {
    let pad = Pad::new(&startup());
    let mut daemon = Daemon::start(GLOBAL, vec![pad.clone()], FakeInputInjector::new());
    daemon.painted().await;
    let status = daemon.status().await;
    assert_eq!(status.version, keyvo_core::VERSION);
    assert_eq!(status.config_dir, daemon.dir.path().to_string_lossy());
    // The geometry is the one the device published, not a built-in one.
    let geometry = Geometry::from(&display::captured_geometry().panel());
    assert_eq!(
        status.device,
        DeviceStatus {
            connected: true,
            keys: Some(9),
            geometry: Some(geometry),
            error: None
        }
    );
    assert_eq!(status.view, view("global", 1, 1, Mode::Auto));
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
            geometry: None,
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
async fn an_exec_child_sees_the_daemon_socket() {
    let marks = tempfile::tempdir().unwrap();
    let out = marks.path().join("socket");
    let global = GLOBAL.replace(
        "sleep 0.1; exit 1",
        &format!("printf '%s' \\\"$KEYVO_SOCKET\\\" > '{}'", out.display()),
    );
    let (mut daemon, _, _) = started(&global).await;
    assert_eq!(daemon.press(1, false).await.result, Outcome::Pending);
    let done = daemon.trace().await;
    assert!(done.result.is_success(), "{done:?}");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        daemon.dir.path().join(SOCKET).to_string_lossy()
    );
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
    assert_eq!(pad.images().len(), 1);
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
        held: false,
    };
    let reported = daemon
        .until(|event| match event {
            Event::Context(context) => Some(context.clone()),
            _ => None,
        })
        .await;
    assert_eq!(reported, notified);
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
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
    assert!(!reported.held);
    daemon.stop().await;
}

impl Daemon {
    /// The next `context` event.
    async fn context(&mut self) -> Context {
        self.until(|event| match event {
            Event::Context(context) => Some(context.clone()),
            _ => None,
        })
        .await
    }

    /// The context and view events up to and including the next view
    /// switch.
    async fn up_to_a_switch(&mut self) -> Vec<Event> {
        let mut seen = Vec::new();
        self.until(|event| {
            if matches!(event, Event::Context(_) | Event::Switched(_)) {
                seen.push(event.clone());
            }
            matches!(event, Event::Switched(_)).then_some(())
        })
        .await;
        seen
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_keyvo_app_holds_the_context() {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_with(
        &[("global", GLOBAL), ("terminal", TERMINAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    daemon.painted().await;
    let app = WindowContext::new("keyvo-app", "keyvo").with_pid(9);
    assert!(watcher.focus(app));
    let held = Context {
        class: "keyvo-app".into(),
        title: "keyvo".into(),
        pid: Some(9),
        source: ContextSource::Watcher,
        held: true,
    };
    assert_eq!(daemon.context().await, held);
    assert_eq!(daemon.status().await.context, Some(held));

    // The next window applies as usual: the first view event after the app's
    // report follows the Konsole report.
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~")));
    let seen = daemon.up_to_a_switch().await;
    assert!(
        matches!(seen.first(), Some(Event::Context(c)) if c.class == "org.kde.konsole" && !c.held),
        "{seen:?}"
    );
    assert_eq!(
        seen.last(),
        Some(&Event::Switched(view("terminal", 1, 1, Mode::Auto)))
    );

    // Held over the terminal: well past the hysteresis the pad still shows
    // it, and the next view event is the Dolphin report's.
    assert!(watcher.focus(WindowContext::new("keyvo-app", "keyvo")));
    assert!(daemon.context().await.held);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        daemon.status().await.view,
        view("terminal", 1, 1, Mode::Auto)
    );
    assert!(watcher.focus(WindowContext::new("org.kde.dolphin", "Home")));
    let seen = daemon.up_to_a_switch().await;
    assert!(
        matches!(seen.first(), Some(Event::Context(c)) if c.class == "org.kde.dolphin"),
        "{seen:?}"
    );
    assert_eq!(
        seen.last(),
        Some(&Event::Switched(view("global", 1, 1, Mode::Auto)))
    );
    daemon.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_hold_list_lets_the_keyvo_app_switch() {
    let pad = Pad::new(&startup());
    let watcher = FakeWindowWatcher::new();
    let mut daemon = Daemon::start_on(
        &format!("{CONFIG}[focus]\nhold_classes = []\n"),
        &[("global", GLOBAL), ("terminal", TERMINAL)],
        vec![pad.clone()],
        FakeInputInjector::new(),
        watcher.clone(),
    );
    daemon.painted().await;
    assert!(watcher.focus(WindowContext::new("org.kde.konsole", "~")));
    assert_eq!(daemon.switched().await, view("terminal", 1, 1, Mode::Auto));
    assert!(watcher.focus(WindowContext::new("keyvo-app", "keyvo")));
    assert!(!daemon.context().await.held);
    assert_eq!(daemon.switched().await, view("global", 1, 1, Mode::Auto));
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
    assert_eq!(now, view("global", 1, 1, Mode::Auto));
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
