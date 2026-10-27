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

The app is a live copy of the pad. The Pad screen shows the nine keys exactly as the
Keypad shows them: the GUI subscribes to the daemon (`subscribe`), receives the
same render inputs, and runs the same renderer as the daemon, so a tile on screen
is bit-identical to the tile on the LCD. The user never has to guess whether the
GUI and the device agree.

Editing happens on the same mirror. Selecting a key on the Pad opens its
inspector under the drawing; the page thumbnails of Profiles are the mirror
again, not a second drawing. There is no separate "design canvas".

Three sources can feed the mirror:

| Source | When | Writes go to |
|---|---|---|
| Connected device through the daemon | default when the daemon reports a device | the real profile files, immediate apply (§4.1) |
| Simulator, sandboxed | the user flips the sandbox switch | a working copy; Apply merges, Discard drops (§4.2) |
| Simulator, direct | no device connected at start, or no daemon | the real profile files |

The header shows which source is live. The sandbox switch is in the header on
every screen; it is not a mode the user can forget, because the header turns
to the sandbox colour token and the Apply and Discard pair stays visible until
one of them is pressed.

## 2. Shell and navigation

*Implements d8, d13; P1, P3. Layout idiom: the Elgato Stream Deck desktop
app adapted to keyvo's content (owner's decision of 2026-10-26,
`docs/research/2026-10-26-keypad-app-layouts.md`,
`docs/plans/2026-10-26-pad-screen-brief.md`); its visual style stays an
anti-reference. The sketch of `docs/ui-concepts/v3/` is superseded.*

```
+---------------------------------------------------------------------------+
| keyvo   [ Device v ]  [ Profile v ]   [sandbox] Apply Discard   (gear) [...]|
+-----------+------------------------------------------------+--------------+
| Pad       |   device drawing with the live mirror in it    | Actions      |
| Profiles  |   Pages [context] [git] [+]      Accent [sw]   |  [search]    |
| Behaviors |   The pad moved to vscode, page 1   (Follow)   |  v General   |
|           |------------------------------------------------|  v System    |
|           |   Key 5: Exec  file line 42    [Press] [Clear] |  v Apps      |
|           |   [tile] Label, Icon, Colour; Action, fields   |              |
|           |------------------------------------------------|              |
|           |   Trace v  newest entries, one line            |              |
+-----------+------------------------------------------------+--------------+
```

- **Navigation rail** (left): Pad, Profiles, Behaviors, in that order,
  `Ctrl+1` to `Ctrl+3`; the product map, nothing else. Behaviors is
  disabled with its roadmap sentence until M4a-5. Icons with labels; the
  rail collapses to icons below 1100 px width (*design detail*). The rail
  shows no device status: that lives in the header selector (P3).
- **Header**: app name, device selector (every device the daemon reports
  plus "Simulator"), the profile dropdown on the Pad (the profiles by
  priority, the global one first, the one on the pad marked; choosing one
  pins the inspector, §3.1), the sandbox switch with the Apply and Discard
  pair while the sandbox is on, a gear that opens Preferences, and the
  overflow menu (Export, Import, Open config folder, Re-run setup step,
  Keyboard shortcuts, About). Changing the selected device changes which
  device the drawing and the inspector focus; it never hides a panel (d13).
- **Preferences**: a window (or a screen in the same shell) with the tabs
  General, Devices (the device colour; the Dialpad panel of §3.3 and §4.2 moves here),
  Integrations, Projects. A tab not built yet shows one roadmap sentence
  there; "Coming soon" never sits in the rail.
- **Actions catalogue** (right column of the Pad): a search field and
  collapsible groups, General, System, Apps in v1, Dialpad and Behaviors
  later (§3.1). Collapses into the inspector's Action select below
  1200 px.
- **Window**: default 1200 x 760, minimum 960 x 640. Below 1200 px the
  catalogue collapses, below 1100 px the rail drops its labels, below
  1000 px the list column of Profiles collapses. Modals are used only for
  confirmations (Discard sandbox, Delete profile, Restore snapshot) and
  the trust prompt (§4.5).
- **Hint bar**: one dismissable bar on the Pad after first run (§4.4); no
  tutorial overlay anywhere (P2).

## 3. Screens

Each screen is described as: what the list column holds, what the editor
holds, what is special in the right column.

### 3.1 Pad

*Implements d1, d9, d11; P4. The screen id stays `home`; the label is Pad.*

- No list column. The device fills the centre: a stylised SVG of the
  Keypad (rounded body, the 3 x 3 key cutouts, the two page buttons, no
  vendor mark), coloured from `--kv-device` (§7; graphite by default, the
  pale grey and white editions selectable in Preferences > Devices), the
  key rims and the page buttons as shades derived from it. The mirror
  tiles rendered by keyvo-core sit in the cutouts at the geometry rects
  and scale with the drawing: `width: 100%`, `max-width` the panel's
  pixel width, the device aspect ratio, never above 1:1, never clipped.
  The Dialpad gets its own drawing with the Dialpad slice (the panel of
  §3.3 and §4.2, then a Devices tab of Preferences).
- The mirror shows the pad: the active profile and page, live. A key that
  falls through from the global layer is dimmed with a corner mark; the
  selected key has the accent outline; key feedback (§5.1) draws on the
  tile with an icon or a word.
- Under the drawing: the pages as named tabs (`context` first, "+" adds a
  page) and the accent swatch with its label. The hardware page buttons
  page the pad only; the app's page selection stays where the user put it.
- **Pinned inspector**: the header's profile dropdown and the page tab pin
  the inspector; the mirror keeps following the pad. When the two differ,
  one line under the tabs says "The pad moved to vscode, page 1" with a
  Follow control. The daemon holds the context while the keyvo window has
  focus (`[focus].hold_classes`, #117), so the line is rare; the status
  area then reads "keyvo-app focused, holding org.kde.konsole".
- **Inspector** (below the drawing, full width of the column): the header
  "Key 5: Exec" with the file path and line as the subtitle ("from global"
  for a fall-through key, with an "Override here" action), Press (dry run)
  and Clear at the right; one row with the tile, Label, Icon (picker with
  search over the bundled lucide names) and Colour (swatch and text); then
  the Action select and the kind's fields, or a preset's "What it does"
  select above them. Forms capped at 48ch. Every change writes through
  `toml_edit` immediately (§4.1), the result line with a time stamp under
  the fields. Empty: "Select a key to edit it."
- **Actions catalogue** (right, from a 1200 px window; below it the
  entries collapse into the inspector's Action select): a search field
  (`/`, "Search actions", empty: "No action matches"), then the groups,
  each collapsible on its heading. Drag an entry onto a tile to assign it,
  click to assign it to the selected tile; a raw tool sets the action
  type with an empty field, written once the field is entered; a preset
  writes its default "What it does" with its label and icon at once
  (§4.3). The search covers group, name and variant names. v1 groups:
  General (Open
  folder, Open website, New email, then the raw tools Chord and Exec),
  System (Lock the desktop, Volume, Brightness, Open a settings page),
  Apps (one entry per app, Spectacle, Calculator, VS Code, Konsole and so
  on; dropped on a tile the inspector asks "What it does"). Later: Dialpad
  (a key selects what the dial adjusts: volume, brightness, a Home
  Assistant light) and Behaviors. The entries are the recipes of §4.3.
  Moving a key between tiles stays `Ctrl+X` and `Ctrl+V` and "Move key"
  (d15).
- **Trace strip** (under the inspector): one line with the newest trace
  entries (§8), newest first, each with its 24 h time and opening the
  trace drawer on itself; the chevron expands the strip in place to the
  20 entries the Pad keeps. Debug log switch in the drawer header (Debug
  mode adds HID-level events), disabled with its sentence until the
  daemon sends them.
- Status facts (device, profile, page, mode, top overlay, context with the
  project name or "unknown project" with the reason, d7) live in the
  header and the drawing's caption, each once; they are no longer a strip
  of links.

### 3.2 Profiles

*Implements d1, d2, d14; ADR-0004. The profile manager, the Preferences >
Profiles dialog of Stream Deck as a screen, because keyvo profiles carry
priority and match rules.*

- List column: profiles by priority, the global profile pinned at the top,
  a 12 px accent square after the name, "on the pad" on the live one. "+"
  creates a profile from a template (empty, copy of the active, starter
  pack).
- Editor: the profile header (name, accent as a swatch and `#rrggbb` text,
  priority with "higher wins the window", enabled), the file path and
  "Open in editor" in the heading row, the match rules as one list of kind
  and pattern rows (class, title regex) with inline validation (editing
  the rows arrives with M4a-4), the pages as small thumbnails rendered by
  keyvo-core, and "Edit keys on the Pad", which opens the Pad pinned to
  this profile and page.
- No key grid, no key editor, no right column: the key grid exists once,
  on the Pad. Every change writes through `toml_edit` immediately (§4.1).

**As built (pre-alpha, #109).** `app/src/Profiles.svelte` follows the
above with what has a backend today: name and accent are written (the
editor's `ProfileFields`); priority shows as read with its hint; Enabled,
the templates behind "+", "+ rule" and each rule row's Edit are disabled
controls with their M4a-4 sentence (`Soon.svelte`). Each rule kind is
named once with its patterns under it. The thumbnails are the device
drawing (`Device.svelte`) with the page's rendered tiles; the selected
one is the page "Edit keys on the Pad" pins. An invalid field carries its
own message under it. The list column sits beside the editor from
1000 px and above it below; the list and the editor each scroll with a
reserved gutter and the thin scroll bar. Spec §10 has the detail.

### 3.3 Behaviors

*Implements d4, d9, d12, d13, d16.*

*Sections 3.3 to 3.7 still describe the pre-2026-10-26 shell (rail screens
with list columns, the key card, the Home status strip). Under §2 as
rewritten, Behaviors stays a rail screen and Projects, Integrations, Devices
and Settings become tabs of Preferences; these sections are rewritten with
#108 and #109 and the slices that build them. Until then, read "Home" as the
Pad, "status strip" as the header and the drawing's caption, "key card" as
the inspector's subtitle.*

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
- **Virtual pad**: a click selects a key and does not press it; `Space`
  presses the selected key, `Shift+Space` long (pad screen brief §5.2,
  #108 open point 2); the Dialpad panel has buttons for `cw`, `ccw` and press.
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

**As built (pre-alpha, catalogue v1, #119, #129).** Ahead of the `recipe`
action type and its mini-wizard, a recipe is a pack of `contrib/recipes/`
or the user's `recipes/` (spec §7.5): a group, a name, an icon, a label and
one or more variants ("What it does"), each a plain `chord` or `exec`
action. The daemon lists the packs on the `recipes` request; the Pad's
actions catalogue shows them in their groups, and a dropped or clicked
pack writes its first variant with its label and icon into the key as
plain fields, through the round-trip editor. Nothing resolves at press
time and the profile never names the pack: the inspector recognises a key
that carries one by its icon and an action of its variants, and offers
the "What it does" select above the Action, which rewrites the action to
the chosen variant. No detector, project parameter or wizard step runs
yet.

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
- **Device missing**: the device selector shows "Simulator"; the Pad says which
  device is missing and the last time it was seen.
- **Schema error in a file**: the profile row and the Pad's header show the
  error with file and line; the editor is read-only for that file until the
  file parses again.

## 6. Keyboard map and accessibility

*Implements d15; P1.*

| Keys | Action |
|---|---|
| `Ctrl+1` to `Ctrl+3` | rail screens in order (Pad, Profiles, Behaviors) |
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
| `--kv-device` | the body colour of the device drawing (§3.1), the rims and page buttons derived from it in the component | fixed per edition, chosen in Preferences > Devices: graphite `#2f3133` (default), pale grey `#b9bcc0`, white `#f1f1f1` (*design detail*, confirmed with the owner's SVG in #118) | same | same |

- Sources are read once at start and then followed: KDE through the
  `kdeglobals` file watcher and the `org.kde.KGlobalSettings` D-Bus signal,
  GNOME through `gsettings` monitors, Windows through `UISettings` events.
- Built-in themes (for example a dark developer look) are TOML files that set
  the same tokens; selecting one in Settings stops following the desktop for the
  tokens the theme sets. Native is the default and the only theme the first-run
  flow offers.
- The pad's theme file is separate (it renders tiles); the GUI reads it only for
  the feedback-state tokens so that the mirror and the screens agree.

### Derived muted text and ring

The bridge does not pass two of the desktop's colours through when they fall
short of the contrast §6 promises; it derives them, once per theme change,
from the colours it read:

- **Muted rule.** `--kv-fg-muted` is the desktop's inactive text
  (`ForegroundInactive`, libadwaita's 55 % foreground) when it reaches 4.5:1
  on both `--kv-bg` and `--kv-bg-alt`. Otherwise it is `--kv-fg` mixed
  toward `--kv-bg` in OKLCH, at the largest mix in 1 % steps that still
  passes 4.5:1 on both surfaces: as muted as the theme allows, never
  unreadable.
- **Ring rule.** `--kv-accent` is the desktop's accent when it reaches 3:1
  (the focus ring) on both surfaces. Otherwise it is the accent mixed
  toward `--kv-fg` in OKLCH, at the smallest mix in 1 % steps that passes
  3:1 on both: the desktop's hue, darkened or lightened only as far as
  the ring needs.

Mixing in OKLCH follows `color-mix(in oklch, ...)` (shorter hue arc, a grey's
hue taken from the other colour), clipped to sRGB. The result is computed,
not written as `color-mix()` in CSS, so the test suite can measure it. Until
the bridge exists, `app/src/lib/mock/themes.ts` applies both rules to the
measured sets below (`mutedOf`, `accentOf`).

### Measured token sets (mock dev server)

Until the bridge exists (M4a-1), the mock dev server (`just app-dev`,
`app/src/lib/mock/themes.ts`) sets the tokens from four sets measured from
the sources above, picked with `?theme=` (default `breeze-dark`), so a
layout is judged in light and dark. Measured on 2026-10-26; the rows marked
*applied* are what the mock sets after the rules of "Derived muted text and
ring" (the file keeps the measured sets as `MEASURED`).

| Token | `breeze-dark` | `breeze-light` | `adwaita-dark` | `adwaita-light` |
|---|---|---|---|---|
| `color-scheme` | `dark` | `light` | `dark` | `light` |
| `--kv-bg` | `#202326` | `#eff0f1` | `#222226` | `#fafafb` |
| `--kv-bg-alt` | `#292c30` | `#e3e5e7` | `#2e2e32` | `#ebebed` |
| `--kv-fg` | `#fcfcfc` | `#232629` | `#ffffff` | `#323237` |
| `--kv-fg-muted` | `#a1a9b1` | `#707d8a` | `#9c9c9d` | `#8c8c8f` |
| `--kv-fg-muted`, *applied* | `#a1a9b1` | `#636668` (derived) | `#9c9c9d` | `#6a6a6e` (derived) |
| `--kv-accent` | `#3daee9` | `#3daee9` | `#81d0ff` | `#0461be` |
| `--kv-accent`, *applied* | `#3daee9` | `#4088b6` (derived) | `#81d0ff` | `#0461be` |
| `--kv-font` | `"Noto Sans", sans-serif` | same | `"Adwaita Sans", sans-serif` | same |
| `--kv-font-size` | `10pt` | `10pt` | `11pt` | `11pt` |
| `--kv-radius` | `5px` | `5px` | `9px` | `9px` |
| `--kv-sandbox`, `--kv-project` | `#b48ef0`, `#3cbfae` | same | same | same |
| `--kv-state-running`, `-success`, `-error` | `#f08c00`, `#2f9e44`, `#e03131` | same | same | same |
| `--kv-contrast` | `normal` | `normal` | `normal` | `normal` |
| `--kv-motion` | the system's `prefers-reduced-motion` | same | same | same |
| `--kv-device` | `#2f3133` | same | same | same |

Sources:

- Breeze: `colors/BreezeDark.colors` and `colors/BreezeLight.colors` of
  `plasma/breeze` (master at 3f23386c), the files a Breeze `kdeglobals`
  copies. `--kv-bg` and `--kv-bg-alt` are `[Colors:Window]`
  `BackgroundNormal` and `BackgroundAlternate`, `--kv-fg` and
  `--kv-fg-muted` its `ForegroundNormal` and `ForegroundInactive`. The
  schemes set no `[General] AccentColor`, so the accent is the scheme's
  own, `DecorationFocus` (61,174,233). Font: `Noto Sans` 10 pt, the
  default of `kcms/fonts/fontssettings.kcfg` in `plasma/plasma-workspace`.
  Radius: `Frame_FrameRadius = 5` of `kstyle/breezemetrics.h` in
  `plasma/breeze`.
- Adwaita: libadwaita 1.10.0, `src/stylesheet/_colors.scss` and
  `_common.scss`. `--kv-bg` is `window_bg_color`, `--kv-bg-alt`
  `sidebar_bg_color`; `--kv-fg` is `window_fg_color` (light:
  `RGB(0 0 6 / 80%)` over the background), `--kv-fg-muted` the same at
  the `--dim-opacity` of 55 %, both composited to opaque colours. The
  accent is the default `blue` (`accent-color` of
  `org.gnome.desktop.interface`, gsettings-desktop-schemas 51.0,
  `#3584e4`) as libadwaita's standalone `accent_color`: OKLab lightness
  capped at 0.5 in light, raised to 0.85 in dark. Font: `Adwaita Sans 11`
  (`font-name`, gsettings-desktop-schemas 51.0). Radius: `$button_radius`
  of `_common.scss`.
- Not from the desktop: the derived `--kv-sandbox` and `--kv-project` keep
  the pre-alpha defaults of `tokens.css` until M4a-1 defines the
  derivation; the feedback states are the pad theme's
  (`keyvo_core::render::Theme`); `--kv-contrast` is `normal` (none of the
  four is a high-contrast scheme).

Contrast against the WCAG thresholds that `app/src/styles/tokens.test.ts`
holds the default set and the four applied sets to (every text pair 4.5:1,
the focus ring 3:1 on both surfaces): the dark sets pass as measured and
keep every measured value. The light sets do not: in `breeze-light` the
measured muted text is 3.7:1 on `--kv-bg` and 3.3:1 on `--kv-bg-alt` and
the accent 2.2:1 and 2.0:1, in `adwaita-light` the muted text 3.2:1 and
2.8:1 (the P1 of `critique-2026-10.md`). The measured values stay recorded
as measured; the rules replace them in the applied set: `breeze-light`
muted `#636668` is 5.1:1 and 4.6:1, its accent `#4088b6` 3.4:1 and 3.1:1;
`adwaita-light` muted `#6a6a6e` is 5.2:1 and 4.5:1 (#140).

## 8. Trace drawer and debug log

*Implements d9; P4.*

One trace entry per press, real or simulated. The drawer opens from the Pad's
trace strip, from the "Test this behavior" result line, and from a key's error
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
| in | `subscribe` (`events`: key, profile, page, mode, device, context, trace, config) | the mirror, the header facts, the trace strip |
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
| P3 no redundancy | 2 (rail, header), 3.1 (inspector subtitle) |
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
