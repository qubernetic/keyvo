# ADR-0009: Project layer with direnv-style trust

Status: Proposed
Date: 2026-10-24

## Context

keyvo profiles live under `~/.config/keyvo/` and match on the focused window.
That cannot express "this key builds *this* repository": the same editor window
class hosts every project, and a per-project profile selected by `match` would
need a title regex per repository and would still not know the project root.
The UI/UX grill of 2026-10-24 (decisions 5, 6 and 7 in
`docs/plans/2026-10-24-gui-ux-grill-decisions.md`) settled that keyvo needs a
layer that travels with the repository, and that such a layer is the first
input keyvo reads that the user did not write: a checked-out repository can
carry a `.keyvo/` directory from anyone.

The spec's threat model (`SECURITY.md`) trusts everything that runs as the
user. A repository is not that; it is data the user fetched. The decision is
expensive to reverse because it fixes the precedence of configuration sources
(what every profile edit, export and GUI view assumes) and the trust semantics
(what users will rely on when they clone an untrusted repository).

## Decision

1. **A fourth layer in the stack.** Rendering resolves `global < app profile <
   project layer < overlays` (overlays stay on the context page only). The
   project layer is the nearest `.keyvo/` directory found walking up from the
   working directory the context source reports, the same way `git` finds
   `.git`.
2. **Three files, one schema, no `match`.** `.keyvo/profile.toml` applies to
   every app, `.keyvo/profiles/<app-id>.toml` overrides keys for one app
   profile, `.keyvo/local.toml` is personal and a `.gitignore` candidate. The
   files use the profile schema minus `[profile].match`; they override at key
   level and may clear a key explicitly.
3. **Scoped by configuration.** `[project_layer]` in `config.toml` has
   `enabled`, the position in the stack, and allow-lists for keys, action types
   and apps. The layer cannot reach keys or action types the user did not
   allow.
4. **Trust before execution, direnv style.** A project layer is rendered as
   soon as it is found, but no action from it runs until the project is
   trusted. Trust is a record in `~/.config/keyvo/trust.toml` bound to the
   content hash of the three files; any change to them re-asks. `keyvo project
   trust` and the GUI prompt show the actions the layer would run before
   asking. `[project_layer].trust = ask | never | always`; `always` is
   reported by `keyvo doctor` as a warning.
5. **Context comes from the user's tools.** Integrations push the working
   directory with `notify_context` (shell prompt hooks, VS Code, tmux); the
   daemon binds it to the focused window by matching the hook's ancestor pids
   with the window pid. The fallback is the window title matched against a
   registry of trusted and recently seen roots. Anything else is "unknown
   project" with the reason shown.

## Consequences

Positive:

- A team commits `.keyvo/` and every member's pad builds, tests and deploys
  that project with the same keys; personal tweaks stay in `local.toml`.
- Cloning a repository cannot run anything: the pad shows the keys, the first
  press shows the prompt with the commands, and the user decides.
- Recipes (decision 10) get their per-project parameters from the same files,
  so `project.build` means the right thing in every repository.
- Precedence is fixed once; the GUI's layer-origin display, the trace's layer
  resolution and `keyvo profile export` all read the same stack.

Negative:

- Trust bound to a content hash re-asks after every legitimate edit to the
  three files; that is the direnv trade-off and is accepted.
- Context binding by pid is heuristic for windows that are not terminals
  (Dolphin lives on the title fallback); "unknown project" will show more often
  than a user might like, and the reason must be visible.
- One more place a key can come from; without the GUI's origin display and the
  trace, debugging "why does this key do that" gets harder, so those two are
  not optional.
- Windows (M5) has no shell hook path in v1; the project layer is Linux-only
  until then.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Per-repository profiles selected by `match` on the window title | Fragile, per-app, no project root, nothing travels with the repository. |
| Trust on first use (no hash, trust the path) | A pull can change the actions silently; the prompt would lie. |
| Signed project files or an allow-list of authors | Key management for a nine-key pad; the content hash gives the same guarantee for the user who reviewed the files. |
| Run project-layer actions in a sandbox instead of asking | Sandboxing `exec` defeats the purpose (build, deploy, open editor); deferred to v2 for plugins only (decision 17). |
| No project layer; recipes only, parameters in the global config | The global config would grow a per-repository table; nothing shareable with a team. |
