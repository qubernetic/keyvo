//! The keyvo daemon (spec §4): a tokio controller that keeps the Keypad
//! painted with the active profile, runs the actions of its presses and
//! reloads the configuration when it changes. `keyvo-core` stays
//! synchronous; the runtime, the processes and every OS binding live here.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod daemon;
pub mod device;
pub mod engine;
pub mod exec;
#[cfg(target_os = "linux")]
pub mod kwin;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub mod watch;
