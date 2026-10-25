# Socket API v1

The control socket is the only inward control surface of the keyvo daemon
(`docs/spec.md` §8, ADR-0003). `keyvo ctl`, the GUI, plugins and
integrations speak it. This page is the wire contract. The Rust types are
`keyvo_core::socket`, and the fixtures in
[`tests/protocol-fixtures/socket/`](../tests/protocol-fixtures/socket/) are
shared by every client.

Pre-alpha subset: `status`, `subscribe`, `press` (with `dry_run`), `reload`
and `notify_context`. The other v1 commands of spec §8 (`set_key`,
`set_badge`, `set_state`, `push_overlay`, `pop_overlay`, `switch_profile`,
`set_mode`, `set_page`, `trace`, `project`, `setup`, `doctor`) come with
their roadmap issues. Until then they are rejected as `invalid_request`.

## Transport

- A Unix stream socket at `$XDG_RUNTIME_DIR/keyvo/keyvo.sock`. Without an
  absolute `XDG_RUNTIME_DIR` it is `keyvo-<uid>/keyvo.sock` in the
  temporary directory. `keyvo-daemon --socket PATH` puts it elsewhere.
- The socket is mode `0600`. Its directory is created `0700`. The daemon
  refuses to start when the directory exists but is not private to the
  user, or when another daemon answers on the socket. A socket file left
  behind by a daemon that is gone is replaced. The daemon removes the
  socket when it stops.
- Trust model as in `ssh-agent`: any process of the same user may send any
  command (`SECURITY.md`).
- The clients check the other side before they send anything
  (`keyvo_client::open`, used by `keyvo ctl` and the app): the socket's
  directory must be a directory of the user that nobody else may enter
  (a `0755` directory is refused), and the process listening on the
  socket must run as the user (`SO_PEERCRED`). A failed check is an error
  that names the socket and the reason.

## Framing

- Newline-delimited JSON: one UTF-8 JSON object per line, ending in `\n`.
  Blank lines are ignored.
- Every message carries `"v": 1`. A request without it, or with another
  version, is answered with `unsupported_version`.
- Key order carries no meaning. Compare parsed values, not text.
- A client ignores fields it does not know in responses and events, at any
  depth, so it keeps working when a later daemon adds fields within
  version 1. Requests are the other way round: the daemon rejects unknown
  fields (below).
- Limits (`docs/spec.md` §8). A request line longer than
  `[daemon].socket_line_bytes` (default 65536 bytes, without the newline)
  is answered with `invalid_request` without an `id` ("the line is longer
  than N bytes"), and the daemon closes the connection; it never holds
  more than the limit of an unfinished line. The daemon serves at most
  `[daemon].socket_connections` connections at once (default 32); one more
  gets an `unavailable` response without an `id` and is closed, so try
  again later. `keyvo_client` reads daemon lines up to 1 MiB and fails a
  longer one.

## Requests and responses

A request names its command in `cmd`, with the command's fields beside it.
The optional `id` may be any JSON value, and the response echoes it:

```json
{"v":1,"id":3,"cmd":"press","key":5,"dry_run":true}
```

The daemon answers each request with exactly one response, in the order
the requests arrived on the connection:

```json
{"v":1,"id":3,"ok":true,"result":{"entry":{...}}}
{"v":1,"id":3,"ok":false,"error":{"code":"invalid_request","message":"..."}}
```

A request with an unknown command, an unknown field or a bad value is
rejected and nothing runs. Because of this, a typo such as `"dryrun"`
never turns a dry run into a real press. The connection stays open after
an error, except after a line that is not UTF-8 or longer than the line
limit, which closes it.

| Error `code` | When |
|---|---|
| `invalid_json` | The line is not one JSON object. |
| `unsupported_version` | `v` is missing or not `1`. |
| `invalid_request` | Unknown command, unknown field, or a bad value (for example `key` outside 1..9); or a line over the line limit, after which the connection closes. |
| `reload_failed` | `reload`: the configuration directory does not load. The last good state stays, and `message` lists every problem as `<file>:<line>:<column>: <message>`. |
| `unavailable` | The daemon is shutting down, or already serves `[daemon].socket_connections` connections (the new one is closed). |

## Commands

### `status`

No fields. The result:

| Field | Type | Meaning |
|---|---|---|
| `version` | string | The daemon's version. |
| `device` | object | `connected` (bool); `keys` (the key count) when connected; `error` (why not) when the daemon knows. |
| `view` | object | What the pad shows: `profile` (id), `page` (from 1), `mode` (`auto` or `manual`). |
| `context` | object, optional | The last focus report: `class`, `title`, `pid` when known, `source` (`watcher` or `notify_context`). Left out before the first report. |
| `trace` | integer | The entries in the trace ring buffer. |

### `subscribe`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `events` | array of class names | every class | The event classes this connection receives. |
| `replay` | integer | 0 | The newest ring-buffer trace entries sent first, oldest first. Only sent when `trace` is among the classes. |

The result has the subscribed `events` and the current `view`. After the
response come the replayed `trace` events, then every later event of the
chosen classes. Nothing is lost or duplicated between the view, the replay
and the stream. Responses to later requests on the same connection are
interleaved with the events, so a client tells them apart by the `event`
key. A second `subscribe` replaces the first.

A subscriber that does not read falls behind. Once it is more than the
daemon's event backlog (256 events) behind, it loses the oldest events,
and the daemon logs how many. Its view can then be stale, so a client
should send `status` when it notices a gap.

### `press`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `key` | integer 1..9 | required | The key, row-major from the top left. |
| `kind` | `short` or `long` | `short` | The press kind. |
| `dry_run` | bool | `false` | Resolve and trace, execute nothing. |

The press resolves on the active profile and page, as a device press does.
The result is `{"entry": <trace entry>}`, and the same entry goes to the
ring buffer and to `trace` subscribers.

- With `"dry_run": true` (the simulator's path), the entry's `trigger`
  has `"simulated": true` and its `result` is `{"status": "dry_run"}`, or
  `unbound` for a key without an action. No `chord` is sent, no `exec`
  runs, and the key's look does not change.
- Without it, the action runs. A `chord`'s entry carries its result
  (`sent` or `failed`). An `exec` returns `pending`, and its result
  follows as a second `trace` event when the command ends.

### `reload`

No fields. The daemon loads the configuration directory now, without
waiting for inotify. The result is `{}`, or the error `reload_failed`.
Either way, `config` subscribers get the event.

### `notify_context`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `class` | string | required | The window class, reverse-DNS or short. |
| `title` | string | `""` | The window title. |

An external focus source (a GNOME extension, a Hyprland or Sway reader)
asserts the focused window. It takes the watcher's path, so
`[timing].focus_hysteresis` applies and the profile follows by the same
rules. The result is `{}`. Pre-alpha: the report does not take precedence
over the watcher (the last report wins), and the `cwd` and pid fields come
with M2-5.

## Events

```json
{"v":1,"event":"profile","profile":"terminal","page":1,"mode":"auto"}
```

| `event` | Fields | When |
|---|---|---|
| `key` | `key` (1..9), `edge` (`press` or `release`) | A key on the device went down or up. Page buttons show up as `page` and `mode` events instead. |
| `profile` | `profile`, `page`, `mode` | Another profile shows. |
| `page` | `profile`, `page`, `mode` | Another page shows. |
| `mode` | `profile`, `page`, `mode` | The mode changed. |
| `device` | `connected`, `keys` or `error` | The Keypad was claimed or lost. |
| `context` | `class`, `title`, `pid` (optional), `source` | A focus source reported a window. A report does not always switch the profile. |
| `trace` | `entry` | A press was resolved (dry run, unbound, chord) or an `exec` ended. |
| `config` | `reloaded`, `error` (when it failed) | The configuration directory was reloaded, or did not load. |

`profile`, `page` and `mode` each carry the whole new view. One switch that
changes several of them sends one event per changed class, in the order
`profile`, `page`, `mode`.

## Trace entries

The pre-alpha subset of spec §8: `timestamp_ms` (milliseconds since the
Unix epoch), `trigger` (`press`, `simulated`), `profile`, `page`, `key`,
`action` (the key's action as in the profile, or `null`), and `result`
with `status` set to one of the following:

- `unbound`
- `dry_run`
- `pending`
- `sent`, with `events` (`code`, `down`)
- `exited`, with `code` and `duration_ms`
- `failed`, with `error`

The daemon keeps the newest `[trace].ring` entries (default 1000) in
memory. They are lost when it stops. The trace file and `keyvo ctl
trace|tail` come with M1-8.

## `keyvo ctl`

```bash
keyvo ctl status
keyvo ctl press '{"key":5,"dry_run":true}'
keyvo ctl notify_context '{"class":"org.kde.konsole","title":"~"}'
keyvo ctl subscribe '{"events":["trace","profile"],"replay":10}'
```

`keyvo ctl <command> [json]` sends one request. `json` is an object with
the command's fields, and `v` is added unless given. The response is
printed as the daemon sent it, one line on stdout. A refused request
prints the response, then the code and message on stderr, and exits 1.
`keyvo ctl subscribe` goes on printing every line until the daemon hangs
up or Ctrl-C. The socket is `--socket PATH`, else `$KEYVO_SOCKET`, else
the default above.
