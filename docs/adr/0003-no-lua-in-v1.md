# ADR-0003: No embedded scripting language in v1

Status: Accepted
Date: 2026-09-09

## Context

Draft spec v0.1 left "embedded logic" open: an optional Lua runtime (`mlua`)
for stateful keys, conditional icons and timers without spawning processes.
The audience decision (developers only, project plan §1 row 9) means users are
comfortable with scripts in their own language and with a local API. The main
first-party integration, the Claude Code plugin, is driven by hooks that already
call out to the shell.

Adding a language means adding its API surface, its sandboxing story, its
documentation and its GUI editing story, all of which compete with the M0 to M2
core for attention.

## Decision

- v1 ships no embedded scripting language.
- Inward control is a newline-delimited JSON socket at
  `$XDG_RUNTIME_DIR/keyvo/keyvo.sock` (mode 0600), every message carrying
  `"v": 1`; `keyvo ctl` is the command-line client for it.
- Stateful behaviour lives in **plugins**: executables under
  `~/.config/keyvo/plugins/<name>/` described by a `plugin.toml`, started and
  restarted by the daemon, speaking the same NDJSON protocol on stdin/stdout.
- No D-Bus façade in v1. The daemon *consumes* D-Bus (KWin, session bus) but does
  not *export* an object; the socket is the single control surface.

## Consequences

Positive:

- One protocol to document, test (shared NDJSON fixtures under
  `tests/protocol-fixtures/`) and version.
- Plugins can be written in any language and crash without taking the daemon
  down.
- The GUI has nothing to embed; it edits TOML and launches an editor for scripts
  (ADR-0004).

Negative:

- Per-key logic that would be one line of Lua needs a process, a socket round
  trip and JSON.
- Latency-sensitive behaviour (timers, animations) must be implemented in the
  daemon as features, not by users.
- Tools that only speak D-Bus (KDE shortcuts, some launchers) need a `keyvo ctl`
  wrapper.

The decision is explicitly for v1; a scripting layer can be revisited once the
socket API has stabilised and real plugin pain points are known.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Lua via `mlua` | Second API surface to design and keep stable; sandboxing and GUI editing unresolved; not needed by the developer audience. |
| Rhai or another Rust-native scripting language | Same cost as Lua with a smaller ecosystem. |
| D-Bus as the primary control API | Linux-only, harder from shell hooks than a socket, and would duplicate the protocol needed for Windows named pipes later. |
| D-Bus façade in addition to the socket | Two surfaces to keep in sync; deferred until a concrete consumer asks for it. |
