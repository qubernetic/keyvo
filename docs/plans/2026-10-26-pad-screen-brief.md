# Design brief: the Pad and Profiles screens on the Stream Deck idiom

Status: grilled and approved in chat on 2026-10-26 (eleven decisions, below).
Slice 4 of the GUI design pass (`2026-10-26-gui-design-pass.md`, #103,
#107). The `impeccable shape` brief that #108 (Pad) and #109 (Profiles)
build from; `docs/design/gui-ux.md` §2 and §3 carry the same decisions as
the design truth, this file carries the reasoning and the states.

## 0. What happened before

The three Profiles variants of the first slice 4 (draft PR #116, branch
`feature/107-profiles-variants`, kept as the primary source and never
merged) were rejected by the owner as chaotic: they were generated without
a shape brief and each rearranged the same overload. The prior-art study
`docs/research/2026-10-26-keypad-app-layouts.md` followed, and the owner
ruled: the Elgato Stream Deck desktop app is the strongest analogy, its
hierarchy is adopted and adapted to keyvo's content; its visual style
stays an anti-reference (`docs/PRODUCT.md`).

## 1. Feature summary

Two screens replace the pre-alpha Home and Profiles. **Pad** is the main
screen: the device drawn as itself with the live mirror in it, the pages
under it, the inspector of the selected key below, the actions catalogue
on the right, the trace strip at the bottom. **Profiles** is the profile
manager: the list, the header and the match rules of a profile, its pages
as thumbnails, and a jump to the Pad. The key grid exists once, on the Pad.
For the developer who owns the Keypad today and the junior who gets one
later.

## 2. Primary user action

Select a key on the mirror and change what it does, watching the tile
change under the cursor and the pad change on the desk.

## 3. Design direction

- **Colour strategy:** Restrained (PRODUCT.md, DESIGN.md Borrowed Palette):
  tinted neutrals from the desktop theme, the accent as an outline, the
  state colours with an icon or a word. One per-surface override: the
  device drawing carries its own body colour token (`--kv-device`, §7 of
  gui-ux.md) because the pad is a physical object, not a panel.
- **Scene sentence:** a developer at a desk under the room's light, the
  Keypad beside the keyboard, the app open on the second monitor next to
  the editor they are configuring it for, alt-tabbing between the two,
  wanting the app to look like the rest of Plasma and the pad on screen to
  look like the pad on the desk.
- **Anchors:** the Elgato Stream Deck app for the hierarchy (device canvas,
  profile and page above, property inspector below, actions list right);
  Boatswain for the native composition (libadwaita, one framed device, a
  form, a list); VS Code Settings for the form register (labels at 0.85 em,
  48ch inputs, one fact per line).
- No visual direction probes: the harness has no native image generation,
  and the owner draws the device SVG themselves.

## 4. Scope

Production-ready, the two screens and the shell changes they need, shipped
quality with component tests; polished until the critique re-run passes
(the score to beat is 23/40). The actions catalogue ships in v1 with its
groups and the raw tools; its preset content is a separate slice.

## 5. Layout strategy

### 5.1 Shell (decision Q7)

- Rail: Pad, Profiles, Behaviors (the product map; Behaviors stays
  "Coming soon" until M4a-5 but is the destination of the catalogue). The
  other entries leave the rail.
- Header: app name, device selector (every device the daemon reports, then
  Simulator), the sandbox switch with Apply and Discard while the sandbox
  is on, a gear that opens Preferences (tabs General, Devices,
  Integrations, Projects; "Coming soon" lives there, one sentence per tab),
  and the overflow menu as today.
- Window: default 1200 by 760, minimum 960 by 640. Below 1100 px the rail
  drops its labels; below 1200 px the catalogue column collapses into the
  inspector's Action select. Nothing else competes horizontally.

### 5.2 Pad (decisions Q1, Q3 to Q6, Q8, Q9)

```
+----+-----------------------------------------------------------+-------------+
|rail| Keypad v   Profile: vscode v         [sandbox] Apply Discard | Actions    |
|    |                                                           | [search   ] |
|    |      +---------------------------------------+            | v General   |
|    |      |  device drawing (SVG, device colour)  |            |   Open fold |
|    |      |   [git ] [test] [run ]                |            |   Open URL  |
|    |      |   [ 4  ] [ 5  ] [ 6  ]  mirror tiles  |            |   New email |
|    |      |   [ 7  ] [ 8  ] [ 9  ]  in the cutouts|            |   Chord     |
|    |      |        [ < ]  [ > ]    page buttons   |            |   Exec      |
|    |      +---------------------------------------+            | v System    |
|    |   Pages  [context] [git] [test] [+]      Accent [sw]      |   Lock      |
|    |   The pad moved to vscode, page 1   (Follow)              |   Volume    |
|    |-----------------------------------------------------------| v Apps      |
|    | Key 5: Exec   vscode.toml line 42         [Press] [Clear] |   Spectacle |
|    | [tile]  Label  [Run tests       ]   Icon   [play      v]  |   VS Code   |
|    |         Colour [sw] #2a6f4e         Action [Exec      v]  |   Konsole   |
|    |         Command [just test        ]  Cwd  [~/Git/keyvo ]  |             |
|    |-----------------------------------------------------------|             |
|    | Trace v  12:01:03 key 5 exec ok 12 ms | 12:00:58 ...      |             |
+----+-----------------------------------------------------------+-------------+
```

- **Device drawing (Q9).** The Keypad as a stylised SVG drawn by the owner
  from the device's proportions (rounded body, the 3 by 3 key cutouts, the
  two page buttons under them, no vendor mark), coloured from
  `--kv-device` (graphite by default; pale grey and white selectable in
  Preferences > Devices for the other editions), the key rims and the page
  buttons as shades derived from it. The mirror tiles, rendered by
  keyvo-core, sit in the cutouts at the geometry rects, scaled with the
  drawing: `width: 100%`, `max-width` the panel's pixel width, the device
  aspect ratio, tiles positioned by percentage. Never above 1:1, never
  clipped (critique P0). The Dialpad gets its own drawing in the Dialpad
  slice.
- **Mirror.** Shows the pad: the active profile and page, live (the mirror
  is the truth). A tile that falls through from the global layer is dimmed
  with a corner mark (Q4). The selected tile has the accent outline. Key
  feedback states draw on the tile with an icon or a word, never colour
  alone. Click selects; arrows, 1 to 9, Space and Shift+Space as today.
- **Pages (Q5).** Named tabs under the drawing (`context` first, then the
  profile's pages by name, "+" adds a page), the accent swatch with its
  label at the right. The hardware page buttons page the pad only; the
  app's page selection stays where the user put it.
- **Pinned inspector (Q2, Q5).** The profile dropdown in the header pins
  the inspector to a profile; the page tab pins the page; the mirror keeps
  following the pad. When the pad and the editor differ, one line under
  the page tabs says "The pad moved to vscode, page 1" with a Follow
  control; it disappears when they agree. Daemon-side, #117 holds the
  context while the keyvo window has focus, so the line is rare.
- **Inspector.** Header "Key 5: Exec" with the file path and line as the
  subtitle (the three facts of the old key card, once), Press (dry run)
  and Clear at the right. One row: the tile at the left, then Label, Icon
  (picker with search over the bundled lucide names, the name as text
  under it), Colour (swatch and `#rrggbb` text). Then the Action select
  and the kind's fields (Chord: a record field and the text; Exec: Command,
  Cwd, hold). A key from a preset shows the preset's "What it does" select
  above the fields. Forms capped at `48ch`; written on change through
  `toml_edit` as today, the sandbox deciding whether the write reaches the
  real file. The result line of the write sits under the fields in the
  body colour, not muted, with a time stamp.
- **Actions catalogue (Q3, Q8).** A fixed-width right column: a search
  field, then collapsible groups. v1 groups: **General** (Open folder,
  Open website, New email, and the raw tools Chord and Exec at the end),
  **System** (Lock the desktop, Volume, Brightness, Open a settings page),
  **Apps** (one entry per app: Spectacle, Calculator, VS Code, Konsole, and
  so on; dropped on a tile the inspector asks "What it does": Spectacle
  offers full screen, region, recording; VS Code the palette, build, run).
  Later groups: **Dialpad** (key plus dial combinations: a key selects
  volume, brightness or a Home Assistant light, the dial adjusts it) and
  **Behaviors**. Dragging an entry onto a tile assigns it there; clicking
  assigns it to the selected tile. Moving a key stays `Ctrl+X` and `Ctrl+V`
  plus a "Move key" menu entry (d15). The presets are the recipes of
  gui-ux.md §4.3, TOML packs under `contrib/recipes/`; the column's v1
  content is its own slice after #108.
- **Trace strip (Q6).** One line under the inspector with the newest
  entries, newest first, a chevron that opens the drawer of gui-ux.md §8
  (Trigger, Context snapshot, Resolved action, Result or Block reason).
  Press and its result share one line of sight.

### 5.3 Profiles (decisions Q1, Q10)

```
+----+---------------------------+-------------------------------------------+
|rail| Profiles        [+]       | VS Code                     [Open in editor]|
|    |---------------------------| ~/.config/keyvo/profiles/vscode.toml      |
|    | Global          (pinned)  |                                           |
|    | VS Code  [sw]  on the pad | Name     [VS Code       ]   Enabled  [x]  |
|    | Konsole  [sw]             | Accent   [sw] #2a6f4e                      |
|    | Firefox  [sw]             | Priority [20]  higher wins the window     |
|    |                           |                                           |
|    |                           | Match rules                     [+ rule]  |
|    |                           |   class  code                      [x]    |
|    |                           |   title  Visual Studio Code$       [x]    |
|    |                           |                                           |
|    |                           | Pages  [ctx] [git] [test]  thumbnails     |
|    |                           | Edit keys on the Pad  ->                  |
+----+---------------------------+-------------------------------------------+
```

- List: profiles by priority, the global one pinned first, a 12 px accent
  square after the name, "on the pad" on the live one, "+" from a template
  (empty, copy of the active, starter pack).
- Editor: the header (Name, Accent swatch and text, Priority with "higher
  wins the window", Enabled), the file path and "Open in editor" in the
  heading row, Match rules as one list of kind and pattern rows with
  inline validation ("+ rule" and the row editing stay "Coming soon" until
  M4a-4), the pages as small thumbnails rendered by keyvo-core, and "Edit
  keys on the Pad", which opens the Pad with this profile and page pinned.
- No key grid, no key editor, no right column.

## 6. Key states

| State | Pad | Profiles |
|---|---|---|
| Default | Drawing with the live tiles, the pad's profile in the dropdown, no key selected: the inspector shows one sentence, "Select a key to edit it" | The pad's profile selected, its header and rules |
| Loading | Skeleton tiles in the cutouts, the dropdowns disabled | Skeleton rows |
| Daemon unreachable | The drawing with empty cutouts, one notice with a full border and a leading word: the socket path, the OS error, and how to start the daemon | The list from the last read if any, else the same notice |
| No Keypad (Simulator) | The drawing in the Simulator's geometry, the header says Simulator | Unchanged |
| Pad moved | The mirror changes, the "pad moved" line appears with Follow; the inspector and its typing stay | The "on the pad" mark moves |
| keyvo focused (#117) | No change on the pad; the context fact in the status area reads "keyvo-app focused, holding org.kde.konsole" | Unchanged |
| Global fall-through key selected | Dimmed tile with the corner mark; the inspector subtitle says "from global", an "Override here" action copies the key into the pinned profile | n/a |
| Write succeeded | The result line with the time, the tile re-rendered, a trace entry | The result line under the header |
| Write failed (schema, io) | A notice with a full border and a leading word, the file, line and column, the field keeps the typed value | The same |
| Invalid field | The field's own one-line message under it, nothing written | The same |
| Sandbox on | The header carries the sandbox token; Apply and Discard visible; writes go to the sandbox copy; a Discard asks for confirmation (the one modal) | The same header |
| Profile with no pages | The tabs show "+" only, the drawing shows the global page dimmed | "This profile has no pages" and the "+" |
| Empty catalogue search | "No action matches" under the field | n/a |
| Narrow (960 to 1200 px) | The catalogue collapses into the Action select; the drawing scales down | The list column stays until 1000 px |

## 7. Interaction model

1. Open the app: the Pad shows what the pad shows. The dropdown reads the
   pad's profile; nothing is selected.
2. Click a tile (or press 1 to 9): the tile gets the outline, the
   inspector fills; the subtitle names the file and line and, for a
   fall-through key, the origin.
3. Change a field and leave it (Tab or click elsewhere): the write runs,
   the result line and the tile update, the pad repaints, a trace entry
   arrives. Undo and Redo as today.
4. Drag "Spectacle" from the catalogue onto tile 7: the tile takes the
   preset's icon and label, the inspector opens on it with "What it does"
   set to the preset's default; change it to "Region".
5. Press "Press": a dry run, the result in the trace strip; the chevron
   opens the drawer for the four sections.
6. Alt-tab to VS Code and back: with #117 the pad holds; without it the
   mirror follows and the "pad moved" line offers Follow. The inspector
   never changes under the user.
7. Choose another profile in the dropdown: the inspector pins to it, the
   tabs show its pages, the mirror keeps the pad; the "pad moved" line
   states the difference.
8. Go to Profiles, click "Edit keys on the Pad" on Konsole: the Pad opens
   pinned to Konsole, page 1.

Keyboard: the rail's `Ctrl+1` to `Ctrl+3`; on the Pad `1` to `9`, arrows,
Space (dry run), Shift+Space (long), `Ctrl+Z` and `Ctrl+Shift+Z`, `Ctrl+X`
and `Ctrl+V` between tiles, `/` to the catalogue search, Escape closes the
drawer. Every control keeps `aria-keyshortcuts`; the keyboard map gets a
"Keyboard shortcuts" entry in the overflow menu (critique heuristic 6).

## 8. Content requirements

- Inspector header: "Key {n}: {kind}" with the kind's display name (None,
  Chord, Exec, or the preset's name); subtitle "{file} line {line}" and
  "from global" when it applies; buttons "Press", "Clear", "Override here".
- Empty inspector: "Select a key to edit it."
- Pad moved line: "The pad moved to {profile}, page {page}" and "Follow".
- Context hold: "{class} focused, holding {profile}" (#117).
- Catalogue groups and entries as in §5.2; search placeholder "Search
  actions"; empty: "No action matches".
- Result line: "Wrote key {n} to {file} at {time}"; failures as the
  daemon's diagnostic with file, line and column.
- Unreachable: the socket path, the error, and "Start it with
  `keyvo-daemon`" (one sentence, the command in code).
- Profiles: "higher wins the window" under Priority; "The global profile
  is under every window" instead of rules for the global one; "Edit keys
  on the Pad".
- All strings in `app/src/lib/i18n.ts`; no literal words in markup (the
  strings test).
- Dynamic ranges: 1 to 9 keys (geometry-driven), 1 to 10 pages, 1 to about
  20 profiles, catalogue entries about 30 in v1, trace entries 20 in the
  strip and the ring buffer's size in the drawer.

## 9. Recommended references

From the impeccable skill: `reference/product.md` (register),
`reference/interaction-design.md` (forms, drag and drop, keyboard),
`reference/spatial-design.md` (the two-level layout, the collapsing
column), `reference/ux-writing.md` (the result and error lines). From the
repository: `docs/DESIGN.md` (Outline Accent, Muted Label, Flat Panel, One
Level, the Don'ts), `docs/design/gui-ux.md` §5 to §8, `docs/design/critique-2026-10.md`
§5 (the priority issues each slice closes).

## 10. Open questions for the implementer

- The device SVG's geometry contract: the owner's drawing exposes the key
  cutouts and the page buttons as named groups; the component maps the
  daemon's geometry rects onto them by index, and a drawing whose cutout
  count differs from the geometry falls back to the plain panel.
- The Simulator's drawing: the same SVG with the Keypad's default geometry
  until a Simulator geometry exists.
- Whether "Clear" writes an empty key or removes the key table from the
  file (the editor client decides; the file must round-trip).
- The icon picker's list: the bundled lucide names of `keyvo-core` through
  the editor client, or a static list generated at build time.
- `--kv-device` and the derived rim shades: the derivation lives in the
  component's CSS from one token; §7 records the token and its default.

## 11. Decisions of the grill (2026-10-26)

| # | Decision |
|---|---|
| Q1 | Two screens: Pad (the only key grid) and Profiles (the manager). |
| Q2 | The keyvo window never changes the context: `[focus].hold_classes` in the daemon (#117); the inspector is pinned in the GUI. |
| Q3 | The actions catalogue column ships in v1 with groups General, System, Apps; Dialpad combinations and Behaviors later. |
| Q4 | Global fall-through tiles dimmed with a corner mark, "from global" in the subtitle, "Override here". |
| Q5 | Named page tabs; the hardware page buttons page the pad only. |
| Q6 | The trace is a strip under the inspector opening the drawer. |
| Q7 | Rail: Pad, Profiles, Behaviors; Preferences behind a gear for the rest. |
| Q8 | Drag and drop from the catalogue onto tiles in v1; key moves stay `Ctrl+X` and `Ctrl+V`. |
| Q9 | The device drawn as a stylised SVG by the owner, coloured from a token, the tiles in its cutouts. |
| Q10 | Profiles as sketched in §5.3; rule editing stays "Coming soon" until M4a-4. |
| Q11 | Slices: #107 this brief and the spec rewrite; #108 Pad; #109 Profiles; the SVG integration (#118) and the catalogue v1 (#119) as sandbox slices; #116 closed, its branch kept. |
