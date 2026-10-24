# Plan: GUI and UX design document, roadmap revision for sandbox-first delivery (Issue #29)

> Documentation plan for the issue that lands the 2026-10-24 UI/UX grill. Governing
> decisions: `docs/plans/2026-10-24-gui-ux-grill-decisions.md` (principles P1 to P4 and
> decisions 1 to 18, all approved in chat; referenced below by number, never restated),
> ADR-0002 (two-layer development model), ADR-0004 (GUI edits TOML with round-trip),
> ADR-0006, ADR-0007, ADR-0008. Status: **approved in chat on 2026-10-24** (plan and the four recommendations in §6).

## 1. Why

The grill produced 4 principles and 18 decisions that change the spec (§6, §7.2, §8, §9,
§10, §16), the roadmap (M1 to M5), the security policy, the glossary and need one new ADR.
None of that is in the repository yet. At the same time the owner wants development to
run as far as possible on paperclip (the agent-horde instance with a docker sandbox for
the devcontainer). Those agents commit as `q-horde`, cannot see `/dev/hidraw*` or
`/dev/uinput`, and therefore need issues whose proof is a `just` recipe and a fixture, not
a device. The roadmap today lists milestones and a DoD; it does not say which issue an
agent can take. This issue lands both: the design, and a roadmap cut along the
sandbox/hardware line.

## 2. Outcome

- A reader of `docs/design/gui-ux.md` can implement any M4a screen without reopening a
  decision, and a reader of the spec finds the new concepts (project layer, trust, recipe,
  trace, simulator, feedback states) where the old ones live.
- Every planned issue in `docs/roadmap.md` is marked **S** (sandbox-complete: buildable,
  testable and reviewable in the devcontainer) or **H** (hardware-bound: owner on the
  host). Every S issue names its fixtures and the `just` commands that prove it; every H
  issue is the thin device edge of an S issue and carries a User test.
- paperclip agents can pick any S issue of the current milestone with no further
  grilling beyond that issue's own plan file.

## 3. Files

| File | Change |
|---|---|
| `docs/design/gui-ux.md` | New. The design document, derived from the decisions file. Sections: 1 Mental model (d1, d3); 2 Shell and navigation (d8, layout reference `docs/ui-concepts/v3/...-3.png`); 3 Screens, one subsection each: Home, Profiles, Behaviors, Projects, Integrations, Devices, Settings (d4, d5, d13, d16, d17); 4 Flows: edit and apply (d2, d14), simulate (d3, d9), first run (d11), trust prompt (d6), recipe mini-wizard (d10), export and import (d17); 5 States: key feedback (d12), empty, loading, error, offline device (d3); 6 Keyboard map and accessibility (d15); 7 Theme bridge (P1): token list as CSS custom properties, source per desktop, live update, contrast check; 8 Trace drawer and debug log (P4, d9); 9 Socket surface used by the GUI (which `subscribe` events, `set_key`, `push_overlay`, `notify_context`, dry run); 10 Windows differences (d18); 11 Out of scope for v1 (d17 closures). Each section opens with the decision numbers it implements. Wireframes as ASCII only where the prose is ambiguous; the PNGs are not referenced as the visual spec. |
| `docs/spec.md` | §6: project layer position in the stack, context source and its fallback (d5, d7). §7.2: `recipe` action type, `on_fail`, automatic `states` from `exec` results, `.keyvo/` files and precedence, `[project_layer]` table with every key and default (d4, d5, d6, d10). §8: trace record fields and the `ctl trace`, `ctl tail`, `exec-check` commands, `notify_context` carries the working directory (d9, d7). §9: feedback primitives and `[render].feedback_hold = 2000` ms (d12). §10: rewritten around d1, d2, d3, d8, d13, d15, d18, pointing at the design document for screens. §16: question 16 (splash default) stays open and is decided with the M1 renderer issue, since the grill did not fix it; add rows for the open items in §7 below. Changelog entry v0.3. |
| `docs/roadmap.md` | Revised as described in §4 below: S/H classification, per-issue proof, the M4a split and the new ordering constraints. The DoD texts of M1, M2, M3, M4a and M5 take the additions listed under "Documents this changes" in the decisions file. |
| `docs/adr/0009-project-layer-and-trust.md`, `docs/adr/README.md` | New ADR (Proposed, Accepted on merge): the project layer as a fourth stack level with key-level override, and direnv-style trust bound to a content hash (d5, d6). Alternatives: per-repo profiles by `match`, trust on first use, signed packs. |
| `SECURITY.md` | New subsection under "Threat model": "The project layer is the first foreign input": what is rendered before trust, what never runs, where trust is stored, how `trust = always` is surfaced by `doctor` (d6). |
| `CONTEXT.md` | New terms: Behavior, Project layer, Trust, Recipe, Starter pack, Trace, Simulator, Feedback state, Context source. Existing "Action" entry gains `recipe` and `on_fail`. |
| `CONTRIBUTING.md` | "Issue labels": new row `sandbox`. New subsection "Sandbox-complete and hardware-bound issues" under "Two-layer development model": the definition, the rule that every S issue's plan file names fixtures and `just` commands, that H issues stay small and carry the User test, and that `q-horde` agents take S issues only. |
| `.github/ISSUE_TEMPLATE/feature_request.yml` | "Hardware" checkbox group gains "This can be built and tested entirely in the devcontainer (no device)"; the markdown intro points at the CONTRIBUTING subsection. |
| `docs/ui-concepts/README.md` | New. "Sketch, not spec": provenance (ChatGPT image generation, 2026-10-24, three rounds), which image is the layout reference, what the sketches got wrong against the decisions (save button, global simulation dock, duplicate status), licence note. |
| `docs/ui-concepts/**/*.png` | Committed after downscaling (see §6, owner decision). |
| GitHub labels | New label `sandbox` ("Buildable, testable and reviewable entirely in the devcontainer; agents take these"), colour `#0e8a16`. Created with `gh label create` by the owner or in the issue step. |

No code, no fixtures, no schema file. The JSON Schema and the `toml_edit` changes for
`recipe`, `on_fail` and `[project_layer]` are M1 and M2 code issues; this PR only fixes
their TOML shape in the spec.

## 4. Roadmap revision shaped for paperclip

### 4.1 Rules

1. Every issue line carries **S** or **H**. S means: the PR is complete when `just ci`
   is green in the devcontainer and the reviewer can verify every acceptance criterion
   from the diff, fixtures and test output. H means: a step needs `/dev/hidraw*`,
   `/dev/uinput`, Solaar, udev, systemd, KWin or the Windows VM.
2. A hardware feature is always two issues: the S issue implements against
   `FakeHidTransport`, golden fixtures and captures, with the protocol facts cited from
   `docs/research/`; the H issue runs the owner's User test on the device, records the
   measured numbers in `docs/hardware-notes.md`, and fixes only what the device
   contradicts. H issues are kept small; if an H issue grows code, the code moves to a
   new S issue.
3. Every S issue's plan file has a "Proof" block: fixture paths and the exact `just`
   commands with expected output. Proof commands are the devcontainer recipes only
   (`just test`, `just lint`, `just fmt-check`, `just deny`, `just cov`, `just ci`, and
   `docker compose exec rust cargo ...` where the justfile has no recipe). No `just run`,
   no `just test-hw` in an S issue.
4. The roadmap keeps the milestone order for the DoD, but S issues may be opened across
   milestone boundaries when their inputs exist. The explicit cross-milestone
   dependencies are listed in 4.3.
5. The split is recorded as a `Layer` column in a per-milestone table in the roadmap
   (Issue, Title, Layer, Proof or User test, Depends on) and as the `sandbox` label on
   GitHub. The `hardware` label already exists and marks H issues.

### 4.2 Milestone tables (draft, to be written into `docs/roadmap.md`)

Fixture sources: `hc` = derived from the hcooper captures (`docs/research/hcooper-captures.md`),
`inv` = facts from `docs/research/options-plus-inventory.md`, `own` = our captures after
M0-2, `golden` = frames recorded on the host by an H issue and committed under
`tests/protocol-fixtures/`.

**M0 Protocol**

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M0-1 | research: TS quick-test with Julusian's library | H | owner, measurement list A | |
| M0-2 | chore: Windows VM and USB capture toolkit | H | owner, `captures-2026-xx` release asset, fixtures extracted | |
| M0-3 | feat(hid): HID++ 2.0 core | S | `just test -p keyvo-hid`, golden tests from `hc` and `inv` | |
| M0-4a | feat(hid): keypad enumeration, keep-alive and hot-plug against FakeHidTransport | S | `just test`, fixtures `hc` | M0-3 |
| M0-4b | hardware: keep-alive window and hot-plug verified on the Keypad | H | owner, numbers into hardware-notes | M0-4a |
| M0-5a | feat(hid): VLP display plane: geometry query, image writer, ACK flow control | S | `just test`, golden frames `hc` and `inv` (0x19A1 fn table) | M0-3 |
| M0-5b | hardware: paint timing, fragment pacing, JPEG size guard measured | H | owner, `just run -- fill`, numbers into hardware-notes | M0-5a |
| M0-6a | feat(hid): key and page-button input decoding | S | `just test`, report fixtures `hc` | M0-3 |
| M0-6b | hardware: 0x1B04 divert and input verified | H | owner | M0-6a |
| M0-7a | feat(hid): brightness 0x8040 | S | `just test` | M0-3 |
| M0-7b | hardware: brightness unit and idle-dim re-assert | H | owner | M0-7a |
| M0-8 | feat(cli): probe, keys, fill, paint-test, doctor | S | `just test`, CLI tests against FakeHidTransport; `doctor` hardware checks are stubs the H issues fill | M0-4a, M0-5a |
| M0-9 | docs: protocol.md and hardware-notes.md | S | review; numbers come from the H issues | M0-4b to M0-7b |

**M1 Static pad (v0.1.0)**: all S except the two packaging checks.

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M1-1 | feat(core): profile model, TOML load, JSON Schema | S | `just test -p keyvo-core`, schema fixtures | |
| M1-2 | feat(core): toml_edit round-trip editing API | S | round-trip fixtures (comments, order preserved) | M1-1 |
| M1-3 | feat(core): renderer with insta snapshots and `keyvo preview` | S | `just test`, `insta` snapshots, Inter font bundled | M1-1 |
| M1-4 | feat(core): actions chord, text, page, profile, mode; chord parser | S | unit tests | M1-1 |
| M1-5 | feat(daemon): exec and http actions, exec result to key `states`, `on_fail` | S | tests with a fake injector and a local HTTP stub (d4, d12) | M1-4 |
| M1-6 | feat(core): feedback primitives: running arc, success and error badges, `feedback_hold` | S | `insta` snapshots (d12) | M1-3 |
| M1-7 | feat(core): recipe action type and detectors | S | fixtures: sample project trees for each detector (d10) | M1-4 |
| M1-8 | feat(daemon): unified trace core, ring buffer, `trace.ndjson` rotation, `ctl trace`, `ctl tail`, `exec-check` | S | NDJSON fixtures under `tests/protocol-fixtures/` (d9) | M1-5 |
| M1-9 | feat(daemon): press kinds short, long, repeat; inotify hot-reload | S | tests with FakeHidTransport | M1-4 |
| M1-10 | feat(cli): `keyvo setup` interactive flow and `profile export|import` | S | tests drive the flow with scripted answers and a fake `doctor` (d11, d17) | M1-5 |
| M1-11 | chore: systemd user unit, udev rule, `doctor` checks, cargo-dist config | S | unit tests for the `doctor` checks; file content fixtures | |
| M1-12 | hardware: install path verified on the owner's machine, v0.1.0 tagged | H | owner: `keyvo setup`, service start, first paint | M1-11 |

**M2 Context**

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M2-1 | feat(core): auto and manual state machine, overlays, `general_hint`, return-to-context timeout | S | unit tests | M1-4 |
| M2-2 | feat(daemon): socket API v1 with `subscribe`, `set_key`, `push_overlay`, `notify_context`, dry run | S | NDJSON fixtures shared with TS and C# clients | M2-1, M1-8 |
| M2-3 | feat(core): project layer: `.keyvo/` discovery, precedence, key clearing, `[project_layer]` | S | fixture trees (d5) | M1-2 |
| M2-4 | feat(daemon): trust store, content hash, `keyvo project trust`, trust prompt protocol | S | tests (d6) | M2-3 |
| M2-5 | feat(daemon): context source: `notify_context` pid binding, project registry fallback, "unknown project" reason | S | tests with a fake WindowWatcher reporting pids (d7) | M2-2, M2-3 |
| M2-6 | feat(daemon): KWin script and D-Bus object, X11 watcher | S | zbus and x11rb tests against mocks; the script file itself | M2-1 |
| M2-7 | hardware: KWin script installed and reloaded on KWin restart; focus switch on the pad | H | owner | M2-6 |
| M2-8 | feat(daemon): plugin supervisor and two reference plugins (clock or timer, git branch and dirty state) | S | tests (d17) | M2-2 |
| M2-9 | feat(integrations): shell prompt hooks (bash, zsh, fish), tmux, git | S | shell tests in the container | M2-5 |
| M2-10 | feat(integrations): Claude Code plugin (hooks, MCP server) | S | fixtures for hook payloads | M2-2 |
| M2-11 | feat(core): `.lp5` import | S | sample `.lp5` fixtures (`inv` schema) | M1-2 |

**M3 Desk state and Dialpad**

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M3-0 | research: 0x4610 request and response layouts (ADR-0008 pass) | S | report under `docs/research/` | |
| M3-1 | feat(hid): Dialpad driver over Bolt: 0x1B04 buttons, 0x4610 dial and roller | S | fixtures `own` and M3-0 (d13) | M0-3, M3-0, M0-2 |
| M3-2 | hardware: Dialpad over Bolt and Bluetooth verified | H | owner | M3-1 |
| M3-3 | feat(hid): MX Master 4 and MX Mechanical as trigger sources, `getCidInfo` walk, volatile diverts | S | fixtures from `docs/research/hidpp-features-dump.txt` (d16) | M3-1 |
| M3-4 | hardware: MX triggers and Solaar divert warning verified | H | owner | M3-3 |
| M3-5 | feat(daemon): desk state per profile (decision pending, see §7) | S or dropped | | |
| M3-6 | feat(integrations): VS Code extension (`notify_context`) | S | extension tests | M2-5 |

**M4a GUI**: every issue is S against the simulator; one H issue at the end.

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M4a-1 | feat(app): Tauri 2 and Svelte 5 scaffold, theme bridge (KDE, GNOME), tokens, contrast check | S | `just app-test` (new recipe: vitest and svelte-check in the container), Playwright against the simulator (P1, d15) | M2-2 |
| M4a-2 | feat(app): simulator and socket client, mirror on Home, debug log | S | Playwright flows (d1, d3) | M4a-1 |
| M4a-3 | feat(app): shell, rail, header device selector, keypad preview, key card | S | Playwright, snapshot tests (d8) | M4a-2 |
| M4a-4 | feat(app): Profiles editor with immediate apply, undo, external editor, snapshots, concurrent edits | S | Playwright with a daemon in the container (d2, d14) | M4a-3 |
| M4a-5 | feat(app): Behaviors list, trigger rows, Conditions, Actions, Feedback cards, "Test this behavior" | S | Playwright (d4, d9) | M4a-4 |
| M4a-6 | feat(app): Projects screen and trust prompt | S | Playwright (d5, d6) | M4a-4, M2-4 |
| M4a-7 | feat(app): first-run wizard over `keyvo setup`, recipe mini-wizard, starter packs | S | Playwright with a fake `doctor` (d10, d11) | M4a-4, M1-10 |
| M4a-8 | feat(app): Dialpad panel, Integrations, Devices, Settings, tray icon, keyboard map and palette | S | Playwright (d13, d15, d17) | M4a-5 |
| M4a-9 | hardware: GUI mirror against the real Keypad and Dialpad, wizard end to end | H | owner, step-by-step User test (P2) | M4a-8 |

**M4b Flathub**: M4b-1 manifest and portal permissions (S, `flatpak-builder` in the
container); M4b-2 install on the owner's host and Flathub submission (H).

**M5 Windows**: M5-1 C# plugin skeleton and NDJSON fixtures (S, dotnet in the
container); M5-2 named-pipe bridge and the GUI theme bridge for Windows (S); M5-3 VM
build and Options+ coexistence (H).

### 4.3 Ordering changes against the current roadmap

- M4a no longer waits for M3: M4a-1 to M4a-5 need M2-2 (socket) and M1 only. The
  Dialpad panel content (M4a-8) waits for M3-1.
- M0-8 (`doctor`) and M1-11 split the `doctor` checks into testable predicates (S) and
  their host verification (H).
- M1 gains six issues from the decisions file (M1-5 to M1-10); M2 gains the project
  layer, trust, context source and reference plugins (M2-3, M2-4, M2-5, M2-8).
- M3-0 is the static pass already planned in the roadmap; it is S and can start now.

## 5. Order of work (atomic commits, one PR)

1. `docs(plans): add the gui and ux design plan (#29)`
2. `docs(design): add the gui and ux design document`
3. `docs(adr): add adr-0009 project layer and trust model`
4. `docs(spec): add project layer, recipes, trace and feedback; rewrite the desktop app section`
5. `docs(roadmap): split milestones into sandbox-complete and hardware-bound issues`
6. `docs: add the sandbox label and the sandbox-complete issue rule to contributing and the issue template`
7. `docs(security): describe the project layer trust model`
8. `docs(glossary): add behavior, project layer, trust, recipe, trace, simulator terms`
9. `docs(ui-concepts): add the concept sketches with a provenance readme`

Before every commit: `git status --short | awk '{print $2}' | xargs grep -nP
'\x{2014}|[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]'` prints nothing.

## 6. Decisions the owner still has to make

| # | Question | Recommendation |
|---|---|---|
| 1 | Issue label and milestone | `documentation`, no milestone (the roadmap revision spans all milestones); the design document itself is referenced from M4a. |
| 2 | `docs/ui-concepts/` storage (11 PNG, 18 MB) | Downscale on the host with `magick` to 1200 px wide; commit as plain files; no LFS. The originals stay in the owner's ChatGPT history. (Done as JPEG quality 88, 1.9 MB total: 1200 px PNG still came to 11 MB.) |
| 3 | Label name | `sandbox` (short, pairs with the existing `hardware`). |
| 4 | M3-5 desk state without Solaar | Not decided here; the roadmap row says "decision pending" and §16 of the spec gets a row. |

## 7. Acceptance criteria

- [ ] Every one of the 18 decisions and 4 principles is implemented by at least one
      section of `docs/design/gui-ux.md` or `docs/spec.md`, traceable by its number (a
      coverage table at the end of the design document lists decision to section).
- [ ] No decision is restated with changed meaning; where the design document adds a
      detail the decisions file does not fix (for example exact token names), it is marked
      "design detail, not a grill decision".
- [ ] Every issue row in `docs/roadmap.md` has a Layer value, a Proof or User test cell and
      a Depends on cell; no S row's Proof cell names `just run`, `just test-hw`, Solaar,
      udev, systemd or the VM (a title may name the file an S issue produces, such as the
      unit file or the KWin script).
- [ ] Every S row's proof names only recipes that exist in the justfile or are introduced
      by that row (M4a-1 introduces `just app-test`).
- [ ] ADR-0009 is listed in `docs/adr/README.md` as Proposed; the owner flips it to
      Accepted on merge.
- [ ] `CONTEXT.md` terms are used verbatim in the design document, the spec and the
      roadmap (grep for "behaviour" versus "behavior" as a GUI term: the glossary entry
      decides the spelling, and the spelling is consistent).
- [ ] No em-dash and no emoji in any changed file.
- [ ] `just fmt-check lint test deny` unaffected (docs only); the PR template's User test
      section is removed because no device behaviour changes; the Automated checks are
      ticked only after CI.

## 8. User test

None: documentation only.
