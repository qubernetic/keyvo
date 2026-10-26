# Key editor fixtures

The steps of one key edit, shared by the Rust editor and the GUI tests, so
the request the GUI sends and the file the Rust side writes for it are
checked against the same bytes. Every file is hand-written.

| File | What it is |
|---|---|
| `terminal.toml` | the profile before the edits: comments, aligned `=`, an inline action |
| `steps.json` | the profile id, page and key edited, the file's path inside the configuration directory, the key's fields and `line` before the first step, and each step: the form field the GUI changes, the key's fields after it, the expected file, and the key's `line` in it; `line` is the line, from 1, of the key's `[pages.keys.2]` header, as `KeyView` and `Edited` answer it |
| `terminal.1.toml` | after step 1: key 2's label `Copy` becomes `Paste` |
| `terminal.2.toml` | after step 2: key 2's chord becomes an `exec` of `wl-paste` |
| `profile.json` | the same for the profile header (`docs/design/gui-ux.md` §3.2): the profile id, the file, its path, the header's name and accent before the first step, and each step: the header field the GUI changes, the name and accent after it, the expected file |
| `terminal.name.toml` | after header step 1: the name `Terminal` becomes `Shell work` |
| `terminal.accent.toml` | after header step 2: the accent `#2ecc71` becomes `#e67e22` |

Tests:

- `crates/keyvo-client/tests/editor.rs`: `Editor::key` answers the
  fixture's `line`; `Editor::set_key` on a configuration directory on disk
  answers each step's `line`, writes each step's file byte for byte,
  takes a snapshot of the file before each write, and undoing the steps
  (each step's fields before, written again) restores the files byte for
  byte.
- `crates/keyvo-client/tests/editor.rs` replays `profile.json` the same way
  with `Editor::set_profile`.
- `app/src/Profiles.test.ts`: the profile header sends each header step's
  name and accent when its field changes, against a fake that holds the
  file these steps give.
- `app/src/KeyEditor.test.ts`: the key editor beside the mirror sends each
  step's fields when its field changes, against a fake editor that holds
  the file these steps give; undo and redo send the fields back.
