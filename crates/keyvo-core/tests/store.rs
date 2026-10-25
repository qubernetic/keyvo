//! Writing profile files (spec §7.1, `docs/design/gui-ux.md` §4.1): the
//! atomic replace and the snapshots taken before it.

use std::path::Path;
use std::time::{Duration, SystemTime};

use keyvo_core::store::{self, SNAPSHOTS_KEPT};

fn at(ms: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn write_replaces_the_file_and_leaves_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.toml");
    std::fs::write(&path, "old").unwrap();
    store::write(&path, "new").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(names(dir.path()), ["global.toml"]);
}

#[test]
fn write_creates_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.toml");
    store::write(&path, "new").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[cfg(unix)]
#[test]
fn write_keeps_the_mode_of_the_file() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.toml");
    std::fs::write(&path, "old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    store::write(&path, "new").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn write_through_a_symlink_replaces_its_target_and_keeps_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("dotfiles/global.toml");
    std::fs::create_dir(dir.path().join("dotfiles")).unwrap();
    std::fs::write(&target, "old").unwrap();
    let link = dir.path().join("global.toml");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    store::write(&link, "new").unwrap();
    assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
}

#[test]
fn write_into_a_missing_directory_fails() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("none/global.toml");
    assert!(store::write(&path, "new").is_err());
}

#[test]
fn snapshot_copies_the_file_under_its_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.toml");
    std::fs::write(&path, "before").unwrap();
    let snapshots = dir.path().join("state/snapshots");
    let copy = store::snapshot(&path, &snapshots, at(1_761_387_300_123), SNAPSHOTS_KEPT)
        .unwrap()
        .unwrap();
    assert_eq!(copy, snapshots.join("1761387300123-global.toml"));
    assert_eq!(std::fs::read_to_string(&copy).unwrap(), "before");
}

#[test]
fn snapshot_of_a_missing_file_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let snapshots = dir.path().join("snapshots");
    let copy = store::snapshot(&dir.path().join("global.toml"), &snapshots, at(1), 50).unwrap();
    assert_eq!(copy, None);
}

#[test]
fn snapshots_in_the_same_millisecond_do_not_overwrite_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.toml");
    let snapshots = dir.path().join("snapshots");
    std::fs::write(&path, "one").unwrap();
    store::snapshot(&path, &snapshots, at(5), 50).unwrap();
    std::fs::write(&path, "two").unwrap();
    let second = store::snapshot(&path, &snapshots, at(5), 50)
        .unwrap()
        .unwrap();
    assert_eq!(second, snapshots.join("0000000000006-global.toml"));
    assert_eq!(
        names(&snapshots),
        ["0000000000005-global.toml", "0000000000006-global.toml"]
    );
}

#[test]
fn snapshots_rotate_per_file_keeping_the_newest() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global.toml");
    let other = dir.path().join("konsole.toml");
    let snapshots = dir.path().join("snapshots");
    std::fs::write(&global, "g").unwrap();
    std::fs::write(&other, "k").unwrap();
    store::snapshot(&other, &snapshots, at(1), 2).unwrap();
    std::fs::create_dir_all(&snapshots).unwrap();
    std::fs::write(snapshots.join("notes.txt"), "kept").unwrap();
    for ms in [10, 20, 30, 40] {
        store::snapshot(&global, &snapshots, at(ms), 2).unwrap();
    }
    assert_eq!(
        names(&snapshots),
        [
            "0000000000001-konsole.toml",
            "0000000000030-global.toml",
            "0000000000040-global.toml",
            "notes.txt",
        ]
    );
}
