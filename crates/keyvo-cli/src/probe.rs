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
    let mut lines = vec![match mode {
        Ok(mode) => {
            let supported = modes.as_ref().map_or_else(
                |error| format!("supported modes: {error}"),
                |modes| {
                    let bytes: Vec<_> = modes.iter().map(|m| m.0).collect();
                    format!("supported {}", keyvo_core::hid::to_hex(&bytes))
                },
            );
            format!("mode {:02x} ({supported})", mode.0)
        }
        Err(error) => format!("error: {error}"),
    }];
    match step(display::capabilities(hidpp))? {
        Ok(caps) => lines.extend(capability_lines(&caps)),
        Err(error) => lines.push(format!("capabilities: error: {error}")),
    }
    report.lines("display", &lines)?;

    match step(display::geometry(hidpp))? {
        Ok(geometry) => {
            let mut lines = vec![format!(
                "{}x{}, {} buttons, {} visible areas",
                geometry.width,
                geometry.height,
                geometry.keys.len(),
                geometry.visible_areas
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

/// The display fn0 fields under the names Options+ logs them by.
fn capability_lines(caps: &display::Capabilities) -> [String; 4] {
    let flag = |set: bool| u8::from(set);
    [
        format!("deviceScreenCount {}", caps.screen_count),
        format!("maxImageSize {} bytes", caps.max_image_size),
        format!("maxImageFPS {}", caps.max_image_fps),
        format!(
            "deferrableDisplayUpdate {}  rgb565 {}  rgb888 {}  jpeg {}  calibrated {}  origin {}",
            flag(caps.deferrable_display_update),
            flag(caps.rgb565),
            flag(caps.rgb888),
            flag(caps.jpeg),
            flag(caps.calibrated),
            caps.origin
        ),
    ]
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

/// `keyvo probe --feature <id>`: feature `id` asked of the ROOT of both
/// planes, each answer in its line: index and version, `absent`, or the
/// error reply. Read-only like [`probe`].
///
/// # Errors
///
/// The transport errors and write errors on `out`.
pub fn probe_feature<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    device: &str,
    id: u16,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    let mut report = Report { out };
    report.line("device", device)?;
    let classic = step(hidpp.get_feature(id))?;
    let classic = outcome(classic.map(|i| i.map(|i| (i.index, i.version))));
    report.line("classic", format!("0x{id:04x}  {classic}").as_str())?;
    let vlp = step(hidpp.vlp_get_feature(id))?;
    let vlp = outcome(vlp.map(|i| i.map(|i| (i.index, i.version))));
    report.line("vlp", format!("0x{id:04x}  {vlp}").as_str())?;
    Ok(())
}

fn feature_line(id: u16, name: &str, info: Result<Option<(u8, u8)>, String>) -> String {
    format!("0x{id:04x} {name:<18}  {}", outcome(info))
}

/// A `getFeature` answer as `index 0x0b  version 6`, `absent` or the error.
fn outcome(info: Result<Option<(u8, u8)>, String>) -> String {
    match info {
        Ok(Some((index, version))) => format!("index 0x{index:02x}  version {version}"),
        Ok(None) => "absent".to_owned(),
        Err(error) => format!("error: {error}"),
    }
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
            deviceScreenCount 1
            maxImageSize 124928 bytes
            maxImageFPS 30
            deferrableDisplayUpdate 1  rgb565 1  rgb888 0  jpeg 1  calibrated 1  origin 0
geometry    480x480, 9 buttons, 0 visible areas
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
            .replace("< 11 ff 04 0e 01 f4 27 10", "< 11 ff ff 04 0e 02 00 00")
            // getCapabilities answers a VLP error.
            .replace(
                "< 13 ff 02 0e c0 01 00 7a 1e 00",
                "< 13 ff ff 02 0e 02 00 00 00 00",
            );
        let text = run(&script).unwrap();
        assert!(
            text.contains("\n            capabilities: error: vlp error reply: 13 ff ff 02 0e 02"),
            "{text}"
        );
        assert!(!text.contains("maxImageSize"), "{text}");
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

    fn run_feature(script: &str, id: u16) -> anyhow::Result<String> {
        let mut hidpp = replay(script);
        let mut out = Vec::new();
        probe_feature(&mut hidpp, "/dev/hidraw2", id, &mut out)?;
        assert!(hidpp.transport_mut().is_done(), "script not used up");
        Ok(String::from_utf8(out)?)
    }

    #[test]
    fn probe_feature_resolves_a_present_feature_on_each_plane() {
        // 0x1b04 is classic only, 0x19a1 VLP only.
        let vlp_absent = fixture("vlp-get-feature-absent.txt").replace("00 20", "1b 04");
        let script = format!("{}{vlp_absent}", fixture("root-get-feature-reprog.txt"));
        assert_eq!(
            run_feature(&script, 0x1b04).unwrap(),
            "device      /dev/hidraw2\n\
             classic     0x1b04  index 0x0b  version 6\n\
             vlp         0x1b04  absent\n"
        );
        let classic_absent = fixture("root-get-feature-absent.txt").replace("00 20", "19 a1");
        let script = format!("{classic_absent}{}", fixture("vlp-get-feature-display.txt"));
        assert_eq!(
            run_feature(&script, 0x19a1).unwrap(),
            "device      /dev/hidraw2\n\
             classic     0x19a1  absent\n\
             vlp         0x19a1  index 0x02  version 0\n"
        );
    }

    #[test]
    fn probe_feature_reports_an_absent_feature() {
        let script = format!(
            "{}{}",
            fixture("root-get-feature-absent.txt"),
            fixture("vlp-get-feature-absent.txt")
        );
        assert_eq!(
            run_feature(&script, 0x0020).unwrap(),
            "device      /dev/hidraw2\n\
             classic     0x0020  absent\n\
             vlp         0x0020  absent\n"
        );
    }

    #[test]
    fn probe_feature_reports_the_error_reply_and_asks_the_other_plane() {
        let text = run_feature(&fixture("probe-feature-error.txt"), 0x1234).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(
            lines[1],
            "classic     0x1234  error: device error InvalidArgument on feature index 0x00 function 0"
        );
        assert!(
            lines[2]
                .starts_with("vlp         0x1234  error: vlp error reply: 13 ff ff 00 0e 02 00"),
            "{text}"
        );
        assert_eq!(lines.len(), 3);
        assert!(run_feature("", 0x1234).is_err());
    }

    #[test]
    fn a_transport_error_ends_the_probe() {
        let error = run("").unwrap_err();
        assert!(error.to_string().starts_with("unexpected write"), "{error}");
    }
}
