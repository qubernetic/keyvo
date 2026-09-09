<div align="center">

# keyvo

**Nine tiles that follow your context.**

[![CI](https://github.com/qubernetic/keyvo/actions/workflows/ci.yml/badge.svg)](https://github.com/qubernetic/keyvo/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-000000.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/github/v/release/qubernetic/keyvo?include_prereleases)](https://github.com/qubernetic/keyvo/releases)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![codecov](https://codecov.io/gh/qubernetic/keyvo/graph/badge.svg)](https://codecov.io/gh/qubernetic/keyvo)

</div>

keyvo turns the Logitech MX Creative Keypad (and later the Dialpad) into a
context-aware, scriptable control surface for developers on Linux. Profiles switch
with the focused window, the nine LCD keys show the actions that matter right now,
and everything is plain TOML plus a local socket, so scripts, editors, and coding
agents can drive it.

Logitech ships no Linux support for the MX Creative Console. Community projects
have mapped the wire protocol; keyvo builds on that work to deliver the missing
piece: a daemon, a CLI, and a small desktop app that make the hardware useful on a
developer's desk.

> **Status: pre-alpha.** The project is in its bootstrap phase. Nothing is
> installable yet. Follow the [roadmap](docs/roadmap.md) and the
> [milestones](https://github.com/qubernetic/keyvo/milestones) for progress.

## What it will do

- **Context profiles** — the active application (KDE Wayland first, X11 fallback)
  selects a page; overlays layer on top for transient state such as a running
  build or a waiting coding agent.
- **Nine LCD keys** — icons and labels rendered locally, updated live; page buttons
  for manual navigation.
- **Actions** — key chords via `uinput`, shell commands, HTTP calls, text, page and
  profile switches; short, long, and repeat presses.
- **Scriptable** — an NDJSON socket API (`keyvo ctl`) and supervised plugin
  processes; first-party integrations for Claude Code, VS Code, tmux, and git.
- **Text first** — profiles live in `~/.config/keyvo/` as TOML with a JSON Schema;
  the GUI edits the same files without destroying comments.
- **Dialpad** — buttons, dial, and roller over the Logi Bolt receiver via a small
  in-house HID++ 2.0 subset.
- **Windows bridge** — a Logi Actions SDK plugin so the same profiles work next to
  Logi Options+.

## Hardware

| Device | Connection | Support |
|---|---|---|
| MX Creative Keypad (`046d:c354`) | USB-C | primary target |
| MX Creative Dialpad | Logi Bolt / Bluetooth | planned (M3) |
| MX Master 4, MX Mechanical | Logi Bolt | desk-state settings via Solaar (optional) |
| MX Keypad (2026) | USB-C | unverified; geometry and features are queried at runtime |

## Documentation

- [Specification](docs/spec.md) — what keyvo is and why.
- [Project plan](docs/plans/2026-09-09-project-plan.md) — decisions, architecture,
  milestones.
- [Research](docs/research/) — protocol findings and prior-art analyses.

## Contributing

keyvo follows an issue-driven Gitflow with test-driven development in a
devcontainer and hardware tests on the host. See [CONTRIBUTING.md](CONTRIBUTING.md)
once it lands with the project foundation; until then, open an issue to discuss.

## Prior art

keyvo would not exist without the people who reverse-engineered the MX Creative
Console. No code is copied from these projects; keyvo re-derives the protocol from
their findings and from its own captures, and checks its packet layouts against
them.

| Project | Contribution |
|---|---|
| [node-logitech-mx-creative-console](https://github.com/Julusian/node-logitech-mx-creative-console) (Julian Waller / Bitfocus, MIT) | The earliest public implementation (Oct 2024) of the Keypad's LCD frame format, page-button divert writes and brightness command, in TypeScript for Node and WebHID. |
| [mx-creative-controller](https://github.com/hcooper-idealbuilders/mx-creative-controller) (Hunter Cooper, MIT) | Windows controller plus the only public USBPcap captures and journal of the reverse-engineering; its captured Options+ frames serve as golden data. |
| [mx-creative-console](https://github.com/shensquared/mx-creative-console) (Shen Shen, MIT) | macOS C probes that verified the HID++ feature table, the keep-alive requirement, page-button diverts, brightness, and the Dialpad's MultiRoller feature over Bluetooth. |
| [logilinux](https://github.com/logilinux/logilinux) (LauzHack 2025, MIT) and [logilinux-sdk](https://github.com/logilinux/logilinux-sdk) | C++ library and CLI tools for the Keypad and Dialpad on Linux: hidraw/evdev discovery, JPEG upload, button-report parsing, Dialpad rotation via evdev. |
| [logimap](https://github.com/abishekmuthian/logimap) (Abishek Muthian, MIT) | Python daemon with a KWin focus script and a Tk GUI on top of logilinux; the first application-aware mapper for KDE. |
| [creative-console-daemon](https://github.com/notno/creative-console-daemon) (Nathan Rosquist) | Rust/hidapi daemon for Windows whose packetizer unit tests provide independent expected values. |
| [companion-surface-logitech-mx-creative-console](https://github.com/bitfocus/companion-surface-logitech-mx-creative-console) (Bitfocus, MIT) | Bitfocus Companion surface module; reference for draw throttling and retry behaviour. |
| [mx-creative-console-webhid](https://github.com/mario-gutierrez/mx-creative-console-webhid) (Mario Gutierrez) | Browser WebHID client with runtime HID++ feature discovery. |
| [mx-console-suite](https://github.com/CatalystMonish/mx-console-suite) (macOS) | Keep-alive behaviour and launcher pattern. |
| [Solaar](https://github.com/pwr-Solaar/Solaar) (GPL-2.0) | HID++ engine for mice and keyboards; keyvo calls it, it does not copy it. |

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for copyright and attribution.

Logitech, Logi, MX, MX Creative Console, MX Creative Keypad, MX Creative Dialpad,
Logi Bolt and Logi Options+ are trademarks or registered trademarks of Logitech
International S.A. keyvo is an independent community project and is not
affiliated with, endorsed by, or supported by Logitech.
