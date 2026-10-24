//! The `Clock` trait: monotonic time for the timing logic (keep-alive cadence,
//! long press, hysteresis), so it runs without `tokio` and is tested with
//! [`FakeClock`] instead of sleeping.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// A monotonic time source.
pub trait Clock {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// The real monotonic clock ([`Instant::now`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A clock that stands still until a test advances it.
#[derive(Debug)]
pub struct FakeClock {
    start: Instant,
    elapsed: Cell<Duration>,
}

impl FakeClock {
    /// A clock frozen at the moment of the call.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            elapsed: Cell::new(Duration::ZERO),
        }
    }

    /// Moves the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        self.elapsed.set(self.elapsed.get() + by);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.start + self.elapsed.get()
    }
}

impl<C: Clock + ?Sized> Clock for &C {
    fn now(&self) -> Instant {
        (**self).now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_does_not_go_backwards() {
        let first = SystemClock.now();
        assert!(SystemClock.now() >= first);
    }

    #[test]
    fn fake_clock_moves_only_when_advanced() {
        let clock = FakeClock::new();
        let start = clock.now();
        assert_eq!(clock.now(), start);
        clock.advance(Duration::from_millis(250));
        assert_eq!(clock.now() - start, Duration::from_millis(250));
    }
}
