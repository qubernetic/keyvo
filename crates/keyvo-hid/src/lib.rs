//! HID transport, HID++ 2.0 core and the VLP display plane for the Logitech
//! MX Creative Keypad and Dialpad. `unsafe` is allowed here for the hidapi
//! FFI that lands later; `unsafe_op_in_unsafe_fn` is denied workspace-wide.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod display;
pub mod dump;
pub mod firmware;
pub mod frame;
pub mod hidpp;
#[cfg(target_os = "linux")]
pub mod hidraw;
pub mod image;
pub mod input;
pub mod keep_alive;
pub mod multiroller;
pub mod reprog;
pub mod root;
pub mod sysfs;
