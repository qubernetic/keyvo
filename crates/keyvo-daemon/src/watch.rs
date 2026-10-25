//! Hot-reload source (spec §7.2): inotify on the configuration directory
//! and its `profiles/` directory. Directories are watched, not files, so an
//! editor's write-to-temp-and-rename is seen like an in-place write; a
//! `profiles/` directory created later is picked up when it appears.
//!
//! The watcher only says "something changed"; the controller waits
//! `[timing].reload_settle` for the burst to end and reloads everything.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use inotify::{EventMask, Inotify, WatchDescriptor, WatchMask};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// The subdirectory with the profile files.
const PROFILES: &str = "profiles";

/// Changes that can alter what `layout::load` reads.
fn mask() -> WatchMask {
    WatchMask::CREATE
        | WatchMask::DELETE
        | WatchMask::CLOSE_WRITE
        | WatchMask::MODIFY
        | WatchMask::MOVED_FROM
        | WatchMask::MOVED_TO
}

/// Starts watching `dir`; one message per change until the receiver is
/// dropped. Must be called inside the tokio runtime.
///
/// # Errors
///
/// When inotify cannot start or `dir` cannot be watched (it does not
/// exist, for example).
pub fn watch(dir: &Path) -> io::Result<UnboundedReceiver<()>> {
    let inotify = Inotify::init()?;
    let root = inotify.watches().add(dir, mask())?;
    let profiles = dir.join(PROFILES);
    if profiles.is_dir() {
        inotify.watches().add(&profiles, mask())?;
    }
    let stream = inotify.into_event_stream([0u8; 4096])?;
    let (changes, receiver) = mpsc::unbounded_channel();
    tokio::spawn(forward(stream, root, profiles, changes));
    Ok(receiver)
}

async fn forward(
    mut stream: inotify::EventStream<[u8; 4096]>,
    root: WatchDescriptor,
    profiles: PathBuf,
    changes: UnboundedSender<()>,
) {
    while let Some(Ok(event)) = stream.next().await {
        let appeared = event.wd == root
            && event.mask.contains(EventMask::ISDIR)
            && event
                .mask
                .intersects(EventMask::CREATE | EventMask::MOVED_TO)
            && event.name.as_deref() == Some(OsStr::new(PROFILES));
        if appeared && let Err(error) = stream.watches().add(&profiles, mask()) {
            tracing::warn!("cannot watch {}: {error}", profiles.display());
        }
        if changes.send(()).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const PATIENCE: Duration = Duration::from_secs(10);

    async fn changed(receiver: &mut UnboundedReceiver<()>) {
        tokio::time::timeout(PATIENCE, receiver.recv())
            .await
            .expect("a change")
            .expect("an open channel");
    }

    fn quiet(receiver: &mut UnboundedReceiver<()>) {
        while receiver.try_recv().is_ok() {}
    }

    #[tokio::test]
    async fn a_new_profile_and_a_renamed_config_are_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(PROFILES)).unwrap();
        let mut changes = watch(dir.path()).unwrap();
        std::fs::write(dir.path().join("profiles/global.toml"), "x").unwrap();
        changed(&mut changes).await;
        quiet(&mut changes);
        std::fs::write(dir.path().join(".config.toml.tmp"), "y").unwrap();
        std::fs::rename(
            dir.path().join(".config.toml.tmp"),
            dir.path().join("config.toml"),
        )
        .unwrap();
        changed(&mut changes).await;
    }

    #[tokio::test]
    async fn a_profiles_directory_created_later_is_watched() {
        let dir = tempfile::tempdir().unwrap();
        let mut changes = watch(dir.path()).unwrap();
        std::fs::create_dir(dir.path().join(PROFILES)).unwrap();
        changed(&mut changes).await;
        // Let the watcher add the new directory before writing into it.
        tokio::time::sleep(Duration::from_millis(50)).await;
        quiet(&mut changes);
        std::fs::write(dir.path().join("profiles/global.toml"), "x").unwrap();
        changed(&mut changes).await;
    }

    #[tokio::test]
    async fn a_missing_directory_cannot_be_watched() {
        let dir = tempfile::tempdir().unwrap();
        assert!(watch(&dir.path().join("absent")).is_err());
    }
}
