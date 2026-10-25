//! keyvo command-line interface.

mod ctl;
mod fill;
mod keys;
mod preview;
mod probe;
mod session;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use keyvo_core::clock::{Clock, SystemClock};
use keyvo_core::hid::{HidTransport, to_hex};
use keyvo_core::layout;
use keyvo_hid::hidpp::{DEFAULT_REPLY_TIMEOUT, DEVICE_DIRECT, Hidpp, SoftwareId};
use keyvo_hid::hidraw::HidrawTransport;
use keyvo_hid::image::{ImageConfig, IndexBase};
use keyvo_hid::keep_alive::KeepAliveTiming;
use keyvo_hid::reprog::DivertTiming;
use keyvo_hid::sysfs;

use crate::fill::{Placement, Source};
use crate::session::Session;

/// Software id of keyvo's requests; Options+ uses `0xe` (captured), so the
/// fixtures replay unchanged.
const REQUEST_SWID: u8 = 0xe;

/// keyvo: context-aware control surface for the Logitech MX Creative Console.
#[derive(Parser)]
#[command(name = "keyvo", version = keyvo_core::VERSION)]
struct Cli {
    /// The hidraw node to use, instead of searching sysfs for the Keypad
    #[arg(long, global = true, value_name = "PATH")]
    device: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(flatten)]
    Device(DeviceCommand),
    /// Render one page of a profile onto the Keypad's panel and write it as
    /// a PNG, without a device
    Preview {
        /// A profile id from the configuration directory, or a profile file
        profile: String,
        /// The page, counted from 1
        #[arg(long, value_name = "N", default_value_t = 1,
              value_parser = clap::value_parser!(u64).range(1..))]
        page: u64,
        /// The PNG to write
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
    /// Send one command to the running daemon and print its response;
    /// `subscribe` prints the events until interrupted
    Ctl {
        /// `status`, `subscribe`, `press`, `reload` or `notify_context`
        /// (docs/socket-api.md)
        command: String,
        /// The command's fields as a JSON object, such as
        /// `{"key":5,"dry_run":true}`
        json: Option<String>,
        /// The control socket; default `$KEYVO_SOCKET`, else
        /// `$XDG_RUNTIME_DIR/keyvo/keyvo.sock`
        #[arg(long, value_name = "PATH")]
        socket: Option<PathBuf>,
    },
}

/// The commands that talk to the Keypad.
#[derive(Subcommand)]
enum DeviceCommand {
    /// Print what the Keypad reports about itself (read-only)
    Probe,
    /// Claim the Keypad, divert the page buttons and print every press and
    /// release until interrupted
    Keys {
        /// One JSON object per event (NDJSON)
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        keep_alive: KeepAliveArgs,
    },
    /// Claim the Keypad and paint a color or a PNG on the keys or the panel
    Fill {
        /// `#rrggbb`, or the path of a PNG, scaled to each target
        source: String,
        /// One key, numbered from 1 in the published geometry
        #[arg(long, value_name = "N", group = "placement")]
        key: Option<u8>,
        /// Every key, one write each (the default)
        #[arg(long, group = "placement")]
        all: bool,
        /// The whole panel in one write
        #[arg(long, group = "placement")]
        panel: bool,
        /// Keep the claim (and the keep-alive) this many seconds after painting
        #[arg(long, value_name = "SECONDS", default_value_t = 0)]
        hold: u64,
        /// Fragment index of an image's first fragment (plan §5 item 7)
        #[arg(long, value_name = "0|1", default_value_t = 0,
              value_parser = clap::value_parser!(u8).range(0..=1))]
        index_base: u8,
        /// Software id of the image writes, `0x1` to `0xf` (plan §5 item 7)
        #[arg(long, value_name = "SWID", default_value = "0xd", value_parser = parse_swid)]
        swid: SoftwareIdArg,
        #[command(flatten)]
        keep_alive: KeepAliveArgs,
    },
}

/// A parsed `--swid`.
#[derive(Clone, Copy)]
struct SoftwareIdArg(SoftwareId);

/// `0xd` (hex) or `13` (decimal).
fn parse_swid(text: &str) -> Result<SoftwareIdArg, String> {
    let value = match text.strip_prefix("0x") {
        Some(hex) => u8::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .map_err(|error| format!("`{text}`: {error}"))?;
    SoftwareId::new(value)
        .map(SoftwareIdArg)
        .map_err(|error| error.to_string())
}

/// The placement the `fill` flags select; every key without one.
const fn placement(key: Option<u8>, panel: bool) -> Placement {
    match (key, panel) {
        (Some(key), _) => Placement::Key(key),
        (None, true) => Placement::Panel,
        (None, false) => Placement::All,
    }
}

/// The keep-alive settings, for the hardware check (plan §5 items 2, 3).
#[derive(clap::Args)]
struct KeepAliveArgs {
    /// Do not send keep-alive pings
    #[arg(long)]
    no_keep_alive: bool,
    /// Time between two pings in ms (`[timing].keep_alive_period`, default
    /// 1000)
    #[arg(long, value_name = "MS")]
    keep_alive_period: Option<u64>,
    /// Timeout requested with every ping in ms
    /// (`[timing].keep_alive_timeout`, default 3000)
    #[arg(long, value_name = "MS")]
    keep_alive_timeout: Option<u64>,
}

impl KeepAliveArgs {
    /// `None` without keep-alive, else the timing with the defaults filled in.
    fn timing(&self) -> Option<KeepAliveTiming> {
        let defaults = KeepAliveTiming::default();
        (!self.no_keep_alive).then(|| KeepAliveTiming {
            period: self
                .keep_alive_period
                .map_or(defaults.period, Duration::from_millis),
            timeout: self
                .keep_alive_timeout
                .map_or(defaults.timeout, Duration::from_millis),
        })
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let command = match cli.command {
        Command::Device(command) => command,
        Command::Preview {
            profile,
            page,
            output,
        } => {
            let dir = layout::config_dir(
                std::env::var_os("XDG_CONFIG_HOME").as_deref(),
                std::env::var_os("HOME").as_deref(),
            );
            return preview::preview(
                &profile,
                usize::try_from(page)?,
                &output,
                dir.as_deref(),
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            );
        }
        Command::Ctl {
            command,
            json,
            socket,
        } => {
            let path = ctl::socket_path(
                socket,
                std::env::var_os("KEYVO_SOCKET").as_deref(),
                std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
                rustix::process::getuid().as_raw(),
            );
            let line = ctl::request_line(&command, json.as_deref())?;
            return ctl::ctl(
                ctl::connect(&path)?,
                &line,
                command == "subscribe",
                &mut std::io::stdout().lock(),
            );
        }
    };
    let (transport, device) = open(
        cli.device.as_deref(),
        Path::new(sysfs::SYSFS_HIDRAW),
        Path::new(sysfs::DEV_DIR),
    )?;
    run(
        command,
        hidpp(transport)?,
        &device,
        &SystemClock,
        std::thread::sleep,
        || true,
        &mut std::io::stdout().lock(),
    )
}

/// Runs `command` on the Keypad behind `hidpp`, described as `device`.
/// Time comes from `clock`, waits go through `sleep`, and `keys` streams
/// until `keep_going` says stop.
///
/// # Errors
///
/// The command's errors.
fn run<T: HidTransport, C: Clock>(
    command: DeviceCommand,
    mut hidpp: Hidpp<T>,
    device: &str,
    clock: &C,
    sleep: impl FnMut(Duration),
    keep_going: impl FnMut() -> bool,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    match command {
        DeviceCommand::Probe => probe::probe(&mut hidpp, device, out),
        DeviceCommand::Keys { json, keep_alive } => {
            let mut session = Session::open(hidpp, clock, keep_alive.timing())?;
            eprintln!("keyvo keys on {device}; Ctrl-C to stop");
            keys::keys(
                &mut session,
                &DivertTiming::default(),
                sleep,
                json,
                out,
                keep_going,
            )
        }
        DeviceCommand::Fill {
            source,
            key,
            panel,
            hold,
            index_base,
            swid,
            keep_alive,
            ..
        } => {
            let source = Source::parse(&source)?;
            let config = ImageConfig {
                swid: swid.0,
                index_base: if index_base == 0 {
                    IndexBase::Zero
                } else {
                    IndexBase::One
                },
                ..ImageConfig::default()
            };
            let mut session = Session::open(hidpp, clock, keep_alive.timing())?;
            let placement = placement(key, panel);
            fill::fill(&mut session, &source, placement, &config, out)?;
            fill::hold(
                &mut session,
                clock,
                Duration::from_secs(hold),
                sleep,
                |session| {
                    writeln!(out, "display reclaimed after a lost claim; painting again")?;
                    fill::fill(session, &source, placement, &config, out)
                },
            )
        }
    }
}

/// Opens `device`, or the Keypad's node found under the sysfs `class_dir`
/// with its device file in `dev_dir`, and describes it for the user.
fn open(
    device: Option<&Path>,
    class_dir: &Path,
    dev_dir: &Path,
) -> anyhow::Result<(HidrawTransport, String)> {
    let (path, description) = if let Some(path) = device {
        (path.to_path_buf(), path.display().to_string())
    } else {
        let node = sysfs::find_keypad(class_dir, dev_dir)?;
        let description = format!(
            "{} ({}, {}, reports {})",
            node.path.display(),
            node.hid_name,
            node.id,
            to_hex(&node.report_ids)
        );
        (node.path, description)
    };
    let transport = HidrawTransport::open(&path).context("cannot open the Keypad")?;
    Ok((transport, description))
}

/// The HID++ endpoint of a Keypad attached by USB.
fn hidpp<T: HidTransport>(transport: T) -> anyhow::Result<Hidpp<T>> {
    Ok(Hidpp::new(
        transport,
        DEVICE_DIRECT,
        SoftwareId::new(REQUEST_SWID)?,
        DEFAULT_REPLY_TIMEOUT,
    ))
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, DeviceCommand, KeepAliveTiming};
    use clap::Parser;
    use keyvo_core::clock::FakeClock;
    use keyvo_core::hid::FakeHidTransport;
    use keyvo_hid::hidpp::Hidpp;
    use keyvo_hid::image::{ImageConfig, Target};
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::Duration;

    fn command(args: &[&str]) -> DeviceCommand {
        match Cli::try_parse_from([&["keyvo"], args].concat())
            .unwrap()
            .command
        {
            Command::Device(command) => command,
            Command::Preview { .. } | Command::Ctl { .. } => panic!("not a device command"),
        }
    }

    /// `(profile, page, output)` of a `preview` command line.
    fn preview(args: &[&str]) -> Result<(String, u64, PathBuf), clap::Error> {
        let cli = Cli::try_parse_from([&["keyvo", "preview"], args].concat())?;
        let Command::Preview {
            profile,
            page,
            output,
        } = cli.command
        else {
            panic!("not preview");
        };
        Ok((profile, page, output))
    }

    #[test]
    fn preview_takes_a_profile_a_page_and_an_output() {
        assert_eq!(
            preview(&["demo", "-o", "grid.png"]).unwrap(),
            ("demo".to_owned(), 1, PathBuf::from("grid.png"))
        );
        assert_eq!(
            preview(&["demo", "--page", "2", "--output", "grid.png"])
                .unwrap()
                .1,
            2
        );
        for args in [
            &["demo"][..],
            &["demo", "-o", "x.png", "--page", "0"],
            &["-o", "x.png"],
        ] {
            assert!(preview(args).is_err(), "{args:?}");
        }
    }

    /// `run` on a replayed device, with a fake clock that the sleeps move.
    fn run(args: &[&str], script: &str) -> String {
        let clock = FakeClock::new();
        let mut out = Vec::new();
        super::run(
            command(args),
            replay(script),
            "/dev/hidraw2",
            &clock,
            |wait| clock.advance(wait),
            || false,
            &mut out,
        )
        .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn run_dispatches_probe() {
        let text = run(&["probe"], &fixture("probe.txt"));
        assert!(
            text.starts_with("device      /dev/hidraw2\nhid++       4.5\n"),
            "{text}"
        );
    }

    #[test]
    fn run_dispatches_keys() {
        let text = run(&["keys", "--json"], &crate::keys::tests::script());
        assert_eq!(text.lines().count(), 8);
        assert_eq!(text.lines().next(), Some(r#"{"edge":"press","key":5}"#));
    }

    #[test]
    fn run_dispatches_fill_and_holds() {
        let source = crate::fill::Source::parse("#ff8000").unwrap();
        let config = ImageConfig {
            index_base: keyvo_hid::image::IndexBase::One,
            ..ImageConfig::default()
        };
        let key = crate::fill::tests::paints(&source, &[Target::Key(5)], &config);
        // The paint, a lost claim during the hold, the reclaim, the repaint.
        let script = format!(
            "{}{key}{}{}{key}",
            fixture("display-claim.txt"),
            fixture("claim-lost.txt"),
            fixture("display-reclaim.txt"),
        );
        let text = run(
            &[
                "fill",
                "#ff8000",
                "--key",
                "5",
                "--index-base",
                "1",
                "--hold",
                "2",
                "--no-keep-alive",
            ],
            &script,
        );
        assert!(text.starts_with("key 5  118x118 at 181,163  "), "{text}");
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert_eq!(
            lines[1],
            "display reclaimed after a lost claim; painting again"
        );
        assert_eq!(lines[2], lines[0]);
    }

    #[test]
    fn open_finds_the_keypad_node_or_takes_the_given_one() {
        let root = tempfile::tempdir().unwrap();
        let device = root.path().join("class/hidraw2/device");
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(
            device.join("uevent"),
            "HID_ID=0003:0000046D:0000C354\nHID_NAME=Logitech MX Creative Keypad\n",
        )
        .unwrap();
        std::fs::write(device.join("report_descriptor"), [0x85, 0x11, 0x85, 0x14]).unwrap();
        let dev = root.path().join("dev");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("hidraw2"), []).unwrap();

        let (_, description) = super::open(None, &root.path().join("class"), &dev).unwrap();
        assert_eq!(
            description,
            format!(
                "{} (Logitech MX Creative Keypad, 046d:c354, reports 11 14)",
                dev.join("hidraw2").display()
            )
        );
        let given = dev.join("hidraw2");
        let (_, description) = super::open(Some(&given), Path::new("/absent"), &dev).unwrap();
        assert_eq!(description, given.display().to_string());

        let error = super::open(None, Path::new("/absent"), &dev).unwrap_err();
        assert_eq!(error.to_string(), "no hidraw node for 046d:c354");
    }

    fn keep_alive(args: &[&str]) -> Option<KeepAliveTiming> {
        let cli = Cli::try_parse_from([&["keyvo", "keys"], args].concat()).unwrap();
        let Command::Device(DeviceCommand::Keys { keep_alive, .. }) = cli.command else {
            panic!("not keys");
        };
        keep_alive.timing()
    }

    #[test]
    fn keep_alive_flags_override_the_timing_defaults() {
        assert_eq!(keep_alive(&[]), Some(KeepAliveTiming::default()));
        assert_eq!(
            keep_alive(&[
                "--keep-alive-period",
                "3000",
                "--keep-alive-timeout",
                "10000"
            ]),
            Some(KeepAliveTiming {
                period: Duration::from_secs(3),
                timeout: Duration::from_secs(10),
            })
        );
        assert_eq!(keep_alive(&["--no-keep-alive"]), None);
    }

    /// `(placement, index base, swid)` of a `fill` command line.
    fn fill(args: &[&str]) -> Result<(super::Placement, u8, u8), clap::Error> {
        let cli = Cli::try_parse_from([&["keyvo", "fill", "#ff8000"], args].concat())?;
        let Command::Device(DeviceCommand::Fill {
            key,
            panel,
            index_base,
            swid,
            ..
        }) = cli.command
        else {
            panic!("not fill");
        };
        Ok((super::placement(key, panel), index_base, swid.0.get()))
    }

    #[test]
    fn fill_paints_every_key_by_default_with_the_options_plus_settings() {
        use super::Placement;
        assert_eq!(fill(&[]).unwrap(), (Placement::All, 0, 0xd));
        assert_eq!(fill(&["--all"]).unwrap().0, Placement::All);
        assert_eq!(fill(&["--key", "5"]).unwrap().0, Placement::Key(5));
        assert_eq!(fill(&["--panel"]).unwrap().0, Placement::Panel);
        assert_eq!(
            fill(&["--index-base", "1", "--swid", "0xb"]).unwrap(),
            (Placement::All, 1, 0xb)
        );
        assert_eq!(fill(&["--swid", "11"]).unwrap().2, 0xb);
    }

    #[test]
    fn fill_refuses_two_placements_and_bad_settings() {
        for args in [
            &["--key", "5", "--panel"][..],
            &["--index-base", "2"],
            &["--swid", "0"],
            &["--swid", "0x10"],
            &["--swid", "zz"],
        ] {
            assert!(fill(args).is_err(), "{args:?}");
        }
    }

    /// A fixture of `tests/protocol-fixtures/hid/`.
    pub fn fixture(name: &str) -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/protocol-fixtures/hid/"
        );
        std::fs::read_to_string(format!("{path}{name}")).unwrap()
    }

    /// The exchanges `range` of a fixture script (a `>` line with the `<`
    /// lines after it, counted from 0), comments dropped.
    pub fn exchanges(script: &str, range: std::ops::Range<usize>) -> String {
        let mut exchange = None::<usize>;
        let mut out = String::new();
        for line in script.lines() {
            if line.starts_with('>') {
                exchange = Some(exchange.map_or(0, |n| n + 1));
            }
            let wanted = exchange.is_some_and(|n| range.contains(&n));
            if wanted && line.starts_with(['>', '<']) {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    /// A Keypad endpoint replaying `script`, as `main` builds it.
    pub fn replay(script: &str) -> Hidpp<FakeHidTransport> {
        super::hidpp(FakeHidTransport::from_script(script).unwrap()).unwrap()
    }
}
