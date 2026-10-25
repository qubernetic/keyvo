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

/// The control socket: `--socket` (`flag`), else `$KEYVO_SOCKET` when set
/// and not empty, else `keyvo.sock` in the runtime directory `dir`. The
/// order of `keyvo ctl`, `keyvo-client` and the app (spec §8), so a client
/// with only the variable set finds the daemon.
#[must_use]
pub fn socket(flag: Option<PathBuf>, keyvo_socket: Option<&OsStr>, dir: &Path) -> PathBuf {
    flag.or_else(|| {
        keyvo_socket
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    })
    .unwrap_or_else(|| dir.join(keyvo_core::socket::SOCKET_FILE))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_is_the_flag_then_the_variable_then_the_default() {
        let run = Path::new("/run/user/1000/keyvo");
        let variable = Some(OsStr::new("/tmp/other.sock"));
        assert_eq!(
            socket(Some("/flag.sock".into()), variable, run),
            PathBuf::from("/flag.sock")
        );
        assert_eq!(
            socket(None, variable, run),
            PathBuf::from("/tmp/other.sock")
        );
        assert_eq!(
            socket(None, None, run),
            PathBuf::from("/run/user/1000/keyvo/keyvo.sock")
        );
        // An empty variable counts as unset, as in `keyvo_client::socket_path`.
        assert_eq!(
            socket(None, Some(OsStr::new("")), run),
            PathBuf::from("/run/user/1000/keyvo/keyvo.sock")
        );
    }
}
