# keyvo: domain glossary

These words mean one thing in keyvo. Code identifiers, documentation, issues,
commit messages and GUI labels use them exactly as defined here. When a new
concept appears, add it here in the same PR that introduces it. Architecture
choices behind these terms are recorded in `docs/adr/`; defaults and numbers in
`docs/spec.md`.

**Action**: What a key does when pressed: `chord`, `exec`, `http`, `text`,
`page`, `profile` or `mode`. Defined per key in a profile's TOML; executed by
`keyvo-core` (internal kinds) or `keyvo-daemon` (injection, processes, HTTP).

**Bolt receiver**: Logitech's USB dongle (`046d:c548`) through which the
Dialpad, MX Master 4 and MX Mechanical connect; one hidraw node carries several
devices distinguished by a device index. `keyvo-hid` opens it for the Dialpad
(ADR-0006).

**Chord**: A key combination such as `ctrl+shift+g`, written as a string in a
profile and turned into evdev key codes by the chord parser in `keyvo-core`,
then injected through `uinput` by the daemon.

**Classic HID++ plane**: The HID++ 2.0 protocol on report ID `0x11` (and
`0x10`) that the Keypad and Dialpad share with other Logitech devices: keep-alive
(`0x0008`), page and Dialpad buttons (`0x1B04`), brightness (`0x8040`), rollers
(`0x4610`). Implemented in `keyvo-hid`; distinct from the VLP plane.

**Context page**: The page of the active profile that auto mode shows while the
matched application has focus; overlays apply only here. Named `context` in the
profile's `[[pages]]`.

**Desk state**: The mouse and keyboard settings a profile applies on
activation (DPI, backlight, button remaps), stored under `[devices]` and applied
through Solaar when it is installed (M3, optional).

**Dialpad**: The MX Creative Dialpad: four divertable buttons, a dial (180
detents per revolution) and a small roller (40 per revolution), connected over the
Bolt receiver or Bluetooth. Driven by `keyvo-hid` in M3.

**Display mode**: The Keypad's LCD ownership state on the VLP plane, claimed by
the daemon at start so that painted images replace the Logitech logo and
released on exit. Part of the Keypad driver in `keyvo-hid`.

**Feature index**: The per-device slot number under which an HID++ feature
(such as `0x1B04`) answers; it differs between Keypad and Dialpad and is resolved
through `ROOT.getFeature` at connect time, never hard-coded.

**Focus hysteresis**: The delay (default 150 ms, `[timing].focus_hysteresis`)
that a focus change must survive before the state machine switches profile, so
that brief focus flickers do not repaint.

**Focus source**: Whatever tells the daemon which window is active: the KWin
script on KDE Wayland, `_NET_ACTIVE_WINDOW` on X11, or an external
`notify_context` message from a hook or plugin. External sources take precedence
over heuristics.

**General hint**: An optional, small visual cue (`general_hint` in the profile)
shown on the general page when an overlay is active on the context page, so the
user knows something is waiting without leaving the general page.

**General page**: The page shared by every profile (media keys, launchers)
reachable from auto mode with a page button; overlays never draw on it. Named
`general` in the global profile.

**Hot-plug**: Device connect and disconnect at runtime (USB replug, suspend,
Bolt sleep). The daemon watches udev, reopens the node, re-claims display mode,
re-asserts brightness and re-diverts buttons, then repaints.

**Keep-alive**: The periodic `0x0008 KEEP_ALIVE` request (3000 ms requested,
sent about every second) without which the Keypad emits no key events and
reverts its LCD to the logo. A dedicated task in `keyvo-hid`, started before the
first read.

**Keypad**: The MX Creative Keypad (`046d:c354`): nine LCD keys in a 3×3 grid,
numbered 1..9 row-major, plus two page buttons. keyvo's primary device.

**Mode**: `auto` or `manual`. In auto mode the focus source selects the profile
and page; in manual mode the user pages through the active profile with the page
buttons and focus changes are ignored. Toggled by a long press on a page button
or by `set_mode` on the socket.

**MX Keypad (2026)**: The Keypad-only product Logitech announced on 2026-09-08
for Windows and macOS. Assumed to share the Keypad's hardware; its PID and feature
table are unverified, so geometry and features are always queried at runtime.

**Overlay**: A transient layer pushed onto the context page by a plugin or the
socket (`push_overlay`), for example a coding agent's working/waiting status.
Overlays stack in push order and are removed with `pop_overlay`.

**Page**: One arrangement of the nine keys inside a profile. Every profile has a
`context` page; the global profile also has the `general` page; manual mode may
add more.

**Plugin**: An executable under `~/.config/keyvo/plugins/<name>/` described by
`plugin.toml`, started and restarted by the daemon, speaking the NDJSON socket
protocol on stdin and stdout (ADR-0003).

**Press kind**: How a key was pressed: `short`, `long` (held past
`[timing].long_press`, default 1000) or `repeat` (held, re-fired). Each kind
can bind a different action.

**Profile**: One TOML file under `~/.config/keyvo/profiles/<id>.toml` that
describes the whole desk for one context: match rules (window class as
reverse-DNS or short name), accent colour, pages, Dialpad bindings and desk
state. The global profile always exists; app profiles layer on top of it.

**Render coalescing**: The window (default 50 ms, `[timing].render_coalesce`)
in which several state changes are merged into one repaint of the nine keys, so
that bursts of socket messages do not flood the device.

**Socket API**: The newline-delimited JSON protocol, every message tagged
`"v": 1`, served on `$XDG_RUNTIME_DIR/keyvo/keyvo.sock` (mode 0600) and spoken by
`keyvo ctl`, plugins and integrations. The only inward control surface in v1.

**VLP plane**: The Keypad's second protocol, on report IDs `0x13` (control,
per-fragment acknowledgements) and `0x14` (bulk), with its own root and the
display feature `0x19A1`, used to query key geometry and to upload baseline JPEG
images per key or for the whole panel. Implemented in `keyvo-hid` next to the
classic plane.

**Window watcher**: The platform trait in `keyvo-core` (`WindowWatcher`) whose
implementations in the daemon turn a focus source into `(class, title)` events
for the state machine.
