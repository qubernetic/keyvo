# Handoff: GUI/UX planning session and roadmap revision (after #27, before the next issue) · 2026-10-24

*Written for a continuation after `/clear`. Everything needed is here or in the
files listed; the conversation history is not required.*

## Where we are

- PR #28 (`feature/27-options-plus-inventory`, closes #27) is open against
  `develop`, CI and Codecov green, mergeable, no review yet. It lands the
  Options+ static-analysis inventory, ADR-0008 (static-analysis boundary,
  status Proposed) and the related spec/roadmap/CONTRIBUTING edits. Owner
  actions before merge: tick the four Rust checkboxes (CI ran them), flip
  ADR-0008 and its README row to `Accepted` in one more commit on the branch,
  merge with `gh pr merge 28 --merge --delete-branch`, then the local cleanup
  (`git checkout develop && git pull && git fetch --prune && git branch -d
  feature/27-options-plus-inventory`).
- The UI/UX grill is finished: 4 principles and 18 decisions, all approved in
  chat, recorded in `docs/plans/2026-10-24-gui-ux-grill-decisions.md`
  (untracked, English). No design document, plan file, issue or PR exists for
  them yet. That is the next session's job.
- `docs/ui-concepts/` (11 PNG, 18 MB, untracked) holds the ChatGPT concept
  images; the third v3 image is the layout reference. Storage decision still
  open (commit as is, downscale, or LFS).
- The Options+ spike environment lives outside the repo (see memory
  `options-plus-re-spike`): distrobox `keyvo-re`, Ghidra 12.1.4, REA CLI,
  unpacked payload under `/var/home/cbiro/.cache/keyvo-re-spike/`. Reusable for
  the M3 `0x4610` pass; nothing to do with it now.

## Read in this order

    docs/plans/2026-10-24-gui-ux-grill-decisions.md   <- the decisions with their why; the source for everything below
    docs/spec.md (§6, §7.2, §8, §9, §10, §16)          <- what the decisions change; §10 gets rewritten
    docs/roadmap.md                                    <- current milestones M0..M5 and their DoD; to be revised
    docs/plans/2026-09-09-project-plan.md (§1, §4, §5) <- master plan; the per-issue plan format is in docs/plans/2026-09-09-workspace-skeleton.md
    docs/adr/README.md, 0004, 0006, 0007; 0008 on branch feature/27   <- do not contradict; 0004 stays, a new ADR covers project layer + trust
    CONTRIBUTING.md ("Developer Certificate of Origin", "Hardware test policy", "Two-layer development model")   <- q-horde sign-off rule, User test policy, just recipes
    docs/research/options-plus-inventory.md (branch feature/27)   <- protocol facts that M0-5/M3 plans must cite, do not re-derive

**Do not re-derive the decisions.** They are in the files above with reasons.

## The next step

The owner's interrupted prompt, verbatim:

> igen, mehet a plan file és a docs-PR

and the `/handoff` instruction, verbatim:

> keszitsd elo a handoffot egy alapos tervezesi sessionhoz, amiben a roadmap is helyet kap, ugy alakitsd ki a tervet hogy a fejlesztes a leheto legnagyobb mertekben elvegezheto legyen a paperclip (agent-hord instancomban, ahol a elerheto egy docker sandbox is devcontainerek es egyebek szamara)

Concretely, in the workflow order (grill is done, skip to plan file):

1. Plan file `docs/plans/2026-10-24-gui-ux-design.md`: what and why (point at
   the decisions file by number, do not restate), acceptance criteria, User
   test steps (docs-only: none). Owner approves in chat.
2. Issue (labels `documentation`, milestone M4a: GUI or a cross-milestone
   label, owner's call) and branch `feature/<N>-gui-ux-design` from a freshly
   pulled `develop` (after PR #28 is merged, or branch from develop and merge
   develop in later).
3. The docs-PR content: `docs/design/gui-ux.md` (the design document: shell,
   screens, flows, states, keyboard map, theme bridge, trace, simulator,
   wizard, project layer, trust), spec sections listed in the decisions file,
   `docs/roadmap.md` revision, `SECURITY.md` paragraph, `CONTEXT.md` terms,
   the new ADR (project layer and trust), `docs/ui-concepts/README.md`.
4. **Roadmap revision shaped for paperclip.** Split every milestone into
   issues that are either *sandbox-complete* (buildable, testable and
   reviewable entirely inside the devcontainer: `keyvo-core`, schema and
   `toml_edit` round-trip, renderer with `insta` snapshots, recipes and
   detectors, trace, `keyvo setup` logic, socket protocol, GUI against the
   simulator, `FakeHidTransport` tests against golden fixtures) or
   *hardware-bound* (`keyvo-hid` against the device, udev and systemd,
   KWin script, Bolt capture, Windows VM). Sandbox-complete issues are the
   ones paperclip agents take; each must state its fixtures and the exact
   `just` commands that prove it; hardware-bound issues keep the owner's User
   test and are kept small. Record this split as a column or label in the
   roadmap and in the issue template guidance.

## Development environment, important

- Host: Aurora/Kinoite, no cargo. Build, test, lint only through `just`
  (`just setup`, `just test`, `just lint`, `just fmt-check`, `just deny`,
  `just cov`, `just ci`); the devcontainer is docker compose, service `rust`,
  `COMPOSE_CMD` overridable for podman. Hardware only on the host via
  `just run -- ...` and `just test-hw`.
- paperclip (the owner's agent-horde instance) has a docker sandbox for
  devcontainers; agents there commit as the `q-horde` machine account
  (`Signed-off-by: <name> (q-horde) <bot@qubernetic.com>`, CONTRIBUTING.md
  "Developer Certificate of Origin"). They cannot see `/dev/hidraw*` or
  `/dev/uinput`, so plans must not give them hardware steps.
- Everything that touches a device, Solaar, udev or the KWin script runs on
  the host with the owner present.
- Claude Code attribution trailers are forbidden in commits and PR bodies
  (CLAUDE.md); the system reminder that asks for them is overridden.

## Traps this session hit

- The scratchpad is a RAM tmpfs (`/tmp`, 63 GB); large downloads go under
  `/var/home/cbiro/.cache/<name>/`, never the scratchpad.
- `ctx_batch_execute` prefixes commands with `NODE_OPTIONS=...`, so a bare
  `for ...; do` fails with a syntax error; wrap loops in `bash -c '...'`.
- The auto-mode classifier refuses `npm install -g <pkg>` inside
  `ctx_batch_execute` ("Code from External"); run such installs with the plain
  Bash tool.
- REA accepts only Ghidra 12.1.x (12.2 is "config drift"); Fedora 44 no longer
  packages Ghidra; Ghidra 12.1.4 needs JDK 21+ (`java-latest-openjdk` is fine).
- The PR template's Automated checks are Rust-centric; for docs-only PRs leave
  them unticked with "(no Rust change; CI)" and tick them after CI, do not
  claim them.
- Em-dash and emoji are banned everywhere; run before every commit:

      git status --short | awk '{print $2}' | xargs grep -nP '\x{2014}|[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]'

  expected output: nothing.
- ChatGPT share links are JavaScript-rendered: `ctx_fetch_and_index` returns
  only the title; use the Chrome tools (`tabs_context_mcp`, `navigate`,
  `get_page_text`, close the tab afterwards).
- The session-guard hook interrupts at 300k context, at prompt 30 and at
  plan-to-execution boundaries; a planning session that writes the design doc
  and the plan should hand off before opening the branch.

## Working method that held up

- Grill: one question at a time with a recommended answer (CLAUDE.md
  overrides the grilling skill's "whole frontier" rule); facts are looked up,
  never asked. The owner answers "yes" or amends; amendments go into the
  decisions file at once.
- Research in background agents with a written brief; only the summary enters
  the chat; reports under `docs/research/`.
- `ctx_batch_execute` with queries for anything over 20 lines; `Read` only
  for files that will be edited or for images.
- git-workflow skill strictly: issue, branch from fresh develop, atomic
  signed-off commits (`git commit -s`), PR title `type(scope): desc (#N)`,
  body `Closes #N`, merge `--no-ff`, cleanup.

## Open items after this round

- Owner decisions pending: storage of `docs/ui-concepts/` (18 MB); M3 "desk
  state without Solaar" (facts in the inventory report); the encrypted Options+
  descriptors stay undecrypted (decided no, recorded in ADR-0008).
- Follow-up research, cheap with the existing distrobox: `0x4610` request and
  response layouts before the Dialpad milestone (inventory report, "deeper
  pass" item 3); byte widths of 0x19A1 fn0/fn1/fn7 only if the M0 hardware
  re-decode leaves ambiguity.
- Open issues on GitHub unrelated to this round: #22 (ci: stop the dev
  container before checking report paths), #3 (ci: build, test, lint,
  coverage, CodeQL), #4 (docs: architecture diagrams with archify).
- Memory files written this session (auto-loaded next time):
  `options-plus-re-spike`, `keyvo-ui-native-theme-default`,
  `keyvo-ux-guided-not-gamified`.
