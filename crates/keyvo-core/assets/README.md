# Bundled renderer assets

Third-party files that `keyvo-core` embeds with `include_str!` /
`include_bytes!` (spec §9). Each directory keeps the upstream licence text
next to the files.

| Directory | Origin | Version | Licence |
|---|---|---|---|
| `lucide/` | <https://github.com/lucide-icons/lucide>, release asset `lucide-icons-1.61.0.zip` (sha256 `d4cfe5949e0cf202ec558c3fe2cc6c8373371c1cb8643fac2f7c68ce4153ff4b`): every `icons/<name>.svg`, and the `aliases` of `icons/<name>.json` in the same asset. Generated: run `python3 tools/lucide/generate.py`, then `just fmt lint test` | 1.61.0 | ISC (`lucide/LICENSE`) |
| `fonts/Inter-SemiBold.ttf` | <https://github.com/rsms/inter>, `extras/ttf/Inter-SemiBold.ttf` of release asset `Inter-4.1.zip` (sha256 `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e`) | 4.1 | SIL OFL 1.1 (`fonts/LICENSE.txt`) |

The font is unchanged. The lucide SVGs are the whole set, unchanged except
for a one-line header comment naming their origin and licence; the
generator also writes the icon table and the aliases,
`src/render/icon/lucide.rs`. The asset's metadata carries the same
`aliases` as `icons/<name>.json` of the `1.61.0` source archive, so the
release asset is the one pin.

To bump lucide: move `VERSION` and `SHA256` in `tools/lucide/generate.py`
together (and the row above), run `python3 tools/lucide/generate.py`, check
`LICENSE` against the release's, then `just fmt lint test`. The generator
checks the digest before it writes anything and removes the files of icons
the new release dropped.
