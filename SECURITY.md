# Security Policy

## Scope

keyvo is a user-session daemon that talks to USB and wireless input devices,
injects keyboard input on the user's behalf, executes user-configured commands,
and exposes a local control socket. Security issues can arise if the tool:

- Lets a process that is not the owning user read from or write to the control
  socket
- Injects input, executes commands, or paints keys in response to data that did
  not come from the owning user's configuration, socket, or device
- Escalates privileges or requires more device access than documented
- Leaks the contents of profiles, environment, or window titles to a third
  party (logs, crash reports, network)
- Parses device reports, profile files, `.lp5` archives, or socket messages in a
  way that can crash the daemon or corrupt memory
- Verifies TLS incorrectly, or contacts a host that the configuration did not
  name, in `http` actions

If you discover any of the above, or any other security-relevant behaviour,
please report it.

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report privately through GitHub's Security Advisory feature on this repository:

https://github.com/qubernetic/keyvo/security/advisories/new

Please include:

- A description of the issue and its potential impact
- Steps to reproduce, or a scenario demonstrating the problem
- Your distribution, desktop environment, and `keyvo --version` output
- Which devices are attached and how (USB, Bolt, Bluetooth), if relevant

## Response timeline

- **Acknowledgement**: within 72 hours of receiving your report
- **Initial assessment**: within 7 days
- **Resolution or mitigation**: targeted within 30 days, depending on severity

We will keep you informed of progress and credit reporters in the fix unless
anonymity is requested.

## Supported versions

keyvo is pre-release. Until the first tagged release, only the `develop` branch
and the latest pre-release build receive security fixes. After `v0.1.0`, only
the latest released version is supported.

## Threat model

The following properties are by design. Reports that only restate them are
appreciated but will be closed as "working as intended".

### Trust boundary: the user session

keyvo runs as an unprivileged process inside one user's desktop session. Its
trust boundary is the same as `ssh-agent`: everything that runs as the same
user is trusted, and nothing else is.

- The control socket lives at `$XDG_RUNTIME_DIR/keyvo/keyvo.sock` with mode
  `0600` in a directory owned by the user. Any process running as that user
  can send any command, including switching profiles, painting keys, and
  triggering actions. This is the intended scripting surface.
- The daemon creates the socket's directory with mode `0700` and refuses to
  start when the directory is not the user's own with that mode, or when
  another daemon already answers on the socket. A socket file left behind
  by a stopped daemon is replaced. Anything else in its place is not
  touched.
- The clients check the daemon the same way from their side: `keyvo ctl`
  and the app refuse a socket whose directory is not private to the user
  and a socket another uid listens on (`SO_PEERCRED`), before they send a
  request. Another account cannot pose as the daemon, for example in the
  `keyvo-<uid>` fallback under the shared temporary directory.
- A socket request with an unknown command or field is rejected before
  anything runs, so a mistyped `dry_run` never executes a key's action.
  Requests are parsed as untrusted input; a malformed line gets an error
  response and never ends the daemon.
- There is no authentication layer on the socket beyond file permissions, and
  none is planned for v1.
- On KDE the daemon loads its own focus script into KWin
  (`contrib/kwin/keyvo-focus.js`, `docs/spec.md` §6.2). The script is
  written to `$XDG_RUNTIME_DIR/keyvo/` (or `keyvo-<uid>` in the temporary
  directory), and the daemon refuses a directory that is not its user's
  with mode `0700`, because the file runs inside the compositor. The
  directory is checked and the script rewritten before every load, also
  after a KWin restart, so KWin never reads a file another account could
  have placed or changed. The script calls a private object on the session bus; any process of the
  same user can call that object too and so assert a focused window, which
  is no more than `notify_context` on the socket allows.
- The daemon never runs as root. Device access comes from a udev rule that
  grants the user (or the `input` group) access to the Keypad's `hidraw` nodes
  and to `/dev/uinput`. Running the daemon with elevated privileges is
  unsupported and reduces isolation.

### The desktop app

- The app (`app/`) is a client of the socket like `keyvo ctl` and has no
  other way into the daemon. Its window may call the app's six own
  commands and listen to their events; the capability names each of them
  and grants no `core:default` and no plugin, so a script injected into the
  page could not reach the filesystem, shell, window or other core APIs.
  The content security policy loads only the app's own code and connects
  only to the Tauri IPC (`docs/spec.md` §10).
- Both app lockfiles are checked in CI: `app/src-tauri/Cargo.lock` by
  `cargo deny` with `app/src-tauri/deny.toml` (the root policy plus
  documented exceptions), `app/package-lock.json` by `npm audit
  --audit-level=high`. Dependabot watches both.

### Actions run what the user configured

- `exec` actions run arbitrary commands from the user's profile files under the
  user's own account, with `KEYVO_*` environment variables describing the key,
  profile, page, and focused window. This is the feature, not a bug. Profile
  files are as sensitive as shell rc files and should be treated the same way.
- `chord` actions inject keystrokes through `uinput`. Injected input goes to
  whatever window has focus at that moment.
- `http` actions make outbound requests to the URL written in the profile.
  keyvo has no inbound network listener in v1; nothing on the network can reach
  the daemon.

### The project layer is the first foreign input

A repository may carry a `.keyvo/` directory (`docs/spec.md` §7.4, ADR-0009).
Unlike everything else keyvo reads, the user did not necessarily write it: it
arrives with a clone or a pull. keyvo treats it as untrusted data until the
user says otherwise.

- The layer is parsed and rendered as soon as it is found, so the pad shows
  what the project offers, but none of its `exec`, `http`, `chord`, `text` or
  `recipe` actions runs until the project is trusted. A press on an untrusted
  key only writes a trace entry with the block reason.
- Trust is recorded in `~/.config/keyvo/trust.toml` against the SHA-256 of the
  three files' contents. Any change, including a pull, invalidates it and the
  prompt is shown again. `keyvo project trust` and the GUI prompt list every
  action and every overridden key before asking; `keyvo project revoke`
  withdraws trust.
- `[project_layer]` in `config.toml` limits what a project may touch: keys,
  action types (`profile` and `mode` are never allowed from a project) and app
  profiles. Setting `trust = "always"` disables the gate; `keyvo doctor`
  reports it as a warning and the setting is not offered by the GUI.
- Starter packs and imported profile bundles pass the same gate when they
  contain `exec`, `http` or `recipe` actions.
- A trusted project's actions run with the user's rights, like any other
  profile action; reviewing `.keyvo/` before trusting it is the same duty as
  reviewing a `.envrc` or a `justfile` from a clone.

### Plugins are user-installed processes

Plugins under `~/.config/keyvo/plugins/` are executables the user chose to
install. The daemon supervises them (start, restart, stop) and exchanges NDJSON
with them over pipes. A plugin has the same rights as any other process of the
user; keyvo does not sandbox it. Install plugins from sources you trust, as you
would a shell script.

### Configuration files are user data

`~/.config/keyvo/` is expected to be readable and writable only by the user.
keyvo warns in `keyvo doctor` if the directory or the socket directory is more
permissive than that, but does not refuse to start.

### Device input is untrusted

Reports from the Keypad, Dialpad, and Bolt receiver, and the contents of
imported `.lp5` archives, are parsed as untrusted input. A malformed report or
archive must never crash the daemon or cause undefined behaviour. Bugs in this
area are in scope and welcome.
