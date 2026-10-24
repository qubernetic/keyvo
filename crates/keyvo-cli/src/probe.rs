//! `keyvo probe`: what the Keypad says about itself, read-only. No
//! keep-alive, no claim, no divert, so the display mode it reports is the
//! one the device is in (plan §5 items 4 to 6).

use std::io::Write;

use keyvo_core::hid::HidTransport;
use keyvo_hid::display;
use keyvo_hid::firmware;
use keyvo_hid::hidpp::{Hidpp, HidppError};
use keyvo_hid::keep_alive;
use keyvo_hid::root::feature;

/// Data of the classic ROOT ping; the device echoes it.
const PING_DATA: u8 = 0x39;

/// The classic features keyvo uses, in the order probe lists them.
const CLASSIC: [(u16, &str); 4] = [
    (feature::DEVICE_FW_VERSION, "DEVICE_FW_VERSION"),
    (feature::KEEP_ALIVE, "KEEP_ALIVE"),
    (feature::REPROG_CONTROLS_V4, "REPROG_CONTROLS_V4"),
    (feature::BRIGHTNESS_CONTROL, "BRIGHTNESS_CONTROL"),
];

/// The VLP features keyvo uses.
const VLP: [(u16, &str); 1] = [(feature::VLP_DISPLAY, "VLP_DISPLAY")];

/// Width of the label column.
const LABEL: usize = 12;

/// Writes the probe report for the device at `device` to `out`. A step the
/// device refuses or does not answer is reported in its line and the probe
/// goes on; a transport error ends it.
///
/// # Errors
///
/// The transport errors and write errors on `out`.
pub fn probe<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    device: &str,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let mut report = Report { out };
    report.line("device", device)?;

    let version = step(hidpp.ping(PING_DATA))?;
    report.line("hid++", version.map(|v| format!("{}.{}", v.major, v.minor)))?;

    let mut lines = Vec::new();
    for (id, name) in CLASSIC {
        let info = step(hidpp.get_feature(id))?;
        lines.push(feature_line(
            id,
            name,
            info.map(|i| i.map(|i| (i.index, i.version))),
        ));
    }
    report.lines("classic", &lines)?;

    let entities = step(firmware::entities(hidpp))?;
    match entities {
        Ok(entities) => report.lines(
            "firmware",
            &entities
                .iter()
                .map(|e| {
                    let active = if e.active { ", active" } else { "" };
                    format!("{e}  {}{active}", e.kind)
                })
                .collect::<Vec<_>>(),
        )?,
        Err(error) => report.line("firmware", Err::<String, _>(error))?,
    }

    let range = step(keep_alive::timeout_range(hidpp))?;
    report.line(
        "keep-alive",
        range.map(|r| format!("{}..{} ms", r.min.as_millis(), r.max.as_millis())),
    )?;

    let protocol = step(hidpp.vlp_protocol())?;
    let mut lines = vec![match protocol {
        Ok(p) => format!(
            "protocol {}.{}, available memory {} bytes",
            p.major, p.minor, p.available_memory
        ),
        Err(error) => format!("error: {error}"),
    }];
    for (id, name) in VLP {
        let info = step(hidpp.vlp_get_feature(id))?;
        lines.push(feature_line(
            id,
            name,
            info.map(|i| i.map(|i| (i.index, i.version))),
        ));
    }
    report.lines("vlp", &lines)?;

    let modes = step(display::supported_modes(hidpp))?;
    let mode = step(display::current_mode(hidpp))?;
    report.line(
        "display",
        mode.map(|mode| {
            let supported = modes.as_ref().map_or_else(
                |error| format!("supported modes: {error}"),
                |modes| {
                    let bytes: Vec<_> = modes.iter().map(|m| m.0).collect();
                    format!("supported {}", keyvo_core::hid::to_hex(&bytes))
                },
            );
            format!("mode {:02x} ({supported})", mode.0)
        }),
    )?;

    match step(display::geometry(hidpp))? {
        Ok(geometry) => {
            let mut lines = vec![format!(
                "{}x{}, {} keys",
                geometry.width,
                geometry.height,
                geometry.keys.len()
            )];
            lines.extend(geometry.keys.iter().map(|key| {
                format!(
                    "key {}  {}x{} at {},{}",
                    key.index, key.width, key.height, key.x, key.y
                )
            }));
            report.lines("geometry", &lines)?;
        }
        Err(error) => report.line("geometry", Err::<String, _>(error))?,
    }
    Ok(())
}

/// The outcome of one probe step: a transport error ends the probe, any
/// other error is reported in the step's line.
fn step<V>(result: Result<V, HidppError>) -> anyhow::Result<Result<V, String>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(HidppError::Transport(error)) => Err(error.into()),
        Err(error) => Ok(Err(error.to_string())),
    }
}

fn feature_line(id: u16, name: &str, info: Result<Option<(u8, u8)>, String>) -> String {
    let what = match info {
        Ok(Some((index, version))) => format!("index 0x{index:02x}  version {version}"),
        Ok(None) => "absent".to_owned(),
        Err(error) => format!("error: {error}"),
    };
    format!("0x{id:04x} {name:<18}  {what}")
}

/// The two-column report.
struct Report<'a, W> {
    out: &'a mut W,
}

impl<W: Write> Report<'_, W> {
    fn line(&mut self, label: &str, value: impl Into<Value>) -> std::io::Result<()> {
        let text = match value.into() {
            Value(Ok(text)) => text,
            Value(Err(error)) => format!("error: {error}"),
        };
        writeln!(self.out, "{label:<LABEL$}{text}")
    }

    fn lines(&mut self, label: &str, lines: &[String]) -> std::io::Result<()> {
        for (n, text) in lines.iter().enumerate() {
            let label = if n == 0 { label } else { "" };
            writeln!(self.out, "{label:<LABEL$}{text}")?;
        }
        Ok(())
    }
}

/// A line's value or the error to print instead.
struct Value(Result<String, String>);

impl From<&str> for Value {
    fn from(text: &str) -> Self {
        Self(Ok(text.to_owned()))
    }
}

impl From<Result<String, String>> for Value {
    fn from(result: Result<String, String>) -> Self {
        Self(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{fixture, replay};

    const EXPECTED: &str = "\
device      /dev/hidraw2
hid++       4.5
classic     0x0003 DEVICE_FW_VERSION   index 0x02  version 7
            0x0008 KEEP_ALIVE          index 0x04  version 1
            0x1b04 REPROG_CONTROLS_V4  index 0x0b  version 6
            0x8040 BRIGHTNESS_CONTROL  index 0x0f  version 1
firmware    BL2 20.00.B0017  bootloader
            U1 66.00.B0017  application, active
            IM 01.00.B0017  type 0x0b
            HW 00.00.B0000  hardware
keep-alive  500..10000 ms
vlp         protocol 1.0, available memory 124960 bytes
            0x19a1 VLP_DISPLAY         index 0x02  version 0
display     mode a1 (supported a0 a1 a2 a4 a5 a6 d0 d1 d2 d3 d4 d5 d6)
geometry    480x480, 9 keys
            key 1  118x118 at 24,6
            key 2  118x118 at 182,7
            key 3  118x118 at 340,6
            key 4  118x118 at 23,164
            key 5  118x118 at 181,163
            key 6  118x118 at 339,164
            key 7  118x118 at 23,322
            key 8  118x118 at 181,321
            key 9  118x118 at 340,321
";

    fn run(script: &str) -> anyhow::Result<String> {
        let mut hidpp = replay(script);
        let mut out = Vec::new();
        probe(&mut hidpp, "/dev/hidraw2", &mut out)?;
        assert!(hidpp.transport_mut().is_done(), "script not used up");
        Ok(String::from_utf8(out)?)
    }

    #[test]
    fn probe_reports_the_captured_keypad() {
        assert_eq!(run(&fixture("probe.txt")).unwrap(), EXPECTED);
    }

    #[test]
    fn an_absent_feature_and_a_refused_step_are_reported_and_the_probe_goes_on() {
        let script = fixture("probe.txt")
            // getFeature(0x8040) answers index 0: absent.
            .replace("< 11 ff 00 0e 0f 00 01", "< 11 ff 00 0e 00 00 00")
            // getTimeoutRange answers an invalid-argument error.
            .replace("< 11 ff 04 0e 01 f4 27 10", "< 11 ff ff 04 0e 02 00 00");
        let text = run(&script).unwrap();
        assert!(
            text.contains("0x8040 BRIGHTNESS_CONTROL  absent\n"),
            "{text}"
        );
        assert!(
            text.contains("keep-alive  error: device error InvalidArgument"),
            "{text}"
        );
        assert!(text.ends_with("key 9  118x118 at 340,321\n"), "{text}");
    }

    #[test]
    fn a_transport_error_ends_the_probe() {
        let error = run("").unwrap_err();
        assert!(error.to_string().starts_with("unexpected write"), "{error}");
    }
}
