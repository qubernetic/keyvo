# keyvo roadmap

Milestones run in the order Pre-alpha, M4b, M0, M1, M2, M3, M4a, M5 (M4b
moved up on 2026-10-25, see the M4b section); the Beta epic after the
pre-alpha takes M4b and the M1 and M3 rows marked "beta" in Status. Each gets its own planning round before its issues
are opened, and every issue carries its plan in a plan file under `docs/plans/`
or in the issue body (CONTRIBUTING.md, "Sandbox-complete and hardware-bound
issues"). Definition
of done (DoD) is what must be true on `develop` for the milestone to close. The
decisions behind the order are in `docs/plans/2026-09-09-project-plan.md`,
`docs/plans/2026-10-24-gui-ux-grill-decisions.md` and `docs/adr/`.

## Sandbox-complete and hardware-bound issues

Every planned issue below carries a **Layer**:

- **S (sandbox-complete)**: buildable, testable and reviewable entirely inside
  the devcontainer. The PR is done when `just ci` is green and the reviewer can
  check every acceptance criterion from the diff, the fixtures and the test
  output. These are the issues the project's agents (the `q-horde` account,
  working in a docker sandbox without `/dev/hidraw*` or `/dev/uinput`) take.
  The "Proof" block of an S issue, in its plan file or in its issue body,
  names its fixtures and the exact `just` commands with their expected
  output; the project's agents use the issue body. It never names `just run`,
  `just test-hw`, Solaar, udev, systemd, the KWin script or the Windows VM.
- **H (hardware-bound)**: a step needs the device, `/dev/uinput`, Solaar,
  udev, systemd, KWin or the Windows VM. The owner runs the User test on the
  host. H issues are kept small: they verify an S issue on the device, record
  the measured numbers in `docs/hardware-notes.md` and fix only what the
  device contradicts. Code that grows inside an H issue moves to a new S
  issue.

A hardware feature is therefore two issues: the S issue implements against
`FakeHidTransport`, golden fixtures and captures, citing the protocol facts
from `docs/research/`; the H issue proves it on the device. The GitHub labels
are `sandbox` and `hardware`. S issues may be opened across milestone
boundaries when their inputs exist; the **Depends on** column lists them.

Fixture sources used below: `hc` = derived from the hcooper captures
(`docs/research/hcooper-captures.md`); `inv` = facts from
`docs/research/options-plus-inventory.md`; `own` = our captures after M0-2;
`golden` = frames recorded on the host by an H issue and committed under
`tests/protocol-fixtures/`.

## Bootstrap (2026-09, no milestone)

Repository, policy and skeleton, before any protocol code.

- B1 Rename `qubernetic/tessera` to `qubernetic/keyvo` (done).
- B2 Bootstrap commit on `main` (README, LICENSE, NOTICE, .gitignore), `develop` created (done).
- B3 GitHub settings: merge commits only, auto-delete branches, branch protection, labels, milestones (done).
- #1 `docs: project foundation`: spec v0.2, roadmap, ADRs, glossary, research, policies, templates (done).
- #2 `chore: workspace skeleton and dev environment`: four crates, toolchain pin, devcontainer, justfile (done).
- #3 `ci: build, test, lint, coverage, CodeQL`: Linux and Windows jobs, cargo-deny, coverage, docs to Pages (S, open).
- #4 `docs: architecture diagrams with archify`: first five diagrams, CI freshness check (S, open).
- #27 `docs(research): Logi Options+ device-communication inventory` and ADR-0008 (done).
- #29 `docs: GUI and UX design document, roadmap split` (done).

## Pre-alpha ([milestone 8](https://github.com/qubernetic/keyvo/milestone/8))

**Goal.** A pre-alpha build that shows the whole stack working end to end on
the owner's machine: the Keypad paints profile keys, presses run actions, the
profile follows the focused KDE window, and a minimal Tauri GUI mirrors and
edits the pad, so the direction can change before the layers harden.

**DoD.** Every slice below is merged into the epic branch `epic/34-pre-alpha`,
the owner's two hardware checks (#40, #52) passed, Home and Profiles pass the
critique of `docs/design/critique-2026-10.md` (#108, #109 re-run it; the
score to beat is 23/40), and the epic PR is merged into `develop`.

Each slice is a thin cut of the roadmap issue named in **From**; the roadmap
issue stays open for the rest. The slices form one chain on the epic branch
(CONTRIBUTING.md, "Branch types"), because almost every one touches the root
`Cargo.toml`, `Cargo.lock` or the same crate. The Proof block of every slice
is in its issue body. Out of scope: the project layer, trust, recipes, the
Dialpad, plugins and integrations, X11, Windows and every GUI screen beyond
Home and the one-key editor.

| Slice | Issue | Title | Layer | From | Depends on | Status |
|---|---|---|---|---|---|---|
| 0 | #34 | `docs(contributing): record issue-body proof blocks and epic branches` | S | conventions | | merged |
| 1 | #35 | `feat(hid): hid++ and vlp framing with feature resolution` | S | M0-3 | #34 | merged |
| 2 | #36 | `feat(hid): keypad claim, geometry and keep-alive` | S | M0-4a | #35 | merged |
| 3 | #37 | `feat(hid): vlp image writer with ack flow control` | S | M0-5a | #36 | merged |
| 4 | #38 | `feat(hid): key input and page button divert` | S | M0-6a | #37 | merged |
| 5 | #39 | `feat(cli): hidraw transport with probe, keys and fill` | S | M0-8 | #38 | merged |
| 6 | #40 | `test(hardware): run first light on the keypad` | H | M0-4b, M0-5b, M0-6b | #39 | passed |
| 6b | #41 | `docs(hardware): record the first-light measurements` | S | M0-9 | #40 | merged |
| 6c | #69 | `fix(hid): reclaim the display after a keep-alive expiry` | S | M0-4a | #40 | merged |
| 6d | #71 | `feat(cli): dump raw hid frames and probe any feature` | S | M0-8 | #69 | merged |
| 6e | #70 | `test(hardware): run the no-strip check after the composite paint` (was the visible key window measurement) | H | M0-5b | #72 | open, owner |
| 6g | #76 | `docs(contrib): add example profiles and the home test guide` | S | none | | merged |
| 6h | #77 | `fix(daemon): paint the panel background before the keys` (superseded by 6l) | S | M0-5a, M1-5 | #76 | merged |
| 6i | #80 | `feat(cli): add a margin option to keyvo fill` (kept as a diagnostic) | S | M0-5b | #77 | merged |
| research | #86 | `docs(research): options+ display update pipeline and reference diagrams` | S | none | | merged |
| 6j | #88 | `docs(spec): apply the display pipeline report corrections` | S | M0-9 | #86 | merged |
| 6k | #89 | `feat(cli): print the display fields in probe and add fill --full` | S | M0-8 | #88 | merged |
| 6l | #72 | `feat(render,hid,daemon): paint one composite image per change` (was 6f, the per-key margin, dropped) | S | M0-5a, M1-5 | #89 | merged |
| 6m | #92 | `docs(roadmap): add the m3 static pass and move m4b after the pre-alpha` | S | roadmap maintenance | #72 | in review |
| 7 | #42 | `feat(core): profile model and toml loading` | S | M1-1 | #39 | merged |
| 8 | #43 | `feat(core): key renderer and keyvo preview` | S | M1-3 | #42 | merged |
| 9 | #44 | `feat(core): chord parser, press resolution and feedback states` | S | M1-4, M1-6 | #43 | merged |
| 10 | #45 | `feat(daemon): drive the keypad from the active profile` | S | M1-5, M1-9 | #44 | merged |
| 11 | #46 | `feat(core): focus context and page buttons` | S | M2-1 | #45 | merged |
| 12 | #47 | `feat(daemon): kwin focus watcher` | S | M2-6 | #46 | merged |
| 13 | #48 | `feat(daemon): socket api v1 subset and keyvo ctl` | S | M2-2, M1-8 | #47 | merged |
| 14 | #49 | `build(app): tauri and svelte scaffold with ci build` | S | M4a-1 | #48 | merged |
| 15 | #50 | `feat(app): home mirror and virtual pad` | S | M4a-2 | #49 | merged |
| 16 | #51 | `feat(app): edit a key with immediate apply` | S | M1-2, M4a-4 | #50 | merged |
| 17 | #52 | `test(hardware): run the pre-alpha end-to-end check` | H | M2-7, M4a-9 | #51, #41 | open, owner |
| R1 | #81 | `fix(daemon): harden the kwin script, the socket and the app lockfiles` | S | none | #80 | merged |
| R3 | #82 | `docs: align the docs and tests with the pre-alpha as built` | S | none | #81 | merged |

Status as of 2026-10-25: `merged` is merged into `epic/34-pre-alpha`,
`passed` a hardware check the owner ran, `open, owner` a hardware check
still to run, `in review` an open pull request into the epic branch.
Slices 6c to 6i come from first light (#40) and the owner's requests; the
research issue #86 and slices 6j to 6m from the owner's display pipeline
plan change (Options+ paints one composite image per change); R1 and R3
from the epic pre-review.

## Beta (milestone `Beta`, epic #180)

**Goal.** A build the owner installs as a Flatpak on the Aurora host and
uses daily with both devices: the Keypad as today, the Dialpad over Bolt,
and combined key-plus-dial actions.

**DoD.** Every slice below is merged into the epic branch `epic/180-beta`;
the owner's hardware checks (the Dialpad over Bolt and Bluetooth, #187; the
Flatpak install and a week of daily use, #190) passed;
`docs/beta-test.md` was walked once end to end on the final epic commit;
the epic PR is merged into `develop`. The Flathub submission is the
owner's step after the merge.

**Rules.**

- One chain on `epic/180-beta`, because every code slice touches
  `docs/beta-test.md` and the spec, and the core slices share
  `profile.rs`, `state.rs`, `schemas/` and `daemon.rs`. Only the owner's
  slices (#185, #187, #190) run beside the chain.
- Findings of the owner's hardware re-test of the pre-alpha slices become
  `fix(...)` issues on `develop`, with their own PR into `develop`, placed
  before the next unstarted slice; that slice starts with
  `git merge origin/develop` on its branch, so its PR carries the fix into
  the epic. Nobody pushes to the epic branch directly, nothing is rebased.
- A PR into the epic does not close its issue: the Reviewer names the
  issue in the merge comment and closes it by hand with the PR number.
- The application id is `io.github.qubernetic.keyvo` (ADR-0005). The
  Flatpak ships the GUI and the CLI; the daemon runs on the host under the
  systemd user unit (spec §16 question 9).
- Every slice's User test block goes into `docs/beta-test.md` section 5;
  `docs/pre-alpha-test.md` is frozen.

Each slice is a thin cut of the roadmap issue named in **From**; the
roadmap issue stays open for the rest. The Proof block of every slice is
in its issue body. Out of scope: #161, #159, #160, #145, M5, the project
layer and trust (M2-3 to M2-5), plugins and integrations, the trace core
(M1-8), `keyvo setup` (M1-10), the X11 watcher (M2-6), cargo-dist and
tags, the MX triggers (M3-3, M3-4) and desk state (M3-5).

| Slice | Issue | Title | Layer | From | Depends on | Status |
|---|---|---|---|---|---|---|
| 0 | #181 | `docs(roadmap,beta-test): open the beta section and the beta test guide` | S | conventions | | in review |
| 1 | #182 | `chore(flatpak): manifest, portal and device permissions` | S | M4b-1 | #181 | open |
| 2 | #179 | `feat(core,daemon,app): a launch keeps the global page until its window arrives` | S | pre-alpha follow-up | #182 | open |
| 3 | #183 | `feat(core,daemon): run the on_fail action when a key's action fails` | S | M1-5 | #179 | open |
| 4 | #184 | `chore(contrib,cli): systemd user unit, udev rule and keyvo doctor` | S | M1-11 | #183 | open |
| 5 | #185 | `research: static pass over the dialpad features` | H | M3-0 | | open, owner |
| 6 | #186 | `feat(hid,cli): dialpad driver over bolt with a dial command` | S | M3-1 | #184, #185 | open |
| 7 | #187 | `test(hardware): verify the dialpad over bolt and bluetooth` | H | M3-2 | #186 | open, owner |
| 8 | #188 | `feat(core,daemon,app): dialpad bindings and the dialpad panel` | S | M3-1, M4a-8 | #186, #187 | open |
| 9 | #189 | `feat(core,daemon,app): a key as the dial modifier, held or toggled` | S | beta goal | #188 | open |
| 10 | #156 | `feat(core,app): page switches, page deletion and per-profile global pages` | S | #156 | #189 | open |
| 11 | #190 | `test(hardware): install the flatpak on the host and use it daily` | H | M4b-2 | #182, then the chain | open, owner |

Status as of 2026-10-27: `merged` is merged into `epic/180-beta`,
`passed` a hardware check the owner ran, `open, owner` a hardware or
research slice the owner runs, `in review` an open pull request into the
epic branch. Slice 5 runs in the owner's time from the start; its report
PR goes into the epic. Slice 11 starts on the bundle of slice 1 and ends
after slice 10.

## M4b: Flathub ([milestone 6](https://github.com/qubernetic/keyvo/milestone/6))

**Goal.** One-click install on any Linux desktop. Flatpak manifest, portal and
device permissions, Flathub submission; udev rule upstreamed to systemd so
Flathub users need no manual step.

**Order.** M4b runs right after the pre-alpha (owner's plan change of
2026-10-25). The owner's Aurora host is atomic and has no webkit packages, so
until the Flatpak exists the app runs only inside the `org.gnome.Platform//51`
runtime. M4b-1 packages the pre-alpha app and no longer waits for M4a-8; the
later M4a screens ship in the same manifest when they land.

| Issue | Title | Layer | Proof or User test | Depends on | Status |
|---|---|---|---|---|---|
| M4b-1 | `chore(flatpak): manifest, portal and device permissions` | S | `flatpak-builder` on the host (`just flatpak`), proven by the CI `flatpak` job | Pre-alpha (`epic/34-pre-alpha` merged into `develop`) | beta (#182) |
| M4b-2 | `hardware: Flatpak install on the owner's host, Flathub submission` | H | owner | M4b-1 | beta (#190, the install and daily use; the Flathub submission after the epic merge) |

## M0: Protocol ([milestone 1](https://github.com/qubernetic/keyvo/milestone/1))

**Goal.** Hardware truth: measure the Keypad on Linux, capture Options+ on
Windows, and implement the HID++ and VLP planes in Rust against those numbers.

**DoD.** `keyvo probe`, `keyvo keys --json`, `keyvo fill <color|png> [--key N|--all|--panel]`,
`keyvo paint-test`; brightness get/set; keep-alive task; hot-plug; page-button
divert; `docs/protocol.md` and `docs/hardware-notes.md` with measured values;
Windows build green.

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M0-1 | `research: TS quick-test with Julusian's library` | H | owner; measurement list A of the project plan §5 | |
| M0-2 | `chore: Windows VM and USB capture toolkit` | H | owner; VM scripts, USBPcap, `captures-<date>` release asset, fixtures extracted. The capture target is the Logi Plugin Service traffic, which drives the Creative Console (`docs/research/options-plus-inventory.md`) | |
| M0-3 | `feat(hid): HID++ 2.0 core` (framing, ROOT and FEATURE_SET walk, index resolution, swid, errors) | S | `just test`; golden tests from `hc` and `inv` | |
| M0-4a | `feat(hid): keypad enumeration, keep-alive and hot-plug` against `FakeHidTransport` | S | `just test`; fixtures `hc` (0x0008 exchange, VLP fn4 claim) | M0-3 |
| M0-4b | `hardware: keep-alive window and hot-plug verified on the Keypad` | H | owner; numbers into `docs/hardware-notes.md` | M0-4a |
| M0-5a | `feat(hid): VLP display plane` (geometry query, image writer, 24-bit length, ACK flow control, per-key and panel paint, size guard) | S | `just test`; golden frames `hc`; 0x19A1 function table `inv` | M0-3 |
| M0-5b | `hardware: paint timing, fragment pacing and JPEG size guard measured` | H | owner; `just run -- fill`, `paint-test`; numbers into `docs/hardware-notes.md` | M0-5a |
| M0-6a | `feat(hid): key and page-button input decoding` (0x13 held-set to press and release, 0x1B04 divert requests, ACK and echo filtering) | S | `just test`; report fixtures `hc` | M0-3 |
| M0-6b | `hardware: 0x1B04 divert and key input verified` | H | owner | M0-6a |
| M0-7a | `feat(hid): brightness 0x8040` (range, get, set) | S | `just test` | M0-3 |
| M0-7b | `hardware: brightness unit, value 0 and idle-dim re-assert` | H | owner | M0-7a |
| M0-8 | `feat(cli): probe, keys, fill, paint-test, doctor` | S | `just test`; CLI tests against `FakeHidTransport`; the `doctor` hardware checks are predicates with fixture inputs | M0-4a, M0-5a |
| M0-9 | `docs: protocol.md and hardware-notes.md` | S | review; the numbers come from the H issues | M0-4b, M0-5b, M0-6b, M0-7b |

## M1: Static pad, v0.1.0 ([milestone 2](https://github.com/qubernetic/keyvo/milestone/2))

**Goal.** A usable static macro pad: profiles on disk, keys rendered and
actions fired, installable as a user service, with the feedback and trace that
make a customised pad debuggable.

**DoD.** TOML profiles with JSON Schema and round-trip editing; renderer with
snapshot tests and `keyvo preview`; actions `chord`, `exec`, `http`, `text`,
`page`, `profile`, `mode`, `recipe`; `on_fail`; automatic `running`, `success`
and `error` states with the feedback primitives; recipes and detectors; short,
long and repeat presses; hot-reload on file change; trace core (ring buffer,
`trace.ndjson`, `keyvo ctl trace|tail`, `keyvo exec-check`); `keyvo setup`;
`keyvo profile export|import`; systemd user unit; udev rule with `keyvo doctor`
checks; first cargo-dist release tagged `v0.1.0`.

Open: `.lp5` (Options+ profile) import lands here or in M2.

| Issue | Title | Layer | Proof or User test | Depends on | Status |
|---|---|---|---|---|---|
| M1-1 | `feat(core): profile model, TOML load, JSON Schema` | S | `just test`; schema fixtures under `crates/keyvo-core/tests/fixtures/` | | |
| M1-2 | `feat(core): toml_edit round-trip editing API` | S | `just test`; round-trip fixtures (comments, order, formatting preserved) | M1-1 | |
| M1-3 | `feat(core): renderer with insta snapshots and keyvo preview` | S | `just test`; `insta` snapshots for every primitive and the demo profiles | M1-1 | |
| M1-4 | `feat(core): actions chord, text, page, profile, mode; chord parser` | S | `just test` | M1-1 | |
| M1-5 | `feat(daemon): exec and http actions, result to key states, on_fail` | S | `just test`; fake `InputInjector`, local HTTP stub | M1-4 | beta (#183, `on_fail`) |
| M1-6 | `feat(core): feedback primitives running, success, error; feedback_hold, long_run_threshold, splash` | S | `just test`; `insta` snapshots | M1-3 | |
| M1-7 | `feat(core): recipe action type and detectors` | S | `just test`; sample project trees per detector under `crates/keyvo-core/tests/fixtures/projects/` | M1-4 | |
| M1-8 | `feat(daemon): unified trace core` (ring buffer, `trace.ndjson` rotation, `ctl trace`, `ctl tail`, `exec-check`) | S | `just test`; NDJSON fixtures under `tests/protocol-fixtures/trace/` | M1-5 | |
| M1-9 | `feat(daemon): press kinds short, long, repeat; inotify hot-reload` | S | `just test`; `FakeHidTransport` sequences | M1-4 | |
| M1-10 | `feat(cli): keyvo setup and profile export, import` | S | `just test`; the flow driven with scripted answers and a fake `doctor` | M1-5 | |
| M1-11 | `chore: systemd user unit, udev rule, doctor checks, cargo-dist config` | S | `just test`; unit tests for every `doctor` predicate; file content fixtures | | beta (#184, without the cargo-dist config) |
| M1-12 | `hardware: install path verified, v0.1.0 tagged` | H | owner; `keyvo setup`, service start, first paint, udev rule effective after replug | M1-11, M1-9 | |

## M2: Context ([milestone 3](https://github.com/qubernetic/keyvo/milestone/3))

**Goal.** The pad follows the focused window and the current project, and
external tools can drive it.

**DoD.** KWin script pushed by the daemon (reloaded on KWin restart) and X11
watcher; auto/manual state machine with page buttons; overlays and
`general_hint`; socket API v1 with `subscribe`, `press` (dry run),
`notify_context` with `cwd`, `project`, `trace`; `keyvo ctl`; project layer
with trust (ADR-0009) and context source (shell hooks, pid binding, registry
fallback); plugin supervisor with two reference plugins; Claude Code plugin
(hooks and MCP server); tmux and git integrations; `.lp5` import if not done
in M1.

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M2-1 | `feat(core): auto and manual state machine, overlays, general_hint, page grace` | S | `just test` | M1-4 |
| M2-2 | `feat(daemon): socket API v1` (`subscribe`, `set_key`, `push_overlay`, `notify_context`, `press` with dry run, `status`, `reload`) | S | `just test`; NDJSON fixtures under `tests/protocol-fixtures/` shared with the TS and C# clients | M2-1, M1-8 |
| M2-3 | `feat(core): project layer` (`.keyvo/` discovery, precedence, key clearing, `[project_layer]`) | S | `just test`; fixture trees under `crates/keyvo-core/tests/fixtures/projects/` | M1-2 |
| M2-4 | `feat(daemon): trust store, content hash, keyvo project trust, revoke, list, trust prompt protocol` | S | `just test` | M2-3 |
| M2-5 | `feat(daemon): context source` (`notify_context` pid binding, project registry fallback, unknown-project reason) | S | `just test`; fake `WindowWatcher` reporting pids | M2-2, M2-3 |
| M2-6 | `feat(daemon): KWin script, D-Bus object, X11 watcher` | S | `just test`; zbus and x11rb against mocks; the script file itself | M2-1 |
| M2-7 | `hardware: KWin script installed and reloaded on KWin restart; focus switch on the pad` | H | owner | M2-6 |
| M2-8 | `feat(daemon): plugin supervisor and two reference plugins` (clock or timer; git branch and dirty state) | S | `just test` | M2-2 |
| M2-9 | `feat(integrations): shell prompt hooks for bash, zsh, fish; tmux; git` | S | shell tests in the container (`just test` runs them) | M2-5 |
| M2-10 | `feat(integrations): Claude Code plugin` (hooks, MCP server) | S | `just test`; hook payload fixtures | M2-2 |
| M2-11 | `feat(core): .lp5 import` | S | `just test`; sample `.lp5` fixtures (schema from `inv`) | M1-2 |

## M3: Desk state and Dialpad ([milestone 4](https://github.com/qubernetic/keyvo/milestone/4))

**Goal.** The rest of the desk: Dialpad input, MX Master 4 and MX Mechanical
controls as trigger sources, and per-profile mouse and keyboard settings.

**DoD.** Dialpad over the Bolt receiver (buttons on `0x1B04`, dial and roller
on `0x4610`) with Bluetooth verified as fallback, the `0x4610` request and
response layouts taken from a targeted static pass under ADR-0008 plus the
Bolt capture; MX Master 4 and MX Mechanical divertable controls as triggers
(volatile diverts for the active profile, no haptics, `doctor` warns about
Solaar diverts); desk state per profile (open, spec §16 question 20); VS Code
extension.

**First step.** M3-0 runs before the M3 slices are planned: a static-analysis
pass under ADR-0008 over the Dialpad features, from `getCapabilities` to
`RotationEvent` (deeper-pass item 3 of `docs/research/options-plus-inventory.md`),
so the Dialpad work does not meet the surprise the Keypad display pipeline
did (#86). Every M3 slice depends on it.

| Issue | Title | Layer | Proof or User test | Depends on | Status |
|---|---|---|---|---|---|
| M3-0 | `research: static pass over the Dialpad features` (ADR-0008, inventory deeper-pass item 3: `0x4610` getCapabilities, getRollerCapabilities, getMode, setMode, getRollerSettings, setRollerSettings, RotationEvent) | S (research, sandbox) | report under `docs/research/` | | beta (#185, run by the owner) |
| M3-1 | `feat(hid): Dialpad driver over Bolt` (0x1B04 buttons, 0x4610 dial and roller, device index) | S | `just test`; fixtures `own` and M3-0 | M0-3, M0-2, M3-0 | beta (#186 the driver, #188 the profile bindings) |
| M3-2 | `hardware: Dialpad over Bolt and Bluetooth verified` | H | owner | M3-1, M3-0 | beta (#187) |
| M3-3 | `feat(hid): MX Master 4 and MX Mechanical as trigger sources` (`getCidInfo` walk, volatile diverts) | S | `just test`; fixtures from `docs/research/hidpp-features-dump.txt` | M3-1, M3-0 | |
| M3-4 | `hardware: MX triggers and the Solaar divert warning verified` | H | owner | M3-3, M3-0 | |
| M3-5 | `feat(daemon): desk state per profile` | S or dropped | decided with spec §16 question 20 | M3-0 | |
| M3-6 | `feat(integrations): VS Code extension` (`notify_context` with `cwd`) | S | extension tests (`just test`) | M2-5, M3-0 | |

## M4a: GUI ([milestone 5](https://github.com/qubernetic/keyvo/milestone/5))

**Goal.** The Tauri 2 + Svelte 5 app specified in `docs/design/gui-ux.md`: a
device mirror in the native desktop theme, profiles edited in place, a
simulator sandbox, behaviors with "Test this behavior", the first-run wizard
over `keyvo setup`, projects and trust, trace drawer. Scripts open in the
user's editor; TOML round-trip is preserved.

**DoD.** Every screen and flow of the design document; `just app-test` green
(vitest, svelte-check, Playwright against the simulator); theme bridge for KDE
and GNOME with the 4.5:1 token check; keyboard map complete; the owner's
end-to-end User test on the Keypad and the Dialpad.

Every issue is S against the simulator and a daemon in the container; one H
issue at the end. M4a-1 to M4a-5 need only M1 and the M2 socket, so they may
run in parallel with M2-3 onwards and with M3.

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M4a-1 | `feat(app): Tauri 2 and Svelte 5 scaffold, theme bridge, tokens, contrast check` (introduces `just app-test`) | S | `just app-test`; token contrast test; Playwright smoke | M2-2 |
| M4a-2 | `feat(app): simulator, socket client, mirror on Home, event list, debug log` | S | `just app-test`; Playwright flows | M4a-1 |
| M4a-3 | `feat(app): shell` (rail, header device selector, keypad preview, key card, collapsible columns) | S | `just app-test`; snapshot tests | M4a-2 |
| M4a-4 | `feat(app): Profiles editor` (immediate apply, undo, external editor, snapshots, external-change reload) | S | `just app-test`; Playwright with a daemon in the container | M4a-3 |
| M4a-5 | `feat(app): Behaviors` (list, trigger rows, Conditions, Actions, Feedback cards, Test this behavior, trace drawer) | S | `just app-test`; Playwright | M4a-4 |
| M4a-6 | `feat(app): Projects screen and trust prompt` | S | `just app-test`; Playwright | M4a-4, M2-4 |
| M4a-7 | `feat(app): first-run wizard over keyvo setup, recipe mini-wizard, starter packs` | S | `just app-test`; Playwright with a fake `doctor` | M4a-4, M1-10 |
| M4a-8 | `feat(app): Dialpad panel, Integrations, Devices, Settings, tray icon, keyboard map, palette` | S | `just app-test`; Playwright | M4a-5, M3-1 for the Dialpad content |
| M4a-9 | `hardware: GUI mirror against the Keypad and the Dialpad, wizard end to end` | H | owner; step-by-step User test | M4a-8 |

## M5: Windows ([milestone 7](https://github.com/qubernetic/keyvo/milestone/7))

**Goal.** The same profiles next to Logi Options+ on Windows through a C# Logi
Actions SDK plugin built and tested in the Windows VM. On Windows the Creative
Console is owned by the Logi Plugin Service that Options+ bundles
(`docs/research/options-plus-inventory.md`), so the plugin talks to that
service. The GUI is the same Tauri app behind the plugin's named pipe; the
simulator works without a device. Native mode (raw HID without Options+)
depends on the M0-2 ownership measurement. Not in v1 on Windows: project
layer via shell hook, MX buttons, desk state.

| Issue | Title | Layer | Proof or User test | Depends on |
|---|---|---|---|---|
| M5-1 | `feat(plugin): C# plugin skeleton and NDJSON client against the shared fixtures` | S | dotnet tests in the container | M2-2 |
| M5-2 | `feat(app): named-pipe bridge and the Windows theme bridge` | S | `just app-test` | M4a-1, M5-1 |
| M5-3 | `hardware: VM build, Options+ coexistence, Marketplace listing` | H | owner, Windows VM | M5-2, M0-2 |

## Deferred / open

- `.lp5` import placement: M1 or M2 (proposal: M2).
- Claude Code plugin key layout for the owner's workflow, grilled before M2-10.
- Desk state per profile without Solaar: spec §16 question 20, decided before M3-5.
- Flatpak manifest details (in M4b-1, Beta slice 1, #182), Windows native mode (after the M0-2 ownership test).
- MX Keypad (2026) PID and firmware compatibility: confirm as soon as an owner publishes `lsusb` or `solaar show`.
- Upstream: Solaar crash on the Dialpad (`settings_templates.py` in 1.1.20); udev `uaccess` rule for `046d:c354` in systemd.
- v2 candidates from the UI/UX grill: referenced `[[behaviors]]`, workflow insights from the trace, `keyvo pack add <git-url>`, plugin sandbox, system widgets.
