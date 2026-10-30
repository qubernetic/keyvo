# HID++ golden fixtures

Request/response scripts replayed by `FakeHidTransport` (`keyvo-core`,
`FakeHidTransport::from_script`) in the `keyvo-hid` golden tests
(`crates/keyvo-hid/tests/golden.rs`), the `keyvo-cli` command tests
(`crates/keyvo-cli/src/`) and the `keyvo-daemon` tests
(`crates/keyvo-daemon/tests/`), whose `Pad` replays `keep-alive-ping.txt`
(exchange 0), `display-claim.txt` and `reprog-divert.txt` for every
connection and answers the keep-alive pings and the key image fragments
itself.

## Notation

- `> <hex>`: a report the host writes; the test fails if keyvo writes anything else.
- `< <hex>`: a report the device sends. After a `>` line it answers that write;
  before the first `>` line it is unsolicited and readable at once.
- `< +<ms> <hex>`: the same report, readable only `<ms>` milliseconds after
  the write it answers. `FakeHidTransport` counts the time on a clock of its
  own that every waiting read advances, so a slow reply costs no wall-clock
  time; a report that misses its request's wait stays queued for the next
  read, like a late reply.
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
| `display-capabilities.txt` | #1643, #1645, #1663, #1665 | display fn0 getCapabilities: 1 screen, `maxImageSize` 0x007a x 1024 = 124 928 bytes, `maxImageFPS` 30, flags 0x1b (deferrable, rgb565, jpeg, calibrated; origin 0) |
| `display-claim.txt` | #1559 to #1773 (selected) | the claim of spec §5.4 in keyvo's order, ending with setMode(a0) and the mode-change event |
| `firmware-version.txt` | #1567 to #1581, #1309 to #1315 | getFeature(0x0003) = 2, four firmware entities: `BL2 20.00.B0017`, `U1 66.00.B0017` (active), `IM 01.00.B0017`, `HW 00.00.B0000`; entities 2 and 3 retagged from swid 0xd to 0xe |
| `probe.txt` | #1559 to #1691 (selected), #1309 to #1315 | `keyvo probe` in its request order: ping, getFeature for 0x0003, 0x0008, 0x1B04 and 0x8040, the firmware walk, the keep-alive range, the VLP protocol, getFeature(0x19A1), display modes, mode, capabilities and geometry (replayed by `crates/keyvo-cli/src/probe.rs`) |
| `image-write-panel.txt` | #1643, #1645, #1779 to #1785 | a 4362-byte panel JPEG in two `0x14` fragments, each followed by its `0x13` ACK (keyvo's order; Options+ sends both fragments first) |

## First light

Frames captured on the owner's Keypad on first light (2026-10-25, firmware
`U1 66.00.B0017`, qubernetic/keyvo#40). The source is the first-light log
archive attached to #40: `sniff.log`, a passive second reader on
`/dev/hidraw6` that logged every input report with its Unix timestamp, and
the numbered command logs `<NN>-<label>.log`. The sniffer reads only, so a
fixture's `>` lines are derived from keyvo's encoder; every `<` line is
copied whole and its comment names the `sniff.log` timestamp. The
archive's helper scripts are not in the repository.

| File | Source | Content |
|---|---|---|
| `key-input.txt` | `sniff.log` 1792912081.451 to 1792913088.037, `20-keys-json.log` lines 26-35, `35-item13-fill-while-keys.log` | LCD key held-set reports (key 2, keys 2 and 1, key 2, none, key 1, none), the mode-change event to `a0` and the ACK of a key 1 paint, which yield nothing |
| `image-acks.txt` | `sniff.log` 1792912084.527 to 1792912112.783, `07-fill-panel-blue.log`, `10-fill-panel-ib1.log`, `13-fill-panel-swidb.log`, `16-fill-panel-ib1-swidb.log` | the ACKs of four two-fragment panel writes, index base 0 and 1 with swid `0xd` and `0xb`: `13 ff 02 2<swid> c<index> 00 <01 on the last fragment>`; the golden test builds the writes with keyvo's encoder |
| `reprog-divert.txt` | `sniff.log` 1792913172.674 to 1792913172.799, `21-keys-json-replug.log` | the divert after an unplug and replug: getFeature(0x1B04) = 0x0b, getCount = 2, getCidInfo for 0x01A1 and 0x01A2, then per page button setCidReporting(cid, 0x03) and getCidReporting; the replies equal hcooper #1683 to #1731 byte for byte |
| `page-buttons.txt` | `sniff.log` 1792912854.503 to 1792913298.613, `20-keys-json.log` lines 30-33, `21-keys-json-replug.log` lines 4-7 | diverted page-button CID lists, both buttons held (`01 a2 01 a1`), around the setCidReporting reply of the divert after a replug |

Two fixtures carry the reclaim after a lost claim (spec §5.3):

| File | Source | Content |
|---|---|---|
| `claim-lost.txt` | first light (see above), `sniff.log` 1792912466.383 and .386, the end of `28-item3-default.log` | the `KEEP_ALIVE` expiry event, then the display mode-change event to `a1`, 0 to 4 ms apart, as every expiry of the log sends them |
| `display-reclaim.txt` | hcooper #1765 to #1773, steps 6 and 7 of `display-claim.txt` | setMode(a0) and the getMode that confirms it, with the mode-change event announcing `a0` |

Three fixtures are not captured; `keyvo probe --feature` (spec §5.10) is
tested on the first two until the owner's `--dump` run records the real
bytes, the claim's splash wait (spec §5.4) on the third until a replug is
captured:

| File | Source | Content |
|---|---|---|
| `vlp-get-feature-absent.txt` | built from `vlp-get-feature-display.txt` and the id of `root-get-feature-absent.txt` | VLP getFeature(0x0020) answered with index 0, the absent answer keyvo assumes |
| `probe-feature-error.txt` | the classic error layout of HID++ 2.0, the VLP error shape keyvo matches | getFeature(0x1234) refused with `InvalidArgument` on the classic and the VLP root |
| `display-splash.txt` | the layout of `display-reclaim.txt` with the getMode answer `a4` of the owner's daemon log of a replug (#52, finding 2, 2026-10-25, firmware `U1 66.00.B0017`) | steps 6 and 7 of `display-claim.txt` while the plug-in splash runs: getMode answers `a4` twice, the third setMode(a0) takes |

The image writer's golden tests also reproduce the LCD writes of
`02-binding-paint.pcap` (frames #521, #639, #640, software id `0xd`) from
`captures/external/hcooper/derived/binding-paint-frames.tsv`, which carries
the whole 4095-byte reports.

## Dialpad over the Bolt receiver (own)

Frames of the owner's hardware check of slice 7 (#187): part 1, the probe
over Bolt (2026-10-29, at 1530129), and part 2, the fixed build over Bolt
(2026-10-30, at 8f56281, `keyvo --dump dial`), MX Creative Dialpad
firmware `RBO 07.00.B0016` on slot 3 of the owner's receiver
(`docs/hardware-notes.md`, "Dialpad over Bolt"). Each file's `#` lines name
the comment, the dump block and its milliseconds, and every byte that is
not a frame of the dump. keyvo uses software id `0xe`, the slot search
`0xf`, `0x1` and `0x2` (spec §5.1). The golden tests in
`crates/keyvo-hid/tests/golden.rs` join them in keyvo's order, and the
Bluetooth test replays them with device index `ff`.

Two files are not own frames: `receiver-arrival.txt` (no `0x41` arrival
appeared in either check) and the getFwInfo replies of
`receiver-firmware.txt` (neither check carries the frames; the replies are
built to decode to part 1's `firmware` lines). The input cases the
capture has none of (the ends of the `i8` delta, another slot's
notification, a DJ report) are written into their golden test.

| File | Source | Content |
|---|---|---|
| `receiver-slot-probe.txt` | own, part 2 | the slot search: slots 1 and 2 "no 0x4610" after 910 ms and 624 ms, slot 3 getFeature(0x4610) = 0x0d, getFeature(0x0003) = 0x02, getDeviceInfo with model id `bc 00 00 00 00 00` |
| `receiver-cold-search.txt` | own, part 1, edited (getDeviceInfo part 2) | the search from cold: slots 1 and 2 answer "no 0x4610" after 734 ms and 541 ms, slot 3 after 180 ms, each reply comes twice, and the copy of slot 3's 0x4610 reply arrives after the getFeature(0x0003) request |
| `receiver-asleep.txt` | own, parts 1 and 2 | the search while the Dialpad is in deep sleep: slots 1 and 2 "no 0x4610", slot 3 connect fail (`10 03 8f 00 0f 04`), slots 4 to 6 resource error (`0x09`), then the Dialpad's `0x1D4B` reconnect and `0x1004` battery reports, no `0x41` |
| `receiver-controls.txt` | own, part 2 | getFeature(0x1B04) = 0x0a, getCount = 4, getCidInfo for CIDs 0x0053, 0x0056, 0x0059, 0x005A (flags 0x31, additional flags 0x04) |
| `receiver-firmware.txt` | synthetic frames, part 1 values | getDeviceInfo and getFwInfo for four entities: `BL2 18.00.B0016`, `RBO 07.00.B0016` (active), type 0x05 `00.00.B0126`, `HW 00.01.B0000` (replayed by `crates/keyvo-cli/src/probe.rs`) |
| `receiver-multiroller.txt` | own, part 2 | 0x4610 getCapabilities = 2 rollers, getRollerCapabilities 40 and 180 increments |
| `receiver-divert.txt` | own, part 2 | setCidReporting(cid, 0x03) and getCidReporting per button, setMode(roller, DIVERTED) for both, echoed |
| `receiver-restore.txt` | own, part 2 | setCidReporting(cid, 0x02) per button, the first after 868 ms, setMode(roller, NATIVE) for both |
| `receiver-input.txt` | own, part 2 | the Dialpad's `0x1D4B` and `0x1004` status reports, the four buttons, a hold and a chord, the slow and fast rotation blocks of both rollers |
| `receiver-arrival.txt` | synthetic | the arrival of slot 3 with the link lost (`10 03 41 10 40 00 bc`), then established |
