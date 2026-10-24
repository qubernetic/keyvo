//! keyvo command-line interface.

mod fill;
mod keys;
mod probe;
mod session;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use keyvo_core::clock::SystemClock;
use keyvo_core::hid::{HidTransport, to_hex};
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
    let (transport, device) = open(cli.device.as_deref())?;
    let mut hidpp = hidpp(transport)?;
    let mut stdout = std::io::stdout().lock();
    match cli.command {
        Command::Probe => probe::probe(&mut hidpp, &device, &mut stdout),
        Command::Keys { json, keep_alive } => {
            let mut session = Session::open(hidpp, SystemClock, keep_alive.timing())?;
            eprintln!("keyvo keys on {device}; Ctrl-C to stop");
            keys::keys(
                &mut session,
                &DivertTiming::default(),
                std::thread::sleep,
                json,
                &mut stdout,
                || true,
            )
        }
        Command::Fill {
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
            let mut session = Session::open(hidpp, SystemClock, keep_alive.timing())?;
            fill::fill(
                &mut session,
                &source,
                placement(key, panel),
                &config,
                &mut stdout,
            )?;
            fill::hold(
                &mut session,
                &SystemClock,
                Duration::from_secs(hold),
                std::thread::sleep,
            )
        }
    }
}

/// Opens `device`, or the Keypad's node found through sysfs, and describes
/// it for the user.
fn open(device: Option<&Path>) -> anyhow::Result<(HidrawTransport, String)> {
    let (path, description) = if let Some(path) = device {
        (path.to_path_buf(), path.display().to_string())
    } else {
        let node = sysfs::find_keypad(Path::new(sysfs::SYSFS_HIDRAW), Path::new(sysfs::DEV_DIR))?;
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
    use super::{Cli, Command, KeepAliveTiming};
    use clap::Parser;
    use keyvo_core::hid::FakeHidTransport;
    use keyvo_hid::hidpp::Hidpp;
    use std::time::Duration;

    fn keep_alive(args: &[&str]) -> Option<KeepAliveTiming> {
        let cli = Cli::try_parse_from([&["keyvo", "keys"], args].concat()).unwrap();
        let Command::Keys { keep_alive, .. } = cli.command else {
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
        let Command::Fill {
            key,
            panel,
            index_base,
            swid,
            ..
        } = cli.command
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
