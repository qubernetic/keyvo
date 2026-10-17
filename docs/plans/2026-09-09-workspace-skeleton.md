# Plan: workspace skeleton and dev environment (Issue #2)

> Implementation plan for [#2](https://github.com/qubernetic/keyvo/issues/2). Governing
> decisions: ADR-0001 (Rust core), ADR-0002 (two-layer dev model), project plan §1 row 5
> (tooling) and §4 Issue #2. Status: **awaiting approval**.

## 1. Outcome

A clean clone on a host with only `docker` (or `podman` with the compose plugin) and
`just` installed can run `just setup && just test && just lint && just build` and then
`just run -- --version` executes the container-built `keyvo` binary on the host. No Rust,
Node or cargo tool is installed on the host. The workspace has four empty-but-compiling
crates and one real test.

## 2. Files

| File | Purpose |
|---|---|
| `Cargo.toml` | Workspace root: members, `[workspace.package]` (version `0.0.0`, edition 2024, license, repository), `[workspace.lints]` (clippy `all` + `pedantic` as warn, `unwrap_used` deny in lib crates), `[workspace.dependencies]` empty for now, `[profile.release]` `lto = "thin"`, `strip = true` |
| `rust-toolchain.toml` | `channel = "1.98.0"` (matches the image tag; bump together), components `rustfmt`, `clippy`, `llvm-tools-preview` |
| `crates/keyvo-hid/` | lib, `#![forbid(unsafe_code)]` off (hidapi needs FFI later) but `#![deny(unsafe_op_in_unsafe_fn)]`; one doc-test-free placeholder module |
| `crates/keyvo-core/` | lib, `#![forbid(unsafe_code)]`, sync, no tokio |
| `crates/keyvo-daemon/` | bin `keyvo-daemon`, `main` prints version and exits 0 |
| `crates/keyvo-cli/` | bin `keyvo`, `--version` via `clap` derive; the only real dependency in this issue |
| `Dockerfile` | `rust:1.98.0-slim-trixie`; apt: `pkg-config libudev-dev libssl-dev git curl ca-certificates`; Node 24 LTS from the official tarball; `just`; cargo tools via `cargo-binstall`: `cargo-deny`, `cargo-llvm-cov`, `cargo-nextest`, `cargo-insta`; non-root user `dev` created with build args `UID`/`GID` (default 1000) so bind-mounted files stay owned by the host user |
| `docker-compose.yml` | service `rust`: build with `UID`/`GID` args, `working_dir: /workspace`, volumes `.:/workspace`, named `cargo-registry` (`/usr/local/cargo/registry`), named `cargo-git`, `target` stays on the bind mount so the host can run the binary; `command: sleep infinity`; `init: true` |
| `.devcontainer/devcontainer.json` | compose file above, service `rust`, workspace `/workspace`, extensions `rust-lang.rust-analyzer`, `tamasfe.even-better-toml`, `vadimcn.vscode-lldb`, `skellock.just`; settings: format on save, clippy as check command |
| `justfile` | see §3 |
| `deny.toml` | licenses allow: Apache-2.0, MIT, BSD-2/3, ISC, Unicode-3.0, Zlib, OFL-1.1 (Inter, later), MPL-2.0 (allowed, flagged); bans: none yet; advisories: deny unmaintained/yanked; sources: crates.io only |
| `.editorconfig` | copia-cli's, with `[*.rs] indent_style = space, indent_size = 4` and `[justfile] indent_style = space, indent_size = 4` |
| `.gitattributes` | copia-cli's, `*.rs text diff=rust`, `Cargo.lock text -diff=false` not set (keep diff), `*.snap text` |
| `.github/dependabot.yml` | ecosystems `cargo`, `github-actions`, `docker` (Dockerfile base image), weekly, `target-branch: develop`, prefixes `chore(deps):` / `ci:` / `build:` |
| `Cargo.lock` | committed (binaries) |
| `bin/` | gitignored; `just build` copies the host-runnable binaries here |

`.gitignore` already covers `/target/`, `/coverage/`, `.envrc`; add `/bin/`.

## 3. justfile contract

```
set shell := ["bash", "-euo", "pipefail", "-c"]
compose := "docker compose"
exec    := compose + " exec rust"

default            # list recipes
setup              # compose build (passes UID/GID) + compose up -d + cargo fetch
dev                # compose up -d
down / clean       # stop; stop + remove volumes + rm -rf target bin
shell              # interactive shell in the container
build [profile]    # cargo build --workspace; copy keyvo + keyvo-daemon to bin/
test               # cargo nextest run --workspace (falls back to cargo test if needed)
lint               # cargo clippy --workspace --all-targets -- -D warnings
fmt / fmt-check    # cargo fmt --all [-- --check]
deny               # cargo deny check
cov                # cargo llvm-cov nextest --workspace --lcov --output-path coverage/lcov.info + summary
run *args          # HOST: exec bin/keyvo "$@" (fails with a hint if bin/ is missing)
test-hw *args      # HOST: placeholder that runs bin/keyvo doctor once it exists; for now prints the hidraw/uinput permission checks from the handoff
diagrams / docs    # placeholders that print "lands with #4 / #3"
ci                 # fmt-check + lint + test + deny: the exact sequence CI runs (#3)
```

Rules: every container recipe first checks that the `rust` service is up and starts it if
not (`_up` helper); `run`/`test-hw` never touch docker. Podman users set
`compose := "podman compose"` via `just --set` or an `.env` `COMPOSE_CMD` override.

## 4. TDD sequence

1. **Red**: `crates/keyvo-cli/tests/version.rs`: `assert_cmd` runs `keyvo --version` and
   asserts stdout equals `keyvo <CARGO_PKG_VERSION>`. Also a `keyvo-core` unit test asserting
   `keyvo_core::VERSION == env!("CARGO_PKG_VERSION")` so the lib crate is exercised.
2. **Green**: minimal `clap` derive in `keyvo-cli`, `pub const VERSION` in `keyvo-core`.
3. **Refactor**: nothing to refactor; verify `just lint` is clean under pedantic.
4. `keyvo-hid` and `keyvo-daemon` get a `#[test] fn compiles()` placeholder so `nextest`
   lists all four crates.

## 5. Order of work (atomic commits)

1. `chore: add cargo workspace with four empty crates` (Cargo.toml, rust-toolchain.toml, crates/*, Cargo.lock)
2. `test(cli): assert keyvo --version prints the crate version` (red)
3. `feat(cli): print version with clap` (green)
4. `build: add devcontainer image and compose service`
5. `build: add justfile with container and host recipes`
6. `chore: add cargo-deny policy`
7. `chore: add editorconfig and gitattributes`
8. `chore: add dependabot for cargo, actions and docker`
9. `docs: describe the dev environment in CONTRIBUTING and CLAUDE.md` (replace the "lands with Issue #2" notes)

Commits 1–3 are done inside the container (`just shell` once the image exists, so
commit 4 is built first locally but committed in sequence; alternatively commits 4–5 go
first. Decide at execution and keep the history logical not chronological).

## 6. Verification (PR checklists)

Automated: `just ci` green in the container; `cargo tree` shows only `clap` and its
transitive deps; `deny` passes; image builds in under 10 minutes cold.

User test (host, Csaba):
- `git clone` into a scratch dir, `just setup` → image builds, no Rust on host used
- `just test` → four crates listed, 3 tests pass
- `just build && just run -- --version` → prints `keyvo 0.0.0`
- `ls -l target bin` → files owned by `cbiro`, not root
- `just clean && just setup` → idempotent

## 7. Out of scope

CI workflows (#3), diagrams (#4), any device code, Tauri app scaffolding (M4a), Windows
cross-compilation (CI-only in #3), pre-commit hooks (decide in #3).

## 8. Open points for approval

- Image base `slim-trixie` vs `slim-bookworm`: trixie proposed (newer glibc is fine, host
  runs Aurora 44 with a newer glibc still).
- `cargo-nextest` as the test runner from day one (faster, per-test isolation) vs plain
  `cargo test`: nextest proposed; `just test` hides the choice.
- Pedantic clippy as **warn** in this issue, promoted to deny per crate as code lands.
