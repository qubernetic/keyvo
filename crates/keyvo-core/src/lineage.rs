//! The `Lineage` platform trait (spec §6.3): whether the process that owns
//! a window belongs to a launch. The daemon implements it over `/proc` on
//! Linux; tests use [`FakeLineage`], whose pairs the test sets by hand.

use std::collections::BTreeSet;

/// Tells whether a process descends from a launch.
pub trait Lineage {
    /// Whether process `pid` belongs to the launch whose shell is `launch`:
    /// it is in the session the shell leads, or the shell is among its
    /// ancestors.
    fn descends(&self, pid: u32, launch: u32) -> bool;
}

/// A [`Lineage`] that knows no process: every window is foreign to every
/// launch, so only the window class matches a launch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoLineage;

impl Lineage for NoLineage {
    fn descends(&self, _pid: u32, _launch: u32) -> bool {
        false
    }
}

/// A [`Lineage`] of the pairs a test gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FakeLineage {
    pairs: BTreeSet<(u32, u32)>,
}

impl FakeLineage {
    /// A lineage that knows no process yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same lineage, where `pid` descends from the launch `launch`.
    #[must_use]
    pub fn with(mut self, pid: u32, launch: u32) -> Self {
        self.pairs.insert((pid, launch));
        self
    }
}

impl Lineage for FakeLineage {
    fn descends(&self, pid: u32, launch: u32) -> bool {
        self.pairs.contains(&(pid, launch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_knows_its_pairs_alone() {
        let lineage = FakeLineage::new().with(42, 7);
        assert!(lineage.descends(42, 7));
        assert!(!lineage.descends(7, 42));
        assert!(!lineage.descends(42, 8));
        assert!(!NoLineage.descends(42, 7));
    }
}
