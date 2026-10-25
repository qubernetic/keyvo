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
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();
    let config_dir = config_dir(
        cli.config_dir,
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    tracing::info!(
        "keyvo-daemon {} with {}",
        keyvo_core::VERSION,
        config_dir.display()
    );

    let script_dir = keyvo_daemon::kwin::script_dir(std::env::var_os("XDG_RUNTIME_DIR").as_deref());
    let watcher = keyvo_daemon::kwin::KWinWatcher::session(&script_dir);
    keyvo_daemon::linux::serve(&config_dir, cli.device, watcher, shutdown()).await?;
    Ok(())
}

/// `--config-dir`, else the directory of the environment (spec §7.1).
fn config_dir(
    given: Option<PathBuf>,
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> anyhow::Result<PathBuf> {
    given
        .or_else(|| keyvo_core::layout::config_dir(xdg_config_home, home))
        .ok_or_else(|| anyhow::anyhow!("no configuration directory: set HOME or pass --config-dir"))
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

    #[test]
    fn the_given_config_dir_wins_over_the_environment() {
        let home = Some(std::ffi::OsStr::new("/home/u"));
        assert_eq!(
            config_dir(Some("/c".into()), None, home).unwrap(),
            PathBuf::from("/c")
        );
        assert_eq!(
            config_dir(None, None, home).unwrap(),
            PathBuf::from("/home/u/.config/keyvo")
        );
        assert_eq!(
            config_dir(None, None, None).unwrap_err().to_string(),
            "no configuration directory: set HOME or pass --config-dir"
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn shutdown_completes_on_sigterm() {
        let signal = shutdown();
        tokio::pin!(signal);
        // The first poll installs the handlers; nothing has arrived yet.
        let early = tokio::time::timeout(std::time::Duration::from_millis(10), &mut signal);
        assert!(early.await.is_err());
        // nextest runs every test in its own process.
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TERM)
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(10), signal)
            .await
            .unwrap();
    }
}
