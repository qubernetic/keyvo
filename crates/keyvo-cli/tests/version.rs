//! Black-box test of the `keyvo` binary.

use assert_cmd::Command;

#[test]
fn version_flag_prints_crate_version() {
    let expected = format!("keyvo {}\n", env!("CARGO_PKG_VERSION"));

    Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .arg("--version")
        .assert()
        .success()
        .stdout(expected);
}
