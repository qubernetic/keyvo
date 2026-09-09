# ADR-0001: Rust workspace core, TypeScript quick-test first

Status: Accepted
Date: 2026-09-09

## Context

keyvo is a long-running user daemon that talks to USB HID devices, renders
images at interactive rates, injects input, and exposes a local socket. It must
run on Linux first and on Windows later (see ADR-0007), be packaged as a single
binary, and reach a high test coverage without hardware in CI.

The community work that mapped the MX Creative Console protocol exists in
several languages: TypeScript (Julusian's `node-logitech-mx-creative-console`),
C++ (logilinux), Python (logimap), C (shensquared's probes), and Rust
(notno's `creative-console-daemon` for Windows). None of it is a Linux daemon
with context awareness, so keyvo re-derives the protocol rather than depending
on any of them (`docs/research/logilinux-and-julusian-analysis.md` §9).

Julusian's library is the most complete public implementation of the Keypad's
LCD path and runs unmodified on Linux with Node, which is installed on the
host. Rust is not installed on the host (ADR-0002).

## Decision

- The product is a Cargo workspace with four crates: `keyvo-hid` (transport and
  protocol), `keyvo-core` (profiles, state machine, renderer, actions; sync and
  hardware-free), `keyvo-daemon` (tokio runtime, watchers, injector, socket,
  plugin supervisor), `keyvo-cli` (`keyvo` binary).
- Before porting anything to Rust, milestone M0 starts with a throwaway
  TypeScript quick-test (issue M0-1) that drives the real Keypad through
  Julusian's library on the host. Its only purpose is to answer the measurement
  list in the project plan §5 and produce golden byte fixtures.
- The GUI (M4a) is Tauri 2 with a Svelte 5 frontend, reusing the Rust core.

## Consequences

Positive:

- One statically linked binary per platform, no runtime to install; cargo-dist
  can produce deb, rpm, AppImage and Windows artefacts.
- `keyvo-core` stays free of I/O, so the state machine and renderer are unit
  testable with snapshot tests and can be reused by the GUI in-process.
- The hardware ground truth is measured with a tool that already works, before
  any Rust protocol code exists, so the port is checked against numbers rather
  than against someone else's constants.
- Tauri lets the GUI call the same Rust code paths instead of a second
  implementation.

Negative:

- Two toolchains (Rust and Node) in the devcontainer from day one.
- The quick-test code is disposable; nothing from it ships.
- Rust's compile times and the hidapi/evdev/zbus dependency surface are heavier
  than a Python prototype would have been.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Python daemon (logimap model) | Packaging a long-running daemon with native HID and rendering dependencies for Flatpak and Windows is fragile; coverage and typing discipline are harder to enforce at the quality bar of `copia-cli`. |
| TypeScript/Node daemon on Julusian's library | Would make keyvo a thin wrapper over a library it does not control; Node cannot render dynamic key images on the Windows side (ADR-0007); a Node runtime is an awkward system service. |
| Go | No mature HID, D-Bus and uinput story comparable to hidapi/zbus/evdev; no path to the Tauri GUI. |
| Port to Rust immediately, skip the quick-test | Every downstream of the public libraries reports revert-to-logo and pacing problems that the libraries themselves do not explain; measuring first avoids porting the wrong constants. |
