//! The client side of the keyvo socket (spec §8, `docs/socket-api.md`),
//! shared by the desktop app (`app/src-tauri`) and later clients.
//!
//! Like `keyvo-core` it is synchronous and hardware-free: [`Client`] speaks
//! the protocol over any `Read + Write` stream, [`Mirror`] folds the
//! daemon's answers and events into what the pad shows, and [`Tiles`]
//! renders a page with the same `keyvo-core` renderer the daemon paints the
//! Keypad with.

pub mod client;

pub use client::{Client, ClientError};
