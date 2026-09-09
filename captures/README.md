# Captures

Protocol captures and the fixtures derived from them. keyvo re-derives the MX
Creative Console protocol from captured traffic and from measurements on real
hardware; this directory is the evidence trail.

## Layout

| Path | Tracked | Content |
|---|---|---|
| `external/hcooper/` | README, license, `derived/` | Hunter Cooper's USBPcap captures of Logi Options+ driving the Keypad on Windows (MIT, see `LICENSE.hcooper`). The README (punctuation normalised) and the derived fixtures come from the upstream repository. |
| `external/hcooper/*.pcap` | no | The raw captures. Fetch them from <https://github.com/hcooper-idealbuilders/mx-creative-controller> (`captures/`). |
| `local/` | no | Captures taken on this project's own Windows VM (M0-2). Published as release assets (`captures-YYYY-MM`), never committed. |
| `fixtures/` | yes | Golden frames and NDJSON fixtures extracted from captures, shared by the Rust, TypeScript, and C# test suites (lands with M0-2). |

Decoded walkthroughs of the external captures live in
[`docs/research/hcooper-captures.md`](../docs/research/hcooper-captures.md) and
[`docs/research/hcooper-startup-keypad-sequence.txt`](../docs/research/hcooper-startup-keypad-sequence.txt).

## Rules

- Raw `.pcap`/`.pcapng` files never enter git (see `.gitignore`); attach them to a
  GitHub release instead and reference the asset from the research report.
- Every derived fixture names its source capture and frame numbers so it can be
  regenerated.
- Third-party material keeps its original license file next to it.
