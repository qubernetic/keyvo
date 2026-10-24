# UI concept sketches

These images are **sketches, not the visual specification**. The design is
`docs/design/gui-ux.md`, derived from the grill decisions in
`docs/plans/2026-10-24-gui-ux-grill-decisions.md`; where a sketch and the
design disagree, the design wins.

## Provenance

Generated with ChatGPT image generation on 2026-10-24 from the owner's prompts
in three rounds, as input to the UI/UX grill. Downscaled to 1200 px JPEG for
the repository; the originals (1448 x 1086 and 1536 x 1024 PNG) are not
tracked. No third-party artwork is reproduced; the Logitech wordmark visible on
the rendered keypad is a depiction of the device, not a mark used by keyvo
(see `README.md`, trademark notice).

| File | Round | What it shows |
|---|---|---|
| `concept-board.jpg` | 0 | first concept board, light theme |
| `v1/v1-1.jpg` to `v1/v1-5.jpg` | 1 | five screen ideas: dashboard, grid editor, actions, integrations, settings |
| `v2/v2-1-concept-board.jpg`, `v2/v2-2-dashboard.jpg` | 2 | dark developer look (now an opt-in theme, principle P1) |
| `v3/v3-1.jpg`, `v3/v3-2.jpg` | 3 | shell variants |
| `v3/v3-3-layout-reference.jpg` | 3 | **the layout reference** for the shell (design §2): rail, list column, editor, right column with keypad preview and key card |

## What the sketches got wrong

The decisions file overrides these points; they are listed so that nobody
copies them from the image:

- "Save Changes" button: there is none. Edits apply immediately; the sandbox
  has an Apply and Discard pair (decision 2 and 3).
- Device status in the rail and in the header at once: the header device
  selector is the only place (principle P3, decision 8).
- A global simulation dock: simulation lives at the bottom of the editor as
  "Test this behavior" (decision 8).
- "Play sound" feedback toggle: no sound in v1 (decision 8).
- Rail order and entries: Home, Profiles, Behaviors, Projects, Integrations,
  Devices, Settings (decision 8); the sketch lacks Projects.
- "Hold for 0.5 seconds": the long-press threshold is `[timing].long_press`
  (default 1000 ms), not a per-key number.
- Colours, fonts and radii: the app follows the desktop theme (principle P1);
  the sketch's palette is one possible opt-in theme.
