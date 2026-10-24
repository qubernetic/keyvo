# GUI and UX design: grill decisions (2026-10-24)

> Outcome of the UI/UX grill of 2026-10-24 (owner and Claude, one question at a
> time, every answer approved in chat). This is the **what and why**; the design
> document (`docs/design/gui-ux.md`), the per-issue plans and the roadmap
> revision are derived from it and do not re-open these points. Inputs: spec §9
> and §10, ADR-0004, the ChatGPT concept conversation and the concept images
> under `docs/ui-concepts/` (sketches, not the visual spec; the third v3 image is
> the layout reference), the `ui-ux-pro-max` skill checklist (sections 1 and 2).

## Principles

- **P1. Native theme by default.** The app ships with the host desktop theme as
  the default (KDE Plasma, GNOME, later Windows): colours, accent, fonts and
  window decorations come from the OS through a Rust-side theme bridge exposed
  as CSS custom properties, followed live. Built-in themes (for example a dark
  developer look) are opt-in overrides, never the default. Why: an app that
  clashes with the desktop gets replaced.
- **P2. Guided, not gamified.** "Gamified" means the hand-holding UX of modern
  games: the next step is obvious, nothing to discover, it just works. No XP,
  streaks or achievements, ever. Guided flows get detailed end-to-end tests and
  step-by-step User test checklists.
- **P3. No redundant information.** Every fact appears once on screen.
- **P4. Serious customization is debuggable.** Every key press, real or
  simulated, leaves a detailed, reachable trace.

## Decisions

1. **Mental model: device mirror.** Home is the live copy of the pad (socket
   `subscribe`, the same renderer, bit-identical tiles). Profiles are edited on
   the same mirror in design mode. Why: what the pad shows is the truth; a
   junior understands it at once.
2. **Immediate apply, no save button, undo.** Edits write TOML through
   `toml_edit` at once, the daemon hot-reloads, the pad repaints. `exec`
   scripts open in the external editor. Why: ADR-0004, GUI and vim edits are
   indistinguishable.
3. **Simulator as a sandbox.** A switch creates a working copy of the edited
   profiles; the virtual pad renders it; Apply merges into the real files,
   Discard drops it. Without a connected device the GUI starts in the
   simulator (writing directly). Clicks are presses (long press by holding or
   right click); actions are not executed but logged (dry run on the socket).
4. **The key stays the unit of truth; "behavior" is the GUI view.** No new
   top-level TOML entity. Two schema additions in v1: an `exec` result
   (running, success, error) switches the key's `states` automatically, and an
   optional `on_fail` action. The Behaviors screen is a flat, searchable list
   of all bindings with templates that are copied, not referenced. Referenced
   `[[behaviors]]` deferred to v2.
5. **Project layer (`.keyvo/`).** Stack: global < app profile < project layer <
   overlays (context page only). Files: `.keyvo/profile.toml` (all apps),
   `.keyvo/profiles/<app-id>.toml` (per-app key overrides), `.keyvo/local.toml`
   (personal, gitignore candidate); same schema, no `match`. Key-level
   override, explicit key clearing, nearest `.keyvo/` upwards (like `.git`).
   `exec` cwd is the project root; `KEYVO_PROJECT_ROOT`, `KEYVO_PROJECT_NAME`,
   `KEYVO_GIT_BRANCH`. `[project_layer]` in `config.toml`: `enabled`, position
   in the stack, allowed keys, allowed action types, allowed apps. Milestone M2.
6. **Trust model, direnv style.** A project layer is rendered but no action
   runs until the project is trusted; trust is bound to the content hash of the
   three files (`~/.config/keyvo/trust.toml`), re-asked on change; `keyvo
   project trust` and a GUI prompt listing the actions. `[project_layer].trust
   = ask|never|always` (always is warned against by `doctor`). SECURITY.md
   gets a paragraph: the project layer is the first foreign input.
7. **Context source.** Integrations push the working directory with
   `notify_context` (shell prompt hook for bash, zsh, fish; VS Code extension;
   tmux); the daemon binds it to the focused window by matching the hook's
   ancestor pids with the KWin-reported window pid. Fallback: window title
   against a project registry (trusted and recently seen roots); Dolphin lives
   on this fallback. Otherwise "unknown project", with the reason visible.
8. **Shell, after the third v3 image.** Navigation rail: Home, Profiles,
   Behaviors, Projects, Integrations, Devices, Settings; device selector in the
   header (no duplicate status in the rail); list column plus editor; right
   column with keypad preview (page selector) and the selected key card
   (position, page, layer origin only). Simulation lives at the bottom of the
   editor ("Test this behavior": Simulate, short, long, hold, one-line result
   with expandable "would run"); no global dock; Debug log on Home and Devices.
   Trigger section: one row per press kind. Conditions, Actions, Feedback as
   stacked cards; no sound toggle in v1. Default 1200x760, minimum 960x640,
   collapsible columns, modals only for confirmations and the trust prompt.
   The sandbox Apply/Discard pair replaces the mock-up's Save Changes.
9. **Unified trace.** One entry per press: timestamp, trigger (real or
   simulated), context snapshot, layer resolution, each condition's result,
   resolved action with substituted variables, execution result (`exec`: exit
   code, duration, bounded stdout and stderr tail; `http`: status and body
   excerpt; `chord`: event sequence), `on_fail` branch, block reason. Ring
   buffer plus `~/.local/state/keyvo/trace.ndjson` (rotated). Reachable from the
   result line and Home events (drawer), `keyvo ctl trace`, `keyvo ctl tail`,
   `subscribe`. Debug mode adds HID-level events. `keyvo exec-check <script>`
   runs a script with the key's environment. `KEYVO_STATE=<name>` on stdout
   selects a custom state. Core in M1, context fields in M2.
10. **Recipe action type and starter packs.** `action = { type = "recipe", id =
    "project.build" }`, resolved at press time: project-layer parameters, then
    detectors (file presence: `Cargo.toml`, `package.json`, `platformio.ini`,
    `CMakeLists.txt`, `west.yml`, `*.tsproj` with a "configure manually" hint),
    then the progressive two-step mini-wizard whose answer lands in the project
    layer. "Expand to a concrete action" rewrites to plain `exec`. Recipes are
    TOML in the bundle, `~/.config/keyvo/recipes/` and `.keyvo/recipes/`.
    Starter packs (Rust, web, Python, embedded, PLC, AI-Assisted Coding) are
    copied, through the trust gate. "Make this a keyvo action" from shell
    history yields plain `exec`. Recipes and detectors in M1, mini-wizard M4a.
11. **First run.** `keyvo setup` (interactive CLI, M1) owns the flow; the M4a
    wizard wraps it over the socket. Four steps: device and system (`doctor`
    results with one button per fix, each with its own approval), starter pack
    (previewed in the simulator), integrations (commands shown, all skippable),
    review and Finish as a transaction (file list with backups, system commands,
    atomic apply, schema validation, reload, smoke paint). No tutorial overlay;
    one dismissable hint bar on Home. Re-runnable per step from Settings.
12. **Key feedback.** `running`: dimmed icon, accent arc (spinner primitive),
    static badge after a configurable long-run threshold; `success`: green
    badge and a single flash, reverts after `[render].feedback_hold` (2 s);
    `error`: red badge that stays until the next press or GUI inspection.
    Built-in state looks live in the theme file, per-key `states` override.
    Desktop notification optional, default only on error. Primitives and the
    exec binding in M1.
13. **Dialpad in the mirror.** Second, collapsible panel with a schematic
    dial, roller and buttons, GUI labels (no LCD); rotation as a press kind in
    Trigger; simulated rotation; the header device selector changes focus, not
    visibility. Shell in M4a, content with M3.
14. **Concurrent edits and undo.** The GUI watches profile files and reloads on
    external change; undo is per GUI session as `toml_edit` diffs (Ctrl+Z,
    Ctrl+Shift+Z); a rotated snapshot directory under
    `~/.local/state/keyvo/snapshots/` before every GUI write, restored with
    `keyvo profile restore`; on schema errors the daemon keeps the last good
    state and the GUI shows file and line and opens the external editor.
15. **Keyboard first and accessibility.** Full keyboard traversal in visual
    order; 1..9 and arrows on the grid, Enter edits, Space simulates, Ctrl+K
    palette, Ctrl+1..7 rail; keyboard pair for drag and drop. System font size
    and high-contrast scheme pass through the theme bridge; 4.5:1 check on
    tokens; reduced motion respected in the GUI. `aria-label` everywhere, 32 px
    minimum targets. English only in v1, i18n-ready.
16. **MX Master 4 and MX Mechanical as trigger sources (M3).** Their divertable
    controls (`0x1B04`, `getCidInfo` walk) become Behaviors triggers as a
    generalisation of the Dialpad buttons; volatile diverts only for the active
    profile; no haptics; Devices view lists them; `doctor` warns about Solaar
    diverts. Desk state (DPI, SmartShift, backlight) stays a separate M3
    decision.
17. **Scope closure of the concept list.** XP gamification: never. Workflow
    insights: v2, local, opt-out, derived from the trace. Focus sessions: a
    recipe. Profile bundles: v1, `keyvo profile export|import` (zip of TOML,
    icons, theme), GUI button later. Marketplace: no; `keyvo pack add
    <git-url>` in v2 through the trust gate. Plugin SDK: the M2 supervised
    plugins and socket API; sandbox v2. Dynamic widgets: through `set_key` and
    `push_overlay` plus two reference plugins (clock or timer, git branch and
    dirty state); system widgets v2. Tray icon from the GUI in v1. `.lp5`
    import unchanged.
18. **Windows (M5).** The same Tauri GUI, behind the C# plugin's named pipe;
    differences only in the theme bridge and `doctor`; simulator works without
    a device. Not in v1 on Windows: project layer via shell hook, MX buttons,
    desk state.

## Documents this changes

- `docs/spec.md`: §6 (project layer in the stack, context source), §7.2
  (recipe type, `on_fail`, automatic `states`, `.keyvo/` files,
  `[project_layer]`), §8 (trace on the socket, `notify_context` cwd), §9
  (feedback primitives), §10 (rewritten around the shell), §16 open questions.
- `docs/roadmap.md`: M1 (recipes, detectors, trace core, `keyvo setup`,
  feedback primitives, export/import), M2 (project layer, trust, context
  source, shell hook, VS Code, reference plugins), M3 (MX Master and
  Mechanical triggers), M4a (shell, simulator, wizard, mini-wizard), M5.
- `SECURITY.md`: project layer threat and trust model.
- `CONTEXT.md`: behavior, project layer, recipe, trace, simulator, starter pack.
- ADR: a new record for the project layer and its trust model (expensive to
  reverse); ADR-0004 stays.
- `docs/ui-concepts/README.md`: "sketch, not spec", provenance, which image is
  the layout reference.
