# Bundled renderer assets

Third-party files that `keyvo-core` embeds with `include_str!` /
`include_bytes!` (spec §9). Each directory keeps the upstream licence text
next to the files.

| Directory | Origin | Version | Licence |
|---|---|---|---|
| `lucide/` | <https://github.com/lucide-icons/lucide>, release asset `lucide-icons-1.61.0.zip` (sha256 `d4cfe5949e0cf202ec558c3fe2cc6c8373371c1cb8643fac2f7c68ce4153ff4b`) | 1.61.0 | ISC (`lucide/LICENSE`) |

The lucide SVGs are unchanged except for a one-line header comment naming
their origin and licence. To add an icon, copy it from the same release,
add the header, and list it in `src/render/icon.rs`.
