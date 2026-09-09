# Architecture Decision Records

Each record captures one decision that shapes keyvo, the context it was made in,
and the alternatives that lost. Records are immutable once accepted; a change of
mind produces a new record that supersedes the old one.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-rust-core.md) | Rust workspace core, TypeScript quick-test first | Accepted |
| [0002](0002-two-layer-dev-model.md) | Two-layer development model | Accepted |
| [0003](0003-no-lua-in-v1.md) | No embedded scripting language in v1 | Accepted |
| [0004](0004-gui-edits-toml-round-trip.md) | GUI edits TOML with round-trip | Accepted |
| [0005](0005-no-crates-io-in-v1.md) | No crates.io publishing in v1 | Accepted |
| [0006](0006-dialpad-own-hidpp.md) | Dialpad via an in-house HID++ 2.0 subset | Accepted |
| [0007](0007-csharp-logi-plugin.md) | C# Logi Actions SDK plugin for Windows | Accepted |

## Adding a record

1. Take the next free number (four digits) and a short kebab-case slug:
   `docs/adr/NNNN-slug.md`.
2. Use the same layout as the existing records:

   ```markdown
   # ADR-NNNN: <title>

   Status: Proposed
   Date: YYYY-MM-DD

   ## Context
   ## Decision
   ## Consequences
   ## Alternatives considered
   ```

3. Status values: `Proposed` (under discussion in an issue or PR), `Accepted`
   (approved by the project owner), `Superseded by ADR-NNNN` (kept for history).
4. Add a row to the table above in the same PR.
5. A decision that is only a default value (a timing, a path) belongs in
   `docs/spec.md`, not here. A record is for choices that are expensive to reverse.
