# keyvo — Software Specification (v0.2)

> Context-aware, scriptable control-surface daemon, CLI and desktop app for the
> Logitech MX Creative Console family (Keypad, Dialpad) and related HID++
> devices. Linux-native first (KDE Wayland, X11 fallback); Windows via a
> Logi Actions SDK plugin and an optional native mode.
>
> Status: v0.2, research-corrected. Supersedes draft v0.1. Decisions were
> taken in the planning grill of 2026-09-09 and are recorded in
> `docs/plans/2026-09-09-project-plan.md` (§1) and in `docs/adr/`. Protocol
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
- Embedded scripting language (Lua) — see §8 and ADR-0003.

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
│   ├── keyvo-daemon    # tokio runtime: socket server, KWin/X11 watchers (zbus/x11rb), uinput
│   │                   #   injector (evdev), plugin supervisor, inotify hot-reload, systemd unit
│   └── keyvo-cli       # `keyvo probe|keys|fill|paint-test|preview|doctor|ctl|profile …`
├── app/                # Tauri 2 + Svelte 5 GUI (M4a)
├── integrations/       # claude-code/ (plugin: hooks, commands, MCP), vscode/, tmux/, nvim/ …
├── plugins/logi/       # C# Logi Actions SDK plugin (M5), built in the Windows VM
├── contrib/            # udev rule, KWin script, systemd unit, Flatpak manifest
├── captures/           # README + derived fixtures; raw pcapng external (release assets)
├── tools/windows-vm/   # libvirt VM scripts (from TwinCat-MCP)
├── tests/protocol-fixtures/  # shared NDJSON fixtures for Rust/TS/C# clients
└── docs/               # spec.md, roadmap.md, adr/, diagrams/, research/, plans/, hardware-notes.md, mdBook
```

`keyvo-core` is synchronous and hardware-free; `keyvo-daemon` owns the
tokio runtime and every OS binding.

### 4.1 Platform abstraction (traits in `keyvo-core`, implemented per OS)

```rust
trait HidTransport   // open/read/write HID reports; hidraw on Linux, hidapi elsewhere; FakeHidTransport in tests
trait InputInjector  // Linux: uinput (evdev); Windows: SendInput
trait WindowWatcher  // Linux: KWin script over D-Bus (KDE), X11 _NET_ACTIVE_WINDOW; Windows: SetWinEventHook
```

Core rendering is the pure function

```
(profile, page, mode, overlay_stack, window_ctx) → 9 KeySpecs + accent + dialpad bindings
```

so the whole context engine is testable without a device.

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
- Hot-plug via udev netlink; `POLLHUP`/`EIO`/`ENODEV` mean disconnect. On
  reconnect the driver re-runs the claim sequence (§5.4), re-diverts the page
  buttons (§5.6) and replays the last wanted frame set. Never sleep under a
  lock; one device actor owns the file descriptor and an mpsc queue.
- Never exclusive-grab. Solaar and the kernel `hid-logitech-dj` driver may
  share the Bolt receiver; keyvo filters HID++ traffic by device index and
  matches replies on report id + feature index + function/swid byte, never on
  "first inbound report".

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
both planes, and never hard-code them.

### 5.3 Keep-alive (mandatory)

- The Keypad exposes HID++ `0x0008 KEEP_ALIVE` (measured). Without pings the
  device delivers no key input and does not hold paints; it reverts to the
  logo / device mode (reported by shensquared, mx-console-suite, hcooper).
- Options+ calls `fn0 getTimeoutRange` → `500..10000 ms`, then `fn1
  keepAlive(3000)` and repeats it every ~1.01 s (captured). keyvo runs a
  keep-alive task with the same defaults from the moment the device opens,
  before any read; period and requested timeout are configurable under
  `[timing]`. Repainting is not a substitute (it flickers).
- The firmware self-dims after idle or USB suspend while still accepting
  writes; the daemon re-asserts brightness after resume, after reconnect and
  on wake events (§5.7). Exact revert and dim timings are measured in M0-1
  (plan §5 items 2–4, 11).

### 5.4 Display claim and geometry

- Startup (captured): VLP root fn1 (version info), VLP root fn2 with the
  eight values `0x5e..0x65` (meaning unclear, requirement to be measured),
  `getFeature(0x19A1)` → VLP index 2, display fn3 (mode list
  `a0 a1 a2 a4 a5 a6 d0..d6`), display fn5 get current mode (`a1` at boot),
  display fn4 `setMode(a0)` = host-controlled display, acknowledged by an
  unsolicited mode-change event. `a1` is the device/logo mode; keep-alive
  expiry presumably flips back to it.
- **The device publishes its own key geometry** via display fn1: panel
  480×480, 9 keys, per key `index, x, y, w, h` (w = h = 118, x ∈ {24, 182,
  340}, y ∈ {6/7, 164, 322} in the capture — close to, but not identical
  with, Julusian's `23 + col*158`). keyvo queries the table on every claim
  and uses it for placement; nothing about the grid is hard-coded. This is
  what makes the 2026 MX Keypad a runtime question rather than a code change.

### 5.5 Image writes and flow control

- Format: baseline JPEG, RGB888 (RGB565 is rejected; reported). Per-key
  images are 118×118 at the key's published rectangle; a full-panel paint is
  435×434 at (23, 6) on the 480×480 canvas (Options+, captured; logilinux uses
  434×434). Quality 85–95; the header dimensions must equal the JPEG SOF
  dimensions exactly (mismatch = nothing drawn, write succeeds; reported).
- Fragment layout (`0x14`): first fragment carries a 20-byte header
  `14 ff 02 <fn|swid> <seq> 01 00 01 00 <x:u16> <y:u16> <w:u16> <h:u16> <len:u24>`
  then JPEG bytes; continuation fragments carry a 5-byte header; every
  fragment is exactly 4095 bytes, zero-padded. Sequence byte: `e0` =
  first+last, `a0` = first of several, `61`-style = continuation/last with
  index. Options+ uses index base 0 and swid `0xd`; Julusian/logilinux use
  base 1 and swid `0xb`; both are reported to work.
- **The length field is 24-bit** (captured: `00 11 0a`). The 16-bit limit in
  logilinux/Julusian is a library limitation; keyvo honours 24 bits after the
  M0 check (plan §5 item 10) and guards JPEG size before encoding a frame.
- **The device ACKs every fragment on `0x13`** (`13 ff 02 <fn|swid> c0 …`
  for fragment 0, `c1 00 01 …` for fragment 1, …). The "own writes echo back
  as key 1" trap in other projects is this ACK. keyvo uses **ACK-driven flow
  control**: send the next fragment when the previous one is acknowledged,
  with a timeout fallback. This replaces the blind 5–10 ms pacing of prior
  art and is expected to settle the "nine keys out of sync" problem
  (Julusian #11); confirmed in M0-1 (plan §5 items 8–9).
- Single writer task; repaints are coalesced (superseded frames dropped);
  no shared static buffers.

### 5.6 Input

- LCD keys arrive as `0x13` reports on VLP index 2 (`13 ff 02 00 <seq> 01
  <k1> <k2> … 0`): a list of held key indices (1..9), from which the driver
  derives press/release by diffing against the previous held set. Byte 3
  (function/swid) is `0` for input and non-zero for ACKs; the parser gates
  on it. Repeat rate while held, the meaning of the sequence byte, chords and
  the maximum listed keys are measured in M0-1 (plan §5 item 13).
- Page buttons are `0x1B04 REPROG_CONTROLS_V4` controls CID `0x01A1`
  (left) and `0x01A2` (right), flags `0x20` divertable (measured). Divert
  with `setCidReporting(cid, 0x03)` after a 100 ms settle and 20 ms between
  writes; verify with `fn2 getCidReporting`; the divert is volatile and is
  re-sent on every open and reconnect. Diverted presses arrive as `0x11`
  notifications carrying a u16 CID list; both buttons held is representable.
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

Golden buffers from hcooper's captured frames and notno's packetizer tests
live in `captures/` and `tests/protocol-fixtures/`; `FakeHidTransport`
integration tests replay them. Every protocol claim above that is not marked
*measured* is an item in the M0 measurement list (plan §5) and is recorded
with numbers in `docs/hardware-notes.md`.

---

## 6. Context engine

### 6.1 Modes, pages and overlays

```
mode           ∈ { auto, manual }
page           : auto → { context, general }; manual → page_index
active_profile : chosen by WindowWatcher rules (auto) or user (manual)
overlay_stack  : ordered overlays pushed via the socket API (e.g. Claude Code status)
```

Rendering is a **stack**: global layer < app profile < overlays. Overlays
apply to the *context* page only, never to the *general* page, so media
keys stay reachable while an agent is working. A profile may declare an
optional `general_hint`: a single key on the general page that mirrors the
top overlay's state (decided in the grill).

Defaults, all configurable under `[timing]` in `config.toml`:

| Key | Default | Meaning |
|---|---|---|
| `focus_hysteresis` | 150 ms | a focus change shorter than this does not repaint |
| `render_coalesce` | 50 ms | repaints within this window are merged |
| `long_press` | 1000 ms | hold duration that turns a press into `long` |
| `page_grace` | 10 s | see §6.3 |
| `keep_alive_period` / `keep_alive_timeout` | 1000 ms / 3000 ms | §5.3 |

### 6.2 Window/context detection

- **KDE Wayland (v1):** the daemon pushes its own KWin script over D-Bus
  (`org.kde.kwin.Scripting.loadScript`); the script emits `resourceClass` +
  `caption` on `windowActivated` back to the daemon. Event-driven, never
  polling (OpenDeck #425). The daemon watches `NameOwnerChanged` for
  `org.kde.KWin` and reloads the script after a KWin restart.
- **X11 (v1):** `_NET_ACTIVE_WINDOW` via `x11rb`.
- **GNOME, Hyprland, Sway (community):** no cross-compositor Wayland focus
  protocol exists (KWin 6.7 and Mutter 51 implement neither
  `wlr-foreign-toplevel-management` nor `ext-foreign-toplevel-list`). keyvo
  ships the `notify_context` socket command as the contract: any external
  source (GNOME extension, Hyprland socket2 reader, `swaymsg subscribe`) can
  assert the active class and title. Backends for those compositors are
  contributions behind the `WindowWatcher` trait.
- **Windows:** `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` (native mode only).
- Matching rules: by class (reverse-DNS `org.kde.konsole` and short
  `konsole` both accepted, glob/regex), regex on title (VS Code file →
  language pages, terminal title → Claude Code), explicit priority, and
  **external hooks** via `notify_context`, which take precedence over
  heuristics.

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

---

## 7. Profiles and "desk state"

### 7.1 Config layout

```
~/.config/keyvo/
├── config.toml            # [timing], [daemon], [render], [desk_state], socket, editor
├── profiles/<id>.toml     # one profile per file; <id> is the profile id
├── icons/                 # user PNG/SVG
├── themes/                # theme TOML files
└── plugins/<name>/        # plugin.toml + executable
```

Profiles are plain TOML with a `schema_version`. `keyvo-core` reads and
writes them with `toml_edit` so that comments, ordering and formatting
survive a GUI or CLI edit (**round-trip is an M1 invariant**, ADR-0004). A
JSON Schema generated with `schemars` ships with every release and backs
editor validation. Validation errors carry file, line and column.

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

[dialpad]
dial   = { type = "chord", cw = "ctrl+]", ccw = "ctrl+[" }
roller = { type = "chord", cw = "ctrl+tab", ccw = "ctrl+shift+tab" }

[desk_state]                # applied via Solaar on profile activation (optional, M3)
mouse.thumb_button   = "goto_definition"
keyboard.backlight   = { mode = "static", brightness = 60 }
mouse.dpi            = 1200
```

- Action types (v1): `chord` (key combo, sequences with `,`, per-step
  delays, hold/release), `exec` (non-blocking subprocess with `KEYVO_*`
  env, §8), `http` (GET/POST), `text` (type a string), `page` / `profile` /
  `mode` (internal). Chord aliases (`ctrl`/`control`, `super`/`meta`) and
  shifted symbols (`ctrl+?`) are accepted; the full evdev keycode table is
  supported.
- Press kinds: `short`, `long`, `repeat` (hold).
- A global profile always exists; app profiles overlay it; matching priority
  is explicit, never alphabetical.
- Hot-reload with inotify (mtime poll fallback), `SIGHUP` and the socket
  `reload` command; writes are atomic.
- Desk state is applied by calling `solaar config …` (v1, optional feature,
  M3); never on Windows in plugin mode (Options+ owns the devices).

### 7.3 Options+ profile import (`.lp5`)

An Options+ profile is a ZIP of documented JSON (`ApplicationInfo.json`,
`ProfileInfo.json`, `ActionIcons/*.ict` = JSON wrapping base64 SVG plus a
text layer). `keyvo profile import <file.lp5>` converts icons, labels and
simple chords into a keyvo profile; layout-id-encoded chords and
platform-specific actions are imported best-effort and flagged. Placement:
**[OPEN]** M1 or M2 (plan §8; the plan proposes M2).

---

## 8. Scripting and control API

**Outward — key → script.** `exec` actions receive `KEYVO_KEY`,
`KEYVO_PROFILE`, `KEYVO_PAGE`, `KEYVO_WINDOW_CLASS`, `KEYVO_WINDOW_TITLE`,
`KEYVO_PRESS`, `KEYVO_SOCKET`. Exec is always non-blocking with a
configurable timeout; the daemon never blocks on user scripts or network.

**Inward — script → daemon.** Unix domain socket at
`$XDG_RUNTIME_DIR/keyvo/keyvo.sock`, mode `0600`, owned by the session
user (named pipe on Windows). Newline-delimited JSON, **versioned from day
one**: every message carries `"v": 1`. Trust model: same as `ssh-agent` —
any process running as the user may control the pad (see `SECURITY.md`).
No D-Bus façade in v1 (ADR-0003); the KWin script talks to a private D-Bus
object owned by the daemon, which is an implementation detail, not an API.

Commands (v1): `set_key`, `set_badge`, `set_state`, `push_overlay`,
`pop_overlay`, `switch_profile`, `set_mode`, `set_page`, `notify_context`
(external hook asserting the active context), `reload`, `status`,
`subscribe` (event stream: key events, profile/page/mode changes, device
connect/disconnect, focus changes). `keyvo ctl <command> [json]` is the
CLI front-end; `keyvo ctl subscribe` streams events. Wire details live in
`docs/socket-api.md`; fixtures in `tests/protocol-fixtures/` are shared by
the Rust, TypeScript and C# clients.

**Plugins.** `~/.config/keyvo/plugins/<name>/plugin.toml` declares an
executable, its arguments, restart policy and the profiles it serves. The
daemon supervises the process: JSON events on stdin, commands on stdout,
restart with back-off, log capture. Plugins are how integrations that need
state (tmux, git status, editor sessions) are built.

**No embedded logic in v1.** Lua (`mlua`) was considered and rejected for v1
(ADR-0003): plugins and `exec` cover stateful keys, and one process model is
easier to secure and test. Revisit after M2 if the plugin path proves too
heavy.

**First-class integration.** `integrations/claude-code/` is a Claude Code
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
  text → baseline JPEG via `jpeg-encoder` (quality 85–95, 4:2:0 default).
  Rendered tiles are content-hashed and cached; rendering runs off the
  device thread. A size guard rejects frames above the device limit before
  they are queued.
- Key images are rendered at the geometry the device published (§5.4);
  the full-panel path is used for splashes and previews.
- Animation = frame push; renderer supports spinner/progress/blink
  primitives. Sustainable frame rate is measured in M0-1 (plan §5 item 9)
  and documented; there is no native animated format.
- `keyvo preview` renders the grid to PNG with no hardware; `insta`
  snapshot tests cover every renderer primitive and the demo profiles.
- Accent color per profile is the primary context indicator; optional
  ~300 ms full-grid splash on profile switch (**[OPEN]**: default on/off).
- Test policy: `keyvo-core` ≥ 85 % and `keyvo-hid` ≥ 80 % line coverage,
  reported (not enforced) via `cargo-llvm-cov` → codecov;
  `FakeHidTransport` integration tests replay golden captures.

---

## 10. Desktop app (`app/`, M4a)

- Tauri 2 with a Svelte 5 frontend (TypeScript). Audience: developers only;
  the GUI exists for layout editing and for onboarding a junior developer,
  not to hide the text files.
- Talks to the daemon only via the socket API (no device access from the
  GUI, no privileged code).
- Scope (v1 GUI): 3×3 grid layout editor (drag and drop, per-page), icon
  picker, simple actions (`chord`, `page`, `profile`, `mode`, `text`,
  `http`), dashboard (device status, active profile/page/mode, overlays,
  log), first-run wizard. `exec` scripts are opened in a configurable
  external editor rather than edited in the GUI. Shortcut entry is
  text-first with capture as a helper (compositor-grabbed chords cannot be
  captured).
- Every edit goes through `keyvo-core`'s `toml_edit` round-trip; the GUI
  never rewrites a profile file wholesale.
- First-run wizard (Linux): install the udev rule via `pkexec`, install the
  KWin script, enable the systemd user unit, run `keyvo doctor`.
- Flatpak (M4b): `--device=all`, `--socket=wayland`, `--socket=fallback-x11`,
  `--talk-name=org.kde.KWin`, socket dir under `xdg-run`; the daemon runs on
  the host as a systemd user service, the Flatpak ships the GUI and CLI.

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
- Toolchain: pinned stable Rust (`rust-toolchain.toml`, edition 2024),
  clippy `-D warnings`, `cargo-deny`, `cargo-llvm-cov`, CodeQL, `cargo-dist`,
  mdBook, dependabot.
- Crates (v1): `tokio` (daemon only; `keyvo-core` is sync), `zbus`,
  `hidapi` (non-Linux) with a direct hidraw path on Linux, `evdev`
  (uinput), `x11rb`, `toml_edit`, `serde`, `schemars`, `tiny-skia`,
  `resvg`, `jpeg-encoder`, `insta` (tests), `clap`, `tracing`.

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
- Trademark notice for Logitech names; descriptive use only.
- Employer/side-project clearance obtained by the author; co-owner of the
  Qubernetic brand informed and agreed.

---

## 15. Milestones

Detailed issue lists, definitions of done and the measurement list are in
`docs/plans/2026-09-09-project-plan.md` §4–§5 and `docs/roadmap.md`.

0. **Bootstrap**: repository, docs foundation, workspace skeleton,
   devcontainer, CI, diagrams.
1. **M0 — Protocol (hardware truth)**: TS quick-test with Julusian's
   library, Windows VM + USB capture toolkit, HID++ 2.0 core, keypad
   enumeration + keep-alive, VLP display plane, key and page-button input,
   brightness, `keyvo probe|keys|fill|paint-test|doctor`,
   `docs/protocol.md`, `docs/hardware-notes.md`; Windows build green.
2. **M1 — Static pad (v0.1.0)**: TOML profiles with schema and round-trip,
   renderer + snapshots, `preview`, actions, press kinds, hot-reload,
   systemd unit, udev rule + `doctor`, cargo-dist release.
3. **M2 — Context**: KWin script + X11 watcher, auto/manual state machine,
   page buttons, overlays + `general_hint`, socket API v1 + `subscribe`,
   `keyvo ctl`, plugin supervisor, Claude Code plugin, tmux and git
   integrations, `.lp5` import (if not M1).
4. **M3 — Desk state and Dialpad**: Dialpad over Bolt, Bluetooth
   verification, Solaar-backed settings per profile, VS Code extension.
5. **M4a — GUI** · **M4b — Flathub**.
6. **M5 — Windows**: C# Actions SDK plugin, Marketplace listing; native
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
| 16 | Splash on profile switch default | **[OPEN]** (§9) |
| 17 | MX Keypad (2026) PID / firmware compatibility | **[OPEN]** until an owner publishes `lsusb` / `solaar show` |
| 18 | Is the VLP root fn2 `5e..65` registration required | → item 5 |
| 19 | Brightness unit and the meaning of value 0 | → item 11 (M0-7) |

---

## Changelog

- **v0.2 (2026-09-09)** — renamed `tessera` → `keyvo`; §1 prior art
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
- **v0.1 (draft)** — initial specification from design discussions.
