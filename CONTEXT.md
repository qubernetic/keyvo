# keyvo: domain glossary

These words mean one thing in keyvo. Code identifiers, documentation, issues,
commit messages and GUI labels use them exactly as defined here. When a new
concept appears, add it here in the same PR that introduces it. Architecture
choices behind these terms are recorded in `docs/adr/`; defaults and numbers in
`docs/spec.md`.

**Action**: What a key does when pressed: `chord`, `exec`, `launch`, `http`,
`text`, `page`, `profile`, `mode` or `recipe`. Defined per key in a profile's
TOML; executed by `keyvo-core` (internal kinds) or `keyvo-daemon` (injection,
processes, HTTP). `exec` runs a command and its exit is the result; `launch`
opens an application and lets it go once it outlives
`[timing].launch_settle`. An optional `on_fail` action runs when the primary
one fails.

**Behavior**: The GUI's view of one key binding: a key (or Dialpad or MX
control) on a page of a profile with its trigger rows, conditions, actions and
feedback. A view over the profile TOML, not a TOML entity; the key stays the
unit of truth. Spelled `behavior` in GUI labels and code.

**Bolt receiver**: Logitech's USB dongle (`046d:c548`) through which the
Dialpad, MX Master 4 and MX Mechanical connect; one hidraw node carries several
devices distinguished by a device index. `keyvo-hid` opens it for the Dialpad
(ADR-0006).

**Context source**: Whatever tells the daemon which project the focused window
belongs to: the working directory pushed by an integration with
`notify_context` (shell prompt hook, VS Code, tmux), bound to the window by
pid; or, as fallback, the window title matched against the project registry.
Otherwise the context is "unknown project" with a visible reason.

**Chord**: A key combination such as `ctrl+shift+g`, written as a string in a
profile and turned into evdev key codes by the chord parser in `keyvo-core`,
then injected through `uinput` by the daemon.

**Classic HID++ plane**: The HID++ 2.0 protocol on report ID `0x11` (and
`0x10`) that the Keypad and Dialpad share with other Logitech devices: keep-alive
(`0x0008`), page and Dialpad buttons (`0x1B04`), brightness (`0x8040`), rollers
(`0x4610`). Implemented in `keyvo-hid`; distinct from the VLP plane.

**Context page**: The page of the active profile that auto mode shows while the
matched application has focus, index 1 of its page list; overlays apply only
here. Named `context` in the profile's `[[pages]]`; a profile without one uses
its first page.

**Desk state**: The mouse and keyboard settings a profile applies on
activation (DPI, backlight, button remaps), stored under `[devices]` and applied
through Solaar when it is installed (M3, optional).

**Device actor**: The thread in `keyvo-daemon` that alone owns the Keypad's
transport: keep-alive, claim, divert, key image writes and input. It takes
the wanted key specs from the controller, writes only the keys that changed,
and reconnects when the device is gone.

**Dialpad**: The MX Creative Dialpad: four divertable buttons, a dial (180
detents per revolution) and a small roller (40 per revolution), connected over the
Bolt receiver or Bluetooth. Driven by `keyvo-hid` in M3.

**Divert**: Taking a control's presses from the device's default action to
keyvo: `setCidReporting(cid, 0x03)` on `0x1B04` for the page buttons (and the
Dialpad's buttons in M3), after which the device sends their presses as HID++
notifications instead of acting on them. Volatile; re-sent on every open and
reconnect, with the `[timing].divert_settle` and `divert_spacing` waits.

**Display mode**: The Keypad's LCD ownership state on the VLP plane, claimed by
the daemon at start so that painted images replace the Logitech logo and
released on exit. Part of the Keypad driver in `keyvo-hid`.

**Feature index**: The per-device slot number under which an HID++ feature
(such as `0x1B04`) answers; it differs between Keypad and Dialpad and is resolved
through `ROOT.getFeature` at connect time, never hard-coded.

**Feedback state**: One of the automatic key states `running`, `success` and
`error` that an `exec` or `http` action sets while it runs and when it ends,
drawn with the theme's feedback primitives unless the key's own `states`
override them. `success` reverts after `[render].feedback_hold`; `error`
stays until the next press.

**First light**: The first hardware session on the owner's Keypad (#40,
2026-10-25): `keyvo probe`, `keys` and `fill` run on the device, with the
measurements recorded in `docs/hardware-notes.md` and its frames turned into
fixtures. Findings from it are the pre-alpha slices 6c to 6i.

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

**General page**: The page shared by every profile (media keys, launchers),
index 0 of every page list, one left press from any context page; overlays
never draw on it. Named `general` in the global profile.

**Hot-plug**: Device connect and disconnect at runtime (USB replug, suspend,
Bolt sleep). The daemon watches udev, reopens the node, re-claims display mode,
re-asserts brightness and re-diverts buttons, then repaints. The pre-alpha has
no udev watch: the device actor retries every `[timing].reconnect_interval`.

**Hot-reload**: Reloading the configuration directory when a file in it
changes (inotify), after `[timing].reload_settle` of quiet. A directory that
does not load keeps the last good state.

**Input injector**: The platform trait in `keyvo-core` (`InputInjector`) that
sends a chord's key presses and releases to the desktop; on Linux a uinput
keyboard named `keyvo` in the daemon.

**Keep-alive**: The periodic `0x0008 KEEP_ALIVE` request (3000 ms requested,
sent about every second). Once a ping has set the timeout, a missing ping
lets it expire and the Keypad reverts its LCD to the logo (mode `a1`); a
client that never pings keeps key input and its paint (measured, spec §5.3).
A dedicated task in `keyvo-hid`, started before the first read.

**Key spec**: What one key shows: background color, profile accent, icon,
label, badge text and feedback look, taken from a key of a profile page and
its `states` (`KeySpec` in `keyvo_core::render`). The renderer turns it into
a tile.

**Keypad**: The MX Creative Keypad (`046d:c354`): nine LCD keys in a 3×3 grid,
numbered 1..9 row-major, plus two page buttons. keyvo's primary device.

**Mirror**: A client's copy of the daemon's state (`keyvo_client::Mirror`):
the view, device, focus context, held keys, recent trace entries and the last
reload error, filled from the `status`, `subscribe` and `press` replies and kept
current by the events. The GUI's Home screen draws the pad from it.

**Mode**: `auto` or `manual`. In auto mode the focus source selects the profile;
manual mode is a focus lock that keeps the shown profile and page list index
while focus changes are recorded, not shown. The page buttons walk the page
list in both. Toggled by a long press on a page button or by `set_mode` on the
socket.

**MX Keypad (2026)**: The Keypad-only product Logitech announced on 2026-09-08
for Windows and macOS. Assumed to share the Keypad's hardware; its PID and feature
table are unverified, so geometry and features are always queried at runtime.

**Overlay**: A transient layer pushed onto the context page by a plugin or the
socket (`push_overlay`), for example a coding agent's working/waiting status.
Overlays stack in push order and are removed with `pop_overlay`.

**Page**: One arrangement of the nine keys inside a profile. Every profile has a
`context` page; the global profile also has the `general` page; a profile may
have more.

**Page grace**: The time (default 10 s, `[timing].page_grace`) after the last
use of a page button during which an app switch keeps the page list index
instead of showing index 1, so the user's choice sticks.

**Page list**: The ordered pages the page buttons walk for the shown profile:
the global profile's pages after `general` at -1, -2, ..., the general page at
0, the shown profile's context page at 1 and its other pages from 2. It does
not wrap. The daemon reports the position as `index`.

**Plugin**: An executable under `~/.config/keyvo/plugins/<name>/` described by
`plugin.toml`, started and restarted by the daemon, speaking the NDJSON socket
protocol on stdin and stdout (ADR-0003).

**Press kind**: How a key was pressed: `short`, `long` (held past
`[timing].long_press`, default 1000) or `repeat` (held, re-fired). Each kind
can bind a different action; in the pre-alpha a key has one action and
`short` and `long` both run it (`keyvo_core::press`).

**Project layer**: The `.keyvo/` directory of a repository (`profile.toml`,
`profiles/<app-id>.toml`, `local.toml`), found by walking up from the context
source's working directory, that overrides single keys between the app profile
and the overlays (ADR-0009). Rendered at once, executed only once trusted.

**Profile**: One TOML file under `~/.config/keyvo/profiles/<id>.toml` that
describes the whole desk for one context: match rules (window class as
reverse-DNS or short name), accent colour, pages, Dialpad bindings and desk
state. The global profile always exists; app profiles layer on top of it.

**Recipe**: A named action (`type = "recipe"`, for example
`project.build`) resolved at press time from the project layer's parameters,
then from detectors (files at the project root), then from a two-step
mini-wizard whose answer is stored in `.keyvo/local.toml`. Recipes are TOML
files in the bundle, `~/.config/keyvo/recipes/` and `.keyvo/recipes/`.

**Recipe pack**: One TOML file under `contrib/recipes/` (bundled) or
`~/.config/keyvo/recipes/` that is one entry of the GUI's actions catalogue:
group, name, icon, label and its variants. Applying it writes plain fields
into a key; the profile does not reference it.

**Render coalescing**: The window (default 50 ms, `[timing].render_coalesce`)
in which several state changes are merged into one repaint of the nine keys, so
that bursts of socket messages do not flood the device.

**Simulator**: The GUI's virtual pad, rendered by the same renderer as the
device. In sandbox mode it works on a copy of the profiles that Apply merges
and Discard drops; simulated presses are dry runs on the socket, resolved and
traced but never executed.

**Snapshot**: A copy of a profile file taken under
`~/.local/state/keyvo/snapshots/` before every GUI write, rotated, restored
with `keyvo profile restore`.

**Software id (swid)**: The low nibble of byte 3 of every HID++ request,
`(function << 4) | swid`, chosen by the host (1 to 15) and echoed in the reply
so that replies can be matched to their requests; `0` marks device
notifications.

**Socket API**: The newline-delimited JSON protocol, every message tagged
`"v": 1`, served on `$XDG_RUNTIME_DIR/keyvo/keyvo.sock` (mode 0600) and spoken by
`keyvo ctl`, plugins and integrations. The only inward control surface in v1.
The message types are `keyvo_core::socket`; the wire contract is
`docs/socket-api.md`.

**Starter pack**: A profile bundle (Rust, web, Python, embedded, PLC,
AI-Assisted Coding) built from recipes and copied into the user's profiles
through the trust gate; never referenced after copying.

**Tile**: The rendered image of one key at the size the device publishes for
it, sent to the Keypad as a baseline JPEG; `keyvo preview` places the tiles
of a page on the panel and writes a PNG.

**Trace**: One record per press, real or simulated: trigger, context snapshot,
layer resolution, condition results, resolved action with substituted
variables, execution result, `on_fail` branch and block reason. Kept in a ring
buffer and in `~/.local/state/keyvo/trace.ndjson`; reachable from the GUI
drawer, `keyvo ctl trace|tail` and `subscribe`. The record type
(`TraceRecord`) and the press resolution are in `keyvo-core`, the buffer and
the file in the daemon.

**Trust**: The user's recorded decision that a project layer may run its
actions, bound in `~/.config/keyvo/trust.toml` to the content hash of its
three files and re-asked whenever they change (direnv style). Granted with
`keyvo project trust` or the GUI prompt.

**Variant**: One "What it does" choice of a recipe pack, an action of the
profile schema; the first is the default.

**View**: What the pad shows: the profile id, the page (counted from 1) and the
mode (`keyvo_core::state::View`). The state machine computes it, the daemon
paints it, and the socket reports it in `status`, `subscribe` and the
`profile`, `page` and `mode` events.

**VLP plane**: The Keypad's second protocol, on report IDs `0x13` (control,
per-fragment acknowledgements) and `0x14` (bulk), with its own root and the
display feature `0x19A1`, used to query key geometry and to upload baseline JPEG
images per key or for the whole panel. Implemented in `keyvo-hid` next to the
classic plane.

**Window watcher**: The platform trait in `keyvo-core` (`WindowWatcher`) whose
implementations in the daemon turn a focus source into `(class, title)` events
(`WindowContext`, with the window's `pid` when the source knows it) for the
state machine. The daemon binary uses the KWin watcher (`KWinWatcher`);
`FakeWindowWatcher` reports windows by hand in tests.
