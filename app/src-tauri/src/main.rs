//! The keyvo desktop app: a Tauri window around the Svelte frontend in
//! `app/src`. The commands are thin glue over `keyvo-client`, which holds
//! the tested logic (spec §10: the GUI talks to the daemon only through the
//! socket API).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use keyvo_client::{Client, Tiles};
use keyvo_core::render::TileSize;
use keyvo_core::socket::{Command, Reply};
use keyvo_core::state::View;
use serde_json::Value;

/// The tile size of the mirror: the Keypad's key side as captured from
/// Options+ (`keyvo_hid::display::captured_geometry`), until the daemon
/// reports the device geometry.
const TILE: TileSize = TileSize {
    width: 118,
    height: 118,
};

/// The Keypad's key count, for the same reason.
const KEYS: u8 = 9;

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

/// The keys of `profile`'s page `page` from the user's configuration
/// directory, as PNG data URLs in key order.
#[tauri::command]
fn tiles(profile: String, page: usize) -> Result<Vec<String>, String> {
    let dir = keyvo_core::layout::config_dir(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
    .ok_or("no configuration directory: neither XDG_CONFIG_HOME nor HOME is set")?;
    let profiles = keyvo_core::layout::load(&dir)
        .map_err(|errors| errors.to_string())?
        .profiles;
    let view = View {
        profile,
        page,
        mode: keyvo_core::state::Mode::Auto,
    };
    Tiles::new()
        .page(&profiles, &view, KEYS, TILE)
        .map_err(|error| error.to_string())?
        .iter()
        .map(|tile| {
            tile.png()
                .map(|png| format!("data:image/png;base64,{}", STANDARD.encode(png)))
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn main() {
    if let Err(error) = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![status, tiles])
        .run(tauri::generate_context!())
    {
        eprintln!("keyvo-app: {error}");
        std::process::exit(1);
    }
}
