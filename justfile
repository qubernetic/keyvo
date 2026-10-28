# keyvo task runner (ADR-0002, two-layer dev model).
# Container recipes run inside the `rust` compose service; `run` and `test-hw`
# execute the container-built binary on the host, where the hardware is;
# `app-dev` serves the app against a mock daemon for a host browser.
# Podman users: `COMPOSE_CMD="podman compose" just ...` or put it in `.env`.
# Native mode: with AGENT_HORDE=1 the check and build recipes run the toolchain
# directly (for a shell that already is the container, such as the project's
# agent sandbox or `just shell`); the lifecycle and host recipes refuse.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true
set positional-arguments := true

compose := env("COMPOSE_CMD", "docker compose")
service := "rust"
uid     := `id -u`
gid     := `id -g`
env     := "KEYVO_UID=" + uid + " KEYVO_GID=" + gid
dc      := env + " " + compose
native  := env("AGENT_HORDE", "")
exec    := if native == "1" { "" } else { dc + " exec " + service }

# List recipes
default:
    @just --list --unsorted

# ---- Lifecycle -------------------------------------------------------------

# First-time setup: build the image, start the container, fetch dependencies
setup: _container_only
    {{ dc }} build
    {{ dc }} up -d
    {{ exec }} cargo fetch

# Start the dev container (detached)
dev: _container_only
    {{ dc }} up -d

# Stop the container (keep volumes and target/)
down: _container_only
    {{ dc }} down

# Stop the container, remove volumes, delete target/ and bin/
clean: _container_only
    {{ dc }} down -v
    rm -rf target bin

# Rebuild the image without cache (after Dockerfile changes)
rebuild: _container_only
    {{ dc }} build --no-cache

# Interactive shell in the container
shell: _container_only _up
    {{ exec }} bash

# ---- Build and checks (container) -----------------------------------------

# Build the workspace and copy host-runnable binaries to bin/ (profile: dev or release)
build profile="dev": _up
    {{ exec }} cargo build --workspace --profile {{ profile }}
    @case "{{ profile }}" in dev) dir=debug ;; *) dir="{{ profile }}" ;; esac; \
    mkdir -p bin; \
    install -m 755 "target/$dir/keyvo" "target/$dir/keyvo-daemon" bin/; \
    echo "binaries in bin/: keyvo, keyvo-daemon ($dir)"

# Run the test suite with cargo-nextest
test *args: _up
    {{ exec }} cargo nextest run --workspace {{ args }}

# Clippy on all targets, warnings are errors
lint: _up
    {{ exec }} cargo clippy --workspace --all-targets -- -D warnings

# Format the workspace
fmt: _up
    {{ exec }} cargo fmt --all

# Check formatting without changing files
fmt-check: _up
    {{ exec }} cargo fmt --all -- --check

# Licenses, advisories, bans and sources (deny.toml)
deny: _up
    {{ exec }} cargo deny check

# Coverage with cargo-llvm-cov in the nextest ci profile: lcov in coverage/lcov.info,
# JUnit in target/nextest/ci/junit.xml, plus a summary
cov: _up
    {{ exec }} mkdir -p coverage
    {{ exec }} cargo llvm-cov nextest --workspace --profile ci --lcov --output-path coverage/lcov.info
    {{ exec }} cargo llvm-cov report --summary-only

# ---- Desktop app (app/) ----------------------------------------------------

# App checks: clean install from package-lock.json, svelte-check, vitest, and
# both app lockfiles against advisories and licences (app/src-tauri/deny.toml)
app-test: _up
    {{ exec }} npm --prefix app ci --no-fund --no-audit
    {{ exec }} npm --prefix app run check
    {{ exec }} npm --prefix app test
    {{ exec }} npm --prefix app audit --audit-level=high
    {{ exec }} cargo deny --manifest-path app/src-tauri/Cargo.toml check

# Build the frontend and the Tauri shell (outside the root workspace, needs the
# dev image's webkit2gtk) and copy keyvo-app to bin/ (profile: dev or release)
app-build profile="dev": _up
    {{ exec }} npm --prefix app ci --no-fund --no-audit
    {{ exec }} npm --prefix app run build
    {{ exec }} cargo fmt --manifest-path app/src-tauri/Cargo.toml -- --check
    {{ exec }} cargo clippy --manifest-path app/src-tauri/Cargo.toml --locked --all-targets -- -D warnings
    {{ exec }} cargo build --manifest-path app/src-tauri/Cargo.toml --locked --profile {{ profile }} --features tauri/custom-protocol
    @case "{{ profile }}" in dev) dir=debug ;; *) dir="{{ profile }}" ;; esac; \
    mkdir -p bin; \
    install -m 755 "app/src-tauri/target/$dir/keyvo-app" bin/; \
    echo "binary in bin/: keyvo-app ($dir)"

# ---- Flatpak (contrib/flatpak/) --------------------------------------------

# Regenerate the manifest's offline sources (cargo-sources.json from both
# Cargo.lock files, node-sources.json from app/package-lock.json) with the
# pinned flatpak-builder-tools generators (tools/flatpak/)
flatpak-sources: _up
    {{ exec }} python3 tools/flatpak/generate.py

# Fail when the committed Flatpak sources differ from the lockfiles
flatpak-sources-check: _up
    {{ exec }} python3 tools/flatpak/generate.py --check

# Host: build and install the Flatpak from the checkout with flatpak-builder
# (the first run downloads the GNOME SDK), then lint it when
# flatpak-builder-lint is on the path
flatpak: _host_only
    flatpak-builder --user --install --install-deps-from=flathub --force-clean --ccache build/flatpak contrib/flatpak/io.github.qubernetic.keyvo.yml
    @if command -v flatpak-builder-lint >/dev/null; then \
        flatpak-builder-lint --exceptions --user-exceptions contrib/flatpak/lint-exceptions.json manifest contrib/flatpak/io.github.qubernetic.keyvo.yml; \
        flatpak-builder-lint --exceptions --user-exceptions contrib/flatpak/lint-exceptions.json builddir build/flatpak; \
    else echo "flatpak-builder-lint not on the path: lint skipped"; fi

# The exact sequence CI runs (.github/workflows/ci.yml), tests in the nextest ci profile
ci: fmt-check lint (test "--profile" "ci") deny cov

# ---- Host (hardware, app dev server) ---------------------------------------

# Serve the app on the Vite dev server at http://localhost:1420 against the
# in-memory mock of the daemon (app/src/lib/mock/), for a host browser:
# ?theme=breeze-dark|breeze-light|adwaita-dark|adwaita-light, ?mock-error=unreachable
app-dev: _host_only
    npm --prefix app ci --no-fund --no-audit
    cd app && VITE_KEYVO_MOCK=1 npm run dev

# Run the container-built keyvo binary on the host: just run -- --version
run *args: _host_only
    @[ -x bin/keyvo ] || { echo "bin/keyvo is missing: run 'just build' first" >&2; exit 1; }
    @[ "${1:-}" = "--" ] && shift; exec bin/keyvo "$@"

# Hardware checks on the host; runs `keyvo doctor` once it exists (roadmap M0)
test-hw *args: _host_only
    @echo "hidraw nodes:"; ls -l /dev/hidraw* 2>/dev/null || echo "  none"
    @echo "uinput:"; ls -l /dev/uinput 2>/dev/null || echo "  none"
    @echo "groups of $(id -un): $(id -nG)"
    @echo "keyvo doctor lands with M0; for the Keypad itself: just run -- probe"

# ---- Docs ------------------------------------------------------------------

# Regenerate docs/diagrams with archify (lands with Issue #4)
diagrams:
    @echo "just diagrams lands with Issue #4"

# Build the mdBook (lands with Issue #3)
docs:
    @echo "just docs lands with Issue #3"

# ---- Helpers ---------------------------------------------------------------

# Start the container if it is not running (no-op in native mode)
[private]
_up:
    @if [ "{{ native }}" != "1" ] && [ -z "$({{ dc }} ps --status running -q {{ service }})" ]; then {{ dc }} up -d; fi

# Refuse the container lifecycle recipes in native mode
[private]
_container_only:
    @if [ "{{ native }}" = "1" ]; then echo "not applicable in native mode (AGENT_HORDE=1): no container to manage" >&2; exit 1; fi

# Refuse the host recipes (hardware, app dev server) in native mode
[private]
_host_only:
    @if [ "{{ native }}" = "1" ]; then echo "not applicable in native mode (AGENT_HORDE=1): host recipes run on the owner's host" >&2; exit 1; fi
