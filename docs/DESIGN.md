---
name: keyvo
description: The live mirror and layout editor of a developer macro pad, drawn in the desktop's own theme
colors:
  # Pre-alpha defaults of app/src/styles/tokens.css. On a desktop the theme
  # bridge (docs/design/gui-ux.md §7) replaces every value with the host
  # theme's; the hex values below are what the app shows until then and in
  # the mock dev server's breeze-dark set. Keys are the token names without
  # the --kv- prefix.
  bg: "#1e1f22"
  bg-alt: "#26282c"
  fg: "#f2f3f5"
  fg-muted: "#a8adb5"
  accent: "#4c9aff"
  sandbox: "#b48ef0"
  project: "#3cbfae"
  state-running: "#f08c00"
  state-success: "#2f9e44"
  state-error: "#e03131"
typography:
  title:
    fontFamily: "system-ui, sans-serif"
    fontSize: "1.2em"
    fontWeight: 600
    lineHeight: 1.3
  heading:
    fontFamily: "system-ui, sans-serif"
    fontSize: "1em"
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: "system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.4
  label:
    fontFamily: "system-ui, sans-serif"
    fontSize: "0.85em"
    fontWeight: 400
    lineHeight: 1.3
  caption:
    fontFamily: "system-ui, sans-serif"
    fontSize: "0.75em"
    fontWeight: 400
    lineHeight: 1.3
rounded:
  control: "6px"
spacing:
  hairline: "2px"
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
components:
  control:
    backgroundColor: "{colors.bg-alt}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "32px"
  control-selected:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "32px"
  control-disabled:
    backgroundColor: "{colors.bg-alt}"
    textColor: "{colors.fg-muted}"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "32px"
  rail-item:
    backgroundColor: "{colors.bg-alt}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "40px"
  rail-item-current:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "40px"
  menu:
    backgroundColor: "{colors.bg-alt}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "4px"
  tooltip:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "4px 8px"
  notice:
    backgroundColor: "{colors.bg-alt}"
    textColor: "{colors.fg}"
    rounded: "{rounded.control}"
    padding: "8px 12px"
---

# Design System: keyvo

> Visual context for the impeccable design skill, scanned from
> `app/src/styles/tokens.css`, `app/src/styles/controls.css` and the
> components under `app/src/` on 2026-10-26, with the owner's answers of the
> same day. `docs/design/gui-ux.md` §6 and §7 remain the design truth for
> tokens, keyboard map and accessibility; this file says how to apply them
> and what the pre-alpha code does today. Strategy lives in `docs/PRODUCT.md`.

## 1. Overview

**Creative North Star: "The Instrument Panel"**

The app is the instrument panel around the pad. The pad's nine keys, drawn by
the same renderer as the device, are the one object on screen that is not an
instrument; everything else is a gauge or a control next to it. A gauge
shows one fact. A control sits beside the thing it changes. Nothing is
decoration, and the density that developers want comes from order and rhythm,
not from more boxes.

The system has no palette of its own. Every colour, the font and the corner
radius come from the host desktop through the `--kv-*` custom properties
(gui-ux.md §7); the values in this file's frontmatter are only the pre-alpha
defaults the bridge will replace. One exception, by design: `--kv-device`,
the body colour of the device drawing on the Pad, is fixed per edition of
the hardware (graphite, pale grey, white) and chosen in Preferences, because
the pad is a physical object on the screen, not a panel of the app. That is why the system is flat and quiet:
it has to look right inside Breeze and Adwaita, light and dark, without
knowing in advance which. It explicitly rejects the marketing gloss of Logi
Options+, the visual style of the Stream Deck software (its fixed dark
chrome, glossy tiles and marketplace clutter; its layout hierarchy, device
canvas, pages, inspector, actions catalogue with drag and drop, is adopted
since 2026-10-26, see gui-ux.md §2), the
generic SaaS dashboard (card grids, big numbers, gradients, glass) and the
Electron dark developer-tool cliché (black, neon, monospace everywhere, an own
dark look). It also rejects the pre-alpha Profiles screen of 2026-10-26: a
clipped preview, a scroll bar over the inputs, three columns with no
hierarchy.

**Key Characteristics:**

- Theme-following: zero literal colours in components, tokens only.
- Flat: two background levels and 1 px borders carry all depth.
- One accent, used as a 1 px outline on the current or selected thing, never as a fill.
- One system font, five sizes, two weights.
- An 8 px rhythm with 2 and 4 px for the finest joints and 12, 16, 24 px between groups.
- Every fact appears once; labels are muted, values are not.

## 2. Colors

A tonal pair of surfaces and a tonal pair of text, one accent and six
semantic colours, all borrowed from the desktop.

### Primary
- **Accent** (`--kv-accent`, pre-alpha `#4c9aff`): the desktop's accent colour. It marks the current rail item, the selected tab, the selected profile, the selected key and the focus ring, always as a 1 to 2 px outline. It never fills a surface and never colours text.

### Neutral
- **Window** (`--kv-bg`, pre-alpha `#1e1f22`): the page and the editor column; also the "pressed in" surface of a selected control.
- **Panel** (`--kv-bg-alt`, pre-alpha `#26282c`): the header, the rail, the drawer, menus and the resting surface of every control.
- **Text** (`--kv-fg`, pre-alpha `#f2f3f5`): values, headings, control labels.
- **Muted text** (`--kv-fg-muted`, pre-alpha `#a8adb5`): field labels, metadata, disabled controls, and the only border colour of the system.

### Semantic (tertiary; used sparingly and never alone)
- **Sandbox** (`--kv-sandbox`, pre-alpha `#b48ef0`): the header turns to it while the simulator is sandboxed (gui-ux.md §1); it is the one case where a surface takes a non-neutral colour.
- **Project layer** (`--kv-project`, pre-alpha `#3cbfae`): the outline of keys that come from a project layer.
- **Running, success, error** (`--kv-state-running` `#f08c00`, `--kv-state-success` `#2f9e44`, `--kv-state-error` `#e03131`): the feedback states, the same values as the pad's theme so the mirror agrees with the device. In the app each is always paired with an icon or a word.

**The Borrowed Palette Rule.** No component declares a colour. It reads a
`--kv-*` token, and a new need adds a token to gui-ux.md §7 in the same PR.
A literal hex, rgb or named colour in a component is a defect.

**The Outline Accent Rule.** The accent is an outline, never a fill. The
selected state is "pressed in": `--kv-bg` surface inside a `--kv-accent`
border on a `--kv-bg-alt` panel. If the accent covers more than a border,
the design has drifted.

**The Muted Label Rule.** A label is `--kv-fg-muted` at 0.85 em; its value
is `--kv-fg` at body size. Never the other way round, and never both muted.

## 3. Typography

**Display Font:** none; the system has no display size.
**Body Font:** `system-ui, sans-serif` (the desktop's interface font through `--kv-font`, Noto Sans on the owner's Plasma, Cantarell or Adwaita Sans on GNOME)
**Label/Mono Font:** none distinct; file paths and ids stay in the body font.

**Character:** the desktop's own voice. The app must read like a System
Settings page, not like a product. Hierarchy comes from a short size scale
and the regular versus semibold contrast; nothing is large.

### Hierarchy
- **Title** (600, 1.2 em, 1.3): the app name in the header only.
- **Heading** (600, 1 em, 1.3; the drawer's at 1.1 em): section headings such as "Profile", "Match rules", "Page preview", "Selected key". Same size as body, weight does the work.
- **Body** (400, 14 px base through `--kv-font-size`, 1.4): values, control text, notes. Lines of prose cap at 65 to 75 ch.
- **Label** (400, 0.85 em, muted): field labels, metadata ("global, priority 0"), tab counts, "Coming soon".
- **Caption** (400, 0.75 em, muted): the key number inside a slot and nothing else.

**The Two Weights Rule.** Regular and semibold only. No light, no bold, no
italics for emphasis; emphasis is weight or position.

**The No Big Number Rule.** No size above 1.2 em anywhere. A status is a
word in body size, never a hero metric.

## 4. Elevation

Flat, with tonal layering, the way Breeze and libadwaita do it. Depth is two
surfaces (`--kv-bg` window, `--kv-bg-alt` panel) and 1 px borders in
`--kv-fg-muted`. There are no shadows at rest and no shadows on cards or
columns. The only things that leave the plane are the floating elements the
desktop also floats: the overflow menu, a tooltip, the trace drawer; they
sit on the panel surface with a 1 px border, and if a shadow ever appears
on them it comes from the theme, not from a component.

**The Flat Panel Rule.** A surface is flat at rest. Hover and press change
the border colour or the surface level, never add a shadow. If it looks like
a card hovering over the page, it is wrong.

**The One Level Rule.** A panel (`--kv-bg-alt`) on the window (`--kv-bg`)
is the whole depth budget. A panel inside a panel, or a card inside a card,
is forbidden; use spacing and a heading instead.

## 5. Components

Native and restrained: the size and weight of the desktop's own controls,
1 px border, a filled accent nowhere, the one primary action of a screen
distinguished by position and label, not by colour.

### Buttons
- **Shape:** softly rounded (6 px, `--kv-radius`), 32 px minimum height, 8 px horizontal padding, body size text.
- **Default:** panel surface (`--kv-bg-alt`), text `--kv-fg`, 1 px border `--kv-fg-muted`.
- **Selected / current** (tabs, rail items, the selected profile): window surface (`--kv-bg`) inside a `--kv-accent` border: pressed in, not lit up.
- **Hover / Focus:** focus is the 2 px `--kv-accent` outline with 2 px offset from tokens.css, plus the border turning accent on controls. Hover moves the border to `--kv-fg` over 150 ms (ease-out), nothing else; a disabled control does not react, and a selected or current control keeps its accent border (the hover rule sits below it through `:where()`).
- **Disabled:** panel surface with muted text and `cursor: not-allowed`; "Coming soon" controls stay disabled and labelled, never hidden.
- **Icon buttons** (rail toggle, overflow, column toggles): 32 px square; the rail's targets are 40 px (gui-ux.md §6).

### Inputs / Fields
- **Style:** the same box as a button: 32 px, 6 px radius, panel surface, 1 px muted border; placeholder in muted text.
- **Label:** above the field, muted, 0.85 em, 2 px gap; the label and its control are one column.
- **Focus:** border turns `--kv-accent`, plus the outline.
- **Error:** the field keeps its box; the message sits below as a notice with an icon or the word "Error", in body text, and names the file and the field. No red fill, no red border alone.

### Notices (the `.problem` and `.note` boxes)
- **Note:** plain muted text, no box.
- **Problem:** panel surface, 8 px by 12 px padding, 6 px radius, a leading icon or the state word in the state colour, the message in `--kv-fg`. The 4 px coloured left stripe the pre-alpha uses is a defect to remove (see Don'ts).

### Navigation
- **Rail:** a panel surface column with 8 px padding and 4 px between items; each item 40 px tall, icon plus label, label hidden when collapsed with a tooltip beside it. The current screen is pressed in (window surface, accent border). Keyboard: the map of gui-ux.md §6.
- **Header:** panel surface, 8 px by 16 px padding, the title at the left, the device selector and the overflow menu at the right; it turns to `--kv-sandbox` while sandboxed.
- **Tabs** (pages of a profile): buttons in a row with 8 px gaps; the selected one pressed in.
- **Columns:** list 240 px, editor flexible, right column 320 px, 16 px between them; each scrolls on its own with a reserved gutter so no control sits under a scroll bar.

### Lists (profiles)
- A row is a button that spans the list width: name, metadata in muted label size, a status mark at the right. The profile's accent is shown as a small square swatch inside the row, not as a stripe along its edge.

### The Mirror (signature component)
- The nine tiles are images from the renderer, in the device's proportions (480x480 panel, 118 px tiles at 1:1), scaled as a whole to the column that holds them, never clipped and never drawn larger than 1:1. The selected tile has the accent outline; a project-layer tile the `--kv-project` outline. The grid spacing between tiles mirrors the device's gaps. A tile is part of the device, not a control: it sits inside the cutout's rim and takes the cutout's corner radius (`--kv-tile-radius`, from the drawing) and the well colour behind its transparent corners (`--kv-device-well`), the same in every theme and at every size; the theme radius applies only on the plain panel a drawing has no cutouts for.
- A key slot in an editor grid (where the image is not shown) is a 96 by 56 px button with the number as a caption and the label in body size.

### Menus and tooltips
- **Menu:** panel surface, 4 px padding, 1 px muted border, items as full-width borderless buttons with 16 px between label and shortcut.
- **Tooltip:** window surface, 4 px by 8 px padding, 1 px muted border, at most 280 px wide.

## 6. Do's and Don'ts

### Do:
- **Do** read every colour, font and radius from a `--kv-*` token, and add any new token to gui-ux.md §7 in the same PR.
- **Do** show the selected state as pressed in: `--kv-bg` surface inside a 1 px `--kv-accent` border.
- **Do** keep the 8 px rhythm: 2 and 4 px inside a control, 8 px between controls, 12 to 16 px between groups, 24 px between sections.
- **Do** pair every running, success and error colour with an icon or a word.
- **Do** give every scrolling column a reserved scroll bar gutter and the theme's thin scroll bar.
- **Do** scale the mirror to its column as one image in device proportions.
- **Do** keep "Coming soon" controls visible, disabled and labelled, so the map of the product stays legible.
- **Do** check every text and surface pair at 4.5:1 in breeze-light, breeze-dark, adwaita-light and adwaita-dark before merging.

### Don't:
- **Don't** declare a literal colour, font or radius in a component; the Borrowed Palette Rule has no exceptions.
- **Don't** fill a surface or colour text with the accent; the accent is an outline.
- **Don't** use `border-left` or `border-right` wider than 1 px as a coloured stripe on notices, list rows or cards (the pre-alpha `.problem` boxes and the profile row swatch do this; remove it).
- **Don't** nest a panel in a panel or a card in a card; spacing and a heading separate groups.
- **Don't** add a shadow to a column, a card or a button; the only floating things are the menu, the tooltip and the drawer, and their shadow is the theme's.
- **Don't** build the generic SaaS dashboard: identical card grids, a big number with a small label, gradient accents, glass effects.
- **Don't** build the Electron dark developer-tool cliché: black backgrounds, neon accents, monospace everywhere, an own dark look that ignores the desktop theme.
- **Don't** imitate Logi Options+ (marketing gloss, hero cards, illustrations in place of settings) or the Stream Deck software (a crowded drag-and-drop puzzle with a plugin store).
- **Don't** draw the nine tiles larger than 1:1 or let the column clip them.
- **Don't** reach for a modal; the editor, the drawer and inline notices come first.
- **Don't** use an em dash anywhere, in copy or in code comments.
