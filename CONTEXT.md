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
`[timing].launch_settle`, and may name its window `class` and whether it
waits for that window (`wait_focus`, see Pending launch). An optional `on_fail` action runs when the primary
one fails (see on_fail).

**Blocked**: The trace result of a press that was not resolved and ran
nothing, with its reason (`Outcome::Blocked { reason }`): as built
`session locked`, a press under the session lock; later an untrusted
project's key. No look, no `on_fail`.

**Behavior**: The GUI's view of one key binding: a key (or Dialpad or MX
control) on a page of a profile with its trigger rows, conditions, actions and
feedback. A view over the profile TOML, not a TOML entity; the key stays the
unit of truth. Spelled `behavior` in GUI labels and code.

**Bolt receiver**: Logitech's USB dongle (`046d:c548`) through which the
Dialpad, MX Master 4 and MX Mechanical connect; one hidraw node carries several
devices distinguished by a device index. `keyvo-hid` opens it for the Dialpad
(ADR-0006): the receiver's node is the `046d:c548` node that carries the HID++
reports `0x10` and `0x11` (`keyvo_hid::sysfs::find_receiver`), shared with the
kernel's `hid-logitech-dj` and Solaar. Short: the receiver. See Slot.

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

**Control**: One of the Dialpad's six inputs as a binding names it: `dial`,
`roller`, `button:1` to `button:4` (`keyvo_core::dialpad::Control`). A
profile's `[dialpad]` table binds them; a control the app profile leaves
out comes from the global profile's table. Trace records, `key` events and
socket `press` requests name a control where a Keypad key has its number.

**Context page**: The page of the active profile that auto mode shows while the
matched application has focus, index 1 of its page list; overlays apply only
here. Named `context` in the profile's `[[pages]]`; a profile without one uses
its first enabled page (#156).

**Desk state**: The mouse and keyboard settings a profile applies on
activation (DPI, backlight, button remaps), stored under `[devices]` and applied
through Solaar when it is installed (M3, optional).

**Detent**: One increment of a Dialpad roller as the device reports it: 180
a rotation on the dial, 40 on the roller; neither has a ratchet, so a
detent is not felt. A turn's delta counts as that many detents (a fast dial
turn's accelerated delta too); a binding's `step` detents run its action
once, the rest carries to the next turn in the same direction.

**Device actor**: The thread in `keyvo-daemon` that alone owns the Keypad's
transport: keep-alive, claim, divert, key image writes and input. It takes
the wanted key specs from the controller, writes only the keys that changed,
and reconnects when the device is gone. The Dialpad has an actor of its own
(`keyvo_daemon::dialpad`, #188) on its own thread and transport, so either
device may be absent.

**Dialpad**: The MX Creative Dialpad: four divertable buttons, a dial (180
detents per revolution) and a small roller (40 per revolution), connected over the
Bolt receiver or Bluetooth. Driven by `keyvo_hid::dialpad` (beta, #186):
found on its slot, its buttons and rollers diverted, its turns read as
rotations; `keyvo dial` prints them, the daemon runs a profile's
`[dialpad]` bindings on them (#188). Buttons 1 to 4 are its four CIDs in
order; roller 1 is the dial and roller 0 the roller, + is clockwise
(measured, #187).

**Divert**: Taking a control's presses from the device's default action to
keyvo: `setCidReporting(cid, 0x03)` on `0x1B04` for the page buttons and the
Dialpad's buttons, `setMode(roller, 1)` on `0x4610` for the Dialpad's
rollers, after which the device sends their presses and turns as HID++
notifications instead of acting on them. Volatile; re-sent on every open and
reconnect, with the `[timing].divert_settle` and `divert_spacing` waits.

**Display mode**: The Keypad's LCD ownership state on the VLP plane, claimed by
the daemon at start so that painted images replace the Logitech logo and
released on exit. Part of the Keypad driver in `keyvo-hid`.

**Doctor**: `keyvo doctor`, the CLI's nine checks of the setup (device
access, the udev rule, the session, KWin's focus stealing prevention, the
user unit, the daemon, Solaar, the Flatpak app), one line each with `ok`,
`warn` or `fail` and the fix; it exits 1 on a `fail` and changes nothing.

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

**Modifier key**: A key with a `dial` or `roller` table (#189): while it is
the *active modifier* the Dialpad's dial and roller run its bindings instead
of the profile's `[dialpad]` ones, and the key shows the `modifier` look (a
ring in the accent and a dial badge). A *held* one (`modifier = "held"`, the
default) is active while it is held and runs its action at a release without
a turn; a *toggle* key (`modifier = "toggle"`) is active from a short press
until the toggle ends. The last pressed held key wins over an armed toggle.
The view and the socket name it as `modifier: { key, mode }`.

**Mode**: `auto` or `manual`. In auto mode the focus source selects the profile;
manual mode is a focus lock that keeps the shown profile and page list index
while focus changes are recorded, not shown. The page buttons walk the page
list in both. Toggled by a long press on a page button or by `set_mode` on the
socket.

**MX Keypad (2026)**: The Keypad-only product Logitech announced on 2026-09-08
for Windows and macOS. Assumed to share the Keypad's hardware; its PID and feature
table are unverified, so geometry and features are always queried at runtime.

**on_fail**: A key's second action, next to `action`, run once when the
key's action fails: a non-zero exit, a timeout, a command that does not
start, a chord not sent. Any `chord`, `exec` or `launch`, with the key's
`KEYVO_*` variables and `KEYVO_FAILED_CODE`; never retried, no `on_fail` of
its own, not run on a block, a dry run or an unbound key. The key keeps its
`error` look; the run is a second trace record of the press on the
`on_fail` branch. The GUI shows it read-only.

**Overlay**: A transient layer pushed onto the context page by a plugin or the
socket (`push_overlay`), for example a coding agent's working/waiting status.
Overlays stack in push order and are removed with `pop_overlay`.

**Page**: One arrangement of the nine keys inside a profile. Every profile has a
`context` page; the global profile also has the `general` page; a profile may
have more.

**Page list**: The ordered pages the page buttons walk for the shown profile:
the global profile's pages after `general` at -1, -2, ..., the general page at
0, the shown profile's context page at 1 and its other pages from 2. It does
not wrap. The daemon reports the position as `index`. A page with
`enabled = false` is left out (no index, its keys kept in the file); the
context page is in the list either way. An app profile with
`global_pages = false` has no global pages: its list starts at 1 (#156).

**Page return**: In auto mode the pad goes back to index 1 of the page list by
itself after a time without a page-button or key press: after
`[timing].page_return_global` (default 10 s) on a global page (index 0 and
below), after `[timing].page_return_own` (default 0, off) on the shown
profile's further pages (index 2 and above). Replaced the page grace, which
kept the index across an app switch (#157). A Dialpad button press restarts
the clock like a key press; a turn does not (#188).

**Pending launch**: A press of a `launch` key with `wait_focus` on, waiting up
to `[timing].launch_focus` (default 15 s) for its window. An app switch whose
window descends from the launch's process, carries the launch's class, or
comes while a launch without a class waits, consumes it and keeps a global
page instead of showing the context page; the page return starts over at the
window's arrival. A failed launch drops it (#179).

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

**Rotation**: One turn notification of a diverted Dialpad roller (`0x4610`
event 0): the roller id and a signed delta in increments (`i8`), plus an
optional device timestamp keyvo ignores. `keyvo_hid::input` reports it as
`InputEvent::Rotate { roller, delta }`; the delta is passed on as the device
sends it.

**Session lock**: The user's session locked by the screen locker, as
systemd-logind reports it (`LockedHint`, `Lock`, `Unlock`) through the
`SessionWatcher` platform trait (`keyvo_core::session`; the daemon's
`LogindSessionWatcher`, `FakeSessionWatcher` in tests). A veil over the
state, not a mode: the view keeps its profile, page and mode and says
`locked`, the pad shows the lock page, every press is *blocked* and the
page buttons do nothing; the socket reports it as a `session` event.

**Slot**: One paired device's place on the Bolt receiver, addressed by its
device index 1 to 6. keyvo finds the Dialpad's slot at runtime, never as a
constant: `getFeature(0x4610)` per slot within `[timing].slot_probe`, then
the model id. A slot is empty (the receiver's HID++ 1.0 error report, `0x09`
on the owner's receiver), silent (no answer or connect fail: asleep or out of
range, asked again on its arrival notification or in the next search round),
another device, or the Dialpad's.

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
notifications. A connection keeps one; the Dialpad's slot search tags each of
its requests with an id of its own, so a reply the receiver repeats late
answers nothing.

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

**Toggle**: A toggle modifier key armed by a short press (#189). It ends
at the next press of the same key, of any other key or Dialpad button (which
otherwise acts as usual), at a page change, a profile switch, a reload that
drops the key, the lock, or after `[timing].modifier_timeout` without a turn
(default 0, off). One toggle is armed at a time.

**Trace**: One record per press, real or simulated: trigger, context snapshot,
layer resolution, condition results, resolved action with substituted
variables, execution result, `on_fail` branch and block reason; a key's
`on_fail` run is a second record of the same press (`branch: "on_fail"`); a
Dialpad control's record names the `control` instead of the key, a turn a
modifier key took both and its `modifier` mode. Kept in a ring
buffer and in `~/.local/state/keyvo/trace.ndjson`; reachable from the GUI
drawer, `keyvo ctl trace|tail` and `subscribe`. The record type
(`TraceRecord`) and the press resolution are in `keyvo-core`, the buffer and
the file in the daemon.

**Trust**: The user's recorded decision that a project layer may run its
actions, bound in `~/.config/keyvo/trust.toml` to the content hash of its
three files and re-asked whenever they change (direnv style). Granted with
`keyvo project trust` or the GUI prompt.

**udev rule**: `contrib/udev/70-keyvo.rules`, installed into
`/etc/udev/rules.d/`: it tags the Keypad's and the Bolt receiver's hidraw
nodes and `/dev/uinput` `uaccess`, so the user at the seat gets an ACL on
them; no mode, no group.

**User unit**: `contrib/systemd/keyvo-daemon.service`, the systemd user
unit that runs the daemon from `~/.local/bin` with the graphical session,
restarts it on failure and stops the daemon alone (`KillMode=process`), so
launched applications survive.

**Variant**: One "What it does" choice of a recipe pack, an action of the
profile schema; the first is the default.

**View**: What the pad shows: the profile id, the page (counted from 1) and the
mode (`keyvo_core::state::View`), with its page-list `index` and whether the
session is `locked`, and the active `modifier` key. The state machine
computes it, the daemon paints it, and the socket reports it in `status`,
`subscribe` and the `profile`, `page` and `mode` events, a lock or unlock as a
`session` event, a modifier change as a `modifier` event.

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
