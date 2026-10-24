//! Black-box tests of the device commands without a device.

use assert_cmd::Command;

#[test]
fn a_missing_device_node_is_an_error_naming_it() {
    let output = Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .args(["probe", "--device", "/nonexistent/hidraw99"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("cannot open the Keypad") && stderr.contains("/nonexistent/hidraw99"),
        "{stderr}"
    );
}

#[test]
fn a_command_is_required() {
    Command::new(env!("CARGO_BIN_EXE_keyvo")).assert().failure();
}
