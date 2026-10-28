//! The [`Lineage`] over `/proc` (spec §6.3): a window's process belongs to
//! a launch when it is in the session the launch's shell leads (keyvo
//! starts every launch with `setsid`, and the `flatpak` to `bwrap` to
//! application chain keeps the session) or when the shell is among its
//! ancestors. `KWin` reports the host pid of a window's client, also for a
//! Flatpak application inside its own pid namespace.

use std::path::PathBuf;

use keyvo_core::lineage::Lineage;

/// How many parents the ancestor walk follows at most; a process tree is
/// rarely more than a few levels deep between a shell and its window.
const MAX_DEPTH: usize = 64;

/// The process tree as `/proc` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcLineage {
    root: PathBuf,
}

impl Default for ProcLineage {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcLineage {
    /// The lineage of the system's `/proc`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: PathBuf::from("/proc"),
        }
    }

    /// The fields of `/proc/<pid>/stat` after the command name, which may
    /// hold spaces and parentheses itself: the state first, then the
    /// parent, the process group and the session.
    fn stat(&self, pid: u32) -> Option<Vec<String>> {
        let text = std::fs::read_to_string(self.root.join(pid.to_string()).join("stat")).ok()?;
        let (_, rest) = text.rsplit_once(") ")?;
        Some(rest.split(' ').map(str::to_owned).collect())
    }

    fn session(&self, pid: u32) -> Option<u32> {
        self.stat(pid)?.get(3)?.parse().ok()
    }

    fn parent(&self, pid: u32) -> Option<u32> {
        self.stat(pid)?.get(1)?.parse().ok()
    }
}

impl Lineage for ProcLineage {
    fn descends(&self, pid: u32, launch: u32) -> bool {
        if self.session(pid) == Some(launch) {
            return true;
        }
        let mut at = pid;
        for _ in 0..MAX_DEPTH {
            if at == launch {
                return true;
            }
            match self.parent(at) {
                Some(parent) if parent != 0 && parent != at => at = parent,
                _ => return false,
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::exec::Launch;

    #[test]
    fn the_test_process_descends_from_its_parent_and_its_session() {
        let lineage = ProcLineage::new();
        let me = std::process::id();
        let parent = lineage.parent(me).unwrap();
        let session = lineage.session(me).unwrap();
        assert!(lineage.descends(me, me));
        assert!(lineage.descends(me, parent), "{me} under {parent}");
        assert!(lineage.descends(me, session), "{me} in {session}");
        // Not the other way round, and not a process that is gone.
        assert!(!lineage.descends(parent, me));
        assert!(!lineage.descends(u32::MAX, me));
        assert!(!lineage.descends(me, u32::MAX));
    }

    #[tokio::test]
    async fn a_launch_leads_the_session_of_what_it_starts() {
        let lineage = ProcLineage::default();
        let launch = Launch::spawn("sleep 5 & sleep 5; wait", &[]).unwrap();
        let shell = launch.pid().unwrap();
        // The shell's children appear once it has forked them.
        let children = || {
            std::fs::read_to_string(format!("/proc/{shell}/task/{shell}/children"))
                .unwrap_or_default()
                .split_whitespace()
                .filter_map(|pid| pid.parse::<u32>().ok())
                .collect::<Vec<_>>()
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while children().is_empty() {
            assert!(std::time::Instant::now() < deadline, "no child of {shell}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        for child in children() {
            assert!(lineage.descends(child, shell), "{child} of {shell}");
        }
        assert!(lineage.descends(shell, shell));
        // The test process is outside the launch's session and above it.
        assert!(!lineage.descends(std::process::id(), shell));
        let killed = std::process::Command::new("sh")
            .args(["-c", &format!("kill -9 -{shell}")])
            .status()
            .unwrap();
        assert!(killed.success());
        let _ = launch.settle(Duration::from_secs(5)).await;
    }
}
