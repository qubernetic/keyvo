# keyvo task runner (ADR-0002, two-layer dev model).
# Container recipes run inside the `rust` compose service; `run` and `test-hw`
# execute the container-built binary on the host, where the hardware is.
# Podman users: `COMPOSE_CMD="podman compose" just ...` or put it in `.env`.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true
set positional-arguments := true

compose := env("COMPOSE_CMD", "docker compose")
service := "rust"
uid     := `id -u`
gid     := `id -g`
env     := "KEYVO_UID=" + uid + " KEYVO_GID=" + gid
dc      := env + " " + compose
exec    := dc + " exec " + service

# List recipes
default:
    @just --list --unsorted

# ---- Lifecycle -------------------------------------------------------------

# First-time setup: build the image, start the container, fetch dependencies
setup:
    {{ dc }} build
    {{ dc }} up -d
    {{ exec }} cargo fetch

# Start the dev container (detached)
dev:
    {{ dc }} up -d

# Stop the container (keep volumes and target/)
down:
    {{ dc }} down

# Stop the container, remove volumes, delete target/ and bin/
clean:
    {{ dc }} down -v
    rm -rf target bin

# Rebuild the image without cache (after Dockerfile changes)
rebuild:
    {{ dc }} build --no-cache

# Interactive shell in the container
shell: _up
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

# Coverage report with cargo-llvm-cov: lcov in coverage/lcov.info plus a summary
cov: _up
    {{ exec }} cargo llvm-cov nextest --workspace --lcov --output-path coverage/lcov.info
    {{ exec }} cargo llvm-cov report --summary-only

# The exact sequence CI runs (Issue #3)
ci: fmt-check lint test deny

# ---- Host (hardware) -------------------------------------------------------

# Run the container-built keyvo binary on the host: just run -- --version
run *args:
    @[ -x bin/keyvo ] || { echo "bin/keyvo is missing: run 'just build' first" >&2; exit 1; }
    @[ "${1:-}" = "--" ] && shift; exec bin/keyvo "$@"

# Hardware checks on the host; runs `keyvo doctor` once it exists (roadmap M0)
test-hw *args:
    @echo "hidraw nodes:"; ls -l /dev/hidraw* 2>/dev/null || echo "  none"
    @echo "uinput:"; ls -l /dev/uinput 2>/dev/null || echo "  none"
    @echo "groups of $(id -un): $(id -nG)"
    @echo "keyvo doctor lands with M0; nothing else to run yet"

# ---- Docs ------------------------------------------------------------------

# Regenerate docs/diagrams with archify (lands with Issue #4)
diagrams:
    @echo "just diagrams lands with Issue #4"

# Build the mdBook (lands with Issue #3)
docs:
    @echo "just docs lands with Issue #3"

# ---- Helpers ---------------------------------------------------------------

# Start the container if it is not running
[private]
_up:
    @if [ -z "$({{ dc }} ps --status running -q {{ service }})" ]; then {{ dc }} up -d; fi
