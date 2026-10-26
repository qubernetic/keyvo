# Bundled app assets

Third-party files the app imports (`src/lib/icons.ts`). Each directory keeps
the upstream licence text next to the files.

| Directory | Origin | Version | Licence |
|---|---|---|---|
| `device/` | The owner's stylised drawings of the MX Creative Keypad (`keypad.svg`, 512 x 610) and Dialpad (`dialpad.svg`, 512 x 512), drawn on 2026-10-26 with ChatGPT assistance from the devices' proportions, not traced from a photograph, no vendor mark | 2026-10-26 | Apache-2.0 (the repository's `LICENSE`) |
| `lucide/` | <https://github.com/lucide-icons/lucide>, `icons/` of the npm package `lucide-static-1.61.0.tgz` (sha256 `62773cd2ee411ad2e37c1973d1e4c4b49f9a6c57da6b5d5d0abca08756ba75f1`) | 1.61.0 | ISC (`lucide/LICENSE`) |

The SVGs are unchanged except for the first line, a header comment naming
their origin and licence (it replaces the package's own `@license`
comment). The version is the one `keyvo-core` renders on the pad
(`crates/keyvo-core/assets/`). To add an icon, copy it from the same
release, add the header, and list it in `src/lib/icons.ts`.

The device drawings are the owner's originals with a cleanup for the app:
ids on the parts (`key-1` to `key-9`, `key-5-notch` (the tactile bar of the home key), `page-prev`, `page-next`; `button-1`
to `button-4`, `roller`, `led`, `dial`), prefixed gradient ids so both can
be inlined on one page, two stray paths and three empty groups removed,
the two page buttons made the same size, and every colour as a CSS custom
property with the original value as the fallback (`--kv-device-light`,
`-shade`, `-button`, `-metal`, `-well`, `-rim`, `-edge`, `-glyph`; the LED
is `--kv-state-success`). The component of #118 derives these from the
`--kv-device` token of `docs/design/gui-ux.md` section 7 and puts the
mirror tiles into the `key-N` cutouts.
