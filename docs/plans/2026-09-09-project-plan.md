# keyvo: Project Plan (v1, 2026-09-09)

> Outcome of the planning grill of 2026-09-09 plus three research passes
> (`docs/research/`). This document is the bridge between `docs/spec.md` (what and why)
> and the per-issue implementation plans (`docs/plans/YYYY-MM-DD-<slug>.md`). Everything
> here was approved by the project owner unless marked **[open]**.

## 0. One-paragraph summary

keyvo is a Rust daemon + CLI (+ later Tauri/Svelte GUI) that turns the Logitech MX Creative
Keypad, and later the Dialpad, into a context-aware, scriptable control surface for
developers on Linux (KDE Wayland first, X11 fallback), with a Windows bridge via a C# Logi
Actions SDK plugin. Development is issue-driven Gitflow with TDD in a devcontainer and
hardware tests on the host. Quality bar: `qubernetic/copia-cli`. The name changed from
`tessera` to `keyvo` (free on crates.io/npm/PyPI/Flathub, unique in search).

## 1. Decisions locked in the grill

| # | Decision | Where it lands |
|---|---|---|
| 1 | Rust workspace core; TS quick-test with Julusian's library for hardware ground truth first | ADR-0001 |
| 2 | Two-layer dev model: devcontainer (docker compose + justfile) for build/test/lint/CI parity; host runs the built binary for hardware | ADR-0002 |
| 3 | Apache-2.0, DCO, copyright Qubernetic, Logitech trademark notice; no AI co-author lines | LICENSE, NOTICE, CONTRIBUTING |
| 4 | Diagrams with archify: JSON IR tracked, SVG tracked, HTML on Pages, pinned by git tag via justfile | docs/diagrams, justfile |
| 5 | Tooling: rust-toolchain.toml (pinned stable, edition 2024), clippy `-D warnings`, cargo-deny, cargo-llvm-cov → codecov (report only), CodeQL, cargo-dist, mdBook, dependabot, close-linked-issues workflow; Windows build+test job from day one; v0.1.0 at end of M1 | .github, justfile |
| 6 | Hardware verification: libvirt Windows VM (TwinCat-MCP scripts) with USB passthrough of `046d:c354` and Bolt `046d:c548`; USBPcap captures; pcapng as release assets, golden fixtures in git | tools/windows-vm, captures/ |
| 7 | Context engine defaults: overlays on context page + optional `general_hint`; return to context page on app switch unless page button used within 10 s; focus hysteresis 150 ms, render coalescing 50 ms, long-press 1000 ms; all under `[timing]` | spec §6, config.toml |
| 8 | No Lua in v1; NDJSON socket `{"v":1,…}` at `$XDG_RUNTIME_DIR/keyvo/keyvo.sock` (0600); `keyvo ctl`; supervised plugin processes with `plugin.toml`; no D-Bus façade in v1 | ADR-0003, docs/socket-api.md |
| 9 | Audience: developers. GUI v1 = grid layout editor, icon picker, simple actions, dashboard, first-run; exec scripts open in a configurable external editor; TOML round-trip (toml_edit) is an M1 invariant; JSON Schema for profiles; Svelte 5 | spec §10, ADR-0004 |
| 10 | Milestone order M0 → M1 → M2 → M3 → M4a (GUI) → M4b (Flathub) → M5 (Windows) | §4 |
| 11 | Name `keyvo`; no crates.io publishing in v1 | ADR-0005 |
| 12 | Dialpad via own HID++ 2.0 subset (0x1B04 + 0x4610 MultiRoller) on the Bolt receiver's hidraw; Solaar optional, only for mouse/keyboard desk state; no haptics in v1 | ADR-0006 |
| 13 | Config layout `~/.config/keyvo/{config.toml,profiles/<id>.toml,icons,themes,plugins}`; keys `1..9` row-major; class match accepts reverse-DNS and short names; tokio in daemon, sync `keyvo-core`; zbus, hidapi, evdev, toml_edit, schemars | spec §7, §12 |
| 14 | Desktop coverage: KDE Wayland (KWin script pushed by daemon, reloaded on KWin restart) + X11 in v1; GNOME/Hyprland/Sway via the `notify_context` contract | spec §6.2 |
| 15 | Renderer: tiny-skia + resvg + embedded Inter (OFL) + jpeg-encoder; insta snapshots; FakeHidTransport integration tests; coverage targets core ≥85 %, hid ≥80 %, report-only | spec §9, CONTRIBUTING |
| 16 | Integrations in monorepo `integrations/`: tier 1 Claude Code plugin (hooks + MCP server), VS Code extension, tmux, git; tier 2 Neovim, justfile page, gh, kubectl, other agent CLIs, Kitty/WezTerm; C# Logi plugin built/tested in the Windows VM | §4 M2+, ADR-0007 |
| 17 | Process: grill → plan file → approval → I create issue + branch → TDD → PR with automated + "User test" checklists → docs in same PR → `--no-ff` merge → cleanup | CONTRIBUTING, CLAUDE.md |

## 2. What the research changed (must be reflected in spec v0.2)

1. **Prior art exists**: logimap (Python, KDE, 2026-04) and logilinux (LauzHack 2025), see spec §1
   "nothing provides" becomes "no mature, scriptable solution"; both credited in a Prior-art
   table together with Julusian, hcooper, shensquared, notno, Bitfocus, mx-console-suite.
2. **Two protocol planes** on the Keypad: classic HID++ `0x11` (18 features, incl. 0x0008
   KEEP_ALIVE, 0x8040 BRIGHTNESS_CONTROL, 0x1B04 page buttons) and a VLP plane (`0x13` control
   / `0x14` bulk, own root, display feature 0x19A1 at VLP index 2). Images are baseline JPEG,
   118×118 per key or full panel 435×434 at (23,6) on a 480×480 canvas; 24-bit length field;
   per-fragment ACKs on `0x13` enable ACK-driven flow control. Device publishes its own key
   geometry (display fn1): query it, do not hard-code.
3. **Keep-alive is mandatory**: no pings → no input, no paints held. Options+ sets 3000 ms and
   pings every ~1 s. Firmware self-dims after idle/USB suspend; brightness re-assert needed.
4. **Dialpad**: 4 divertable buttons on 0x1B04 (CIDs 0x0053/56/59/5A), dial and roller on
   0x4610 MultiRoller (roller 0 = small wheel 40/rev, roller 1 = dial 180/rev, signed int8
   deltas); no keep-alive feature; Bolt path untested by anyone.
5. **Feature indices differ per device** (0x1B04 is 0x0b on Keypad, 0x0a on Dialpad): always
   resolve via ROOT.getFeature.
6. **No cross-compositor Wayland focus protocol** in KWin 6.7 / Mutter 51: backend per
   compositor stands. Event-driven KWin script, never polling (OpenDeck #425).
7. **Flatpak cannot deliver hidraw/uinput permissions**: a udev rule is required regardless;
   plan to upstream a `046d:c354` `uaccess` rule to systemd (Boatswain model).
8. **Options+ `.lp5` profiles are importable** (ZIP + JSON + base64 SVG): new M2/M3 feature.
9. **Logi Actions SDK**: only C# renders dynamic key images, which confirms the C# plugin.
10. **New hardware**: "MX Keypad" for developers launched 2026-09-08 (Windows/macOS only). Same
    audience as keyvo; PID/firmware compatibility **[open]**: confirm as soon as any owner
    publishes `lsusb`/`solaar show`.

## 3. Architecture (post-research)

```
keyvo/
├── crates/
│   ├── keyvo-hid       # HidTransport impls, HID++ 2.0 core (root, feature set, 0x0008, 0x1B04,
│   │                   #   0x8040, 0x4610), VLP display plane (0x13/0x14, 0x19A1), Keypad + Dialpad
│   │                   #   drivers, hot-plug, keep-alive task, ACK-driven image writer
│   ├── keyvo-core      # profiles (TOML, toml_edit round-trip, JSON Schema), state machine
│   │                   #   (mode/page/overlay), renderer (tiny-skia/resvg/Inter → JPEG), actions,
│   │                   #   socket protocol types, chord parser; sync, hardware-free, ≥85 % covered
│   ├── keyvo-daemon    # tokio runtime: socket server, KWin/X11 watchers (zbus/x11rb), uinput
│   │                   #   injector (evdev), plugin supervisor, inotify hot-reload, systemd unit
│   └── keyvo-cli       # `keyvo probe|keys|fill|paint-test|preview|doctor|ctl|profile …`
├── app/                # Tauri 2 + Svelte 5 GUI (M4a)
├── integrations/       # claude-code/ (plugin: hooks, commands, MCP), vscode/, tmux/, nvim/ …
├── plugins/logi/       # C# Logi Actions SDK plugin (M5), built in the Windows VM
├── contrib/            # udev rule, KWin script, systemd unit, Flatpak manifest
├── captures/           # README + derived fixtures; raw pcapng external (release assets)
├── tools/windows-vm/   # libvirt VM scripts (from TwinCat-MCP)
├── tests/protocol-fixtures/  # shared NDJSON fixtures for Rust/TS/C# clients
└── docs/               # spec.md, roadmap.md, adr/, diagrams/, research/, plans/, hardware-notes.md, mdBook
```

Platform traits in `keyvo-core`: `HidTransport`, `InputInjector`, `WindowWatcher`. Core
rendering is the pure function `(profile, page, mode, overlay_stack, window_ctx) → 9 KeySpecs
+ accent + dialpad bindings`.

## 4. Milestones and definition of done

### Bootstrap (no milestone, this week)
- B1 Rename GitHub repo `qubernetic/tessera` → `qubernetic/keyvo`, local dir → `~/Git/keyvo`.
- B2 Bootstrap commit on `main`: README (positioning, prior-art table, trademark notice),
  LICENSE (Apache-2.0), NOTICE, .gitignore; push; create `develop`.
- B3 GitHub settings: merge commits only, auto-delete branches, branch protection on
  `main`/`develop` (admin not exempt), labels (`bug`, `enhancement`, `documentation`, `hotfix`,
  `chore`, `hardware`, `windows`, `research`), milestones M0–M5, close-linked-issues workflow.
- Issue #1 `docs: project foundation`: spec v0.2 (all changes from §2), roadmap.md, ADR-0001…0007,
  CONTEXT.md glossary, research reports, this plan, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT,
  CLAUDE.md, PR template with "User test" section, `.github/ISSUE_TEMPLATE`.
- Issue #2 `chore: workspace skeleton and dev environment`: Cargo workspace with the four
  crates (empty but compiling), rust-toolchain.toml, Dockerfile (Rust + Node), docker-compose,
  devcontainer.json, justfile (`setup dev test build lint fmt deny cov run test-hw diagrams docs`),
  `.editorconfig`, `.gitattributes`, dependabot.
- Issue #3 `ci: build, test, lint, coverage, CodeQL`: Linux + Windows jobs, cargo-deny, llvm-cov
  → codecov, CodeQL, docs (mdBook) to Pages.
- Issue #4 `docs: architecture diagrams with archify`: install skill, pin tag, `just diagrams`,
  first five diagrams (architecture, key-press data flow, mode/page lifecycle, Claude hook
  sequence, dev/release workflow), CI check that SVGs are up to date.

### M0: Protocol (hardware truth)
DoD: `keyvo probe`, `keyvo keys --json`, `keyvo fill <color|png> [--key N|--all|--panel]`,
`keyvo paint-test`, brightness get/set, keep-alive task, hot-plug, page-button divert,
`docs/protocol.md`, `docs/hardware-notes.md` with measured numbers; Windows build green.
Issues (each with its own plan file):
- M0-1 `research: TS quick-test with Julusian's library` (hardware, host): answers §5 list A.
- M0-2 `chore: Windows VM and USB capture toolkit`: VM scripts, USBPcap procedure,
  Keypad + Dialpad (Bolt) captures under Options+, ownership test (Q11), release asset
  `captures-2026-09`, fixtures extracted.
- M0-3 `feat(hid): HID++ 2.0 core`: report framing (0x10/0x11), ROOT/FEATURE_SET walk, feature
  index resolution, swid, error handling, golden-buffer tests.
- M0-4 `feat(hid): keypad enumeration and keep-alive`: hidraw enumerate by VID/PID (+ descriptor
  dump), KEEP_ALIVE 0x0008 (range, timeout, 1 s task), display mode claim (VLP fn4 `a0`),
  hot-plug reconnect (re-claim, re-divert).
- M0-5 `feat(hid): VLP display plane`: 0x13/0x14 framing, geometry query (fn1), image writer
  with 24-bit length, fragment ACK flow control, per-key and full-panel paint, JPEG size guard.
- M0-6 `feat(hid): key and page-button input`: 0x13 key reports (held-set → press/release),
  0x1B04 divert for 0x01A1/0x01A2, ACK/echo filtering.
- M0-7 `feat(hid): brightness`: 0x8040 range/get/set, unit resolved by measurement, idle-dim
  re-assert policy.
- M0-8 `feat(cli): probe, keys, fill, paint-test, doctor (hardware checks)`.
- M0-9 `docs: protocol.md and hardware-notes.md`.

### M1: Static pad (v0.1.0)
Profiles (TOML, schema, round-trip), renderer (+ insta snapshots), `preview`, actions `chord`
(uinput), `exec`, `http`, `text`, `page`, `profile`, `mode`; press kinds short/long/repeat;
inotify hot-reload; systemd user unit; udev rule + `doctor` checks; cargo-dist release;
`.lp5` import **[open: M1 or M2]**.

### M2: Context
KWin script + daemon D-Bus object, X11 watcher, auto/manual state machine, page buttons,
overlays + `general_hint`, socket API v1 + `subscribe`, `keyvo ctl`, plugin supervisor,
Claude Code plugin (hooks → overlay, MCP server), tmux and git integrations, `.lp5` import.

### M3: Desk state and Dialpad
Dialpad over Bolt (0x1B04 buttons, 0x4610 MultiRoller), Bluetooth verification, Solaar-backed
mouse/keyboard settings per profile (optional), VS Code extension.

### M4a: GUI · M4b: Flathub · M5: Windows
As decided (§1 rows 9, 10, 16). Each gets its own grill before planning.

## 5. M0 measurement list (consolidated from research)

**A. TS quick-test (M0-1, host, Solaar stopped):**
1. `HIDIOCGRDESC` descriptor dump: how many hidraw nodes/collections does Linux expose for c354.
2. Silent device: with no keep-alive, do key presses arrive? Do paints hold? For how long?
3. keepAlive(3000) pinged every 1 s vs 3 s vs none: time until revert to logo / mode `a1`.
4. KEEP_ALIVE fn0 range reply (expect 500..10000 ms); behaviour at 500 and 10000.
5. Display mode: value at boot, after fn4 `a0`, after keep-alive expiry; is the `5e..65` VLP
   root fn2 call required?
6. Geometry table (VLP display fn1) exact decode vs Julusian constants.
7. Per-key paint (118×118) and full-panel paint (435×434) in one session; index base 0 vs 1;
   swid `0xb` vs `0xd`.
8. Fragment pacing: back-to-back vs 5 ms vs ACK-wait; tearing/"out of sync" observation.
9. Max sustainable fps: 1 key and 9 keys, JPEG q85 118×118; full-panel fps.
10. JPEG limits: > 65535 bytes (24-bit length), RGB565 rejection, quality floor, progressive vs baseline.
11. Brightness fn0/fn1 replies, unit (percent vs raw), effect of value 0, idle-dim timing with
    and without keep-alive, USB suspend behaviour.
12. Page buttons: divert volatility across reconnect; event bytes; long-press feasibility.
13. Own-write ACK (`13 ff 02 xx c1 00 01`) vs real key-1 press: exact discriminator.

**B. Windows VM captures (M0-2):**
14. Options+ startup and per-key paint on Keypad (compare with hcooper).
15. Dialpad over Bolt: 0x4610 configuration (fn0–fn3), divert of roller 0/1, event format,
    acceleration; 0x1B04 divert for the 4 buttons; wireless status notifications.
16. Ownership: with Options+ running, does a second reader on the vendor collection receive
    reports (Windows exclusive-open question).
17. Any keep-alive equivalent for the Dialpad; battery notifications.

## 6. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Single upstream protocol source (Julusian) | Own captures (M0-2), hcooper's pcaps decoded, hardware measurements before porting |
| Keep-alive/idle-dim semantics differ on Linux from macOS/Windows reports | M0-1 measures on Linux first; policies configurable |
| Dialpad over Bolt untested anywhere | M0-2 capture + M3 spike; Bluetooth as fallback path |
| Solaar/kernel hid-logitech-dj coexistence on the Bolt hidraw | Non-exclusive open, filter by device index, document; test with Solaar running and stopped |
| KWin script lifecycle (restart, session restore) | D-Bus name watch + reload; integration test with fake bus |
| Flatpak permission gaps | Standalone daemon is the primary install path; udev rule upstreaming issue; Flathub in M4b only |
| New "MX Keypad" may differ (PID/firmware) | Geometry and features are queried at runtime; add PID when known |
| Scope creep in integrations | Tier list fixed; each integration is its own issue after M2 core |

## 7. Documents to write in Issue #1

- `docs/spec.md` v0.2: rename, §1 prior art, §2 add MX Keypad status, §4 add VLP plane and
  `integrations/`, §5 rewrite from research (two planes, keep-alive, geometry query, ACK flow),
  §6 defaults, §7 config layout and `.lp5` import, §8 socket protocol details, §9 JPEG pipeline,
  §10 GUI scope, §11 C# plugin, §13 prior-art table, §16 open questions → resolved/issue links.
- `docs/roadmap.md`, `CONTEXT.md` (terms: profile, page, context page, general page, overlay,
  general hint, mode, desk state, VLP plane, keep-alive, display mode, focus source, plugin).
- ADR-0001 Rust core · 0002 two-layer dev model · 0003 no Lua v1 · 0004 GUI edits TOML with
  round-trip · 0005 no crates.io in v1 · 0006 Dialpad via own HID++ · 0007 C# Logi plugin.
- `CONTRIBUTING.md` (copia-cli structure + TDD + hardware test policy + DCO), `SECURITY.md`
  (socket trust model = same user as ssh-agent), `CODE_OF_CONDUCT.md`, `CLAUDE.md`,
  `.github/PULL_REQUEST_TEMPLATE.md` with `## User test` checklist.

## 8. Open items deferred to later grills

- `.lp5` import placement (M1 vs M2).
- Claude Code plugin key layout for the owner's workflow (before M2 integration issue).
- First-run experience and `doctor` UX (before M1 udev/systemd issue).
- GUI design (before M4a). Flatpak manifest details (before M4b). Windows native mode (after Q16).
- MX Keypad (2026) compatibility.
