# Key editor fixtures

The steps of one key edit, shared by the Rust editor and the GUI tests, so
the request the GUI sends and the file the Rust side writes for it are
checked against the same bytes. Every file is hand-written.

| File | What it is |
|---|---|
| `terminal.toml` | the profile before the edits: comments, aligned `=`, an inline action |
| `steps.json` | the profile id, page and key edited, the file's path inside the configuration directory, the key's fields before the first step, and each step: the form field the GUI changes, the key's fields after it, the expected file |
| `terminal.1.toml` | after step 1: key 2's label `Copy` becomes `Paste` |
| `terminal.2.toml` | after step 2: key 2's chord becomes an `exec` of `wl-paste` |

Tests:

- `crates/keyvo-client/tests/editor.rs`: `Editor::set_key` on a
  configuration directory on disk writes each step's file byte for byte,
  takes a snapshot of the file before each write, and undoing the steps
  (each step's fields before, written again) restores the files byte for
  byte.
- `app/src/KeyEditor.test.ts`: the key editor beside the mirror sends each
  step's fields when its field changes, against a fake editor that holds
  the file these steps give; undo and redo send the fields back.
