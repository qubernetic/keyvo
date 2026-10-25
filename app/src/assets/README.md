# Bundled app assets

Third-party files the app imports (`src/lib/icons.ts`). Each directory keeps
the upstream licence text next to the files.

| Directory | Origin | Version | Licence |
|---|---|---|---|
| `lucide/` | <https://github.com/lucide-icons/lucide>, `icons/` of the npm package `lucide-static-1.61.0.tgz` (sha256 `62773cd2ee411ad2e37c1973d1e4c4b49f9a6c57da6b5d5d0abca08756ba75f1`) | 1.61.0 | ISC (`lucide/LICENSE`) |

The SVGs are unchanged except for the first line, a header comment naming
their origin and licence (it replaces the package's own `@license`
comment). The version is the one `keyvo-core` renders on the pad
(`crates/keyvo-core/assets/`). To add an icon, copy it from the same
release, add the header, and list it in `src/lib/icons.ts`.
