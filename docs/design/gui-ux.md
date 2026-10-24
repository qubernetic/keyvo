# keyvo GUI and UX design

> Design document for the desktop app (`app/`, M4a) and the UX decisions that
> reach into M1, M2 and M3. Derived from the grill decisions in
> `docs/plans/2026-10-24-gui-ux-grill-decisions.md` (principles P1 to P4,
> decisions d1 to d18). Every section opens with the decisions it implements;
> details that the decisions file does not fix are marked *design detail*.
> Terms are those of `CONTEXT.md`. Protocol and file formats live in
> `docs/spec.md`; this document says what the screens show and how the flows
> behave. Status: approved with Issue #29.

## 1. Mental model: the device mirror

*Implements d1, d3; P2, P3.*

The app is a live copy of the pad. Home shows the nine keys exactly as the
Keypad shows them: the GUI subscribes to the daemon (`subscribe`), receives the
same render inputs, and runs the same renderer as the daemon, so a tile on screen
is bit-identical to the tile on the LCD. The user never has to guess whether the
GUI and the device agree.

Editing happens on the same mirror. Selecting a key on Home or on a Profiles
page opens its editor; the preview in the right column is the mirror again, not
a second drawing. There is no separate "design canvas".

Three sources can feed the mirror:

| Source | When | Writes go to |
|---|---|---|
| Connected device through the daemon | default when the daemon reports a device | the real profile files, immediate apply (§4.1) |
| Simulator, sandboxed | the user flips the sandbox switch | a working copy; Apply merges, Discard drops (§4.2) |
| Simulator, direct | no device connected at start, or no daemon | the real profile files |

The header shows which source is live. The sandbox switch is on Home and in the
Profiles editor; it is not a mode the user can forget, because the header turns
to the sandbox colour token and the Apply and Discard pair stays visible until
one of them is pressed.

## 2. Shell and navigation

*Implements d8, d13; P1, P3. Layout reference: `docs/ui-concepts/v3/`, third
image (see `docs/ui-concepts/README.md`); the sketch is a reference for the
column arrangement only.*

```
+---------------------------------------------------------------------------+
| keyvo                     [ Device selector v ]   [sandbox]   [...] - [] x |
+-----------+---------------------------+---------------------------------+
| Home      | List column               | Editor                          |
| Profiles  |   (profiles, behaviors,   |   title, enabled switch         |
| Behaviors |    projects, ...)         |   Trigger (one row per kind)    |
| Projects  |                           |   Conditions                    |
| Integr.   |                           |   Actions                       |
| Devices   |                           |   Feedback                      |
| Settings  |                           |   Test this behavior            |
|           |                           +---------------------------------+
|           |                           | Keypad preview  [page v]        |
|           |                           |   3 x 3 mirror                  |
|           |                           | Selected key card               |
|           |                           | (Dialpad panel, collapsible)    |
+-----------+---------------------------+---------------------------------+
```

- **Navigation rail** (left): Home, Profiles, Behaviors, Projects, Integrations,
  Devices, Settings, in that order, `Ctrl+1` to `Ctrl+7`. Icons with labels; the
  rail collapses to icons below 1100 px width (*design detail*). The rail shows
  no device status: that lives in the header selector (P3).
- **Header**: app name, device selector (every device the daemon reports plus
  "Simulator"), the sandbox indicator when active, overflow menu (Export,
  Import, Open config folder, Re-run setup step, About). Changing the selected
  device changes which device the preview and the key card focus; it never hides
  a panel (d13).
- **List column**: the screen's primary list (profiles, behaviors, projects,
  integrations, devices). Searchable where the list can grow (Behaviors,
  Projects). Collapsible.
- **Editor**: the selected item's form. Sections are stacked cards: Trigger,
  Conditions, Actions, Feedback, then "Test this behavior" at the bottom. No
  global simulation dock exists; simulation is always attached to the thing
  being tested.
- **Right column**: Keypad preview with a page selector, the selected key card
  (position, page, layer origin: global, app profile, project layer or overlay),
  and the collapsible Dialpad panel (§3.3). The card shows only what the editor
  does not already show (P3).
- **Window**: default 1200 x 760, minimum 960 x 640. Columns collapse in the
  order right column, list column. Modals are used only for confirmations
  (Discard sandbox, Delete profile, Restore snapshot) and the trust prompt
  (§4.5).
- **Hint bar**: one dismissable bar on Home after first run (§4.4); no tutorial
  overlay anywhere (P2).

## 3. Screens

Each screen is described as: what the list column holds, what the editor
holds, what is special in the right column.

### 3.1 Home

*Implements d1, d9, d11; P4.*

- No list column. The mirror fills the centre: the live 3 x 3 grid of the active
  profile and page, the page selector, the accent bar of the active profile.
- Status strip above the grid, one fact each: device, active profile, page,
  mode, top overlay, context (project name or "unknown project" with the reason,
  d7). Each fact is a link to the screen that owns it.
- Event list below the grid: the most recent trace entries (§8), one line each,
  newest first; clicking opens the trace drawer.
- Debug log toggle in the event list header (Debug mode adds HID-level events).
- Sandbox switch and the Apply and Discard pair when the sandbox is active.
- Hint bar (dismissable) after first run.

### 3.2 Profiles

*Implements d1, d2, d14; ADR-0004.*

- List column: profiles by priority, the global profile pinned at the top, the
  active one marked. "+" creates a profile from a template (empty, copy of the
  active, starter pack).
- Editor: profile header (id, name, accent, priority, enabled), match rules as
  editable rows (class, title regex), pages as tabs, and the key grid for the
  selected page. Clicking a key selects it; `Enter` opens the key's behavior
  in the Behaviors editor with the Profiles screen remembered for `Esc`.
- Drag and drop between keys moves a key definition; the keyboard pair is
  `Ctrl+X` on a key followed by `Ctrl+V` on another (d15).
- Every change writes through `toml_edit` immediately (§4.1). The editor shows
  the file path and a "Open in editor" button for the whole file.
- Right column: the mirror of the selected page; the key card shows position,
  page and layer origin.

### 3.3 Behaviors

*Implements d4, d9, d12, d13, d16.*

"Behavior" is the GUI's word for one key binding: a key (or Dialpad control, or
MX device control) on a page of a profile, with its triggers, conditions, actions
and feedback. It is a view over the profile TOML, not a new entity; the key
stays the unit of truth.

- List column: a flat, searchable list of every binding across all profiles,
  grouped by profile, each row with icon, label, profile and position. Filters:
  profile, device, action type. "+" opens the template picker: templates are
  copied into the profile, never referenced.
- Editor, stacked cards:
  - **Header**: icon picker, label, colour, enabled switch, the key position and
    page (read-only here; changed by drag on Profiles).
  - **Trigger**: one row per press kind: `short`, `long`, `repeat`; for Dialpad
    and MX controls the rows are the control's kinds (`cw`, `ccw`, `press`).
    Each row shows the bound action summary and opens that action's card.
  - **Conditions**: the match rules that gate this binding beyond the profile's
    own match: active application (inherited from the profile, shown read-only),
    project trust state, key state (`states` name). Conditions are stored as
    the TOML fields that already exist; the card is a view.
  - **Actions**: the primary action for the selected trigger row (type selector:
    `chord`, `exec`, `http`, `text`, `page`, `profile`, `mode`, `recipe`), its
    fields, and the optional `on_fail` action. For `recipe` the card shows the
    resolved concrete action for the current project (d10) and "Expand to a
    concrete action". For `exec` the script path has "Open in editor"; the GUI
    never embeds a code editor.
  - **Feedback**: desktop notification (off, on error, always; default on
    error), state overrides for `running`, `success`, `error` (colour, badge),
    `long_run_threshold`. No sound toggle in v1.
  - **Test this behavior**: Simulate button with a trigger kind selector (short,
    long, hold), a one-line result ("would run `just build` in
    `/home/u/proj`, 0.3 s") and an expandable "would run" block with the
    substituted command, environment and the condition results. Simulated
    presses are dry runs on the socket; nothing executes.
- Right column: the mirror with the key highlighted; the Dialpad panel when the
  behavior is a Dialpad binding.

### 3.4 Projects

*Implements d5, d6, d7.*

- List column: known project roots from the trust store and the project
  registry (trusted, untrusted, recently seen), the current one marked.
- Editor: root path, trust state with the content hash date, the three files
  (`.keyvo/profile.toml`, `.keyvo/profiles/<app-id>.toml`, `.keyvo/local.toml`)
  with "Open in editor" each, the keys the project layer overrides (as a grid
  diff: added, overridden, cleared), the recipe parameters the project sets, and
  the actions that the layer would run (the same list the trust prompt shows).
  Buttons: Trust, Revoke trust, Open in file manager.
- Right column: the mirror with project-layer keys outlined in the project
  layer token colour.

### 3.5 Integrations

*Implements d7, d11, d17.*

- List column: the integrations keyvo ships: shell prompt hook (bash, zsh,
  fish), VS Code, tmux, git, Claude Code, and the installed plugins.
- Editor: state (installed, not installed, outdated), the exact command or file
  snippet to install it (copyable), "Install" where keyvo can do it itself (a
  file under the user's config), the last event received from it (from the
  trace), and for plugins the supervisor state (running, restarting, stopped),
  restart button and log tail.

### 3.6 Devices

*Implements d13, d16; P4.*

- List column: every device the daemon reports: Keypad, Dialpad, MX Master 4,
  MX Mechanical, the Bolt receiver; state (connected, sleeping, missing).
- Editor: identifiers (VID, PID, hidraw node, firmware), brightness slider
  (Keypad), keep-alive state, the divertable controls (MX devices) with their
  current divert state, `doctor` findings for this device with one button per
  fix, and the debug log (HID-level events when Debug mode is on).

### 3.7 Settings

*Implements d2, d11, d14, d15; P1.*

- No list column; grouped form: Appearance (theme: native, or an opt-in built-in
  theme; follow system font size; reduced motion follows the system), Editor
  (external editor command), Timing (every `[timing]` key with its documented
  default), Render (`feedback_hold`, splash), Project layer (`enabled`,
  position, allowed keys, action types, apps, trust default), Snapshots
  (directory, count, "Restore" opening the snapshot list), Setup (re-run any of
  the four first-run steps), Trace (ring size, file rotation, debug mode).
- Every field maps to a `config.toml` key and shows it.

## 4. Flows

### 4.1 Edit and apply

*Implements d2, d14; ADR-0004.*

1. The user changes a field.
2. The GUI takes a snapshot of the file into
   `~/.local/state/keyvo/snapshots/<timestamp>-<file>` (rotated, default 50
   per file, *design detail*).
3. The GUI writes the change through `toml_edit` (atomic rename).
4. The daemon's inotify watcher reloads; the pad repaints; the mirror repaints
   from the daemon's event.
5. The undo stack records the `toml_edit` diff; `Ctrl+Z` and `Ctrl+Shift+Z`
   replay it through the same write path. Undo is per GUI session.

External edits: the GUI watches the profile files too. On an external change
it reloads the form; if the user has an unsaved field focused, the GUI keeps
the field value, shows "file changed on disk" inline and applies the field
on blur as a new write.

Schema error on write: the daemon keeps the last good state; the GUI marks the
form with file and line and offers "Open in editor" at that line. The GUI never
rewrites the file wholesale to "fix" it.

### 4.2 Simulate

*Implements d3, d9; P4.*

- **Sandbox**: the sandbox switch copies the edited profiles into
  `~/.local/state/keyvo/sandbox/`; the simulator renders from the copy; edits
  go to the copy. Apply merges the copy into the real files through `toml_edit`
  (one snapshot per file first); Discard deletes the copy. Closing the app with
  a live sandbox asks Apply, Discard or Keep (the sandbox survives restarts).
- **Virtual pad**: clicks are presses; holding or right-clicking is a long
  press; the Dialpad panel has buttons for `cw`, `ccw` and press.
- **Dry run**: a simulated press is sent as a socket request with
  `"dry_run": true`; the daemon resolves layers, conditions and the action,
  substitutes variables, writes a trace entry marked `simulated` and returns
  it; no `exec`, `chord`, `http` or `text` executes.
- **Result line**: "Test this behavior" shows the trace entry's summary and
  expands to the full entry.
- Without a daemon (no device, no service), the GUI runs the simulator in
  process against `keyvo-core` through the Tauri backend; trace entries then
  stay in the GUI only.

### 4.3 Recipe mini-wizard

*Implements d10.*

When a `recipe` action cannot be resolved for the current project (no project
layer parameter and no detector match), the first press shows a two-step
prompt in the GUI (or a one-line hint on the pad and a `doctor` entry when the
GUI is closed):

1. "Which tool builds this project?" with the detector candidates first and
   "configure manually" last.
2. The command line to run, prefilled from the chosen tool.

The answer is written to `.keyvo/local.toml` under `[recipes.<id>]`; the next
press resolves without the prompt. "Expand to a concrete action" rewrites the
key's action to plain `exec` with the resolved command.

### 4.4 First run

*Implements d11; P2.*

`keyvo setup` owns the flow (M1); the GUI wizard drives it over the socket
(`setup` commands) and shows the same four steps. Each step can be re-run from
Settings.

1. **Device and system**: `doctor` results as a list; each failing item has one
   button with its own approval (`pkexec` for the udev rule, `systemctl --user`
   for the unit, the KWin script install). Nothing runs without a click.
2. **Starter pack**: pick a pack (Rust, web, Python, embedded, PLC, AI-Assisted
   Coding); the simulator previews it; the pack is copied through the trust
   gate.
3. **Integrations**: each integration with its install command shown; all
   skippable.
4. **Review and Finish**: the file list with the backups that will be taken,
   the system commands that will run, then Finish applies atomically: write
   files, validate against the schema, reload the daemon, paint a smoke frame
   on the pad. A failure rolls back the files from the backups and shows the
   failing step.

### 4.5 Trust prompt

*Implements d6; ADR-0009.*

- Trigger: the daemon sees a project layer whose content hash is not in
  `~/.config/keyvo/trust.toml` (new project, or changed files).
- The GUI shows a modal: project root, the three files with "Open in editor",
  the list of actions the layer would run (type and command, one row each),
  the keys it overrides, and three buttons: Trust, Not now, Never for this
  project. `Not now` keeps rendering the layer with every project-layer key
  dimmed and badged; a press on such a key does nothing but open the trace with
  the block reason.
- CLI equivalent: `keyvo project trust [<root>]` prints the same list and asks.

### 4.6 Export and import

*Implements d17.*

`keyvo profile export <id|all> <file.zip>` and `keyvo profile import
<file.zip>` (M1) bundle TOML, referenced icons and the theme file. The GUI
exposes them in the header menu; import goes through the trust gate when the
bundle contains `exec`, `http` or `recipe` actions.

## 5. States

### 5.1 Key feedback

*Implements d12.*

| State | Look | Enters | Leaves |
|---|---|---|---|
| `running` | icon dimmed, accent arc (spinner primitive); after `long_run_threshold` the arc becomes a static badge | `exec` or `http` started | result known |
| `success` | green badge, one flash | exit code 0 or 2xx | after `[render].feedback_hold` (2 s) |
| `error` | red badge, stays | non-zero exit, timeout, non-2xx, `on_fail` taken | next press of the key, or the GUI inspects the trace entry |

The built-in looks are in the theme file; a key's own `states.running`,
`states.success`, `states.error` override them. A script may set a custom state
by printing `KEYVO_STATE=<name>` on stdout.

### 5.2 Screen states

*Design detail.*

- **Empty**: a list with nothing in it shows one sentence and the one action
  that fills it ("No projects trusted yet. Open a project folder in your
  terminal with the shell hook installed.").
- **Loading**: skeleton rows, no spinner on the whole screen; the mirror shows
  the last frame it had.
- **Daemon unreachable**: header badge, the mirror switches to the in-process
  simulator (§4.2), Devices shows the `doctor` entry for the service.
- **Device missing**: the device selector shows "Simulator"; Home says which
  device is missing and the last time it was seen.
- **Schema error in a file**: the profile row and the Home status strip show the
  error with file and line; the editor is read-only for that file until the
  file parses again.

## 6. Keyboard map and accessibility

*Implements d15; P1.*

| Keys | Action |
|---|---|
| `Ctrl+1` to `Ctrl+7` | rail screens in order |
| `Ctrl+K` | command palette (screens, profiles, behaviors, projects, actions such as "Trust current project") |
| `1` to `9`, arrows | select a key on the focused grid |
| `Enter` | edit the selected key |
| `Space` | simulate a short press on the selected key; `Shift+Space` long |
| `Ctrl+X`, `Ctrl+V` | move a key definition (keyboard pair for drag and drop) |
| `Ctrl+Z`, `Ctrl+Shift+Z` | undo, redo (per GUI session) |
| `Ctrl+E` | open the selected item's file in the external editor |
| `Esc` | back to the remembered screen, close drawer or modal |
| `F1` | shortcut list |

- Full keyboard traversal in visual order; focus ring from the theme tokens.
- `aria-label` on every control; the grid is an `aria-grid` with row and
  column; the mirror tiles announce label, action summary and state.
- Minimum target 32 x 32 px; the rail and the Dialpad panel buttons are 40 px.
- System font size and the high-contrast scheme arrive through the theme bridge
  (§7); every token pair used for text passes a 4.5:1 contrast check in the
  test suite.
- Reduced motion from the system disables the flash and the arc animation in
  the GUI (the pad keeps its own feedback).
- English only in v1; every string goes through the i18n layer from the start.

## 7. Theme bridge

*Implements P1; d15, d18.*

The Rust side of the app (Tauri backend) reads the desktop theme and exposes
it to the frontend as CSS custom properties on `:root`, updated live when the
desktop changes.

| Token | Meaning | KDE source | GNOME source | Windows source (M5) |
|---|---|---|---|---|
| `--kv-bg`, `--kv-bg-alt` | window and panel background | `kdeglobals` `[Colors:Window]` | `gsettings` `color-scheme` plus the libadwaita defaults | `UISettings` background |
| `--kv-fg`, `--kv-fg-muted` | text | `[Colors:Window] ForegroundNormal`, `ForegroundInactive` | libadwaita | `UISettings` foreground |
| `--kv-accent` | accent | `[General] AccentColor` | `accent-color` (GNOME 47+) | `UISettings` accent |
| `--kv-sandbox` | sandbox indicator | derived (accent rotated, *design detail*) | derived | derived |
| `--kv-project` | project-layer outline | derived | derived | derived |
| `--kv-state-running`, `-success`, `-error` | feedback states, same as the pad theme | theme file | theme file | theme file |
| `--kv-font`, `--kv-font-size` | UI font and size | `[General] font` | `interface-font-name`, `text-scaling-factor` | `UISettings` text scale |
| `--kv-radius` | corner radius | fixed per desktop (*design detail*) | | |
| `--kv-contrast` | `normal` or `high` | `[General] ColorScheme` name contains "HighContrast", or `kdeglobals` `[KDE] contrast` | `high-contrast` | `UISettings` high contrast |
| `--kv-motion` | `full` or `reduced` | `kdeglobals` `[KDE] AnimationDurationFactor` = 0 | `enable-animations` | `UISettings` animations |

- Sources are read once at start and then followed: KDE through the
  `kdeglobals` file watcher and the `org.kde.KGlobalSettings` D-Bus signal,
  GNOME through `gsettings` monitors, Windows through `UISettings` events.
- Built-in themes (for example a dark developer look) are TOML files that set
  the same tokens; selecting one in Settings stops following the desktop for the
  tokens the theme sets. Native is the default and the only theme the first-run
  flow offers.
- The pad's theme file is separate (it renders tiles); the GUI reads it only for
  the feedback-state tokens so that the mirror and the screens agree.

## 8. Trace drawer and debug log

*Implements d9; P4.*

One trace entry per press, real or simulated. The drawer opens from the Home
event list, from the "Test this behavior" result line, and from a key's error
badge. It renders the entry as sections in resolution order:

1. Trigger: timestamp, device, key or control, press kind, real or simulated.
2. Context snapshot: window class and title, project root and name, git branch,
   trust state, mode, page, overlay stack.
3. Layer resolution: which layer supplied the binding (global, app profile,
   project layer, overlay) and which layers were shadowed.
4. Conditions: each condition with its result.
5. Resolved action: type, every variable substituted, the recipe id and the
   detector that resolved it when applicable.
6. Execution result: `exec` exit code, duration, bounded stdout and stderr tail
   (default 2 KiB each, *design detail*); `http` status and body excerpt;
   `chord` event sequence; `on_fail` branch taken; block reason (untrusted
   project, disabled, dry run).

The same entries stream on the socket (`subscribe` with the `trace` event
class), print with `keyvo ctl trace` (last N) and `keyvo ctl tail` (follow),
and persist to `~/.local/state/keyvo/trace.ndjson` (rotated). Debug mode adds
HID-level events (reports in and out, ACKs) to the Devices debug log, not to the
trace entries.

`keyvo exec-check <script>` runs a script with the environment a key would give
it and prints a trace-shaped result; the GUI offers it on the `exec` action
card as "Run with key environment".

## 9. Socket surface used by the GUI

*Design detail; the protocol is specified in `docs/spec.md` §8.*

| Direction | Message | Used for |
|---|---|---|
| in | `subscribe` (`events`: key, profile, page, mode, device, context, trace, config) | the mirror, status strip, event list |
| in | `status` | first paint, reconnect |
| in | `press` with `dry_run: true` | Test this behavior, virtual pad |
| in | `set_key`, `push_overlay`, `pop_overlay` | reference plugins only; the GUI does not call them for editing |
| in | `notify_context` | the GUI never sends it; shown in Integrations when received |
| in | `reload` | after a sandbox Apply (belt and braces; inotify already fires) |
| in | `setup` (`step`, `action`) | the first-run wizard driving `keyvo setup` |
| in | `project` (`trust`, `revoke`, `list`) | Projects screen and the trust prompt |
| in | `trace` (`last`, `follow`) | the drawer |
| in | `doctor` | Devices, Settings, the wizard |
| out (events) | `trust_requested` | opens the trust prompt |
| out (events) | `recipe_unresolved` | opens the mini-wizard |

All GUI edits go through the file system, never through `set_key`, so that a
GUI edit and a vim edit are indistinguishable (ADR-0004).

## 10. Windows differences

*Implements d18.*

The same Tauri app runs on Windows behind the C# plugin's named pipe. The
differences are confined to: the theme bridge sources (§7), `doctor` checks
(service and plugin state instead of udev and systemd), the first-run step 1
actions, and the missing features: no shell-hook project layer, no MX device
triggers, no desk state. The simulator works without a device, so the app is
usable before the plugin is installed.

## 11. Out of scope for v1

*Implements d17.*

Never: XP, streaks, achievements, any gamification. v2: workflow insights
(local, opt-out, derived from the trace), referenced `[[behaviors]]`, `keyvo
pack add <git-url>`, plugin sandboxing, system widgets. Not a GUI feature:
marketplace. Unchanged: `.lp5` import (spec §7.3).

## 12. Coverage of the grill decisions

| Decision | Sections |
|---|---|
| P1 native theme | 2, 6, 7 |
| P2 guided | 2 (hint bar), 4.4, 5.2 |
| P3 no redundancy | 2 (rail, header, key card) |
| P4 debuggable | 3.1, 3.6, 4.2, 8 |
| d1 device mirror | 1, 3.1 |
| d2 immediate apply | 3.2, 4.1 |
| d3 simulator | 1, 4.2, 5.2 |
| d4 key is the unit, behavior view | 3.3 |
| d5 project layer | 3.4, 4.5 |
| d6 trust | 3.4, 4.5, 4.6 |
| d7 context source | 3.1, 3.4, 3.5 |
| d8 shell | 2 |
| d9 unified trace | 3.1, 3.3, 4.2, 8 |
| d10 recipes | 3.3, 4.3 |
| d11 first run | 3.7, 4.4 |
| d12 key feedback | 3.3, 5.1 |
| d13 Dialpad panel | 2, 3.3, 3.6 |
| d14 concurrent edits and undo | 3.2, 3.7, 4.1 |
| d15 keyboard and accessibility | 3.2, 6, 7 |
| d16 MX devices as triggers | 3.3, 3.6 |
| d17 scope closures | 3.5, 4.6, 11 |
| d18 Windows | 7, 10 |
