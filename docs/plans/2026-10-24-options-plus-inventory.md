# Plan: Logi Options+ device-communication inventory (Issue #27)

> Documentation plan for [#27](https://github.com/qubernetic/keyvo/issues/27). Governing
> decisions: ADR-0006 (own HID++ subset), ADR-0007 (C# plugin on Windows), the new
> ADR-0008 (static-analysis boundary), project plan §1 row 6 (hardware verification).
> Status: **approved in chat on 2026-10-24** (content and the "no decryption" decision).

## 1. Why

The open points in `docs/research/hcooper-captures.md` and `hidpp-features.md` (VLP root
fn1/fn2, display fn0/fn7 fields, the `a0..a6` and `d0..d6` identifiers, the hidden
`0x92E2`, the unknown feature names) are names and semantics, which captures cannot give.
A time-boxed static-analysis spike on the public macOS offline installer of Options+
answered most of them and produced one finding that changes the plan: the Creative
Console is driven not by the Options+ agent but by the bundled Logi Plugin Service and
its native helper. The spike ran outside the repository; this issue lands its results.

## 2. Outcome

A reader of `docs/research/` can see what Options+ contains about the Keypad and the
Dialpad, in what form, with evidence and confidence per claim, and the project has a
written boundary for any future static analysis of vendor software.

## 3. Files

| File | Change |
|---|---|
| `docs/research/options-plus-inventory.md` | New. The spike report, protocol facts only: component map, where the device knowledge lives, 0x19A1 function table and field names, device state names, image record layout, VLP root functions, unknown feature names, data-file schemas, package format, cost of a deeper pass, legal-boundary log. |
| `docs/research/hcooper-captures.md` | Resolve steps 2, 4, 6, 7, 9 and quick-test item 1 with cross-references to the inventory. |
| `docs/research/hidpp-features.md` | Fill in the names of 0x1602, 0x1807, 0x1E02; mark 0x92E2, 0x1EB0, 0x180B as unused by Options+. |
| `docs/adr/0008-static-analysis-boundary.md`, `docs/adr/README.md` | New ADR (Proposed, Accepted on merge): interoperability only, confined to the device-communication parts, facts only, never decompiled code or pseudocode, no circumvention of protection measures on data, findings documented with evidence kind and confidence. |
| `CONTRIBUTING.md` | New section "Static analysis of vendor software" pointing to the ADR and describing the isolated tooling (distrobox, Ghidra, REA) as optional research tooling, not a build dependency. |
| `docs/spec.md` | §14 bullet on static analysis; sources table row for Options+ and the Plugin Service. |
| `docs/roadmap.md` | M0-2: the Windows capture targets the Plugin Service traffic; M3: targeted 0x4610 static pass before the Bolt capture; M5: wording (Options+ and its Plugin Service own the devices). |

No code, no fixtures, no captures. The depot container format is described in the report
so that the unpacker can be rewritten; the spike's own script stays outside the repo.

## 4. Legal boundary applied

EU Directive 2009/24/EC Art. 5(3) (observation while running: the captures) and Art. 6
(decompilation for interoperability: this spike). The report contains names, IDs,
constants, layouts, enumerations and data-file schemas; no decompiled code, no pseudocode,
no function bodies. Licensing, telemetry, updater, crash reporting, cloud and UI code were
skipped. The encrypted device descriptors and depots were inventoried but not decrypted,
by owner decision: that would be circumvention of a protection measure on data, not
decompilation for interoperability, and the geometry comes from the device at runtime.

## 5. Order of work (atomic commits)

1. `docs(plans): add the Options+ inventory plan (#27)`
2. `docs(research): add the Logi Options+ device-communication inventory`
3. `docs(research): resolve capture open points from the Options+ inventory`
4. `docs(adr): add ADR-0008 static-analysis boundary`
5. `docs: reference the Options+ inventory from contributing, spec and roadmap`

## 6. Acceptance criteria

- [ ] Every claim in the inventory names its evidence kind (string, symbol, decompiled
      request builder, data file, capture cross-check) and a confidence level.
- [ ] No code block, no pseudocode and no em-dash in any changed file (`grep` before commit).
- [ ] The hcooper open points that the inventory answers link to the inventory section.
- [ ] ADR-0008 is listed in `docs/adr/README.md` as Proposed; the owner flips it to
      Accepted on merge.
- [ ] `just fmt-check lint test deny` unaffected (docs only); the PR template's User test
      section is removed because no device behaviour changes.

## 7. User test

None: documentation only.
