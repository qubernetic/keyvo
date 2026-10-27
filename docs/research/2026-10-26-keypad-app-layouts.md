# Prior art: desktop layouts for configuring an LCD-key pad

Date: 2026-10-26. Scope: the arrangement of the main window in apps that
configure a Stream Deck class device or a programmable keyboard, as input to
the Profiles layout decision of slices 4 to 6 (#107, #108, #109) and the
critique of 2026-10 (`docs/design/critique-2026-10.md`, sections 5 and 9).
Layout ideas only; no code was copied, and every open-source app's licence is
named so that nothing is borrowed beyond the idea.

The owner's instruction mid-study: the Elgato Stream Deck desktop app is the
primary analogy. Section 2 therefore describes it in depth and section 3 maps
every keyvo feature onto its idiom. The other apps are short counterpoints
(section 4), followed by the comparison table, the recommendation with
sketches, and the open questions.

## 1. The constraints the layout must satisfy

From `docs/PRODUCT.md` and `docs/design/gui-ux.md` sections 1 to 3:

- Native theme by default: colours, fonts and radius come from the desktop
  through the `--kv-*` tokens; no own palette, no fixed dark look.
- The mirror is the truth: the tile on screen is rendered by the same renderer
  as the LCD; editing happens on the mirror, there is no second canvas.
- Every fact once; dense but calm; one primary action per screen; cards are
  the exception, nested cards never.
- Developers only: the owner on KDE Wayland with the app in a side window,
  and a junior who configures a key without reading TOML.
- Window 1200 x 760 default, 960 x 640 minimum; rail collapses to icons
  below 1100 px; columns collapse right column first, list column second.
- The critique's findings the new layout must solve: the clipped preview
  (P0), the editor that swaps its file when the pad moves (P1), forms that
  stretch across the column (P2), and the selected key shown twice, as a form
  in the middle and a three-fact card on the right (question 2).

## 2. Elgato Stream Deck desktop app (the baseline)

Proprietary (Corsair/Elgato); Windows and macOS; versions 6.x and 7.x. Sources:
the SDK documentation at docs.elgato.com, which carries screenshots of the
app, the Explorer and Marketplace how-to articles at elgato.com, and the
help-centre articles at help.elgato.com. Screenshots studied:
`elgato-app-sdk.png` (6.x, Windows, Stream Deck +, a key selected),
`elgato-sdk-default-profile.png` (6.x, macOS, Stream Deck Neo, no key
selected), `elgato-smartprofile-4.png` (Preferences, Profiles tab),
`elgato-sdplus-setup-actions.png` and `elgato-sdplus-personalize.png` (quick
start drawings). Paths in section 9.

### 2.1 Anatomy of the main window

One window, two columns, no navigation rail, no tabs:

```
+-----------------------------------------------------------+------------------+
| Stream Deck +  v                  [bell][market][gear][me] | [search      ] ≡ |
| Profile 1  v                                               | ( Keys | Dials ) |
|                                                            |------------------|
|        [ ] [ ] [ ] [ ]        canvas: the device,          | v Soundboard     |
|        [ ] [ ] [ ] [ ]        keys at a fixed size,        | v Stream Deck    |
|        [=============]        touch strip, dials           | v System         |
|        ( ) ( ) ( ) ( )                                     | v Multi Action   |
|                 [pin] (1) [+]       page bar                | v Navigation     |
|------------------------------------------------------------| v Volume Ctrl    |
| Hello World: Counter                               [trash] | v Hello World    |
| [tile v]   Title: [__________________] T v                 |     + Counter    |
|            ... fields of the selected action ...           |                  |
+-----------------------------------------------------------+------------------+
```

- Header of the left column: the device name as a dropdown (every connected
  device plus the virtual deck and Mobile), under it the profile name as a
  second dropdown. Top right of the same column: notifications, Marketplace,
  the gear (Preferences) and the account avatar. There is no device status
  text; the device name is the status.
- Canvas: the device, drawn as its key grid in device proportions, with the
  touch strip and dials of a Stream Deck + or the info bar of a Neo. Keys are
  drawn at a fixed pixel size (about 72 px in an 840 px window; the XL uses
  smaller keys to fit 8 x 4) and the grid is centred in the free space. The
  canvas does not scale with the window; it is the one fixed element and
  everything else yields.
- Page bar under the canvas: a pin, one pill per page ("1", "2"), and "+".
  Pages 2 to 9 receive Next Page and Previous Page keys automatically, the
  last page only Previous; up to 10 pages per profile; right-click a pill to
  delete a page (help.elgato.com, "Elgato Stream Deck, Pages").
- Property inspector below the canvas, full width of the left column: a
  header line "Plugin: Action" with a trash icon at the right, then one row
  with the key thumbnail at the left (its "+" and chevron open the icon menu:
  Icon, Set from File, Create New Icon) and the Title field with a text-style
  dropdown at the right, then the action's own fields. With no key selected
  the panel shows one sentence: "Select a key to configure its action."
- Actions list, right column, fixed width: a search field with a list/grid
  toggle, a segmented control Keys | Dials (Keys | Infobar on the Neo), then
  collapsible groups, one per plugin, with an icon and the plugin name, and
  the actions inside. An action is dragged onto a key (the quick start says
  "drag actions from the right panel and drop them onto keys on the left").
  Since 7.1 the list also searches the Marketplace and installs plugins in
  place (help.elgato.com, "Stream Deck 7.1 Beta Changelog").
- Settings: the gear opens a separate Preferences window with the tabs
  General, Devices, Accounts, Profiles, Plugins, Style.

### 2.2 Profiles and the manage-profiles dialog

Profiles are per device. The dropdown under the device name switches the
profile the canvas edits. Management lives in Preferences > Profiles: a device
dropdown at the top left, the profile list on the left (expanders per
profile, "+" and "-" at the bottom), and on the right the sentence "Stream
Deck will automatically switch to this profile when the Stream Deck app
window is closed and the following application is in the foreground", an
Application dropdown (recently used apps, or "Other..." to browse) and a
checkbox "Make this my default profile". Once an application is chosen its
icon appears next to the profile in the list (elgato.com, "How to set up
Smart Profiles on Stream Deck"). There is no priority and no title matching:
one application maps to one profile, the default profile is the fallback.
Smart Profiles are deactivated while the app window is open, "to prevent
profile switching while editing" (same article), which is how Elgato avoids
the editor-swap problem the critique found in keyvo.

### 2.3 Pages, folders and multi-actions

Pages are flat siblings inside a profile (section 2.1). Folders are a key
type: dragging "Folder" from the Navigation group onto a key turns it into a
folder; opening it shows a fresh layout with a back key in the first slot;
folders nest; a folder can auto-exit after up to 60 s; an action is moved
into a folder by dragging it over the folder key (elgato.com, "Organize your
Stream Deck with folders"). Multi Action is also a key type whose property
inspector is a list of child actions; editing a child swaps the inspector to
that child's fields with a back control. So the hierarchy on the canvas is
profile > page > key, and a key may open a sub-layout (folder) or a sub-list
(multi action), both edited in the same inspector slot.

### 2.4 How the property inspector changes per action

The inspector is the plugin's own HTML page (`PropertyInspectorPath` in the
manifest) loaded into the panel under the canvas; Elgato ships
`sdpi-components` so that every plugin's fields look like the built-in ones.
The Marketplace guidelines require: checkbox for booleans, select or radio
for single choice, validation feedback, settings saved automatically on
change, and no Save button for action settings; they recommend hiding
components until needed and splitting a complex action into smaller actions
rather than a long form (docs.elgato.com, Plugin Guidelines, "Property
Inspectors (UI)"). The app-owned part of the inspector (thumbnail, icon menu,
Title and text style, the trash) is the same for every action; only the area
below it belongs to the plugin. Keys have states (a multi-state key shows a
state selector), and the SDK's `setTitle`, `setImage`, `showOk` and
`showAlert` update the key on the device and on the canvas (docs.elgato.com,
SDK, Keys).

### 2.5 Live device state

The canvas is the device: the key images on screen are the images sent to
the hardware, including plugin updates (clocks, counters), and the success and
alert overlays drawn by `showOk` and `showAlert` appear on both. What the
canvas does not do is follow the device: it shows the profile and page chosen
in the dropdowns, and Smart Profile switching is suspended while the window
is open. There is no event log; a failed action shows the alert triangle on
the key and nothing else.

### 2.6 Narrow widths

The app has no narrow mode. The window has a minimum size, the actions column
keeps its width, and the canvas keeps its pixel size; the inspector takes the
remaining height and scrolls. Below the minimum the window simply stops
shrinking. The screenshots at 840 px width show that the two-column form
survives a side-window size because the canvas is small and the list is a
single column of text.

### 2.7 What works and what does not, measured against keyvo's rules

Works: one screen for the whole job; the device as the only fixed-size
element with everything else yielding; the inspector directly under the
canvas so the control sits next to the gauge; the app-owned header row of the
inspector (thumbnail, title, delete) so every action looks the same at the
top; profile management kept out of the main screen; the explicit rule that
editing and auto-switching do not run at the same time.

Does not work for keyvo: the own dark theme; the plugin store in the action
list; drag-and-drop as the only way to assign an action (the quick start's
first instruction); folders and multi-actions as hidden depth; the fixed-size
canvas (keyvo's preview is already clipped at 320 px and needs scaling); no
trace at all; profile matching reduced to one executable per profile.

## 3. Mapping keyvo onto the Stream Deck idiom

| Stream Deck element | keyvo equivalent | Notes |
|---|---|---|
| Device dropdown, top left of the canvas column | Device selector in the header (gui-ux §2) | Same place, same role; "Simulator" is an entry like Elgato's virtual deck |
| Profile dropdown under the device name | Profile dropdown under the device name on the main screen | Lists profiles by priority, global pinned first, the one on the pad marked; choosing one pins the editor to it (critique P1) |
| Preferences > Profiles (list, Application dropdown, default checkbox) | Profiles screen reduced to a profile manager: list by priority, header form (name, accent swatch, priority, enabled), match rules (class, title regex) as a list, file path and "Open in editor" | Priority and two rule kinds replace the single Application field; the global profile replaces "Make this my default profile" |
| Page pills under the canvas with "+" | Page tabs under the mirror, the `context` page labelled, "+" adds a page | The Keypad's two hardware page buttons map to the same pills; no auto-inserted navigation keys because paging is hardware |
| Default profile fallback | Global layer fall-through | No Stream Deck equivalent; a tile the app profile lacks is drawn from the global profile, dimmed, with an origin tag "from global" in the inspector subtitle; editing it offers "Override here" |
| Inspector header "Plugin: Action" + trash | "Key 5: Exec" + the file path and line as subtitle + Press (dry run) + Clear | The subtitle carries the three facts of the old key card (position, page, origin) once |
| Thumbnail with icon menu, Title field, text style | Tile (rendered by keyvo-core), Label, Icon picker, Colour swatch | Same row; the tile is the mirror tile, not a second drawing |
| Plugin fields under the app-owned row, auto-saved, no Save | Action kind select (Chord, Exec, Go to page, Switch profile), then the kind's fields: chord record field, command, working directory, hold | Written through `toml_edit` on change, as today; the sandbox decides whether the write reaches the real file |
| Actions list on the right, grouped, searchable, drag to key | A catalogue column is optional in v1 (open question 2); its content later is Behaviors and recipes (gui-ux §3.3, §4.3) | keyvo has four action kinds, not forty plugins; a select in the inspector covers v1 |
| Canvas shows the chosen profile; Smart Profiles suspended while open | Mirror shows the pad (the truth); the inspector is pinned to the chosen profile and page; a one-line notice "The pad moved to vscode, page 1" with Follow | keyvo keeps the live mirror because P4 (debuggable) needs it; the daemon may exclude the keyvo window from matching, as Elgato effectively does (critique question 1) |
| showOk / showAlert on the key | Key feedback states on the tile (gui-ux §5.1) plus the trace drawer | The trace is keyvo's addition: a strip under the inspector, newest first, opening the drawer |
| No sandbox, writes are immediate | Sandbox switch in the header with the Apply and Discard pair (gui-ux §1) | Header turns to the sandbox token; the pair stays visible until pressed |
| Preferences window with tabs | Settings, Devices, Integrations as Preferences tabs or rail entries | Decides what to do with the five "Coming soon" rail entries (critique question 4) |
| Folders, Multi Action | Overlays and modes (keyvo-core state machine), not folders | Shown as a tag on the tile and in the status line, not as a sub-layout to drill into |

## 4. Counterpoints

Each paragraph records: hierarchy, where the key editor lives, how profiles
and pages are chosen, how the device image scales, how many controls are on
the main screen, live state, below 1100 px, one thing to borrow, one to
avoid, licence.

**StreamController** (Linux, Python, GTK4 and libadwaita, GPL-3.0,
github.com/StreamController/StreamController). `mainWindow.py` builds an
`Adw.NavigationSplitView` whose sidebar is the editor (width fraction 0.4,
450 to 600 px) and whose content is the deck: a `Gtk.Grid` of key images with
a 75 px minimum that grows with the window, centred, with the deck name pill
on top and a "Page Key Grid | Page Settings" switcher at the bottom. The
editor sits on the left: Page dropdown, State tabs, a large key preview,
then expanders Layout, Background, Labels, Actions and an accent-filled "Add
Action" button; choosing an action pushes a chooser page into the same
sidebar. Profiles do not exist as such: pages are the unit, with automatic
page switching per window class. Live: the key images are the rendered
images sent to the deck. Narrow: the split view collapses the editor into a
pushed page behind a toggle. Borrow: the editor as a stack of pushed pages in
one slot. Avoid: the editor on the left of the device and the 40 % of the
editor spent on a preview.

**Boatswain** (GNOME, C, libadwaita, GPL-3.0, gitlab.gnome.org/World/boatswain).
`bs-window.ui`: an `AdwNavigationSplitView` with a Devices sidebar (device
popover, profile list with a check on the active one, "New profile..." entry,
brightness, firmware) and a device editor as content; `bs-device-editor.ui`
puts the regions grid inside an `AdwClamp` (max 700 px, tightening 500) and
the button editor in an `AdwOverlaySplitView` with `sidebar-position: end`
(right, 280 px minimum). `bs-button-editor.ui` is an `AdwNavigationView`:
"Button Properties" (128 px icon with an icon-grid popover, Name row,
Background Color row, action rows, Remove Action) pushes "Actions" (the
selector list) and pops back. Default 1200 x 720. Live: the grid shows the
device's images. Narrow: the overlay split can collapse the editor over the
content. Borrow: the clamp that caps the device at 1:1 and lets it shrink,
and the profile list as a plain list with a check. Avoid: the 128 px icon,
the full-width destructive button, the empty centre.

**OpenDeck** (Linux, Tauri 2 and SvelteKit with Tailwind, GPL-3.0,
github.com/ninjadev64/OpenDeck). `src/routes/+page.svelte` is a flex row:
a growing left column with a nav bar (`DeviceSelector` and `ProfileManager`
selects top left, `PluginManager` and `SettingsView` buttons top right),
`DeviceView` in the middle and `PropertyInspectorView` at the bottom (an
iframe area, 16 to 24 rem tall, with a border on top); `ActionList` is a
fixed 18 rem right column with a search field and `<details>` groups whose
rows are draggable. `DeviceView` lays the grid out at a fixed 132 px pitch
and `Key.svelte` scales each key with a CSS transform (base 144, 192 for the
XL); the view is `overflow-auto`, so a small window scrolls the device
instead of shrinking it. A right-click menu on a key offers Edit, Copy,
Paste, Delete, and Edit opens `InstanceEditor`, an absolutely positioned
modal for states, image, colours and text. `ProfileManager` is a select whose
last entry opens a `Popup` for create, rename, duplicate, delete. This is a
Stream Deck clone, which makes its component split the useful map for keyvo:
one component per Stream Deck region plus `Key`, `Popup` and `Tooltip`.
Live: keys show the plugin images. Narrow: no media queries at all. Borrow:
the component boundaries. Avoid: the hard-coded neutral-700 theme and the
modal key editor.

**VIA** (web, React and styled-components, GPL-3.0, github.com/the-via/app,
usevia.app). Top tabs Configure, Design, Test, Settings, Debug. In
`panes/configure.tsx` the keyboard is rendered on top in a `CenterPane`, a
`LayerControl` sits above it, and under the keyboard a left menu of rows
(Keymap, Layouts, Macros, Lighting, Save + Load, plus custom panes) selects
the sub-pane shown to its right; the Keymap pane is a keycode palette by
category. Click a key, click a keycode: no form. The keyboard scales with the
viewport. Live: the Test pane highlights pressed keys. Narrow: the keyboard
shrinks; the palette wraps. Borrow: the palette flow for the chord field.
Avoid: the showcase rendering.

**Vial** (desktop, PyQt5, GPL-2.0, github.com/vial-kb/vial-gui). A
`QTabWidget` with nine tabs (Keymap, Layout, Macros, Lighting, Tap Dance,
Combos, Key Overrides, QMK Settings, Matrix tester); the Keymap tab has a
layer button row, the keyboard widget with manual zoom buttons, and a
`TabbedKeycodes` palette with its own nine tabs at the bottom
(`vial-keymap.png`). Native Qt widgets, so it follows the system theme
without trying. Live: the matrix tester shows presses. Narrow: the keyboard
zooms by hand, the palette scrolls. Borrow: trusting the toolkit for the
theme. Avoid: two tab bars on one screen.

**Oryx** (web, proprietary, configure.zsa.io). Layer tabs above the
keyboard, the keyboard centred and scaled to the viewport, and, when editing,
a key panel that opens under the keyboard with the key's type and modifiers;
public layouts are read-only (`oryx-voyager-default-layout.jpg`). Live state
is delegated to the Keymapp companion app, which shows the active layer.
Narrow: responsive web. Borrow: the layer tabs directly above the device and
a legend colour per layer. Avoid: the account gating and the pop-under editor
with its own sub-tabs.

**Loupedeck** (proprietary, anti-reference). Three columns: an actions tree
with search on the left, a photo-real device in the centre with live labels on
its touch keys, and the workspaces and pages tree on the right; the header
holds Device and Application dropdowns, the right column a Profile dropdown
(`loupedeck-v4-ppro.jpg`). Four levels (profile, workspace, page, action)
compete for the same screen, and a key's settings are a popover. Nothing to
borrow beyond "the device is the canvas", which Stream Deck does with less.

**Logi Options+** (proprietary, anti-reference). A home with hero device
photos ("Good Evening", `optionsplus-main.jpg`), then one page per device
with a large illustration and the settings beside it; the Keypad's actions
are configured per application in a list on the left with the illustration on
the right. The application is the first choice, which is the one idea in
common with keyvo's match rules; everything else is decoration around a
small form.

**Bitfocus Companion** (web UI, licence per the repository's own file,
GitHub reports NOASSERTION, github.com/bitfocus/companion). Top tabs
Connections, Buttons, Surfaces, Triggers, Variables, Settings, Import/Export,
Log. Buttons: the grid on the left with a page number, a right column with
Pages, Presets and Recorder tabs, and the editor for the selected button
below the grid: Step tabs (press and release actions, feedbacks, local
variables) with one inline form per action (`companion-button-actions.png`,
`companion-pages.png`). Live: the grid shows the rendered buttons. Narrow:
Bootstrap stacking. Borrow: inline per-action forms in one list and a visible
feedback list. Avoid: the density of a broadcast shotbox.

## 5. Comparison table

| App | Primary element | Key editor | Profile / page choice | Device scaling | Live state | Below 1100 px | Licence |
|---|---|---|---|---|---|---|---|
| Stream Deck | device canvas | below the canvas | two dropdowns top left; page pills under the canvas; Preferences dialog to manage | fixed px, centred | canvas is the device; no log | minimum window size, no reflow | proprietary |
| StreamController | device grid | left sidebar (40 %) | page dropdown in the editor; no profiles | grid grows with window, 75 px minimum | rendered images | sidebar collapses into a pushed page | GPL-3.0 |
| Boatswain | device grid | right sidebar, 280 px, navigation stack | profile list in the left sidebar; no pages | clamped at 700 px, shrinks | device images | overlay split collapses | GPL-3.0 |
| OpenDeck | device grid | bottom panel (iframe) | selects top left; popup to manage | fixed 132 px pitch, scrolls | plugin images | none | GPL-3.0 |
| VIA | keyboard | palette below | layer buttons above | scales with viewport | Test pane | shrinks | GPL-3.0 |
| Vial | keyboard | palette below | layer buttons above; nine top tabs | manual zoom | matrix tester | scrolls | GPL-2.0 |
| Oryx | keyboard | panel below (pop-under) | layer tabs above | scales with viewport | in Keymapp | responsive | proprietary |
| Loupedeck | device photo | popover | header dropdowns, tree on the right | fixed | labels on the photo | fixed | proprietary |
| Options+ | device photo | list beside the photo | app list on the left | fixed | none | fixed | proprietary |
| Companion | button grid | below the grid, tabbed steps | page number, Pages tab on the right | fixed cells | rendered buttons | stacks | see repo |
| keyvo today | three columns, none primary | middle column form + right card | list column; page tabs | 1:1, clipped at 320 px | Home mirror | columns scroll | Apache-2.0 |

The pattern across the field: the device is the primary element and sits in
the largest, calmest area; the editor is directly below it (Stream Deck,
OpenDeck, VIA, Vial, Oryx, Companion) or in a side panel on the device's side
of the window (Boatswain, StreamController); profiles and pages are chosen by
a dropdown or a tab row attached to the device, never by a third column that
competes with it. keyvo's current Profiles screen is the only layout in the
table with three equal columns and the device in the smallest one.

## 6. Recommendation

Idiom: the Stream Deck main screen, native-themed, with keyvo's content.
One main screen ("Pad", today's Home) holds the mirror, the page bar and the
inspector under it; the Profiles screen shrinks to a profile manager (the
Preferences > Profiles dialog of Stream Deck, as a screen because keyvo
profiles carry more fields than one Application dropdown) and has no key
grid. The key grid exists once, on the mirror.

Why this beats the alternatives for a developer tool on the native theme:

- The mirror is the truth becomes visible instead of implied: there is one
  grid, it is the device, and the control for a key sits directly under it.
  This closes critique question 2 (two places for one key) by construction.
- Every fact once: the inspector header carries position, page and origin;
  the profile dropdown carries the profile; the page bar carries the page.
  The three-fact card and the duplicated "File:" lines disappear.
- The device scales because it is the only thing in its area: the mirror
  wrapper is `width: 100%; max-width: <panel px>; aspect-ratio: <panel>`
  with tiles positioned by percentage from the geometry rects, which solves
  the P0 clip and keeps 1:1 as the ceiling, as the critique's fix demands.
- The native theme has nothing to fight: a header, a framed device, a form
  at `48ch`, a list. No column that needs its own surface colour, no card
  nesting. Boatswain and Vial show that this composition follows a desktop
  theme without effort; Stream Deck and OpenDeck show it with a fixed dark
  theme, which keyvo drops.
- Developers already know the layout from Stream Deck, VIA and Companion,
  so the junior's first key needs no tour: pick a key, fill the form under
  it, watch the tile change.
- Below 1100 px nothing competes horizontally: the inspector is under the
  mirror, the rail is icons, and the only optional column (the catalogue, if
  it exists) collapses into the inspector's action select. 960 px holds.

### 6.1 Pad screen (Home), sketch

```
+----+-----------------------------------------------------------+-------------+
|rail| Keypad v   Profile: vscode v   [sandbox] Apply  Discard   | Actions     |
|    |                                                           | [search   ] |
|    |   +-----------------------------------------------+       | v Keys      |
|    |   |   [git]  [test] [run ]     mirror in device   |       |    Chord    |
|    |   |   [ 4 ]  [ 5 ]  [ 6 ]     proportions, scales |       |    Exec     |
|    |   |   [ 7 ]  [ 8 ]  [ 9 ]     down, never above 1:1|      |    Go to pg |
|    |   +-----------------------------------------------+       |    Switch   |
|    |   Pages  [context] [2 git] [3 test] [+]    Accent [sw]    | v Behaviors |
|    |   On the pad: vscode, context   (Follow)                  |   (M4a-2)   |
|    |-----------------------------------------------------------|             |
|    | Key 5: Exec   vscode.toml line 42         [Press] [Clear] |             |
|    | [tile]  Label   [Run tests        ]   Icon  [play     v]  |             |
|    |         Colour  [sw] #2a6f4e          Action [Exec     v] |             |
|    |         Command [just test          ]  Cwd [~/Git/keyvo ] |             |
|    |-----------------------------------------------------------|             |
|    | Trace v  12:01:03 key 5 exec ok 12 ms | 12:00:58 focus ... |             |
+----+-----------------------------------------------------------+-------------+
```

- Header row inside the screen: device selector, profile dropdown (global
  pinned first, the one on the pad marked), sandbox switch, Apply and
  Discard when the sandbox is on. Choosing a profile pins the inspector to
  that profile; the mirror keeps showing the pad.
- Mirror: tiles rendered by keyvo-core; a tile that falls through from the
  global layer is dimmed with a corner mark; the selected tile has the
  outline accent; key feedback states (running, success, error) draw on the
  tile with an icon or word, never colour alone.
- Under the mirror: the page bar (the `context` page named, numbered pages
  after it, "+"), the accent swatch with its label, and one status line
  "On the pad: vscode, context" with a Follow control that appears only
  when the pad and the editor differ (the critique's P1 fix).
- Inspector: header "Key 5: Exec" with the file and line as subtitle, Press
  (dry run) and Clear at the right; one row with the tile at the left and
  Label, Icon, Colour; then the Action select and the kind's fields; forms
  capped at `48ch`; written on change (sandbox permitting). Empty state: one
  sentence, "Select a key to edit it", as Stream Deck does.
- Trace: a one-line strip with the newest entries, the chevron opens the
  drawer (gui-ux §8); it is the gauge for Press.
- Actions column: optional (open question 2). If present, it is a plain
  list grouped by kind, searchable, drag to a tile or click to assign to the
  selected tile. If absent, the Action select in the inspector is the
  catalogue.

### 6.2 Profiles screen, sketch

```
+----+---------------------------+-------------------------------------------+
|rail| Profiles        [+] [...] | VS Code                     [Open in editor]|
|    |---------------------------| ~/.config/keyvo/profiles/vscode.toml      |
|    | Global          (pinned)  |                                           |
|    | VS Code  [sw]  on the pad | Name     [VS Code       ]   Enabled  [x]  |
|    | Konsole  [sw]             | Accent   [sw] #2a6f4e                      |
|    | Firefox  [sw]             | Priority [20]  higher wins the window     |
|    |                           |                                           |
|    |                           | Match rules                     [+ rule]  |
|    |                           |   class  [^code$            ]      [x]    |
|    |                           |   title  [Visual Studio Code$]     [x]    |
|    |                           |                                           |
|    |                           | Pages: context, git, test                 |
|    |                           | [ctx] [git] [test]   small page thumbnails|
|    |                           | Edit keys on the Pad  ->                  |
+----+---------------------------+-------------------------------------------+
```

- The list column is Stream Deck's profile list with keyvo's facts: name,
  a 12 px accent square after the name, "on the pad" for the live one, the
  global profile pinned. "+" creates from a template (gui-ux §3.2).
- The editor is the profile header and the match rules, nothing else: one
  "Match rules" heading, rows of kind and pattern, inline validation. The
  page thumbnails are small previews rendered by keyvo-core (the mirror again,
  not a second drawing) and the single link "Edit keys on the Pad" switches
  to the Pad screen with this profile pinned and this page chosen.
- No key grid, no key editor, no right column: the screen has one job.

### 6.3 What this means for the three variants of slice 4

Variant A (editor beside the preview under a chip bar) is closest to the
idiom but keeps the key grid on Profiles; variant B (document column with a
pad aside) is the profile-manager half; variant C (three columns) is the
layout the field has moved away from. The recommendation is A's bench moved
to the Pad screen and B's document column kept as the Profiles screen, with
the grid removed from it.

## 7. Open questions for the owner

1. **Two screens or one.** Merge Home and Profiles into one Stream Deck
   style main screen, with profile management in a dialog, or keep two
   screens as sketched above (Pad with the only grid; Profiles as a manager).
   Merge: fewer screens, one grid, one editor, the junior's whole task on one
   screen; but the match rules and priority are a form that outgrows a
   dialog, and a modal manager breaks "modals only for confirmations" (gui-ux
   §2). Two screens: the Profiles screen keeps the file-level facts (path,
   rules, priority, pages) where a developer expects a file editor, the Pad
   keeps the mirror; but the user switches screens to go from a rule to a
   key. The sketches assume two screens with the grid on the Pad only; the
   difference from a merge is one rail entry.
2. **Catalogue column in v1.** Four action kinds do not need a column; a
   select in the inspector is the Stream Deck list folded into the form.
   The column earns its place when Behaviors and recipes arrive (M4a-2,
   gui-ux §3.3, §4.3). Decide whether to reserve the slot now (collapsed by
   default) or add it then.
3. **Pinned editor versus follow.** The mirror follows the pad and the
   inspector is pinned to the chosen profile and page, with a "pad moved"
   notice and Follow. Alternatively, as Elgato does, suspend matching while
   the keyvo window is focused (daemon-side exclusion of the keyvo window
   class), which makes the notice rare. Both can hold; the second is a
   daemon change (critique question 1).
4. **Global fall-through tiles.** Dimmed with a corner mark and "from
   global" in the inspector subtitle, with an "Override here" action; or
   drawn exactly as on the pad with the origin only in the subtitle. The
   first makes the layer visible, the second keeps the mirror literal.
5. **Page bar form.** Tabs with names (the `context` page needs its name)
   versus Stream Deck's numbered pills; and whether the Keypad's hardware
   page buttons should move the app's page selection (follow) or only the
   pad.
6. **Trace placement.** A strip under the inspector that opens the drawer
   (sketched), or a right-side drawer that replaces the catalogue column
   when open. The strip keeps the gauge next to Press.
7. **Rail versus Preferences.** With Home and Profiles the only built
   screens, show the two plus Settings and move Devices and Integrations to
   Preferences tabs (Stream Deck's General, Devices, Plugins), or keep the
   seven-entry rail with "Coming soon" (critique question 4).
8. **Drag and drop.** Stream Deck's first instruction is dragging; keyvo's
   d15 keeps `Ctrl+X` and `Ctrl+V` between keys. Whether dragging from a
   catalogue to a tile is worth building before the catalogue has more than
   four entries.
9. **The device frame.** Draw the mirror with a bezel-like frame at device
   proportions (critique question 5) so that "the mirror is the truth" reads
   on a light desktop, or keep nine tiles on the app surface with only the
   outline accent on the selection.

## 8. Sources

Stream Deck:

- https://docs.elgato.com/streamdeck/sdk/introduction/getting-started (main window screenshot, Windows, Stream Deck +)
- https://docs.elgato.com/streamdeck/sdk/guides/profiles/ (Preferences > Profiles screenshot; the macOS main window screenshot with the Neo)
- https://docs.elgato.com/streamdeck/sdk/guides/ui/ (property inspector as plugin HTML, sdpi-components)
- https://docs.elgato.com/guidelines/streamdeck/plugins/ui (inspector rules: auto-save, no Save button, checkbox and select, split complex actions)
- https://docs.elgato.com/streamdeck/sdk/guides/keys/ (states, titles, images, showOk and showAlert)
- https://www.elgato.com/us/en/explorer/products/stream-deck/smart-profiles-stream-deck/ (Smart Profiles steps and screenshots; suspended while the app is open)
- https://www.elgato.com/us/en/explorer/products/stream-deck/how-to-use-folders-stream-deck/ (folders, nesting, auto exit)
- https://www.elgato.com/us/en/explorer/products/stream-deck/elgato-stream-deck-plus-quick-start-guide/ (drag from the right panel; the inspector row drawing)
- https://help.elgato.com/hc/en-us/articles/4410312027277-Elgato-Stream-Deck-Pages (page bar, 10 pages, automatic navigation keys, delete by right-click)
- https://help.elgato.com/hc/en-us/articles/40724626846481-Stream-Deck-7-1-Beta-Changelog (Marketplace search in the action list, device disable)
- https://help.elgato.com/hc/en-us/articles/360028234471 (System actions: Hotkey, Open, Website, Text, Multimedia)

Open-source apps (source trees read through the GitHub and GitLab APIs on 2026-10-26):

- https://github.com/StreamController/StreamController (GPL-3.0): `src/windows/mainWindow/mainWindow.py`, `elements/Sidebar/Sidebar.py`, `elements/KeyGrid.py`, `elements/PageSelector.py`, `elements/DeckConfig.py`; screenshots https://streamcontroller.core447.com/assets/screenshots/main_screen.png and https://streamcontroller.github.io/docs/latest/assets/screenshots/StreamController_2.png
- https://gitlab.gnome.org/World/boatswain (GPL-3.0): `src/bs-window.ui`, `src/bs-device-editor.ui`, `src/bs-button-editor.ui`, `src/bs-profile-row.ui`; screenshot via https://flathub.org/apps/com.feaneron.Boatswain
- https://github.com/ninjadev64/OpenDeck (GPL-3.0): `src/routes/+page.svelte`, `src/components/{DeviceView,Key,PropertyInspectorView,InstanceEditor,ActionList,ProfileManager,DeviceSelector}.svelte`; screenshots `.github/readme/mainmenu.png`, `multiaction.png`
- https://github.com/the-via/app (GPL-3.0): `src/components/panes/configure.tsx`, `configure-panes/*`
- https://github.com/vial-kb/vial-gui (GPL-2.0): `src/main/python/main_window.py`, `editor/keymap_editor.py`; screenshot and tab description https://docs.splitkb.com/resources/firmware/vial/usage-guide; layers https://get.vial.today/manual/layers.html
- https://github.com/bitfocus/companion (licence per repository file; GitHub reports NOASSERTION): `docs/user-guide/3_config/buttons/**`; https://companion.free/user-guide/v5.0/config/buttons/creating/

Proprietary apps:

- https://configure.zsa.io/voyager/layouts/default/latest/0 (Oryx public layout, viewed in the browser) and https://www.zsa.io/oryx
- https://www.provideocoalition.com/loupedeck-rethinks-their-software-with-a-big-update/ (Loupedeck 4 screenshots)
- https://www.provideocoalition.com/review-logitech-mx-creative-console/ (Options+ screenshots with the MX Creative Console)
- https://www.logitech.com/en-us/software/logi-options-plus.html

Project files read first: `docs/design/critique-2026-10.md` sections 5, 8 and 9; `docs/PRODUCT.md`; `docs/design/gui-ux.md` sections 1 to 3; `app/src/prototype/ProfilesVariant{A,B,C}.svelte` (layout grids only).

## 9. Saved screenshots

Saved locally during the study and not committed (proprietary product images); the sources:

| File | Source |
|---|---|
| `elgato-app-sdk.png` | https://docs.elgato.com/img/streamdeck/sdk/your-first-plugin-in-stream-deck.png |
| `elgato-sdk-default-profile.png` | https://docs.elgato.com/img/streamdeck/sdk/default-profile.png |
| `elgato-smartprofile-2.png`, `-4.png`, `-5.png` | https://d32bbxx72p2025.cloudfront.net/images/Explorer_2.width-702.format-webp.webp, `Explorer_4`, `Explorer_5` (elgato.com Smart Profiles article) |
| `elgato-sdplus-setup-actions.png`, `elgato-sdplus-personalize.png` | elgato.com Stream Deck + quick start guide images |
| `streamcontroller-main.png` | https://streamcontroller.core447.com/assets/screenshots/main_screen.png |
| `streamcontroller-docs-1.png`, `-2.png` | https://streamcontroller.github.io/docs/latest/assets/screenshots/StreamController_1.png, `_2.png` |
| `boatswain-main.png` | Flathub screenshot of com.feaneron.Boatswain |
| `opendeck-mainmenu.png`, `opendeck-multiaction.png` | https://raw.githubusercontent.com/ninjadev64/OpenDeck/main/.github/readme/ |
| `vial-keymap.png` | https://docs.splitkb.com/img/hlc-vial-keymap.png |
| `oryx-voyager-default-layout.jpg` | browser screenshot of https://configure.zsa.io/voyager/layouts/default/latest/0 |
| `loupedeck-v4-ppro.jpg` | https://www.provideocoalition.com/wp-content/uploads/Loupedeck-update-ppro.jpg |
| `optionsplus-main.jpg`, `optionsplus-knob.jpg` | https://www.provideocoalition.com/wp-content/uploads/Logi-Options-000662-main.jpg, `Logi-Options-000660-knob-roller.jpg` |
| `companion-button-actions.png`, `companion-pages.png`, `companion-grid-toolbar.png`, `companion-action-group-sequential.png`, `companion-button-notes.png` | https://raw.githubusercontent.com/bitfocus/companion/main/docs/user-guide/3_config/buttons/ |
| `via-readme.png` | the-via/app README image (logo only, not useful) |

The `.webp` originals of the Elgato images sit next to their `.png`
conversions.
