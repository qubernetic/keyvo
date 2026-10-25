//! Writing profile files (spec §7.1, `docs/design/gui-ux.md` §4.1): a
//! snapshot of the file first, then the new text through a temporary file
//! renamed over the old one, so a reader (the daemon's reload) sees the
//! old file or the new one, never half of it.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The snapshots kept per profile file; older ones are deleted.
pub const SNAPSHOTS_KEPT: usize = 50;

/// Replaces the file at `path` with `text` atomically: written to a
/// temporary file beside it, flushed to disk, renamed over it. The file
/// keeps its permissions; a symlink keeps pointing at its target, which is
/// the file replaced, so a profile linked from a dotfile repository stays
/// linked.
///
/// # Errors
///
/// When the temporary file cannot be written or renamed; the old file is
/// then unchanged and the temporary file removed.
pub fn write(path: &Path, text: &str) -> io::Result<()> {
    let target = match fs::canonicalize(path) {
        Ok(target) => target,
        Err(e) if e.kind() == io::ErrorKind::NotFound => path.to_path_buf(),
        Err(e) => return Err(e),
    };
    let name = target.file_name().unwrap_or(OsStr::new("profile"));
    let mut temp_name = OsStr::new(".").to_os_string();
    temp_name.push(name);
    temp_name.push(format!(".keyvo-{}.tmp", std::process::id()));
    let temp = target.with_file_name(temp_name);
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        if let Ok(meta) = fs::metadata(&target) {
            file.set_permissions(meta.permissions())?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, &target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Copies the file at `path` into `dir` as `<ms>-<file>`, `<ms>` the
/// milliseconds since the Unix epoch at `now`, 13 digits, one more for
/// every snapshot already taken in that millisecond; then deletes the
/// oldest snapshots of that file beyond `keep`. `None` when there is no
/// file to copy. `dir` is created when missing.
///
/// # Errors
///
/// When `dir` cannot be created or listed, or the copy fails.
pub fn snapshot(
    path: &Path,
    dir: &Path,
    now: SystemTime,
    keep: usize,
) -> io::Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let file = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    fs::create_dir_all(dir)?;
    let mut ms = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let copy = loop {
        let copy = dir.join(format!("{ms:013}-{file}"));
        if !copy.exists() {
            break copy;
        }
        ms += 1;
    };
    fs::copy(path, &copy)?;
    rotate(dir, file, keep)?;
    Ok(Some(copy))
}

/// Deletes the oldest snapshots of `file` in `dir` beyond `keep`.
fn rotate(dir: &Path, file: &str, keep: usize) -> io::Result<()> {
    let suffix = format!("-{file}");
    let mut snapshots = Vec::new();
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        let Some(name) = name.to_str() else { continue };
        let ours = name
            .strip_suffix(&suffix)
            .is_some_and(|ms| !ms.is_empty() && ms.bytes().all(|b| b.is_ascii_digit()));
        if ours {
            snapshots.push(name.to_owned());
        }
    }
    snapshots.sort();
    let excess = snapshots.len().saturating_sub(keep);
    for name in &snapshots[..excess] {
        fs::remove_file(dir.join(name))?;
    }
    Ok(())
}
