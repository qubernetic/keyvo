//! Where a client looks for the daemon's socket.

use std::ffi::OsStr;
use std::path::PathBuf;

use keyvo_client::socket_path;

#[test]
fn keyvo_socket_wins_then_the_runtime_directory() {
    let os = |s: &'static str| Some(OsStr::new(s));
    let run = os("/run/user/1000");
    assert_eq!(
        socket_path(os("/b.sock"), run, 1000),
        PathBuf::from("/b.sock")
    );
    assert_eq!(
        socket_path(os(""), run, 1000),
        PathBuf::from("/run/user/1000/keyvo/keyvo.sock")
    );
    assert_eq!(
        socket_path(None, None, 1000),
        std::env::temp_dir().join("keyvo-1000/keyvo.sock")
    );
}
