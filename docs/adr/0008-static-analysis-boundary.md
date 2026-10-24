# ADR-0008: Static analysis of vendor software: interoperability boundary

Status: Proposed
Date: 2026-10-24

## Context

keyvo must speak the VLP display plane (feature `0x19A1`) and a few HID++ 2.0
features that Logitech has not documented publicly. Captures (ADR-0006, project
plan §5) give numbers: frame layouts, timings, ACK flow. They cannot give names
and semantics: which capability field is which, what the device states `a0..a6`
and `d0..d6` mean, what the hidden features are called. The only complete
implementation of that plane is Logitech's own software, shipped as a public
installer.

EU Directive 2009/24/EC permits two things for exactly this situation: Art. 5(3)
lets a lawful user observe, study and test a program while running it, which
covers the USB captures; Art. 6 lets a lawful user decompile the parts of a
program that are indispensable to achieve interoperability of an independently
created program, provided the information is not otherwise readily available,
the analysis is confined to those parts, and the result is not used for any
other purpose or for a substantially similar program. Art. 8 voids contract
terms that forbid this. Hungarian law transposes it in Szjt. 59. and 60. §.
Separately, circumventing a technological protection measure on data falls
under Directive 2001/29/EC Art. 6 and Szjt. 95. §, which the interoperability
exception does not cover.

A time-boxed spike on 2026-10-24 (`docs/research/options-plus-inventory.md`)
showed the approach works and that it needs a written boundary.

## Decision

- **Purpose.** Static analysis of Logitech software is done only to make keyvo
  interoperate with the MX Creative Keypad and Dialpad. Nothing is analysed for
  any other reason.
- **Scope.** Analysis is confined to the components and code paths that talk to
  the devices (device classes, HID++ and VLP feature implementations, device
  data-file schemas). Licensing, account, telemetry, updater, crash reporting,
  cloud and UI code are skipped and the report says so.
- **Output.** Reports in `docs/research/` carry protocol facts only: feature IDs,
  function numbers, field names and layouts, constants, enumerations, state
  names, error codes, string tables, data-file schemas, symbol names. Decompiled
  code, pseudocode, function bodies and copied comments never enter the
  repository, an issue, a PR or a commit message. Every claim states its
  evidence kind (string, symbol, decompiled request builder, data file, capture
  cross-check) and a confidence level.
- **Protection measures.** Encrypted data files and depots are inventoried
  (container format, header fields, key coverage) but not decrypted, even when
  the keys ship with the installer. Reopening this needs a concrete need that
  no other source can meet and an explicit owner decision recorded in an ADR.
- **Implementation stays clean-room.** Code in `keyvo-hid` is written from the
  documented facts and verified against captures and hardware, never
  transcribed from a decompiler. Decompiler output stays outside the repository.
- **Tooling and hygiene.** Analysis runs in an isolated container on the host
  (the devcontainer is not touched; Ghidra and the REA CLI are research tools,
  not build dependencies). No vendor binary is executed and no vendor server is
  contacted during analysis. Installers come from Logitech's public download
  CDN, and the report records the exact version analysed.
- **Audit trail.** Each report ends with a legal-boundary log: what was
  analysed, what was skipped and why, which functions were decompiled, and that
  nothing beyond facts was recorded.

## Consequences

Positive:

- Open protocol questions that captures cannot answer (names, semantics,
  enumerations) can be closed in minutes instead of being guessed at on
  hardware.
- The boundary is auditable from the repository alone: anyone can check that
  the research docs contain facts, not code.
- The Plugin Service helper is small and unstripped, so future targeted passes
  (for example `0x4610` before the Dialpad milestone) are cheap.

Negative:

- The project owns a legal position and must keep it consistent: every future
  analysis follows this record, and a contributor who cannot accept it does not
  do analysis work.
- Some answers stay at "medium" confidence (byte widths of response fields)
  because the boundary stops short of exhaustive decompilation; the hardware
  re-decode in M0 remains the authority.
- Analysis artefacts live outside the repository, so reproducing a finding
  means re-running the method from the public installer.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Captures only (no static analysis) | Leaves names and semantics to guesswork; the hidden features and the state identifiers would stay unexplained. |
| Unbounded reverse engineering with decompiler output in the research docs | Exceeds Art. 6 (not confined to interoperability needs) and risks tainting the clean-room implementation. |
| Decrypting the shipped device descriptors | Different legal category (protection measure on data); the content is UI registry data, and the device publishes its own geometry. |
| Relying on third-party decompilations | Unknown provenance and licence; the project re-derives protocol knowledge from research and its own captures (CLAUDE.md conventions). |
