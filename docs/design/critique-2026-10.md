# GUI design critique, October 2026

Slice 3 of the GUI design pass (`docs/plans/2026-10-26-gui-design-pass.md`,
#103, #106). The critique of the app shell, Home and Profiles as built by
#100 to #102, run on the mock dev server of #113 in a host browser on
2026-10-26 at `bfdf6cf` (the epic after #112, #113 and #114). It folds in
the owner's findings of the pre-alpha run of the same morning
(`~/keyvo-prealpha-2026-10-26/findings.md`, items 0 and 1) and sets the
priorities of slices 4 to 6 (#107, #108, #109). The method is the
`impeccable critique` command: two assessments that never saw each other,
woven into one report.

- **Assessment A**, an LLM design review in its own browser tab: the
  sources under `app/src/`, the live screens in `breeze-dark`,
  `breeze-light` and `adwaita-dark`, the `?mock-error=unreachable` state,
  and an emulated 1100 by 700 window (the window manager kept the browser
  at 2560 by 1313, so `innerWidth` and `innerHeight` were overridden and a
  resize dispatched, which is what `App.svelte` and `lib/shell.ts` read).
- **Assessment B**, the deterministic detector: the CLI scan of the sources
  (`npx impeccable --json app/src`, impeccable 4.7.0) and the browser
  detector (`cli/engine/detect-antipatterns-browser.js` of the impeccable
  checkout at `e587004`) injected into the pages in a second tab, in
  `breeze-dark`, `breeze-light` and `adwaita-light`.

Line numbers refer to `app/src/` at `bfdf6cf`. The preview tiles of the
mock all read "Commit" by design (snapshot stand-ins) and are not critiqued.

## 1. Design health score

Nielsen's ten heuristics, 0 to 4 each (the scale of the impeccable
`heuristics-scoring` reference).

| # | Heuristic | Score | Key issue |
|---|---|---|---|
| 1 | Visibility of system status | 3 | A write confirms only in a muted 0.85 em line under the form (`KeyEditor.svelte:299`, `Profiles.svelte:392`); no in-flight state; the editor retargets silently when the active profile changes. |
| 2 | Match between system and real world | 3 | TOML vocabulary everywhere ("chord", "exec", "Mode auto", "Layer origin: app layer", "Title regex"); "unknown project," prefixed to every Context value (`Home.svelte:318-323`). |
| 3 | User control and freedom | 2 | Profile name and accent are written on blur with no undo and no cancel (`Profiles.svelte:160-181, 354-372`); the Home editor's target and the Profiles selection follow the pad unless a profile was picked (`Home.svelte:343-347`, `Profiles.svelte:101-105`); "Page shown" snaps back on every view update (`Home.svelte:130-133`). |
| 4 | Consistency and standards | 2 | The same `KeyEditor` is a 260 px panel on Home and a full-width form on Profiles; a `role="switch"` drawn as a plain button (`Home.svelte:497-507`); three disabled affordances ("+", "Edit match rules", "Move key"); `<code>` against the body font; link-buttons against buttons. |
| 5 | Error prevention | 2 | Colour, icon and chord are free text validated after the write attempt (`lib/i18n.ts:212-214`); no swatch before an accent is written; no list for lucide names; no notice when the file changed on disk. The one guard is `home.otherPage` (`Home.svelte:204-209`). |
| 6 | Recognition rather than recall | 2 | The keyboard map (1 to 9, Space, Shift+Space, arrows, Ctrl+1 to 7, Ctrl+Z) exists only as `aria-keyshortcuts`; icon and chord syntax must be recalled; no field hints; the "global" badge on a tile is unexplained. |
| 7 | Flexibility and efficiency | 3 | Good accelerators (`Home.svelte:239-250`, `Profiles.svelte:207-222, 263-272`, `KeyEditor.svelte:191-205`); no record-a-chord, no bulk edit. |
| 8 | Aesthetic and minimalist design | 2 | Clipped preview with a scroll bar; inputs the width of the column; "File:" twice, "Window class" twice, "Key 2" twice; up to nine "Coming soon" controls on Profiles; Home uses under half of a wide window. |
| 9 | Recognise, diagnose, recover from errors | 3 | Schema errors name file, line and column and link to Profiles (`Home.svelte:328-341`); the unreachable banner names the socket and the OS error but not how to start the daemon; "Select a key on the mirror to edit it." shows when there is no mirror. |
| 10 | Help and documentation | 1 | No help anywhere: no field hints, no link to the schema or the docs, no explanation of chord syntax, mode, layer or dry run; only the roadmap sentences on disabled controls. |
| **Total** | | **23/40** | **Acceptable: significant improvements needed before users are happy** |

Cognitive load (the reference's eight-item checklist): 5 of 8 fail at
1200 px and wider, 6 below 1200 px, where the preview column collapses
(`lib/shell.ts:49`). The failures: single focus (an events list inserts a
row every 2 s and a profile swap every 10 s redraws the mirror and the
editor), chunking (six facts in one status strip row, five stacked sections
in the Profiles editor column), visual hierarchy (every heading is 1 em
semibold, every body line 1 em regular, the loudest element on Profiles is
the Name input), one thing at a time (a selected key opens the editor in
one column and the card in another), minimal choices (rail 7, strip 6, key
editor 8, Profiles editor column up to 27 interactive elements). Progressive
disclosure passes (the chord or command field appears only for its action,
`KeyEditor.svelte:259-272`; the trace detail is behind one click).

## 2. Anti-patterns verdict

**LLM assessment.** At arm's length the app passes: system font, two
surfaces, 1 px borders, no gradients, no glass, no hero metric, no modal, a
drawer instead of a dialog. Someone glancing at the `breeze-dark` Home would
say "a KDE settings page", which is what PRODUCT.md asks for. The structural
choices are honest: the nine tiles are renderer output at 1:1
(`Home.svelte:449-457`), "Coming soon" controls stay visible and disabled
with a roadmap sentence instead of a fake (`Soon.svelte`,
`Rail.svelte:66-77`), skeleton rows replace spinners (`Home.svelte:509-515`).
Up close the details say "generated", and most of them are the items
DESIGN.md already lists under Don'ts: the side stripe on every notice and
list row, the accent as text colour, a `background` on every text span, the
same `gap: 8px` in every container, forms stretched to the column, paths in
`<code>` monospace against DESIGN.md's "paths stay in the body font", and the
clipped preview, which is the pre-alpha anti-reference of PRODUCT.md still on
screen. "AI made this" would not be the first sentence; "nobody has looked at
this in light mode or below 1200 px" would be.

**Deterministic scan.** The CLI finds 3 warnings, all `side-tab` (slop):
`Home.svelte:558`, `KeyEditor.svelte:347`, `Profiles.svelte:731`, each
`border-left: 4px solid var(--kv-state-error)`. Unchanged since `2e3409b`.
The browser detector per screen and theme:

| Screen, theme | Findings | What |
|---|---|---|
| Home, breeze-dark | 3 | `single-font` (Noto Sans only); `flat-type-hierarchy` (11, 11.3, 13.3, 16 px, ratio 1.5:1); `cramped-padding` on the trace rows (`button.entry`, 0 px vertical padding for 13.3 px text, need 4 px) |
| Profiles, breeze-dark | 2 | `single-font`; `flat-type-hierarchy` (11.3, 13.3, 16 px, ratio 1.4:1); the same with a key selected |
| Profiles, breeze-light | 11 | the two above plus 9 `low-contrast`: 3.7:1 for `--kv-fg-muted` on `--kv-bg` (Id, Priority, File:, the match-rule note, "Select a key to see its card"), 3.3:1 on `--kv-bg-alt` (the muted labels of "+", "Edit match rules", "Move key", "Open in Behaviors") |
| Home, breeze-light | 18 | the three of Home plus 15 `low-contrast`: 3.7:1 on the strip labels, 2.2:1 for the accent links ("global", "1", "auto") |
| Profiles, adwaita-light | 11 | `single-font` (Adwaita Sans); `flat-type-hierarchy` (11, 12.5, 14.7, 16, 17.6 px, ratio 1.6:1); 9 `low-contrast` at 2.8:1 and 3.2:1 |

False positives and agreements: `single-font` is by design (P1 native
theme, DESIGN.md "Two Weights"); the flat scale is partly by design ("No Big
Number"), but the 11 versus 11.3 px pair is a unit inconsistency worth
fixing. The contrast failures are the values #113 recorded in
`gui-ux.md` section 7 as "measured, not tuned"; the detector confirms them
on the live pages. The detector did not flag the 6 px profile-row swatch
(an element, not a border) nor the `.problem` stripe (no problem shown on
the mock); the LLM review did. Both assessments agree on the stripes, the
contrast and the type scale; the LLM review alone found the editor
retargeting, the duplicated facts and the layout faults; the detector alone
found the trace rows' padding.

**Visual overlays.** The browser detector draws its findings on the page.
To see them: `just app-dev`, open `http://localhost:1420/` in a browser,
serve the detector (`python3 -m http.server 4972 --bind 127.0.0.1` in
`cli/engine/` of the impeccable checkout) and run in the console:

```js
const s = document.createElement("script");
s.src = "http://127.0.0.1:4972/detect-antipatterns-browser.js";
document.head.appendChild(s);
```

`window.impeccableScan()` re-scans after a navigation inside the app.

## 3. Overall impression

The first open in `breeze-dark` is calm and native, "a settings page, not a
product", which is the intended feeling. The peak comes a second later when
the nine tiles appear at 1:1 with the "global" badge on the fall-through
keys: "that is my pad". The first click delivers again: a one-line result and
a drawer whose four sections read in resolution order (P4 felt, not
described). Then the anxiety starts: edits are written to disk on blur with
the quietest text on the screen as the only confirmation, the profile header
writes with no undo at all, and while the user types the editor can change
file under the typing. The end of the journey is the Profiles screen in any
theme: a clipped preview with a horizontal scroll bar, a lone "context" tab,
nine "Coming soon" controls, and inputs the width of the monitor. Peak-end
rule: the peak is excellent and the end says unfinished. The single biggest
opportunity is the Profiles layout: one hierarchy, the preview as the
instrument, the editor beside what it changes.

## 4. What's working

1. **The mirror is the truth, literally.** Tiles from the renderer at the
   device's tile size (`Home.svelte:449-457`), the global fall-through
   marked on the tile (`Home.svelte:196-199, 458-460`), the selected tile
   with the accent outline (`Profiles.svelte:690-693`), a real `role="grid"`
   with roving tabindex, arrow keys, number keys, Space and Shift+Space
   (`Home.svelte:239-250, 439-469`).
2. **The trace model is right.** One summary line per press
   (`lib/home.ts`), then a drawer in resolution order: Trigger, Context
   snapshot, Resolved action, Result or Block reason
   (`TraceDrawer.svelte:24-114`), non-modal, Escape to close, focus
   returned (`Home.svelte:223-232`). Errors name file, line and column
   (`Home.svelte:328-341`) and the socket path.
3. **The discipline.** No literal colour in any component, skeleton rows,
   `role="status"` for results, `aria-keyshortcuts` on every accelerator,
   "Coming soon" stated and never faked (`Soon.svelte`), i18n through one
   table with a throwing `t()` (`lib/i18n.ts:222-228`). A base worth
   polishing, not replacing.

## 5. Priority issues

Severity per the impeccable scale: P0 blocks the task, P1 causes significant
difficulty, P2 is an annoyance with a workaround, P3 is polish.

### [P0] The page preview is clipped (findings.md item 1a)

- **What:** the right column is 320 px (`Columns.svelte:121-124`) and the
  preview grid renders tiles at device size, 3 x 118 px plus two 8 px gaps
  = 370 px (`Profiles.svelte:493-521`); the third column is cut and a
  horizontal scroll bar appears. Observed in all themes at every width.
- **Why it matters:** it is the signature component and the named
  anti-reference (PRODUCT.md "Anti-references", DESIGN.md "Don'ts").
- **Fix:** lay the preview out as one image in device proportions: a wrapper
  at `width: 100%; max-width: <panel px>; aspect-ratio: <panel>`, tiles
  positioned by percentage from the geometry rects, so it scales down and
  never exceeds 1:1; `scrollbar-gutter: stable` on the column. Slice 5
  (#108); the three variants of slice 4 (#107) must each solve it.

### [P1] The editor follows the focused window and swaps the file under the user (new finding 2)

- **What:** with no profile picked, `selected` falls back to the pad's
  profile (`Profiles.svelte:101-105`) and the identity change resets page
  and key (`Profiles.svelte:113-119`); on Home the key editor's target is
  `home.view.profile` (`Home.svelte:343-347`) and a target change replaces
  the form without keeping edits (`KeyEditor.svelte:106-122`). Observed on
  the mock: an accent typed into Global landed in `konsole.toml` after the
  focus cycle moved the active profile mid-edit; Profiles dropped the
  selected key and its card; on Home the editor went from `konsole.toml`
  key 2 to `vscode.toml` key 2 while the result line still described the
  Konsole chord.
- **Why it matters:** in the real app the focused window is keyvo itself
  whenever the user looks at it, so this fires on every alt-tab, and
  half-typed text is discarded or written to the wrong file silently.
- **Fix:** the mirror follows the pad, the editor does not. Pin the editor
  to the chosen profile, page and key; when the pad moves, show a one-line
  notice next to the mirror ("The pad moved to vscode, page 1") with a
  "Follow" control; keep unsaved typing until blur or cancel. Profiles in
  slice 5 (#108), Home in slice 6 (#109).

### [P1] Light themes fail the contrast the product promises

- **What:** measured live in `breeze-light`, `--kv-fg-muted` is 3.7:1 on
  `--kv-bg` and 3.3:1 on `--kv-bg-alt`; the accent as text 2.2:1 and
  2.0:1; `adwaita-light` 3.2:1 and 2.8:1 (gui-ux.md section 7). Muted is
  every field label (`KeyEditor.svelte:321-328`,
  `Profiles.svelte:639-646`), every strip label (`Home.svelte:568-572`),
  every path, hint, result and "Coming soon"; the accent is every status
  link (`Home.svelte:578-584`).
- **Why it matters:** PRODUCT.md promises 4.5:1 for every text pair in the
  native light and dark schemes; half the words fail on the owner's second
  theme.
- **Fix:** the theme bridge (M4a-1, out of this pass) derives
  `--kv-fg-muted` by mixing the desktop's inactive colour toward `--kv-fg`
  until 4.5:1 holds on both surfaces; this pass drops the accent from text
  (links in `--kv-fg` with an underline, which also satisfies the Outline
  Accent rule) and records the derivation rule in gui-ux.md section 7 as a
  token change. Slices 5 and 6.

### [P1] The banned side stripes and the accent as fill are still shipped

- **What:** `.error` 4 px left border (`Home.svelte:557-559`), `.problem`
  4 px left border (`Profiles.svelte:731`, `KeyEditor.svelte:347`), the
  6 px full-height row swatch (`Profiles.svelte:589-595`), and the Home
  accent bar, a filled `flex: 1` 6 px stripe beside "Page shown" that reads
  as a progress bar with no visible label (`Home.svelte:430-437, 627-632`).
- **Why it matters:** absolute bans of the design laws and explicit Don'ts
  of DESIGN.md; the strongest "generated" tell.
- **Fix:** notices get a 1 px full border and a leading word or icon in the
  state colour (DESIGN.md "Problem"); the profile accent becomes a 12 px
  square swatch after the name in the row and a labelled swatch ("Accent")
  next to the page select on Home. Slices 5 and 6.

### [P2] Forms stretch across the column and the scroll bar sits over them (findings.md items 1b and 1c)

- **What:** no `max-width` on the header form or the key editor in the
  Profiles column (`Profiles.svelte:633-637`, `KeyEditor.svelte:315-319`
  inside the full-width `.card` at `Profiles.svelte:479-481`); the Name
  input measured 1712 px at 2560 px. Below 1200 px the editor column
  scrolls (`Columns.svelte:115-119`) with no `scrollbar-gutter` and the
  track overlaps the right ends of the inputs (the owner's screenshot
  `finding-01-profiles-clipped-preview.png`, WebKitGTK; reproduced at
  1100 by 700 in the browser); selecting a key appends the editor below the
  fold with no scroll into view (`Profiles.svelte:478-482`).
- **Fix:** `max-width: 48ch` on both forms; `scrollbar-gutter: stable` plus
  8 px right padding on every scrolling column (list, editor, right, trace
  drawer); `scrollIntoView({ block: "nearest" })` on key choice. Better:
  the key editor moves into the right column in place of the card, so the
  control sits beside the preview it changes. This is the layout question
  the three variants of slice 4 answer.

### [P2] "Every fact once" (P3 of PRODUCT.md) is broken in six places

"File: path" under Profile (`Profiles.svelte:378`) and again under Key N
(`KeyEditor.svelte:220`); "Window class" repeated per rule
(`Profiles.svelte:406-411`); "Key 2" as the editor heading and "Position:
Key 2" in the card (`Profiles.svelte:534-536`, `KeyEditor.svelte:215`);
"unknown project," prefixed to every Context value (`Home.svelte:318-323`);
the strip's "Page 1" beside "Page shown: Page 1"; "Coming soon" as a span
beside the Debug switch and again in its tooltip (`Home.svelte:497-507`).
Fix: one "Window class" label with the rules as a list; the card's three
facts become the editor's subtitle; drop the project prefix until Projects
exists; hide the switch until the daemon sends HID events. Slices 5 and 6.

### Closed: the app-only sandbox could not reach the host daemon (findings.md item 0)

Fixed by #112 (#104): the peer check reads the raw `ucred` and tolerates
pid 0. Verified on the host on 2026-10-26 with the app in the GNOME runtime
against the host daemon.

## 6. Persona red flags

- **Alex, the impatient power user:** the keyboard map is invisible (only
  `aria-keyshortcuts`); no "press the chord to record it" (the chord is
  typed as text); "Page shown" resets on every pad view
  (`Home.svelte:130-133`); the profile header writes on blur with no undo
  (`Profiles.svelte:160-181`); no way to create a profile ("+" disabled),
  so "Open in editor" is the one real action in the header section.
- **Mia, the junior developer of PRODUCT.md:** the first action is not
  obvious within 5 seconds (a grid, a form, one hint using "dry-run" and
  "long"); Action offers none, chord, exec (`KeyEditor.svelte:254-256`),
  Icon wants `lucide:hammer`, Color wants `#rrggbb`, no picker or list;
  why the pad shows what it shows is split between the strip ("Profile
  konsole") and Match rules on another screen, with "Mode auto", "Layer
  origin: app layer" and the "global" badge never explained; no help
  affordance anywhere.
- **Sam, who depends on accessibility:** every muted label and link fails
  contrast in both light themes; the pressed-in state and the focus ring
  are both the accent (`styles/tokens.css:45-48`, `styles/controls.css:83-89`),
  a blue ring around a blue border; the held-key state is border colour
  only (`Home.svelte:662-664`); `role="gridcell"` with `aria-selected` on a
  wrapper with a button inside (`Home.svelte:443-444`,
  `Profiles.svelte:453-454`) doubles the semantics; the events list
  re-renders every 2 s with no live-region politeness (`Home.svelte:519`);
  fixed 118 px tiles and a 320 px column clip further at 200 % zoom, so the
  1.5x text scale promise of PRODUCT.md cannot hold.
- **Riley, the stress tester:** a profile with zero pages empties the right
  column (`Profiles.svelte:487`); an invalid accent keeps the bad text with
  no sign of which value is on disk; the result line keeps the last result
  forever with no time stamp while rows roll in; the unreachable daemon
  shows three "unknown" links to Profiles and "Select a key on the mirror"
  with no mirror; a long slot label is ellipsised with no `title`
  (`Profiles.svelte:701-706`).

## 7. Minor observations

- Flat type scale (1.2, 1.1, 1, 1, 0.85 em; `styles/tokens.css:24` sets
  14 px while the themes set 10 pt or 11 pt): headings and body the same
  size, weight alone separates them, thin in light.
- `<code>` monospace for paths and rules contradicts DESIGN.md.
- "Debug log" is `role="switch"` drawn as a plain button with no on or off
  visual (`Home.svelte:497-507`).
- The lone "context" tab on single-page profiles looks like a stray button;
  "Pages" has no visible heading (`Profiles.svelte:422-426`).
- List rows change height when active (`Profiles.svelte:574-607`).
- The two column toggles are icon-only; the right one hangs alone above the
  preview (`Columns.svelte:73-82`).
- The overflow menu is five disabled entries; five of seven rail entries are
  disabled on every screen.
- Event times use `toLocaleTimeString()` with AM or PM
  (`Home.svelte:270-272`); 24 h `HH:MM:SS` would align and match the
  audience.
- The trace summary line packs up to seven facts in one comma list; the
  trace rows have no vertical padding (`Home.svelte:706-712`).
- The accent shows only as the Home bar and the row swatch; neither is
  visibly labelled "Accent".
- `color-scheme: dark` is hard-coded in `styles/tokens.css:10`.
- `.key:focus-visible` replaces the outline with a border
  (`Home.svelte:657-660`), a different focus style from every other control.
- Undo and Redo sit under the form; in `breeze-light` their disabled state
  is a muted border on a near-identical surface (1.11:1 between surfaces).
- "Edit match rules" uses `Soon` with `aria-disabled` while "Open in editor"
  uses native `disabled`: two disabled vocabularies.
- Home leaves the right 55 % of a 2560 px window empty while the events
  list runs full width below; no max width, no second column.

## 8. Questions to consider

1. When the owner looks at the app, the focused window is keyvo itself. What
   should the mirror and the editor show then: the profile the app window
   matches (global), the last non-keyvo window, or a frozen frame? Should
   the daemon exclude its own window from matching, and should Home say so?
2. Why does a selected key appear in two places on Profiles, a full editor
   in the middle column and a three-fact card on the right? Which is the
   control and which the gauge; could the right column hold the preview and
   the editor, and the middle column only the profile?
3. Is "written the moment the field changes, no confirm, no undo for the
   header" the contract a developer expects for a file they also edit by
   hand? What does the screen say when the file changed on disk between the
   read and the write?
4. Five of seven rail entries are dead until M4a-5 to M4a-8. Does a legible
   product map justify a navigation that is 71 % "Coming soon" for months,
   or should the rail show the two built screens and one "Roadmap" entry?
5. The tiles are dark on a light desktop because the pad is dark. Should the
   mirror look like the device (a bezel-like frame at 1:1) rather than nine
   buttons on the app's surface, so "the mirror is the truth" is visible
   instead of implied?

## 9. Priorities for slices 4 to 6

Slice 4, three Profiles variants (#107), on the real route behind
`?variant=`. Every variant must answer questions 2 and 4 above and solve
the P0 and the P2 layout issue; the variants differ in where the key editor
lives (beside the preview, under the grid, in a third column), how the
profile header and match rules are grouped, and how the accent is shown.
All three stay inside the tokens of gui-ux.md section 7 and the rules of
DESIGN.md (Borrowed Palette, Outline Accent, Flat Panel, One Level).

Slice 5, Profiles after the chosen variant (#108): the P0, the two P1 items
that touch Profiles (editor pinned to the chosen profile; no stripes, no
accent as text), the P2 items (forms at `48ch`, `scrollbar-gutter`, scroll
into view, the duplicated facts), the trace of the findings report
(items 1a to 1c), and the contrast rule for `--kv-fg-muted` recorded in
section 7 as the bridge's derivation. The losing variants are deleted in
the same PR.

Slice 6, Home after the Profiles language (#109): the editor pinned to its
target with the "pad moved" notice, the accent as a labelled swatch, the
status strip without "unknown project," and without accent-coloured links,
the trace rows with vertical padding and 24 h times, the Debug switch hidden
until it works, the error banner with a full border and a leading word, and
a sentence on how to start the daemon in the unreachable state.

Out of this pass, recorded for later: the theme bridge's muted-colour
derivation (M4a-1), the record-a-chord field, pickers for icon and colour,
help affordances (heuristic 10), the rail with five dead entries (question
4), the keyvo-window focus question (question 1), the 1.5x text scale check
(after the bridge).

A re-run of the critique after slice 6 closes the pass; the score to beat
is 23/40.

## 10. Re-run (2026-10-27)

The closing pass of #130 on `epic/34-pre-alpha` at `aa8e3aa`, after the
harden and polish of the Pad and Profiles screens (commits `cb9dad1`,
`b19296c`, `1956cb4`, `aa8e3aa`) and the slices #108 to #151. Method as in
the introduction: two independent assessments that did not see each other or this
file, the LLM review in its own `[LLM]` tab of the mock dev server (#113)
at 1363 px and 960 px in breeze-dark and breeze-light, the deterministic
detector as the CLI scan (impeccable 4.8.0, `npx impeccable --json app/src`)
and the browser overlay served from the local engine on five pages (the Pad
and Profiles in breeze-dark, breeze-light and adwaita-light, the Pad with
`?mock-error=unreachable`), the overlays left in a `[Human]` tab.

### Design health score

| # | Heuristic | 10-26 | 10-27 | Key issue now |
|---|---|---|---|---|
| 1 | Visibility of system status | 3 | 3 | The pin state is visible only as the missing ", on the pad" suffix of the header dropdown (`Header.svelte:141`); the Profiles editor followed the pad until a row was clicked (fixed the same day: a field taking the focus pins it, `b19296c`). |
| 2 | Match between system and real world | 3 | 2 | Roadmap milestones in user copy ("arrives with M4a-4 on the roadmap"), one of them permanently visible beside Enabled (fixed: `aa8e3aa` drops it from the visible line; the Coming soon tooltips keep theirs); "Chord", "Exec", "app layer", "Title regex", "higher wins the window"; the drawer's time was 12 h locale while the strip is 24 h (fixed, `aa8e3aa`). |
| 3 | User control and freedom | 2 | 3 | Key edits have undo and redo; Profiles name and accent writes have none; a catalogue preset writes on click. |
| 4 | Consistency and standards | 2 | 3 | Two chevron idioms (clip-path triangles beside lucide icons); swatch radius 2 px on Profiles and the theme radius on the tabs; the Profiles write note had no time while the inspector's has (fixed, `aa8e3aa`). |
| 5 | Error prevention | 2 | 2 | Chord and Command are free text checked for emptiness only; Clear wipes a key in one click; no preview of the write. |
| 6 | Recognition rather than recall | 2 | 3 | Below 1200 px the catalogue is an 18-option native select without icons, search or drag; the global tabs explain themselves through `title` only. |
| 7 | Flexibility and efficiency | 3 | 3 | Keyboard map, drag and drop; no bulk edit, move not built. |
| 8 | Aesthetic and minimalist design | 2 | 3 | `lucide:<name>` echoed under the Icon field; Undo and Redo shown disabled in the empty inspector; 18 catalogue entries open at once in 240 px. |
| 9 | Recognise, diagnose, recover from errors | 3 | 3 | The unreachable notice names the socket, the command and Retry; a failed exec in the drawer gives the error but no path to the key's file and line. |
| 10 | Help and documentation | 1 | 2 | The shortcuts drawer and the Coming soon tooltips; no hint for chord syntax, window classes or match rules. |
| **Total** | | **23/40** | **27/40** | **Acceptable, top of the band; the acceptance criterion of #130 (above 23, no P0, no P1 left) is met after the fixes below** |

Cognitive load: 3 of 8 fail at 1200 px and wider (was 5), 4 below 1200 px
(was 6). Chunking (three catalogue groups open at once), minimal choices
(18 catalogue options plus five header controls at the first decision
point), working memory in the "moved" state (the mirror drops the outline
by the #143 decision and the sentence is the only anchor); below 1200 px
also progressive disclosure (the catalogue collapses into one flat select
and the rail drops its labels).

### Anti-patterns verdict

LLM assessment: passes. No absolute ban is hit: the notice is a full 1 px
border in the state colour (section 5, P1 of 10-26, closed by #109), no
gradient, glass, hero metric or card grid, the only floating surface a
non-modal drawer. The product test passes: a Raycast or VS Code Settings
user trusts the shell at once, the device drawing with the live tiles is
the one memorable object. Two spots of vocabulary drift, not slop: the
hand-made clip-path chevrons beside lucide icons everywhere else
(`Catalogue.svelte`, `TraceStrip.svelte`), and the raw `result.status`
words in the drawer (`TraceDrawer.svelte`) outside i18n.

Deterministic scan: the CLI finds nothing in `app/src` (exit 0; 82 files by
regex only, the element rules need a parsed page). The browser overlay
reports four findings per page, the same across pages: `single-font` on
the body (the system font by design, false positive), `flat-type-hierarchy`
(10 to 16 px, ratio 1.6:1: DESIGN.md's five sizes, two weights, by design),
`cramped-padding` on inline spans, paragraphs and the trace strip's entry
buttons (0 px vertical padding on elements that centre their text or are
inline: artefacts), and `tiny-text` on the catalogue's hint at 11.3 px
under the 10 pt Breeze base. The last one was real: the hint and the notes
of the catalogue were set in the 0.85 em label size although they are
sentences; `1956cb4` sets them in the body size as DESIGN.md "Body" says.
The light themes report no `low-contrast` (the 10-26 P1, closed by #144).

### What's working

- The device drawing with the live tiles (`Device.svelte`, `Mirror.svelte`)
  carries the hierarchy on its own and makes the Profiles thumbnails
  legible at a glance.
- The unreachable path is a model notice: cause, socket path, the command
  to run and Retry in one line (`Pad.svelte`).
- The theme bridge holds: breeze-light reads as native, tokens only, labels
  muted and values plain, 48ch forms, keyboard and ARIA coverage on every
  control.

### Priority issues and their disposition

1. **[P1] Three notions of "current profile" disagree.** The header
   dropdown names the pinned profile, the Profiles editor followed the pad
   until a row was clicked, the pad shows a third; the pin has no visible
   control. Fixed in part the same day: a field of the Profiles editor
   taking the focus pins the selection (`b19296c`, spec §10). The visible
   pin control in the header and the Profiles question are #164.
2. **[P1] Roadmap milestones in user copy.** Fixed in part: the
   permanently visible Enabled hint reads "Switching a profile off is not
   built yet." (`aa8e3aa`); the Coming soon tooltips keep their milestone
   sentence by the gui-ux.md decision of #100 (the audience is developers
   and the roadmap is public). No P1 remains here.
3. **[P1] The moved state loses the user's place.** The reviewer asks for
   a dimmed outline on the pinned key while the pad shows another page;
   the owner decided the opposite in #143 (no selection outline while the
   pad is away), so that part is declined. The rest, Follow as the leading
   control of the line and a "pinned" mark on the tab, is #164.
4. **[P2] Unreachable leaves the drawing column empty and the catalogue
   enabled.** The entries now read disabled and do not drag while no daemon
   is connected (`aa8e3aa`); the device drawn with empty wells is #165.
5. **[P2] Two scroll regions on Profiles, the catalogue taller than the
   window.** #166: one scroll owner per screen, System and Apps collapsed
   by default, "Edit keys on the Pad" in the Pages heading row.

Heuristics 5, 6 and 10 (chord validation before the write, field hints, the
catalogue below 1200 px) are #167.

### Persona red flags

- Alex (power user): the Profiles selection hijacked by focus changes
  (fixed); `/` focuses nothing below 1200 px (#167); the pin set in the
  header has no visible state (#164).
- Jordan and the junior developer of PRODUCT.md: "Chord" and "Exec" as
  action names, "Window class org.kde.konsole" with no hint how to find a
  class, "higher wins the window", the "global" badge on a tile, the colour
  field asking for `#rrggbb` (#167); after a catalogue preset is clicked
  the write happens at once and the only confirmation is the muted result
  line, so the first success can go unnoticed (question 2 below).
- Sam (screen reader, low vision): structure is strong (roles, labels,
  status regions, focus rings). In breeze-light the selected profile row
  differs from its neighbours by a 1 px border and a 4 percent surface
  shift; the mirror's focus ring is `--kv-fg` while every other control
  uses the accent (the #141 decision for the device: the tile is part of
  the pad, not a control of the app).
- Riley (stress): long names and paths are handled (the harden commit
  `cb9dad1`: a long profile name wraps in the heading, a long page name is
  cut with an ellipsis and carried in the title, a long catalogue entry
  wraps; every physical side property is logical now, for RTL). The
  Profiles write note carries the time now; the drawer covers the header's
  selectors while open (`Drawer.svelte`), so the pin cannot change while
  reading a trace.

### Minor observations

- The mock's tiles all read "Commit" while the inspector shows another
  label: the mock renders fixed snapshots, not the key; the product's
  mirror is rendered by keyvo-core (the mirror-is-truth principle holds
  on the runtime, which the owner's User test covers).
- `<code>keyvo-daemon</code>` in the unreachable notice is the one
  monospace run in the app; DESIGN.md keeps paths and ids in the body
  font. Acceptable for a command line.
- The header's profile select shows an empty option while unreachable.
- The "+" buttons (profiles, pages) are a bare glyph with the name only in
  `aria-label`.

### Questions to consider

1. Should Profiles ever follow the pad after the first paint, or is "on
   the pad" in the list enough? (#164 decides.)
2. Is write-on-change with undo the right model for the junior, or does a
   fixed "Saved to konsole.toml" place (the header's right side) buy the
   confidence the form line does not?
3. Should the catalogue open with General only, and should the sub-1200
   fallback be a popover with icons and search instead of a native select?
   (#166 and #167.)
4. Does the trace drawer need "open the key" (pin the inspector to the
   entry's key) to honour P4, since the drawer knows profile, page and key?
5. Should the global page tabs be labelled "Global: general" instead of
   relying on the dashed rim and a title tooltip?
