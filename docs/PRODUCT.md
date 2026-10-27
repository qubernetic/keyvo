# Product

> Strategic context for the impeccable design skill, extracted from
> `docs/spec.md` §1 and §10, `docs/design/gui-ux.md` and
> `docs/plans/2026-10-24-gui-ux-grill-decisions.md` (principles P1 to P4), and
> the owner's teach interview of 2026-10-26. `docs/design/gui-ux.md` remains
> the design truth: when this file and that one disagree, that one wins and
> this one is corrected.

## Register

product

## Users

Software developers, and only them. First the owner: KDE Plasma on Wayland,
daylight at a desk, VS Code, Konsole, a browser and mail open, the Keypad at
hand; the app sits in a side window while they rearrange the pad in minutes.
Second a junior developer who does not yet live in terminals and text files:
same desk, same app, configures a key without ever reading the TOML or the
schema. Not home users, not "creative" (photo and video) users.

The job on any screen: see what the pad shows right now, change what a key
does or how it looks, and when a press misbehaves, read exactly what happened.
Scripts stay text; the app edits layout, icons, labels and simple actions, and
every edit round-trips through the same TOML files the user may also edit by
hand.

## Product Purpose

keyvo turns the Logitech MX Creative Keypad into a context-aware, scriptable
control surface for developers on Linux: a daemon renders the nine LCD keys
locally, profiles follow the focused window, and everything is TOML plus a
documented NDJSON socket. The desktop app is the live mirror of the pad and
its layout editor; it exists to make customisation an opportunity instead of
a task, to make a customised pad debuggable, and to open the door to
developers who prefer a visual editor. It is never the only way: TOML, the
CLI and the JSON Schema stay first class.

Success: the owner lays out and debugs their own pad faster in the app than
by editing TOML; a junior sets up their first key in the app in their first
hour without help; the app never looks out of place on the desktop it runs on.

## Brand Personality

Quiet, precise, guiding. Between an instrument and a mentor: the pad and the
profile are the subject, the app is the instrument panel around them, and
when a step is needed the next action is obvious and named. Calm confidence,
no decoration, no cleverness. Three words: quiet, precise, guiding.

Category references (the feel, not the look): Raycast for the command-centred
panels with instant feedback and the focus on content; the VS Code Settings UI
for the developer-familiar searchable settings, inline editors and the file one
click away.

## Anti-references

- Logi Options+: marketing gloss, rounded hero cards, large illustrations, the
  setting hidden behind decoration.
- Stream Deck software: a drag-and-drop puzzle with a plugin store and an icon
  library, everything one click away and crowded.
- The generic SaaS dashboard: identical card grids, big number with small
  label, gradient accents, glass effects.
- The Electron dark "developer tool" cliché: black background, neon accent,
  monospace everywhere, an own dark look that ignores the desktop theme.
- The current pre-alpha Profiles screen as found on 2026-10-26: clipped
  preview, scroll bar over the inputs, three competing columns with no
  hierarchy.

## Design Principles

1. **Native theme by default (P1).** Colours, accent, fonts and corner radius
   come from the host desktop through the `--kv-*` tokens of gui-ux.md §7;
   the app must not stand out from anyone's desktop. Own themes are opt-in
   overrides, never the default. No palette of its own, no fixed dark or light.
2. **The mirror is the truth (d1, d3).** What the app shows is what the pad
   shows, rendered by the same renderer; editing happens on the mirror, there
   is no separate design canvas.
3. **Guided, not gamified (P2).** One primary action per screen, empty states
   that name the next action, inline validation with a recovery hint, wizard
   steps that say what Finish will do. No XP, streaks or achievements, ever.
4. **Every fact once (P3).** No restated headings, no duplicated status, no
   label that repeats the value next to it.
5. **Serious customisation is debuggable (P4).** Every press, real or dry-run,
   leaves a detailed trace one gesture away; errors say what failed and where
   the file is.
6. **Dense but calm.** Developers want the information on one screen; the
   density comes from hierarchy and rhythm, not from more boxes. Cards are the
   exception, nested cards never.

## Accessibility & Inclusion

WCAG 2.2 AA. Every text token pair passes 4.5:1 in the native light and dark
schemes and in the high-contrast scheme (`--kv-contrast: high`). The system
font and font size are followed; layouts survive a 1.5x text scale. Reduced
motion follows the desktop (`--kv-motion: reduced`). Everything reachable by
keyboard with the map of gui-ux.md §6, visible focus on every control. The
three feedback states (running, success, error) are never colour alone in the
app: an icon or a word accompanies the colour (the strip on the pad itself is
a rendering decision taken separately). Strings go through one i18n module;
English first.
