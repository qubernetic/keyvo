# Contributing

Thank you for your interest in improving keyvo.

keyvo is a small project with a strict process. The process exists so that a
device-level bug found six months from now can be traced to one issue, one
branch, one PR, and one atomic commit, and so that anyone with the hardware can
reproduce the test that proved a change worked. Please read this page before
opening a pull request.

## Where to start

- **Questions and ideas** go into a GitHub Issue. Use the issue templates; they
  ask for the details a maintainer needs.
- **Read the glossary** in [CONTEXT.md](CONTEXT.md). The words there (profile,
  page, overlay, VLP plane, keep-alive, ...) are used consistently in code,
  docs, issues, and the GUI. Use them the same way.
- **Read the decisions** in [docs/adr/](docs/adr/README.md) before proposing a
  change in direction. If a decision no longer holds, open an issue that
  proposes a new ADR superseding the old one.
- **The specification** is [docs/spec.md](docs/spec.md); the plan and
  milestones are in [docs/roadmap.md](docs/roadmap.md).

### Issue labels

| Label | Meaning |
|---|---|
| `bug` | Something is broken |
| `enhancement` | New feature or capability |
| `documentation` | Documentation only |
| `chore` | Maintenance, config, dependencies |
| `hotfix` | Urgent production fix, branched from `main` |
| `hardware` | Needs the physical device on the host to reproduce or verify |
| `windows` | Windows VM, USB capture, or the Logi Actions SDK plugin |
| `research` | Investigation, measurement, prior-art analysis; output is a document or a capture |

## Prerequisites

- **Docker** (or Podman with the docker-compose shim) — all Rust and Node
  tooling runs in a container
- **[just](https://just.systems/)** — command runner
- **[direnv](https://direnv.net/)** — optional, per-directory environment
- **GitHub CLI (`gh`)** — recommended for issue and PR management
- **VS Code / Cursor** — optional, supports "Reopen in Container"

No Rust toolchain is required on the host, and on an immutable desktop (the
maintainer runs Aurora / Fedora Kinoite) there is none.

## Two-layer development model

keyvo separates *where code is built* from *where it touches hardware*
([ADR-0002](docs/adr/0002-two-layer-dev-model.md)):

| Layer | Where | What runs there |
|---|---|---|
| Build, test, lint, coverage, docs, CI parity | devcontainer (docker compose, Rust + Node image) | `just setup dev test build lint fmt deny cov diagrams docs` |
| Hardware execution | host | the binary built in the container, via `just run` and `just test-hw` |

```bash
git clone https://github.com/qubernetic/keyvo.git
cd keyvo

just setup        # build the dev image, fetch dependencies
just dev          # start the dev container (detached)
just test         # cargo test inside the container
just build        # cargo build inside the container; binary lands in target/
just lint         # clippy with -D warnings
just fmt          # rustfmt
just deny         # cargo-deny (licenses, advisories, bans)
just cov          # cargo-llvm-cov report
just run -- probe # run the container-built binary on the host against real hardware
just test-hw      # hardware test suite on the host
just diagrams     # regenerate docs/diagrams from the archify IR
just docs         # build the mdBook
```

The `justfile`, `Dockerfile`, `docker-compose.yml`, and `.devcontainer/` land
with Issue #2 (workspace skeleton and dev environment). Until then this section
describes the target, not the current tree.

The container has no access to `/dev/hidraw*` or `/dev/uinput`. Anything that
needs the device runs on the host with the container-built binary. Claude Code
and other agents run on the host as well, never inside the container.

## Development workflow

keyvo follows a strict Gitflow. Every contribution goes through these steps:

```
1. Open a GitHub Issue describing the change (every branch traces to an issue)
2. Fork the repo (external contributors) or branch directly (maintainers)
3. Create a branch from develop:
   git checkout develop
   git pull origin develop
   git checkout -b feature/<issue>-<slug>
4. Write a failing test, make it pass, refactor (see Test-driven development)
5. Make atomic commits in Conventional Commits format, signed off (DCO)
6. Run the checks:
   just fmt && just lint && just test && just deny
7. Push and open a PR targeting develop:
   git push -u origin feature/<issue>-<slug>
   gh pr create --base develop
8. Fill in the PR template; tick the "Automated checks" items as they pass
9. If the PR touches device behaviour, the "User test" checklist is ticked by
   the maintainer on real hardware before merge
10. Merge with a merge commit (--no-ff); the branch is deleted automatically
11. Clean up locally:
    git checkout develop && git pull origin develop
    git fetch --prune && git branch -d feature/<issue>-<slug>
```

### Branch types

| Change type | Branch prefix | Base | Example |
|---|---|---|---|
| New feature | `feature/` | `develop` | `feature/42-keep-alive-task` |
| Bug fix | `fix/` | `develop` | `fix/57-brightness-unit` |
| Hotfix on a release | `hotfix/` | `main` | `hotfix/89-daemon-crash` |
| Release preparation | `release/` | `develop` | `release/0.1.0` |

Branch names are lowercase, hyphen-separated, prefixed with the issue number.

### Commit format

```
<type>(<optional scope>): <description>

<optional body>

Signed-off-by: Your Name <you@example.com>
```

Imperative mood, lowercase after the colon, no trailing period, description
line at most 72 characters. One logical change per commit; if the description
needs "and", split the commit.

| Type | When to use |
|---|---|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `chore` | Maintenance, config, dependencies |
| `refactor` | Code restructure without behaviour change |
| `test` | Adding or updating tests |
| `perf` | Performance improvement |
| `ci` | CI/CD configuration |
| `build` | Build system or dependency changes |

Scopes follow the crate or area: `hid`, `core`, `daemon`, `cli`, `app`,
`integrations`, `logi`, `contrib`, `docs`, `ci`.

### Pull requests

- **Title** in Conventional Commits format with the issue number:
  `feat(hid): add keep-alive task (#42)`
- **Body** contains `Closes #42`. PRs target `develop`, so GitHub does not
  auto-close the issue; the `close-linked-issues` workflow does it on merge.
- **One PR per issue.**
- **Merge commits only.** Squash and rebase merges are disabled on the
  repository; the atomic commit history is the point.
- **Documentation lands in the same PR** as the change it describes.
- Keep the PR template sections. Delete the "User test" section only when the
  PR touches no device behaviour.

## Developer Certificate of Origin

keyvo uses the [Developer Certificate of Origin](https://developercertificate.org/)
instead of a contributor licence agreement. By signing off a commit you certify
that you wrote the change or have the right to submit it under the project
licence.

Sign off every commit with your real name and a working email address:

```bash
git commit -s -m "feat(hid): add keep-alive task"
```

This appends `Signed-off-by: Your Name <you@example.com>`. Commits without a
sign-off, or signed off with a pseudonym, are not merged.

Do not add `Co-Authored-By`, `Generated-by`, or similar trailers for AI
assistants or tools. The person who signs off is the author of record and is
responsible for the change, regardless of which tools helped write it.

## Test-driven development

Tests come first. For every change:

1. **Red** — write a test that fails for the right reason.
2. **Green** — write the smallest change that makes it pass.
3. **Refactor** — clean up with the test still green.

Where the tests live:

| Layer | Test style | Hardware |
|---|---|---|
| `keyvo-core` | Unit tests; the render function and the state machine are pure and fully testable | none |
| `keyvo-hid` | Golden-buffer tests against captured frames; integration tests with `FakeHidTransport` | none |
| `keyvo-daemon` | Integration tests with a fake D-Bus / X11 and the fake transport | none |
| `keyvo-cli` | Snapshot tests of output; `--json` output is schema-checked | none |
| Renderer | [insta](https://insta.rs/) snapshot tests of rendered key images | none |
| `just test-hw` | Hardware suite on the host: probe, paint, keep-alive, brightness, input | Keypad / Dialpad |

Coverage is measured with `cargo-llvm-cov` and reported to Codecov. Targets are
`keyvo-core` at or above 85 % and `keyvo-hid` at or above 80 %. The report is
informational: CI does not fail on coverage, but a PR must not lower it without
saying why.

## Hardware test policy

keyvo talks to real devices, and the protocol was reverse-engineered by the
community. Unit tests prove the code matches our understanding; only the device
proves the understanding is right.

- Every PR that touches device behaviour (`keyvo-hid`, the daemon's device
  paths, `keyvo-cli` hardware commands, udev or systemd files) carries a
  `## User test` checklist in its description. The maintainer runs those steps
  on real hardware and ticks them before the PR is merged. Contributors without
  the hardware write the checklist; the maintainer executes it.
- **Stop Solaar during keep-alive and idle-dim measurements.** Solaar polls the
  same hidraw nodes and skews timing results.
- **Never divert the Creative Console devices in Solaar.** Diverts set through
  Solaar on the Keypad or Dialpad conflict with keyvo's own `0x1B04` diverts and
  are hard to undo without a reconnect.
- Measured numbers (keep-alive window, dim timing, fragment pacing, fps) go into
  `docs/hardware-notes.md` with the date, firmware version, and method, never
  into code comments alone.
- Raw USB captures are not committed. Decoded and derived fixtures live under
  `captures/` and `tests/protocol-fixtures/`; the raw `pcapng` files are
  attached to a GitHub release as assets.

## Code style

- Rust edition 2024, stable toolchain pinned in `rust-toolchain.toml`.
- `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean.
- No `unwrap()` or `expect()` in library code; `thiserror` error types in
  crates, `anyhow` only at binary boundaries.
- HID++ feature indices are resolved at runtime through `ROOT.getFeature`;
  never hard-code an index.
- Timing constants live under `[timing]` in the configuration with a documented
  default, never as bare literals.
- Keep files focused: one responsibility per module.

## Architecture decision records

Decisions that shape the project are recorded in [docs/adr/](docs/adr/README.md).
Read them before proposing a change that touches a recorded decision. To
propose a new decision, add an ADR with the next number in `Proposed` status in
your PR and link it from the issue; it becomes `Accepted` when the PR merges.

## Licence and trademarks

By contributing you agree that your contribution is licensed under the
[Apache License 2.0](LICENSE). The copyright and attribution notice is in
[NOTICE](NOTICE).

Logitech, Logi, MX, MX Creative Console, MX Creative Keypad, MX Creative
Dialpad, Logi Bolt, and Logi Options+ are trademarks of Logitech International
S.A. keyvo is an independent community project. Do not use the Logitech marks in
a way that suggests endorsement, and do not copy code from third-party projects
into keyvo without stating the origin and licence in the file header.

## Reporting bugs

Open an issue with the bug report template. Please include:

- The command you ran or the profile you loaded
- What happened, including the daemon log (`keyvo doctor` output helps)
- What you expected to happen
- Your distribution, desktop (KDE Wayland, X11, other), and `keyvo --version`
- Which devices are attached and how (USB, Bolt, Bluetooth)
