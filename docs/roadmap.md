# keyvo roadmap

Milestones run in order; each gets its own planning round before its issues
are opened, and every issue carries a plan file under `docs/plans/`. Definition
of done (DoD) is what must be true on `develop` for the milestone to close. The
decisions behind the order are in `docs/plans/2026-09-09-project-plan.md` and
`docs/adr/`.

## Bootstrap (2026-09, no milestone)

Repository, policy and skeleton, before any protocol code.

- B1 Rename `qubernetic/tessera` to `qubernetic/keyvo` — done.
- B2 Bootstrap commit on `main` (README, LICENSE, NOTICE, .gitignore), `develop` created — done.
- B3 GitHub settings: merge commits only, auto-delete branches, branch protection, labels, milestones — done.
- #1 `docs: project foundation` — spec v0.2, roadmap, ADRs, glossary, research, policies, templates.
- #2 `chore: workspace skeleton and dev environment` — four crates, toolchain pin, devcontainer, justfile.
- #3 `ci: build, test, lint, coverage, CodeQL` — Linux and Windows jobs, cargo-deny, coverage, docs to Pages.
- #4 `docs: architecture diagrams with archify` — first five diagrams, CI freshness check.

## M0 — Protocol · [milestone 1](https://github.com/qubernetic/keyvo/milestone/1)

**Goal.** Hardware truth: measure the Keypad on Linux, capture Options+ on
Windows, and implement the HID++ and VLP planes in Rust against those numbers.

**DoD.** `keyvo probe`, `keyvo keys --json`, `keyvo fill <color|png> [--key N|--all|--panel]`,
`keyvo paint-test`; brightness get/set; keep-alive task; hot-plug; page-button
divert; `docs/protocol.md` and `docs/hardware-notes.md` with measured values;
Windows build green.

- M0-1 `research: TS quick-test with Julusian's library` (host, hardware)
- M0-2 `chore: Windows VM and USB capture toolkit` (VM scripts, USBPcap, captures release asset, fixtures)
- M0-3 `feat(hid): HID++ 2.0 core` (framing, ROOT/FEATURE_SET, index resolution, errors, golden tests)
- M0-4 `feat(hid): keypad enumeration and keep-alive`
- M0-5 `feat(hid): VLP display plane` (geometry query, image writer, ACK flow control, per-key and panel paint)
- M0-6 `feat(hid): key and page-button input`
- M0-7 `feat(hid): brightness`
- M0-8 `feat(cli): probe, keys, fill, paint-test, doctor (hardware checks)`
- M0-9 `docs: protocol.md and hardware-notes.md`

## M1 — Static pad, v0.1.0 · [milestone 2](https://github.com/qubernetic/keyvo/milestone/2)

**Goal.** A usable static macro pad: profiles on disk, keys rendered and
actions fired, installable as a user service.

**DoD.** TOML profiles with JSON Schema and round-trip editing; renderer with
snapshot tests and `keyvo preview`; actions `chord`, `exec`, `http`, `text`,
`page`, `profile`, `mode`; short, long and repeat presses; hot-reload on file
change; systemd user unit; udev rule with `keyvo doctor` checks; first
cargo-dist release tagged `v0.1.0`.

Open: `.lp5` (Options+ profile) import lands here or in M2.

## M2 — Context · [milestone 3](https://github.com/qubernetic/keyvo/milestone/3)

**Goal.** The pad follows the focused window and external tools can drive it.

**DoD.** KWin script pushed by the daemon (reloaded on KWin restart) and X11
watcher; auto/manual state machine with page buttons; overlays and
`general_hint`; socket API v1 with `subscribe`; `keyvo ctl`; plugin supervisor;
Claude Code plugin (hooks and MCP server); tmux and git integrations; `.lp5`
import if not done in M1.

## M3 — Desk state and Dialpad · [milestone 4](https://github.com/qubernetic/keyvo/milestone/4)

**Goal.** The rest of the desk: Dialpad input and per-profile mouse and keyboard
settings.

**DoD.** Dialpad over the Bolt receiver (buttons on `0x1B04`, dial and roller on
`0x4610`) with Bluetooth verified as fallback; optional Solaar-backed desk state
per profile; VS Code extension.

## M4a — GUI · [milestone 5](https://github.com/qubernetic/keyvo/milestone/5)

**Goal.** A Tauri 2 + Svelte 5 app for people who prefer arranging nine tiles
visually: grid layout editor, icon picker, simple actions, dashboard, first-run
flow. Scripts open in the user's editor; TOML round-trip is preserved. Design
grill before planning.

## M4b — Flathub · [milestone 6](https://github.com/qubernetic/keyvo/milestone/6)

**Goal.** One-click install on any Linux desktop. Flatpak manifest, portal and
device permissions, Flathub submission; udev rule upstreamed to systemd so
Flathub users need no manual step.

## M5 — Windows · [milestone 7](https://github.com/qubernetic/keyvo/milestone/7)

**Goal.** The same profiles next to Logi Options+ on Windows through a C# Logi
Actions SDK plugin built and tested in the Windows VM. Native mode (raw HID
without Options+) depends on the M0-2 ownership measurement.

## Deferred / open

- `.lp5` import placement: M1 or M2 (proposal: M2).
- Claude Code plugin key layout for the owner's workflow, grilled before the M2 integration issue.
- First-run experience and `keyvo doctor` UX, grilled before the M1 udev/systemd issue.
- GUI design (before M4a), Flatpak manifest details (before M4b), Windows native mode (after the M0-2 ownership test).
- MX Keypad (2026) PID and firmware compatibility: confirm as soon as an owner publishes `lsusb` or `solaar show`.
- Upstream: Solaar crash on the Dialpad (`settings_templates.py` in 1.1.20); udev `uaccess` rule for `046d:c354` in systemd.
