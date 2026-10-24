//! A claimed Keypad for the commands that paint or listen: the keep-alive
//! starts with the first ping before anything else (spec §5.3), then the
//! display is claimed (spec §5.4). The keep-alive can be left out for the
//! hardware check (plan §5 items 2 and 3).

use std::time::Duration;

use keyvo_core::clock::Clock;
use keyvo_core::hid::HidTransport;
use keyvo_hid::display::{self, Claim};
use keyvo_hid::hidpp::{Hidpp, HidppError};
use keyvo_hid::keep_alive::{DEFAULT_PERIOD, KeepAlive, KeepAliveTiming};

/// An open, claimed Keypad.
pub struct Session<T, C> {
    /// The HID++ endpoint.
    pub hidpp: Hidpp<T>,
    /// What the claim found: display index, modes, geometry.
    pub claim: Claim,
    keep_alive: Option<KeepAlive<C>>,
}

impl<T: HidTransport, C: Clock> Session<T, C> {
    /// Sends the first keep-alive ping (unless `keep_alive` is `None`), then
    /// claims the display.
    ///
    /// # Errors
    ///
    /// The ping and claim errors.
    pub fn open(
        mut hidpp: Hidpp<T>,
        clock: C,
        keep_alive: Option<KeepAliveTiming>,
    ) -> anyhow::Result<Self> {
        let mut keep_alive = keep_alive.map(|timing| KeepAlive::new(clock, timing));
        if let Some(scheduler) = &mut keep_alive {
            scheduler.poll(&mut hidpp)?;
        }
        let claim = display::claim(&mut hidpp)?;
        Ok(Self {
            hidpp,
            claim,
            keep_alive,
        })
    }

    /// Sends the keep-alive ping when it is due.
    ///
    /// # Errors
    ///
    /// The ping's request errors.
    pub fn tick(&mut self) -> Result<(), HidppError> {
        if let Some(scheduler) = &mut self.keep_alive {
            scheduler.poll(&mut self.hidpp)?;
        }
        Ok(())
    }

    /// How long the caller may wait before the next [`Session::tick`]; one
    /// default period without keep-alive.
    pub fn until_due(&self) -> Duration {
        self.keep_alive
            .as_ref()
            .map_or(DEFAULT_PERIOD, KeepAlive::until_due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{exchanges, fixture, replay};
    use keyvo_core::clock::FakeClock;
    use keyvo_hid::display::DisplayMode;

    #[test]
    fn open_pings_before_the_claim_and_ticks_once_per_period() {
        // getFeature(0x0008) and the first ping, the claim, the second ping.
        let pings = fixture("keep-alive-ping.txt");
        let script = format!(
            "{}{}{}",
            exchanges(&pings, 0..2),
            fixture("display-claim.txt"),
            exchanges(&pings, 2..3),
        );
        let clock = FakeClock::new();
        let mut session =
            Session::open(replay(&script), &clock, Some(KeepAliveTiming::default())).unwrap();
        assert_eq!(session.claim.boot_mode, DisplayMode::STANDBY);
        assert_eq!(session.until_due(), DEFAULT_PERIOD);
        session.tick().unwrap();
        clock.advance(DEFAULT_PERIOD);
        assert_eq!(session.until_due(), Duration::ZERO);
        session.tick().unwrap();
        assert!(session.hidpp.transport_mut().is_done());
    }

    #[test]
    fn without_keep_alive_the_claim_comes_first_and_tick_sends_nothing() {
        let clock = FakeClock::new();
        let mut session =
            Session::open(replay(&fixture("display-claim.txt")), &clock, None).unwrap();
        session.tick().unwrap();
        assert_eq!(session.until_due(), DEFAULT_PERIOD);
        assert!(session.hidpp.transport_mut().is_done());
    }
}
