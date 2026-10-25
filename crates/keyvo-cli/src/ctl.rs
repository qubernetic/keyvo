//! `keyvo ctl <command> [json]`: one request to the daemon's control socket
//! (spec §8, `docs/socket-api.md`), its response printed as the daemon
//! sent it. `keyvo ctl subscribe` goes on printing the events until the
//! daemon hangs up or the user interrupts.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
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
    given
        .or_else(|| {
            keyvo_socket
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| {
            keyvo_core::layout::runtime_dir(xdg_runtime_dir, &std::env::temp_dir(), uid)
                .join(socket::SOCKET_FILE)
        })
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

/// Opens the socket at `path`.
///
/// # Errors
///
/// When nobody listens there.
pub fn connect(path: &Path) -> anyhow::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(path).with_context(|| {
        format!(
            "cannot reach the keyvo daemon at {}; is keyvo-daemon running?",
            path.display()
        )
    })
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
    fn the_socket_is_given_then_keyvo_socket_then_the_runtime_directory() {
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
        assert_eq!(
            socket_path(None, os(""), run, 1000),
            PathBuf::from("/run/user/1000/keyvo/keyvo.sock")
        );
        assert_eq!(
            socket_path(None, None, None, 1000),
            std::env::temp_dir().join("keyvo-1000/keyvo.sock")
        );
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
