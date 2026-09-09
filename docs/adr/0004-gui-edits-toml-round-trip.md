# ADR-0004: GUI edits TOML with round-trip

Status: Accepted
Date: 2026-09-09

## Context

keyvo's audience is developers (project plan §1 row 9). They keep their
configuration in version control, comment it, and expect a text file to be the
source of truth. At the same time a grid of nine icons with colours is easier
to arrange visually than by hand, and a junior developer joining a team should
be able to start from a shared profile without reading a schema first.

Profiles are TOML files under `~/.config/keyvo/profiles/<id>.toml`; `exec`
actions reference scripts that users write themselves.

## Decision

- TOML is the only persisted format. The GUI (M4a) reads and writes the same
  files as the daemon; there is no GUI-private database.
- Writes go through `toml_edit` so that comments, key order and formatting the
  user wrote survive a GUI edit. **Round-trip fidelity is an M1 invariant**,
  enforced by tests before any GUI exists.
- A JSON Schema is generated from the Rust types with `schemars`, published with
  releases, and used for editor completion and for GUI validation.
- GUI scope for v1: grid layout editor, icon picker, simple actions (chord,
  page, profile, mode), a dashboard of device and daemon state, and a first-run
  flow. Scripts for `exec` actions are opened in the user's external editor
  (`$VISUAL`, then `$EDITOR`, then `xdg-open`, configurable); the GUI never
  embeds a code editor.
- Frontend stack: Tauri 2 with Svelte 5.

## Consequences

Positive:

- Text remains authoritative; diffs, reviews and dotfile repos work.
- A profile edited in the GUI and one edited in vim are indistinguishable.
- The daemon and the GUI validate against one schema.

Negative:

- `toml_edit` is more work than a serde round trip and must be kept in step with
  the typed model.
- Some layouts (deeply nested tables, inline tables) constrain what the GUI can
  express without reformatting.
- No in-app scripting editor means the GUI cannot be the only tool a user ever
  touches; that is intended for this audience.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| GUI owns a database, exports TOML | Two sources of truth; hand edits get overwritten. |
| serde serialise on save | Destroys comments and ordering; unacceptable for versioned config. |
| Embedded code editor in the GUI | Large scope for little gain over the user's own editor. |
| YAML or JSON profiles | TOML is the Rust ecosystem default and reads better for a nine-key grid. |
