# HID++ golden fixtures

Request/response scripts replayed by `FakeHidTransport` (`keyvo-core`,
`FakeHidTransport::from_script`) in the `keyvo-hid` golden tests
(`crates/keyvo-hid/tests/golden.rs`), the `keyvo-cli` command tests
(`crates/keyvo-cli/src/`) and the `keyvo-daemon` tests
(`crates/keyvo-daemon/tests/`), whose `Pad` replays `keep-alive-ping.txt`
(exchanges 0 and 1), `display-claim.txt` and `reprog-divert.txt` for every
connection and answers the key image fragments itself.

## Notation

- `> <hex>`: a report the host writes; the test fails if keyvo writes anything else.
- `< <hex>`: a report the device sends. After a `>` line it answers that write;
  before the first `>` line it is unsolicited and readable at once.
- `#` starts a comment. Every file names its source capture, frame numbers and
  the line range in the decoded sequence.
- Bytes are the whole report, report id first, padding included (20 bytes for
  `0x11`, 32 for `0x13`).

## Sources and licence

The frames are copied from Hunter Cooper's USBPcap captures of Logi Options+
on Windows (<https://github.com/hcooper-idealbuilders/mx-creative-controller>,
MIT, `captures/external/hcooper/LICENSE.hcooper`), as decoded in
`docs/research/hcooper-startup-keypad-sequence.txt`. Options+ uses software
id `0xe` in these frames, so the tests use it too.

| File | Frames | Content |
|---|---|---|
| `root-ping.txt` | #1563, #1565 | classic ROOT ping, HID++ 4.5 |
| `keep-alive-range.txt` | #1631 to #1637 | getFeature(0x0008) = 4, KEEP_ALIVE range 500..10000 ms |
| `vlp-get-feature-display.txt` | #1643, #1645 | VLP getFeature(0x19A1) = VLP index 2 |
| `root-get-feature-absent.txt` | #1713, #1715 | getFeature(0x0020) = 0, not present |
| `root-get-feature-reprog.txt` | #1681, #1683 | getFeature(0x1B04) = 0x0b |
| `keep-alive-ping.txt` | #1631, #1633, #1753 to #1801 | getFeature(0x0008) = 4, three keepAlive(3000) pings echoed |
| `display-geometry.txt` | #1643, #1645, #1667 to #1675 | display fn1 geometry over four packets: 480x480, nine 118x118 keys |
| `display-claim.txt` | #1559 to #1773 (selected) | the claim of spec §5.4 in keyvo's order, ending with setMode(a0) and the mode-change event |
| `reprog-divert.txt` | #1681 to #1731 (selected) | getFeature(0x1B04) = 0x0b, getCount = 2, getCidInfo for 0x01A1 and 0x01A2, then per page button setCidReporting(cid, 0x03) and getCidReporting (keyvo's order; Options+ reads it before the set) |
| `key-input.txt` | #1770, #1782, #1785 | LCD key held-set reports interleaved with two image ACKs and the mode-change event; the key reports are not captured (layout from logilinux and Julusian) |
| `page-buttons.txt` | #1719 | diverted page-button CID lists around a setCidReporting reply; the notifications are not captured (layout from logilinux and Julusian) |
| `firmware-version.txt` | #1567 to #1581, #1309 to #1315 | getFeature(0x0003) = 2, four firmware entities: `BL2 20.00.B0017`, `U1 66.00.B0017` (active), `IM 01.00.B0017`, `HW 00.00.B0000`; entities 2 and 3 retagged from swid 0xd to 0xe |
| `probe.txt` | #1559 to #1691 (selected), #1309 to #1315 | `keyvo probe` in its request order: ping, getFeature for 0x0003, 0x0008, 0x1B04 and 0x8040, the firmware walk, the keep-alive range, the VLP protocol, getFeature(0x19A1), display modes, mode and geometry (replayed by `crates/keyvo-cli/src/probe.rs`) |
| `image-write-panel.txt` | #1643, #1645, #1779 to #1785 | a 4362-byte panel JPEG in two `0x14` fragments, each followed by its `0x13` ACK (keyvo's order; Options+ sends both fragments first) |

Two fixtures carry the reclaim after a lost claim (spec §5.3):

| File | Source | Content |
|---|---|---|
| `claim-lost.txt` | first-light report on #40 (2026-10-25, firmware `U1 66.00.B0017`) | the display mode-change event to `a1` and the `KEEP_ALIVE` expiry event one expiry sends in the same millisecond; the report quotes the leading bytes, the padding is zero |
| `display-reclaim.txt` | hcooper #1765 to #1773, steps 6 and 7 of `display-claim.txt` | setMode(a0) and the getMode that confirms it, with the mode-change event announcing `a0` |

The image writer's golden tests also reproduce the LCD writes of
`02-binding-paint.pcap` (frames #521, #639, #640, software id `0xd`) from
`captures/external/hcooper/derived/binding-paint-frames.tsv`, which carries
the whole 4095-byte reports.
