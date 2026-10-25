//! The daemon's runtime directory (`keyvo_core::layout::runtime_dir`): the
//! socket and the `KWin` script live there, so it must be this user's
//! alone.

use std::ffi::OsStr;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

/// `keyvo` under `$XDG_RUNTIME_DIR`, else `keyvo-<uid>` in the temporary
/// directory.
#[must_use]
pub fn dir(xdg_runtime_dir: Option<&OsStr>) -> PathBuf {
    keyvo_core::layout::runtime_dir(
        xdg_runtime_dir,
        &std::env::temp_dir(),
        rustix::process::getuid().as_raw(),
    )
}

/// Creates `dir` with mode `0700` when it is missing.
///
/// # Errors
///
/// When it cannot be created, or exists but is not a directory owned by
/// this user that nobody else may enter.
pub fn ensure_private(dir: &Path) -> io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir()
        || meta.uid() != rustix::process::getuid().as_raw()
        || meta.mode() & 0o077 != 0
    {
        return Err(io::Error::other(format!(
            "{} is not private to this user",
            dir.display()
        )));
    }
    Ok(())
}
