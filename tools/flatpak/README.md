# tools/flatpak

`generate.py` writes the offline sources of the Flatpak manifest
(`contrib/flatpak/io.github.qubernetic.keyvo.yml`):

| File | From | Generator |
|---|---|---|
| `contrib/flatpak/cargo-sources.json` | `Cargo.lock` (the `keyvo` CLI) and `app/src-tauri/Cargo.lock` (the app), shared crates once | `cargo/flatpak-cargo-generator.py` |
| `contrib/flatpak/node-sources.json` | `app/package-lock.json` | `node/flatpak_node_generator` |

The generators come from
[flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools)
(MIT) at the commit `TOOLS_COMMIT` in `generate.py`, fetched into a
temporary directory on every run; nothing of theirs is committed here. They
need python3 with aiohttp, tomlkit and PyYAML, which the dev image carries.

```bash
just flatpak-sources        # regenerate both files
just flatpak-sources-check  # regenerate into a temporary directory, fail on a diff
```

Run `just flatpak-sources` after every change of one of the three lockfiles
and commit the result with it; the CI `app` job runs the check. Two runs on
the same lockfiles give the same files.

To bump the generators, move `TOOLS_COMMIT`, regenerate, review the diff
and let the CI `flatpak` job build the bundle (CONTRIBUTING.md, "Bumping a
pinned version").
