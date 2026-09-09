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
- There is no authentication layer on the socket beyond file permissions, and
  none is planned for v1.
- The daemon never runs as root. Device access comes from a udev rule that
  grants the user (or the `input` group) access to the Keypad's `hidraw` nodes
  and to `/dev/uinput`. Running the daemon with elevated privileges is
  unsupported and reduces isolation.

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
