# HID++ golden fixtures

Request/response scripts replayed by `FakeHidTransport` (`keyvo-core`,
`FakeHidTransport::from_script`) in the `keyvo-hid` golden tests
(`crates/keyvo-hid/tests/golden.rs`).

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
