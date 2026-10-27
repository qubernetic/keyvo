# Chord fixtures

Chords the daemon's parser (`keyvo_core::chord`, spec §7.2) accepts and
refuses, shared with the app's chord check (`app/src/lib/chord.ts`), so the
key editor refuses before the write exactly what the daemon would refuse at
its reload. Hand-written.

| File | What it is |
|---|---|
| `chords.json` | `valid`: chords that parse; `invalid`: each with the `error` as `ChordError` names it in snake case (`empty`, `missing_key`, `unknown_key`, `not_a_modifier`, `expected_separator`) and, for the last three, the `token` it names |

Tests:

- `crates/keyvo-core/tests/chord.rs`: `Chord::parse` accepts every valid
  chord and answers each invalid one's error and token.
- `app/src/lib/editor.test.ts`: `checkChord` answers the same; the key
  names of `app/src/lib/keynames.ts` equal the Rust table's, both written
  by `tools/keycodes/generate.py`.
