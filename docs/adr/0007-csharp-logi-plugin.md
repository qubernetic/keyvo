# ADR-0007: C# Logi Actions SDK plugin for Windows

Status: Accepted
Date: 2026-09-09

## Context

On Windows, Logi Options+ owns the MX Creative Console: it holds the device,
renders the keys and applies profiles. keyvo's Windows story therefore has to
coexist with Options+ rather than replace it (spec §1 non-goals). Logitech's
Actions SDK is the supported way to add actions and dynamic key content to
Options+.

Research (`docs/research/web-research.md`, "Implications") found that only the
C# flavour of the Actions SDK can render dynamic key images; the Node flavour
cannot. Two other routes exist and are proven by others: raw HID from a native
process (notno's Rust daemon), which fights Options+ for the device, and the
Options+ icon editor, which freezes live keys.

The project owner's professional background is in C#/.NET. There is no Windows
machine; a libvirt Windows VM with USB passthrough of the Keypad and the Bolt
receiver is planned (project plan §1 row 6), reusing the TwinCat-MCP VM scripts,
and is also the capture platform for USBPcap.

## Decision

- The Windows bridge is a C#/.NET plugin for the Logi Actions SDK, living in
  `plugins/logi/` in the monorepo.
- The plugin is a thin bridge: it renders key images and exposes keyvo actions
  and overlays; profiles come from the same TOML files and the same socket
  protocol (a named pipe on Windows), with shared NDJSON fixtures under
  `tests/protocol-fixtures/` keeping the Rust, TypeScript and C# clients in step.
- The plugin is built and tested in the Windows VM over SSH; the Windows CI job
  covers `plugins/logi/` only. Cross-compilation of the Rust crates for Windows
  is a separate, day-one CI job.
- Milestone M5, after the GUI and Flathub. A Windows "native mode" that talks raw
  HID without Options+ stays an open question until the ownership test in M0-2
  (project plan §5 item 16) is answered.

## Consequences

Positive:

- Windows users keep Options+ for their other devices and get keyvo profiles on
  the Keypad without driver fights.
- Dynamic key images work, which the Node SDK cannot deliver.
- The VM doubles as the capture rig for M0.

Negative:

- A third language and toolchain (.NET) in the repository.
- The plugin's feature set is bounded by what the Actions SDK exposes; Dialpad
  and desk-state features may not map.
- Everything Windows-related depends on a VM that does not exist yet (M0-2).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Node Actions SDK plugin | Cannot render dynamic key images. |
| Native Rust daemon on Windows (notno model) | Competes with Options+ for the device; whether a second reader even receives reports is unmeasured (plan §5 item 16). Kept as the "native mode" open question. |
| Options+ icon editor and static profiles | Static only; live overlays impossible. |
| No Windows support | Rejected by the owner; the same profiles should follow the hardware between machines. |
