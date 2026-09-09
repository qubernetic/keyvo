# ADR-0006: Dialpad via an in-house HID++ 2.0 subset

Status: Accepted
Date: 2026-09-09

## Context

The MX Creative Dialpad connects through the Logi Bolt receiver (`046d:c548`)
or Bluetooth and speaks HID++ 2.0. The author's feature dump
(`docs/research/hidpp-features.md`) shows its four buttons as divertable
controls on `0x1B04 REPROG_CONTROLS_V4` (CIDs `0x0053`, `0x0056`, `0x0059`,
`0x005A`) and the dial and roller on `0x4610 MultiRoller` (roller 0 is the
small wheel at 40 detents per revolution, roller 1 the dial at 180; deltas are
signed 8-bit). The Dialpad has no keep-alive feature. Feature indices differ
from the Keypad (`0x1B04` is index `0x0a` on the Dialpad, `0x0b` on the
Keypad), so indices must be resolved through `ROOT.getFeature` at runtime.

Solaar, the established Linux HID++ tool, declined to add Dialpad support
(pwr-Solaar/Solaar issue #3100), and `solaar show` crashes on the device in
1.1.20. Nobody has published a working Dialpad path over the Bolt receiver;
the public probes (shensquared) used Bluetooth.

The Keypad already requires an HID++ 2.0 core in `keyvo-hid` for keep-alive,
brightness and page-button divert, so most of the machinery exists regardless.

## Decision

- `keyvo-hid` implements the HID++ 2.0 subset keyvo needs itself: report
  framing (`0x10`/`0x11`), `ROOT` and `FEATURE_SET` walks, feature index
  resolution, software ID handling, `0x0008`, `0x1B04`, `0x8040` and `0x4610`.
- The Dialpad is driven over the Bolt receiver's hidraw node with the device
  index filter; Bluetooth is the fallback path. The Bolt path is measured in
  M0-2 (Windows VM capture) and implemented in M3.
- Solaar is **optional** and used only to apply mouse and keyboard "desk state"
  settings (DPI, backlight, button remaps) when a profile activates, by calling
  its CLI. keyvo never copies Solaar code (GPL-2.0) and never asks Solaar to
  divert anything on the Creative Console devices.
- No haptics in v1.

## Consequences

Positive:

- No dependency on Solaar's release cycle or on a maintainer decision for the
  core devices.
- One transport and one protocol layer for Keypad and Dialpad, testable with
  `FakeHidTransport` and golden frames.
- The subset is small and documented in `docs/protocol.md` (M0-9).

Negative:

- keyvo owns HID++ edge cases: reply matching by report ID, feature index and
  software ID; filtering the receiver's mouse-report flood; coexistence with the
  kernel `hid-logitech-dj` driver and with a running Solaar on the same node.
- The Bolt path is unproven anywhere; the M3 milestone carries that risk with
  Bluetooth as the fallback.
- Haptics on the MX Master 4 stay out of scope even though the hardware
  supports them.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Extend Solaar upstream | Maintainer declined (#3100); Solaar's Python and GPL-2.0 license do not fit a Rust daemon. |
| Use the Dialpad's evdev nodes only (logilinux model) | Works for rotation but cannot divert buttons or read raw deltas; native scrolling keeps happening. |
| Depend on Solaar for everything HID++ | Adds a mandatory Python dependency for the core path and gives no Dialpad support anyway. |
| Haptics in v1 | Unmeasured HID Haptics usage page; deferred until the core is stable. |
