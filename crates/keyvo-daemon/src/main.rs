//! `keyvo-daemon`: keeps the Keypad painted with the active profile and
//! runs the actions of its presses (spec §4). Logs go to stderr.

use std::path::PathBuf;

use clap::Parser;

/// Command line of `keyvo-daemon`.
#[derive(Debug, Parser)]
#[command(name = "keyvo-daemon", version, about)]
struct Cli {
    /// Configuration directory; default `$XDG_CONFIG_HOME/keyvo` or
    /// `~/.config/keyvo`.
    #[arg(long, value_name = "DIR")]
    config_dir: Option<PathBuf>,
    /// Hidraw node of the Keypad; default: searched under sysfs on every
    /// connection attempt.
    #[arg(long, value_name = "PATH")]
    device: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use anyhow::Context;
    use keyvo_core::layout;
    use keyvo_daemon::daemon::{self, Event};
    use keyvo_daemon::linux::{HidrawConnector, Injector};
    use tokio::sync::broadcast;

    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();
    let config_dir = cli
        .config_dir
        .or_else(|| {
            layout::config_dir(
                std::env::var_os("XDG_CONFIG_HOME").as_deref(),
                std::env::var_os("HOME").as_deref(),
            )
        })
        .context("no configuration directory: set HOME or pass --config-dir")?;
    tracing::info!(
        "keyvo-daemon {} with {}",
        keyvo_core::VERSION,
        config_dir.display()
    );

    let (events, mut log) = broadcast::channel(256);
    tokio::spawn(async move {
        while let Ok(event) = log.recv().await {
            if let Event::Trace(record) = event {
                tracing::info!("press: {record:?}");
            }
        }
    });
    daemon::run(
        &config_dir,
        HidrawConnector::new(cli.device),
        Injector::open(),
        events,
        shutdown(),
    )
    .await?;
    Ok(())
}

/// Completes on `SIGINT` or `SIGTERM`.
#[cfg(target_os = "linux")]
async fn shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    match signal(SignalKind::terminate()) {
        Ok(mut terminate) => {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
        }
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    let _ = Cli::parse();
    anyhow::bail!("keyvo-daemon runs on Linux only for now")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_config_dir_and_device_are_optional() {
        let cli = Cli::try_parse_from(["keyvo-daemon"]).unwrap();
        assert!(cli.config_dir.is_none() && cli.device.is_none());
        let cli = Cli::try_parse_from([
            "keyvo-daemon",
            "--config-dir",
            "/c",
            "--device",
            "/dev/hidraw2",
        ])
        .unwrap();
        assert_eq!(cli.config_dir, Some(PathBuf::from("/c")));
        assert_eq!(cli.device, Some(PathBuf::from("/dev/hidraw2")));
    }
}
