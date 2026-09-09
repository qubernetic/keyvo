# Decoding hcooper's public USBPcap captures (2026-09-09)

Source: https://github.com/hcooper-idealbuilders/mx-creative-controller (Hunter Cooper, MIT),
`captures/02-binding-paint.pcap` and `captures/03-options-plus-startup.pcap`, Windows,
Logi Options+, MX Creative Keypad over USB. Local copies (not tracked):
`captures/external/hcooper/`. Decoded with a small pure-Python USBPcap reader
(linktype 249); full Keypad-only sequence in `hcooper-startup-keypad-sequence.txt`.
Everything below is my own decode and must be verified on Linux hardware in M0.
Byte 3 of every HID++ packet is `(function << 4) | swid`; Options+ uses swid `0xd`/`0xe`.

## Two protocol planes on the same device

| Report ID | Size | Role | Feature index space |
|---|---|---|---|
| `0x11` | 20 B | classic HID++ 2.0 long reports | the 18-feature table in `hidpp-features.md` (index 4 = KEEP_ALIVE, 0x0b = REPROG_CONTROLS_V4, 0x0f = BRIGHTNESS_CONTROL) |
| `0x13` | 32 B | "very long packet" (VLP) control channel, **own root and own feature index space** | VLP index 0 = VLP root, VLP index 2 = display feature **0x19A1** |
| `0x14` | 4095 B | VLP bulk channel, LCD image fragments | same VLP index 2 |

VLP packets carry a sequence/flag byte after the function byte: host sends `e0`, device answers
`c0`, `c1`, … (per fragment), unsolicited events use `10` as function byte and `c6`-style
sequence. Evidence: `13 ff 00 0e e0 19 a1` → `c0 19 a1 02 …` (VLP root getFeature(0x19A1) = index 2).

## Startup sequence Options+ performs on the Keypad (03-options-plus-startup, dev 15)

1. Classic HID++: root ping (`11 ff 00 1d … 39` → `04 05 39`, HID++ 4.5), DEVICE_FW_VERSION
   entity walk (index 2), DFUCONTROL (index 6) query, repeated several times.
2. `13 ff 00 1e e0` → `c0 01 00 00 01 e8 20`: VLP root, fn 1 (protocol/version info, unclear).
3. `11 ff 04 0e` → `01 f4 27 10`: **KEEP_ALIVE fn0 = timeout range 500 ms .. 10000 ms**.
4. `13 ff 00 2e e0 5e 5f 60 61 62 63 64 65` → echoed back: VLP root fn 2 with the eight values
   `0x5e..0x65` (notno's "key CIDs"). Meaning unclear; possibly a registration of software IDs
   or areas. Must test whether it is required.
5. `13 ff 00 0e e0 19 a1` → `c0 19 a1 02 00 00 00 01 e8 03`: VLP root fn0 getFeature(0x19A1)
   → VLP index 2.
6. `13 ff 02 7e e0` → `c0 00 01`: display fn7, unclear (status?). Repeated.
7. `13 ff 02 3e e0` → `c0 a0 a1 a2 a4 a5 a6 d0 d1 d2 d3 d4 d5 d6`: display fn3, list of 13
   IDs. `a0`/`a1` reappear below as **display modes**; `d0..d6` unclear.
8. `13 ff 02 5e e0` → `c0 a1`: display fn5 = **get current mode; `a1` at boot**.
9. `13 ff 02 0e e0` → `c0 01 00 7a 1e 00 00 00 1b 01`: display fn0 capabilities (unclear fields).
10. `13 ff 02 1e e0 01` → 4 packets (`80`, `01`, `02`, `43` sequence bytes): display fn1 =
    **display/key geometry table**. First packet: `01 00 60 01 e0 01 e0 09 00 01 00 18 00 06 00 76 00 76 01 00 b6 00 07 00 76 00 76`,
    reading: panel **480 × 480 (0x01e0)**, **9 keys**, then per key: index, x(u16), y(u16), w(u16), h(u16) with
    w = h = **0x76 = 118**, x values `0x18`(24), `0xb6`(182), `0x154`(340), y values `0x06/0x07`, `0xa4`(164),
    `0x142`(322). This is close to but not identical with Julusian's `23 + col*158` and needs an
    exact re-decode on hardware. **The device publishes its own geometry**, so keyvo should query
    it instead of hard-coding (future-proof for the 2026 MX Keypad).
11. REPROG_CONTROLS_V4 (index 0x0b): getCount → 2; getCidInfo(0)/(1) → CID `0x01A1`, `0x01A2`,
    flags `0x20` (divertable); `setCidReporting(cid, 0x03)` = divert + dvalid for both page buttons.
12. `13 ff 02 4e e0 a0` → `c0 a0`, then event `13 ff 02 10 c6 a0`: display fn4 = **set mode `a0`**
    (host-controlled display), confirmed by an unsolicited mode-change event. Boot mode `a1` is
    the device/logo mode. This is the "display claim"; losing keep-alive presumably flips back to `a1`.
13. `11 ff 04 1e 0b b8` → echoed: **KEEP_ALIVE fn1 keepAlive(timeout = 3000 ms)**.
14. `11 ff 0f 2e 00 46` → ack: **BRIGHTNESS_CONTROL fn2 setBrightness(0x0046 = 70)**. fn0/fn1
    (range / get) are not in this capture; unit (percent vs raw) still open.
15. First LCD paint (see below), then `keepAlive(3000)` **every ~1.01 s for the rest of the capture**.

## LCD write format (both captures)

```
14 ff 02 2e | a0 | 01 00 01 00 | 00 17 | 00 06 | 01 b3 | 01 b2 | 00 11 0a | ff d8 ff e0 ...
report dev vlpidx fn/swid seq  ?           x=23   y=6    w=435   h=434   len=0x00110a (24-bit) JPEG...
```

- Options+ paints the **whole panel (435×434 at 23,6)** as one JPEG, not nine key images.
- Sequence byte: `e0` = first+last (single fragment, e.g. a 0x05b4 = 1460-byte JPEG),
  `a0` = first of several, `61` = continuation/last with index 1. Julusian/logilinux use index
  base 1 (`0xa1`), Options+ uses base 0; both are reported to work.
- **Length is 24-bit** (`00 11 0a` = 4362 bytes), so the 64 KiB limit assumed by logilinux is
  a library limitation, not a device one. Verify with a > 64 KiB JPEG in M0.
- **The device ACKs every fragment on report 0x13**: `13 ff 02 2e c0 00 00 …` for fragment 0,
  `c1 00 01 00` for fragment 1. The "own writes echo back as key 1" trap described by other
  projects is this ACK (`c1 00 01`), not an input echo. keyvo can use **ACK-driven flow control**
  instead of blind 5–10 ms pacing, which also settles the "9 keys out of sync" problem.
- Options+ sent two consecutive full-panel paints 2 ms apart without waiting (#639/#647), so
  the device tolerates back-to-back writes at least at that size.
- Key input reports (`13 ff 02 00 xx 01 …` per logilinux) share VLP index 2 and report 0x13.

## Not in these captures

- No Dialpad, no Bolt receiver traffic for Creative Console devices (dev 13 in the startup
  capture is an unrelated Logitech receiver with two other devices, WPIDs `0x2123`, `0x9142`).
- No brightness range query, no per-key paint (only full panel), no page-button events, no
  sleep/timeout expiry (captures are short).

## Quick-test items derived from this file (add to the M0 measurement list)

1. Is step 4 (`5e..65` registration) required for input or painting?
2. Decode fn1 geometry table exactly and compare with Julusian's constants.
3. What does mode `a1` look like (logo?) and does keep-alive expiry flip `a0` → `a1`? Measure
   with timeout 500, 3000, 10000 ms and no pings.
4. Does the device accept per-key rectangles (x,y,w,h of one key) as logilinux does, and a
   full-panel paint as Options+ does, in the same session?
5. Is the 24-bit length honoured for a > 65535-byte JPEG?
6. ACK timing per fragment; does waiting for `c<n>` before the next fragment remove tearing?
7. Brightness fn0/fn1 replies, and whether `setBrightness(0)` really resets the device.
