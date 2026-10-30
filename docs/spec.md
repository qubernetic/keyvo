# keyvo Software Specification (v0.3)

> Context-aware, scriptable control-surface daemon, CLI and desktop app for the
> Logitech MX Creative Console family (Keypad, Dialpad) and related HID++
> devices. Linux-native first (KDE Wayland, X11 fallback); Windows via a
> Logi Actions SDK plugin and an optional native mode.
>
> Status: v0.3, UX-revised. Supersedes v0.2. Decisions were taken in the
> planning grill of 2026-09-09 (`docs/plans/2026-09-09-project-plan.md` §1)
> and the UI/UX grill of 2026-10-24
> (`docs/plans/2026-10-24-gui-ux-grill-decisions.md`), and are recorded in
> `docs/adr/`; the desktop app screens and flows are in
> `docs/design/gui-ux.md`. Protocol
> statements come from the reports in `docs/research/` and are marked
> *measured* only when observed on the author's hardware; everything else
> is *reported* by prior art or *captured* from Options+ traffic and must be
> confirmed in M0. Items marked **[OPEN]** are undecided.

---

## 1. Vision and goals

**Problem.** Logitech ships no Linux support for the MX Creative Console.
Community work has mapped the wire protocol (Julusian's
`@logitech-mx-creative-console`, logilinux, shensquared's macOS probes,
hcooper's Options+ captures, notno's Rust daemon) and two projects run the
Keypad on Linux today: logimap (Python, per-app profiles on Plasma 6 Wayland)
and Bitfocus Companion's surface module (broadcast-style control). No mature,
scriptable solution exists: nothing offers a developer macro pad with
profiles that follow the focused application, locally rendered LCD icons,
overlays driven by external tools, and a documented control API that
scripts, editors and coding agents can use.

**Goal.** A single daemon + CLI + desktop app that turns the Keypad (and
later the Dialpad, mouse, keyboard) into a context-aware developer control
surface on Linux, installable as one static binary plus a udev rule, with a
Windows story that coexists with Logi Options+.

**Primary user.** The author (developer, KDE Plasma on Aurora DX, uses
VS Code, Claude Code in a terminal, a browser, and a mail client). Secondary:
Linux developers with the same hardware; Stream Deck refugees. keyvo targets
developers only; see §10.

**Non-goals (v1).**
- Reimplementing Logi Options+ or running it under Wine/VM.
- Replacing Solaar for mouse/keyboard configuration. Solaar is *called*
  (optional, for desk state); its GPL code is never copied.
- Cloud sync, accounts, telemetry.
- macOS support (may come for free via the Actions SDK plugin; not a target).
- Haptics on the MX Master 4.
- Embedded scripting language (Lua), see §8 and ADR-0003.

---

## 2. Hardware in scope

| Device | Connection | v1 | Notes |
|---|---|---|---|
| MX Creative Keypad (3×3 LCD keys + 2 page buttons), USB `046d:c354` | USB-C | **yes** | Two protocol planes (§5). Keep-alive mandatory. HID++ 4.5, firmware `U1 66.00.B0017` on the author's unit. |
| MX Creative Dialpad (Bolt WPID `BC00`, Bluetooth PID `046d:bc00`) | Bolt receiver `046d:c548` / Bluetooth | M3 | HID++ 2.0 over the receiver's hidraw: 4 divertable buttons on `0x1B04` (CIDs `0x0053/0x0056/0x0059/0x005A`), dial and roller on `0x4610` MultiRoller. No keep-alive feature. The Bolt path is untested by anyone; Bluetooth is the fallback. |
| MX Master 4 (Bolt WPID `B042`) | Bolt receiver | via Solaar (optional) | DPI, SmartShift, button divert/remap; haptics (`0x19B0`) deferred past v1. |
| MX Mechanical (Bolt WPID `B366`) | Bolt receiver | via Solaar (optional) | Backlight mode/brightness/timeout, fn-swap. |
| Logi Bolt receiver `046d:c548` | USB-C | shared | Pairing handled by Solaar or Logitech tools, not keyvo. keyvo opens the receiver's hidraw non-exclusively and filters by device index. |
| MX Keypad (launched 2026-09-08, Windows/macOS only) | USB-C | **[OPEN]** | Nine LCD keys, "builds on" the MX Creative Keypad. PID and firmware family unknown until an owner publishes `lsusb` / `solaar show`. keyvo queries geometry and features at runtime so a compatible unit works without code changes; a differing PID is one table entry. |

Out of scope but architecturally allowed: Elgato Stream Deck (the
`HidTransport` abstraction must not preclude it).

---

## 3. Platforms and packaging

| Platform | Delivery | Mode |
|---|---|---|
| Linux (KDE Plasma / Wayland first) | Standalone static binaries `keyvo`, `keyvo-daemon` via cargo-dist (tar, deb, rpm), plus a udev rule, a systemd user unit and a KWin script under `contrib/`. Flatpak on Flathub in M4b. | native |
| Linux (X11, other Wayland compositors) | same | native, reduced context detection (§6.2) |
| Windows 10/11 | MSI (cargo-dist) + Logi Marketplace plugin (`.lplug4`, C#) | plugin (default) or native (M5) |

The standalone daemon is the primary install path. Flatpak cannot deliver
hidraw or `/dev/uinput` permissions on its own, so a udev rule is required
regardless of packaging; keyvo plans to upstream a `046d:c354` `uaccess`
rule to systemd's hwdb (the Boatswain model) before the Flathub submission.

**As built (beta, #182).** The Flatpak `io.github.qubernetic.keyvo`
(`contrib/flatpak/`) ships the desktop app (`keyvo-app`, the command) and
the `keyvo` CLI on `org.gnome.Platform//51`; the daemon stays on the host
and keeps everything that touches the machine (hidraw, `/dev/uinput`, the
KWin script, logind). The manifest builds offline the way Flathub builds:
the `rust-stable` and `node24` SDK extensions of the runtime's freedesktop
branch, the vendored crates of both `Cargo.lock` files in
`cargo-sources.json` and npm's cache of `app/package-lock.json` in
`node-sources.json`, both generated by the pinned flatpak-builder-tools
generators (`tools/flatpak/`, `just flatpak-sources`; CI fails on a stale
file). The one application id is the manifest's, the desktop file's, the
metainfo's, the icons' and the Tauri identifier; the D-Bus interface stays
`org.qubernetic.keyvo.Focus`. `just flatpak` builds and installs it on the
host; the CI `flatpak` job builds the bundle `keyvo.flatpak`. The Flathub
manifest (M4b-2) is a copy with a `git` source.

**As built (beta, #184).** The udev rule `contrib/udev/70-keyvo.rules`
tags the Keypad's hidraw nodes (`046d:c354`), the Bolt receiver's HID++
node (`046d:c548`, the Dialpad's path; matched on the immediate HID
parent's `HID_ID` and USB interface `02`, #197) and `/dev/uinput` (a
`static_node`, so the tag applies before the module loads) with
`uaccess`; named `70-` so it sorts before `73-seat-late.rules`, where
logind turns the tag into an ACL for the user at the active seat. No
mode, no group. The receiver's HID++ node carries only the HID++ reports
`0x10` and `0x11` of the paired devices: nothing while no control is
diverted, the Dialpad's `0x1B04` and `0x4610` reports while keyvo
diverts them, no keyboard or mouse input; its other three nodes, which
the paired devices share (keyboard input on interface `00`, mouse input
and the undiverted Dialpad on `01`, a digitizer on `03`), stay closed
(measured 2026-10-29 and 2026-10-30, `docs/hardware-notes.md` record
D8; §5.1). The HID++ node still reaches every paired device, so the
user at the seat can configure the keyboard and the mouse over it as
Solaar does (`SECURITY.md`). Install it with
`sudo install -m 644 contrib/udev/70-keyvo.rules /etc/udev/rules.d/ &&
sudo udevadm control --reload && sudo udevadm trigger`, then replug the
Keypad. The daemon runs under the systemd user unit
`contrib/systemd/keyvo-daemon.service` (§8) from `~/.local/bin`, and
`keyvo doctor` (§5.10, §12) checks both. The cargo-dist packages and the
hwdb upstreaming stay after the beta.

Immutable-distro constraints (Aurora DX, Fedora Atomic, Bazzite): nothing
may require writing to `/usr`. Allowed locations: `/etc/udev/rules.d/`,
`~/.config/systemd/user/`, `~/.local/share/kwin/scripts/`, `~/.local/bin`.

---

## 4. Architecture

Monorepo. Rust workspace for the core; separate packages for the GUI, the
integrations and the Logi plugin.

```
keyvo/
├── crates/
│   ├── keyvo-hid       # HidTransport impls, HID++ 2.0 core (root, feature set, 0x0008, 0x1B04,
│   │                   #   0x8040, 0x4610), VLP display plane (0x13/0x14, 0x19A1), Keypad driver,
│   │                   #   Dialpad driver (Bolt receiver node, slot search, 0xff over Bluetooth),
│   │                   #   hot-plug, keep-alive task, ACK-driven image writer
│   ├── keyvo-core      # profiles (TOML, toml_edit round-trip, JSON Schema), state machine
│   │                   #   (mode/page/overlay), renderer (tiny-skia/resvg/Inter → JPEG), actions,
│   │                   #   socket protocol types, chord parser; sync, hardware-free, ≥85 % covered
│   ├── keyvo-daemon    # tokio runtime: socket server, KWin watcher (zbus), uinput injector
│   │                   #   (evdev), inotify hot-reload; planned: X11 watcher (x11rb), plugin
│   │                   #   supervisor
│   ├── keyvo-client    # socket client, mirror state, tile rendering through keyvo-core, key
│   │                   #   editor file side, for the GUI and later clients; sync, hardware-free
│   └── keyvo-cli       # `keyvo probe|keys|dial|fill|preview|ctl|doctor`; planned: `paint-test|profile`
├── app/                # Tauri 2 + Svelte 5 GUI (M4a); src-tauri/ is outside the root workspace
├── integrations/       # planned: claude-code/ (plugin: hooks, commands, MCP), vscode/, tmux/, nvim/ …
├── plugins/logi/       # planned: C# Logi Actions SDK plugin (M5), built in the Windows VM
├── contrib/            # KWin script, example profiles, flatpak/ (the manifest and its metadata), systemd/ (the user unit), udev/ (the uaccess rule)
├── captures/           # README + derived fixtures; raw pcapng external (release assets)
├── tools/keycodes/     # generator of the evdev key code table of the chord parser and the app's key names
├── tools/lucide/       # generator of the bundled lucide icons and their table
├── tools/flatpak/      # pin and wrapper of the flatpak-builder-tools generators of the Flatpak's offline sources
├── tools/windows-vm/   # planned: libvirt VM scripts (from TwinCat-MCP)
├── schemas/            # generated JSON Schemas of profiles and config.toml
├── tests/protocol-fixtures/  # shared fixtures for Rust/TS clients: socket/ NDJSON, editor/, hid/ golden HID++ scripts
└── docs/               # spec.md, roadmap.md, socket-api.md, adr/, design/, research/, plans/,
                        #   hardware-notes.md, pre-alpha-test.md, beta-test.md; planned: diagrams/, mdBook
```

"Planned" marks what the pre-alpha has not built yet; the milestone that
builds it is in §15 and `docs/roadmap.md`.

The root workspace has five crates. `keyvo-core` and `keyvo-client` are
synchronous and hardware-free; `keyvo-daemon` owns the tokio runtime and
every OS binding. `keyvo-client` keeps socket I/O out of `keyvo-core`: the
desktop app's Rust side (`app/src-tauri`, a crate outside the root
workspace because it needs webkit2gtk, compiled by the CI `app` job) is thin
glue over it, so the GUI's logic is tested without a display.

### 4.1 Platform abstraction (traits in `keyvo-core`, implemented per OS)

```rust
trait HidTransport   // open/read/write HID reports; hidraw on Linux, hidapi elsewhere; FakeHidTransport in tests
trait InputInjector  // Linux: uinput (evdev); Windows: SendInput
trait WindowWatcher  // Linux: KWin script over D-Bus (KDE), X11 _NET_ACTIVE_WINDOW; Windows: SetWinEventHook
trait SessionWatcher // Linux: systemd-logind on the system bus; Windows and X11 sources later
trait Lineage        // Linux: /proc session id and parent chain; Windows: job object (M5); X11 pid through XRES
```

`HidTransport` (`keyvo_core::hid`) moves whole reports, report id first:
`write(report)` and `read(timeout) -> Option<report>`, where `None` is a
timeout. It knows nothing about HID++; framing, reply matching and feature
resolution live in `keyvo-hid`. `FakeHidTransport` replays a script of
request/response pairs in the fixture notation of §5.9 and fails on any
write the script does not expect.

The test doubles (`FakeHidTransport`, `FakeClock`, `FakeInputInjector`,
`FakeWindowWatcher`, `FakeSessionWatcher`) are public items of `keyvo-core`, not behind a
feature: the tests of `keyvo-hid`, `keyvo-daemon` and `keyvo-cli` use
them, and they hold no device or system resource, so shipping them costs
nothing. They are for tests; no production path constructs one.

Timing logic (keep-alive cadence, later long press and hysteresis) reads
time through the `Clock` trait (`keyvo_core::clock`): `SystemClock` in
production, `FakeClock` in tests, which moves only when a test advances it.
No timing logic in `keyvo-core` or `keyvo-hid` depends on `tokio`; the
daemon decides how to wait.

`InputInjector` (`keyvo_core::inject`) takes a chord's key events in press
and release order: `send(&[KeyEvent])`. The Linux implementation in
`keyvo-daemon` is a uinput keyboard named `keyvo`, created with `evdev`,
that declares every key code of the chord table and emits each edge as its
own report. Without a usable `/dev/uinput` the daemon still paints and runs
`exec` actions; chord presses fail with the reason in their trace record.
`FakeInputInjector` records the events in tests.

`WindowWatcher` (`keyvo_core::focus`) reports the focused window as a
`WindowContext` (`class`, `title`, and `pid` when the source knows it):
`start(sink)` hands it a callback that
it calls from any thread on every focus change until the watcher is
dropped; a watcher that knows the focused window reports it at once. A
watcher that cannot start returns the reason, and the daemon logs
"focus switching off" and runs on with the page buttons. The daemon
binary uses the KWin watcher (`keyvo_daemon::kwin::KWinWatcher`, §6.2);
`FakeWindowWatcher` lets a test report a window by hand.

`SessionWatcher` (`keyvo_core::session`) reports the lock state of the
user's session as `Session::Locked` or `Session::Unlocked`: `start(sink)`
the same way, a watcher that knows the state reports it at once, so a
daemon started under the lock starts locked. A watcher that cannot start
returns the reason, and the daemon logs "lock detection off" once and runs
on without it. The daemon binary uses the logind watcher
(`keyvo_daemon::logind::LogindSessionWatcher`, §6.2); `FakeSessionWatcher`
lets a test lock and unlock by hand, `FakeSessionWatcher::starting` reports
a state at the start.

`Lineage` (`keyvo_core::lineage`, #179) answers whether a process belongs
to a launch: `descends(pid, launch)` for a window's `pid` and the pid of
a launch's shell, which leads its own session. The daemon binary uses
`keyvo_daemon::lineage::ProcLineage` over `/proc` (§6.3); `FakeLineage`
answers the pairs a test gives it, `NoLineage` knows no process. The
daemon takes the two watchers and the lineage as
`keyvo_daemon::daemon::Sources { window, session, lineage }`.

Clients implement none of the five traits: they reach the device only
through the daemon's socket. `keyvo-client` is that client side, used by the
desktop app: `Client` sends one request at a time over any `Read + Write`
stream (`Client::connect` for the Unix socket at `socket_path`:
`$KEYVO_SOCKET`, else the runtime directory) and keeps events that arrive
before an answer for `next_event`; `Client::subscribe` subscribes to every
event class with a trace replay and `Client::dry_run` sends a simulated
press and returns its trace entry; `follow` keeps a subscription to its end
and hands on `Update`s (the subscribed view, every event, then why it
ended, also when the daemon cannot be reached), which the app's shell emits
to the frontend as they are; `Mirror` folds the `status`, `subscribe`
and `press` replies and the events into the view, device, focus context,
held keys, recent trace entries and the last reload error; `Tiles` renders a
page with the `keyvo-core` renderer, so a tile in the GUI has the pixels the
daemon paints on the LCD (for a locked view the lock page, §9), and
`tiles::keys` gives each key's label and action for the GUI to announce
(none for a locked view); `Mirror` takes a `session` event into the view's
`locked`. Its tests run against the socket fixtures in
`tests/protocol-fixtures/socket/`, the same lines the TypeScript side of
the app checks itself against. `Editor` is the key editor's file side
(§7.1, §10): `Editor::key` reads a key's file and fields, `Editor::set_key`
writes new fields through `keyvo_core::edit` after a snapshot, atomically,
and returns the fields before the write for undo; `Problem` is an error as
the GUI shows it, with file, line and column when it has a place. Its tests
replay the key editor steps in `tests/protocol-fixtures/editor/`, which the
app's key editor tests use too.

The daemon as built (pre-alpha, `keyvo-daemon`): one tokio controller task
owns the profile state and the per-key feedback; one device actor thread
owns the Keypad's transport (§5.1, §5.5); `exec` actions run as their own
tasks (§8); an inotify watcher reports changes in the configuration
directory (§7.2); the control socket serves each client in its own task
and hands the commands to the controller (§8). Its end-to-end tests drive
the controller with `FakeHidTransport` pads, a `FakeInputInjector`, a
`FakeWindowWatcher`, a `FakeSessionWatcher`, socket clients and a
configuration directory on disk. Run it with `just build`, then `bin/keyvo-daemon [--config-dir DIR]
[--device PATH] [--dialpad-device PATH] [--socket PATH]` on the host;
logs go to stderr.

Core rendering is, as the target, the pure function

```
(profile, page, mode, overlay_stack, window_ctx) → 9 KeySpecs + accent + dialpad bindings
```

so the whole context engine is testable without a device. As built
(pre-alpha) it is `keyvo_core::render::page(stack, keys)`: the specs of
keys 1 to `keys` of one page over the global layer
(`keyvo_core::stack::Stack`, §6.1); the state machine picks the profile
and page, overlays are not built yet, and the accent is read from the
profile on top. The dialpad bindings (beta, #188) are the profile's
`[dialpad]` table over the global one, `keyvo_core::dialpad::Bindings`
(§7.2); they belong to the profile, not to a page.

### 4.2 Device ownership (Windows coexistence)

Daemon holds `owner ∈ {none, native, options_plus}`. While owner is not
`native`, keyvo **never** writes to LCDs or injects input; it only
listens. Detection: presence of `LogiOptionsPlus.exe` / `LogiPluginService.exe`.
The user is told explicitly; Options+ is never stopped on their behalf.
Whether Windows exclusive-opens the vendor collection while Options+ runs is
measured in M0-2 (plan §5 item 16).

---

## 5. Device layer (`keyvo-hid`)

### 5.1 Enumeration and ownership

- Enumerate via udev/sysfs by VID/PID; select the hidraw node by its report
  descriptor (the node that carries report `0x14`), never first-match. The
  kernel binds `hid-generic` to `c354`, so the node is clean of HID++ driver
  quirks.
- keyvo's search (`keyvo_hid::sysfs::find_keypad`): every
  `/sys/class/hidraw/<node>` whose `device/uevent` carries `HID_ID=<bus>:
  <vendor>:<product>`, ordered by node number; the report ids are the
  Report ID items (`0x85`) of `device/report_descriptor`, read with a short
  and long item walk. Among the nodes of `046d:c354` the one that carries
  `0x14` wins; none of them carrying it is an error that lists the nodes
  and their reports, and no node at all is "not found". `keyvo --device
  <path>` skips the search. Linux creates one USB interface and one hidraw
  node for the Keypad, carrying all five reports `04 05 11 13 14` (measured,
  first light, `docs/hardware-notes.md` item 1); the search by `0x14` stays
  so a split topology is still handled. The sysfs fixtures carry the
  captured 140-byte descriptor.
- The Bolt receiver as built (beta, #186; `keyvo_hid::sysfs::find_receiver`):
  among the nodes of `046d:c548` the first, by node number, whose
  descriptor carries both HID++ reports `0x10` and `0x11`. The receiver
  has four USB interfaces with one node each, keyboard, mouse, HID++ and
  digitizer, and on the owner's host the kernel does not run it in DJ
  mode, so no paired device gets a node of its own: every paired device
  shares the receiver's interfaces (measured 2026-10-29 and 2026-10-30,
  `docs/hardware-notes.md` record D8; this corrects the earlier text,
  which had `hid-logitech-dj` give every paired device a node). keyvo opens it
  read-write, non-exclusive, like the Keypad's, and talks to a paired
  device by its device index next to the kernel and Solaar. The
  Dialpad's slot is found at runtime, never a constant
  (`keyvo_hid::dialpad`): for device index 1 to 6, `ROOT.getFeature(0x4610)`
  waiting at most `[timing].slot_probe`; a present feature makes the slot
  a candidate, and the candidate's `0x0003 getDeviceInfo` model id must
  carry `BC00` in one of its three product ids. The search stops at the
  Dialpad. A paired device's first reply from idle or sleep is slow over
  Bolt (measured on #187: 734 ms, 302 to 541 ms, and 180 to 294 ms or
  none within 300 ms for the three slots, 771 ms for a raw request to the
  sleeping Dialpad; 11 to 22 ms warm), so `slot_probe` defaults to
  1000 ms; an empty slot still settles at once on the receiver's error.
  With 1000 ms the slow slots answer inside the wait (measured
  2026-10-30: 791 ms and 819 ms; `docs/hardware-notes.md` record D3).
  The receiver delivers such a late reply twice, and every `getFeature`
  reply carries the same header, so the search tags its three requests
  per slot with software ids of their own, the three after the
  connection's (`0xf` for `getFeature(0x4610)`, `0x1` for
  `getFeature(0x0003)`, `0x2` for `getDeviceInfo` after keyvo's `0xe`):
  the copy of one reply carries the previous request's id and answers
  nothing; it is kept for the input path like any unsolicited report
  (#200; measured 2026-10-30: no copy matched in any run). The rest of the connection keeps its one software id: the
  Keypad is on USB without a receiver in between, and its golden scripts
  are captures with a fixed id. The receiver's HID++ 1.0 error report for a slot (`10 <index>
  8f <feature index> <fn|swid> <code> 00`, codes of the public Unifying
  receiver specification) answers the pending request at once
  (`HidppError::Receiver`): `0x04` connect fail, which a paired device in
  deep sleep answers at once, and `0x07` busy mark the slot silent like a
  timeout; `0x09` resource error, which the Bolt receiver answers for a
  slot with nothing paired, `0x08` unknown device and the other codes
  mark it empty (measured 2026-10-29 and 2026-10-30,
  `docs/hardware-notes.md` record D2; the static pass named `0x09`
  retryable, and the search followed it until #202). A
  device without `0x4610` and one with another model id are skipped. When
  no slot is the Dialpad, the caller waits for the receiver's arrival
  notification (`0x41`) with the link established for the Dialpad's WPID
  or a silent slot, at most `[timing].reconnect_interval`, then asks that
  slot alone (or every slot again after the wait). On the owner's
  receiver no `0x41` appeared: a waking Dialpad sends a `0x1D4B`
  reconnect report on its own device index instead, so the search round
  after the wait finds it (measured 2026-10-30, record D6; accepting the
  reconnect report as an arrival is an open follow-up). `--device` on a node
  sysfs knows as a Bluetooth Dialpad (`0005:046d:bc00`) reaches it on
  device index `0xff` with the same driver; any other `--device` node is
  taken as a receiver. The static pass's register `0x02` replay of the
  arrivals is not used yet.
- The Linux transport (`keyvo_hid::hidraw::HidrawTransport`) opens the node
  read-write, never exclusively. One `read` is one report (up to 4096
  bytes), its timeout a `poll`; `write` must take the whole report at once.
  `EIO`, `ENODEV`, `EPIPE`, an end of file and `POLLHUP` without queued data
  are a disconnect; queued reports are read before a hang-up is reported.
- Hot-plug via udev netlink; `POLLHUP`/`EIO`/`ENODEV` mean disconnect. On
  reconnect the driver re-runs the claim sequence (§5.4), re-diverts the page
  buttons (§5.6) and replays the last wanted frame set. Never sleep under a
  lock; one device actor owns the file descriptor and an mpsc queue.
- Reconnect as built (pre-alpha, `keyvo_daemon::device`): no udev watch
  yet. The device actor opens the node (`--device`, or the sysfs search
  above on every attempt, so a new node number after a replug is found),
  sends the first keep-alive ping, claims, diverts and reports the key
  count; then it paints and reads. Any transport, claim or divert error
  ends the connection; the actor reports it once (not on every failed
  attempt), waits `[timing].reconnect_interval` and opens again. A new
  connection forgets what the pad showed and repaints every key from the
  last wanted specs.
- Never exclusive-grab. Solaar and the kernel `hid-logitech-dj` driver may
  share the Bolt receiver; keyvo filters HID++ traffic by device index and
  matches replies on report id + device index + feature index +
  function/swid byte, never on "first inbound report". Reports that answer
  no pending request are kept for the input path, not dropped; the queue
  holds at most `UNMATCHED_CAP` (64) reports and drops the oldest past it,
  and a report that does not parse as a frame is dropped at once. Both
  kinds of drop are counted (`Hidpp::dropped_reports`). The input path
  reads the kept reports first (`Hidpp::next_frame`).
- HID++ 2.0 error replies arrive on the request's report with feature index
  `0xff`, followed by the request's feature index, its function/swid byte and
  the error code (`11 ff ff <index> <fn|swid> <code>`; codes from the HID++
  2.0 draft: 1 unknown, 2 invalid argument, 3 out of range, 4 hardware error,
  5 internal, 6 invalid feature index, 7 invalid function, 8 busy, 9
  unsupported). The VLP error layout is not captured: keyvo recognises the
  same `ff <index> <fn|swid>` prefix on `0x13` and reports the whole frame
  until a hardware check provokes one.

### 5.2 Two protocol planes on the Keypad (captured, hcooper)

| Report ID | Size | Role | Feature index space |
|---|---|---|---|
| `0x11` | 20 B | classic HID++ 2.0 long reports | the 18-feature table: index 4 = `0x0008 KEEP_ALIVE`, `0x0b` = `0x1B04 REPROG_CONTROLS_V4`, `0x0f` = `0x8040 BRIGHTNESS_CONTROL` (measured on the author's unit) |
| `0x13` | 32 B | "very long packet" (VLP) control channel with **its own root and its own feature index space** | VLP index 0 = VLP root, VLP index 2 = display feature `0x19A1` |
| `0x14` | 4095 B | VLP bulk channel for LCD image fragments | same VLP index 2 |

Byte 3 of every HID++ packet is `(function << 4) | swid`. VLP packets carry
a sequence/flag byte after it: the host sends `e0`, the device answers
`c0`, `c1`, … per fragment; unsolicited events use function byte `10`.
Feature indices differ per device (`0x1B04` is `0x0b` on the Keypad and
`0x0a` on the Dialpad): **always resolve indices via `ROOT.getFeature`**, on
both planes, and never hard-code them. The root sits at index 0 of each plane,
the one index the protocol fixes. Classic `getFeature(id)` answers `<index>
<type flags> <version>`, index 0 meaning absent; VLP root `getFeature(id)`
(captured) echoes the id first: `<id:u16> <index> <hidden> <version>
<maxMemory:u32>`, e.g. `19 a1 02 00 00 00 01 e8 03`; keyvo takes a VLP
index 0 as absent too (not captured; `keyvo probe --feature`, §5.10, is
the check). Resolved indices are
cached per connection, including those a direct `getFeature` call
returned. Host requests on `0x13` that fit one packet carry the
sequence byte `e0`. Bit `0x40` of the sequence byte marks the last packet of
a message (`e0`, `c0`); a reply over several packets starts without it
(display fn1: `80`, `01`, `02`, `43`), and keyvo joins the packets'
parameters up to the last one, all within the one reply timeout
(`[timing].reply_timeout`, §6.1).

The firmware version (`keyvo_hid::firmware`, classic `0x0003
DEVICE_FW_VERSION`, captured): `fn0 getEntityCount` answers the count
first; `fn1 getFwInfo(n)` answers `<type> <prefix:3 ascii> <number>
<revision> <build:u16> <active> <transport pid:u16>`, number, revision and
build in BCD, shown as Solaar does (`U1 66.00.B0017`). Type 0 is the
application, 1 the bootloader, 2 the hardware; the Keypad also lists a
type `0x0b` entity (`IM`). The active application entity is the firmware
version `docs/hardware-notes.md` records.

### 5.3 Keep-alive (mandatory)

- The Keypad exposes HID++ `0x0008 KEEP_ALIVE` (measured). Prior art
  reports that without pings the device delivers no key input and reverts
  to the logo (shensquared, mx-console-suite, hcooper). First light on
  firmware `U1 66.00.B0017` measured otherwise (`docs/hardware-notes.md`
  items 2 and 3): without pings key input arrives and a paint stays (still
  `a0` 120 s after the paint). The revert to the logo is the `KEEP_ALIVE`
  expiry, not a display-side watchdog: it comes the granted timeout after
  the last ping (3.001 s with 3000 ms, 0.501 s with 500 ms, 10.001 s with
  10000 ms), so only a client that has pinged can lose its claim this way.
  keyvo still pings from the moment the device opens; the expiry is what
  reverts the panel when keyvo stops or stalls.
- Options+ calls `fn0 getTimeoutRange` → `500..10000 ms`, then `fn1
  keepAlive(3000)` and repeats it every ~1.01 s (captured). keyvo runs a
  keep-alive task with the same defaults from the moment the device opens,
  before any read; period and requested timeout are configurable under
  `[timing]`. Repainting is not a substitute (it flickers).
- The scheduler (`keyvo_hid::keep_alive::KeepAlive`) is driven by the
  `Clock` trait (§4.1): the first ping is due at once, each next one a
  period after the previous due time, so the cadence does not drift; after a
  stall longer than a period it sends one ping and restarts from then
  instead of catching up with a burst. A failed ping stays due. The device
  echoes the granted timeout.
- A ping has to arrive while the timeout of the one before still runs, so
  `keep_alive_period` stays below `keep_alive_timeout` (§6.1). First light
  on firmware `U1 66.00.B0017` (#40): with period and timeout both 3000 ms
  the timeout ran out 3.000 s after the first ping and the claim was lost,
  although the device kept answering the pings.
- **Lost claim and recovery.** When the timeout runs out, the Keypad sends
  the `KEEP_ALIVE` expiry event (`11 ff 04 00`, function 0, software id 0)
  and then the display mode-change event to `a1` (`13 ff 02 10 cb a1`),
  0 to 4 ms apart (measured, every expiry of first light), and the panel
  shows the logo; it stays there until a host sends `setMode(a0)` again. `keyvo_hid::display::ClaimWatch` treats
  either report, and a mode-change event to any other mode than `a0`, as a
  lost claim: it re-sends `setMode(a0)`, confirms it with `getMode`, and
  drops the mode-change and expiry events read in the meantime, so one
  expiry gives one reclaim. The caller then repaints: the daemon's device
  actor paints every key of the current state again (`Event::Reclaimed`,
  then `Painted`), `keyvo fill --hold` paints its image again, `keyvo keys`
  only reclaims. Fixtures: `claim-lost.txt` and `display-reclaim.txt` in
  `tests/protocol-fixtures/hid/`.
- The firmware self-dims after idle or USB suspend while still accepting
  writes; the daemon re-asserts brightness after resume, after reconnect and
  on wake events (§5.7). The revert timing is measured (above); the dim
  timing is deferred past the pre-alpha (plan §5 item 11).

### 5.4 Display claim and geometry

- Startup (captured): VLP root fn1 (version info), VLP root fn2 with the
  eight values `0x5e..0x65` (meaning unclear; not needed, below),
  `getFeature(0x19A1)` → VLP index 2, display fn3 (mode list
  `a0 a1 a2 a4 a5 a6 d0..d6`), display fn5 get current mode (`a1` in the
  capture), display fn4 `setMode(a0)` = host-controlled display,
  acknowledged by an unsolicited mode-change event.
- Display modes (measured, first light, `docs/hardware-notes.md` item 5):
  after plugging in, and on a Keypad plugged in since boot and idle, the
  mode is `a2`, not `a1`. `a1` is the device/logo mode a `KEEP_ALIVE`
  expiry leaves (§5.3); the capture's `a1` is that state. `a0` is the
  host-controlled mode `setMode(a0)` sets; without pings it stays.
- Plug-in splash (observed, #52, `docs/hardware-notes.md` item 5): for a
  few seconds after a plug-in the display is in `a4` (SplashBright) and
  `setMode(a0)` does not take; `getMode` after it still answers `a4`. The
  daemon log of the replug showed `a4` about three seconds after the
  plug-in and the profile painted about seven seconds after that. The
  `a2` of the item above is the state about one minute after a replug,
  when the splash is over.
- Display state names (the Options+ state enum, read in the display
  pipeline report, `docs/research/options-plus-display-pipeline.md`
  section 6; it corrects the inventory's mapping of `a3` to `a6`):

  | Value | Name | Value | Name |
  |---|---|---|---|
  | `a0` | Streaming (host-controlled) | `d0` | Unknown |
  | `a1` | Standby | `d1` | USB Preconfiguration |
  | `a2` | StandbyBright | `d2` | Idle |
  | `a3` | Splash (not in this device's list) | `d3` | LowPower |
  | `a4` | SplashBright | `d4..d6` | TDE embedded image, TDE embedded animation, TDE pattern |
  | `a5` | OnboardingBright | | |
  | `a6` | Dfu | | |

  The `a*` states are the ones a host sets; the `d*` states are reported
  by the device. ClearScreen is not a command of the feature: Options+
  clears the panel by writing a black 480×480 JPEG at (0, 0) through fn2
  (§5.5), and never does so on its redraw path. keyvo does not need it.
- keyvo's claim (`keyvo_hid::display::claim`, after the keep-alive has
  started): VLP root fn1, `getFeature(0x19A1)`, display fn3 (refused unless
  the list holds `a0`), display fn5, display fn1 geometry, then, unless the
  display already reports `a0`, fn4 `setMode(a0)` confirmed by a second fn5
  read, as Options+ does. The mode-change event (`13 ff <display> 10 <seq>
  a0`, function 1, swid 0) arrives during that read in the capture; keyvo
  consumes it, waiting at most the reply timeout, and records whether it
  came. A missing event is not an error, a mode other than `a0` after
  `setMode` is, with one exception: `a4` is the plug-in splash, and the
  claim waits it out. It waits `[timing].claim_splash_retry` for the
  mode-change event to `a0` (which ends the wait), then sends
  `setMode(a0)` again and confirms it with fn5, until fn5 reports `a0` or
  the retries that fit into `[timing].claim_splash_timeout` are used up
  (§6.1). The keep-alive keeps pinging between the retries. Only a display
  still in `a4` after the timeout, or in any mode other than `a0` and `a4`
  after `setMode(a0)` (at once, without a wait), fails the claim with
  `display stayed in mode 0x<mode> after setMode(a0)`. Mode-change events
  the splash sent before `a0` are dropped once `a0` is confirmed, so they
  do not look like a lost claim (§5.3). The daemon logs a splash it waited
  out at `debug` and reports no disconnect for it; `keyvo keys` and
  `keyvo fill` wait the same way with the user's `[timing]`. VLP root fn2
  is left out: painting works without it (measured, first light).
- **The device publishes its own key geometry** via display fn1
  `getGeometry(01)` (Options+ calls it `getDisplayInfo(1)`), answered over
  four packets. The joined reply: byte 0 `displayShape` (u8, 1), bytes 1..2
  `dimension` (u16 BE, `0x0060` = 96), bytes 3..4 and 5..6 the horizontal
  and vertical resolution (u16 BE, 480×480), byte 7 the button count (9),
  byte 8 the visible-area count (0), then one 9-byte record per button and
  after them one per visible area: shape (u8, 1), x, y, w, h (u16 BE each)
  (field names from the display pipeline report,
  `docs/research/options-plus-display-pipeline.md` section 0). The
  author's Keypad (firmware `U1 66.00.B0017`,
  measured on first light, `docs/hardware-notes.md` item 6) gives w = h =
  118 and these origins:

  | Key | Origin | Key | Origin | Key | Origin |
  |---|---|---|---|---|---|
  | 1 | (24, 6) | 2 | (181, 7) | 3 | (340, 6) |
  | 4 | (23, 164) | 5 | (181, 164) | 6 | (340, 164) |
  | 7 | (23, 321) | 8 | (181, 322) | 9 | (339, 321) |

  close to, but not identical with, Julusian's `23 + col*158`. The hcooper
  capture differs on five keys: (182, 7) for key 2, (181, 163) for key 5,
  (339, 164) for key 6, (181, 321) for key 8, (340, 321) for key 9; the
  table varies between units or firmware. keyvo numbers the keys by their
  position in the table, rejects a table shorter than its key count or
  with a key outside the panel, queries it on every claim and uses it for
  placement; nothing about the grid is hard-coded. This is what makes the
  2026 MX Keypad a runtime question rather than a code change. Without a
  device, `keyvo preview` uses the capture's table
  (`display::captured_geometry`).
- The visible key window is not the published rectangle: after a panel
  paint, a key paint leaves a strip of the panel colour at the bottom and
  right edge of the key (first light, `docs/hardware-notes.md` item 7).
  The daemon paints the panel background before the keys (§5.5, #77), so
  the strip shows the profile's background, not an earlier paint. Photos
  cannot measure the strip (the domed key caps act as a lens, the strip
  changes with the viewing angle). Options+ avoids the strip by painting
  the background and the keys as one composite image and a single key as
  a crop of the same canvas (display pipeline report,
  `docs/research/options-plus-display-pipeline.md`, "What changes the
  plan" item 1); #72 adopts that, which makes a per-key margin
  unnecessary, and #70 checks the result on the device
  (`docs/pre-alpha-test.md` section 4). `keyvo fill --all --margin <M>`
  (§5.10) stays as a diagnostic that grows every key rectangle by M
  pixels. The published
  geometry bounds it: the clamp to the panel stops the top at 6 px, left
  23, right 22, bottom 40; the keys are at least 39 px apart, so up to
  19 px no key overlaps a neighbour.

### 5.5 Image writes and flow control

- Capabilities (display fn0 `getCapabilities`, captured in hcooper step 9
  as `01 00 7a 1e 00 00 00 1b`; layout from the display pipeline report,
  `docs/research/options-plus-display-pipeline.md` section 0):

  | Byte | Field | Width and unit | This unit |
  |---|---|---|---|
  | 0 | `deviceScreenCount` | u8 | 1 |
  | 1..2 | `maxImageSize` | u16 BE in KiB (value × 1024 bytes), per image | `0x007a` = 124 928 bytes |
  | 3 | `maxImageFPS` | u8 | 30 |
  | 4..6 | not read by Options+ | | |
  | 7 | flags: bit 0 `deferrableDisplayUpdate`, bit 1 `rgb565`, bit 2 `rgb888`, bit 3 `jpeg`, bit 4 `calibrated`, bits 5..6 `origin` (2 bits) | u8 | `0x1b`: deferrable, rgb565, jpeg, calibrated; origin 0 |

  The inventory's 31 262 bytes was a misread of the same bytes. The VLP
  root's available memory, 124 960 bytes, is `maxImageSize` + 32.
- Format: baseline JPEG. Prior art reported RGB888 accepted and RGB565
  rejected; the fn0 flags above say the opposite (`rgb565` 1, `rgb888`
  0). keyvo writes JPEG only, so neither raw format is tested. Per-key
  images are 118×118 at the key's published rectangle; a full-panel paint is
  435×434 at (23, 6) on the 480×480 canvas (Options+, captured; logilinux uses
  434×434). Encoder: baseline, quality 100 (both quantisation tables all
  ones, as Options+ sends). Options+ subsamples 4:2:0; keyvo's encoder (the
  `image` crate, MIT/Apache-2.0) writes 4:4:4 only, which the device draws
  (first light), and the IJG-licensed `jpeg-encoder` that could write 4:2:0
  is refused by `cargo deny`; a 4:4:4 frame is larger, and the band split
  below catches one above the limit. The header dimensions must equal the
  JPEG SOF dimensions exactly (mismatch = nothing drawn, write succeeds;
  reported).
- Fragment layout (`0x14`, display fn2): first fragment carries a 20-byte header
  `14 ff 02 <fn|swid> <seq> 01 00 01 00 <x:u16> <y:u16> <w:u16> <h:u16> <len:u24>`
  then JPEG bytes; continuation fragments carry a 5-byte header; every
  fragment is exactly 4095 bytes, zero-padded, so the first carries 4075
  JPEG bytes and each continuation 4090. The fn2 `setImage` payload
  (display pipeline report, `docs/research/options-plus-display-pipeline.md`
  section 0, which corrects the inventory's "defer=1, count u16"): byte 0
  `displayIndex` (u8, send 1, the index of `getDisplayInfo(1)`), byte 1
  `deferDisplayUpdate` in bit 0 (send 0, the other bits 0), byte 2
  `count` (u8, the number of images), then per image a 12-byte image
  header, `format` (u8, low nibble; JPEG is 0), `x`, `y`, `w`, `h` (u16 BE
  each) and `length` (u24 BE), followed by `length` bytes of image data.
  So `01 00 01 00` is display 1, no deferred update, one image, format
  JPEG. Options+ always sends one image without the defer bit; keyvo does
  the same (`keyvo_hid::image::ImageHeader`, golden test against frame
  #521), and both fields stay hardware experiments.
  Sequence byte: bit `0x80` first, bit `0x40` last, bit `0x20` always set,
  the low four bits the fragment index: `e0` = first+last, `a0` = first of
  several, `21` a middle one, `61`-style = last with index. Options+ uses
  index base 0 and swid `0xd`; Julusian/logilinux use base 1 and swid
  `0xb` (`e1`, `a1 22 63`). Both bases with both software ids paint
  (measured, first light, `docs/hardware-notes.md` item 7), so keyvo's
  writer (`keyvo_hid::image`) keeps both as settings with the Options+
  defaults, index base 0 and swid `0xd`. Golden tests
  reproduce the captured writes byte for byte (frames #521, #639/#640 of
  `02-binding-paint`, #1779/#1780 of `03-options-plus-startup`).
- Placement comes from the published geometry (§5.4): a key's image goes
  to its record's rectangle; a panel paint goes to the bounding box of all
  keys, which on the captured table is exactly the 435×434 at (23, 6) that
  Options+ paints. Before a write the writer reads the JPEG's frame header
  and refuses anything but baseline SOF0 at the target's width and height.
- **The length field is 24-bit** (captured: `00 11 0a`). The 16-bit limit in
  logilinux/Julusian is a library limitation; keyvo honours 24 bits after the
  M0 check (plan §5 item 10) and guards JPEG size before encoding a frame.
  Until then the guard is the fragment index: one write carries at most as
  many fragments as the four index bits count from the index base without
  wrapping (16 with base 0, 65 425 bytes; 15 with base 1, 61 335 bytes).
  The device's own per-image limit is fn0 `maxImageSize` (124 928 bytes
  on this unit, above). The daemon reads fn0 after the claim and caps
  every image at the smaller of the two limits; a display that refuses
  fn0 is logged and capped by the fragment index alone. A crop above the
  cap is cut into horizontal bands of the full crop width, top to bottom,
  one `setImage` each, as Options+ does: n = ceil(length / cap) bands of
  equal height (the last may be lower), and more bands while one is still
  above the cap (`keyvo_hid::image::bands`). A single row above the cap is
  refused.
- **The device ACKs every fragment on `0x13`** (`13 ff 02 <fn|swid> c0 …`
  for fragment 0, `c1 00 01 …` for fragment 1, …). The "own writes echo back
  as key 1" trap in other projects is this ACK. keyvo uses **ACK-driven flow
  control**: send the next fragment when the previous one is acknowledged,
  with a timeout fallback. This replaces the blind 5–10 ms pacing of prior
  art and settles the "nine keys out of sync" problem (Julusian #11): on
  first light every fragment was acknowledged, about 1 ms after it, and
  nine keys painted without tearing or a key out of sync
  (`docs/hardware-notes.md` item 8; frame rate, plan §5 item 9, is
  deferred). The ACK is `13 ff <display> <fn|swid> c<index> 00 <last>`
  with the write's function and software id, the fragment index as sent
  and `<last>` `01` on the last fragment, else `00` (measured, fixture
  `image-acks.txt`); keyvo matches it on the function, software id and
  index bits. Without an ACK within `[timing].ack_timeout` (§6.1) the next
  fragment goes out anyway and the write reports how many fragments were
  acknowledged. Reports read while waiting (key input) stay for the input
  path. In the capture the ACKs come about 1 ms after a fragment; Options+
  does not wait for them.
- Single writer task; repaints are coalesced (superseded frames dropped);
  no shared static buffers.
- One composite image per change (pre-alpha, #72; replaces the
  background-then-keys order of #77). The device renders image commands
  one after another, about 35 ms each (owner's 240 fps video, #52), so a
  background and nine key writes are a visible cascade, and a key written
  over an older panel image leaves a strip beside it. Options+ never writes
  a key over a background; keyvo copies its pipeline (display pipeline
  report, sections 1, 3 and 4):
  - The controller sends the device actor the view (profile, page, mode),
    the active profile's accent and the specs of every key
    `[timing].render_coalesce` after the first change, so a burst of
    changes is one request; a newer request replaces an older one still
    queued.
  - The actor keeps one canvas of the panel size from the fn1 geometry
    (480×480 on this unit, never hard-coded; §9): the panel background with
    every key's tile at its rectangle. After every claim, every reclaim
    (§5.3), on a new view and on a new panel color (§9) the canvas is drawn
    anew and the area to send is the union of the key rectangles (435×434
    at (23, 6) on this unit, computed from fn1). Otherwise only the keys
    whose spec differs are drawn again (key states and feedback included)
    and the area is their bounding box: one key's reported rectangle for a
    one-key change.
  - Areas wait in a queue of at most two. An area a pending one covers only
    adds its keys to it; pending areas a new one covers are dropped as
    stale; a third area joins the newest pending one in their bounding box,
    so no change is lost (Options+ drops the oldest image). An area is
    cropped from the canvas and encoded when it is sent, so what goes out
    is always the newest content.
  - One area goes out per loop, as one `setImage` or as bands (above); each
    image after the device's response to the one before (the ACK of its
    last fragment, `c<n> 00 01`, which is the fn2 response with result 0
    and count 1). There is no frame timer; `[timing].ack_timeout` only
    bounds the wait for a missing ACK. Between two areas the actor reads
    what the device sent and takes new requests.
  - A key that cannot be drawn is logged and not tried again until its
    spec changes; an area that cannot be cropped, encoded or written
    (size guard, frame header) is logged and dropped; a failing device
    ends the connection. The actor reads at most `render_coalesce` at a
    time while nothing waits, so a request waits no longer.
  So a key and the background beside it always come from one image:
  nothing of an earlier panel paint (`keyvo fill --panel`, the logo) stays
  beside a key, a profile switch shows the new background with its keys at
  once, and a page does not appear key by key.

### 5.6 Input

- LCD keys arrive as `0x13` reports on VLP index 2 (`13 ff 02 00 <seq> 01
  <k1> <k2> … 0`): a list of held key indices (1..9), from which the driver
  derives press/release by diffing against the previous held set. Byte 3
  (function/swid) is `0` for input and non-zero for ACKs; the parser gates
  on it. Measured on first light (`docs/hardware-notes.md` item 13): the
  list holds the keys in press order (holding 2 then 1 gives `01 02 01`),
  the low nibble of the sequence byte counts up per report, and no report
  repeats while a key is held (204 reports, none equal to the one before).
  Three keys held at once were reported (`01 01 02 04`); the maximum is
  not measured.
- The decoder (`keyvo_hid::input::Input`) takes a report as input only
  when it comes from the device's index with byte 3 `00`: an LCD key
  report on the display's VLP index whose first parameter is the key-list
  marker `01`, or a diverted-control notification on the `0x1B04` index.
  Held keys are read up to the first `00`; keys outside the geometry's
  key count and repeats are ignored. Each report yields the releases of
  what is no longer held, in press order, then the presses of what is
  newly held, in report order; a report with the same held set yields
  nothing. Keys and page buttons are held independently. An image ACK
  carries the write's non-zero function and software id and never yields
  an event.
- Page buttons are `0x1B04 REPROG_CONTROLS_V4` controls CID `0x01A1`
  (left) and `0x01A2` (right), flags `0x20` divertable (measured). Divert
  with `setCidReporting(cid, 0x03)` after a 100 ms settle and 20 ms between
  writes; verify with `fn2 getCidReporting`; the divert is reported to be
  volatile and is re-sent on every open and reconnect (first light did not
  read it before the re-send, `docs/hardware-notes.md` item 12). Diverted
  presses arrive as `0x11` notifications carrying the held CIDs as u16 in
  press order: `11 ff 0b 00 01 a2 01 a1` is right, then left, held
  (measured).
- keyvo's divert (`keyvo_hid::reprog::divert_page_buttons`, after the
  keep-alive has started): wait `[timing].divert_settle`, resolve the
  `0x1B04` index, `fn0 getCount`, `fn1 getCidInfo(n)` for every control
  (`<cid:u16> <task:u16> <flags> <position> <group> <group mask>`), refuse
  unless both page buttons are listed and divertable, then per button
  `fn3 setCidReporting(cid, 0x03)` (echoed) and `fn2 getCidReporting(cid)`,
  whose reply `<cid:u16> <flags>` must carry the divert bit `0x01`. Every
  `0x1B04` request after `getCount` waits `[timing].divert_spacing` first.
  The caller supplies the wait, so the function does not pick a runtime.
  Options+ reads `getCidReporting` before the set and finds the divert bit
  already set (captured); keyvo reads it after the set as the check.
- Long-press is derived in `keyvo-core` from press/release timing
  (`[timing].long_press`, default 1000 ms).

### 5.7 Brightness

`0x8040 BRIGHTNESS_CONTROL` v1 (measured): `fn0 getInfo` (range/steps),
`fn1 getBrightness`, `fn2 setBrightness(u16)`. Options+ sets `0x0046 = 70`
(captured); whether the unit is percent or raw and what value 0 does
("resets the device", reported) are measured in M0-7. Idle-dim policy:
re-assert the configured brightness on resume, reconnect and, optionally,
on a timer.

### 5.8 Dialpad (M3)

- Identity `046d:bc00` over Bluetooth (not measured yet); over the Bolt
  receiver the HID++ device index is 1–6 on the receiver's `0x11` reports
  (measured 2026-10-29: slot 3 on the owner's receiver, found at runtime;
  `docs/hardware-notes.md`, "Dialpad over Bolt", record D1).
- Bolt slot discovery (static pass,
  `docs/research/2026-10-28-dialpad-static-pass.md` §8): the receiver's
  `0x41` arrival names the slot (byte 1), the WPID (bytes 5..6, little
  endian) and the link state (byte 4 bit 6 set = no link). Writing `0x02`
  to register `0x02` makes the receiver replay `0x41` for every paired
  device; long register `0xB5` at `0x51 + slot index` (0-based) holds the
  Bolt pairing information, WPID in its bytes 2..3. Not seen on the
  owner's receiver (2026-10-30, record D6): neither a wake from deep sleep
  nor a receiver replug gave a `0x41`.
- Feature table (measured over Bolt): `0x1B04` at index `0x0a` with four
  divertable controls, CIDs `0x0053, 0x0056, 0x0059, 0x005A` (Options+
  numbers them buttons 1 to 4); `0x4610` MultiRoller at index `0x0d`;
  `0x1004 UNIFIED_BATTERY`; `0x1D4B WIRELESS_DEVICE_STATUS`; **no `0x0008`
  keep-alive**. Measured again by keyvo (2026-10-29 and 2026-10-30,
  record D1): `0x1b04` at `0x0a`, `0x4610` at `0x0d`, `0x1004` at `0x09`,
  `0x1D4B` at `0x04`, firmware `RBO 07.00.B0016`. These are this device's
  indices; keyvo resolves each with `ROOT.getFeature`. keyvo's buttons 1
  to 4 in CID order are the physical numbering (owner confirmed).
- `0x4610` layouts (static pass §1 to §5; roller ids are `0..fn0 count`,
  always masked `& 0x0F`): fn0 → roller count; fn1(roller) → byte 0
  increments per rotation, byte 1 ratchet increments, byte 2 unnamed;
  fn2/fn3 `ReportingMode` is one bit, `NATIVE = 0`, `DIVERTED = 1`
  (`setMode` = `[roller, mode]`); fn4/fn5 settings = direction (bit 0,
  `INVERTED = 1`), increments per rotation, ratchets per increment (the Plugin Service
  never sends them to the Dialpad). Rotation event 0: payload byte 0
  roller, byte 1 delta `i8`, bytes 2..5 an optional `u32` big-endian
  timestamp (a 2-byte payload is valid). Options+ passes the delta on
  unchanged and drops the timestamp. Measured 2026-10-30 (record D5): the
  reports carry the timestamp in bytes 2..5 and arrive about every
  22 ms; `setMode` echoes `[roller, mode]` (records D5, D6).
- Rollers (reported, shensquared / Julusian PR #22, consistent with the
  static pass): 2 rollers; roller 0 = small wheel, 40 increments per
  rotation; roller 1 = large dial, 180 per rotation; diverting a roller
  stops native scrolling. Measured 2026-10-29 and 2026-10-30 (records D1,
  D5, D6): the ids, the increments and the stop of native scrolling as
  stated; + is clockwise on the dial and forward on the roller; the dial's
  deltas are accelerated in firmware (up to 18 fast, 4 to 6 slow), the
  roller's are not (1 to 2). Still open: the timestamp unit (it rises about
  one per millisecond) and whether the roller's sign is the opposite one
  the third party reported. keyvo reads count and increments from fn0 and
  fn1 instead of hard-coding them. A held diverted button sends no
  repeats (measured 2026-10-30, a 3.1 s and a 2.6 s hold, record D4; this
  corrects the earlier text, which said the presses repeat); keyvo, like
  Options+, reports edges of the held set only. There is no dial click.
- Diverts use `0x1B04` flags `0x03` (no persist bit) and `setMode(roller,
  DIVERTED)`; they are restored on exit and re-sent on every connect, as
  Options+ does. The diverts are volatile: after a receiver replug the
  roller scrolls again once the Dialpad reconnects (measured 2026-10-30,
  record D7). A Bluetooth device that sleeps comes back as a new HID
  object (not measured yet). Over Bolt the static pass expected a `0x41`
  without the link-lost bit on the wake; measured 2026-10-30 (record D6),
  the Dialpad announces it on its own device index instead, with a
  `0x1D4B` reconnect report (`11 <slot> 04 00 01 01 01`) and a `0x1004`
  battery report, and no `0x41` comes. Ten minutes idle is not deep
  sleep: no frame, the diverts hold, no turn lost. From deep sleep
  (overnight) the receiver answers connect fail at once and the host
  cannot wake the Dialpad; the turn that wakes it is lost. Retryable
  errors: HID++ 1.0 `0x04` (connect fail), `0x07` (busy), HID++ 2.0 busy
  and timeouts; `0x08` (unknown device) is not, and neither is `0x09`
  (resource), which the Bolt receiver answers for an empty slot
  (measured 2026-10-29, record D2; this corrects the static pass). Undiverted, the
  Dialpad's input arrives on the receiver's mouse interface node as
  report `0x02`: the dial as AC Pan, the roller as the wheel, buttons 1
  to 4 as mouse buttons 4 to 7 (measured 2026-10-30, record D8); diverted,
  nothing scrolls. keyvo ignores report `0x02` and, as a non-diverted
  fallback, can read the evdev axes (`REL_WHEEL*`, `REL_HWHEEL*`, keys
  275–278).
- Nothing in the Dialpad path uses Solaar.
- As built (beta, #186; `keyvo_hid::dialpad`, `multiroller`, `reprog`,
  `input`), on fixtures that replay the owner's frames of the hardware
  check of #187 (since #202; the receiver's `0x41` arrival stays
  synthetic, as no check saw one): after the slot search (§5.1),
  `Dialpad::find` walks the `0x1B04` controls after `[timing].divert_settle`
  (`getCount`, `getCidInfo` per control, each after
  `[timing].divert_spacing`) and keeps every divertable one, in CID order
  (buttons 1 to 4), then reads `fn0 getCapabilities` and `fn1
  getRollerCapabilities` per roller; both indices come from
  `ROOT.getFeature`. `Dialpad::open` then diverts, each request after the
  spacing: per button `setCidReporting(cid, 0x03)` checked with
  `getCidReporting`, per roller `setMode(roller, 1)`. `Dialpad::restore`
  sends `setCidReporting(cid, 0x02)` and `setMode(roller, 0)`. The input
  decoder (`Input::dialpad`) turns a `0x4610` notification (function 0,
  swid 0) into `InputEvent::Rotate { roller, delta }` and the diverted
  `0x1B04` notifications into button edges by the held-CID diff, so a
  held button is one press and one release (the firmware sends no
  repeats, record D4, and a repeat would change nothing).
  `Dialpad::next_event` also reads the receiver's arrival for the
  Dialpad's slot: the link lost forgets what is held
  (`DialpadEvent::LinkLost`), the link established re-sends every divert
  after the settle (`DialpadEvent::Rediverted`). Reports of other slots
  and of other software are skipped; the DJ reports (`0x20`, `0x21`) do
  not parse as frames and are dropped and counted. `keyvo dial` drives
  the Dialpad by hand (§5.10), the daemon since #188 (below).
- As built in the daemon (beta, #188; `keyvo_daemon::dialpad`): a second
  device actor on its own thread and transport, next to the Keypad's, so
  either device may be absent and neither waits for the other. Its
  connector opens `--dialpad-device` (a node sysfs knows as a Bluetooth
  Dialpad is reached on `0xff`, any other as a receiver) or the Bolt
  receiver's HID++ node found under sysfs on every attempt. Without a
  receiver, or after the transport fails, it reports why once (the
  `dialpad` block's `error`, §8) and opens it again after
  `[timing].reconnect_interval`; a receiver whose Dialpad does not answer
  is reported once (`no Dialpad answers (<slots>); waiting for it`) and
  searched again on the receiver's arrival for it or after the same
  interval. Connected, it reports the slot and the rollers, every turn
  (`roller id`, `delta`) and every button edge (1 to 4); the link lost and
  the re-divert are logged. It reads at most `[timing].render_coalesce`
  at a time and gives the diverts back on a clean stop. The controller
  resolves turns and presses on the profiles (§7.2) and runs them through
  the paths of a key press (§8).
- Measured over Bolt from cold (#187, part 1, 2026-10-29): with the
  Dialpad awake the search finds slot 3 and every expected value; from
  sleep the first replies are slow and a late reply comes twice, which the
  fix of #200 handles (§5.1: `slot_probe` 1000 ms, a software id per
  search request). The fixture `receiver-cold-search.txt` replays that
  run's frames with the slowest measured delays. The sleeping Dialpad can
  also answer the receiver's `8f` connect fail (`0x04`), which leaves the
  slot silent until the Dialpad wakes and a search round finds it.
- Measured over Bolt on the fixed build (#187, part 2, 2026-10-30): every
  Bolt step passes; the buttons, the rollers, the diverts and their
  restore, sleep and wake, the receiver unplug and the receiver's nodes
  are in `docs/hardware-notes.md`, "Dialpad over Bolt", records D1 to D8,
  and the fixtures `receiver-*.txt` replay the frames. Bluetooth is not
  measured yet.

### 5.9 Verification

Golden buffers from hcooper's captured frames, the frames captured on the
author's Keypad on first light (key and page-button input, image ACKs, the
divert after a replug, a keep-alive expiry, the sysfs descriptor) and
notno's packetizer tests live in `captures/`, `tests/protocol-fixtures/`
and `crates/keyvo-hid/tests/fixtures/sysfs/`; `FakeHidTransport`
integration tests replay them. HID fixtures live in
`tests/protocol-fixtures/hid/` as scripts: `> <hex>` is a host write,
`< <hex>` a device report answering the write above it (or unsolicited
before the first write), `#` a comment naming the source capture, frame
numbers and line range of the decode. Every protocol claim above that is not marked
*measured* is an item in the M0 measurement list (plan §5) and is recorded
with numbers in `docs/hardware-notes.md`.

### 5.10 Device commands (`keyvo-cli`)

The first CLI commands talk to the Keypad directly, without the daemon, for
the hardware check. They use the node of §5.1 or `--device <path>`, the
Keypad's device index `0xff`, software id `0xe` for requests (as Options+)
and `[timing].reply_timeout`.

- `keyvo probe`: read-only, no keep-alive, no claim, so it reports the mode
  the device is in. It prints the chosen node and its report ids, the HID++
  version (ROOT ping), the index and version of every classic feature keyvo
  uses (`0x0003`, `0x0008`, `0x1B04`, `0x8040`), the firmware entities,
  the keep-alive range, the VLP protocol and the `0x19A1` index, the
  display mode with the supported modes, the display capabilities and the
  geometry. A step the device refuses is reported in its line and the
  probe goes on; a transport error ends it. The display fields carry the
  names of §5.4 and §5.5; `keyvo_hid::display` decodes them, so the
  daemon reads `maxImageSize` the same way:

  | Line | Source | Captured Keypad |
  |---|---|---|
  | `deviceScreenCount N` | fn0 byte 0 | `1` |
  | `maxImageSize N bytes` | fn0 bytes 1..2, u16 BE × 1024 | `124928 bytes` |
  | `maxImageFPS N` | fn0 byte 3 | `30` |
  | `deferrableDisplayUpdate 0/1  rgb565 0/1  rgb888 0/1  jpeg 0/1  calibrated 0/1  origin N` | fn0 byte 7, bits 0..4 and bits 5..6 | `1  1  0  1  1  0` |
  | `geometry WxH, N buttons, N visible areas` | fn1 bytes 3..6, byte 7, byte 8 | `480x480, 9 buttons, 0 visible areas` |
  | `key N  WxH at X,Y` | fn1 button records | nine lines, §5.4 |
- `keyvo probe` and the Dialpad (beta, #186): when sysfs has a Bolt
  receiver node (§5.1), a receiver block follows the Keypad's, after an
  empty line; without a Keypad but with a receiver only the block is
  printed (the missing Keypad goes to stderr). It is read-only like the
  rest: the slot search with `[timing].slot_probe`'s default (probe reads
  no `config.toml`), the control walk and the roller capabilities, no
  divert. `--device` on a node sysfs knows as the receiver or as a
  Bluetooth Dialpad prints the block alone (its first label `dialpad`
  then, its slots line `direct dialpad`); any other node is probed as the
  Keypad. On the synthetic fixtures (§5.8):

  ```
  receiver    /dev/hidraw9 (Logitech USB Receiver, 046d:c548, reports 10 11)
  slots       1 empty, 2 silent, 3 dialpad
  features    0x1b04 REPROG_CONTROLS_V4 index 0x0a, 0x4610 MULTI_ROLLER index 0x0d
  firmware    BL2 18.00.B0016  bootloader
              RBO 07.00.B0016  application, active
  buttons     1 0x0053, 2 0x0056, 3 0x0059, 4 0x005a
  rollers     0 roller  40 increments per rotation, 0 ratchet increments
              1 dial  180 increments per rotation, 0 ratchet increments
  ```

  A receiver without the Dialpad prints the slots line and `dialpad
  not found`; a step the Dialpad refuses ends the block with its error.
- `keyvo dial [--json]` (beta, #186): opens the receiver's node (or
  `--device`, §5.1), finds the Dialpad and diverts it (§5.8), then prints
  one line per event until Ctrl-C or SIGTERM: `button 1 press`, `button 1
  release` (buttons numbered 1 to 4 in CID order), `dial +3`, `roller -1`,
  or with `--json` one NDJSON object each, `{"edge":"press","button":1}`,
  `{"turn":"dial","delta":3}`. Roller 1 is named `dial` and roller 0
  `roller` after the third-party reading (M3-2 measures the mapping); a
  further roller is `roller N`. On stop it gives every divert back and
  says `diverts given back` on stderr. A receiver without an awake Dialpad
  is reported once on stderr (`no Dialpad answers (<slots>); waiting for
  it`), and the search runs again on the next arrival or after
  `[timing].reconnect_interval`. The lost link and the re-divert after the
  arrival are lines on stderr. Stop is checked whenever a read found
  nothing; a read waits at most `[timing].reply_timeout`. It reads
  `[timing]` from `config.toml` like `keys`; `--dump` applies.
- `keyvo probe --feature <id>`: the feature id (hex, `0x` optional) asked
  of the classic ROOT and of the VLP root, one line per plane with the
  index and version, `absent` (index 0), or the error reply (the classic
  error code, the whole VLP error report in hex); a refused plane does
  not stop the other.
- `--dump` (global): every report written to and read from the Keypad is
  printed to stderr, one line each: the milliseconds since the start, `>`
  for out or `<` for in, and the bytes in hex. A `0x14` report (a VLP
  bulk fragment, 4094 bytes) prints its 5-byte header, `..` and its
  length instead of the whole fragment. Timeouts print nothing. stdout is
  unchanged, so `keys --json --dump` stays valid NDJSON. It is a
  `HidTransport` wrapper (`keyvo_hid::dump::Dump`), so `probe`, `keys`
  and `fill` all have it.
- `keyvo keys [--json]`: first keep-alive ping, claim (§5.4), page-button
  divert (§5.6), then one line per press and release until interrupted:
  `press key 5`, `release page left`, or with `--json` one NDJSON object
  per event, `{"edge":"press","key":5}`, `{"edge":"release","page":"left"}`
  (another diverted control: `"cid":<n>`). The keep-alive runs between
  reads.
- `keyvo fill <#rrggbb|png> [--key N|--all|--panel|--full] [--margin PX]`: first
  ping and claim, then a baseline JPEG (quality 90) of the color, or of the PNG scaled to
  the rectangle, per key in the geometry's order (`--all`, the default),
  on the keys' area (`--panel`, the union of the key rectangles, 435×434
  at 23,6 on the Keypad, §5.5) or on the whole published panel
  (`--full`, the geometry's width and height at 0,0, 480×480 on the
  Keypad); the four flags are mutually exclusive, and two of them are a
  usage error (exit 2); one line per write with the JPEG size,
  the fragments and the ACKs. `--hold <seconds>` keeps the claim and the
  keep-alive afterwards and reads what the device sends after every ping:
  a lost claim (§5.3) is taken back, reported as `display reclaimed after
  a lost claim; painting again`, and the image is painted again.
  `--index-base` and `--swid` set the writer settings the hardware check
  compares. `--margin <px>` (default 0, with `--key` and `--all`; refused
  with `--panel`, which already is the bounding box, and with `--full`) grows every key
  rectangle by that many pixels on all four sides and clamps it to the
  panel (`0..width`, `0..height` of the geometry) on each side; the
  source is scaled to the grown rectangle and the report line shows it,
  `key 1  126x126 at 20,2` for `--margin 4` (the #70 measurement, §5.4).
- `keys` and `fill` take `--no-keep-alive`, `--keep-alive-period <ms>` and
  `--keep-alive-timeout <ms>` for the keep-alive measurements; without them
  `keep_alive_period` and `keep_alive_timeout` come from the user's
  `config.toml` (`[timing]`, §6.1), else from its defaults, and `keys`
  diverts with `[timing].divert_settle` and `divert_spacing`. Only
  `config.toml` is read, not the profiles; one that cannot be loaded
  stops `keys` and `fill` with `configuration not loaded:` and the
  diagnostic (`probe` does not read it). A period that is not below the
  timeout is refused before the device is opened (§6.1), naming the flag
  or the `[timing]` key each value came from. `keys` reclaims a lost
  claim too and says so on stderr.
- `keyvo doctor [--json]` (beta, #184): the nine checks of §12 on this
  machine, without the daemon's help and without claiming the Keypad: it
  opens the node of §5.1 and `/dev/uinput` read-write and closes them
  again. Exit 1 when a check fails. It reads `config.toml` only for
  `[timing].doctor_reply`, and a file that does not load leaves the
  default.

---

## 6. Context engine

### 6.1 Modes, pages and overlays

```
mode           ∈ { auto, manual }
page_list      : per shown profile: ..., -1, 0 = global pages; 1 = context; 2, 3, ... = its others
index          : the position in the shown profile's page_list, in both modes
active_profile : chosen by WindowWatcher rules (auto), locked by the user (manual)
overlay_stack  : ordered overlays pushed via the socket API (e.g. Claude Code status)
```

The *page list* of the shown profile (#151, decided by the owner on
2026-10-27 after the prior-art report
`docs/research/2026-10-27-page-navigation-prior-art.md`):

| Index | Page |
|---|---|
| ..., -2, -1 | the global profile's pages after its `general` page, in file order: the first after `general` at -1, the next at -2 |
| 0 | the global profile's page named `general` |
| 1 | the shown profile's context page (its page named `context`, else its first enabled page, else its first page); for the global profile itself, its context page |
| 2, 3, ... | the shown profile's other pages in file order; a page named `general` in an app profile is an ordinary page here; for the global profile, its pages before `general` other than the context page |

A profile with one page has the list `[..., -1, 0, 1]`; without a global
`general` page the list starts at 1.

Page switches (beta, #156, the grill answers of the beta plan accepted on
2026-10-27): a page with `enabled = false` (§7.2) is left out of the
list: it gets no index, the page buttons skip it, the file keeps it with
its keys. A disabled global page is left out of every profile's list,
including a disabled `general` page itself (the global pages after it
stay, from 0 on); `general_hint` applies only while `general` is in the
list. The context page is in the list whether it is enabled or not; the
app does not offer to disable it. An app profile with
`global_pages = false` in its `[profile]` table has no global pages: its
list starts at 1. The global profile ignores the field. A reload that
disables the shown page shows the nearest end, as a reload that removes
it. The bundled default global profile keeps `apps` and `general`
enabled. The app computes the same list for its tab strip
(`app/src/lib/profiles.ts` `pageList`); both sides pass the shared cases
of `tests/protocol-fixtures/editor/page-list.json`
(`keyvo_core::state::page_list`).

Rendering is a **stack**: global layer < app profile < project layer <
overlays (ADR-0009). The project layer (§7.4) is the nearest `.keyvo/`
directory above the working directory of the focused window's context (§6.2)
and overrides single keys of the global and app layers. Overlays apply to the
*context* page (index 1) only, never to the *general* page, so media keys stay reachable
while an agent is working. A profile may declare an
optional `general_hint`: a single key on the general page (index 0) that mirrors the
top overlay's state (decided in the grill).

As built (pre-alpha, `keyvo_core::state`, no overlays or project layer
yet): the state machine starts in auto mode on the global profile's
context page, index 1. A profile's *context page* is its page named
`context`, else its first enabled page, else its first page; the *general page* is the global
profile's page named `general`. The view the daemon reports is the
profile that owns the page, the page counted from 1 inside it, the mode
and the `index`: at index 0 and below the profile is the global one and
the page its page number, so the VS Code list's index 0 reports
`global`, page 2, index 0. The daemon reports every change of profile,
page, index or mode as a `Switched` event and in the log (`showing
profile vscode index 2 (page 2) in auto mode`), then repaints the keys
that differ. The stack has two layers (`keyvo_core::stack`): under every page
of an app profile lies the global profile's context page, so a key the app
profile does not define shows and runs the global key, with the app
profile's accent; a key the app profile defines hides the global one, even
without an action. The global profile has nothing under it; the pages
at index 0 and below are the global profile's own and render as such. The trace
entry (`layer`), `KEYVO_LAYER` and the GUI's mirror name the layer that
supplied the key. `keyvo preview` renders one profile alone. Key states (`running`, `success`, `error`) belong to a key of one
page of one profile: an `exec` started on one profile keeps its look
there and never shows on another.

Defaults, all configurable under `[timing]` in `config.toml`:

| Key | Default | Meaning |
|---|---|---|
| `focus_hysteresis` | 150 ms | a focus change shorter than this does not repaint |
| `render_coalesce` | 50 ms | repaints within this window are merged |
| `long_press` | 1000 ms | hold duration that turns a press into `long` |
| `page_return_global` | 10000 ms | a global page (list index 0 or below) returns to the context page after this long without a page-button or key press; 0 is off, §6.3 |
| `page_return_own` | 0 (off) | the shown profile's further pages (list index 2 or above) return to the context page after this long without a page-button or key press; 0 is off, §6.3 |
| `keep_alive_period` | 1000 ms | time between two keep-alive pings, §5.3 |
| `keep_alive_timeout` | 3000 ms | timeout requested with every ping: how long the device holds the claim without the next one, §5.3 |
| `ack_timeout` | 50 ms | how long an image fragment waits for its ACK before the next one goes out anyway, §5.5 (captured ACK latency about 1 ms) |
| `divert_settle` | 100 ms | wait before the first page-button divert request, §5.6 (Julusian PR #22) |
| `divert_spacing` | 20 ms | wait between two `0x1B04` divert requests, §5.6 (Julusian PR #22) |
| `reply_timeout` | 1000 ms | how long an HID++ request waits for its matching reply, §5.1 and §5.2 (captured replies come within about 1 ms) |
| `exec_timeout` | 30000 ms | an `exec` action still running after this long is killed with its process group and fails, §8 |
| `launch_settle` | 1000 ms | how long a `launch` action is watched for an early exit; a command still running after this long is let go and has started, §8 |
| `launch_focus` | 15000 ms | how long a `launch` key's window is expected after the press: a window of the launch that takes the focus within it keeps a global page; 0 is off, §6.3 |
| `reconnect_interval` | 1000 ms | wait between two attempts to open the Keypad after it is gone or not found, §5.1 |
| `reload_settle` | 100 ms | quiet time after the last change in the configuration directory before the daemon reloads it, §7.2 |
| `accept_backoff` | 100 ms | wait after a failed `accept` on the control socket before the next one, so a full file table does not spin the daemon, §8 |
| `claim_splash_retry` | 500 ms | while the display shows the plug-in splash (`a4`), the claim re-sends `setMode(a0)` this often, §5.4 |
| `claim_splash_timeout` | 10000 ms | how long the claim waits for the plug-in splash to end before it fails, §5.4 |
| `doctor_reply` | 1000 ms | how long `keyvo doctor` waits for the daemon's answer to `status`, §12 |
| `slot_probe` | 1000 ms | how long the Dialpad search waits for one receiver slot's answer to `getFeature(0x4610)` before it counts the slot as silent (asleep); covers a first reply from idle or sleep (up to 771 ms measured over Bolt), §5.1 and §5.8 |
| `modifier_timeout` | 0 (off) | an armed toggle modifier key ends after this long without a turn of the dial or the roller; 0 keeps it until the next press, §6.1 and §7.2 |

`keep_alive_period` must be shorter than `keep_alive_timeout`: a period at
or above the timeout lets the claim run out between two pings (§5.3). The
configuration is refused with the file, the line of the value that was set
(the period first) and both key names, for example
``config.toml:4:21: `keep_alive_period` (3000 ms) must be shorter than
`keep_alive_timeout` (3000 ms)``. `keyvo fill` and `keyvo keys` check `--keep-alive-period`
and `--keep-alive-timeout` the same way before they open the device.

`claim_splash_retry` must be shorter than `claim_splash_timeout`, or the
claim would never retry; the configuration is refused the same way (the
retry first), for example ``config.toml:4:22: `claim_splash_retry` (2000
ms) must be shorter than `claim_splash_timeout` (2000 ms)``. The claim
re-sends `setMode(a0)` at most `claim_splash_timeout / claim_splash_retry`
times (20 with the defaults). The defaults come from the replug in #52
(`docs/hardware-notes.md` item 5): the daemon opened the Keypad at
20:48:31, its claim still saw `a4` at 20:48:34, and the profile was
painted at 20:48:41, so the splash lasted at most about ten seconds from
the open. The timeout covers that window; the retry keeps the delay
after the splash ends under half a second and the keep-alive pings
(period 1000 ms) on time between two retries. A retry above
`keep_alive_timeout` would let the keep-alive run out during the wait,
because the claim pings only between two retries.

Windows that hold the context, under `[focus]` in `config.toml` (read
when the daemon starts):

| Key | Default | Meaning |
|---|---|---|
| `hold_classes` | `["keyvo-app", "io.github.qubernetic.keyvo"]` | window classes (the app's is `keyvo-app` on the host and its application id under Flatpak), compared ASCII-case-insensitively as the whole name, that never change the context; an empty list holds nothing |

While a window of one of these classes has focus, the context and the
profile stay those of the last other window: a held report cancels a
pending one and leaves the applied window as it is, so the pad, the GUI's
mirror and its editor stay on the profile the user was just in when they
click into the keyvo app to configure it (the Stream Deck app suspends its
Smart Profiles while its window is open for the same reason). The next
report of a window not in the list applies with the usual
`focus_hysteresis`; a held report before any other window keeps the
global profile. A reload selects again for the window held over. The page
buttons, the mode toggle and manual mode work as before while held. The
daemon's `context` event and the `context` field of `status` carry
`held: true` for such a report (§8); no `profile`, `page` or `mode` event
follows it.

The session lock (#158, decided by the owner on 2026-10-27 after the
User test of #151: the pad stayed live under the lock screen, and every
key would have run its chord or `exec` in the locked session) is a veil
over the state, not a mode. While the `SessionWatcher` (§4.1, §6.2)
reports the session locked:

- the view keeps the profile, page, index and mode it had and carries
  `locked: true`; nothing else changes;
- the pad shows the lock page (§9): the lock on key 5 in the accent of
  the profile the unlock returns to, every other key blank;
- every key press, short or long, from the device or a socket `press`
  (`dry_run` included), resolves nothing and is traced with
  `Outcome::Blocked { reason: "session locked" }` (§8): no chord, no
  `exec` or `launch`, no feedback look, no `on_fail`;
- the page buttons do nothing, short, long or both together; a button
  held into the lock or the unlock is forgotten, so neither its long press
  nor its release acts;
- a focus report is recorded and applied as without the lock, with the
  focus hysteresis, and so is the page return; the lock page shows the
  new profile's accent;
- the unlock shows what the state would show anyway.

The lock and the unlock are `Switched` events like any other change of the
view; the socket reports them as `session` events (§8). An animated lock
(a pulse) is a later slice.

The dial modifier (#189, Q4 of the beta plan and the owner's comment of
2026-10-27 22:38Z): a key with a `dial` or `roller` table (§7.2) takes the
Dialpad's dial and roller over while it is the *active modifier*. As built
(`keyvo_core::state`):

- A **held** modifier key (`modifier = "held"`, the default) is active
  from its press to its release. It has no long press; its release runs
  its `action` as a `short` press only when no turn took a binding during
  the hold. Of two held modifier keys the one pressed last is active; the
  other is active again when the last one is released.
- A **toggle** key (`modifier = "toggle"`) is armed by a short press,
  which runs no action; its long press runs its `action`. The toggle ends
  at the next press of the same key (at the press; its release does not
  arm it again), of any other key or of a Dialpad button, which otherwise
  act as usual (a held modifier key takes over, an ordinary key runs its
  action); when a page button changes the page (§6.3) or the shown
  profile switches; when a reload drops the key or makes it another kind
  of key; at the lock; and after `[timing].modifier_timeout` without a
  turn, when that is above 0 (a turn starts the time over). One toggle is
  armed at a time; arming another ends the first.
- A held modifier key is active before an armed toggle. A turn of the dial
  or the roller runs the active key's binding of that roller, once per
  `step` detents (§7.2); a roller the active key leaves out falls to the
  held key before it that binds it, else to the profile's `[dialpad]`
  table. The carry below one step starts over when another binding takes
  the turns.
- The key resolves through the stack like any key: a global modifier key
  works under every app profile that does not define that key, and not
  under one that does. A modifier stays with the page it was pressed on;
  the key shows the `modifier` look (§9) there for its whole active time.
- A held or armed modifier is pad activity: no page return while one is
  active, and its end starts the return over.
- The view carries `modifier: { key, mode }` while one is active; a change
  is a `Switched` event, on the socket a `modifier` event (§8). The lock
  ends every modifier, and a key held through the lock or the unlock is
  forgotten, so its release runs nothing.

### 6.2 Window/context detection

- **KDE Wayland (v1):** the daemon pushes its own KWin script over D-Bus
  (`org.kde.kwin.Scripting.loadScript`); the script emits `resourceClass` +
  `caption` on `windowActivated` back to the daemon. Event-driven, never
  polling (OpenDeck #425). The daemon watches `NameOwnerChanged` for
  `org.kde.KWin` and reloads the script after a KWin restart.
- **KWin watcher as built (pre-alpha, `keyvo_daemon::kwin`, KWin 6):**
  - The script is `contrib/kwin/keyvo-focus.js`, compiled into the daemon.
    On the session bus the daemon serves the private object
    `/org/qubernetic/keyvo/Focus` (interface `org.qubernetic.keyvo.Focus`,
    method `Activated(s class, s caption, s pid)`), fills its own unique
    bus name and that object into the script's placeholders, and writes
    it to `$XDG_RUNTIME_DIR/keyvo/keyvo-focus.js` (without
    `XDG_RUNTIME_DIR`: `keyvo-<uid>` in the temporary directory). The
    directory is created `0700`; one other users can write is refused,
    as the script runs inside KWin. The check and the write are repeated
    before every load, so a KWin restart never loads a script from a
    directory that was aged away and recreated by another account in the
    temporary-directory fallback, or a file changed since the last load.
    A directory refused at start ends the watch; one refused at a later
    load is logged and that load is skipped.
  - Loading: `isScriptLoaded("keyvo-focus")`, and `unloadScript` for a
    stale copy left by an earlier daemon; `loadScript(path,
    "keyvo-focus")`; `run` on `/Scripting/Script<id>`
    (`org.kde.kwin.Script`). A negative id or a failed call is logged as
    a warning and focus switching stays off until the next load.
  - The script reports `workspace.activeWindow` once when it runs, then
    `resourceClass`, `caption` and `pid` on every `windowActivated` and
    on `captionChanged` of the active window, so title rules follow a
    terminal or editor title. The pid travels as a decimal string,
    because `callDBus` would send a JavaScript number as a double; an
    empty or non-positive pid is "unknown". A `null` activation
    (nothing focused) is not reported.
  - KWin missing at start is logged ("KWin is not running; focus
    switching waits for it") and is not fatal; `NameOwnerChanged` with a
    new owner loads the script, and one with no owner logs that KWin is
    gone. A missing session bus logs "focus switching off" once; the
    daemon runs on with the page buttons.
  - The script stays loaded when the daemon stops; its calls then go
    nowhere, and the next daemon replaces it.
  - Tests run without KWin: a peer-to-peer zbus connection to a fake
    that plays `NameHasOwner`, `NameOwnerChanged` and the scripting
    interface, and calls `Activated` as the script does
    (`crates/keyvo-daemon/tests/kwin.rs`). The script file itself is
    checked for its header comment and placeholders; its behaviour inside
    KWin is the owner's User test.
- **X11 (v1, planned):** `_NET_ACTIVE_WINDOW` via `x11rb`; not built in
  the pre-alpha.
- **GNOME, Hyprland, Sway (community):** no cross-compositor Wayland focus
  protocol exists (KWin 6.7 and Mutter 51 implement neither
  `wlr-foreign-toplevel-management` nor `ext-foreign-toplevel-list`). keyvo
  ships the `notify_context` socket command as the contract: any external
  source (GNOME extension, Hyprland socket2 reader, `swaymsg subscribe`) can
  assert the active class and title. Backends for those compositors are
  contributions behind the `WindowWatcher` trait. As built (pre-alpha), a
  `notify_context` report takes the watcher's path, with the focus
  hysteresis, and the last report from either source wins. Its
  precedence over the watcher, `cwd` and the pid come with M2-5.
- **Windows:** `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` (native mode only).
- **Session lock as built (pre-alpha, `keyvo_daemon::logind`):**
  systemd-logind on the system bus. The daemon's session is the object
  `org.freedesktop.login1.Manager.GetSessionByPID` names for the daemon's
  pid; for a daemon outside every session (a systemd user service) it is
  `/org/freedesktop/login1/session/auto`, resolved to the session's own
  path through its `Id` (systemd's path escaping: `2` is `_32`), because
  logind emits the signals on that path. The watcher subscribes to the
  signals of that path first, then reads `LockedHint`
  (`org.freedesktop.login1.Session`) and reports it, so a daemon started
  under the lock starts locked. After that it reports the `Lock` and
  `Unlock` signals and every `PropertiesChanged` that carries
  `LockedHint`: `loginctl lock-session` sends `Lock`, while the screen
  locker sets `LockedHint` for a lock by shortcut or idle timeout and for
  the unlock with the password, which sends no `Unlock`. When the system
  bus or logind is unreachable the daemon logs "lock detection off" once
  and runs without lock detection, like the KWin watcher without KWin.
  The KWin lock-screen window (class `kwin_wayland`, empty title, which
  KWin reports as a focused window) is not used: it is a side effect, not
  a contract. Tests run without logind: a peer-to-peer zbus connection to
  a fake manager and session (`crates/keyvo-daemon/tests/logind.rs`), and
  unit tests of the signal decoding on built message bodies. Windows and
  X11 lock sources come with M5 and the X11 watcher.
- Matching rules: by class (reverse-DNS `org.kde.konsole` and short
  `konsole` both accepted, glob/regex), regex on title (VS Code file →
  language pages, terminal title → Claude Code), explicit priority, and
  **external hooks** via `notify_context`, which take precedence over
  heuristics.
- Matching as built (pre-alpha, `Match::matches` and `Profiles::select`):
  a profile matches a window when one of its `match.class` entries does
  (if it has any) and its `match.title` regex finds the title (if it has
  one); a profile with neither never matches. A class entry matches
  ASCII-case-insensitively as the whole name, and a name without a `.`
  also matches the last segment of a reverse-DNS name on the other side
  (`konsole` and `org.kde.konsole`; `org.kde.konsole` and
  `org.other.konsole` do not). Of the matching app profiles the highest
  `priority` wins; between equal priorities one with a title rule beats
  one without, then the first in id order. No match selects the global
  profile, whose rules are never consulted. A report applies once no
  other report came for `focus_hysteresis`; a report of the window
  already applied cancels a pending one, and so does a report of a class
  in `[focus].hold_classes` (§6.1), which never applies. Class globs and
  regexes come later.
- **Project context (M2).** Integrations push the working directory with
  `notify_context` (`cwd` field): shell prompt hooks for bash, zsh and fish,
  the VS Code extension, tmux. The daemon binds the directory to the focused
  window by matching the hook's ancestor pids with the pid the window watcher
  reports. Fallback: the window title matched against the project registry
  (trusted roots from `trust.toml` plus recently seen roots); file managers
  such as Dolphin live on this fallback. Otherwise the context is "unknown
  project" and the reason (no hook, no pid match, no title match) is part of
  the status and of every trace entry.

### 6.3 Page buttons

- In both modes the left button shows index - 1 of the shown profile's
  page list (§6.1), the right button index + 1. The list does not wrap: a
  press past either end does nothing (no event, no repaint).
- An app switch shows index 1 of the new profile at once, whatever the
  index was. A title change in the same app keeps the index (#157; this
  replaces `page_grace` of #151, which kept the index across an app
  switch for 10 s after a page button's use).
- A launch keeps the page (#179, decided by the owner on 2026-10-27 after
  the first evening on the page return; prior art in
  `docs/research/2026-10-27-launch-focus-matching-prior-art.md`): the pad
  does not switch pages while the hand is on the pad. A press of a
  `launch` key (§7.2) with `wait_focus` on opens a *pending launch* that
  expects its window for `[timing].launch_focus` (15 s, 0 turns this
  off). An app switch is judged against the pending launches, the first
  hit consuming its entry:
  1. lineage: the window's process is in the session the launch's shell
     leads, or the shell is among its ancestors;
  2. class: a launch whose `class` (explicit, or derived from a `flatpak
     run <id>` command) matches the window's class like a `match.class`
     entry; this covers the hand-over to a running instance and D-Bus
     activated applications, where the lineage breaks;
  3. the oldest launch without a class;
  4. no hit: the switch is foreign.
  A hit switches the profile to the window's, keeps an index of 0 or
  below (a global page exists under every profile) and starts the page
  return over at the window's arrival; an index of 2 or above shows
  index 1, as the other profile may not have that page. A foreign switch
  shows index 1 as before and leaves the pending launches until they
  expire or match. A launch that fails to start or exits with an error
  drops its entry; one that exits 0 within `launch_settle` keeps it (the
  single-instance hand-over of `flatpak run`, `code` or a browser).
- The page return: a page that was paged to does not stay on the pad
  once it is no longer used. The clock starts at the last activity on the
  pad, a page button's press or release or a key's press; a key press
  restarts it, so a run of launches from the `apps` page keeps the page.
  When the clock of the side the index is on runs out, the pad shows
  index 1 of the shown profile. The global pages (index 0 and below) are
  visited for a press or two, an app's own further pages can be the
  working page of a whole session, so each side has its own time:
  `[timing].page_return_global` (10 s) for index 0 and below,
  `[timing].page_return_own` (0, off) for index 2 and above; 0 turns a
  side off. An app switch cancels a pending return. A press of a Dialpad
  button is pad activity and restarts the clock too; a turn of the dial
  or the roller is not, as the Dialpad's bindings belong to no page
  (beta, #188). A held or armed dial modifier key (§6.1) holds the clock
  for its whole active time, and its end starts it over (#189).
- A page button that changes the page ends an armed toggle modifier key
  (§6.1, #189); one that changes nothing (the end of the list, the mode
  toggle) keeps it.
- Manual mode is a focus lock: a long press (`[timing].long_press`) on
  either page button toggles it and keeps the shown profile and index;
  focus changes are recorded and not shown; the buttons walk the same
  list; the page return does not apply. Leaving manual mode shows index 1
  of the profile the focused window selects.
- Manual mode is visually distinguishable (dimmed/dashed frame or a corner
  "lock" badge, theme-controlled).
- Both page buttons held together is reserved as an always-available escape
  chord (return to the global profile, context page, auto mode).
- As built (pre-alpha, `keyvo_core::state`): a page button acts on its
  release when it was held shorter than `long_press`, and toggles the mode
  once `long_press` is reached, without waiting for the release. A short
  press walks the list as above in either mode. An app switch is a focus
  change to another window class; a title change in the same app keeps
  the index even when it selects another profile, clamped to that
  profile's list. The page return (#157) runs in auto mode only: every
  press and release of a page button and every press of a key on the pad
  (not a socket `press`) restarts its clock, and the daemon wakes the
  state machine at the deadline, which then shows index 1 and reports it
  as a `page` event with a repaint. The return runs whatever window is in
  front, a window that holds the context included. A reload reads both
  return times and keeps the clock. A `config.toml` that still names
  `page_grace` does not load: unknown keys are errors (§7.2). A reload
  keeps the index, clamped to the list as it is now; a locked profile that is gone gives way to the global
  context page, still locked. The second button held escapes to the
  global profile, index 1, auto mode; neither button then acts again
  until it is pressed anew. Until a later focus change the global profile
  shows. The visual mark for manual mode comes later.
- As built, the launch rule (#179): `State::launched` opens a pending
  launch with the class and an expiry at the press plus `launch_focus`;
  the daemon spawns the shell itself, so `launch_started` gives it the
  shell's pid before the settle, and `launch_failed` drops it on a spawn
  error or a non-zero exit. A focus report records which pending launches
  its process descends from (the `Lineage` platform trait of
  `keyvo_core::lineage`; the daemon's `ProcLineage` reads the session id
  from `/proc/<pid>/stat` and walks the parent chain, at most 64 levels);
  the judgment runs when the window applies after the focus hysteresis,
  in auto mode without the session lock, on an app switch only. Manual
  mode, the lock, a title change and a window that holds the context
  consume no launch. Expired launches are dropped at the next poll or
  focus report; a reload reads `launch_focus` for the next press and keeps
  the pending launches. The view's `profile` names the painted profile,
  the global one at index 0 and below, so a launch's window that keeps a
  global page changes no `profile`, `page` or `mode` event; the socket's
  `context` event reports the window, and the return to the new profile's
  context page comes as `profile` and `page` events
  `page_return_global` after it. A `launch` whose command opens no window
  of its own should name a `class` (the `sleep` key of
  `contrib/profiles/global.toml` does), or it claims the next window
  that takes the focus within `launch_focus`.

---

## 7. Profiles and "desk state"

### 7.1 Config layout

```
~/.config/keyvo/
├── config.toml            # [timing], [daemon], [render], [desk_state], [project_layer], socket, editor
├── profiles/<id>.toml     # one profile per file; <id> is the profile id
├── icons/                 # user PNG/SVG
├── themes/                # theme TOML files
├── recipes/<id>.toml      # user recipes (§7.5)
├── trust.toml             # trusted project layers, keyed by content hash (§7.4)
└── plugins/<name>/        # plugin.toml + executable

~/.local/state/keyvo/
├── trace.ndjson           # unified trace, rotated (§8)
├── snapshots/             # profile file copies taken before every GUI write
└── sandbox/               # the simulator's working copy while a sandbox is open
```

Profiles are plain TOML with a `schema_version`. `keyvo-core` reads and
writes them with `toml_edit` so that comments, ordering and formatting
survive a GUI or CLI edit (**round-trip is an M1 invariant**, ADR-0004). A
JSON Schema generated with `schemars` ships with every release and backs
editor validation. Validation errors carry file, line and column.

Loading (`keyvo_core::layout`, pre-alpha subset: `config.toml` with
`[timing]`, `[focus]`, `[render]`, `[trace]` and `[daemon]`, and
`profiles/`):

- The directory is `$XDG_CONFIG_HOME/keyvo` when that variable is an
  absolute path, else `$HOME/.config/keyvo`. A missing directory,
  `config.toml` or `profiles/` is not an error: the defaults apply.
- Every `profiles/*.toml` is read, other files are ignored; the profile id
  must equal the file name without `.toml` (lowercase letters, digits, `.`,
  `_`, `-`). Without `profiles/global.toml` a built-in global profile with
  one empty page is used, so the global profile always exists.
- Durations in `config.toml` are integer milliseconds; a key left out keeps
  its default (§6.1, §9).
- Unknown keys are errors in both files, so a typo never silently disables
  a setting or a key.
- One load reports every invalid or unreadable file, each as
  `<file>:<line>:<column>: <message>`, the position pointing at the
  offending value or key.
- The generated schemas are committed as `schemas/profile.schema.json` and
  `schemas/config.schema.json`; a test fails when they differ from the
  types (`KEYVO_BLESS=1 just test schema` rewrites them). An editor with a
  TOML language server (taplo, Even Better TOML) validates a file with a
  first line `#:schema <path to the schema>`.

Editing (`keyvo_core::edit` and `keyvo_core::store`, pre-alpha subset: the
icon, label, color and action of one key):

- `ProfileDocument` holds a profile file as `toml_edit` reads it.
  `set_key(page, key, fields)` touches only the values that differ: a
  changed value keeps the whitespace and comment around it, an absent field
  is removed, an action of the same type keeps its table (inline or a
  `[pages.keys.N.action]` sub-table) and only its value changes, and a key
  the page does not define is added at the end of its page. Every byte
  outside the edited key stays as it was; the fixtures in
  `crates/keyvo-core/tests/fixtures/edit/` prove it line by line. `states`
  and anything else in the key's table are kept.
- The edited text is validated as a profile before it is handed out; a key
  that cannot be edited without rewriting the file (a `pages` array that is
  not tables) is an error naming it, never a reformatted file.
- `store::snapshot` copies the file to
  `~/.local/state/keyvo/snapshots/<ms>-<file>` first (`$XDG_STATE_HOME/keyvo`
  when that is an absolute path; `<ms>` the Unix time in milliseconds, 13
  digits) and keeps the newest 50 per file (`store::SNAPSHOTS_KEPT`).
- `store::write` writes a temporary file beside the profile, flushes it and
  renames it over the old one, so the daemon's reload reads the old file or
  the new one, never half of it. The file keeps its permissions; a profile
  that is a symlink (into a dotfile repository) stays a symlink and its
  target is replaced.
- A missing `profiles/global.toml` is created from the built-in global
  profile's text (`edit::BUILTIN_GLOBAL`) on its first edit, together with
  `profiles/` and the configuration directory if they are missing; an app profile
  without a file is an error.

### 7.2 Profile format

```toml
schema_version = 1

[profile]
id     = "vscode"
name   = "VS Code"
match  = { class = ["code", "com.visualstudio.code"], title = "" }
accent = "#4e8bd6"
priority = 10
general_hint = 9            # key on the general page that mirrors the top overlay (optional)

[[pages]]
name = "context"

[pages.keys.3]              # keys are numbered 1..9, row-major, top-left = 1
icon   = "lucide:git-commit"
label  = "Commit"
color  = "#4e8bd6"
action = { type = "chord", keys = "ctrl+shift+g" }
[pages.keys.3.states.dirty]
color = "#e0a83b"
badge = "●"

[pages.keys.1]
icon    = "lucide:hammer"
label   = "Build"
action  = { type = "recipe", id = "project.build" }   # resolved at press time (§7.5)
on_fail = { type = "exec", cmd = "notify-send 'build failed'" }
[pages.keys.1.states.error]  # overrides the theme's built-in error look
badge = "!"

[pages.keys.7]              # a dial modifier: the dial and the roller follow this key
icon     = "lucide:sun"
label    = "Brightness"
modifier = "held"           # the default with a dial or roller table; or "toggle"
action   = { type = "exec", cmd = "brightnessctl set 50%" }
dial     = { cw = { type = "exec", cmd = "brightnessctl set +5%" }, ccw = { type = "exec", cmd = "brightnessctl set 5%-" }, step = 6 }
roller   = { cw = "KEY_BRIGHTNESSUP", ccw = "KEY_BRIGHTNESSDOWN" }

[dialpad]
dial   = { cw = "ctrl+]", ccw = "ctrl+[", step = 15, label = "Indent" }
roller = { cw = { type = "chord", keys = "ctrl+tab" }, ccw = { type = "chord", keys = "ctrl+shift+tab" }, step = 4 }
[dialpad.buttons.1]
label  = "Mute"
action = { type = "chord", keys = "KEY_MUTE" }

[desk_state]                # applied via Solaar on profile activation (optional, M3)
mouse.thumb_button   = "goto_definition"
keyboard.backlight   = { mode = "static", brightness = 60 }
mouse.dpi            = 1200
```

- Pre-alpha subset (schema version 1 as `keyvo-core` reads it today):
  `schema_version`; `[profile]` with `id`, `name`, `match` (`class` list,
  `title` regex, checked when the file loads), `accent`, `priority`
  (integer, default 0), `global_pages` (beta, #156: bool, default
  `true`; `false` leaves the global pages out of this profile's page
  list, §6.1; the global profile ignores it); `[[pages]]` with an
  optional `name`, `enabled` (beta, #156: bool, default `true`; `false`
  leaves the page out of the page list and keeps its keys, §6.1) and
  `keys.1` to `keys.9`, each with `icon`, `label`, `color`, `action` and
  `states.<name>` (`icon`, `label`, `color`, `badge`). Colors are
  `#rrggbb`. Actions are `chord` (`keys`), `exec` (`cmd`) and `launch`
  (`cmd`, an application keyvo starts and lets go, §8; optional `class`,
  the window class the launch opens, matched like a `match.class` entry
  and derived from a `flatpak run <id>` command when absent, and
  `wait_focus`, default `true`, whether the launch's window keeps a
  global page, §6.3; both are unknown fields on a `chord` or an `exec`).
  The editor writes `class` and `wait_focus` only where they differ from
  the defaults. The other
  keys of the example above (`general_hint`, the `recipe` action,
  `[desk_state]`) are rejected as unknown until their issues add them.
- `[dialpad]` as built (beta, #188): `dial` (the large dial) and `roller`
  (the small wheel), each with `cw` and `ccw` (a whole action, or a
  string, which is a `chord`: `cw = "ctrl+]"`), `step` (detents per run of
  the action, at least 1, default 1) and `label` (what the app's Dialpad
  panel shows); `buttons.1` to `buttons.4` (in CID order, §5.8), each a
  key without an icon or a colour: `label`, `action`, `on_fail`, both
  press kinds running the action as for a key. Unknown fields are errors
  with their position. `cw` is a positive delta: clockwise on the dial,
  forward on the roller (record D5). A detent is one increment as the
  device reports it (180 a rotation on the dial, 40 on the roller); a
  turn's delta counts as that many detents, so the firmware's acceleration
  of a fast dial turn runs the action more often. `step` detents run the
  action once, the rest carries to the next turn in the same direction
  (a turn the other way, a reload start from nothing), and one report
  runs it at most `[dialpad].max_per_event` times (`config.toml`, at
  least 1, default 8: a slow dial turn reports 1 to 6 detents, a fast one
  up to 18), the rest is dropped. Each control of an app profile's table
  hides the global profile's control of that name, even without an
  action; a control the app profile leaves out comes from the global
  table, as a key from the global context page (§6.1). The bindings do
  not depend on the page; manual mode keeps its profile and the dial
  still works. `keyvo preview` ignores the table. The editor (§10) writes
  `step` only where it differs from 1, keeps a short-form chord short
  while it stays a chord, and removes a control left without fields, so
  the global one shows through again.
- A key's `modifier`, `dial` and `roller` as built (beta, #189): a key may
  carry a `dial` and a `roller` table next to `action`, each with the
  fields of the `[dialpad]` entries (`cw`, `ccw`, `step`, `label`), and a
  `modifier` mode, `held` (the default when a table is there) or
  `toggle`. A key with either table is a *modifier key*: while it is the
  active modifier (§6.1) a turn of a roller it binds runs its binding
  instead of the profile's `[dialpad]` one, once per `step` detents, with
  the same carry and `max_per_event` cap. `modifier` without a `dial` or
  `roller` table is a load error at the key's table; a key with neither
  table behaves as before. A held modifier key never fires `long`; its
  `action` runs as `short` at a release without a turn. A toggle key's
  short press arms it and runs nothing, its long press runs its `action`
  (the common toggle key has none). Both resolve through the stack
  (§6.1). The table's `label` is what the app's Dialpad panel shows while
  the key is active, else the key's `label`. The editor (§10) writes the
  three fields like a key's other fields: `modifier` only as written, a
  new table as an inline table, `step` only where it differs from 1, an
  empty table (`dial = {}`) kept, so a key can be a modifier with nothing
  bound yet; the mode Off removes all three. A recipe applied to the key
  keeps them. `keyvo preview` renders the key as any other.
- Chords as `keyvo_core::chord` parses them (checked when the profile
  loads; a bad chord is reported at its `keys` value): steps separated by
  `,`, each step modifiers joined with `+` and one key last
  (`ctrl+k, ctrl+s`). Names are case-insensitive, spaces around them are
  ignored. Modifiers: `ctrl` (`control`), `shift`, `alt`, `altgr` (right
  alt), `super` (`meta`); they are pressed in that order and released in
  reverse, and a modifier alone is pressed as its own key. A key is a
  symbol, an alias (`escape`, `return`, `del`, `ins`, `pgup`, `pgdn`,
  `period`, `quote`, `backtick`) or any `KEY_*` name of
  `linux/input-event-codes.h`, with or without the prefix (`f13`,
  `KEY_PLAYPAUSE`). Shifted symbols add `shift` and press the key that
  carries them on the US layout (`ctrl+?` is ctrl, shift and slash). Where
  a key is expected, `+` and `,` are keys (`ctrl++`, `ctrl+,`); a sequence
  step after `ctrl+,` needs its own comma (`ctrl+,, a`). Per-step delays
  and hold/release come later.
- Press kinds as built (`keyvo_core::press`): `short` and `long`; the long
  press fires once `[timing].long_press` is reached, without waiting for
  the release. A profile has one action per key, and both kinds run it;
  per-kind bindings and `repeat` come with M1-9. A modifier key is the
  exception (#189): a held one has no `long`, a toggle key's `short` arms
  it and only its `long` runs the action.
- Action types (v1): `chord` (key combo, sequences with `,`, per-step
  delays, hold/release), `exec` (non-blocking subprocess with `KEYVO_*`
  env whose exit is the result, §8), `launch` (open an application: the
  same command line, let go once it outlives `[timing].launch_settle`, §8;
  Stream Deck's "Open" next to "System: Run command"), `http` (GET/POST), `text` (type a string), `page` / `profile` /
  `mode` (internal), `recipe` (§7.5, resolved to one of the others at press
  time). Chord aliases (`ctrl`/`control`, `super`/`meta`) and shifted symbols
  (`ctrl+?`) are accepted; the full evdev keycode table is supported.
- `on_fail` (optional, per key or per press kind): an action run when the
  primary action fails (non-zero exit, timeout, non-2xx, unresolved recipe).
  It is not retried and has no `on_fail` of its own. As built (beta,
  #183): one `on_fail` per key, next to `action`, any of `chord`, `exec`
  and `launch`; an `on_fail` inside an action (nested, or written inside
  `action`) is a load error at the nested table's line and column. It
  runs once when the key's action ends `exited` with a code other than
  0 (an `exec`, or a `launch` within `[timing].launch_settle`) or
  `failed` (past `exec_timeout`, a command that does not start, a chord
  the injector cannot send); never on `blocked`, a dry run, an unbound
  key, a `started` launch, or when the `on_fail` itself fails. The key
  keeps the primary's `error` look; the `on_fail` run shows no look of
  its own. Per press kind comes with M1-9. The key editor leaves
  `on_fail` in the file as it is and the Inspector shows it read-only
  (§10).
- Automatic states: an `exec`, `launch` or `http` action switches the key's
  state to `running` while it runs, then to `success` or `error` (a
  `launch` is `success` once it has started or exited 0); the looks are the
  theme's built-in feedback primitives (§9) unless the key's own
  `states.running`, `states.success` or `states.error` override them. A
  script may select a custom state by printing `KEYVO_STATE=<name>` on
  stdout. `error` stays until the next press of the key.
- Press kinds: `short`, `long`, `repeat` (hold).
- A global profile always exists; app profiles overlay it; matching priority
  is explicit, never alphabetical. As built (pre-alpha slice R2): the
  overlay is per key. A key an app profile's page leaves out comes from the
  global profile's context page (§6.1); to hide a global key on an app
  page, define the key there (a key without an action is unbound and
  blanks nothing but the global key).
- Hot-reload with inotify (mtime poll fallback), `SIGHUP` and the socket
  `reload` command; writes are atomic.
- Hot-reload as built (pre-alpha, `keyvo_daemon::watch`): inotify watches
  the configuration directory and `profiles/` (directories, not files, so
  a rename over a file counts like a write; a `profiles/` created later is
  added). After `[timing].reload_settle` without a further change the
  daemon loads the whole directory again (§7.1). A directory that does not
  load keeps the last good profiles and config, logs every problem with
  file, line and column and paints nothing new. In auto mode the focused
  window selects its profile again, as the rules may have changed (a new
  profile file for the focused app shows at once); manual mode keeps its
  profile while it exists, else the global context page shows. The page
  list index stays, clamped to the list as it is now: an index whose page
  is gone shows the nearest end of the list (§6.3). Key states carry over. When the directory does not
  load at start, the daemon runs with its `config.toml` (when that one is
  valid) and the built-in global profile. A missing directory turns
  hot-reload off with a warning. The `[timing]` values of the device
  (§5), `long_press`, `focus_hysteresis` and
  `[focus].hold_classes` are read at start; the other `[timing]` and `[render]` values and `[trace].ring`
  apply from the next reload.
  The socket `reload` (§8) loads the directory at once, with the same
  rules, and answers whether it loaded. The mtime poll fallback and
  `SIGHUP` come later.
- Desk state is applied by calling `solaar config …` (v1, optional feature,
  M3); never on Windows in plugin mode (Options+ owns the devices).

### 7.3 Options+ profile import (`.lp5`)

An Options+ profile is a ZIP of documented JSON (`ApplicationInfo.json`,
`ProfileInfo.json`, `ActionIcons/*.ict` = JSON wrapping base64 SVG plus a
text layer). `keyvo profile import <file.lp5>` converts icons, labels and
simple chords into a keyvo profile; layout-id-encoded chords and
platform-specific actions are imported best-effort and flagged. Placement:
**[OPEN]** M1 or M2 (plan §8; the plan proposes M2).

`keyvo profile export <id|all> <file.zip>` and `keyvo profile import
<file.zip>` (M1) bundle profiles, referenced icons and the theme file; the GUI
exposes both. Import passes the trust gate (§7.4) when the bundle contains
`exec`, `http` or `recipe` actions.

### 7.4 Project layer (`.keyvo/`, M2)

Decided in ADR-0009. A repository may carry a `.keyvo/` directory that the
daemon finds by walking up from the context's working directory (§6.2), like
`git` finds `.git`; the nearest one wins.

| File | Scope | Notes |
|---|---|---|
| `.keyvo/profile.toml` | every app profile | committed, shared with the team |
| `.keyvo/profiles/<app-id>.toml` | one app profile | committed |
| `.keyvo/local.toml` | every app, this user | `.gitignore` candidate; also holds recipe answers (§7.5) |

- Same schema as a profile minus `[profile].match`; `schema_version` is
  required. Keys override at key level; `[pages.keys.N] clear = true` removes
  a key the lower layers define. `[recipes.<id>]` tables supply recipe
  parameters.
- `exec` actions from the layer run with the project root as working
  directory and receive `KEYVO_PROJECT_ROOT`, `KEYVO_PROJECT_NAME` and
  `KEYVO_GIT_BRANCH` (§8).
- `[project_layer]` in `config.toml`:

| Key | Default | Meaning |
|---|---|---|
| `enabled` | `true` | read `.keyvo/` at all |
| `position` | `"above_app"` | `"above_app"` (default stack) or `"above_global"` (app profiles win over the project) |
| `allowed_keys` | `[1, 2, 3, 4, 5, 6, 7, 8, 9]` | keys a project may define or clear |
| `allowed_action_types` | `["chord", "exec", "http", "text", "page", "recipe"]` | `profile` and `mode` are never allowed from a project |
| `allowed_apps` | `["*"]` | app profile ids a project may override |
| `trust` | `"ask"` | `ask`, `never` (render only, no prompt) or `always` (`keyvo doctor` warns) |

- **Trust.** A project layer is rendered as soon as it is found, but none of
  its actions runs until the project is trusted. `~/.config/keyvo/trust.toml`
  records, per project root, the SHA-256 of the three files' contents, the
  date and the decision. Any change to the files invalidates the record;
  `keyvo project trust [<root>]` (and the GUI prompt) lists every action the
  layer would run and the keys it overrides before asking. `keyvo project
  revoke <root>` and `keyvo project list` complete the command. Pressing an
  untrusted project key does nothing but write a trace entry with the block
  reason. The threat model is in `SECURITY.md`.

### 7.5 Recipes and starter packs (M1, mini-wizard M4a)

A recipe is a named action resolved at press time:

```toml
action = { type = "recipe", id = "project.build" }
```

Resolution order: parameters in the project layer (`[recipes."project.build"]`
in `.keyvo/local.toml` or `.keyvo/profile.toml`), then detectors (file
presence at the project root: `Cargo.toml`, `package.json`, `platformio.ini`,
`CMakeLists.txt`, `west.yml`, `*.tsproj` with a "configure manually" hint),
then the two-step mini-wizard (tool, then command line) whose answer is written
to `.keyvo/local.toml`. Recipes are TOML files in the bundle,
`~/.config/keyvo/recipes/` and `.keyvo/recipes/`, each declaring its
parameters, detectors and the concrete action template. "Expand to a concrete
action" (CLI and GUI) rewrites the key to the resolved plain action.

Starter packs (Rust, web, Python, embedded, PLC, AI-Assisted Coding) are
profile bundles (§7.3) built from recipes; they are copied into the user's
profiles through the trust gate and never referenced. "Make this a keyvo
action" from shell history yields a plain `exec` key.

**As built (pre-alpha, catalogue v1, #119).** The actions catalogue's
entries are recipe packs ahead of the `recipe` action type: one TOML file
per entry, its id the file name, with `schema_version = 1`, a `[recipe]`
table (`group` = `general`, `system` or `apps`; `name`; `icon`; `label`)
and one or more `[[variants]]` ("What it does": `id`, `name`, and an
`action` of the profile schema, `chord` or `exec`; the first is the
default). keyvo bundles `contrib/recipes/` into the daemon (General: Open
folder, Open website, New email; System: Lock the desktop, Volume up and
down, Mute, Brightness up and down, Open a settings page; Apps: Spectacle,
Calculator, VS Code, Konsole), commands for KDE Plasma first with the
GNOME and X11 differences in each pack's comments, and reads the user's
`recipes/*.toml` (a `.toml` file of at most 8 KiB, at most 64 files; a user
pack replaces the bundled one of its id). The schema is
`schemas/recipe.schema.json`; a bad pack is a diagnostic with file, line
and column and is left out. Applying a recipe writes the variant's action
and the pack's label and icon into the key as plain fields through the
round-trip editor (the color stays); the profile never references the
pack, and no detector or parameter resolves at press time. The socket's
`recipes` request lists the packs (§8).

---

## 8. Scripting and control API

**Outward (key → script).** `exec` actions receive `KEYVO_KEY`,
`KEYVO_PROFILE`, `KEYVO_PAGE`, `KEYVO_WINDOW_CLASS`, `KEYVO_WINDOW_TITLE`,
`KEYVO_PRESS`, `KEYVO_SOCKET`, `KEYVO_LAYER` (`global`, `app`, `project`,
`overlay`) and, when a project context is known, `KEYVO_PROJECT_ROOT`,
`KEYVO_PROJECT_NAME`, `KEYVO_GIT_BRANCH`. Exec is always non-blocking with a
configurable timeout; the daemon never blocks on user scripts or network. A
script selects a custom key state by printing `KEYVO_STATE=<name>` (§7.2).
`keyvo exec-check <script>` runs a script with the environment a key would
give it and prints the trace entry.

As built (pre-alpha, `keyvo_daemon::exec`): the command runs as `sh -c
<cmd>` in its own process group, with stdin and stdout on `/dev/null` and
stderr in the daemon's log. It receives `KEYVO_KEY`, `KEYVO_PROFILE`,
`KEYVO_PAGE`, `KEYVO_PRESS`, `KEYVO_LAYER` (the layer that supplied the
key: `global` for a key of the global profile, also one under an app
profile, else `app`; `KEYVO_PROFILE` stays the active profile) and
`KEYVO_SOCKET` (the daemon's control socket, so the command can run
`keyvo ctl`); the window and project variables come with their slices,
and `KEYVO_STATE` is not read yet. A Dialpad control's action (beta,
#188) gets `KEYVO_CONTROL` (`dial`, `roller`, `button:N`) instead of
`KEYVO_KEY`; a turn's runs are one `exec` each. Its exit code is the
trace result (`exited`, with the duration); past `[timing].exec_timeout`
the whole process group is killed and the result is `failed` with "timed
out after <n> ms". A chord's result is `sent` with its key events, or
`failed` with the injector's error.

As built (beta, #183): after the key's action failed (§7.2), the daemon
runs the key's `on_fail` once through the same paths, a chord through the
injector, an `exec` with `exec_timeout`, a `launch` with `launch_settle`
(and no pending launch, §6.3). It gets the key's `KEYVO_*` variables and
`KEYVO_FAILED_CODE`: the exit code, `timeout` past `exec_timeout`,
`signal` for a command a signal ended, `failed` for an action that did
not run (no `sh`, a chord not sent). Its record is a second trace entry
of the press (below); its own failure is traced and runs nothing more.

As built (pre-alpha, #155): a `launch` action runs the same `sh -c <cmd>`
with the same environment, stdin and stdout on `/dev/null` and stderr in
the daemon's log, but in a new session (`setsid`), so it leaves the
daemon's process group and session and outlives the daemon. The daemon
watches it for `[timing].launch_settle`. A command that ends within it is
reported as an `exec` would be: `exited` with code 0 is `success` (an
instance that is already running hands over and exits 0), another code is
`error` (`sh` reports a missing command with 127; a Flatpak id that is not
installed exits 1), and a spawn error is `failed`. A command still running
then is let go: the result is `started` with the shell's `pid` and the key
shows `success`. A task of the daemon still waits for the child, so its
end leaves no zombie and changes no trace record; once the daemon exits,
the system reaps it. `exec_timeout` does not apply. A failed `launch` runs
the key's `on_fail` as a failed `exec` does; a `started` one does not. A
daemon run as a systemd unit with the default
`KillMode=control-group` would still stop a launched application with
the unit, since `setsid` does not leave the cgroup.

As built (beta, #184): the user unit `contrib/systemd/keyvo-daemon.service`
sets `KillMode=process`, so `systemctl --user stop keyvo-daemon` (and a
restart after a crash) signals the daemon alone and the launched
applications keep running in the unit's cgroup. The unit's other keys:
`After=` and `PartOf=graphical-session.target` (it starts with the
graphical session and stops with it), `ExecStart=%h/.local/bin/keyvo-daemon`,
`Type=simple` (no readiness notification; stderr goes to the journal,
`journalctl --user -u keyvo-daemon`), `Restart=on-failure` with
`RestartSec=2`, `Slice=app.slice`, `WantedBy=graphical-session.target`. A
user service does not see the shell's `PATH`: `exec` and `launch` commands
name their binaries with a full path, or the user sets `PATH` in
`~/.config/environment.d/`. A transient scope per launch (so a launched
application leaves the daemon's cgroup) is a later slice.

A `launch` with `wait_focus` on also opens a pending launch (§6.3, #179)
before the spawn; the daemon starts the shell itself and hands its pid to
the state machine at once, so a window that comes before the settle ends
is judged by its lineage too. A spawn error or a non-zero exit drops the
pending launch; an exit 0 within the settle (the hand-over to a running
instance) and a `started` keep it. The whole rule depends on KWin's focus
stealing prevention: at its default level (Low) every new window is
activated, which is why a launch moves the focus at all; at Medium or
above a new window only demands attention, no app switch comes and the
pad stays where it is. `docs/hardware-notes.md` records the owner's level.

**Inward (script → daemon).** Unix domain socket at
`$XDG_RUNTIME_DIR/keyvo/keyvo.sock`, mode `0600`, owned by the session
user (named pipe on Windows). The daemon and every client resolve it the
same way: the `--socket` flag, else `$KEYVO_SOCKET` when set and not
empty, else the default. Newline-delimited JSON, **versioned from day
one**: every message carries `"v": 1`. Trust model: same as `ssh-agent`:
any process running as the user may control the pad (see `SECURITY.md`).
No D-Bus façade in v1 (ADR-0003); the KWin script talks to a private D-Bus
object owned by the daemon, which is an implementation detail, not an API.

Commands (v1): `set_key`, `set_badge`, `set_state`, `push_overlay`,
`pop_overlay`, `switch_profile`, `set_mode`, `set_page`, `notify_context`
(external hook asserting the active context: class, title, and optionally
`cwd` and the sender's pid for project binding, §6.2), `press` (a key or
control press from a client; with `"dry_run": true` the daemon resolves and
traces but executes nothing: the simulator's path), `reload`, `status`,
`trace` (`last`, `follow`), `project` (`trust`, `revoke`, `list`), `setup`
(drive the first-run steps, §10), `doctor` (not built; the beta has the
CLI's `keyvo doctor`, §5.10, §12), `subscribe` (event stream:
key events, profile/page/mode changes, device connect/disconnect, focus and
context changes, trace entries, config reloads, `trust_requested`,
`recipe_unresolved`). `keyvo ctl <command> [json]` is the CLI front-end;
`keyvo ctl subscribe` streams events, `keyvo ctl trace` prints the last
entries and `keyvo ctl tail` follows them. Wire details live in
`docs/socket-api.md`; fixtures in `tests/protocol-fixtures/` are shared by
the Rust, TypeScript and C# clients.

As built (pre-alpha, `keyvo_core::socket` and `keyvo_daemon::socket`):

- The commands are `status`, `subscribe` (classes `key`, `profile`, `page`,
  `mode`, `device`, `context`, `trace`, `config`, `session`, `dial`,
  `modifier`, with an optional replay
  of the newest trace entries), `press` (`key` or `control` with `delta`,
  `kind`, `dry_run`; `hold` or `release` with a `key`),
  `reload`, `notify_context` (`class`, `title`), and `recipes` (the bundled
  and the user's recipe packs with their variants, `problems` naming each
  pack left out, §7.5). Any other command and any unknown field is
  rejected as `invalid_request`.
- A focus report of a class in `[focus].hold_classes` (§6.1) goes out as
  a `context` event with `held: true` and is the `context` of `status`;
  the view does not change, so no `profile`, `page` or `mode` event
  follows.
- The view in `status`, in the `subscribe` result and in the `profile`,
  `page` and `mode` events is `profile`, `page`, `mode` and `index`, the
  position in the shown profile's page list (§6.1, #151); a change of the
  index alone is a `page` event. The view also carries `locked`, `true`
  while the session is locked (§6.1, #158); a client reads a missing
  `locked` as `false`.
- A lock or an unlock is a `session` event (`{"v":1,"event":"session",
  "locked":true}`), sent after the `profile`, `page` or `mode` events of
  the same change, so a subscriber of `session` alone sees every lock and
  unlock. A `press` under the lock, `dry_run` included, is answered with
  its trace entry, result `blocked` with `reason` `session locked`; the
  socket itself answers as before, to the same user (`SECURITY.md`).
- The Dialpad (beta, #188): `status` carries a `dialpad` block next to the
  Keypad's `device` (`connected`, `slot`, `rollers` with their
  `increments`, `error`), and a change of it is a `dialpad` event in the
  `device` class. A turn is a `dial` event (`roller`, `delta`, `actions`:
  how often its action ran) in the new `dial` class; a button edge is a
  `key` event with `control: "button:N"` and no `key`. Trace records of a
  Dialpad control carry `control` (`dial`, `roller`, `button:N`) instead
  of `key`; a turn traces one record per run, none below one `step` or
  without a binding, one `blocked` record under the lock. `press` takes
  `control` instead of `key`: `{"control":"dial","delta":1}` turns the
  dial (nonzero `delta`, clockwise positive), `{"control":"button:2"}`
  presses a button; `dry_run` as for a key, a turn's result carries
  `runs`. A turn from a client sends no `dial` event, as a press from a
  client sends no `key` event.
- The dial modifier (beta, #189, §6.1): the view carries `modifier`
  (`{"key":7,"mode":"held"}`, `mode` `held` or `toggle`) while a modifier
  key is active and leaves it out otherwise (a client reads it as
  `null`); a change is a `modifier` event in the new `modifier` class
  (`{"v":1,"event":"modifier","modifier":null}` at the end), sent after
  the other events of the same change. `status` and `keyvo ctl status`
  print it with the view. A turn taken by a modifier key is traced with
  `key` (the modifier key), `control` (`dial` or `roller`), `modifier`
  (`held` or `toggle`) and the `layer` that supplied the key; the action
  is the key's binding, `KEYVO_KEY` and `KEYVO_CONTROL` both set for an
  `exec`. A press that arms or ends a toggle, and a client's hold or
  release, is traced with `modifier` and the result `modifier` (`active`
  `true` or `false`), no action. `press` with `"hold": true` holds a held
  modifier key of the active page down as the device does,
  `"release": true` lets it go (its action runs, and its entry answers,
  when no turn took its binding); either with a key that is not a held
  modifier key, or not held, is an `invalid_request`, and `hold` and
  `release` go with `key` alone, one of them, without `kind: long`. A
  plain `press` of a toggle key toggles it, of any other key ends the
  armed toggle; a `press` of a Dialpad button ends it too. With `dry_run`
  a hold, a release or a toggle press resolves and answers with `modifier`
  set and the result `dry_run`, and changes nothing.
- Clients ignore unknown fields in responses and events, so a later daemon
  may add fields within version 1.
- Every response echoes the request's `id` and carries `ok` with `result`
  or `error` (`code`, `message`).
- A dry-run press returns its trace entry marked `simulated` with the
  result `dry_run`. It sends no chord, runs no `exec`, and leaves the
  key's look alone.
- The daemon refuses to start when the socket's directory is not private
  or another daemon answers on the socket. It replaces a stale socket
  file and removes the socket when it stops.
- Limits, under `[daemon]` in `config.toml`, read when the daemon starts
  (a reload does not change them):

  | Key | Default | Meaning |
  |---|---|---|
  | `socket_line_bytes` | 65536 | longest request line in bytes, without its newline; a longer one gets an `invalid_request` error and the connection is closed. At least 1 |
  | `socket_connections` | 32 | connections served at once; one more gets an `unavailable` error and is closed. At least 1 |

  A failed `accept` waits `[timing].accept_backoff` (§6.1). The client
  side reads daemon lines up to 1 MiB (`keyvo_client::client::LINE_LIMIT`).
- The clients (`keyvo_client::open`, shared by `keyvo ctl` and the app)
  send nothing before three checks pass: the user's uid is mapped in
  the client's user namespace (a client running as the overflow uid is
  refused), the socket's directory is private to the user (a `0755`
  directory is refused) and the listening process runs as the user
  (`SO_PEERCRED`, uid only; a sandboxed client sees the daemon as pid 0).
- `keyvo ctl <command> [json]` prints the response line and exits 1 on an
  error. `keyvo ctl subscribe` prints every later line. The socket is
  `--socket`, else `$KEYVO_SOCKET`, else the default.
- The wire format, the error codes and the ordering guarantees are in
  `docs/socket-api.md`. The fixtures are in
  `tests/protocol-fixtures/socket/`.

**Trace.** Every press, real or simulated, produces one trace entry:
timestamp; trigger (device, key or control, press kind, real or simulated);
context snapshot (window class and title, project root and name, git branch,
trust state, mode, page, overlay stack); layer resolution (which layer
supplied the binding, which were shadowed); each condition with its result;
the resolved action with every variable substituted (recipe id and detector
when applicable); the execution result (`exec`: exit code, duration, bounded
stdout and stderr tail; `http`: status and body excerpt; `chord`: event
sequence); the `on_fail` branch taken; the block reason (untrusted project,
disabled key, dry run). Entries live in a ring buffer (`[trace].ring`,
default 1000) and in `~/.local/state/keyvo/trace.ndjson` (rotated at
`[trace].max_size`, default 8 MiB, `[trace].keep`, default 3). Debug mode
(`[trace].debug`) adds HID-level events to a separate device log, never to the
entries. The trace core lands in M1, the context fields in M2. The record
type (`keyvo_core::trace::TraceRecord`) and the press resolution that fills
it (`keyvo_core::press::resolve`) are defined in `keyvo-core`; the ring
buffer, the file and `keyvo ctl trace|tail` live in the daemon. Pre-alpha
subset of a record: timestamp, trigger (press kind, simulated), profile id,
page, key (or, for a Dialpad control since #188, `control`: `dial`,
`roller`, `button:N`), the resolved action, and the result: `unbound`, `dry_run`,
`pending`, `sent` with the chord's key events, `exited` with exit code and
duration, `started` with the `pid` of a `launch` that was let go,
`failed` with the reason, or `blocked` with the block `reason` (as built
only `session locked`, a press under the session lock, §6.1: no layer, no
action, nothing run; the untrusted project's block comes with M2), and
the `branch` (#183): absent for the key's own action, `on_fail` for the
run of the key's `on_fail`, a second entry of the same press with its
own action and result after the key's entry; one
serialized record per line is in
`crates/keyvo-core/tests/fixtures/trace/records.ndjson`. The daemon keeps
the newest `[trace].ring` entries (default 1000, the only `[trace]` key so
far) in memory: device and client presses, dry runs, and each `exec`
result as a second entry once it ends. `status` reports their number, and
`subscribe` with `replay` sends the newest of them. They are lost when the
daemon stops; the file, `max_size`, `keep` and `debug` come with M1-8.

**Plugins.** `~/.config/keyvo/plugins/<name>/plugin.toml` declares an
executable, its arguments, restart policy and the profiles it serves. The
daemon supervises the process: JSON events on stdin, commands on stdout,
restart with back-off, log capture. Plugins are how integrations that need
state (tmux, git status, editor sessions) are built.

**No embedded logic in v1.** Lua (`mlua`) was considered and rejected for v1
(ADR-0003): plugins and `exec` cover stateful keys, and one process model is
easier to secure and test. Revisit after M2 if the plugin path proves too
heavy.

**First-class integration (planned).** `integrations/claude-code/` is a Claude Code
plugin: hooks (`SessionStart`, `Stop`, `Notification`, …) push overlays over
the socket (working / waiting / needs-permission), slash commands map to
keys, and an MCP server exposes `set_key`/`push_overlay` to the agent. Tier 1
integrations: Claude Code, VS Code extension, tmux, git. Tier 2: Neovim,
justfile page, `gh`, `kubectl`, other agent CLIs, Kitty/WezTerm.

---

## 9. Rendering

- Layered key image: background (solid/gradient) → icon (SVG, tinted) →
  label → corner badge. Theme file controls font, radius, icon scale,
  default colors, manual-mode styling.
- Icon sources: `lucide:`, `phosphor:`, `simple-icons:` (bundled, MIT/CC0),
  `file:` (user PNG/SVG).
- Pipeline: `tiny-skia` raster + `resvg` for SVG + embedded Inter (OFL)
  text → baseline JPEG via the `image` crate's encoder (quality 100, as
  Options+ sends, #72; it writes 4:4:4, no chroma subsampling, §5.5).
  `jpeg-encoder`, named here before, carries the IJG licence, which
  `cargo-deny` refuses. Rendered tiles are content-hashed and cached;
  rendering runs off the device thread. The writer splits a crop above
  the device limit into bands (§5.5).
- Key tiles are rendered at the geometry the device published (§5.4) and
  composited into one canvas of the panel size (#72): the panel
  background with every key's tile at its rectangle
  (`keyvo_core::render::Canvas`; `Renderer::canvas` draws the background,
  `Renderer::draw` one key, `Canvas::crop` cuts an area for the device,
  `Panel::bounds` gives the bounding box of the dirty keys). Drawing a key
  resets its rectangle to the panel background first, so nothing of its
  earlier tile stays in the rounded corners. The device gets crops of this
  canvas, never a key tile over an older image (sequence in §5.5); the same
  canvas is the full-panel path for previews (`Renderer::panel`), and
  splashes will use it.
- Panel background (pre-alpha, #77): the theme's panel color, black,
  tinted 20 % toward the profile's accent (theme `panel_tint`; `#4e8bd6`
  gives `#101c2b`), black for a profile without an accent. It is the
  canvas background between the keys and under a key tile's transparent
  corners, so the corners match the panel around them. No `[render]` key:
  the color follows the accent, and the tint moves to the theme file with
  the other theme values. #72 adds no `[render]` or `[timing]` key either:
  the queue depth (two, as Options+) is a constant and images are paced by
  the device's response, not by a timer.
- Renderer as built (`keyvo_core::render`, pre-alpha subset): a key spec
  (background color, profile accent, icon, label) renders to a tile of any
  size; every size and position is a fraction of the tile height, so other
  key sizes keep the proportions. Layers: the background, a rounded
  rectangle in the key's `color` or the theme's dark gray; the accent, a
  rounded bar centered near the bottom edge, drawn on every key of a
  profile with an `accent`; the icon, centered, smaller and higher when
  the key has a label; the label, Inter 4.1 semibold, below the icon or
  centered without one, shrunk from 17 % to 12 % of the tile height to fit
  and then cut with an ellipsis. Icon and label are white on a dark key
  and near-black on a light one (Rec. 709 luma above 140). Icons are the
  whole lucide 1.61.0 set (ISC, about 1900 icons, about 1.1 MB of the
  daemon binary) under `crates/keyvo-core/assets/lucide/`, generated with
  the icon table by `tools/lucide/generate.py` from the pinned release
  asset, with the deprecated names of lucide's metadata accepted
  (`git-commit`, `circle-help`, `home`); an unknown icon, and every
  `phosphor:`, `simple-icons:` and `file:` reference until their issues,
  draws `lucide:circle-question-mark` and `keyvo preview` names it. Only
  the embedded font is loaded, never a system font, so a spec renders to
  the same pixels everywhere. Badges and the feedback looks are described
  below. The theme file, gradients, caching and the size guard come with
  later issues.
- The lock page (#158, `keyvo_core::render::lock_page`): while the session
  is locked (§6.1) every key is *bare*, no key face, so the panel
  background shows through (tinted with the accent of the profile the
  unlock returns to), and the middle key (key 5 of nine) shows
  `lucide:lock` in the theme's muted foreground (`#8b9099`), no label,
  over that profile's accent bar. A key spec carries `bare` and `muted`
  for this. The page goes through the composite path like any other; the
  snapshot is `crates/keyvo-core/tests/snapshots/render__lock-page.snap.png`.
  An animated lock (a pulse within the device's `maxImageFPS`) and a
  configurable lock page are later slices.
- Animation = frame push; renderer supports spinner/progress/blink
  primitives. Sustainable frame rate is measured in M0-1 (plan §5 item 9)
  and documented; there is no native animated format.
- Feedback primitives (M1), the built-in looks of the automatic states
  (§7.2): `running` dims the icon and draws an accent arc with the spinner
  primitive, replaced by a static badge after `[render].long_run_threshold`
  (default 10 s); `success` shows a green badge and a single flash and reverts
  after `[render].feedback_hold` (default 2 s); `error` shows a red badge that
  stays until the next press or until the GUI inspects the trace entry. The
  theme file defines the three looks; per-key `states` override them. A
  desktop notification is optional, default only on `error`. The GUI shows the
  same looks in its mirror and respects the system's reduced-motion setting
  for the flash (the pad does not).
- Feedback as built (`keyvo_core::feedback`, pre-alpha subset): one
  `Feedback` per key moves through the states with a `Clock`'s instants
  and names the time of its next change on its own (long run, end of the
  success hold), so the daemon repaints then. Only `exec` actions have
  automatic states; a chord is instant. A press ends `success` and
  `error`, not `running`; a result that is neither (a dry run) returns the
  key to its own look. The renderer draws the looks from the theme
  defaults: `running` fades the icon 60 % toward the background and
  strokes a 90° arc around it in the profile accent (white without one),
  starting at the key spec's `spinner` angle, which the daemon turns to
  animate; the long run, `success` and `error` draw a corner badge (30 %
  of the tile height) in orange with `refresh-cw`, green with `check`, red
  with `x`. A key's `states.running`, `states.success` or `states.error`
  replaces icon, label, background and badge text; the badge keeps the
  look's color. A custom state's badge text draws in neutral gray. The
  success flash and the desktop notification come with the daemon's frame
  push.
- The `modifier` look (beta, #189), a feedback primitive next to
  `running`, `success` and `error`: the key that has the dial and the
  roller (§6.1) shows a ring along its edge (a stroke of 4.5 % of the
  tile height, the theme's `modifier_ring`) in the profile accent (white
  without one) and a dial badge (`lucide:circle-gauge`) of the badge size
  in the top left corner in the same colour, so a key's own corner badge
  (top right) stays. Icon and label are untouched. It shows for the whole
  active time, the hold of a held key and the armed time of a toggle, on
  the page the key was pressed on, over any other look of the key; a
  key's `states.modifier` replaces icon, label, background and badge text
  as for the other looks. The snapshots are
  `crates/keyvo-core/tests/snapshots/render__looks__modifier.snap.png`
  and `render__looks__modifier-badge.snap.png`. `keyvo preview` draws no
  modifier state.
- The daemon (pre-alpha) paints the looks: `running` when an `exec`
  starts, then `success` or `error` from its result; it repaints the key
  at the feedback's next change (long run, end of the success hold). The
  spinner arc is not turned yet: animating it needs its own `[timing]`
  key and comes with the frame push.
- `keyvo preview <profile> [--page N] -o <file.png>` renders one page
  (counted from 1, default 1) onto the panel with no hardware: the
  captured MX Creative Keypad geometry (480×480, the nine key rectangles
  of §5.4), the profile's panel background between the keys.
  `<profile>` is a profile file when it names an existing file or ends in
  `.toml`, else a profile id from the
  configuration directory (§7.1). `insta` snapshot tests cover every
  renderer primitive and the demo profile
  (`crates/keyvo-core/tests/snapshots/`, PNG files next to the `.snap`
  metadata).
- Accent color per profile is the primary context indicator; optional
  ~300 ms full-grid splash on profile switch, `[render].splash`, **default
  `false`** (owner decision, §16 question 16): the accent bar on every key
  already shows the switch. A Settings toggle in the GUI turns it on.
  As built (pre-alpha), `[render].splash` is parsed and validated but has
  no effect: the splash comes with M1-6.
- Test policy: `keyvo-core` ≥ 85 % and `keyvo-hid` ≥ 80 % line coverage,
  goals measured via `cargo-llvm-cov` → codecov, whose org gates block a PR
  (patch >= 80 %, project at most 1 % below the base);
  `FakeHidTransport` integration tests replay golden captures.

---

## 10. Desktop app (`app/`, M4a)

Screens, flows, states and the keyboard map are specified in
`docs/design/gui-ux.md`; the decisions behind them in
`docs/plans/2026-10-24-gui-ux-grill-decisions.md`. This section fixes what
the rest of the system must provide.

- Tauri 2 with a Svelte 5 frontend (TypeScript). Audience: developers only;
  the GUI exists for layout editing, for debugging a customised pad and for
  onboarding a junior developer, not to hide the text files.
- **Device mirror.** Home is a live copy of the pad: the GUI subscribes to the
  daemon and renders with the same `keyvo-core` renderer, so a tile on screen
  is bit-identical to the LCD. Profiles are edited on the same mirror.
- **Native theme by default.** A Rust-side theme bridge reads the desktop's
  colours, accent, fonts, contrast and motion settings (KDE, GNOME, Windows in
  M5) and exposes them as CSS custom properties, followed live. Built-in
  themes are opt-in overrides.
- **Immediate apply, undo, no save button.** Every edit goes through
  `keyvo-core`'s `toml_edit` round-trip (ADR-0004) and lands on disk at once;
  the daemon hot-reloads. A snapshot of the file is taken under
  `~/.local/state/keyvo/snapshots/` before every GUI write (`keyvo profile
  restore` brings one back); undo and redo are per GUI session. The GUI
  watches the files and reloads on external change; on a schema error the
  daemon keeps the last good state and the GUI shows file and line and opens
  the external editor. The GUI never rewrites a profile file wholesale.
- **Simulator.** A sandbox switch copies the edited profiles to
  `~/.local/state/keyvo/sandbox/`; the virtual pad renders the copy; Apply
  merges it into the real files, Discard drops it. Without a device the GUI
  starts in the simulator writing directly. Simulated presses are `press`
  requests with `dry_run` (§8): resolved and traced, never executed. Without
  a daemon the simulator runs in process against `keyvo-core`.
- **Behaviors.** The GUI's view of a key binding (trigger rows per press kind,
  conditions, actions, feedback, "Test this behavior"); not a TOML entity.
  Templates and starter packs are copied, never referenced.
- **Trace.** One entry per press (§8), reachable from the result line, the
  Home event list and a key's error badge, rendered as a drawer.
- **Talks to the daemon only via the socket API** (no device access from the
  GUI, no privileged code). GUI edits never use `set_key`.
- **First run.** `keyvo setup` (interactive CLI, M1) owns the four steps:
  device and system (`doctor` results with one button per fix, each with its
  own approval: udev rule via `pkexec`, KWin script, systemd user unit),
  starter pack (previewed in the simulator), integrations (commands shown,
  all skippable), review and Finish as a transaction (file list with backups,
  system commands, atomic apply, schema validation, reload, smoke paint). The
  GUI wizard drives the same steps over the `setup` socket command; each step
  is re-runnable from Settings. No tutorial overlay; one dismissable hint bar.
- **Shortcut entry** is text-first with capture as a helper
  (compositor-grabbed chords cannot be captured). `exec` scripts open in the
  configurable external editor; the GUI embeds no code editor.
- **Keyboard first.** Full keyboard traversal, `1`..`9` and arrows on the
  grid, `Ctrl+K` palette, `Ctrl+1`..`7` rail, `aria-label` everywhere, 32 px
  minimum targets, 4.5:1 contrast on every token pair, reduced motion
  respected. English only in v1, i18n-ready.
- **Dialpad** appears as a second, collapsible panel in the mirror (schematic
  dial, roller, buttons with GUI labels); MX Master 4 and MX Mechanical
  controls (M3) appear as trigger sources in Behaviors and in Devices.
  As built (beta, #188): `Dialpad.svelte` draws
  `app/src/assets/device/dialpad.svg` under the mirror on the Pad screen
  with the four buttons, the roller and the dial as selectable controls
  showing their labels (a control the shown app profile leaves out shows
  the global one, dimmed). It collapses on its heading, starts collapsed
  while no Dialpad is connected and neither the shown nor the global
  profile has a `[dialpad]` table, and says `No Dialpad connected` while
  none is. A `dial` event or a button's `key` event pulses the control
  for 400 ms. Selecting a control opens the Inspector's control editor
  (`ControlEditor.svelte`): label, `step`, `cw` and `ccw` for the dial and
  the roller, label and action for a button (its `on_fail` read-only), the
  action fields shared with the key editor (`ActionFields.svelte`), "Override
  here" for a control that comes from the global profile, Clear, and its
  own undo. The catalogue assigns to the selected control as to a key: a
  button's action (and label), the last-focused direction of the dial or
  the roller (`cw` by default). The writes go through the Tauri commands
  `control` and `edit_control` (`keyvo_client::Editor::control`,
  `set_control`). The Preferences Devices tab stays `soon`.
- The dial modifier in the app (beta, #189): the Inspector's key editor
  has a Modifier section with a Mode select (Off, Held, Toggle) and the
  dial and the roller editors of the control editor (`cw`, `ccw`,
  `step`), collapsed while the mode is Off; Off writes none of
  `modifier`, `dial` and `roller`, Held and Toggle write the mode and at
  least an empty `dial` table. The catalogue assigns to the focused
  direction there as in the control editor. While the view carries a
  `modifier`, the mirror draws the ring on that key, and the Dialpad
  panel shows the key's labels on the dial and the roller it binds (the
  table's `label`, else the key's) with `(held)` or `(toggled)` in place
  of the profile's. The mock daemon reads the three fields from the
  contrib profiles and, as its presses are dry runs that change nothing,
  gives the dial to the held key 7, then the toggled key 8, on a 5 s
  timer. The app's `Daemon` interface has no `hold` or `release` yet (the
  Tauri `press` command takes a key and a kind).
- Tray icon from the GUI in v1. `keyvo profile export|import` in the header
  menu.
- Flatpak (M4b, as built in #182): exactly `--share=ipc`,
  `--socket=wayland`, `--socket=fallback-x11`, `--device=dri`,
  `--filesystem=xdg-run/keyvo` (the daemon's socket directory, created
  0700 by the daemon) and `--filesystem=xdg-config/keyvo:create` (the
  profiles the editor writes, at the same path as on the host). No device
  access, no `org.kde.KWin`, no system bus: the daemon runs on the host
  (later as a systemd user service) and the Flatpak ships the GUI and the
  CLI. The snapshots stay in the app's own state directory
  (`~/.var/app/io.github.qubernetic.keyvo/.local/state/keyvo/snapshots`);
  "Open in editor" goes through the OpenURI portal, which the runtime's
  `xdg-open` uses. Inside the sandbox the CLI reaches the daemon
  (`keyvo ctl`); the device commands (`probe`, `keys`, `fill`) and
  `preview` (which reads `$XDG_CONFIG_HOME/keyvo`, the sandbox's own) run
  from the host build.
- Windows (M5): the same app behind the C# plugin's named pipe; differences
  only in the theme bridge and `doctor`; no project layer via shell hook, no
  MX triggers, no desk state in v1.

**As built (pre-alpha scaffold).** `app/` is a Vite project with Svelte 5,
TypeScript, vitest and jsdom, managed with `npm` (exact versions, committed
`package-lock.json`). `app/src/lib/protocol.ts` holds the protocol v1 types
and parsers and passes the shared socket fixtures. The theme bridge is
reduced to the tokens of
`docs/design/gui-ux.md` §7 as CSS custom properties with fixed defaults in
`app/src/styles/tokens.css` (the feedback states use the pad theme's
colours); the KDE and GNOME readers come with M4a-1. `app/src-tauri`
(identifier `io.github.qubernetic.keyvo` since #182, bundling off) has seven commands, all
over `keyvo-client`: `status`; `tiles` (a page from the daemon's
configuration directory, `config_dir` of its `status` reply, at the key
count and tile size the frontend passes from the reported geometry; each
key as a PNG data URL with its label, action and layer); `outline` (a
profile's page count and accent from the same directory,
`keyvo_client::tiles::outline`); `press` (a dry-run press, the trace entry back); `subscribe` (a
subscription with a trace replay on its own thread, each update emitted on
the Tauri event `keyvo://update`; a newer `subscribe` stops the older one);
`key` (a key's file and fields, `Editor::key`, and the `line` on which the
key's table starts in the file, absent for a key the page does not
define); `edit_key` (new fields written into a key, `Editor::set_key`, the
fields before it back and the key's `line` in the written file; an error
is a `Problem` object with `message` and, when it has a place, `path`,
`line` and `column`). `build.rs` declares the seven commands in the Tauri app
manifest, and the main window's capability
(`app/src-tauri/capabilities/default.json`) allows exactly those and
`core:event:allow-listen`/`allow-unlisten` for `keyvo://update`; there is
no `core:default` and no plugin, so the page cannot reach the window,
path, menu or other core APIs. The CSP loads only the app's own code and
connects only to the Tauri IPC. `app/src/shell.test.ts` keeps the command
list, the capability and the CSP as stated here.
`just app-test` runs `npm ci`, `svelte-check`, vitest, `npm audit
--audit-level=high` and `cargo deny` on the shell's own lock
(`app/src-tauri/deny.toml`); `just app-build` builds the frontend and the
shell into `bin/keyvo-app`.

**As built (pre-alpha shell).** The window opens at 1200 x 760 and shrinks
to 960 x 640 (`tauri.conf.json`). It has the header, the navigation rail
and the columns of `docs/design/gui-ux.md` §2:

- **Rail.** Pad (screen id `home`), Profiles,
  Behaviors, on `Ctrl+1` to `Ctrl+3` (pad screen brief §5.1), lucide icons
  (`app/src/assets/lucide`, the pad's lucide release) with labels, icons
  only below 1100 px. Pad and Profiles are built. Behaviors carries
  `aria-disabled`: it stays in the tab order, hovering shows "Coming
  soon", keyboard focus adds the sentence naming M4a-5, and neither a
  click nor the shortcut navigates; the shortcut focuses it instead.
  Devices, Integrations and Projects (and Settings as General) became
  tabs of Preferences.
- **Header.** The app name; the device selector (the Keypad while the
  daemon reports it connected, and "Simulator"; only "Simulator" while
  the Keypad is missing or the daemon unreachable; the choice changes
  nothing else yet); the Profile dropdown (the list of `profile_list`,
  read again on every reload: the global profile first, then by
  priority, the profile on the pad marked "on the pad"), which shows the
  pinned profile, else the pad's, and pins the choice (`app/src/lib/pin.ts`:
  undefined follows the pad, a pinned profile starts on page 1, a view
  event never moves the pin, a profile gone from a re-read list unpins;
  the Pad's inspector and page tabs and the Profiles selection read it).
  The dropdown's accessible name says its state, `Profile, pinned` or
  `Profile, following the pad`; it is at most `24ch` wide and cuts a
  longer name with an ellipsis, its title and its option list carrying
  the whole name (#164). Beside it, the pin glyph (lucide `pin`), a toggle
  button named "Pin": pressed while the inspector is pinned, and pressing
  it then is Follow; not pressed while following, and pressing it pins
  the pad's profile and page; its title says which (`Pinned to <name>,
  page <n>. Press to follow the pad.` or `Following the pad. Press to pin
  <name>, page <n>.`); disabled while nothing is pinned and the daemon
  reports no view (#164). Then the Sandbox
  switch, disabled with its M4a-4 sentence, Apply and Discard not shown;
  the gear, which opens Preferences; and the overflow menu with Export,
  Import, Open config folder, Re-run setup step and About, disabled with
  "Coming soon", and Keyboard shortcuts.
- **Drawers.** One at a time, fixed at the right of the window, a
  non-modal `dialog` (`app/src/Drawer.svelte`): it takes the focus on its
  Close button, closes on `Esc` (unless a menu or field handled that
  `Esc`) and gives the focus back to the control that opened it. The
  trace entry (from the Pad only), Preferences and the keyboard map use
  it. Preferences has the tabs General, Devices, Integrations, Projects,
  each the sentence of its roadmap item (M4a-8, M4a-8, M4a-8, M4a-6),
  arrows, Home and End between the tabs. The keyboard map
  (`app/src/lib/shortcuts.ts`) lists the shortcuts that work (`Ctrl+1` to
  `Ctrl+3`, `1` to `9`, the arrows, `Space`, `Shift+Space`, `Ctrl+Z`,
  `Ctrl+Shift+Z`, `Esc`) and `Ctrl+X` and `Ctrl+V` with their M4a-4
  sentence.
- **Notices.** Every notice goes through `app/src/Notice.svelte`: the
  panel surface with a 1 px full border, the error colour on the border
  of an error only, a leading word ("Error", "Note"), the body in
  `--kv-fg`; an error is an `alert`. The token test refuses a side
  border over 1 px anywhere and a state-coloured border outside Notice.
- **Columns.** Each screen lays out its own columns by the window width
  (`app/src/lib/shell.ts` `layout`): the Pad's catalogue column from
  1200 px, the Profiles list column beside the editor from 1000 px and
  above it below. Each screen owns its height: `main` never scrolls, the
  columns do, each on its own (the Pad's drawing, inspector and
  catalogue; the Profiles list and editor, or the stacked Profiles screen
  as one), so a window shows one scroll bar per column and none beside
  them (#166). Every scrolling column reserves its scroll bar gutter
  (`scrollbar-gutter: stable`) and takes the theme's thin scroll bar, so
  no control sits under it. The Pad stays mounted while another screen
  shows, so the header follows the daemon; its `Ctrl+Z` then does
  nothing.
- **Tokens and controls.** `color-scheme: dark` on `:root` matches the
  dark token set, so the webview's own popups (the option list of a
  `<select>`) are dark. `app/src/styles/controls.css` gives every form
  control (button, input, select, textarea, option, menu, menu item,
  tooltip) a text and background token pair, 32 px minimum height and the
  focus ring from `--kv-accent`; reduced motion turns every animation and
  transition off. `app/src/styles/tokens.test.ts` checks that every rule
  in the stylesheets and components that sets a colour sets the text and
  its background from tokens, that no rule outside `:root` has a literal
  colour, that every such pair passes 4.5:1 and the focus ring 3:1.
- **Strings.** Every string is in `app/src/lib/i18n.ts` (English) and
  reaches the screen through `t(key, values)`; `app/src/strings.test.ts`
  keeps literal words out of the markup of every component under
  `app/src/`, in folders too.

**As built (pre-alpha Pad).** The Pad (`app/src/Pad.svelte`, screen id
`home`) follows the pad screen brief §5.2 and §6 and `docs/design/gui-ux.md`
§3.1, §5.2 and §8; it has no list column. On start the Pad asks `status`,
then subscribes with a replay of the newest 20 trace entries;
`app/src/lib/pad.ts` folds the updates the way `Mirror` does.

- **Mirror** (`app/src/Mirror.svelte`). A `grid` of the tiles of the page
  the pad shows, fetched again when the view changes and after every
  reload and write. Rows, key count and tile size come from the
  `geometry` the daemon reports for the Keypad (`status` and the `device`
  event, §8); the app hard-codes none of them. A key starts a new row when
  its top edge is below the middle of the row's first key. The tiles sit
  in the key cutouts of the Keypad drawing (`app/src/Device.svelte`,
  gui-ux §3.1), coloured from `--kv-device`, each cell placed by the
  percent rect of its cutout, and scale with it: the column's width, at
  most the panel's pixel width, so never above 1:1. A geometry whose key
  count differs from the drawing's cutouts puts the tiles on a plain
  panel at the reported rects instead. The last geometry stays when the
  Keypad goes; before any Keypad reported one (the daemon unreachable, or
  running without a Keypad), the drawing is the device with nine empty
  wells in its own cutouts and the caption "No pad", nothing to select
  (#165).
  A key the active app profile does not define shows the global key
  (§6.1), dimmed, with a "global" mark in its corner. The selected key has
  the accent outline, a key held on the device a dashed mark. A page the
  daemon's configuration cannot render shows why.
- **Selection and the pin.** Nothing is selected at first; the inspector
  asks for a key. A click on a tile, or `1` to `9` (up to the key count)
  and the arrows on the grid, select a key; a click never presses. The
  first selection pins the inspector to the profile and page the pad
  shows (`pinOnSelect` of `app/src/lib/pin.ts`), so a later view event
  moves the drawing and never the inspector. While the pin and the pad
  differ, a line under the page tabs leads with Follow, which unpins
  and keeps the key number, then says `Editing <pinned profile>, key
  <key>; the pad shows <profile>, page <page>.` (`page <page>` of the pin
  instead of the key while none is selected), both profiles by the name
  the header's dropdown shows (the id while the list does not know the
  profile, #150); the pinned page's tab carries the pin mark (Page tabs
  below), and the mirror draws no selection outline (#143, #164).
  A click on a tile then pins the pad's page again when the pin came
  from a key selection; a profile or page chosen in the dropdown, a tab
  or "Edit keys on the Pad" stays pinned (#143).
- **Page tabs** (`app/src/PageTabs.svelte`, #151). The page list (§6.1)
  in index order as a tablist by name (`Page <n>` for an unnamed one):
  the global profile's pages at index 0 and below as muted tabs with a
  dashed rim, labelled `<page>, a page of <global profile>`, then the
  shown profile's pages, context first. The list is the one of the
  pinned profile, else of the profile the pad walks; a pin on a global
  page at index 0 and below keeps the list it was chosen from, a pin on
  the global context page shows the global profile's own list. The
  profile the pad walks is the view's profile from index 1 on and the one
  seen before at index 0 and below (the global profile when none was);
  the daemon reports no change while a title change in the same app keeps
  the pad on a global page, so the tabs right of it can still name the
  profile before. While following, the selected tab is the `index`
  the daemon reports; while pinned, the pinned page's tab. While the pad
  shows another page than the pin, the pinned page's tab carries the
  lucide `pin` glyph before its name and `, pinned` after its name in its
  label and title, a mark that is not colour alone (#164). A tab is at
  most `16ch` wide and cuts a longer name with an ellipsis; every tab
  carries its whole name in its title (#130). Arrows
  wrapping, `Home` and `End`; a tab pins its page of the profile that
  owns it, so clicking `general` edits the global profile's general page.
  A profile with no pages adds no tab of its own. A page of the shown
  profile that is switched off (`enabled = false`, §6.1; beta, #156) has
  no index and is no tab: it sits among the shown profile's tabs in file
  order, muted with a dotted rim and its name struck through, the arrow
  keys skip it, and its switch turns it on again. "+" (add a page),
  after the shown profile's tabs, adds an unnamed page at the end of the
  shown profile through the editor and pins it (#156; disabled with its
  M4a-4 sentence before); a refused edit shows in the result line. At the right, the profile's accent as a 12 px swatch with the
  label "Accent" and its `#rrggbb`; none when the profile has no accent.
  The hardware page buttons page the pad only.
- **Held caption.** While the focused window is held (§6.1,
  `[focus].hold_classes`), a `status` line reads `keyvo-app focused,
  holding <profile>`, the profile the pad keeps.
- **Session lock** (#158). While the view is `locked` (§6.1) the mirror
  shows the lock page the pad shows (the shell's `tiles` command with
  `locked` renders `keyvo_core::render::lock_page`, so the tiles name no
  key), and the same `status` line reads `Session locked; the pad shows
  the lock.` in place of the held caption. The Inspector keeps editing the
  pinned page; edits are file writes. A blocked press reads `Key <n>:
  blocked, session locked` in the trace strip, and the trace drawer gives
  the reason under Block reason.
- **Trace strip** (`app/src/TraceStrip.svelte`). Under the inspector, one
  line "Recent events" with the newest three trace entries, newest first,
  each with its 24 h time: the key, the press kind, whether it was
  simulated, the layer that supplied the key (`app layer` or `global
  layer`), the action and its result. A press and its `on_fail` run are
  one line, the run after a `;` as "on_fail: <action> -> <result>"
  (#183); an `on_fail` entry whose press is no longer kept is a line of
  its own. Each entry opens the trace drawer
  on itself. Once there are more, the chevron expands the strip in place
  to the 20 entries the Pad keeps, one per line, and collapses it again.
- **Actions catalogue** (`app/src/Catalogue.svelte`,
  `app/src/lib/catalogue.ts`). From a 1200 px window on, the right
  column: "Search actions" (`/` focuses it outside a field) narrows the
  entries by name, group name or the name of a variant ("Region" finds
  Spectacle), case-insensitive, and says "No action matches" when none is
  left; then the groups General, System and Apps, each collapsible on its
  heading (`aria-expanded`; a search shows the matches of a collapsed
  group). The column opens with General expanded and System and Apps
  collapsed, so it fits a 640 px window (#166). The presets are the recipe packs of the daemon's `recipes`
  reply (§7.5, §8; the shell's `recipes` command, `daemon.recipes()`), in
  the reply's order, read when the Pad connects and again after every
  reload; the raw tools Chord, Exec and Launch (`lucide:app-window`)
  close General. When the packs
  cannot be read the column says why under the entries and keeps the
  tools; each user pack the daemon left out is listed under "Recipe packs
  left out" with its `file:line:column: message`. Every entry is dragged
  onto a tile of the mirror (the drag carries `tool:<id>` or
  `recipe:<id>` as `application/x-keyvo-action`; every cell is a drop
  target) or clicked to assign it to the selected tile. Either way the
  tile is selected (and pinned, as a click would). A tool sets the action
  type with an empty field: nothing is written until the keys or the
  command are entered. A preset is written at once through the editor
  (`edit_key`, so undo takes it back) with its first variant's action and
  the pack's label and icon, the colour kept, as
  `keyvo_client::Editor::apply_recipe` writes it. A key carries a preset
  when a pack has its icon and a variant with its action (the pack with
  its label first, else the first in catalogue order; the profile never
  names the pack): the inspector then shows "What it does" with the
  pack's variants above the Action select, and another variant rewrites
  the action (and the label and icon to the pack's). Below 1200 px the
  column goes and the inspector's Action select carries the entries in
  the three groups; a preset chosen there is applied like an assigned
  one. Beside the select a "Browse actions" button (`aria-haspopup`,
  `aria-expanded`) opens the catalogue as a popover at the window's right
  edge, a dialog that is not modal, with its icons, search and drag; `/`
  outside a field opens it too, with or without a selected key, and puts
  the focus in its search. A click on an entry assigns it to the selected
  key and closes the popover; Escape and Close close it; the focus goes
  back to the button (#167).
- **Trace drawer** (`app/src/TraceDrawer.svelte` on `Drawer`). Opens from
  an entry of the trace strip and from the result line of a dry-run press, takes the
  focus, and closes on `Esc` or its close button, giving the focus back.
  Its header carries the Debug log switch, disabled with "Coming soon"
  until the daemon sends HID-level events. It shows the
  sections of the entry the daemon records today, in resolution order:
  Trigger (time, key, press kind, the device or simulated), Context
  snapshot (profile, page), Resolved action (layer, type, keys or
  command; Branch `on_fail` for an `on_fail` entry opened alone), then
  Result (status; a chord's key events, an `exec`'s exit code or signal
  and duration, a failure's error; for a press whose `on_fail` ran, an
  `on_fail` row "<action> -> <result>", #183) or Block reason (dry
  run, or no layer binds the key). Conditions, variables and the window
  context come with the daemon fields that carry them.
- **Virtual pad.** On the grid, `Space` presses the selected key short and
  `Shift+Space` long, as dry runs on the page the pad shows; the
  one-line result shows under the drawing. The right-click press is gone.
- **Accessibility.** Each key's `aria-label` names the key, its label, its
  action and its layer; the grid has rows and cells, the selected cell is
  `aria-selected`, and the result line and the held caption are `status`
  regions.
- **Screen states** (design §5.2, brief §6), each notice through `Notice`.
  Empty: the trace strip says how to fill it (press a key on the pad, or
  select one in the mirror and press `Space`). Loading: nine skeleton
  tiles in the drawing (`aria-busy`), no spinner; on a reconnect the
  mirror keeps its last frame instead. Device missing: a note names the
  Keypad and the time it was last seen connected, or that it was not seen
  since the app started; the mirror keeps the last geometry. Schema
  error: an error notice, `Schema error in <file>, line <n>, column <n>:
  <message>` when the error names a place (`<file>:<line>[:<column>]:
  <message>`), else the error as it is, until a reload succeeds. Daemon
  unreachable: when `status` fails or the subscription ends, an error
  notice says why (the socket path and the OS error as the shell reports
  them) and "Start it with `keyvo-daemon`", the mirror keeps its last
  frame (no tiles are fetched while the daemon is away, so the closed
  connection does not empty it; #67, #165), or shows the empty wells and
  "No pad" when there never was one, the inspector and the tabs keep their
  empty states, Press is disabled, and Retry connects again. The status strip of the earlier Home,
  its links and its page selector are gone; every fact shows once. The
  in-process simulator of §4.2 of the design and the header badge come
  later.

**As built (pre-alpha inspector).** Beside the drawing, the pinned
inspector (`app/src/Inspector.svelte`, `app/src/lib/editor.ts`) shows the
key selected on the grid on the pinned profile and page. It is the one
key editor of the app; the key grid exists once, on the Pad.

- **Header.** `Key <n>: <kind>`, the kind being the key's action (Chord,
  Exec, or No action), and under it `<file> line <n>`, the profile file
  and the line its key table starts on (§7.1, `KeyView.line`), or
  `<file>, not on this page yet` for a key the page does not define.
  Press runs the key as a dry run on the page the pad shows, the result
  under the drawing (the "pad moved" line says when that is another page
  than the pinned one), and is disabled while the Pad is not connected
  (#165); Clear writes the key with every field absent, on the undo stack
  like any write.
- **Fall-through.** A key the pinned app profile does not define and the
  global profile does (its context page, §6.1) shows the global key's
  kind, file and line with "from global", and "Override here", which
  writes the global key's fields into the pinned profile's key. The form
  edits the pinned profile's own key.
- **Tile row.** The key's tile as the pinned page renders it (the shell's
  `tiles`; fetched for the pinned page while the pad shows another), then
  Label, Icon and Color. Icon is a combobox (`app/src/IconPicker.svelte`)
  over the icons keyvo-core bundles for the renderer
  (`crates/keyvo-core/assets/lucide/`, read at build time by
  `app/src/lib/tileIcons.ts`, the whole lucide set): typing filters on a
  substring of the name, the list shows the first 60 matches and a line
  counts the rest ("60 of 95, keep typing"; an empty search shows the
  first 60 names), the arrows move over the shown rows, Enter or a click
  picks, `Esc` closes, each row with its glyph and name; the pick is
  written as `lucide:<name>`, the value as written shows under the field, an emptied
  field removes the icon, and a value the picker does not bundle stays as
  written. A typed search, its list and its active row stay through a
  refresh of the key (a new revision, a pad move, an acknowledged
  write); only another value (a pick, an undo, another key) or `Esc`
  sets the field again. Color is a colour swatch and its `#rrggbb` text, kept in step.
- **Action.** A select (No action, Chord, Exec, Launch) and the kind's
  field, the chord's keys or the `exec` or `launch` command; the working directory and hold of
  an `exec` wait for the action schema to carry them. A Launch also shows
  `Window class` (optional, with the hint "The class the launched window
  carries; keyvo ctl status prints the focused one.") and the switch
  `Wait for the window`, on by default (§6.3); another action type, a
  tool or a preset starts both over (no class, waiting). Under the Chord
  field a hint line in the label size says the syntax: "Keys joined with
  +, for example ctrl+shift+g; modifiers ctrl, alt, shift, super." (the
  field is described by it). A key with an `on_fail` shows the read-only
  line "Runs `<keys or command>` when this fails. Edit it in the file."
  under the action fields (#183); no field edits it, and a write of the
  other fields leaves it in the file byte for byte. The form is at most
  `48ch` wide.

- **Immediate apply.** A field is written the moment it changes (the
  input's `change`, on Enter or when the focus leaves it), through the
  shell's `edit_key`: a snapshot, then the atomic write of §7.1. There is
  no save button. Writes run one after the other. The daemon's inotify
  watch reloads the file and repaints the pad; the mirror fetches its tiles
  again after the write and after the daemon's `config` event.
- **Validation.** A color that is not `#rrggbb` or an action without its
  keys or command is not written; an alert under the field says why, and
  the field is marked invalid and described by it. A new action type
  starts empty and is written when its keys or command are entered. A
  chord is checked before the write with the grammar of §7.2
  (`app/src/lib/chord.ts`, a port of the token rules of
  `keyvo_core::chord`; its evdev key names, `app/src/lib/keynames.ts`, are
  written by `tools/keycodes/generate.py` in the same run as the Rust
  table, and a test compares the two); the alert names the wrong token
  ("nope is not a key name.", "g is not a modifier; ...", "A + or , at the
  end needs a key after it.", "Expected +, a comma or the end after +.")
  and nothing is written (#167). Both parsers run the shared cases of
  `tests/protocol-fixtures/chord/`. A chord the parser still refuses comes
  back from the shell as the error.
- **Undo.** `Ctrl+Z` and `Ctrl+Shift+Z`, or the Undo and Redo buttons, undo
  and redo the session's writes, newest first, each by writing the fields
  before it again through the same path; a new write drops what is left to
  redo. Inside a form field the keys undo the typing instead. The history
  lives as long as the window.
- **Errors.** A profile file that does not validate (an external edit, for
  example) shows as an alert `file:line:column: message` and the form is
  hidden; the file is never rewritten to fix it. A failed write shows the
  same way, with its place when the shell names one, keeps the typed
  value and leaves the history as it was.
- **Result line.** `Wrote key <n> to <file> at <time>` (undo and redo say
  so the same way), the time 24 h, in the body colour.
- **Reloads.** After the daemon's `config` event the editor reads the key
  again; a field being edited keeps its text. "Open in editor" for the
  whole profile file is on Profiles (below); the file watch of
  `docs/design/gui-ux.md` §4.1 comes with M4a-4.
- **External changes, as built.** The app has no file watcher of its
  own: it refetches the tiles and the selected key when the
  daemon sends its `config` event, which the daemon's inotify watch
  (§7.2) triggers. `docs/design/gui-ux.md` §4.1 stays the target.

**As built (pre-alpha Profiles).** Profiles (`app/src/Profiles.svelte`,
`app/src/lib/profiles.ts`) is the profile manager of
`docs/design/gui-ux.md` §3.2 and the pad screen brief §5.3 (d1, d2, d14,
ADR-0004): a list column and the editor, no key grid, no key editor and
no right column. It reads the profiles of the daemon's configuration
directory through the shell's `profile_list`
(`keyvo_client::profiles::list`), again after every reload and every
write of the screen.

- **List column.** Every profile: the global one pinned first, then by
  `priority`, highest first (the one that wins a match, §6.2), equal
  priorities by id. Each row shows the name, a 12 px accent square after
  it when the profile has an accent, and "on the pad" on the profile the
  pad shows; its accessible name adds the id and the priority. The
  selection is the shell's pin, the one the header's dropdown names
  (#164, which answers question 1 of the critique re-run): the pinned
  profile, else the pad's, else the global one. It follows the pad only
  while nothing is pinned. A pick in the list pins that profile on page
  1, as the dropdown does; a field of the editor taking the focus pins
  the selected profile (on the pad's page when it is the pad's profile),
  so the pad moving on changes nothing under the user (#130). Follow in
  the header, or on the Pad, returns both to the pad. A selected profile
  starts on its pinned page, else on page 1. Skeleton rows
  stand in while the first read runs; a configuration that does not
  load shows as a notice with the load errors, the last list kept. "+"
  opens the templates (Empty profile, Copy of the active profile, Starter
  pack), each a disabled control with its M4a-4 sentence.
- **Heading row.** The profile's name and "Open in editor", which hands
  the file to the desktop's application for it (`xdg-open`, shell
  command `open_profile`; the path comes from the id, never from the
  page); under it the file path, once. For the built-in global profile,
  which has no file until its first edit, the button is
  `aria-disabled` and a note says so. The row is as wide as the header
  form (`48ch`), so the button ends where the content ends; the "Match
  rules" row with "+ rule" likewise. A name without spaces breaks inside
  the heading and the button keeps its one line beside it (#130).
- **Header form.** At most `48ch` wide, one column: each label above its
  field (`docs/DESIGN.md` "Inputs / Fields", as the Inspector). Name, and
  Accent as a swatch and
  its `#rrggbb` text, each written the moment it changes through the
  shell's `edit_profile` (`keyvo_core::edit::ProfileDocument::set_profile`):
  a snapshot, then the atomic write of §7.1, every byte outside the
  edited value kept, an empty accent removed, a new one added at the end
  of `[profile]`. An empty name or an accent that is not `#rrggbb` is not
  written; the field is marked invalid and the message under it says
  why. Priority shows as read with "higher wins the window"; Enabled
  likewise shows as read, "On", with its M4a-4 sentence beside it (the
  schema has no profile-level `enabled`; the editor writes name, accent
  and, beta #156, `global_pages`). The result
  line under the form says what was written or opened. Header writes are
  not on the Pad's undo stack; the snapshot keeps the file before each
  one.
- **Match rules.** One list of kind and pattern rows as written, each
  kind named once (Window class, Title regex); "+ rule" and each row's
  Edit are disabled with their M4a-4 sentence. Under the Window class
  patterns a muted line in the label size says what the class is: "The
  class KWin reports for the window; keyvo ctl status prints the focused
  one." (#167). A profile without rules
  says focus never selects it; the global profile says it is under every
  window.
- **Pages.** One thumbnail per page, the page's tiles as keyvo-core
  renders them (the shell's `tiles` at the device's tile size) in the
  device drawing (`app/src/Device.svelte`), captioned by the page's
  `name`, else `Page <n>`; without a reported geometry the thumbnails
  are the names and a note says the pictures wait for a Keypad. A click
  selects a page, and "Edit keys on the Pad", at the `48ch` end of the
  Pages heading row (#166), opens the Pad pinned to the
  profile and the selected page (`lib/pin.ts`). A newly selected profile
  starts on page 1; a profile with no pages says so.
- **Page list** (beta, #156). Under the thumbnails each page has a row:
  its name, edited inline (an empty name removes it), an On switch, Up
  and Down, and Delete; "+ page" adds an unnamed page at the end. Each
  change goes through the shell's `edit_page`
  (`keyvo_core::edit::ProfileDocument::edit_page`, a `PageEdit` of `op`
  `add`, `rename`, `move`, `enable` or `delete`): a snapshot, then the
  atomic write, every byte outside the edited page kept; a moved page
  takes its comments and keys along, a page switched on loses its
  `enabled` line. Undo takes back the last page change: the answer's
  inverse edit, or after a delete the shell's `restore_profile`, which
  writes the snapshot back (`keyvo_client::editor::Editor::restore`,
  only a snapshot of the editor's own directory and of the same
  profile); a later header write clears it. The context page's switch
  and Delete on the only enabled page are `aria-disabled`, the reason
  beside them; the editor refuses both edits as well
  (`EditError::Protected`). A file that writes `pages` as an inline
  array can be renamed and switched, not added to, reordered or deleted
  from (the editor's `Shape` error). An app profile's header form has a
  "Show the global pages" checkbox, written as `global_pages = false`
  in `[profile]` and removed when checked; the global profile does not
  offer it.

**As built (mock dev server).** `just app-dev` (a host recipe) serves the
app on the Vite dev server at `http://localhost:1420` with
`VITE_KEYVO_MOCK=1`, for a host browser: `app/src/main.ts` then mounts
the app with the in-memory clients of `app/src/lib/mock/` instead of the
Tauri shell's. The mock reads its data in place: the connected `status`
reply and the `trace` events of `tests/protocol-fixtures/socket/`, the
three profiles of `contrib/profiles/` (edits stay in memory until a
reload), and the PNG snapshots of `crates/keyvo-core/tests/snapshots/` as
tiles. Its focus cycles Konsole, VS Code and a window no profile matches
every ten seconds, and its subscription sends a fixture trace entry every
two seconds. `?theme=` picks one of the four measured token sets of
`docs/design/gui-ux.md` §7, `?mock-error=unreachable` makes `status`
fail, `?mock-error=write` makes every `edit_key` fail with the key's file,
line and column, `?mock-locked` reports the session locked: the view says
`locked`, the tiles of the pad's page are the lock key
(`render__lock-key.snap.png`) and bare keys, and every press is
`blocked`. A build without the variable contains no mock module.

---

## 11. Windows plugin (`plugins/logi/`, M5)

- Logi Actions SDK plugin written in **C#**: only the C# SDK can render
  dynamic per-key images; the Node SDK cannot (ADR-0007). Built and tested
  in the libvirt Windows VM (`tools/windows-vm/`), CI job on Windows from
  day one.
- The plugin is a thin bridge: it exposes keyvo actions (exec/http/overlay,
  Claude Code status) as Commands and the Dialpad as Adjustments, and
  connects to the local daemon over the named pipe. Options+ owns
  app-detection and profiles on Windows in plugin mode.
- Ships a default `.lp5` layout. Published on the Logi Marketplace under the
  Qubernetic name with the trademark notice from `NOTICE`.
- Native mode (§4.2) is a later M5 item and depends on the ownership
  measurement (plan §5 item 16).

---

## 12. Non-functional requirements and dependencies

- Latency key-press → injected input < 30 ms; profile switch → repaint
  < 150 ms (targets; measured in M1).
- Daemon must never block on user scripts, plugins or network.
- Survives device unplug/replug, suspend/resume, KWin restart, Solaar
  restart.
- No writes outside XDG dirs and `/etc/udev/rules.d/`; rules and units are
  installable from git and reversible.
- Configuration is plain TOML; the GUI edits the same files.
- Logging with levels (`tracing`); `keyvo doctor` diagnoses hidraw
  permissions, udev rule, `input` group / ACLs, `/dev/uinput`, KWin script
  state, socket ownership, Solaar presence, Options+ ownership (Windows).

  As built (beta, #184): `keyvo doctor [--json]` (§5.10) runs nine
  checks, one line each: `ok`, `warn` or `fail`, the check's name, one
  sentence, and the fix commands on `fix:` lines below it; `--json`
  prints one object per check and line (`check`, `status`, `message`,
  `fix`: the lines joined by `\n`, or `null`). It exits 1 when a check
  fails and 0 otherwise. Each check is a predicate over a root directory
  (sysfs, `/dev`, `/etc`, `/proc`), the environment and a command runner,
  so the tests fake all three.

  | # | Check | `ok` | otherwise |
  |---|---|---|---|
  | 1 | `keypad` | the Keypad's node (§5.1) opens read-write | `fail`: not found (plug it in), or not accessible (the rule's install lines) |
  | 2 | `uinput` | `/dev/uinput` opens read-write | `fail`: missing (`sudo modprobe uinput`) or not accessible (the rule) |
  | 3 | `rule` | `/etc/udev/rules.d/70-keyvo.rules` is byte for byte the shipped rule | missing or different: `warn` while 1 and 2 pass (the access comes from another setup), else `fail`; the install lines |
  | 4 | `session` | `XDG_SESSION_TYPE` is `wayland` and `XDG_CURRENT_DESKTOP` contains `KDE` | `warn`: profiles follow the focused window on KWin only |
  | 5 | `focus` | `kwinrc` `[Windows]` `FocusStealingPreventionLevel` absent, `0` or `1` | `warn` at `2` or above: launch keys do not switch the pad (§6.3, #179); the `kwriteconfig6` line |
  | 6 | `unit` | `systemctl --user is-active keyvo-daemon` says `active` | `warn`: not installed (the three install lines), or installed and not active (`enable --now`) |
  | 7 | `daemon` | the socket is the user's with mode `0600`; when it lies in keyvo's runtime directory, that directory is the user's and nobody else may enter it (the daemon's predicate, `mode & 0o077 == 0`); `status` answers within `[timing].doctor_reply` | `warn`: no socket or nobody answers (start it), no answer in time (the journal); `fail`: a wrong mode or owner, or the client's checks (§8) refuse it. The fix removes only keyvo's runtime directory; for a `KEYVO_SOCKET` elsewhere it restarts the unit or unsets the variable |
  | 8 | `solaar` | Solaar is not running (`flatpak ps`, a `solaar` process) | `warn`: it must set no divert on the Creative Console devices |
  | 9 | `flatpak` | always `ok`: says whether `flatpak info io.github.qubernetic.keyvo` finds the app | |

  It writes nothing, opens the two device nodes without writing to them,
  and runs only `systemctl --user` and `flatpak` with fixed arguments;
  the fix lines are printed, never run. A `config.toml` that does not load
  leaves `[timing]` at its defaults. `just test-hw` runs `bin/keyvo
  doctor`. The socket's `doctor` command (§8) stays for later.
- Toolchain: pinned stable Rust (`rust-toolchain.toml`, edition 2024),
  clippy `-D warnings`, `cargo-deny`, `cargo-llvm-cov`, CodeQL, `cargo-dist`,
  mdBook, dependabot.
- Crates (v1): `tokio` (daemon only; `keyvo-core` is sync), `zbus` (6.x;
  5.x pulls the `zvariant` and `zbus_names` crates that RUSTSEC-2026-0370
  and its siblings mark unmaintained after their merge into `zbus` 6),
  `hidapi` (non-Linux) with a direct hidraw path on Linux, `evdev`
  (uinput), `x11rb`, `toml_edit`, `serde`, `schemars`, `tiny-skia`,
  `resvg`, `image` (JPEG and PNG encoding), `insta` (tests), `clap`,
  `tracing`. Workspace crates: `keyvo-hid`, `keyvo-core`, `keyvo-daemon`,
  `keyvo-client` (socket client, mirror, tiles; sync and hardware-free like
  `keyvo-core`) and `keyvo-cli`.
- Desktop app: `tauri` 2 and `tauri-build` (in `app/src-tauri`, outside
  the root workspace, so `cargo deny` of the workspace does not cover it;
  `app/src-tauri/deny.toml` checks its lock with the root policy plus two
  documented exceptions; Tauri 2 on Linux pulls the gtk-rs 0.18 crates),
  `base64`; the frontend
  `svelte` 5, `vite`, `vitest`, `jsdom`, `@testing-library/svelte`,
  `svelte-check` and `@tauri-apps/api`. Build packages in the dev image:
  `libwebkit2gtk-4.1-dev`, `build-essential`, `file`, `libxdo-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`.

---

## 13. Prior art and dependencies

keyvo copies no code from these projects; it re-derives the protocol from
their findings and its own captures and checks its packet layouts against
them.

| Project | Contribution and use in keyvo |
|---|---|
| [node-logitech-mx-creative-console](https://github.com/Julusian/node-logitech-mx-creative-console) (Julian Waller / Bitfocus, MIT) | Earliest public implementation (Oct 2024) of the Keypad's LCD frame format, page-button divert writes and brightness command, in TypeScript for Node and WebHID; keyvo's packet layout and input model are checked against it. PR #22 adds Dialpad and Windows collection handling. |
| [mx-creative-controller](https://github.com/hcooper-idealbuilders/mx-creative-controller) (Hunter Cooper, MIT) | Windows controller plus the only public USBPcap captures and journal of the reverse-engineering; its captured Options+ frames are keyvo's golden data (`captures/external/hcooper/`). |
| [mx-creative-console](https://github.com/shensquared/mx-creative-console) (Shen Shen, MIT) | macOS C probes that verified the HID++ feature table, the `0x0008` keep-alive requirement, `0x1B04` diverts, `0x8040` brightness and the Dialpad's `0x4610` MultiRoller over Bluetooth; keyvo's keep-alive and Dialpad design follow its findings. |
| [logilinux](https://github.com/logilinux/logilinux) (Arjun Juneja, Ron0Studios et al., LauzHack 2025, MIT) | C++ library and CLI tools for the Keypad and Dialpad on Linux: hidraw/evdev discovery, JPEG upload over report `0x14`, button-report parsing, Dialpad rotation via evdev; keyvo re-derives the Linux plumbing and input parsing from it. Its `ref/hidpp` (GPL-3.0) is never used. |
| [logilinux-sdk](https://github.com/logilinux/logilinux-sdk) (ron0studios, MIT) | Python/pybind11 SDK over logilinux with a plugin model modelled on Logitech's C# SDK; consulted for API shape only. |
| [logimap](https://github.com/abishekmuthian/logimap) (Abishek Muthian, MIT) | Python per-app profile daemon for the Keypad on Plasma 6 Wayland; keyvo re-implements its KWin-script→D-Bus focus feed, uinput chord-injection design, reconnect behaviour and onboarding/troubleshooting ideas. |
| [mx-creative-console-webhid](https://github.com/mario-gutierrez/mx-creative-console-webhid) (Mario Gutierrez, no license file) | Browser WebHID client with runtime HID++ feature discovery (`0x1B04`, `0x4610`, `0x8040`, `0x0007`); consulted for HID++ mechanics, not copied. |
| [creative-console-daemon](https://github.com/notno/creative-console-daemon) (Nathan Rosquist, no license file) | Rust/hidapi daemon for Windows whose packetizer unit tests provide independent expected values; reference for Windows native mode. |
| [companion-surface-logitech-mx-creative-console](https://github.com/bitfocus/companion-surface-logitech-mx-creative-console) (Bitfocus, MIT) | Bitfocus Companion surface module on top of Julusian's library; reference for draw throttling, retry and reset-on-close behaviour. |
| [mx-console-suite](https://github.com/CatalystMonish/mx-console-suite) (macOS) | Keep-alive-by-repaint observation, launcher pattern, "both page buttons = escape" chord. |
| [claude-console](https://github.com/rshankras/claude-console) (Actions SDK, macOS) | Reference for Claude Code plugin actions and opt-in live status keys that edit user config only after confirmation. |
| [Solaar](https://github.com/pwr-Solaar/Solaar) (GPL-2.0) | HID++ engine for mice and keyboards and the source of the author's feature dumps; keyvo **calls** it for desk state and never copies its code. |
| Logitech HID++ 2.0 public feature documents; Logi Actions SDK docs | Feature semantics (`0x0008`, `0x1B04`, `0x8040`, `0x4610`); Windows/macOS plugin API. |
| Logi Options+ 2.8 and Logi Plugin Service 6.4 (public offline installer, analysed under ADR-0008) | Static inventory of facts: `0x19A1` ContextualDisplay function table and field names, device state names, image record layout, VLP root functions, names of the unknown features, and the finding that the Plugin Service helper, not the Options+ agent, drives the Creative Console (`docs/research/options-plus-inventory.md`). |

---

## 14. Licensing and legal

- License: **Apache-2.0** (patent grant; MIT-compatible; commercial-friendly).
  Copyright Qubernetic; `NOTICE` carries the Logitech trademark statement.
- Contributions under DCO (`Signed-off-by`). No AI co-author lines in
  commits.
- No code copied from GPL projects (Solaar, logilinux `ref/hidpp`). The
  HID++ subset is written from Logitech's public HID++ documents, prior-art
  findings and own captures; third-party captures are used under their MIT
  licenses and credited in `captures/`.
- Static analysis of Logitech software (Options+, Logi Plugin Service) only for
  interoperability under Directive 2009/24/EC Art. 5(3) and Art. 6, within
  ADR-0008: device-communication parts only, protocol facts only, never
  decompiled code, no circumvention of protection measures on data. Findings in
  `docs/research/options-plus-inventory.md`.
- Trademark notice for Logitech names; descriptive use only.
- Employer/side-project clearance obtained by the author; co-owner of the
  Qubernetic brand informed and agreed.

---

## 15. Milestones

Detailed issue lists, definitions of done and the measurement list are in
`docs/plans/2026-09-09-project-plan.md` §4–§5 and `docs/roadmap.md`.

0. **Bootstrap**: repository, docs foundation, workspace skeleton,
   devcontainer, CI, diagrams.

The pre-alpha epic (`docs/roadmap.md`, Pre-alpha) cuts thin slices through
M0 to M4a first, and **M4b: Flathub** follows it directly: the owner's
atomic host has no webkit packages, so the app runs only inside the
`org.gnome.Platform//51` runtime until the Flatpak exists, in the sandbox
of an installed application on that runtime (the bare runtime has no
application id, and GTK's glycin icon loader cannot spawn its sandbox
without one; `docs/pre-alpha-test.md` section 2). The list below
is the order of the remaining milestone work.

The beta epic (`docs/roadmap.md`, Beta; #180) takes M4b and parts of M1
and M3; its hardware checks are in `docs/beta-test.md`.

1. **M0: Protocol (hardware truth)**. TS quick-test with Julusian's
   library, Windows VM + USB capture toolkit, HID++ 2.0 core, keypad
   enumeration + keep-alive, VLP display plane, key and page-button input,
   brightness, `keyvo probe|keys|fill|paint-test|doctor`,
   `docs/protocol.md`, `docs/hardware-notes.md`; Windows build green.
2. **M1: Static pad (v0.1.0)**. TOML profiles with schema and round-trip,
   renderer + snapshots, `preview`, actions, `on_fail`, automatic states and
   feedback primitives, recipes and detectors, press kinds, hot-reload, trace
   core, `keyvo setup`, `profile export|import`, systemd unit, udev rule +
   `doctor`, cargo-dist release.
3. **M2: Context**. KWin script + X11 watcher, auto/manual state machine,
   page buttons, overlays + `general_hint`, socket API v1 + `subscribe`,
   `keyvo ctl`, project layer and trust, context source (shell hooks, pid
   binding, registry fallback), plugin supervisor with two reference plugins,
   Claude Code plugin, tmux and git integrations, `.lp5` import (if not M1).
4. **M3: Desk state and Dialpad**. First a static-analysis pass under
   ADR-0008 over the Dialpad features (`0x4610` getCapabilities to
   RotationEvent), then Dialpad over Bolt, Bluetooth
   verification, MX Master 4 and MX Mechanical controls as trigger sources,
   Solaar-backed settings per profile (open), VS Code extension.
5. **M4a: GUI** (shell, mirror, simulator, wizard, mini-wizard); the
   screens land in the M4b Flatpak as they ship. Sandbox-complete issues of M4a may start once the M2 socket
   exists (`docs/roadmap.md`).
6. **M5: Windows**. C# Actions SDK plugin, Marketplace listing; native
   mode with ownership detection.

---

## 16. Open questions

Status of the v0.1 questions after the grill and the research.

| # | Question | Status |
|---|---|---|
| 1 | Rust port vs TS core + Tauri sidecar | resolved → Rust workspace core; a TS quick-test with Julusian's library runs first for hardware ground truth (ADR-0001, M0-1) |
| 2 | Keep-alive period; do stale keys revert to logo | resolved → HID++ `0x0008`, Options+ 3000 ms / ~1 s ping; revert timing → M0 measurement list items 2–4 (plan §5) |
| 3 | Native animated format; max fps | resolved → frame push only; fps → item 9 |
| 4 | Overlays on context page only, or all pages | resolved → context page only, plus optional `general_hint` |
| 5 | Return to context page on app switch from general page | resolved → yes, unless a page button was used within 10 s; since #151 for any page list index, which an app switch after 10 s sets back to 1 (§6.3) |
| 6 | Focus hysteresis values | resolved → 150 ms hysteresis, 50 ms render coalescing, in `[timing]` |
| 7 | Embedded Lua | resolved → not in v1 (ADR-0003) |
| 8 | GNOME strategy | resolved → KDE Wayland + X11 in v1; GNOME/Hyprland/Sway via the `notify_context` contract, community backends |
| 9 | Daemon inside Flatpak vs host daemon | resolved → host daemon (systemd user unit) is primary; Flatpak in M4b ships GUI + CLI |
| 10 | Node vs C# for the Logi plugin | resolved → C# (only C# renders dynamic key images, ADR-0007) |
| 11 | Windows exclusive-open with Options+ running | → item 16 (M0-2) |
| 12 | Dialpad over Bluetooth vs Bolt | → items 15 and 18; Bolt path primary, Bluetooth fallback (ADR-0006) |
| 13 | MX Master 4 haptics from profiles | resolved → no haptics in v1 |
| 14 | Name collisions | resolved → `keyvo` (free on crates.io / npm / PyPI / Flathub, ADR-0005) |
| 15 | `.lp5` import placement | **[OPEN]** M1 or M2 (plan §8) |
| 16 | Splash on profile switch default | resolved → off, `[render].splash = false` (owner decision, §9) |
| 17 | MX Keypad (2026) PID / firmware compatibility | **[OPEN]** until an owner publishes `lsusb` / `solaar show` |
| 18 | Is the VLP root fn2 `5e..65` registration required | → item 5 |
| 19 | Brightness unit and the meaning of value 0 | → item 11 (M0-7) |
| 20 | Desk state per profile without Solaar (M3) | **[OPEN]** decided before the M3 desk-state issue; facts in `docs/research/options-plus-inventory.md` |
| 21 | Project layer on Windows (shell hook, pid binding) | deferred past M5 v1 (ADR-0009, §10) |

---

## Changelog

- **v0.3, beta slice 10 (2026-10-30)**: page switches, page management
  and per-profile global pages (#156). §6.1 the page list without
  disabled pages and without the global pages of a profile with
  `global_pages = false`, the context page the first enabled one without
  a `context` page; §7.2 `enabled` on a page and `global_pages` in
  `[profile]`; §10 the muted tabs, the switch, and the page list of the
  Profiles screen (add, rename, move, switch, delete with an undo).
- **v0.3, beta slice 9 (2026-10-30)**: a key as the dial modifier, held
  or toggled (#189). §6.1 the modifier rules (held, toggle, the end
  rules, the resolution order, the page return) and the
  `modifier_timeout` row; §6.3 a page button that changes the page ends a
  toggle, a modifier holds the page return; §7.2 a key's `modifier`,
  `dial` and `roller` as built and the press kinds of a modifier key; §8
  the view's `modifier`, the `modifier` event class, the trace fields, the
  `modifier` result, `press` with `hold` and `release`, `KEYVO_KEY` with
  `KEYVO_CONTROL`; §9 the `modifier` look; §10 the Inspector's Modifier
  section, the mirror's ring and the panel's labels.
- **v0.3, beta slice 8 (2026-10-30)**: the Dialpad's bindings and panel
  (#188). §4.1 the dialpad bindings as built; §5.8 the daemon's Dialpad
  actor and `--dialpad-device`; §6.1 a turn is no pad activity for the
  page return, a button press is; §7.2 `[dialpad]` as built (`cw`, `ccw`,
  `step`, `label`, the four buttons, the fallback to the global table)
  and `[dialpad].max_per_event` in `config.toml`; §8 the `dialpad` block
  and event, the `dial` event class, buttons as `key` events, `press` of a
  control, trace records with `control`, `KEYVO_CONTROL`; §10 the Dialpad
  panel and its control editor.

- **v0.3, beta follow-up of slice 4 (2026-10-30)**: the Bolt receiver's
  rule narrowed to the HID++ node (#197, from the security re-review of
  #196 and record D8). §3 the rule matches USB interface `02` as well,
  what that node carries and that the keyboard and mouse nodes stay
  closed.
- **v0.3, beta docs of slice 7 (2026-10-30)**: the Dialpad over Bolt
  measured (#202, from the hardware check of #187, parts 1 and 2). §5.1
  the receiver's four interface nodes without DJ mode, `0x09` as an empty
  slot, the `0x1D4B` reconnect instead of the `0x41` arrival; §5.8
  "measured" marks on the slot, the feature table, the rollers, the
  diverts and the undiverted input, and corrections: held buttons do not
  repeat, no `0x41` on a wake, `0x09` is not retryable.
- **v0.3, beta fix of slice 7 (2026-10-29)**: a sleeping Dialpad over
  Bolt in the slot search (#200, from the hardware check of #187). §5.1
  the cold reply times, `[timing].slot_probe` from 300 ms to 1000 ms and a
  software id per search request, so the receiver's repeated late reply
  answers nothing; §5.8 the cold-run record; §6.1 the `slot_probe` row.
- **v0.3, beta slice 6 (2026-10-28)**: the Dialpad driver over Bolt and
  `keyvo dial` (#186). §5.1 the receiver's node (`046d:c548` with `0x10`
  and `0x11`), the slot search by `getFeature(0x4610)` and the model id,
  the receiver's HID++ 1.0 error report as the pending request's error;
  §5.8 as built: the diverts of every divertable `0x1B04` control and of
  every roller, the rotation and button input, the restore on stop, the
  re-divert on the arrival, the Bluetooth node on `0xff`; §5.10 the
  receiver block of `keyvo probe` and `keyvo dial`; §6.1
  `[timing].slot_probe` (300 ms).
- **v0.3, beta slice 5 (2026-10-28)**: the Dialpad static pass (#185,
  ADR-0008). §5.8 the Bolt slot discovery, the `0x4610` request, response
  and event layouts, the one-bit reporting mode, the settings fields, the
  `0x1B04` divert flags, re-divert on connect and the retryable errors;
  the roller facts of shensquared and Julusian kept as consistent, their
  physical mapping, sign and acceleration left to M3-2.
- **v0.3, beta slice 4 (2026-10-28)**: the systemd user unit, the udev
  rule and `keyvo doctor` (#184). §3 the rule (`uaccess` on the Keypad's
  and the receiver's hidraw nodes and `/dev/uinput`) and the unit as
  built; §5.10 `keyvo doctor`; §6.1 `[timing].doctor_reply` (1000 ms);
  §8 the unit's keys and `KillMode=process` as built, the socket `doctor`
  command marked not built; §12 the nine checks; the layout trees.
- **v0.3, beta slice 3 (2026-10-28)**: a key's `on_fail` runs when its
  action fails (#183). §7.2 `on_fail` as built: one per key, any action,
  a nested one a load error at its position, run once on a non-zero exit
  or a failure, never on a block, a dry run, an unbound key, a started
  launch or its own failure, the key keeping its `error` look; §8 the
  run through the `exec` and `launch` paths with `KEYVO_FAILED_CODE`, the
  #155 paragraph's open `on_fail` line closed, and the trace record's
  `branch`; §10 the strip's one line per press, the drawer's `on_fail`
  row and the Inspector's read-only line.
- **v0.3, beta slice 2 (2026-10-28)**: a launch keeps the global page
  until its window arrives (#179). §6.1 `[timing].launch_focus` (15 s);
  §6.3 pending launches, judged by lineage, class, then the oldest launch
  without a class, a hit keeping an index of 0 or below and starting the
  page return over at the window's arrival, and the as-built paragraph
  (the `Lineage` platform trait, `/proc` in the daemon); §7.2 the
  `launch` fields `class` and `wait_focus`; §8 the pending launch at the
  spawn and the dependency on KWin's focus stealing prevention; §10 the
  Inspector's `Window class` field and `Wait for the window` switch. The
  research report `docs/research/2026-10-27-launch-focus-matching-prior-art.md`
  is committed.
- **v0.3, beta slice 1 (2026-10-28)**: keyvo's own Flatpak (#182). §3 the
  Flatpak as built: `io.github.qubernetic.keyvo` with the app and the CLI
  on `org.gnome.Platform//51`, the daemon on the host, built offline from
  generated cargo and npm sources; §6.1 `[focus].hold_classes` defaults to
  `["keyvo-app", "io.github.qubernetic.keyvo"]`, the app's window class on
  the host and under Flatpak; §10 the Flatpak line is the six
  `finish-args` as built (no `--device=all`, no `org.kde.KWin`), the Tauri
  identifier is the application id. The three binaries report 0.1.0.
- **v0.3, pre-alpha design pass (2026-10-27)**: chords checked before
  the write and the fields' hints (#167). §10 the key editor checks a
  chord with the §7.2 grammar ported to the app, its key names generated
  with the Rust table, and names the wrong token under the field; hint
  lines under Chord and under the Window class rule; below 1200 px
  "Browse actions" (or `/`) opens the catalogue as a popover (critique
  re-run, heuristics 5, 6 and 10).
- **v0.3, pre-alpha design pass (2026-10-27)**: one scroll owner per
  screen (#166). §10 `main` no longer scrolls; the Pad's drawing,
  inspector and catalogue and the Profiles list and editor each scroll on
  their own (critique re-run, priority issue 5); the catalogue opens with
  only General expanded; "Edit keys on the Pad" sits in the Pages
  heading row.
- **v0.3, pre-alpha design pass (2026-10-27)**: the Pad without a pad
  (#165). §10 without a geometry the drawing is the device with nine empty
  wells and the caption "No pad" instead of an empty column (critique
  re-run, priority issue 4); the mirror keeps its last frame when the
  daemon closes the connection, as no tiles are fetched while it is
  away; Press is disabled while the Pad is not connected.
- **v0.3, pre-alpha design pass (2026-10-27)**: the pin state (#164).
  §10 the header's profile dropdown names its state (pinned or following
  the pad) and is capped at `24ch`; the pin glyph beside it is pressed
  while pinned, Follow when pressed then, and pins the pad's place
  otherwise; the Pad's moved line leads with Follow and the pinned page's
  tab carries the pin mark while the pad is away; the Profiles selection
  is the shell's pin and follows the pad only while nothing is pinned, a
  pick or a field taking the focus pins it (critique re-run, question 1).
- **v0.3, pre-alpha design pass (2026-10-27)**: the session lock (#158).
  §4.1 a fourth platform trait, `SessionWatcher`, with
  `FakeSessionWatcher`; §6.1 a locked session veils the view (`locked`),
  paints the lock page, blocks every press and ignores the page buttons,
  while focus reports apply; §6.2 systemd-logind as the lock source
  (`GetSessionByPID`, else the `auto` session; `Lock`, `Unlock`,
  `LockedHint`); §8 the trace result `blocked` with a `reason`, the view's
  `locked`, the event class `session`; §9 the lock page; §10 the mirror
  shows the lock and the Pad's line says so, the mock's `?mock-locked`.
- **v0.3, pre-alpha design pass (2026-10-27)**: §6.1 and §6.3 the page
  return replaces `page_grace`: `[timing].page_return_global` (10 s)
  brings a global page back to the context page after a time without a
  page-button or key press, `[timing].page_return_own` (0, off) the shown
  profile's further pages; an app switch shows index 1 at once; no return
  under the lock; §7.2 both values apply from the next reload; §10 the
  Pad's tabs no longer name a grace (#157).
- **v0.3, pre-alpha design pass (2026-10-27)**: §7.2 a new action type
  `launch` opens an application and lets it go; §6.1 `[timing].launch_settle`
  (1000 ms); §8 the as-built paragraph and the trace result `started` with
  the `pid`; §10 the Inspector's kinds and the catalogue's raw tools gain
  Launch; the recipe packs for the calculator, the settings pages and the
  Spectacle recording are `launch` (#155).
- **v0.3, pre-alpha design pass (2026-10-27)**: §6.1 and §6.3 one page
  list per profile: the global pages after `general` at -1, -2, ...,
  `general` at 0, the shown profile's context page at 1, its other pages
  from 2; both buttons walk it in both modes without wrapping;
  `page_grace` keeps any index across an app switch; manual mode is a
  focus lock. The view gains `index`; §8 the socket state and the
  `profile`, `page` and `mode` events carry it; §10 the Pad's tabs show
  the list, the global tabs muted (#151).
- **v0.3, pre-alpha design pass (2026-10-27)**: §10 Pad: the "pad moved"
  line names the profile the pad shows by its name, as the header does,
  not by its id (#150).
- **v0.3, pre-alpha design pass (2026-10-27)**: §10 the Icon picker keeps
  a typed search through a refresh of the key and starts afresh on
  another key (#149).
- **v0.3, pre-alpha design pass (2026-10-27)**: §9 the whole lucide 1.61.0
  set is bundled, generated with its metadata aliases by
  `tools/lucide/generate.py`; §10 the Icon picker shows the first 60
  matches and counts the rest (#142).
- **v0.3, pre-alpha design pass (2026-10-27)**: §10 Pad: while the pad
  shows another page than the pinned one, the mirror draws no selection
  outline, the "pad moved" line names the pinned profile and key before
  the pad's place, and a click on a tile re-pins a key's pin to the
  pad's page (#143).
- **v0.3, pre-alpha design pass (2026-10-27)**: §10 Profiles: the header
  form is one column with each label above its field; Enabled shows as
  read ("On" with its M4a-4 sentence), no longer a disabled switch; the
  heading row and the "Match rules" row are capped at the form's `48ch`
  (#139).
- **v0.3, pre-alpha slice 7 (2026-10-26)**: §10 Profiles as the profile
  manager: the list with the 12 px accent square and "on the pad", "+"
  with the templates as Soon controls; the heading row with the file
  once and "Open in editor"; the header form at `48ch` with the accent
  swatch, Priority's hint and Enabled as a Soon switch, an invalid field's
  message under it; the match rules with each kind named once; the pages
  as thumbnails; "Edit keys on the Pad" pins the Pad to the profile and
  page. The key grid, the key editor (`KeyEditor.svelte`), the right
  column and `Columns.svelte` are gone; every scrolling column reserves
  its gutter (#109).
- **v0.3, pre-alpha Pad slice 6 (2026-10-26)**: §10 the actions
  catalogue's presets are the recipe packs of the `recipes` reply (the
  shell's `recipes` command); a dropped or clicked preset writes its
  default variant, label and icon through the editor; the inspector's
  "What it does" select for a key that carries a preset rewrites the
  action; the search covers variant names; the packs the daemon left out
  are listed under the entries (#129).
- **v0.3, pre-alpha Pad slice 4 (2026-10-26)**: §10 the actions
  catalogue as built (`Catalogue.svelte`, `lib/catalogue.ts`): the right
  column from 1200 px with "Search actions" (`/`), the groups General,
  System and Apps; Chord and Exec dragged onto a tile of the mirror or
  clicked for the selected tile set the action type with an empty field,
  the presets "Coming soon" until #119's packs reach the app; below
  1200 px the entries sit in the inspector's Action select. The event
  list becomes the trace strip under the inspector (`TraceStrip.svelte`):
  the newest entries on one line with 24 h times, the chevron expanding it
  to the 20 the Pad keeps (#128).
- **v0.3, pre-alpha Pad slice 3 (2026-10-26)**: §10 the pinned inspector
  as built (`Inspector.svelte`) in the key editor's place on the Pad:
  header `Key <n>: <kind>` with the file and line, Press and Clear; "from
  global" and "Override here" for a key that falls through; the tile row
  with the tile, Label, the icon picker over keyvo-core's bundled lucide
  icons (`IconPicker.svelte`, `lib/tileIcons.ts`) and the colour swatch
  with its text; the Action and its field; the form at `48ch`; a
  message under an invalid field; the result line with a time (#127).
  The key editor stays on Profiles until #109.
- **v0.3, pre-alpha Pad slice 2 (2026-10-26)**: §10 the Pad as built:
  Home becomes the Pad (`Pad.svelte`, `lib/pad.ts`); the mirror in the
  device drawing selects on a click and presses only on `Space` and
  `Shift+Space`, dims a global fall-through key with a corner mark and
  outlines the selected key in the accent; selecting a key pins the
  inspector to the pad's profile and page, and a "pad moved" line with
  Follow appears while they differ; page tabs of the pinned profile with
  "+" as a Soon control (M4a-4) and the accent swatch; the held caption
  `<class> focused, holding <profile>`; the status strip, its links, the
  page selector, the accent bar and the right-click press are gone, every
  notice goes through `Notice` (#126).
- **v0.3, pre-alpha Pad slice 1 (2026-10-26)**: §10 the shell of three
  screens: the rail is Pad (screen id `home`), Profiles, Behaviors on
  `Ctrl+1` to `Ctrl+3`; the header gains the Profile dropdown that pins
  the inspector's profile (`lib/pin.ts`), the Sandbox switch as a Soon
  control (M4a-4) and the gear; Preferences (General, Devices,
  Integrations, Projects) and the keyboard map open as non-modal drawers
  on one `Drawer` frame with the trace entry, one at a time; the Debug
  log switch moves into the trace drawer's header; every notice goes
  through `Notice` with a 1 px full border instead of a side stripe
  (#125).
- **v0.3, pre-alpha slice 5 of the GUI epic (2026-10-26)**: §7.5 recipe
  packs as built: `contrib/recipes/` v1 (General, System, Apps), the pack
  schema `schemas/recipe.schema.json`, the user's `recipes/` with a size
  and count cap, a user pack replacing the bundled one of its id, applying
  a variant to a key as plain fields through the editor; §8 the `recipes`
  socket request (#119).
- **v0.3, pre-alpha GUI design pass follow-up (2026-10-26)**: §10 the
  shell's `key` and `edit_key` answer the line of the key's table in its
  profile file (`ProfileDocument::key_line`), for the Pad inspector's
  "file line N" subtitle (#122).
- **v0.3, pre-alpha GUI design pass follow-up (2026-10-26)**: §6.1
  `[focus].hold_classes` (default `["keyvo-app"]`): a focused window of
  such a class keeps the context and profile of the window before it;
  §6.2 a held report cancels a pending one; §7.2 read at start; §8 the
  `context` event and `status` field carry `held: true`; §10 the Context
  fact of Home names the window held over (#117).
- **v0.3, pre-alpha slice 17a (2026-10-26)**: §5.4 the plug-in splash
  (`a4` after `setMode(a0)` for a few seconds after a plug-in, #52) and
  the claim's wait for it; §6.1 `claim_splash_retry` (500 ms) and
  `claim_splash_timeout` (10000 ms), the retry below the timeout (#111).
- **v0.3, pre-alpha GUI design pass slice 2 (2026-10-26)**: §10 the mock
  dev server as built: `just app-dev` serves the app against an in-memory
  mock of the daemon fed by the socket fixtures, the example profiles and
  the renderer snapshots, with four token sets measured from Breeze and
  libadwaita (`docs/design/gui-ux.md` §7) (#105).
- **v0.3, pre-alpha GUI design pass slice 1 (2026-10-26)**: §8 the
  client's peer check reads the raw `ucred` and decides on the uid alone,
  so a client in a Flatpak sandbox, which sees the daemon as pid 0,
  reaches the host daemon; a client whose user namespace does not map
  the user (it runs as the overflow uid) refuses the socket (#104).
- **v0.3, pre-alpha slice G3 (2026-10-26)**: §10 Profiles as built: list
  column by priority with the global profile pinned and the active one
  marked, profile header with name and accent written round-trip (shell
  commands `profile` and `edit_profile`), "Open in editor" for the profile
  file (`open_profile`), read-only match rules, pages as tabs, key grid
  with the key editor, mirror of the selected page and the key card with
  the layer origin; new profile, match rule editing, moving keys and
  Behaviors navigation disabled with "Coming soon".
- **v0.3, pre-alpha slice G2 (2026-10-26)**: §10 Home as built: page
  selector and accent bar above the mirror (shell command `outline`),
  status strip with overlay and project context whose facts link to their
  screens, schema error with file and line, event lines and the dry-run
  result line opening the trace drawer, disabled Debug log switch, and the
  empty, loading and device-missing states.
- **v0.3, pre-alpha slice G1 (2026-10-26)**: §10 the app shell as built:
  rail with the five unbuilt screens disabled and announced, header with
  the device selector and a disabled overflow menu, columns with their
  collapse order, 1200 x 760 window with a 960 x 640 minimum, a text and
  background token pair on every form control with `color-scheme` set
  (fixes the unreadable option list of a select), every string through
  one i18n module.
- **v0.3, pre-alpha slice R2 (2026-10-26)**: §6.1 the global layer as
  built: a key an app profile leaves out shows and runs the global
  profile's context-page key (`keyvo_core::stack`), and the trace entry
  names its `layer`; §7.2 the per-key overlay rule; §8 `KEYVO_LAYER` is the
  supplying layer, `KEYVO_SOCKET` reaches `exec` children, and the daemon
  resolves its socket as flag, `$KEYVO_SOCKET`, default; `status` carries
  `config_dir` and the device `geometry` (`docs/socket-api.md`); §10 the
  app lays the mirror out from that geometry, marks global keys and edits
  the daemon's configuration directory.
- **v0.3, pre-alpha slice 6m (2026-10-25)**: §15 M4b (Flathub) moves to
  right after the pre-alpha, M3 starts with a static-analysis pass over the
  Dialpad features (#92). Matches `docs/roadmap.md`.

- **v0.3, pre-alpha slice 6l (2026-10-25)**: one composite image per
  change, as Options+ paints (#72, display pipeline report items 1, 2, 3
  and 5). §5.5 the daemon keeps a canvas of the panel and sends the union
  of the key rectangles on a claim, reclaim, new view or new panel color,
  else a crop of the changed keys; at most two pending areas, the next
  image after the fn2 response; the setImage header fields named in
  `ImageHeader`; `maxImageSize` from fn0 caps each image and a larger crop
  goes as horizontal bands; encoder quality 100. Replaces the
  background-then-keys order of #77 (slice 6h). §9 the canvas API. No new
  `[render]` or `[timing]` key.

- **v0.3, pre-alpha slice 6k (2026-10-25)**: §5.10 `keyvo probe` prints
  the display fn0 capabilities and the fn1 button and visible-area counts
  by name (#89), decoded in `keyvo_hid::display`; `keyvo fill --full`
  paints the whole published panel at 0,0.

- **v0.3, pre-alpha slice 6j (2026-10-25)**: corrections from the display
  pipeline report (#86, #88). §5.4 the display state names (`a3` Splash
  to `a6` Dfu, `d2` Idle, `d3` LowPower), ClearScreen as a black image,
  the fn1 fields (button count, visible-area count); §5.5 the fn2
  `setImage` header (`displayIndex`, `deferDisplayUpdate`, `count`,
  12-byte image headers) and the fn0 capabilities (`maxImageSize` 124 928
  bytes in KiB units, not 31 262; `maxImageFPS` 30; the flag bits). No
  wire byte changes.

- **v0.3, pre-alpha slice R3 (2026-10-25)**: the docs aligned with the
  pre-alpha as built, from the epic pre-review (#82). §5.10 `keys` and
  `fill` take their keep-alive and divert timing from the user's
  `config.toml` under their flags; §4 the layout tree marks what is
  planned; §4.1 the test doubles are public items of `keyvo-core`, and the
  render signature is the target, with the built `render::page`; §6.2 the
  X11 watcher is planned; §9 `[render].splash` has no effect until M1-6;
  §10 the app refetches on the daemon's `config` event and has no file
  watcher of its own; §8 and §12 `doctor` and the Claude Code integration
  are planned.

- **v0.3, pre-alpha slice R1 (2026-10-25)**: hardening from the epic
  pre-review security pass (#81). §6.2 the KWin script directory is
  checked and the script rewritten before every load; §12 the app's own
  lockfiles are checked by `cargo deny` (`app/src-tauri/deny.toml`) and
  `npm audit` in `just app-test`; §10 the app manifest and a capability
  that allows the six app commands and event listening, without
  `core:default`; §8 the clients check the socket directory and the
  daemon's uid before they send; §8 and §6.1 the socket limits
  `[daemon].socket_line_bytes`, `[daemon].socket_connections` and
  `[timing].accept_backoff`, and the client's line limit.

- **v0.3, pre-alpha slice 6i (2026-10-25)**: §5.10 `keyvo fill
  --margin <px>`, key rectangles grown and clamped to the panel (#80);
  §5.4 the margin measurement on the device replaces the photos of #70
  and is the source of the #72 default, with the limits of the published
  geometry.

- **v0.3, pre-alpha slice 6h (2026-10-25)**: §5.5 the order of a paint,
  panel background first and then the keys, on every claim, reclaim and
  change of the profile's panel color (#77, from first light #40,
  finding 2); §9 the panel background as built (the accent tinted over
  the theme's black panel, tile corners over it, no new `[render]` key)
  and the preview between the keys; §5.4 the strip beside a key now
  names #77 next to #70 and #72.

- **v0.3, pre-alpha slice 6b (2026-10-25)**: the first-light measurements
  (#40, firmware `U1 66.00.B0017`) in `docs/hardware-notes.md`; §5.1 one
  hidraw node; §5.3 without pings input arrives and a paint stays, the
  revert to the logo is the `KEEP_ALIVE` expiry, its timing, the expiry
  event before the mode event; §5.4 `a2` after plugging in, `a1` after an
  expiry, VLP root fn2 not needed, the device's key origins (five differ
  from the capture), the visible key window; §5.5 both index bases and
  software ids paint, defaults kept, the ACK layout with the last-fragment
  byte, ACK-driven pacing confirmed; §5.6 the key report and page-button
  layouts as measured; §5.9 the first-light fixtures.

- **v0.3, pre-alpha slice 6d (2026-10-25)**: §5.10 the global `--dump`
  flag and `keyvo probe --feature <id>` from first light (#40, finding 6);
  §5.2 a VLP `getFeature` index 0 is taken as absent.

- **v0.3, pre-alpha slice 6c (2026-10-25)**: §5.3 the period-below-timeout
  rule, the lost-claim events of a keep-alive expiry from first light (#40)
  and the reclaim with its repaint; §6.1 the rule next to the two
  keep-alive rows, enforced for `[timing]` and the `fill`/`keys` flags.

- **v0.3, pre-alpha slice 16 (2026-10-25)**: §4 and §4.1 `Editor` and
  `Problem` in `keyvo-client`; §7.1 round-trip editing (`keyvo_core::edit`),
  snapshots and the atomic write (`keyvo_core::store`), the state
  directory; §10 the shell's `key` and `edit_key` commands and the key
  editor as built (immediate apply, validation, undo, errors with file and
  line).

- **v0.3, pre-alpha slice 15 (2026-10-25)**: §4.1 `Client::subscribe`,
  `Client::dry_run`, `follow` and `Update`, `tiles::keys`; §10 the shell's
  `press` and `subscribe` commands, `tiles` with labels and actions, and
  Home as built (mirror, status strip, event list, virtual pad, keyboard
  and aria labels, the unreachable state).

- **v0.3, pre-alpha slice 14 (2026-10-25)**: §4 the fifth workspace crate
  `keyvo-client` in the tree, `app/src-tauri` outside the root workspace;
  §4.1 the client side (`Client`, `Mirror`, `Tiles`, `socket_path`) next to
  the platform traits; §10 the app scaffold as built (frontend, tokens with
  fixed defaults, the shell's commands, `just app-test` and
  `just app-build`); §12 the five crates, the desktop app dependencies and
  the Tauri build packages of the dev image.

- **v0.3, pre-alpha slice 13 (2026-10-25)**: §4.1 the daemon serves the
  control socket, `--socket`; §6.2 `notify_context` as built; §7.1
  `[trace]` in `config.toml`; §7.2 the socket `reload`; §8 the socket API
  subset as built (`status`, `subscribe`, `press` with `dry_run`,
  `reload`, `notify_context`), `keyvo ctl`, the in-memory trace ring
  buffer with `[trace].ring`; `docs/socket-api.md` is the wire contract.

- **v0.3, pre-alpha slice 12 (2026-10-25)**: §4.1 `WindowContext`
  carries the window's `pid` when known, the daemon binary uses the KWin
  watcher; §6.2 the KWin watcher as built (script, private object,
  script directory, load sequence, `captionChanged`, a missing KWin or
  session bus, tests without KWin); §12 `zbus` is 6.x.

- **v0.3, pre-alpha slice 11 (2026-10-25)**: §4.1 the `WindowWatcher`
  signature and `FakeWindowWatcher`; §6.1 the state machine as built,
  context and general page, key states per profile page; §6.2 class and
  title matching, priority and tie-break, the focus hysteresis as built;
  §6.3 the page buttons as built (short and long press, manual paging,
  app switch and page grace, the escape); §7.2 a reload selects the
  profile for the focused window again, `focus_hysteresis` and
  `page_grace` are read at start.

- **v0.3, pre-alpha slice 10 (2026-10-25)**: §4.1 the `InputInjector`
  signature, the uinput keyboard and the daemon as built; §5.1 the
  reconnect of the device actor; §5.5 coalesced paint requests and
  writes of only the changed keys; §6.1 rows for `exec_timeout`,
  `reconnect_interval` and `reload_settle`; §7.2 hot-reload as built,
  keeping the last good state; §8 how `exec` runs, its pre-alpha
  environment and timeout; §9 the daemon paints the feedback looks, the
  spinner is not animated yet.

- **v0.3, pre-alpha slice 9 (2026-10-25)**: §7.2 the chord grammar as
  parsed (aliases, shifted symbols, every evdev key name, sequences),
  chords checked when a profile loads, press kinds `short` and `long` as
  built and both running the key's one action; §8 the trace record type
  and the press resolution are defined in `keyvo-core`, the pre-alpha
  record fields and results; §9 the feedback states and their looks as
  built (dimmed icon and spinner arc, badges, per-key state overrides).

- **v0.3, pre-alpha slice 8 (2026-10-25)**: §9 the JPEG encoder is the
  `image` crate's (4:4:4) instead of the IJG-licensed `jpeg-encoder`, the
  renderer as built (layers, proportions, label fitting, contrast, the
  bundled lucide subset and its fallback), `keyvo preview` and its panel,
  `[render].splash` defaults to `false`; §12 the crate list; §16 question
  16 resolved.

- **v0.3, pre-alpha slice 7 (2026-10-25)**: §7.1 how the configuration
  directory is found and loaded, durations in milliseconds, unknown keys as
  errors, diagnostics with file, line and column, the committed JSON
  Schemas in `schemas/`; §7.2 the schema version 1 subset that `keyvo-core`
  reads.

- **v0.3, pre-alpha slice 5 (2026-10-25)**: §5.1 keyvo's sysfs node search
  and the hidraw transport; §5.2 direct `getFeature` answers cached, the
  firmware version decode; §5.10 the device commands `keyvo probe`, `keys`
  and `fill`; §6.1 a row for `reply_timeout`.

- **v0.3, pre-alpha slice 4 (2026-10-25)**: §5.1 the cap on the unmatched
  queue, unparsable reports dropped and counted, the input read order;
  §5.6 the input decoder's gates and event order, keyvo's divert sequence
  and its verify step; §6.1 rows for `divert_settle` and `divert_spacing`.

- **v0.3, pre-alpha slice 3 (2026-10-24)**: §5.5 the fragment payload
  sizes, the sequence byte bits, index base and software id as writer
  settings, placement from the geometry, the frame header check, the
  fragment-index size guard, the ACK layout and the timeout fallback; §6.1
  a row for `ack_timeout`.

- **v0.3, pre-alpha slice 2 (2026-10-24)**: §4.1 the `Clock` trait; §5.2
  multi-packet VLP replies and the last-packet bit; §5.3 the keep-alive
  scheduler; §5.4 keyvo's claim order, the mode-change event handling and
  the exact geometry table layout with all nine key origins; §6.1 one row
  each for `keep_alive_period` and `keep_alive_timeout`.

- **v0.3, pre-alpha slice 1 (2026-10-24)**: §4.1 the `HidTransport`
  signature and `FakeHidTransport`; §5.1 reply matching includes the device
  index, unmatched reports are kept, HID++ 2.0 error replies; §5.2 the root
  index, both `getFeature` reply layouts, the `e0` request sequence byte;
  §5.9 the HID fixture notation in `tests/protocol-fixtures/hid/`.
- **v0.3 (2026-10-24)**: UI/UX grill applied
  (`docs/plans/2026-10-24-gui-ux-grill-decisions.md`): §6 project layer in
  the stack and project context source; §7 `recipe` action, `on_fail`,
  automatic states, `.keyvo/` files and `[project_layer]` (§7.4), recipes and
  starter packs (§7.5), state directory, export and import; §8 project
  environment variables, `press` with dry run, `trace`, `project`, `setup`
  and `doctor` commands, the unified trace; §9 feedback primitives; §10
  rewritten around the device mirror, native theme, simulator and the
  `keyvo setup` flow, pointing at `docs/design/gui-ux.md`; §15 milestone
  texts; §16 questions 20 and 21.
- **v0.2 (2026-09-09)**: renamed `tessera` → `keyvo`; §1 prior art
  restated (logimap, logilinux, Solaar called not copied); §2 MX Keypad
  status, Dialpad facts, Bolt receiver row; §3 standalone daemon primary,
  Flatpak moved to M4b, udev rule mandatory; §4 architecture tree from the
  plan (four crates, `integrations/`, `tools/windows-vm/`,
  `tests/protocol-fixtures/`), platform traits, pure render function; §5
  rewritten from research (two protocol planes, keep-alive, display claim,
  device-published geometry, 24-bit length, ACK-driven flow control,
  feature-index resolution, own-write ACK vs key press, Dialpad over
  `0x1B04` + `0x4610`); §6 timing defaults and `general_hint`, page-grace
  rule, desktop coverage and `notify_context` contract; §7 config layout,
  key numbering, class matching, `toml_edit` round-trip, JSON Schema, `.lp5`
  import; §8 socket path/mode/versioning, `keyvo ctl`, `plugin.toml`, no
  D-Bus façade, no Lua; §9 renderer pipeline and test policy; §10 GUI scope
  and stack; §11 C# plugin in the Windows VM; §12 toolchain and crates; §13
  prior-art table replaced with the credited list; §15 milestones aligned
  with the plan; §16 every question resolved, linked to a measurement item,
  or marked open.
- **v0.1 (draft)**: initial specification from design discussions.
