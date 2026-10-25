# keyvo-core fixtures

Configuration files for the loading tests (spec §7.1 and §7.2) and trace
records (spec §8). Every file is hand-written; none is derived from a
capture.

| Path | Test | What it shows |
|---|---|---|
| `profiles/valid/vscode.toml` | `tests/profile.rs` | every field of schema version 1 |
| `profiles/valid/demo.toml` | `tests/render.rs`, `keyvo-cli` preview tests | the renderer demo: every primitive on page 1 (icon and label, key colors light and dark, icon only, label only, a cut label, a blank key, an unknown icon), a second page; its page 1 is the snapshot `tests/snapshots/render__demo-page-1.snap.png` |
| `profiles/invalid/*.toml` | `tests/profile.rs` | one problem each; the `INVALID` table lists the expected line, column and message, and a test fails when a file is missing from it |
| `edit/before.toml`, `edit/<case>.json`, `edit/<case>.toml` | `tests/edit.rs` | round-trip editing: a profile in every style the editor must keep (comments, aligned `=`, inline and sub-table actions, a second page repeating key numbers); each case applies `<case>.json` (page, key, fields) to `before.toml` and must give `<case>.toml` byte for byte, changing only lines of the edited key; the `CASES` table lists them and a test fails when a file is missing from it |
| `trace/records.ndjson` | `tests/press.rs` | one serialized trace record per line, resolved from presses on `demo.toml`: a sent chord, an exec that exited 101, an exec that failed to start, a simulated long press, an unbound key |
| `config/valid/partial.toml` | `tests/config.rs` | keys left out keep their default |
| `config/invalid/*.toml` | `tests/config.rs` | an unknown `[timing]` key, a negative duration |
| `dirs/full/` | `tests/layout.rs` | `config.toml`, a `global.toml`, an app profile and a non-TOML file that is ignored |
| `dirs/focus/` | `tests/state.rs` | profile selection and the page state machine: a global profile with a `context` and a `general` page, app profiles on a reverse-DNS class, a short class with a title rule and a higher priority, a title rule alone, and a profile whose `context` page is not its first |
| `dirs/fallback/` | `tests/stack.rs`, `keyvo-client` `tests/tiles.rs` | the global layer (spec §6.1): a `konsole` app profile without keys 3 and 9 over a global profile whose `context` page is its second, so a fallback to page 1 would show the wrong key |
| `dirs/broken/` | `tests/layout.rs` | an invalid `config.toml`, an invalid profile, an id that does not match its file name, and one valid profile; the load reports all three problems |

The committed JSON Schemas these files follow are in `schemas/` at the
repository root.
