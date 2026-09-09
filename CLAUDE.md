# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

keyvo turns the Logitech MX Creative Keypad (and later the Dialpad) into a context-aware, scriptable control surface for developers on Linux: a Rust daemon + CLI, later a Tauri/Svelte GUI, with a C# Logi Actions SDK bridge for Windows. Profiles switch with the focused window (KDE Wayland first, X11 fallback), the nine LCD keys are rendered locally, and everything is TOML plus an NDJSON socket.

**Owner:** Qubernetic (Apache-2.0, DCO). Quality bar: `qubernetic/copia-cli`.

**Audience:** developers only. GUI edits layout and icons; scripts stay in text; TOML round-trips.

## Language

- Chat with the owner in Hungarian (professional register).
- Everything that lands in the repository is English: code, comments, commits, docs, issues, PRs.

## Development Environment

Aurora / Fedora Kinoite (immutable) workstation, KDE Wayland. **No Rust, Go, or Cargo on the host.** Available on the host: `node`, `just`, `gh`, `docker`, `podman`, `distrobox`, `flatpak-builder`, `direnv`.

Two-layer model ([ADR-0002](docs/adr/0002-two-layer-dev-model.md)):

| Layer | Where | Commands |
|---|---|---|
| Build, test, lint, coverage, docs, CI parity | devcontainer (docker compose, Rust + Node image) | `just setup dev test build lint fmt deny cov diagrams docs` |
| Hardware execution | host, with the container-built binary | `just run -- <args>`, `just test-hw` |

```bash
just setup        # build the dev image, start the container, fetch dependencies (first time)
just dev          # start the dev container (detached)
just test         # cargo nextest run --workspace (inside the container)
just build        # cargo build (inside the container), host-runnable binaries copied to bin/
just lint         # cargo clippy --workspace --all-targets -- -D warnings
just fmt          # cargo fmt --all (fmt-check only verifies)
just deny         # cargo-deny check
just cov          # cargo-llvm-cov report
just ci           # fmt-check + lint + test + deny, the sequence CI runs
just run -- probe # run the built binary (bin/keyvo) on the host against real hardware
just test-hw      # hardware checks on the host
just diagrams     # regenerate docs/diagrams with archify (Issue #4)
just docs         # build the mdBook (Issue #3)
```

**Claude Code runs on the host**, never inside the devcontainer. The container cannot see `/dev/hidraw*` or `/dev/uinput`; anything touching a device runs on the host. Never call `cargo` on the host; use the `just` recipes, or `docker compose exec rust cargo ...` for anything the justfile does not cover. The container runs as user `dev` with the host UID/GID passed from the justfile (`KEYVO_UID`/`KEYVO_GID`); `target/` is on the bind mount, the cargo registry and git caches are named volumes. Pin bumps: `rust-toolchain.toml` and the Dockerfile base tag move together; cargo tool versions are Dockerfile build args.

## Repository Layout

```
keyvo/
├── crates/
│   ├── keyvo-hid       # HidTransport impls, HID++ 2.0 core (root, feature set, 0x0008, 0x1B04,
│   │                   #   0x8040, 0x4610), VLP display plane (0x13/0x14, 0x19A1), Keypad + Dialpad
│   │                   #   drivers, hot-plug, keep-alive task, ACK-driven image writer
│   ├── keyvo-core      # profiles (TOML, toml_edit round-trip, JSON Schema), state machine
│   │                   #   (mode/page/overlay), renderer (tiny-skia/resvg/Inter -> JPEG), actions,
│   │                   #   socket protocol types, chord parser; sync, hardware-free, >=85 % covered
│   ├── keyvo-daemon    # tokio runtime: socket server, KWin/X11 watchers (zbus/x11rb), uinput
│   │                   #   injector (evdev), plugin supervisor, inotify hot-reload, systemd unit
│   └── keyvo-cli       # `keyvo probe|keys|fill|paint-test|preview|doctor|ctl|profile ...`
├── app/                # Tauri 2 + Svelte 5 GUI (M4a)
├── integrations/       # claude-code/ (plugin: hooks, commands, MCP), vscode/, tmux/, nvim/ ...
├── plugins/logi/       # C# Logi Actions SDK plugin (M5), built in the Windows VM
├── contrib/            # udev rule, KWin script, systemd unit, Flatpak manifest
├── captures/           # README + derived fixtures; raw pcapng external (release assets)
├── tools/windows-vm/   # libvirt VM scripts
├── tests/protocol-fixtures/  # shared NDJSON fixtures for Rust/TS/C# clients
└── docs/               # spec.md, roadmap.md, adr/, diagrams/, research/, plans/, hardware-notes.md
```

Platform traits in `keyvo-core`: `HidTransport`, `InputInjector`, `WindowWatcher`. Core rendering is the pure function `(profile, page, mode, overlay_stack, window_ctx) -> 9 KeySpecs + accent + dialpad bindings`.

## Workflow Loop

Every change follows this loop. Do not skip steps, do not reorder them.

1. **Grill**: `/grill-me` or `/grill-with-docs` until the design is settled. One question at a time, each with a recommended answer. Anything that can be found in a file or on the machine is not asked.
2. **Plan file**: `docs/plans/<YYYY-MM-DD>-<slug>.md`: what and why, acceptance criteria, User test steps.
3. **Approval**: the owner approves the plan in chat.
4. **Issue + branch**: Claude creates the GitHub issue (label, milestone) and the branch `feature/<issue>-<slug>` from a freshly pulled `develop`.
5. **TDD in the container**: red, green, refactor; `just fmt lint test deny` green before every commit.
6. **PR to `develop`**: title `<type>(<scope>): <desc> (#N)`, body `Closes #N`, the PR template's `## Automated checks` and `## User test` checklists filled in. The owner ticks User test on real hardware.
7. **Docs in the same PR**: spec, ADRs, hardware notes, mdBook pages updated together with the code.
8. **Merge `--no-ff`**: merge commit only; never squash or rebase into `develop` or `main`.
9. **Cleanup**: remote branch auto-deleted; locally `git checkout develop && git pull && git fetch --prune && git branch -d <branch>`; `git status` and `git clean -n` must show nothing unexpected.

Research tasks run as background agents; the report goes into `docs/research/`, and only the items that change the plan are discussed in chat. Large command outputs go to a file or through the context-mode tools, not into the conversation.

## Git Rules

Follow the `git-workflow` skill strictly. In short:

- Gitflow: `main` and `develop` are protected; PR-only; merge commits only.
- Every branch starts from an issue; one PR closes exactly one issue.
- Conventional Commits, imperative mood, lowercase, no trailing period, at most 72 characters.
- Atomic commits: one logical change each.
- DCO: every commit is signed off with `git commit -s` by the human author.
- **Never add `Co-Authored-By`, `Generated-by`, `Claude-Session`, or any other AI attribution trailer to commits, PR bodies, or files.** The owner chose the DCO rule over the harness default; the human sign-off is the only trailer.
- Never force-push, never `--amend` a pushed commit, never rebase a pushed branch.

## Coding Conventions

- Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt` clean.
- No `unwrap()` or `expect()` in library code. Crates define error types with `thiserror`; `anyhow` is used only in binaries (`keyvo-daemon`, `keyvo-cli`).
- `keyvo-core` is synchronous and hardware-free; `tokio` lives in the daemon.
- HID++ feature indices differ per device (`0x1B04` is index `0x0b` on the Keypad and `0x0a` on the Dialpad). **Always resolve through `ROOT.getFeature` at runtime; never hard-code an index.**
- Key geometry comes from the device (VLP display feature fn1); do not hard-code the layout.
- Timing constants (focus hysteresis, render coalescing, long-press, keep-alive interval, return-to-context timeout) live under `[timing]` in the configuration with a documented default. No bare timing literals in code.
- Tests: unit tests in `keyvo-core`, golden-buffer tests in `keyvo-hid` against captured frames, `FakeHidTransport` integration tests, `insta` snapshots for the renderer. Coverage targets `core >= 85 %`, `hid >= 80 %`, report-only.
- Third-party code is never copied without the origin and licence in the file header. Protocol knowledge is re-derived from research and our own captures.

## Hardware Facts (owner's machine)

| Item | Value |
|---|---|
| MX Creative Keypad | USB `046d:c354`, currently `/dev/hidraw2` (node number can change) |
| Logi Bolt receiver | USB `046d:c548`, currently `/dev/hidraw9`; paired: MX Mechanical, MX Master 4, MX Creative Dialpad (WPID `BC00`) |
| Permissions | `/dev/hidraw*` mode `0666`; `/dev/uinput` `root:input`; the owner is in the `input` group |
| Solaar | Flatpak `io.github.pwr_solaar.solaar` 1.1.20; CLI `flatpak run io.github.pwr_solaar.solaar show` |
| Windows VM | not created yet; scripts in `tools/windows-vm/` (from TwinCat-MCP) after M0-2 |

Known traps:

- `solaar show` **crashes on the Dialpad** (upstream bug in `settings_templates.py`). Use the raw feature walk in `docs/research/hidpp-features-dump.txt` instead.
- `solaar show` prints unknown feature IDs **byte-swapped**: `unknown:1602 {0216}` means the real ID is `0x1602`.
- The Flatpak Solaar cannot see `/tmp`; pass scripts on stdin: `flatpak run --command=python3 io.github.pwr_solaar.solaar - <<'EOF'`.
- **Stop Solaar during keep-alive and idle-dim measurements**; it polls the same hidraw nodes.
- **Never set diverts on the Creative Console devices in Solaar**; they conflict with keyvo's own `0x1B04` diverts.
- Feature indices differ per device; see Coding Conventions.
- Raw `.pcap` files under `captures/external/` are git-ignored on purpose; only the README, licence, and derived fixtures are tracked.

## Reference Documentation

- `CONTEXT.md`: glossary; use these terms exactly
- `docs/spec.md`: specification (what and why)
- `docs/roadmap.md`: milestones M0 to M5 and their definitions of done
- `docs/adr/`: architecture decision records; read before changing direction
- `docs/plans/`: per-issue implementation plans; `2026-09-09-project-plan.md` is the master plan
- `docs/research/`: protocol findings, captures decoded, prior-art analyses
- `CONTRIBUTING.md`: process, TDD, hardware test policy, DCO
- `SECURITY.md`: threat model (socket trusts the same user, like ssh-agent)
