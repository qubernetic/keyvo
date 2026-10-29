//! `keyvo dial`: finds the Dialpad (spec §5.8), diverts its buttons and
//! rollers and streams one line per event, as text or as NDJSON (`--json`),
//! until it is stopped; then it gives the diverts back.

use std::io::Write;
use std::time::Duration;

use keyvo_core::config::Timing;
use keyvo_core::hid::HidTransport;
use keyvo_hid::dialpad::{self, Dialpad, DialpadError, DialpadEvent, DialpadTiming, Link};
use keyvo_hid::hidpp::Hidpp;
use keyvo_hid::input::{Control, Edge, InputEvent};
use keyvo_hid::reprog::DivertTiming;

/// The names of the rollers by id: the third-party reading of roller 0 as
/// the small wheel and roller 1 as the large dial (spec §5.8); the
/// physical mapping is measured in M3-2.
const ROLLER_NAMES: [&str; 2] = ["roller", "dial"];

/// The Dialpad's timing out of the user's `[timing]`.
#[must_use]
pub const fn dialpad_timing(timing: &Timing) -> DialpadTiming {
    DialpadTiming {
        slot_probe: timing.slot_probe,
        divert: DivertTiming {
            settle: timing.divert_settle,
            spacing: timing.divert_spacing,
        },
    }
}

/// Opens the Dialpad on `link` and writes every event to `out` until
/// `keep_going` says stop; it is asked whenever a read found nothing (a
/// read waits at most `[timing].reply_timeout`). Then the diverts are given
/// back. On a receiver without an awake Dialpad it says so once on `log`
/// and asks again on the next arrival, or after
/// `[timing].reconnect_interval`. The link's changes go to `log`.
///
/// # Errors
///
/// The driver's errors (a disconnect ends the stream) and write errors.
#[allow(clippy::too_many_arguments)]
pub fn dial<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    link: Link,
    timing: &Timing,
    mut sleep: impl FnMut(Duration),
    json: bool,
    out: &mut impl Write,
    log: &mut impl Write,
    mut keep_going: impl FnMut() -> bool,
) -> anyhow::Result<()> {
    let driver = dialpad_timing(timing);
    let mut target = link;
    let mut reported = false;
    let mut dialpad = loop {
        match Dialpad::open(hidpp, target, &driver, &mut sleep) {
            Ok(dialpad) => break dialpad,
            Err(DialpadError::NotFound(slots)) if link != Link::Direct => {
                if !reported {
                    writeln!(log, "no Dialpad answers ({slots}); waiting for it")?;
                    reported = true;
                }
                if !keep_going() {
                    return Ok(());
                }
                target =
                    dialpad::wait_for_arrival(hidpp, &slots.silent(), timing.reconnect_interval)?
                        .map_or(link, Link::Slot);
            }
            Err(error) => return Err(error.into()),
        }
    };
    writeln!(
        log,
        "Dialpad on {}: {} buttons, {}; Ctrl-C to stop",
        place(&dialpad),
        dialpad.buttons().len(),
        rollers(&dialpad)
    )?;
    loop {
        match dialpad.next_event(hidpp, timing.reply_timeout, &mut sleep)? {
            Some(DialpadEvent::Input(event)) => {
                writeln!(out, "{}", format_event(&dialpad, event, json))?;
            }
            Some(DialpadEvent::LinkLost) => {
                writeln!(log, "Dialpad link lost (asleep or out of range)")?;
            }
            Some(DialpadEvent::Rediverted) => writeln!(log, "Dialpad back, diverts sent again")?,
            None if keep_going() => {}
            None => break,
        }
    }
    dialpad.restore(hidpp, sleep)?;
    writeln!(log, "diverts given back")?;
    Ok(())
}

/// `slot 3` or `device index 0xff`.
pub fn place(dialpad: &Dialpad) -> String {
    match dialpad.link() {
        Link::Direct => format!("device index 0x{:02x}", dialpad.device_index()),
        Link::Receiver | Link::Slot(_) => format!("slot {}", dialpad.device_index()),
    }
}

/// `dial 180 and roller 40 increments per rotation`.
fn rollers(dialpad: &Dialpad) -> String {
    dialpad
        .rollers()
        .iter()
        .map(|roller| format!("{} {}", roller_name(roller.id), roller.increments))
        .collect::<Vec<_>>()
        .join(" and ")
        + " increments per rotation"
}

/// `dial`, `roller`, or `roller 2` past the two known ones.
pub fn roller_name(id: u8) -> String {
    ROLLER_NAMES
        .get(usize::from(id))
        .map_or_else(|| format!("roller {id}"), |name| (*name).to_owned())
}

/// `button 1 press`, `dial +3`, or the NDJSON form
/// `{"edge":"press","button":1}`, `{"turn":"dial","delta":3}`.
fn format_event(dialpad: &Dialpad, event: InputEvent, json: bool) -> String {
    let (control, edge) = match event {
        InputEvent::Rotate { roller, delta } => {
            let name = roller_name(roller);
            return if json {
                format!(r#"{{"turn":"{name}","delta":{delta}}}"#)
            } else {
                format!("{name} {delta:+}")
            };
        }
        InputEvent::Button { control, edge } => (control, edge),
    };
    let edge = match edge {
        Edge::Press => "press",
        Edge::Release => "release",
    };
    match control {
        Control::Page(cid) => match (dialpad.button(cid), json) {
            (Some(n), false) => format!("button {n} {edge}"),
            (Some(n), true) => format!(r#"{{"edge":"{edge}","button":{n}}}"#),
            (None, false) => format!("control 0x{cid:04x} {edge}"),
            (None, true) => format!(r#"{{"edge":"{edge}","cid":{cid}}}"#),
        },
        Control::Key(key) if json => format!(r#"{{"edge":"{edge}","key":{key}}}"#),
        Control::Key(key) => format!("key {key} {edge}"),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tests::{exchanges, fixture, replay};

    /// Open, the input fixture, a lost link and the arrival with the
    /// re-divert, then the restore on stop.
    pub(crate) fn script() -> String {
        [
            fixture("receiver-slot-probe.txt"),
            fixture("receiver-controls.txt"),
            fixture("receiver-multiroller.txt"),
            fixture("receiver-divert.txt"),
            fixture("receiver-input.txt"),
            fixture("receiver-arrival.txt"),
            fixture("receiver-divert.txt"),
            fixture("receiver-restore.txt"),
        ]
        .concat()
    }

    /// `dial` on `script`: stdout and the log.
    pub(crate) fn run(script: &str, link: Link, json: bool, idle: usize) -> (String, String) {
        let mut hidpp = replay(script);
        let (mut out, mut log) = (Vec::new(), Vec::new());
        let mut asked = 0;
        dial(
            &mut hidpp,
            link,
            &Timing::default(),
            |_| {},
            json,
            &mut out,
            &mut log,
            || {
                asked += 1;
                asked < idle
            },
        )
        .unwrap();
        assert!(hidpp.transport_mut().is_done(), "script not used up");
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(log).unwrap(),
        )
    }

    #[test]
    fn dial_streams_the_fixture_events_as_ndjson() {
        let (out, log) = run(&script(), Link::Receiver, true, 1);
        insta::assert_snapshot!("dial-json", out);
        insta::assert_snapshot!("dial-log", log);
    }

    #[test]
    fn dial_streams_the_fixture_events_as_text() {
        let (out, _) = run(&script(), Link::Receiver, false, 1);
        insta::assert_snapshot!("dial-text", out);
    }

    #[test]
    fn dial_waits_for_a_sleeping_dialpad_and_says_so_once() {
        let open = script();
        let from_slot_3 = exchanges(&fixture("receiver-slot-probe.txt"), 2..5)
            + &open[open.find("# The Dialpad's 0x1B04").unwrap()..];
        let script = fixture("receiver-asleep.txt") + &from_slot_3;
        let (out, log) = run(&script, Link::Receiver, false, 2);
        assert!(out.starts_with("dial +3\n"), "{out}");
        assert!(
            log.starts_with(
                "no Dialpad answers (1 empty, 2 empty, 3 silent, 4 empty, 5 empty, 6 empty); \
                 waiting for it\nDialpad on slot 3:"
            ),
            "{log}"
        );
    }

    #[test]
    fn dial_stops_while_waiting_when_asked() {
        let asleep = fixture("receiver-asleep.txt").replace("< 10 03 41 10 00 00 bc\n", "");
        let (out, log) = run(&asleep, Link::Receiver, false, 0);
        assert_eq!(out, "");
        assert_eq!(log.lines().count(), 1, "{log}");
    }

    #[test]
    fn a_bluetooth_dialpad_without_0x4610_is_an_error() {
        let script = "> 11 ff 00 0f 46 10 00 00 00 00 00 00 00 00 00 00 00 00 00 00\n\
                      < 11 ff 00 0f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00\n";
        let mut hidpp = replay(script);
        let error = dial(
            &mut hidpp,
            Link::Direct,
            &Timing::default(),
            |_| {},
            false,
            &mut Vec::new(),
            &mut Vec::new(),
            || true,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "no MX Creative Dialpad found (slots: direct no 0x4610)"
        );
    }

    #[test]
    fn rollers_past_the_known_two_are_numbered() {
        assert_eq!(roller_name(0), "roller");
        assert_eq!(roller_name(1), "dial");
        assert_eq!(roller_name(2), "roller 2");
    }

    #[test]
    fn the_timing_comes_from_the_configuration() {
        let timing = Timing {
            slot_probe: Duration::from_millis(50),
            divert_settle: Duration::from_millis(7),
            ..Timing::default()
        };
        let driver = dialpad_timing(&timing);
        assert_eq!(driver.slot_probe, Duration::from_millis(50));
        assert_eq!(driver.divert.settle, Duration::from_millis(7));
        assert_eq!(driver.divert.spacing, Duration::from_millis(20));
    }
}
