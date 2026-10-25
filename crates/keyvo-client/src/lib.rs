//! The client side of the keyvo socket (spec §8, `docs/socket-api.md`),
//! shared by the desktop app (`app/src-tauri`) and later clients.
//!
//! Like `keyvo-core` it is synchronous and hardware-free: [`Client`] speaks
//! the protocol over any `Read + Write` stream, [`Mirror`] folds the
//! daemon's answers and events into what the pad shows, and [`Tiles`]
//! renders a page with the same `keyvo-core` renderer the daemon paints the
//! Keypad with.

pub mod client;
pub mod editor;
pub mod follow;
pub mod mirror;
pub mod tiles;

use std::ffi::OsStr;
use std::path::PathBuf;

pub use client::{Client, ClientError};
pub use editor::{Editor, EditorError, Problem};
pub use follow::{Update, follow};
pub use mirror::Mirror;
pub use tiles::{TileError, Tiles};

/// The daemon's socket: `$KEYVO_SOCKET` when set, else the default in the
/// runtime directory (`keyvo_core::layout::runtime_dir`) of the user `uid`.
#[must_use]
pub fn socket_path(
    keyvo_socket: Option<&OsStr>,
    xdg_runtime_dir: Option<&OsStr>,
    uid: u32,
) -> PathBuf {
    keyvo_socket.filter(|path| !path.is_empty()).map_or_else(
        || {
            keyvo_core::layout::runtime_dir(xdg_runtime_dir, &std::env::temp_dir(), uid)
                .join(keyvo_core::socket::SOCKET_FILE)
        },
        PathBuf::from,
    )
}
