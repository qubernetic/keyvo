# ADR-0002: Two-layer development model

Status: Accepted
Date: 2026-09-09

## Context

The development machine runs Aurora (Fedora Kinoite, immutable, KDE Wayland).
The host has no `cargo`, `rustc` or `go`; it does have Node, `just`, `gh`,
docker, podman, distrobox and direnv. Installing toolchains on the host is
possible (distrobox, rustup in `$HOME`) but drifts from what CI runs.

The hardware, however, is only reachable from the host: `/dev/hidraw*` for the
Keypad and the Bolt receiver, `/dev/uinput` for input injection, the KWin D-Bus
interface for window focus, and the user's session bus. Containers can be given
device access, but the KWin script, uinput and the session bus make a
containerised hardware run more fragile than it is worth.

The quality bar (`qubernetic/copia-cli`) requires that every contributor and CI
produce the same lint, test and coverage results.

## Decision

Two layers with one command surface, the `justfile`:

1. **Devcontainer layer.** A docker compose service built from a Dockerfile with
   pinned Rust (from `rust-toolchain.toml`) and Node. All of `just setup`, `dev`,
   `test`, `build`, `lint`, `fmt`, `deny`, `cov`, `diagrams`, `docs` run inside
   it; `.devcontainer/devcontainer.json` points editors at the same image; CI
   uses the same image or the same pinned toolchain so results match.
2. **Host layer.** `just run` and `just test-hw` execute the binary that the
   container built (bind-mounted `target/`) directly on the host, with real
   devices, KWin and uinput. Hardware tests are `#[ignore]` in Cargo and are only
   enabled by `just test-hw`.

Nothing that needs Rust runs on the host; nothing that needs hardware runs in
the container.

## Consequences

Positive:

- Identical toolchain for owner, contributors and CI; no "works on my machine".
- The host stays clean and immutable-friendly.
- Hardware tests are explicit and cannot accidentally run in CI.
- The Windows VM (ADR-0007, project plan §1 row 6) fits the same pattern: build
  in a container, execute where the hardware is.

Negative:

- Every hardware iteration is a two-step loop (build in container, run on
  host); the `justfile` hides it but latency is higher than a native loop.
- Bind-mounted `target/` must be owned and cached correctly between container
  and host, and binaries must be built against a glibc the host can run.
- A contributor without docker or podman cannot use the supported path.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| rustup on the host | Toolchain drift from CI; pollutes an immutable desktop; every contributor sets up differently. |
| distrobox with the toolchain | Better than raw rustup but still per-machine state; the compose service is reproducible from the repo alone. |
| Everything in the container, including hardware | Device passthrough works for hidraw, but uinput, the session bus and the KWin script make it fragile; hardware tests would not reflect the real install path. |
| No container, native everywhere (copia-cli model) | copia-cli is Go with a trivial toolchain; Rust plus Node plus native HID dependencies benefit from the pinned image. |
