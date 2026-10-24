//! keyvo command-line interface.

mod probe;

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use keyvo_core::hid::{HidTransport, to_hex};
use keyvo_hid::hidpp::{DEFAULT_REPLY_TIMEOUT, DEVICE_DIRECT, Hidpp, SoftwareId};
use keyvo_hid::hidraw::HidrawTransport;
use keyvo_hid::sysfs;

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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let (transport, device) = open(cli.device.as_deref())?;
    let mut hidpp = hidpp(transport)?;
    let mut stdout = std::io::stdout().lock();
    match cli.command {
        Command::Probe => probe::probe(&mut hidpp, &device, &mut stdout),
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
    use keyvo_core::hid::FakeHidTransport;
    use keyvo_hid::hidpp::Hidpp;

    /// A fixture of `tests/protocol-fixtures/hid/`.
    pub fn fixture(name: &str) -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/protocol-fixtures/hid/"
        );
        std::fs::read_to_string(format!("{path}{name}")).unwrap()
    }

    /// A Keypad endpoint replaying `script`, as `main` builds it.
    pub fn replay(script: &str) -> Hidpp<FakeHidTransport> {
        super::hidpp(FakeHidTransport::from_script(script).unwrap()).unwrap()
    }
}
