# Hardware notes

Measured behaviour of the devices keyvo drives, as CONTRIBUTING ("Hardware
test policy") asks: every number here carries the date, the firmware
version and the method, and code comments point here instead of carrying
numbers alone. Protocol claims in `docs/spec.md` §5 that are not marked
*measured* are open items of the M0 measurement list
(`docs/plans/2026-09-09-project-plan.md` §5) until a record below settles
them.

## Record format

One record per measurement session and item:

```markdown
### <item number>. <short title> (<YYYY-MM-DD>)

- Device: MX Creative Keypad 046d:c354, firmware <active application entity
  from `keyvo probe`, for example `U1 66.00.B0017`>
- Method: <commands run, with their flags; Solaar stopped or running; any
  other software on the node>
- Result: <numbers with units, or the raw output in a fenced block>
- Settles: <the spec statement or default it confirms or changes>
```

The firmware version is the `firmware` line marked `application, active` in
`keyvo probe`. A record is never edited to say something else later; a new
measurement gets a new record, and the older one stays for comparison.

## Units

| Device | Firmware | Source |
|---|---|---|
| MX Creative Keypad (author's unit) | `U1 66.00.B0017`, bootloader `BL2 20.00.B0017` | Solaar 1.1.20, 2026-09-09 (`docs/research/hidpp-features.md`) |
| MX Creative Keypad (author's unit, USB serial `398533673333`) | `U1 66.00.B0017` (application, active), `BL2 20.00.B0017`, `IM 01.00.B0017`, `HW 00.00.B0000` | `keyvo probe`, first light 2026-10-25 (`01-probe-auto.log`) |

## First-light checklist (pre-alpha slice 6)

The owner runs these on the host with the binary from `just build`. Before
the first step: stop Solaar (`flatpak kill io.github.pwr_solaar.solaar`),
make sure no Solaar divert is set on the Keypad, and keep the raw output of
every command. Items refer to list A of the project plan §5; items 9
(fps), 10 (JPEG limits) and 11 (brightness) are deferred past the
pre-alpha. First light ran this list on 2026-10-25 (#40); the records are
under "Measurements" below.

1. **Descriptor and nodes.** `ls -l /dev/hidraw*`, then for each node
   `cat /sys/class/hidraw/<node>/device/uevent` and
   `xxd /sys/class/hidraw/<node>/device/report_descriptor`. Record how many
   nodes `046d:c354` has and which report ids each carries; compare with
   `just run -- probe` (its `device` line names the node it chose and the
   report ids it read). The real descriptor replaces the synthetic fixtures
   in `crates/keyvo-hid/tests/fixtures/sysfs/`.
2. **Silent device.** `just run -- keys --no-keep-alive`: do presses arrive,
   and for how long? `just run -- fill '#ff0000' --key 1 --no-keep-alive
   --hold 30`: does the paint appear, and how long does it stay?
3. **Keep-alive cadence.** `just run -- fill '#00ff00' --panel --hold 60`
   with the defaults (ping every 1 s, timeout 3 s), then with
   `--keep-alive-period 2900`, then `--no-keep-alive`. Record the time from
   the end of the hold (or from the paint, without keep-alive) until the
   logo comes back. (First light ran `--keep-alive-period 3000` and lost
   the claim; a period at or above the timeout is refused since #69, spec
   §5.3.)
4. **Keep-alive range.** The `keep-alive` line of `just run -- probe`
   (expected `500..10000 ms`). Then `--keep-alive-timeout 500` and
   `--keep-alive-timeout 10000` with `--keep-alive-period 400` and a
   `--hold`: does the claim hold?
5. **Display mode.** `just run -- probe` right after plugging in (expected
   `a2`, measured on first light), right after a `fill --hold 10` (expected
   `a0`), and
   again a while after the hold ended (after expiry). Note whether the
   paint works without the VLP root fn2 ping keyvo leaves out.
6. **Geometry.** The `geometry` lines of `just run -- probe`: the panel
   size and the nine key rectangles, against spec §5.4.
7. **Per-key and panel paint, index base and swid.** In one session:
   `fill '#ff0000' --key 1`, `fill '#0000ff' --panel`, then `fill` on every
   key (`--all`) with a PNG. Repeat with `--index-base 1`, with `--swid
   0xb`, and with both. Record which combinations paint, and the
   `fragments` and `acked` counts `fill` prints.
8. **Fragment pacing.** `fill <photo.png> --panel` (several fragments) and
   `fill <photo.png> --all`: does every fragment get its ACK (`acked` equals
   `fragments`)? Any tearing or keys out of sync? Back-to-back and fixed
   5 ms pacing are not in the CLI yet; record only the ACK-driven run.
12. **Page buttons.** `just run -- keys`: each page button gives one press
    and one release and no longer switches anything on the device itself.
    Unplug and replug, start `keys` again, and record whether the divert had
    to be sent again (it is, on every start). Note the raw event bytes
    from `just run -- keys --dump` (see "Raw frames" below).
13. **Own-write ACK against key 1.** Start `just run -- keys` in one
    terminal and run `just run -- fill '#ffffff' --key 1` in another: `keys`
    must print nothing for the paint. Then press key 1: one press, one
    release.

## Raw frames

A check that needs the bytes on the wire uses `--dump`, not an external
sniffer on the hidraw node: any device command with `--dump` prints every
report it writes (`>`) and reads (`<`) to stderr, with the milliseconds
since the start (spec §5.10). stdout stays as without it, so
`just run -- keys --json --dump 2>frames.txt | jq .` keeps the events and
the frames apart. A `0x14` image fragment prints only its header and
length. Record the dump lines a result rests on in the record's
`Result`.

- **An absent feature.** `just run -- --dump probe --feature 0x1234`: the
  `classic` and `vlp` lines say `absent` or show the error reply, and the
  dump shows the reply bytes. The VLP answer for a feature the device
  lacks is not captured yet (spec §5.2); the dump settles it.
- **A VLP error reply.** The same run, or any `vlp` line that reads
  `error: vlp error reply: ...`: the whole report is in the line and in
  the dump.

## Measurements

### Display capabilities, `maxImageSize` (2026-10-25)

Not a measurement on the author's unit: `keyvo probe` does not read
display fn0 yet. The value comes from a capture, decoded with the layout
the display pipeline report read from the Options+ helper, and it corrects
the figure the Options+ inventory gave.

- Device: MX Creative Keypad 046d:c354 of the hcooper captures (firmware
  not in the capture notes)
- Method: display fn0 `getCapabilities` reply `01 00 7a 1e 00 00 00 1b`
  (`docs/research/hcooper-captures.md` step 9), decoded with the layout of
  `docs/research/options-plus-display-pipeline.md` section 0.
- Result: `deviceScreenCount` 1; `maxImageSize` u16 BE `0x007a` in units
  of 1024 bytes = **124 928 bytes per image**, not the 31 262 bytes of
  `docs/research/options-plus-inventory.md` (a misread of `00 7a 1e` as
  one u24); `maxImageFPS` `0x1e` = 30; flags `0x1b` = deferrable display
  update, rgb565, jpeg, calibrated, origin 0. The VLP root's available
  memory, 124 960 bytes, is `maxImageSize` + 32.
- Settles: spec §5.5, the capability table and the per-image size limit.
  Confirming it on this unit needs the fn0 fields in `keyvo probe`.

### First light (2026-10-25)

Every record of this session shares the setup:

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`.
- Host: Aurora (Fedora Kinoite), kernel 7.1.8, KDE Wayland. Solaar not
  running (`flatpak ps` empty), no Solaar divert on the Keypad, nothing
  else on the node.
- Binary: `bin/keyvo` from `just build` (dev profile) on
  `epic/34-pre-alpha` at f18ee66, run on the host with the owner at the
  device.
- Bus: a passive second reader on `/dev/hidraw6` logged every input report
  with its Unix timestamp (`sniff.log`). It reads only, so host writes are
  not in it. `--dump` (spec §5.10) did not exist yet.
- Raw output: the first-light log archive attached to #40: `sniff.log`, the
  command logs `<NN>-<label>.log` (first line the local start time, then
  the command and its output) and `00-item1-report-descriptor.hex`. The
  records cite them as `<file>` line or `sniff.log` timestamp. The owner's
  report on #40 carries what the logs do not (what the panel showed).

List A of the project plan §5: items 1 to 8, 12 and 13 are resolved below.
Items 9 (frames per second), 10 (JPEG limits) and 11 (brightness) are
deferred past the pre-alpha.

#### 1. Descriptor and nodes (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: `xxd /sys/class/hidraw/hidraw6/device/report_descriptor`
  (`00-item1-report-descriptor.hex`), `bin/keyvo probe` without
  (`01-probe-auto.log`) and with `--device /dev/hidraw6`
  (`02-probe-device.log`).
- Result: Linux exposes one USB interface (`3-5.1:1.0`, `usbhid`, path
  `usb-0000:80:14.0-5.1/input0`) and one hidraw node (`hidraw6`) for
  `046d:c354`. The descriptor is 140 bytes with five top-level
  collections:

  | Collection | Report id | Size | Direction |
  |---|---|---|---|
  | Vendor `FF43`, usage `1A02` | `0x11` | 19 bytes | in and out (HID++ long) |
  | Vendor `FF43`, usage `1A08` | `0x13` | 31 bytes | in and out (VLP short) |
  | Vendor `FF43`, usage `1A10` | `0x14` | 4094 bytes | out only (VLP long, image fragments) |
  | Generic Desktop, System Control | `0x04` | 2 bits and padding | in |
  | Consumer Control | `0x05` | 1 bit and padding | in |

  `probe` found the same node without `--device` and printed `device
  /dev/hidraw6 (Logitech MX Creative Keypad, 046d:c354, reports 04 05 11 13
  14)` both ways.
- Settles: spec §5.1, one node; the search by report `0x14` stays (it is
  what makes a split topology safe). The descriptor is the sysfs fixture
  of `crates/keyvo-hid/tests/fixtures/sysfs/`.

#### 2. Silent device (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: `keys --json --no-keep-alive` (`22-keys-no-keepalive.log`);
  `fill '#ff0000' --key 1 --no-keep-alive --hold 30`
  (`19-item2-fill-no-keepalive.log`), then `probe` at the end of the hold
  and 25 s later (`20-item2-probe-after.log`, `21-item2-probe-later.log`).
  The `keys` session of item 12 was still running and pinging when the
  first command started; its last ping is at `sniff.log` 1792913447.572,
  its expiry at 1792913450.572.
- Result: input arrives without pings. Key 1 (`sniff.log`
  1792913581.788) and key 9 (1792913614.613), 131 s and 164 s after the
  last ping on the bus, each gave one press and one release. A paint
  without pings stays: `probe` read `a0` at the end of the 30 s hold and
  25 s later, and `sniff.log` has no expiry event between the paint's
  setMode (1792912249.517) and the next run. Item 3's run without pings
  read `a0` 120 s after the paint.
- Settles: spec §5.3. Without pings the device delivers key input and
  keeps a paint; the logo comes back only on a `KEEP_ALIVE` expiry, and an
  expiry needs a timeout a ping has set. Whether an idle timer later moves
  an unpinged display to `a2` is not measured.

#### 3. Keep-alive cadence (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: `fill '#00ff00' --panel --hold 60` with the defaults (ping every
  1 s, timeout 3000 ms, `28-item3-default.log`), with
  `--keep-alive-period 3000` (`30-item3-period3000.log`) and with
  `--no-keep-alive` (`32-item3-none.log`); `probe` after each
  (`29-`, `31-`, `33-item3-probe-*.log`). Times from `sniff.log`: the
  `keepAlive` replies (`11 ff 04 1e`), the expiry event (`11 ff 04 00`)
  and the mode-change event (`13 ff 02 10 c<n> a1`).
- Result:

  | Pings | Timeout | Pings seen | Expiry after the last ping |
  |---|---|---|---|
  | every 1 s (default) | 3000 ms | 61, gaps 0.999 to 1.001 s, every reply echoes `0b b8` | 3.001 s (1792912466.383) |
  | every 3 s | 3000 ms | 21, gaps 3.000 s | the first expiry 3.000 s after the first ping (1792912474.452), the last 3.000 s after the last ping (1792912534.452) |
  | none | - | 0 | no expiry; `a0` 60 s after the hold |

  The expiry event comes first and the mode-change event to `a1` 0 to
  4 ms after it, at every expiry of the log. With period and timeout both
  3000 ms the display went to `a1` 3 s into the hold; the device answered
  the remaining pings and the mode stayed `a1` (`31-` read `a1`), because
  this binary sent `setMode` once.
- Settles: spec §5.3, the period has to stay below the timeout (rule since
  #69, which also reclaims a lost claim); the expiry fires the granted
  timeout after the last ping. The default cadence holds a paint.

#### 4. Keep-alive range (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: the `keep-alive` line of `probe` and the fn0 reply in
  `sniff.log`; `fill '#00ff00' --panel --keep-alive-period 400 --hold 10`
  with `--keep-alive-timeout 500` (`22-item4-ka500.log`) and
  `--keep-alive-timeout 10000` (`24-item4-ka10000.log`), `probe` after
  (`23-`, `25-` to `27-item4-*.log`).
- Result: fn0 `getTimeoutRange` answers `11 ff 04 0e 01 f4 27 10`
  (1792912081.437), 500..10000 ms, as `probe` prints. Timeout 500: 26
  pings 0.399 to 0.401 s apart, every reply echoes `01 f4`, no expiry
  during the hold, the expiry 0.501 s after the last ping (1792912315.073).
  Timeout 10000: 26 pings, replies echo `27 10`, the expiry 10.001 s after
  the last ping (1792912337.639); `probe` read `a0` 7 s after the hold and
  `a1` 13 s after it.
- Settles: spec §5.3, the range and that the device grants the timeout
  asked for; `[timing]` defaults unchanged.

#### 5. Display mode (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: the `display` line of `probe` at the points below; the
  mode-change events in `sniff.log`.
- Result:

  | When | `getMode` | Log |
  |---|---|---|
  | Keypad plugged in since boot, idle about 8 minutes | `a2` | `01-probe-auto.log` |
  | about 1 minute after an unplug and replug | `a2` | `36-item5-probe-after-replug.log` |
  | right after a `fill` (setMode `a0` sent) | `a0` | `06-probe-after-fill.log` |
  | 18 s after an expiry | `a1` | `18-probe-after-expiry.log` |

  The mode-change event arrives on `setMode(a0)` (`13 ff 02 10 c4 a0`,
  1792912081.451) and on an expiry (`13 ff 02 10 c5 a1`, 1792912121.805);
  the low nibble of byte 4 counts up per event. keyvo sent `setMode` only
  when `getMode` was not `a0`: one `setMode` reply (1792912081.451) for the
  twelve fills of item 7 (`05-` to `17-*.log`, 08:08:01 to 08:08:38), whose
  holds followed each other closely enough that no expiry came between
  them. keyvo's claim never sends VLP root fn2,
  and every paint of the session appeared.
- Settles: spec §5.3 and §5.4. The mode after plugging in is `a2`; `a1` is
  the state after a `KEEP_ALIVE` expiry; painting needs no VLP root fn2.

#### 5. Display mode right after a replug: the plug-in splash (2026-10-25)

Not a first-light record: it comes from the owner's end-to-end check
(#52, finding 2), recorded here next to the `a2` measurement above.

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
  (application, active)
- Method: `bin/keyvo-daemon` built at `e4b37d3`, running and painting the
  global profile; the Keypad unplugged and plugged back in; the daemon
  log of the replug. Solaar stopped. No raw capture of the replug exists.
- Result:

  ```
  20:48:30.467 WARN keyvo_daemon::device: keypad: hid i/o error: Protocol error (os error 71)
  20:48:31.467 WARN keyvo_daemon::device: keypad: cannot open the keypad: hid i/o error: no hidraw node for 046d:c354
  20:48:34.476 WARN keyvo_daemon::device: keypad: display stayed in mode 0xa4 after setMode(a0)
  20:48:41.145 INFO keyvo_daemon::daemon: showing profile global page 1 in auto mode
  ```

  About three seconds after the plug-in `getMode` answers `a4`
  (SplashBright, spec §5.4) after `setMode(a0)`: the switch does not take
  while the splash runs. After `[timing].reconnect_interval` the next
  connection claimed the display, and the profile was shown about ten
  seconds after the failed open. The `a2` above was read about one minute
  after a replug, when the splash is over.
- Settles: spec §5.4, the claim waits out `a4` instead of failing; spec
  §6.1, the defaults of `claim_splash_retry` (500 ms) and
  `claim_splash_timeout` (10000 ms). The splash length itself is not
  measured; a `--dump` of a replug would.

#### 6. Geometry (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: the `geometry` lines of `probe` (`01-probe-auto.log` lines
  17-26) and the four fn1 reply packets (`sniff.log` 1792912081.438 and
  .439).
- Result: panel `01e0 x 01e0` = 480x480, 9 keys, each `0076 x 0076` =
  118x118, origins for keys 1 to 9: (24, 6), (181, 7), (340, 6), (23,
  164), (181, 164), (340, 164), (23, 321), (181, 322), (339, 321). The
  decoder matches the raw bytes. Five origins differ from the hcooper
  capture the spec quoted: key 2 (182, 7), key 5 (181, 163), key 6 (339,
  164), key 8 (181, 321), key 9 (340, 321) there.
- Settles: spec §5.4, the device's table; the table differs between units
  or firmware, which is why keyvo reads it on every claim. The geometry
  `keyvo preview` uses without a device (`display::captured_geometry`) is
  still the hcooper table; a change is a follow-up, not part of this
  record.

#### 7. Per-key and panel paint, index base and software id (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: `fill '#ff0000' --key 1`, `fill '#0000ff' --panel` and `fill
  <png> --all`, each with `--hold 3`, with the defaults (`05-` to
  `08-*.log`), `--index-base 1` (`09-` to `11-`), `--swid 0xb` (`12-` to
  `14-`) and both (`15-` to `17-`). A second run watched by the owner
  painted key 5 in all four combinations (`38-` to `41-vis*.log`), the
  panel and all keys with the defaults (`42-` to `44-vis*.log`). ACKs from
  `sniff.log`.
- Result: every write was acknowledged fragment by fragment:

  | Placement | base 0, swid `0xd` | base 1, swid `0xd` | base 0, swid `0xb` | base 1, swid `0xb` |
  |---|---|---|---|---|
  | `--key N` (1024 bytes, 1 fragment) | 1 acked, painted | 1 acked, painted | 1 acked, painted | 1 acked, painted |
  | `--panel` (5924 bytes, 2 fragments) | 2 acked, painted | 2 acked | 2 acked | 2 acked |
  | `--all` PNG (9 x 6135 bytes, 2 fragments each) | 9 x 2 acked, painted | 9 x 2 acked | 9 x 2 acked | 9 x 2 acked |

  The ACK is `13 ff 02 2<swid> c<index> 00 <last>`: the write's swid, the
  fragment index as sent (`c0 c1` with base 0, `c1 c2` with base 1), and
  `01` on the last fragment, else `00` (`sniff.log` 1792912084.527 to
  1792912112.783).
- Observation (the owner's photograph, report on #40): after a panel
  paint, a per-key paint leaves a strip of the panel colour at the bottom
  and right edge of the key. The visible key window is larger than, or
  offset from, the 118x118 rectangle at the published origin; the panel
  write (435x434 at 23, 6) covers those pixels, a key write does not.
  Measured in #70 with `fill --margin` (`docs/pre-alpha-test.md` section
  4). The daemon paints the panel background before the keys (#77), so
  the strip shows the profile's background; the per-key margin over the
  measured window is #72.
- Settles: spec §5.5, both index bases and both software ids paint, so the
  defaults stay: index base 0, swid `0xd`. The ACK layout is the fixture
  `tests/protocol-fixtures/hid/image-acks.txt`.

#### 8. Fragment pacing (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: the ACK-driven writer only: `fill <png> --all --hold 8`
  (`43-vis6-photo-all.log`) and `fill <png> --panel --hold 8`
  (`44-vis7-photo-panel.log`), the PNG a 480x480 colour noise image.
- Result: `acked` equals `fragments` in every run of the session. The
  panel write was 60480 bytes in 15 fragments, its 15 ACKs
  1792913405.852 to .867, about 1 ms per fragment. The `--all` write, nine
  keys of two fragments, has its 18 ACKs within 0.294 s (1792913393.429
  to .723), about 1 ms between the two ACKs of a key and 35 ms from key to
  key. The owner saw no tearing and no key out of sync.
- Settles: spec §5.5, ACK-driven flow control works and the ACK timeout is
  never reached. Back-to-back and fixed 5 ms pacing are not in the CLI and
  were not measured.

#### 12. Page buttons (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: `keys --json` (`20-keys-json.log`), unplug and replug, `keys
  --json` again (`21-keys-json-replug.log`); events and replies from
  `sniff.log`.
- Result: each page button gives one press and one release and switches
  nothing on the device while diverted. The `0x1B04` notifications: left
  `11 ff 0b 00 01 a1`, right `11 ff 0b 00 01 a2`, none held `11 ff 0b 00
  00 00`, both held `11 ff 0b 00 01 a2 01 a1` (1792912854.506), the CIDs
  held in press order. After the replug keyvo sent the divert again;
  `setCidReporting` echoed `01 a1 03` and `getCidReporting` read back `01
  a1 01` (1792913172.736, .757). keyvo reads only after the set, so the
  logs do not show whether the divert survived the replug; the reply the
  report on #40 quotes as `getCidReporting`, `01 a1 00 00 20 00 01`, is
  the `getCidInfo` reply (flags `0x20`, divertable). `keys` ended with
  `Error: device disconnected` on unplug, and the Keypad came back on
  `hidraw6`.
- Settles: spec §5.6, the notification layout including both buttons
  held; re-sending the divert on every connect stays. Divert with settle
  and spacing 0 has no CLI flag and was not run.

#### 13. Own-write ACK against key 1 (2026-10-25)

- Device: MX Creative Keypad 046d:c354, firmware `U1 66.00.B0017`
- Method: `fill '#ffffff' --key 1 --hold 2`
  (`35-item13-fill-while-keys.log`) while `keys --json` ran
  (`20-keys-json.log`), then a real press of key 1.
- Result: `keys` printed nothing for the paint, whose ACK `13 ff 02 2d c0
  00 01` is at 1792912909.376; the next press of key 1 (1792913087.854)
  gave one press and one release (`20-keys-json.log` lines 34-35). A key
  report is the fn0 event with swid 0 (`13 ff 02 00 c<n> 01 <held
  keys...>`), the held keys in press order: holding 2 then 1 gives `01 02
  01`, releasing 1 gives `01 02`, releasing 2 gives `01 00`. The low
  nibble of byte 4 counts up per report. No report repeats while a key is
  held: 204 key reports, none with the payload of the one before.
- Settles: spec §5.6, the input gate on byte 3 and the held-set diff. The
  reports are the fixture `tests/protocol-fixtures/hid/key-input.txt`.
