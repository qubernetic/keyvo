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
| Build, test, lint, coverage, docs, CI parity | devcontainer (docker compose, Rust + Node image) | `just setup dev down clean rebuild shell build test lint fmt fmt-check deny cov ci app-test app-build flatpak-sources flatpak-sources-check diagrams docs` |
| Hardware execution | host, with the container-built binary | `just run -- <args>`, `just test-hw`; `just app-dev` (mock dev server, Node only); `just flatpak` (flatpak-builder) |

```bash
just setup        # build the dev image, start the container, fetch dependencies (first time)
just dev          # start the dev container (detached)
just down         # stop the container (keeps volumes and target/)
just clean        # stop the container, drop the cargo volumes, delete target/ and bin/
just rebuild      # rebuild the image without cache (after Dockerfile changes)
just shell        # interactive shell in the container
just test         # cargo nextest run --workspace (inside the container)
just build        # cargo build (inside the container), host-runnable binaries copied to bin/
just lint         # cargo clippy --workspace --all-targets -- -D warnings
just fmt          # cargo fmt --all
just fmt-check    # cargo fmt --all -- --check, verifies only
just deny         # cargo-deny check
just cov          # cargo-llvm-cov in the nextest ci profile; CI uploads it to Codecov
just ci           # fmt-check + lint + test --profile ci + deny + cov, the sequence CI runs
just app-test     # desktop app: npm ci, svelte-check, vitest, npm audit, cargo deny on app/src-tauri (works in native mode)
just app-build    # frontend build plus the Tauri shell (needs the dev image's webkit2gtk), bin/keyvo-app
just app-dev      # host: the app on the Vite dev server against the mock daemon (app/src/lib/mock/), :1420
just flatpak-sources        # regenerate contrib/flatpak/{cargo,node}-sources.json (pinned generators, tools/flatpak/)
just flatpak-sources-check  # fail when those files are stale against the three lockfiles (CI `app` job)
just flatpak      # host: flatpak-builder build and user install of contrib/flatpak/, then flatpak-builder-lint
just run -- probe # run the built binary (bin/keyvo) on the host against real hardware
just test-hw      # host checks: bin/keyvo doctor (device access, rule, session, unit, daemon, Solaar)
just diagrams     # placeholder: regenerating docs/diagrams with archify lands with Issue #4
just docs         # placeholder: the mdBook lands with Issue #3
```

**Claude Code runs on the host**, never inside the devcontainer. The container cannot see `/dev/hidraw*` or `/dev/uinput`; anything touching a device runs on the host. CI (`.github/workflows/ci.yml`) builds the same image from the `Dockerfile` and runs the five `just ci` commands in it in the same order (`fmt-check`, `lint`, `test --profile ci`, `deny`, `cov`); the nextest `ci` profile (`.config/nextest.toml`: no fail-fast, JUnit report in `target/nextest/ci/junit.xml`) is what `just ci` and CI share. The `ci` job is read-only and passes the coverage and JUnit reports as an artifact to the `codecov` job, the only job with `id-token: write`, which uploads them without running repository code; pull request runs only restore the caches. A second read-only job, `app`, builds the same image and runs the command lines of `just app-test`, `just app-build` and `just flatpak-sources-check`; it is the only place the Tauri shell compiles (the agent sandbox has no webkit2gtk). A third read-only job, `flatpak`, runs in the `flathub-infra/flatpak-github-actions` image of the manifest's runtime (pinned by digest, `--privileged` for flatpak-builder's sandbox): the manifest lint, the offline build of `contrib/flatpak/` by the SHA-pinned `flatpak-builder` action, `appstreamcli validate` on the metainfo, the repository lint, and the bundle `keyvo.flatpak` as its artifact (14 days). The agent sandbox has no `flatpak`; that job is the build proof. Never call `cargo` on the host; use the `just` recipes, or `docker compose exec rust cargo ...` for anything the justfile does not cover. Inside a shell that already is the container (the agent sandbox, `just shell`), `AGENT_HORDE=1` makes the check and build recipes run the toolchain directly (native mode, CONTRIBUTING.md "Two-layer development model"). The container runs as user `dev` with the host UID/GID passed from the justfile (`KEYVO_UID`/`KEYVO_GID`); `target/` is on the bind mount, the cargo registry and git caches are named volumes. Pin bumps: `rust-toolchain.toml` and the Dockerfile base tag move together, and the tag carries its `@sha256:` digest; Node and cargo-binstall versions are build args with a committed sha256 per architecture that moves with the version; cargo tool versions are Dockerfile build args (see CONTRIBUTING.md, "Bumping a pinned version").

## Repository Layout

```
keyvo/
├── crates/
│   ├── keyvo-hid       # HidTransport impls, HID++ 2.0 core (root, feature set, 0x0008, 0x1B04,
│   │                   #   0x8040, 0x4610), VLP display plane (0x13/0x14, 0x19A1), Keypad driver,
│   │                   #   Dialpad driver (Bolt receiver node, slot search, 0xff over Bluetooth),
│   │                   #   hot-plug, keep-alive task, ACK-driven image writer
│   ├── keyvo-core      # profiles (TOML, toml_edit round-trip, JSON Schema), state machine
│   │                   #   (mode/page/overlay), renderer (tiny-skia/resvg/Inter -> JPEG), actions,
│   │                   #   socket protocol types, chord parser; sync, hardware-free, >=85 % covered;
│   │                   #   assets/: bundled Inter font and lucide icons, origin and licence in its README
│   ├── keyvo-daemon    # tokio runtime: socket server, KWin watcher and logind session watcher
│   │                   #   (zbus), uinput injector (evdev), inotify hot-reload, /proc lineage,
│   │                   #   Keypad and Dialpad actors; planned: X11 watcher (x11rb), plugin supervisor
│   ├── keyvo-client    # socket client, mirror state, tile rendering through keyvo-core, key
│   │                   #   editor file side (round-trip write, snapshot) for the GUI; sync, hardware-free
│   └── keyvo-cli       # `keyvo probe|keys|dial|fill|preview|ctl|doctor`; planned: `paint-test|profile`
├── app/                # Tauri 2 + Svelte 5 GUI (M4a); src-tauri/ outside the root workspace
├── integrations/       # planned: claude-code/ (plugin: hooks, commands, MCP), vscode/, tmux/, nvim/ ...
├── plugins/logi/       # planned: C# Logi Actions SDK plugin (M5), built in the Windows VM
├── contrib/            # kwin/ focus script; profiles/ are the example profiles of docs/pre-alpha-test.md;
│                       #   recipes/ are the bundled recipe packs of the actions catalogue (test: keyvo-core
│                       #   contrib.rs); flatpak/ is the Flatpak io.github.qubernetic.keyvo (manifest,
│                       #   desktop file, metainfo, icon, generated cargo/node sources, lint exceptions;
│                       #   test: contrib.rs); systemd/ is the daemon's user unit, udev/ the uaccess rule
│                       #   for the hidraw nodes and /dev/uinput (test: contrib.rs)
├── captures/           # README + derived fixtures; raw pcapng external (release assets)
├── tools/windows-vm/   # planned: libvirt VM scripts
├── tools/keycodes/     # generator of the evdev key code table of the chord parser and the app's key names
├── tools/lucide/       # generator of keyvo-core's bundled lucide icons and their table, from the pinned release
├── tools/flatpak/      # pin and wrapper of the flatpak-builder-tools generators of contrib/flatpak/'s sources
├── schemas/            # generated JSON Schemas of profiles, recipe packs and config.toml; a keyvo-core test keeps them current
├── tests/protocol-fixtures/  # shared fixtures for Rust/TS clients: socket/ NDJSON, editor/, hid/ golden HID++ scripts
└── docs/               # spec.md, roadmap.md, socket-api.md, adr/, design/, research/, plans/,
                        #   hardware-notes.md, pre-alpha-test.md, beta-test.md; planned: diagrams/
```

"Planned" marks what the pre-alpha has not built yet (`docs/roadmap.md` names the milestone).

Platform traits in `keyvo-core`: `HidTransport`, `InputInjector`, `WindowWatcher`, `SessionWatcher` (the session lock; logind in the daemon, `keyvo_daemon::logind`), `Lineage` (whether a window's process belongs to a launch; `/proc` in the daemon, `keyvo_daemon::lineage`). Clients implement none of them; `keyvo-client` is the client side of the socket, and `app/src-tauri` (its own `Cargo.lock`, compiled only by `just app-build` and the CI `app` job) is thin glue over it. Core rendering targets the pure function `(profile, page, mode, overlay_stack, window_ctx) -> 9 KeySpecs + accent + dialpad bindings`; as built it is `keyvo_core::render::page(stack, keys)`, the specs of one page over the global layer (`keyvo_core::stack`: a key the app profile lacks comes from the global profile's context page), with the state machine choosing profile, page and the active dial modifier key (a held or toggled key whose `dial` and `roller` tables take the Dialpad over) and `render::lock_page` painting the lock under a locked session (spec §4.1, §6.1, §9).

## Workflow Loop

Every change follows this loop. Do not skip steps, do not reorder them.

1. **Grill**: `/grill-me` or `/grill-with-docs` until the design is settled. One question at a time, each with a recommended answer. Anything that can be found in a file or on the machine is not asked.
2. **Plan file or issue body**: `docs/plans/<YYYY-MM-DD>-<slug>.md` or the GitHub issue body: what and why, acceptance criteria, the Proof block (sandbox-complete) or User test steps (hardware-bound). The project's agents use the issue body and commit no plan file (CONTRIBUTING.md, "Sandbox-complete and hardware-bound issues").
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
- Slices of one plan that share `Cargo.toml`, `Cargo.lock` or a crate branch from an epic branch `epic/<issue>-<slug>` (base `develop`) and merge into it on the Reviewer's approval; the owner's merge gate applies to the epic PR into `develop` only (CONTRIBUTING.md, "Branch types").
- Conventional Commits, imperative mood, lowercase, no trailing period, at most 72 characters.
- Atomic commits: one logical change each.
- DCO: every commit is signed off with `git commit -s` by the human author.
- **Never add `Co-Authored-By`, `Generated-by`, `Claude-Session`, or any other AI attribution trailer to commits, PR bodies, or files.** The owner chose the DCO rule over the harness default; the human sign-off is the only trailer.
- Never force-push, never `--amend` a pushed commit, never rebase a pushed branch.

## Coding Conventions

- Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml`.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt` clean.
- No `unwrap()` or `expect()` in library code. Crates define error types with `thiserror`; `anyhow` is used only in binaries (`keyvo-daemon`, `keyvo-cli`).
- `keyvo-core` and `keyvo-client` are synchronous and hardware-free; `tokio` lives in the daemon.
- HID++ feature indices differ per device (`0x1B04` is index `0x0b` on the Keypad and `0x0a` on the Dialpad). **Always resolve through `ROOT.getFeature` at runtime; never hard-code an index.**
- Key geometry comes from the device (VLP display feature fn1); do not hard-code the layout.
- Timing constants (focus hysteresis, render coalescing, long-press, keep-alive interval, `page_return_global`, `page_return_own`) live under `[timing]` in the configuration with a documented default. No bare timing literals in code.
- Tests: unit tests in `keyvo-core`, golden-buffer tests in `keyvo-hid` against captured frames, `FakeHidTransport` integration tests, daemon end-to-end tests (`crates/keyvo-daemon/tests/`: a configuration directory on disk, `Pad`s and `Dial`s (a Bolt receiver with the Dialpad) built on `FakeHidTransport`, `FakeInputInjector`, `FakeWindowWatcher`, `FakeSessionWatcher`, `FakeLineage` or the `/proc` lineage), table tests for the context state machine (`crates/keyvo-core/tests/state.rs`), `insta` snapshots for the renderer (binary PNG snapshots in `crates/keyvo-core/tests/snapshots/`; after a deliberate change `INSTA_UPDATE=always just test` rewrites them, then look at every changed PNG before committing). Coverage goals `core >= 85 %`, `hid >= 80 %`; the Codecov org gates block a PR (`codecov/patch` >= 80 %, `codecov/project` at most 1 % below the base).
- Third-party code is never copied without the origin and licence in the file header; bundled assets (fonts, icons) carry it in `crates/keyvo-core/assets/README.md` and the licence text next to the files. Protocol knowledge is re-derived from research and our own captures.

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
- `docs/hardware-notes.md`: measured device behaviour (date, firmware version, method) and the first-light checklist
- `docs/pre-alpha-test.md`: the owner's pre-alpha home session (#70, #52) and the collected User test checklists of the epic's slice PRs; a slice PR merged into `epic/34-pre-alpha` adds its User test items there; frozen at e00b452 except the #179 block
- `docs/beta-test.md`: the owner's beta sessions (#180) and the collected User test checklists of the beta epic's slice PRs; a slice PR into `epic/180-beta` adds its User test block to its section 5
- `CONTRIBUTING.md`: process, TDD, hardware test policy, DCO
- `SECURITY.md`: threat model (socket trusts the same user, like ssh-agent)
