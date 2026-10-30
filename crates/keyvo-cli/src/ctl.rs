//! `keyvo ctl <command> [json]`: one request to the daemon's control socket
//! (spec §8, `docs/socket-api.md`), its response printed as the daemon
//! sent it. `keyvo ctl subscribe` goes on printing the events until the
//! daemon hangs up or the user interrupts.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use keyvo_client::ClientError;
use keyvo_core::socket::{self, ServerMessage};
use serde_json::{Map, Value};

/// The socket: `--socket`, else `$KEYVO_SOCKET`, else the daemon's default
/// in the runtime directory.
pub fn socket_path(
    given: Option<PathBuf>,
    keyvo_socket: Option<&OsStr>,
    xdg_runtime_dir: Option<&OsStr>,
    uid: u32,
) -> PathBuf {
    given.unwrap_or_else(|| keyvo_client::socket_path(keyvo_socket, xdg_runtime_dir, uid))
}

/// The request line for `command` with the fields of `json`.
///
/// # Errors
///
/// When `json` is not a JSON object, or names the command itself.
pub fn request_line(command: &str, json: Option<&str>) -> anyhow::Result<String> {
    let mut fields = match json {
        None => Map::new(),
        Some(text) => match serde_json::from_str(text).context("the JSON argument is not JSON")? {
            Value::Object(fields) => fields,
            _ => bail!("the JSON argument must be an object"),
        },
    };
    if fields.contains_key("cmd") {
        bail!("give the command as the first argument, not as `cmd`");
    }
    fields
        .entry("v")
        .or_insert_with(|| Value::from(socket::VERSION));
    fields.insert("cmd".into(), Value::from(command));
    Ok(Value::Object(fields).to_string())
}

/// Opens the socket at `path` with the checks of [`keyvo_client::open`]:
/// a private socket directory and a daemon of this user.
///
/// # Errors
///
/// When nobody listens there, or a check fails.
pub fn connect(path: &Path) -> anyhow::Result<std::os::unix::net::UnixStream> {
    match keyvo_client::open(path) {
        Ok(stream) => Ok(stream),
        Err(error @ ClientError::Connect { .. }) => bail!("{error}; is keyvo-daemon running?"),
        Err(error) => Err(error.into()),
    }
}

/// Sends `line` on `stream` and prints the response to `out`; with
/// `follow`, every later line too, until the daemon hangs up.
///
/// # Errors
///
/// When the daemon refuses the request (after printing its response), does
/// not answer, or the stream fails.
pub fn ctl<S: Read + Write>(
    mut stream: S,
    line: &str,
    follow: bool,
    out: &mut impl Write,
) -> anyhow::Result<()> {
    stream.write_all(format!("{line}\n").as_bytes())?;
    stream.flush()?;
    let mut lines = BufReader::new(stream).lines();
    let Some(response) = lines.next().transpose()? else {
        bail!("the daemon closed the connection without an answer");
    };
    writeln!(out, "{response}")?;
    out.flush()?;
    if let Ok(ServerMessage::Response(response)) = ServerMessage::parse(&response)
        && let Err(error) = response.result
    {
        let code = serde_json::to_value(error.code).unwrap_or_default();
        bail!("{}: {}", code.as_str().unwrap_or("error"), error.message);
    }
    if follow {
        for line in lines {
            writeln!(out, "{}", line?)?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_socket_is_given_then_the_client_default() {
        let os = |s: &'static str| Some(OsStr::new(s));
        let run = os("/run/user/1000");
        assert_eq!(
            socket_path(Some("/a.sock".into()), os("/b.sock"), run, 1000),
            PathBuf::from("/a.sock")
        );
        assert_eq!(
            socket_path(None, os("/b.sock"), run, 1000),
            PathBuf::from("/b.sock")
        );
    }

    #[test]
    fn a_press_drives_a_dialpad_control_as_the_daemon_reads_it() {
        use keyvo_core::dialpad::{ButtonSlot, Control};
        use keyvo_core::socket::{Command, Request};

        let line = request_line(
            "press",
            Some(r#"{"control":"dial","delta":1,"dry_run":true}"#),
        )
        .unwrap();
        let request = Request::parse(&line).unwrap();
        assert!(matches!(
            request.command,
            Command::Press {
                control: Some(Control::Dial),
                delta: Some(1),
                dry_run: true,
                ..
            }
        ));
        let line = request_line("press", Some(r#"{"control":"button:2"}"#)).unwrap();
        let button = ButtonSlot::new(2).map(Control::Button);
        assert!(matches!(
            Request::parse(&line).unwrap().command,
            Command::Press { control, .. } if control == button
        ));
    }

    #[test]
    fn a_press_holds_and_releases_a_modifier_key_as_the_daemon_reads_it() {
        use keyvo_core::socket::{Command, Request};

        for (json, held, released) in [
            (r#"{"key":7,"hold":true}"#, true, false),
            (r#"{"key":7,"release":true,"dry_run":true}"#, false, true),
        ] {
            let line = request_line("press", Some(json)).unwrap();
            let Command::Press { hold, release, .. } = Request::parse(&line).unwrap().command
            else {
                panic!("{line}: not a press");
            };
            assert_eq!((hold, release), (held, released), "{line}");
        }
        let both = request_line("press", Some(r#"{"key":7,"hold":true,"release":true}"#));
        assert!(Request::parse(&both.unwrap()).is_err());
    }

    #[test]
    fn a_given_version_is_kept() {
        assert_eq!(
            serde_json::from_str::<Value>(&request_line("status", Some(r#"{"v":2}"#)).unwrap())
                .unwrap(),
            serde_json::json!({"v": 2, "cmd": "status"})
        );
    }
}
