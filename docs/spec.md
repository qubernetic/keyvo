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
│   │                   #   0x8040, 0x4610), VLP display plane (0x13/0x14, 0x19A1), Keypad + Dialpad
│   │                   #   drivers, hot-plug, keep-alive task, ACK-driven image writer
│   ├── keyvo-core      # profiles (TOML, toml_edit round-trip, JSON Schema), state machine
│   │                   #   (mode/page/overlay), renderer (tiny-skia/resvg/Inter → JPEG), actions,
│   │                   #   socket protocol types, chord parser; sync, hardware-free, ≥85 % covered
│   ├── keyvo-daemon    # tokio runtime: socket server, KWin watcher (zbus), uinput injector
│   │                   #   (evdev), inotify hot-reload; planned: X11 watcher (x11rb), plugin
│   │                   #   supervisor, systemd unit
│   ├── keyvo-client    # socket client, mirror state, tile rendering through keyvo-core, key
│   │                   #   editor file side, for the GUI and later clients; sync, hardware-free
│   └── keyvo-cli       # `keyvo probe|keys|fill|preview|ctl`; planned: `paint-test|doctor|profile`
├── app/                # Tauri 2 + Svelte 5 GUI (M4a); src-tauri/ is outside the root workspace
├── integrations/       # planned: claude-code/ (plugin: hooks, commands, MCP), vscode/, tmux/, nvim/ …
├── plugins/logi/       # planned: C# Logi Actions SDK plugin (M5), built in the Windows VM
├── contrib/            # KWin script, example profiles; planned: udev rule, systemd unit, Flatpak manifest
├── captures/           # README + derived fixtures; raw pcapng external (release assets)
├── tools/keycodes/     # generator of the evdev key code table of the chord parser
├── tools/windows-vm/   # planned: libvirt VM scripts (from TwinCat-MCP)
├── schemas/            # generated JSON Schemas of profiles and config.toml
├── tests/protocol-fixtures/  # shared fixtures for Rust/TS clients: socket/ NDJSON, editor/, hid/ golden HID++ scripts
└── docs/               # spec.md, roadmap.md, socket-api.md, adr/, design/, research/, plans/,
                        #   hardware-notes.md, pre-alpha-test.md; planned: diagrams/, mdBook
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
```

`HidTransport` (`keyvo_core::hid`) moves whole reports, report id first:
`write(report)` and `read(timeout) -> Option<report>`, where `None` is a
timeout. It knows nothing about HID++; framing, reply matching and feature
resolution live in `keyvo-hid`. `FakeHidTransport` replays a script of
request/response pairs in the fixture notation of §5.9 and fails on any
write the script does not expect.

The test doubles (`FakeHidTransport`, `FakeClock`, `FakeInputInjector`,
`FakeWindowWatcher`) are public items of `keyvo-core`, not behind a
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

Clients implement none of the three traits: they reach the device only
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
daemon paints on the LCD, and `tiles::keys` gives each key's label and
action for the GUI to announce. Its tests run against the socket fixtures in
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
`FakeWindowWatcher`, socket clients and a configuration directory on
disk. Run it with `just build`, then `bin/keyvo-daemon [--config-dir DIR]
[--device PATH] [--socket PATH]` on the host; logs go to stderr.

Core rendering is, as the target, the pure function

```
(profile, page, mode, overlay_stack, window_ctx) → 9 KeySpecs + accent + dialpad bindings
```

so the whole context engine is testable without a device. As built
(pre-alpha) it is `keyvo_core::render::page(stack, keys)`: the specs of
keys 1 to `keys` of one page over the global layer
(`keyvo_core::stack::Stack`, §6.1); the state machine picks the profile
and page, overlays and dialpad bindings are not built yet, and the
accent is read from the profile on top.

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

- Identity `046d:bc00` over Bluetooth; over the Bolt receiver the HID++
  device index is 1–6 on the receiver's `0x11` reports (untested by anyone;
  M0-2 captures it under Options+ in the Windows VM, plan §5 item 15).
- Feature table (measured over Bolt): `0x1B04` at index `0x0a` with four
  divertable controls, CIDs `0x0053, 0x0056, 0x0059, 0x005A`; `0x4610`
  MultiRoller at index `0x0d`; `0x1004 UNIFIED_BATTERY`; `0x1D4B
  WIRELESS_DEVICE_STATUS`; **no `0x0008` keep-alive**.
- Rollers (reported, shensquared / Julusian PR #22): `0x4610 fn0` → 2
  rollers; roller 0 = small wheel, 40 increments per rotation; roller 1 =
  large dial, 180 per rotation; `fn3 setMode(roller, 1)` diverts a roller
  (native scroll stops); rotation notifications `11 ff <idx> 00 <roller>
  <delta:i8> 00 <ts…>` with accelerated signed int8 deltas; roller 0 reports
  the opposite sign of roller 1. Diverted button presses repeat while held.
  There is no dial click.
- Diverts are restored on exit and re-applied on reconnect; a Bluetooth
  device that sleeps comes back as a new HID object. The mouse collection
  floods report `0x02` while turning; keyvo ignores it and, as a
  non-diverted fallback, can read the evdev axes (`REL_WHEEL*`,
  `REL_HWHEEL*`, keys 275–278).
- Nothing in the Dialpad path uses Solaar.

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

---

## 6. Context engine

### 6.1 Modes, pages and overlays

```
mode           ∈ { auto, manual }
page           : auto → { context, general }; manual → page_index
active_profile : chosen by WindowWatcher rules (auto) or user (manual)
overlay_stack  : ordered overlays pushed via the socket API (e.g. Claude Code status)
```

Rendering is a **stack**: global layer < app profile < project layer <
overlays (ADR-0009). The project layer (§7.4) is the nearest `.keyvo/`
directory above the working directory of the focused window's context (§6.2)
and overrides single keys of the global and app layers. Overlays apply to the
*context* page only, never to the *general* page, so media keys stay reachable
while an agent is working. A profile may declare an
optional `general_hint`: a single key on the general page that mirrors the
top overlay's state (decided in the grill).

As built (pre-alpha, `keyvo_core::state`, no overlays or project layer
yet): the state machine starts in auto mode on the global profile's
context page. A profile's *context page* is its page named `context`,
else its first page; the *general page* is the global profile's page
named `general`. The daemon reports every change of profile, page or
mode as a `Switched` event and in the log, then repaints the keys that
differ. The stack has two layers (`keyvo_core::stack`): under every page
of an app profile lies the global profile's context page, so a key the app
profile does not define shows and runs the global key, with the app
profile's accent; a key the app profile defines hides the global one, even
without an action. The global profile has nothing under it. The trace
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
| `page_grace` | 10 s | see §6.3 |
| `keep_alive_period` | 1000 ms | time between two keep-alive pings, §5.3 |
| `keep_alive_timeout` | 3000 ms | timeout requested with every ping: how long the device holds the claim without the next one, §5.3 |
| `ack_timeout` | 50 ms | how long an image fragment waits for its ACK before the next one goes out anyway, §5.5 (captured ACK latency about 1 ms) |
| `divert_settle` | 100 ms | wait before the first page-button divert request, §5.6 (Julusian PR #22) |
| `divert_spacing` | 20 ms | wait between two `0x1B04` divert requests, §5.6 (Julusian PR #22) |
| `reply_timeout` | 1000 ms | how long an HID++ request waits for its matching reply, §5.1 and §5.2 (captured replies come within about 1 ms) |
| `exec_timeout` | 30000 ms | an `exec` action still running after this long is killed with its process group and fails, §8 |
| `reconnect_interval` | 1000 ms | wait between two attempts to open the Keypad after it is gone or not found, §5.1 |
| `reload_settle` | 100 ms | quiet time after the last change in the configuration directory before the daemon reloads it, §7.2 |
| `accept_backoff` | 100 ms | wait after a failed `accept` on the control socket before the next one, so a full file table does not spin the daemon, §8 |
| `claim_splash_retry` | 500 ms | while the display shows the plug-in splash (`a4`), the claim re-sends `setMode(a0)` this often, §5.4 |
| `claim_splash_timeout` | 10000 ms | how long the claim waits for the plug-in splash to end before it fails, §5.4 |

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
| `hold_classes` | `["keyvo-app"]` | window classes, compared ASCII-case-insensitively as the whole name, that never change the context; an empty list holds nothing |

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

- `auto` mode: either page button toggles context ↔ general.
- `manual` mode: page buttons page through the active profile's pages.
- Long-press on either page button toggles `auto ↔ manual`.
- On app switch while on the *general* page in auto mode, keyvo returns to
  the context page, **unless** a page button was used within the last
  `page_grace` (10 s); then the user's choice sticks (decided).
- Manual mode is visually distinguishable (dimmed/dashed frame or a corner
  "lock" badge, theme-controlled).
- Both page buttons held together is reserved as an always-available escape
  chord (return to the global profile, context page, auto mode).
- As built (pre-alpha, `keyvo_core::state`): a page button acts on its
  release when it was held shorter than `long_press`, and toggles the mode
  once `long_press` is reached, without waiting for the release. In auto
  mode a short press of either button toggles context and general; with
  no general page it does nothing. Entering manual mode keeps the profile
  and page the pad shows; there the right button pages forward and the
  left back, both wrapping around, and focus changes are recorded but not
  shown. Leaving manual mode shows the context page of the profile the
  focused window selects. An app switch is a focus change to another
  window class; a title change in the same app keeps the general page.
  Every press and release of a page button counts as its use for
  `page_grace`. The second button held escapes; neither button then acts
  again until it is pressed anew. Until a later focus change the global
  profile shows. The visual mark for manual mode comes later.

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

[dialpad]
dial   = { type = "chord", cw = "ctrl+]", ccw = "ctrl+[" }
roller = { type = "chord", cw = "ctrl+tab", ccw = "ctrl+shift+tab" }

[desk_state]                # applied via Solaar on profile activation (optional, M3)
mouse.thumb_button   = "goto_definition"
keyboard.backlight   = { mode = "static", brightness = 60 }
mouse.dpi            = 1200
```

- Pre-alpha subset (schema version 1 as `keyvo-core` reads it today):
  `schema_version`; `[profile]` with `id`, `name`, `match` (`class` list,
  `title` regex, checked when the file loads), `accent`, `priority`
  (integer, default 0); `[[pages]]` with an optional `name` and
  `keys.1` to `keys.9`, each with `icon`, `label`, `color`, `action` and
  `states.<name>` (`icon`, `label`, `color`, `badge`). Colors are
  `#rrggbb`. Actions are `chord` (`keys`) and `exec` (`cmd`). The other
  keys of the example above (`general_hint`, `on_fail`, the `recipe` action,
  `[dialpad]`, `[desk_state]`) are rejected as unknown until their issues
  add them.
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
  per-kind bindings and `repeat` come with M1-9.
- Action types (v1): `chord` (key combo, sequences with `,`, per-step
  delays, hold/release), `exec` (non-blocking subprocess with `KEYVO_*`
  env, §8), `http` (GET/POST), `text` (type a string), `page` / `profile` /
  `mode` (internal), `recipe` (§7.5, resolved to one of the others at press
  time). Chord aliases (`ctrl`/`control`, `super`/`meta`) and shifted symbols
  (`ctrl+?`) are accepted; the full evdev keycode table is supported.
- `on_fail` (optional, per key or per press kind): an action run when the
  primary action fails (non-zero exit, timeout, non-2xx, unresolved recipe).
  It is not retried and has no `on_fail` of its own.
- Automatic states: an `exec` or `http` action switches the key's state to
  `running` while it runs, then to `success` or `error`; the looks are the
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
  profile while it exists, else the global context page shows, and a page
  that is gone falls back to page 1. A general page that is gone shows
  the context page. Key states carry over. When the directory does not
  load at start, the daemon runs with its `config.toml` (when that one is
  valid) and the built-in global profile. A missing directory turns
  hot-reload off with a warning. The `[timing]` values of the device
  (§5), `long_press`, `focus_hysteresis`, `page_grace` and
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
and `KEYVO_STATE` is not read yet. Its exit code is the
trace result (`exited`, with the duration); past `[timing].exec_timeout`
the whole process group is killed and the result is `failed` with "timed
out after <n> ms". A chord's result is `sent` with its key events, or
`failed` with the injector's error.

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
(drive the first-run steps, §10), `doctor`, `subscribe` (event stream:
key events, profile/page/mode changes, device connect/disconnect, focus and
context changes, trace entries, config reloads, `trust_requested`,
`recipe_unresolved`). `keyvo ctl <command> [json]` is the CLI front-end;
`keyvo ctl subscribe` streams events, `keyvo ctl trace` prints the last
entries and `keyvo ctl tail` follows them. Wire details live in
`docs/socket-api.md`; fixtures in `tests/protocol-fixtures/` are shared by
the Rust, TypeScript and C# clients.

As built (pre-alpha, `keyvo_core::socket` and `keyvo_daemon::socket`):

- The commands are `status`, `subscribe` (classes `key`, `profile`, `page`,
  `mode`, `device`, `context`, `trace`, `config`, with an optional replay
  of the newest trace entries), `press` (`key`, `kind`, `dry_run`),
  `reload`, and `notify_context` (`class`, `title`). Any other command and
  any unknown field is rejected as `invalid_request`.
- A focus report of a class in `[focus].hold_classes` (§6.1) goes out as
  a `context` event with `held: true` and is the `context` of `status`;
  the view does not change, so no `profile`, `page` or `mode` event
  follows.
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
page, key, the resolved action, and the result: `unbound`, `dry_run`,
`pending`, `sent` with the chord's key events, `exited` with exit code and
duration, or `failed` with the reason; one serialized record per line is in
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
  and near-black on a light one (Rec. 709 luma above 140). Icons are a
  bundled subset of lucide 1.61.0 (ISC) under
  `crates/keyvo-core/assets/lucide/`, with lucide's deprecated names
  (`git-commit`, `circle-help`) accepted; an unknown icon, and every
  `phosphor:`, `simple-icons:` and `file:` reference until their issues,
  draws `lucide:circle-question-mark` and `keyvo preview` names it. Only
  the embedded font is loaded, never a system font, so a spec renders to
  the same pixels everywhere. Badges and the feedback looks are described
  below. The theme file, gradients, caching and the size guard come with
  later issues.
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
- Tray icon from the GUI in v1. `keyvo profile export|import` in the header
  menu.
- Flatpak (M4b): `--device=all`, `--socket=wayland`, `--socket=fallback-x11`,
  `--talk-name=org.kde.KWin`, socket dir under `xdg-run`; the daemon runs on
  the host as a systemd user service, the Flatpak ships the GUI and CLI.
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
(identifier `org.qubernetic.keyvo`, bundling off) has seven commands, all
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

- **Rail.** Pad (screen id `home`, today's Home content), Profiles,
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
  the Pad's inspector reads the pin from the next slice on); the Sandbox
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
- **Columns.** List column, editor and right column; the right column
  collapses below 1200 px, the list column below 1000 px, and a toggle at
  each column's edge shows or hides it until the width crosses its limit
  again. Home has the editor area only. Home stays mounted while another
  screen shows, so the header follows the daemon; its `Ctrl+Z` then does
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

**As built (pre-alpha Home).** Home follows `docs/design/gui-ux.md` §3.1,
§5.2 and §8 (d1, d7, d9, P4); it has no list column. On start Home asks
`status`, then subscribes with a replay of the newest 20 trace entries;
`app/src/lib/home.ts` folds the updates the way `Mirror` does.

- **Mirror.** A `grid` of the tiles of the active profile and the shown
  page, fetched again when the view changes and after every reload. Above
  it, the page selector (the pages of the active profile, from the shell's
  `outline`) and the profile's accent bar (none when the profile has no
  accent). The selector shows the pad's page and follows it; a page picked
  there stays until the pad's next view, the key editor then edits that
  page, and a press on it only selects the key, with a result line saying
  that presses simulate the page the pad shows (the daemon resolves a
  press on its own page). Rows, key
  count and tile size come from the `geometry` the daemon reports for the
  Keypad (`status` and the `device` event, §8); the app hard-codes none of
  them. A key starts a new row when its top edge is below the middle of
  the row's first key. The tiles sit in the key cutouts of the Keypad
  drawing (`app/src/Device.svelte`, gui-ux §3.1), coloured from
  `--kv-device`, and scale with it: the column's width, at most the
  panel's pixel width, so never above 1:1. A geometry whose key count
  differs from the drawing's cutouts puts the tiles on a plain panel at
  the reported rects instead. The last geometry stays when the Keypad
  goes; before any Keypad reported one, a note replaces the grid. A key the active app
  profile does not define shows the global key (§6.1) with a "global"
  mark. A key held on the device is outlined. A page the daemon's
  configuration cannot render shows why.
- **Status strip.** One fact each: device, profile, page, mode, top
  overlay (`none`: the daemon has no overlays yet) and context
  (`unknown project`, as the daemon sends no project yet, and the focused
  window; a held window, §6.1, reads `keyvo-app focused, holding
  org.kde.konsole`, or `holding the context` when the app saw no window
  before it). Profile, page and mode link to Profiles; device, overlay and
  context belong to tabs of Preferences, which are not built, so they
  are plain text. A failed reload shows as an error notice in the
  strip: `Schema error in <file>, line <n>, column <n>: <message>` when the
  error names a place (`<file>:<line>[:<column>]: <message>`), else the
  error as it is, with a link to Profiles, until a reload succeeds.
- **Event list.** The trace entries, newest first, one line each: the key,
  the press kind, whether it was simulated, the layer that supplied the
  key (`app layer` or `global layer`), the action and its result. Each
  line opens the trace drawer.
- **Trace drawer** (`app/src/TraceDrawer.svelte` on `Drawer`). Opens from
  an event line and from the result line of a dry-run press, takes the
  focus, and closes on `Esc` or its close button, giving the focus back.
  Its header carries the Debug log switch, disabled with "Coming soon"
  until the daemon sends HID-level events. It shows the
  sections of the entry the daemon records today, in resolution order:
  Trigger (time, key, press kind, the device or simulated), Context
  snapshot (profile, page), Resolved action (layer, type, keys or
  command), then Result (status; a chord's key events, an `exec`'s exit
  code or signal and duration, a failure's error) or Block reason (dry
  run, or no layer binds the key). Conditions, variables and the window
  context come with the daemon fields that carry them.
- **Virtual pad.** A click is a short dry-run press, a right click a long
  one; the one-line result shows under the grid. On the grid, `1` to `9`
  (up to the key count) and the arrows select a key, `Space` presses it
  short and `Shift+Space` long.
- **Accessibility.** Each key's `aria-label` names the key, its label, its
  action and its layer; the grid has rows and cells, the selected cell is
  `aria-selected`, and the result line is a `status` region.
- **Screen states** (design §5.2). Empty: the event list says how to fill
  it (press a key on the pad, or click one in the mirror). Loading: three
  skeleton rows in the event list (`aria-busy`), no spinner; the mirror
  keeps its last frame. Device missing: a note names the Keypad and the
  time it was last seen connected, or that it was not seen since the app
  started; the mirror keeps the last geometry. Schema error: in the status
  strip, above. Daemon unreachable: when `status` fails or the
  subscription ends, an alert says why, the mirror keeps its last frame,
  and Retry connects again. The in-process simulator of §4.2 of the
  design and the header badge come later.

**As built (pre-alpha key editor).** Beside the mirror, the key editor
(`app/src/KeyEditor.svelte`, `app/src/lib/editor.ts`) shows the key
selected on the grid: its profile file, and its label, color (`#rrggbb`),
icon and action (none, a chord's keys or an `exec` command).

- **Immediate apply.** A field is written the moment it changes (the
  input's `change`, on Enter or when the focus leaves it), through the
  shell's `edit_key`: a snapshot, then the atomic write of §7.1. There is
  no save button. Writes run one after the other. The daemon's inotify
  watch reloads the file and repaints the pad; the mirror fetches its tiles
  again after the write and after the daemon's `config` event.
- **Validation.** A color that is not `#rrggbb` or an action without its
  keys or command is not written; an alert says why. A new action type
  starts empty and is written when its keys or command are entered. A chord
  the parser refuses comes back from the shell as the error.
- **Undo.** `Ctrl+Z` and `Ctrl+Shift+Z`, or the Undo and Redo buttons, undo
  and redo the session's writes, newest first, each by writing the fields
  before it again through the same path; a new write drops what is left to
  redo. Inside a form field the keys undo the typing instead. The history
  lives as long as the window.
- **Errors.** A profile file that does not validate (an external edit, for
  example) shows as an alert `file:line:column: message` and the form is
  hidden; the file is never rewritten to fix it. A failed write shows the
  same way and leaves the history as it was.
- **Reloads.** After the daemon's `config` event the editor reads the key
  again; a field being edited keeps its text. "Open in editor" for the
  whole profile file is on Profiles (below); the file watch of
  `docs/design/gui-ux.md` §4.1 comes with M4a-4.
- **External changes, as built.** The app has no file watcher of its
  own: it refetches the tiles and the selected key when the
  daemon sends its `config` event, which the daemon's inotify watch
  (§7.2) triggers. `docs/design/gui-ux.md` §4.1 stays the target.

**As built (pre-alpha Profiles).** Profiles (`app/src/Profiles.svelte`,
`app/src/lib/profiles.ts`) follows `docs/design/gui-ux.md` §3.2 and §4.1
(d1, d2, d14, ADR-0004) with the three columns of the shell. It reads the
profiles of the daemon's configuration directory through the shell's
`profile_list` (`keyvo_client::profiles::list`), again after every reload
and every write of the screen.

- **List column.** Every profile: the global one pinned first, then by
  `priority`, highest first (the one that wins a match, §6.2), equal
  priorities by id. Each row shows the accent, the name, the id and the
  priority; the profile the pad shows is marked "active". The screen opens
  on the active profile, else the global one. The "+" button (new profile
  from a template) is disabled with "Coming soon" (M4a-4). A configuration
  that does not load shows as an alert with the load errors.
- **Profile header.** Id and priority as read from the file; name and
  accent as fields, each written the moment it changes through the shell's
  `edit_profile` (`keyvo_core::edit::ProfileDocument::set_profile`): a
  snapshot, then the atomic write of §7.1, every byte outside the edited
  value kept, an empty accent removed, a new one added at the end of
  `[profile]`. An empty name or an accent that is not `#rrggbb` is not
  written; an alert says why. The file path, and "Open in editor", which
  hands the file to the desktop's application for it (`xdg-open`, shell
  command `open_profile`; the path comes from the id, never from the
  page). For the built-in global profile, which has no file until its
  first edit, the button is disabled and a note says so. Header writes are
  not on the key editor's undo stack; the snapshot keeps the file before
  each one.
- **Match rules.** The classes and the title regex as written, read-only;
  "Edit match rules" is disabled with "Coming soon" (M4a-4). The global
  profile says it has none and is under every window.
- **Pages as tabs.** One tab per page, named by the page's `name`, else
  `Page <n>`; the arrows, `Home` and `End` move between them. A newly
  selected profile starts on page 1.
- **Key grid.** The keys of the selected page with their numbers and
  labels, laid out by the device geometry when the daemon reports one,
  else as the nine key slots of the schema in three rows. A click, `1` to
  `9` or the arrows select a key, and the key editor above (label, color,
  icon, action; immediate apply, `Ctrl+Z` and `Ctrl+Shift+Z`) opens on it
  below the grid. "Move key" (drag and drop, `Ctrl+X` then `Ctrl+V`) and
  "Open in Behaviors" are disabled with "Coming soon" (M4a-4, M4a-5).
- **Right column.** The mirror of the selected page, the tiles from the
  shell's `tiles` at the device's tile size (a click selects the key), and
  the key card: position, page (number and name) and layer origin, the
  layer of the stack that supplies the key (§6.1: `app layer`, `global
  layer`, or none when no layer defines it). Without a reported geometry
  the mirror is a note and the layer origin says it is known once a Keypad
  reported its geometry.

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
fail. A build without the variable contains no mock module.

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
- Logging with levels (`tracing`); `keyvo doctor` (planned) diagnoses hidraw
  permissions, udev rule, `input` group / ACLs, `/dev/uinput`, KWin script
  state, socket ownership, Solaar presence, Options+ ownership (Windows).
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
`org.gnome.Platform//51` runtime until the Flatpak exists. The list below
is the order of the remaining milestone work.

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
| 5 | Return to context page on app switch from general page | resolved → yes, unless a page button was used within 10 s |
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
