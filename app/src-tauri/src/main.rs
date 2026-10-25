//! The keyvo desktop app: a Tauri window around the Svelte frontend in
//! `app/src`. The commands are thin glue over `keyvo-client`, which holds
//! the tested logic (spec §10: the GUI talks to the daemon only through the
//! socket API).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use keyvo_client::editor::{Edited, KeyView};
use keyvo_client::{Client, Editor, Problem, Tiles};
use keyvo_core::edit::KeyFields;
use keyvo_core::press::PressKind;
use keyvo_core::profile::KeySlot;
use keyvo_core::render::TileSize;
use keyvo_core::socket::{Command, Reply, Status};
use keyvo_core::state::View;
use serde_json::Value;
use tauri::{AppHandle, Emitter as _, Manager as _};

/// The daemon's socket, as `keyvo ctl` finds it.
fn socket() -> std::path::PathBuf {
    keyvo_client::socket_path(
        std::env::var_os("KEYVO_SOCKET").as_deref(),
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        rustix::process::getuid().as_raw(),
    )
}

/// Sends `command` on a fresh connection.
fn request(command: Command) -> Result<Reply, String> {
    Client::connect(&socket())
        .and_then(|mut client| client.request(command))
        .map_err(|error| error.to_string())
}

/// The daemon's `status` reply.
#[tauri::command]
fn status() -> Result<Value, String> {
    serde_json::to_value(request(Command::Status {})?).map_err(|error| error.to_string())
}

/// A dry-run press of `key`: the daemon's trace entry.
#[tauri::command]
fn press(key: u8, kind: PressKind) -> Result<Value, String> {
    let key = KeySlot::new(key).ok_or_else(|| format!("no key {key}; keys count from 1"))?;
    let entry = Client::connect(&socket())
        .and_then(|mut client| client.dry_run(key, kind))
        .map_err(|error| error.to_string())?;
    serde_json::to_value(entry).map_err(|error| error.to_string())
}

/// The event every subscription update is emitted on (`UPDATE_EVENT` in
/// `app/src/lib/daemon.ts`).
const UPDATE_EVENT: &str = "keyvo://update";

/// The number of the newest subscription; an older one stops at its next
/// update.
#[derive(Default)]
struct Subscriptions(Arc<AtomicU64>);

/// Follows a new subscription on its own thread and emits its updates
/// (`keyvo_client::Update`) on [`UPDATE_EVENT`], the last one `closed`.
#[tauri::command]
fn subscribe(app: AppHandle, replay: usize) {
    let newest = Arc::clone(&app.state::<Subscriptions>().0);
    let mine = newest.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        keyvo_client::follow(Client::connect(&socket()), replay, |update| {
            newest.load(Ordering::SeqCst) == mine && app.emit(UPDATE_EVENT, &update).is_ok()
        });
    });
}

/// The configuration directory the daemon loads (`config_dir` of its
/// `status` reply), so the GUI edits the tree the pad shows, also under
/// `keyvo-daemon --config-dir`.
fn config_dir() -> Result<PathBuf, String> {
    match request(Command::Status {})? {
        Reply::Status(Status { config_dir, .. }) => Ok(PathBuf::from(config_dir)),
        other => Err(format!("unexpected answer to status: {other:?}")),
    }
}

/// The key editor on the user's configuration directory, its snapshots in
/// the state directory.
fn editor() -> Result<Editor, Problem> {
    let state = keyvo_core::layout::state_dir(
        std::env::var_os("XDG_STATE_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
    .ok_or_else(|| {
        Problem::message("no state directory: neither XDG_STATE_HOME nor HOME is set")
    })?;
    Ok(Editor::new(config_dir().map_err(Problem::message)?, &state))
}

fn slot(key: u8) -> Result<KeySlot, Problem> {
    KeySlot::new(key).ok_or_else(|| Problem::message(format!("no key {key}; keys count from 1")))
}

/// The file and the fields of `key` on `profile`'s page `page`, for the
/// key editor (`keyvo_client::editor::KeyView`).
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri hands command arguments over owned"
)]
fn key(profile: String, page: usize, key: u8) -> Result<KeyView, Problem> {
    editor()?
        .key(&profile, page, slot(key)?)
        .map_err(|error| Problem::from(&error))
}

/// Writes `fields` (`keyvo_core::edit::KeyFields`) into `key` on
/// `profile`'s page `page` after a snapshot of the file
/// (`keyvo_client::editor::Edited`); the daemon reloads it.
#[tauri::command]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri hands command arguments over owned"
)]
fn edit_key(profile: String, page: usize, key: u8, fields: Value) -> Result<Edited, Problem> {
    let fields: KeyFields = serde_json::from_value(fields)
        .map_err(|error| Problem::message(format!("invalid key fields: {error}")))?;
    editor()?
        .set_key(&profile, page, slot(key)?, &fields, SystemTime::now())
        .map_err(|error| Problem::from(&error))
}

/// The `keys` keys of `profile`'s page `page` from the daemon's
/// configuration directory in key order, rendered at `tile`; key count and
/// tile size come from the geometry of the daemon's `status` reply. Each
/// is the tile as a PNG data URL (`image`), with the key's `label`,
/// `action` and `layer` (`keyvo_client::tiles::KeySummary`).
#[tauri::command]
fn tiles(profile: String, page: usize, keys: u8, tile: TileSize) -> Result<Vec<Value>, String> {
    let dir = config_dir()?;
    let profiles = keyvo_core::layout::load(&dir)
        .map_err(|errors| errors.to_string())?
        .profiles;
    let view = View {
        profile,
        page,
        mode: keyvo_core::state::Mode::Auto,
    };
    let summaries =
        keyvo_client::tiles::keys(&profiles, &view, keys).map_err(|error| error.to_string())?;
    Tiles::new()
        .page(&profiles, &view, keys, tile)
        .map_err(|error| error.to_string())?
        .iter()
        .zip(summaries)
        .map(|(rendered, key)| {
            let png = rendered.png().map_err(|error| error.to_string())?;
            let mut value = serde_json::to_value(key).map_err(|error| error.to_string())?;
            value["image"] = format!("data:image/png;base64,{}", STANDARD.encode(png)).into();
            Ok(value)
        })
        .collect()
}

fn main() {
    if let Err(error) = tauri::Builder::default()
        .manage(Subscriptions::default())
        .invoke_handler(tauri::generate_handler![
            status, tiles, press, subscribe, key, edit_key
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("keyvo-app: {error}");
        std::process::exit(1);
    }
}
