# keyvo-core fixtures

Configuration files for the loading tests (spec §7.1 and §7.2). Every file is
hand-written; none is derived from a capture.

| Path | Test | What it shows |
|---|---|---|
| `profiles/valid/vscode.toml` | `tests/profile.rs` | every field of schema version 1 |
| `profiles/invalid/*.toml` | `tests/profile.rs` | one problem each; the `INVALID` table lists the expected line, column and message, and a test fails when a file is missing from it |
| `config/valid/partial.toml` | `tests/config.rs` | keys left out keep their default |
| `config/invalid/*.toml` | `tests/config.rs` | an unknown `[timing]` key, a negative duration |
| `dirs/full/` | `tests/layout.rs` | `config.toml`, a `global.toml`, an app profile and a non-TOML file that is ignored |
| `dirs/broken/` | `tests/layout.rs` | an invalid `config.toml`, an invalid profile, an id that does not match its file name, and one valid profile; the load reports all three problems |

The committed JSON Schemas these files follow are in `schemas/` at the
repository root.
