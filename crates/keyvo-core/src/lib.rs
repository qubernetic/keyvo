//! Hardware-free core of keyvo: profiles, state machine, renderer and
//! protocol types. Synchronous by design; `tokio` lives in `keyvo-daemon`.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod clock;
pub mod diagnostic;
pub mod hid;
pub mod profile;

/// Version of the keyvo workspace, taken from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_cargo_pkg_version() {
        assert_eq!(super::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
