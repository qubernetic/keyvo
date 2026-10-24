# Plan: justfile native mode for AGENT_HORDE=1 (Issue #31)

> Chore plan for [#31](https://github.com/qubernetic/keyvo/issues/31). Governing
> decisions: ADR-0002 (two-layer development model), the roadmap's rule that a
> sandbox-complete issue's proof names the `just` recipes, and the agent-horde
> design's native test mode (`AGENT_HORDE=1` runs the toolchain without
> `docker compose exec`). Status: **approved in chat on 2026-10-24**.

## 1. Why

The project's agents work in a container that carries the pinned toolchain but no
container runtime, so every recipe that wraps `docker compose exec rust ...` is unusable
there. They have been running raw `cargo` commands, which drift from the recipes CI runs
and cannot be named in the Proof block of a sandbox-complete issue. The agent stack
already sets `AGENT_HORDE=1` for every agent; the justfile only has to honour it.

## 2. Outcome

`AGENT_HORDE=1 just test|lint|fmt|fmt-check|deny|cov|ci|build` runs the same commands
the container recipes run, directly. Without the variable nothing changes. Lifecycle and
hardware recipes refuse in native mode with a one-line reason.

## 3. Files

| File | Change |
|---|---|
| `justfile` | `native := env("AGENT_HORDE", "")`; `exec` is empty when native; `_up` is a no-op when native; `_container_only` guard on `setup`, `dev`, `down`, `clean`, `rebuild`, `shell`; `_host_only` guard on `run`, `test-hw`; header comment. |
| `CONTRIBUTING.md` | "Native mode" paragraph under "Two-layer development model". |
| `CLAUDE.md` | One sentence in "Development Environment". |

## 4. Proof

- `AGENT_HORDE=1 just -n test` prints `cargo nextest run --workspace`; `just -n test` prints the `docker compose exec rust` form.
- `AGENT_HORDE=1 just setup` and `AGENT_HORDE=1 just run -- probe` exit 1 with "not applicable in native mode".
- `just fmt-check lint test deny` green in the devcontainer (container mode unchanged).

## 5. Acceptance criteria

- [ ] The three proof items above hold.
- [ ] No em-dash, no emoji in the changed files.
- [ ] The PR template's User test section is removed (no device behaviour changes).

## 6. User test

None.
