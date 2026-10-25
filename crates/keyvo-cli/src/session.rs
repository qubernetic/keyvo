//! A claimed Keypad for the commands that paint or listen: the keep-alive
//! starts with the first ping before anything else (spec §5.3), then the
//! display is claimed (spec §5.4). The keep-alive can be left out for the
//! hardware check (plan §5 items 2 and 3). A claim lost to a keep-alive
//! expiry is taken back as soon as its event is read (spec §5.3).

use std::time::Duration;

use keyvo_core::clock::Clock;
use keyvo_core::hid::HidTransport;
use keyvo_hid::display::{self, Claim, ClaimError, ClaimWatch};
use keyvo_hid::frame::Frame;
use keyvo_hid::hidpp::{Hidpp, HidppError, Plane};
use keyvo_hid::keep_alive::{DEFAULT_PERIOD, KeepAlive, KeepAliveTiming};
use keyvo_hid::root::feature;

/// What [`Session::next_frame`] read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inbound {
    /// A report that is not about the claim.
    Frame(Frame),
    /// The claim was lost and is taken back; what was painted is gone.
    Reclaimed,
}

/// An open, claimed Keypad.
pub struct Session<T, C> {
    /// The HID++ endpoint.
    pub hidpp: Hidpp<T>,
    /// What the claim found: display index, modes, geometry.
    pub claim: Claim,
    keep_alive: Option<KeepAlive<C>>,
    watch: ClaimWatch,
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
        let mut watch = ClaimWatch::new(hidpp.device_index(), claim.display_index);
        if keep_alive.is_some() {
            // Resolved by the first ping; no request.
            watch =
                watch.with_keep_alive(hidpp.feature_index(Plane::Classic, feature::KEEP_ALIVE)?);
        }
        Ok(Self {
            hidpp,
            claim,
            keep_alive,
            watch,
        })
    }

    /// The next inbound report, waiting at most `wait`; a report of a lost
    /// claim is answered with a reclaim and comes back as
    /// [`Inbound::Reclaimed`], so the caller repaints.
    ///
    /// # Errors
    ///
    /// The transport errors and the reclaim's errors.
    pub fn next_frame(&mut self, wait: Duration) -> Result<Option<Inbound>, ClaimError> {
        let Some(frame) = self.hidpp.next_frame(wait)? else {
            return Ok(None);
        };
        if self.watch.reclaim_on(&mut self.hidpp, &frame)? {
            return Ok(Some(Inbound::Reclaimed));
        }
        Ok(Some(Inbound::Frame(frame)))
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
    use keyvo_core::hid::FakeHidTransport;
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

    /// The claim with keep-alive, then the events `lost`, then the reclaim.
    fn lost(lost: &str) -> Session<FakeHidTransport, FakeClock> {
        let script = format!(
            "{}{}{lost}{}",
            exchanges(&fixture("keep-alive-ping.txt"), 0..2),
            fixture("display-claim.txt"),
            fixture("display-reclaim.txt"),
        );
        Session::open(
            replay(&script),
            FakeClock::new(),
            Some(KeepAliveTiming::default()),
        )
        .unwrap()
    }

    fn set_modes(session: &mut Session<FakeHidTransport, FakeClock>) -> usize {
        let set_mode = [0x13, 0xff, 0x02, 0x4e, 0xe0, 0xa0];
        let written = session.hidpp.transport_mut().written();
        written.iter().filter(|r| r.starts_with(&set_mode)).count()
    }

    #[test]
    fn the_two_events_of_one_expiry_give_one_reclaim() {
        let mut session = lost(&fixture("claim-lost.txt"));
        assert_eq!(set_modes(&mut session), 1);
        assert_eq!(
            session.next_frame(Duration::ZERO).unwrap(),
            Some(Inbound::Reclaimed)
        );
        // The twin event and the a0 announcement are gone.
        assert_eq!(session.next_frame(Duration::ZERO).unwrap(), None);
        assert_eq!(set_modes(&mut session), 2);
        assert!(session.hidpp.transport_mut().is_done());
    }

    #[test]
    fn each_event_alone_gives_one_reclaim() {
        let events: Vec<_> = fixture("claim-lost.txt")
            .lines()
            .filter(|line| line.starts_with('<'))
            .map(|line| format!("{line}\n"))
            .collect();
        for event in events {
            let mut session = lost(&event);
            assert_eq!(
                session.next_frame(Duration::ZERO).unwrap(),
                Some(Inbound::Reclaimed),
                "{event}"
            );
            assert_eq!(session.next_frame(Duration::ZERO).unwrap(), None);
            assert_eq!(set_modes(&mut session), 2, "{event}");
            assert!(session.hidpp.transport_mut().is_done());
        }
    }

    #[test]
    fn other_reports_come_through_as_frames() {
        let key = "< 13 ff 02 00 c0 01 05 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00\n";
        let script = format!("{}{key}", fixture("display-claim.txt"));
        let mut session = Session::open(replay(&script), FakeClock::new(), None).unwrap();
        match session.next_frame(Duration::ZERO).unwrap() {
            Some(Inbound::Frame(frame)) => assert_eq!(frame.params[..2], [0x01, 0x05]),
            other => panic!("{other:?}"),
        }
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
