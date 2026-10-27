//! Black-box test of `keyvo preview`: no device, the profile from
//! `$XDG_CONFIG_HOME/keyvo`.

use std::path::Path;

use assert_cmd::Command;

#[test]
fn preview_renders_a_configured_profile_without_a_device() {
    let config = tempfile::tempdir().unwrap();
    let profiles = config.path().join("keyvo/profiles");
    std::fs::create_dir_all(&profiles).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../keyvo-core/tests/fixtures/profiles/valid/demo.toml"),
        profiles.join("demo.toml"),
    )
    .unwrap();
    let output = config.path().join("grid.png");

    Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .args([
            "--device", "/absent", "preview", "demo", "--page", "2", "-o",
        ])
        .arg(&output)
        .env("XDG_CONFIG_HOME", config.path())
        .assert()
        .success()
        .stdout(format!(
            "demo page 2 of 2: 480x480, 9 keys -> {}\n",
            output.display()
        ));
    let png = std::fs::read(&output).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
}

#[test]
fn preview_of_an_unknown_profile_fails() {
    let config = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .args(["preview", "nope", "-o", "grid.png"])
        .env("XDG_CONFIG_HOME", config.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no profile `nope`"), "{stderr}");
}
