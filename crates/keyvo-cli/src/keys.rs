//! `keyvo keys`: streams key and page-button presses and releases, one line
//! per event, as text or as NDJSON (`--json`). The page buttons are diverted
//! first (spec §5.6); the keep-alive runs between reads.

use std::io::Write;
use std::time::Duration;

use keyvo_core::clock::Clock;
use keyvo_core::hid::HidTransport;
use keyvo_hid::input::{Control, Edge, Input, InputEvent};
use keyvo_hid::reprog::{self, DivertTiming, PAGE_LEFT, PAGE_RIGHT};

use crate::session::Session;

/// Diverts the page buttons, then writes every input event to `out` until
/// `keep_going` says stop; it is asked whenever a read found nothing.
///
/// # Errors
///
/// The divert errors, the transport errors (a disconnect ends the stream)
/// and write errors on `out`.
pub fn keys<T: HidTransport, C: Clock>(
    session: &mut Session<T, C>,
    divert: &DivertTiming,
    sleep: impl FnMut(Duration),
    json: bool,
    out: &mut impl Write,
    mut keep_going: impl FnMut() -> bool,
) -> anyhow::Result<()> {
    let diverted = reprog::divert_page_buttons(&mut session.hidpp, divert, sleep)?;
    let key_count = u8::try_from(session.claim.geometry.keys.len())?;
    let mut input = Input::new(
        session.hidpp.device_index(),
        session.claim.display_index,
        key_count,
    )
    .with_page_buttons(diverted.reprog_index);
    loop {
        session.tick()?;
        let wait = session.until_due();
        match session.hidpp.next_frame(wait)? {
            Some(frame) => {
                for event in input.feed(&frame) {
                    writeln!(out, "{}", format_event(event, json))?;
                }
            }
            None if keep_going() => {}
            None => return Ok(()),
        }
    }
}

/// `press key 5`, `release page left`, or the NDJSON form
/// `{"edge":"press","key":5}`, `{"edge":"release","page":"left"}`.
fn format_event(event: InputEvent, json: bool) -> String {
    let edge = match event.edge {
        Edge::Press => "press",
        Edge::Release => "release",
    };
    let page = |cid| match cid {
        PAGE_LEFT => Some("left"),
        PAGE_RIGHT => Some("right"),
        _ => None,
    };
    match (event.control, json) {
        (Control::Key(key), false) => format!("{edge} key {key}"),
        (Control::Key(key), true) => format!(r#"{{"edge":"{edge}","key":{key}}}"#),
        (Control::Page(cid), false) => page(cid).map_or_else(
            || format!("{edge} control 0x{cid:04x}"),
            |side| format!("{edge} page {side}"),
        ),
        (Control::Page(cid), true) => page(cid).map_or_else(
            || format!(r#"{{"edge":"{edge}","cid":{cid}}}"#),
            |side| format!(r#"{{"edge":"{edge}","page":"{side}"}}"#),
        ),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tests::{exchanges, fixture, replay};
    use keyvo_core::clock::FakeClock;
    use keyvo_core::hid::TransportError;
    use keyvo_hid::hidpp::HidppError;
    use keyvo_hid::keep_alive::KeepAliveTiming;

    /// Keep-alive start, claim and divert, then the two input fixtures.
    pub(crate) fn script() -> String {
        format!(
            "{}{}{}{}{}",
            exchanges(&fixture("keep-alive-ping.txt"), 0..2),
            fixture("display-claim.txt"),
            fixture("reprog-divert.txt"),
            fixture("key-input.txt"),
            fixture("page-buttons.txt"),
        )
    }

    fn run(json: bool) -> (String, Vec<Duration>) {
        let clock = FakeClock::new();
        let mut session =
            Session::open(replay(&script()), &clock, Some(KeepAliveTiming::default())).unwrap();
        let mut waits = Vec::new();
        let mut out = Vec::new();
        keys(
            &mut session,
            &DivertTiming::default(),
            |wait| waits.push(wait),
            json,
            &mut out,
            || false,
        )
        .unwrap();
        assert!(session.hidpp.transport_mut().is_done());
        (String::from_utf8(out).unwrap(), waits)
    }

    #[test]
    fn keys_streams_the_fixture_presses_and_releases() {
        let (text, waits) = run(false);
        assert_eq!(
            text,
            "\
press key 5
press key 1
release key 5
release key 1
press page left
press page right
release page left
release page right
"
        );
        assert_eq!(waits.first(), Some(&DivertTiming::default().settle));
    }

    #[test]
    fn keys_json_is_one_object_per_line() {
        let (text, _) = run(true);
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 8);
        assert_eq!(lines[0], r#"{"edge":"press","key":5}"#);
        assert_eq!(lines[3], r#"{"edge":"release","key":1}"#);
        assert_eq!(lines[4], r#"{"edge":"press","page":"left"}"#);
        assert_eq!(lines[7], r#"{"edge":"release","page":"right"}"#);
    }

    #[test]
    fn a_control_other_than_the_page_buttons_shows_its_cid() {
        let event = InputEvent::press(Control::Page(0x0053));
        assert_eq!(format_event(event, false), "press control 0x0053");
        assert_eq!(format_event(event, true), r#"{"edge":"press","cid":83}"#);
    }

    #[test]
    fn a_failed_divert_ends_the_command() {
        // The claim, then nothing scripted: getFeature(0x1B04) is unexpected.
        let clock = FakeClock::new();
        let mut session =
            Session::open(replay(&fixture("display-claim.txt")), &clock, None).unwrap();
        let error = keys(
            &mut session,
            &DivertTiming::default(),
            |_| {},
            false,
            &mut Vec::new(),
            || true,
        )
        .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<reprog::DivertError>(),
                Some(reprog::DivertError::Hidpp(HidppError::Transport(
                    TransportError::UnexpectedWrite { .. }
                )))
            ),
            "{error:?}"
        );
    }
}
