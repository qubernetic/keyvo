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
| `terminal.3.toml` | after step 3: the `exec` becomes a `launch` of the same command |
| `terminal.4.toml` | after step 4: the `launch` gets the window class `wl-paste` |
| `terminal.5.toml` | after step 5: the `launch` does not wait for its window (`wait_focus = false`) |
| `profile.json` | the same for the profile header (`docs/design/gui-ux.md` §3.2): the profile id, the file, its path, the header's name and accent before the first step, and each step: the header field the GUI changes, the name and accent after it, the expected file |
| `terminal.name.toml` | after header step 1: the name `Terminal` becomes `Shell work` |
| `terminal.accent.toml` | after header step 2: the accent `#2ecc71` becomes `#e67e22` |
| `terminal.global.toml` | after header step 3: `global_pages = false` added to `[profile]` |
| `pages.toml` | the profile before the page edits: three pages, the first two with a key, a `[dialpad]` table after them |
| `pages.json` | the page edits (spec §10): the profile id, the file and its path, and each step: the edit as the GUI sends it (`op` `add`, `rename`, `move`, `enable`, `delete`), the expected file, the edited page's number after it and the edit that undoes it (none for a delete, whose undo is the snapshot); `refused` are the edits of the last step's file the editor refuses, with the reason (`context`: the context page cannot be disabled, `only`: the only enabled page cannot be deleted) |
| `pages.1.toml` | after page step 1: a new page without a name at the end, before `[dialpad]` |
| `pages.2.toml` | after page step 2: the new page named `tools` |
| `pages.3.toml` | after page step 3: `tools` moved before `debug` |
| `pages.4.toml` | after page step 4: `edit` switched off (`enabled = false`), its key kept |
| `pages.5.toml` | after page step 5: `tools` deleted |
| `pages.6.toml` | after page step 6: `debug` switched off, which leaves the context page the only enabled one |
| `page-list.json` | the page list the pad walks (spec §6.3) for a global profile and an owner, each given as its page names, disabled pages and `global_pages`; owner `global` is the global profile itself; `list` is every index with the profile and page it shows |
| `recipe.json` | a recipe applied to a key: the profile id, page and key, the file and its path, the bundled recipe (`contrib/recipes/spectacle.toml`) and its variant, the key's fields before (none: key 7 is blank) and after, the expected file and the key's `line` in it |
| `terminal.recipe.toml` | after applying "Spectacle: region" to key 7: a new `[pages.keys.7]` table at the end of the page with the recipe's icon and label and the variant's `exec` |

Tests:

- `crates/keyvo-client/tests/editor.rs`: `Editor::key` answers the
  fixture's `line`; `Editor::set_key` on a configuration directory on disk
  answers each step's `line`, writes each step's file byte for byte,
  takes a snapshot of the file before each write, and undoing the steps
  (each step's fields before, written again) restores the files byte for
  byte.
- `crates/keyvo-client/tests/editor.rs` replays `profile.json` the same way
  with `Editor::set_profile`.
- `crates/keyvo-core/tests/edit_pages.rs`: `ProfileDocument::edit_page`
  gives each page step's file byte for byte, its page and its undo, the
  undo gives the file before, and the `refused` edits change nothing.
- `crates/keyvo-client/tests/editor.rs`: `Editor::edit_page` writes each
  page step's file after a snapshot, and the undo (the inverse edit, for a
  delete `Editor::restore` of the snapshot) restores the file before.
- `crates/keyvo-core/tests/state.rs` and `app/src/lib/profiles.test.ts`:
  `keyvo_core::state::page_list` and the app's `pageList` give every case
  of `page-list.json`.
- `crates/keyvo-client/tests/editor.rs`: `Editor::apply_recipe` with
  `recipe.json` writes `terminal.recipe.toml` byte for byte after a
  snapshot, answers the fields and `line`, and writing `before` back
  clears the key again.
- `app/src/Profiles.test.ts`: the profile header sends each header step's
  name and accent when its field changes, against a fake that holds the
  file these steps give.
- `app/src/KeyEditor.test.ts`: the key editor beside the mirror sends each
  step's fields when its field changes, against a fake editor that holds
  the file these steps give; undo and redo send the fields back.
