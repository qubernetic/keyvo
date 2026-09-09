# ADR-0005: No crates.io publishing in v1

Status: Accepted
Date: 2026-09-09

## Context

The project was renamed from `tessera` to `keyvo` on 2026-09-09 because
`tessera` collided with an existing Rust GUI framework and a live trademark.
`keyvo` was chosen partly because the root name and the sub-crate names
(`keyvo-hid`, `keyvo-core`, `keyvo-daemon`, `keyvo-cli`) are free on
crates.io, npm and PyPI, and the Flathub ID `io.github.qubernetic.keyvo` is
available.

keyvo is an application, not a library. Its crates are internal layers whose
APIs will change freely during M0 to M3.

## Decision

- No crate is published to crates.io in v1 (through M5).
- Releases are binaries built by cargo-dist and attached to GitHub Releases;
  Linux packaging is deb, rpm and AppImage, then Flathub (M4b).
- The workspace uses path dependencies only; crate metadata carries
  `publish = false`.
- The name is protected by its uniqueness and by the GitHub organisation, not
  by a placeholder crate.

## Consequences

Positive:

- Internal APIs can break between milestones without semver obligations.
- No release step depends on crates.io tokens or on the workspace being
  publishable.
- Users install binaries, which is what an end-user daemon needs.

Negative:

- Anyone could register the `keyvo` crate names in the meantime. The project
  owner accepted this risk; the memory of the decision does not record a
  mitigation beyond monitoring.
- Third parties cannot depend on `keyvo-hid` as a library yet, although it may
  become useful to other Logitech projects.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Publish placeholder crates to reserve the names | crates.io discourages squatting; empty crates add noise. |
| Publish `keyvo-hid` early as a library | Its API is unmeasured until M0 finishes; publishing would freeze constants that the quick-test may prove wrong. |
| Publish everything from v0.1.0 | Adds semver and documentation obligations to every milestone with no user benefit. |
