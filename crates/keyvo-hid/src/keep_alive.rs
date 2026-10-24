//! Classic HID++ `0x0008 KEEP_ALIVE` (spec §5.3): the timeout range and the
//! keep-alive scheduler. Without pings the Keypad delivers no key input and
//! drops its paints, so the caller polls [`KeepAlive`] from the moment the
//! device opens, before the claim and before any read. The scheduler is
//! driven by a [`Clock`], not by `tokio`; the daemon decides how to wait
//! ([`KeepAlive::until_due`]).

use std::time::{Duration, Instant};

use keyvo_core::clock::Clock;
use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::feature;

/// `getTimeoutRange`.
const FN_GET_TIMEOUT_RANGE: u8 = 0;
/// `keepAlive(timeout)`.
const FN_KEEP_ALIVE: u8 = 1;

/// Default of `[timing].keep_alive_period` (spec §6.1): Options+ pings about
/// every 1.01 s (captured).
pub const DEFAULT_PERIOD: Duration = Duration::from_millis(1000);
/// Default of `[timing].keep_alive_timeout` (spec §6.1): the timeout Options+
/// requests with every ping (captured).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_millis(3000);

/// The `[timing]` values of the keep-alive scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepAliveTiming {
    /// `[timing].keep_alive_period`: time between two pings.
    pub period: Duration,
    /// `[timing].keep_alive_timeout`: how long the device holds the claim
    /// after a ping, sent with every ping.
    pub timeout: Duration,
}

impl Default for KeepAliveTiming {
    fn default() -> Self {
        Self {
            period: DEFAULT_PERIOD,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// Sends `keepAlive(timeout)` once per period.
#[derive(Debug)]
pub struct KeepAlive<C> {
    clock: C,
    timing: KeepAliveTiming,
    next: Instant,
}

impl<C: Clock> KeepAlive<C> {
    /// A scheduler whose first ping is due at once.
    pub fn new(clock: C, timing: KeepAliveTiming) -> Self {
        let next = clock.now();
        Self {
            clock,
            timing,
            next,
        }
    }

    /// Time left until the next ping is due; zero when it is due now.
    pub fn until_due(&self) -> Duration {
        self.next.saturating_duration_since(self.clock.now())
    }

    /// Sends the ping when it is due and returns the timeout the device
    /// granted; `None` when no ping was due. The next ping is due one period
    /// after the previous due time, so the cadence does not drift; after a
    /// stall longer than a period it restarts from now instead of catching
    /// up with a burst.
    ///
    /// # Errors
    ///
    /// [`HidppError::FeatureNotPresent`] on a device without keep-alive, plus
    /// the request errors. A failed ping stays due.
    pub fn poll<T: HidTransport>(
        &mut self,
        hidpp: &mut Hidpp<T>,
    ) -> Result<Option<Duration>, HidppError> {
        let now = self.clock.now();
        if now < self.next {
            return Ok(None);
        }
        let granted = keep_alive(hidpp, self.timing.timeout)?;
        self.next += self.timing.period;
        if self.next <= now {
            self.next = now + self.timing.period;
        }
        Ok(Some(granted))
    }
}

/// One `keepAlive(timeout)` request; returns the timeout the device echoes.
///
/// # Errors
///
/// [`HidppError::Malformed`] when `timeout` does not fit the u16 millisecond
/// field or the reply is short, plus the request errors.
pub fn keep_alive<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    timeout: Duration,
) -> Result<Duration, HidppError> {
    let millis = u16::try_from(timeout.as_millis()).map_err(|_| {
        HidppError::Malformed(format!("keep-alive timeout {timeout:?} exceeds 65535 ms"))
    })?;
    let index = hidpp.feature_index(Plane::Classic, feature::KEEP_ALIVE)?;
    let reply = hidpp.request(Plane::Classic, index, FN_KEEP_ALIVE, &millis.to_be_bytes())?;
    let [hi, lo, ..] = reply.params[..] else {
        return Err(HidppError::Malformed("short keepAlive reply".into()));
    };
    Ok(Duration::from_millis(u16::from_be_bytes([hi, lo]).into()))
}

/// The keep-alive timeouts the device accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeoutRange {
    /// Shortest accepted timeout.
    pub min: Duration,
    /// Longest accepted timeout.
    pub max: Duration,
}

/// Asks the device for its keep-alive timeout range, resolving the
/// `KEEP_ALIVE` index first.
///
/// # Errors
///
/// [`HidppError::FeatureNotPresent`] on a device without keep-alive (the
/// Dialpad), plus the request errors.
pub fn timeout_range<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<TimeoutRange, HidppError> {
    let index = hidpp.feature_index(Plane::Classic, feature::KEEP_ALIVE)?;
    let reply = hidpp.request(Plane::Classic, index, FN_GET_TIMEOUT_RANGE, &[])?;
    let [min_hi, min_lo, max_hi, max_lo, ..] = reply.params[..] else {
        return Err(HidppError::Malformed("short getTimeoutRange reply".into()));
    };
    Ok(TimeoutRange {
        min: Duration::from_millis(u16::from_be_bytes([min_hi, min_lo]).into()),
        max: Duration::from_millis(u16::from_be_bytes([max_hi, max_lo]).into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidpp::{DEVICE_DIRECT, SoftwareId};
    use keyvo_core::clock::FakeClock;
    use keyvo_core::hid::{FakeHidTransport, TransportError};

    const SWID: u8 = 0x1;

    fn long(head: &[u8]) -> Vec<u8> {
        let mut bytes = head.to_vec();
        bytes.resize(20, 0);
        bytes
    }

    /// A `Hidpp` whose `KEEP_ALIVE` index (4) is already resolved, followed by
    /// `pings` echoed keepAlive(3000) exchanges.
    fn hidpp(pings: usize) -> Hidpp<FakeHidTransport> {
        let mut fake = FakeHidTransport::new().expect(
            &long(&[0x11, 0xff, 0x00, 0x01, 0x00, 0x08]),
            &[&long(&[0x11, 0xff, 0x00, 0x01, 0x04, 0x00, 0x01])],
        );
        let ping = long(&[0x11, 0xff, 0x04, 0x11, 0x0b, 0xb8]);
        for _ in 0..pings {
            fake = fake.expect(&ping, &[&ping]);
        }
        Hidpp::new(
            fake,
            DEVICE_DIRECT,
            SoftwareId::new(SWID).unwrap(),
            Duration::from_secs(1),
        )
    }

    #[test]
    fn a_stall_restarts_the_cadence_instead_of_bursting() {
        let mut hidpp = hidpp(2);
        let clock = FakeClock::new();
        let mut keep_alive = KeepAlive::new(&clock, KeepAliveTiming::default());
        assert!(keep_alive.poll(&mut hidpp).unwrap().is_some());

        clock.advance(Duration::from_millis(3500));
        assert!(keep_alive.poll(&mut hidpp).unwrap().is_some());
        // Three missed periods give one ping, then a full period of quiet.
        assert_eq!(keep_alive.poll(&mut hidpp).unwrap(), None);
        assert_eq!(keep_alive.until_due(), DEFAULT_PERIOD);
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn a_failed_ping_stays_due() {
        // The script ends after getFeature: the ping write is unexpected.
        let mut hidpp = hidpp(0);
        let clock = FakeClock::new();
        let mut keep_alive = KeepAlive::new(&clock, KeepAliveTiming::default());
        assert!(matches!(
            keep_alive.poll(&mut hidpp),
            Err(HidppError::Transport(
                TransportError::UnexpectedWrite { .. }
            ))
        ));
        assert_eq!(keep_alive.until_due(), Duration::ZERO);
    }

    #[test]
    fn a_timeout_beyond_the_u16_field_is_rejected_before_writing() {
        let mut hidpp = hidpp(0);
        assert!(matches!(
            keep_alive(&mut hidpp, Duration::from_secs(66)),
            Err(HidppError::Malformed(_))
        ));
        assert!(hidpp.transport_mut().written().is_empty());
    }

    #[test]
    fn default_timing_matches_spec_6_1() {
        assert_eq!(
            KeepAliveTiming::default(),
            KeepAliveTiming {
                period: Duration::from_millis(1000),
                timeout: Duration::from_millis(3000),
            }
        );
    }
}
