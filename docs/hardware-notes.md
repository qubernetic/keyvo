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
| MX Creative Dialpad (owner's unit, WPID `BC00`, slot 3 of the owner's Bolt receiver) | `RBO 07.00.B0016` (application, active), `BL2 18.00.B0016`, ` 00.00.B0126` (type `0x05`), `HW 00.01.B0000` | `keyvo probe` over Bolt, 2026-10-29 (#187, part 1; "Dialpad over Bolt" below) |

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

### Launch focus on KWin and Flatpak (2026-10-28)

Not a Keypad measurement: the desktop behaviour the launch rule of spec
§6.3 (#179) depends on, measured by the owner on the host and reported on
#179.

- Host: Aurora (Fedora Kinoite), KDE Wayland, KWin 6.7.4 (the research
  report `docs/research/2026-10-27-launch-focus-matching-prior-art.md`).
  The Flatpak version was not noted with the measurement.
- Method, focus stealing prevention: the `[Windows]` group of the owner's
  `kwinrc` read for `FocusStealingPreventionLevel` (the issue body of
  #179); the beta test (#194 block) repeats it with `kreadconfig6 --file
  kwinrc --group Windows --key FocusStealingPreventionLevel`.
- Result: `kwinrc` has no such key, so KWin runs at its default level, 1
  (Low), where every new window is activated.
- Method, session of a Flatpak launch: KCalc started with `flatpak run
  org.kde.kcalc` in a new session (`setsid`, as keyvo starts a launch);
  the session id (the fourth field after the command name in
  `/proc/<pid>/stat`) of the `flatpak run` process, the `bwrap` processes
  and the KCalc process compared.
- Result: the `flatpak` to `bwrap` to application chain keeps the
  session: KCalc's session id equals the pid of the top `flatpak run`
  process. Neither `flatpak` nor `bwrap` starts a new session.
- Settles: spec §6.3 and §8, the lineage check by session id holds for
  Flatpak launches, and the whole launch rule depends on the Low level;
  at Medium or above a launched window only demands attention and no app
  switch reaches the pad. The open question of section 9 of the research
  report is answered.

### Dialpad over Bolt (2026-10-29 and 2026-10-30)

The owner's hardware check of slice 7 (#187) in two parts: part 1, the
probe over Bolt
(<https://github.com/qubernetic/keyvo/issues/187#issuecomment-6504351636>),
and part 2, the build with the fix of #200
(<https://github.com/qubernetic/keyvo/issues/187#issuecomment-6517302440>).
Every number below is in one of the two comments, or counted from the raw
frames they carry. Every record of this section shares the setup:

- Device: MX Creative Dialpad, WPID `BC00`, firmware `RBO 07.00.B0016`
  (application, active), bootloader `BL2 18.00.B0016`, hardware `HW
  00.01.B0000`, a fourth entity ` 00.00.B0126` of type `0x05` with an
  empty prefix (`keyvo probe`, part 1). Fresh batteries: 95 %, level
  `full`, through `0x1004` (part 1).
- Receiver: Logi Bolt `046d:c548` (`Logitech USB Receiver`), the Dialpad
  on slot 3 next to two paired devices without `0x4610`; keyvo finds the
  slot at runtime, it is no constant.
- Binary: `bin/keyvo` from `just build`, on `epic/180-beta` at 1530129
  (part 1, 2026-10-29) and at 8f56281, PR #201 merged (part 2,
  2026-10-30), run on the host. Solaar and the daemon stopped, the Keypad
  connected.
- Raw output: `--dump` lines (spec §5.10) in the two comments; the
  fixtures `tests/protocol-fixtures/hid/receiver-*.txt` replay them (their
  `#` lines name the block and the milliseconds), and the receiver's
  report descriptors are the sysfs fixtures of `keyvo-hid`.
- Not measured yet: the Dialpad over Bluetooth (pairing, `dial --device`,
  the node after sleep, pairing back to Bolt). These steps of #187 stay
  open.

#### D1. Probe, feature indices and controls (2026-10-29)

- Method: `bin/keyvo probe` over Bolt with the Dialpad awake; `bin/keyvo
  --dump probe | grep ' 8f '`.
- Result:

  ```
  receiver    /dev/hidraw3 (Logitech USB Receiver, 046d:c548, reports 10 11)
  slots       1 no 0x4610, 2 no 0x4610, 3 dialpad
  features    0x1b04 REPROG_CONTROLS_V4 index 0x0a, 0x4610 MULTI_ROLLER index 0x0d
  firmware    BL2 18.00.B0016  bootloader
              RBO 07.00.B0016  application, active
               00.00.B0126  type 0x05
              HW 00.01.B0000  hardware
  buttons     1 0x0053, 2 0x0056, 3 0x0059, 4 0x005a
  rollers     0 roller  40 increments per rotation, 0 ratchet increments
              1 dial  180 increments per rotation, 0 ratchet increments
  ```

  The feature indices on this device: `0x1b04` at `0x0a`, `0x4610` at
  `0x0d`, `0x1004 UNIFIED_BATTERY` at `0x09` (part 1), `0x1D4B
  WIRELESS_DEVICE_STATUS` at `0x04` (its reconnect report, D6). They are
  this device's values; keyvo resolves every index with
  `ROOT.getFeature` and never holds one as a constant. The part 2 dump
  adds: `getCidInfo` gives flags `0x31` and additional flags `0x04` for
  all four controls; `getDeviceInfo` answers `04 b9 78 a8 29 00 02 bc 00
  00 00 00 00 00 01 00` (4 entities, unit id `b978a829`, transport bits
  `0x0002`, model id `bc 00 00 00 00 00`).
- Settles: spec §5.8, the feature table, the four CIDs and the 40 and
  180 increments; the cosmetic empty prefix of the `type 0x05` entity is
  open.

#### D2. Receiver error replies in the slot search (2026-10-29 and 2026-10-30)

- Method: the `8f` lines of `--dump probe` (part 1) and of `--dump dial`
  and `probe` with the Dialpad in deep sleep (part 2).
- Result: the receiver answers the empty slots 4 to 6 at once with `10 04
  8f 00 0e 09 00`, `10 05 ...` and `10 06 ...`: code `0x09`, resource
  error. A paired Dialpad that sleeps gives `10 03 8f 00 0e 04 00`
  (part 1) and `10 03 8f 00 0f 04 00` (part 2, after a night): code
  `0x04`, connect fail, at once.
- Settles: spec §5.1 and §5.8, `0x09` marks an empty slot on this
  receiver, not a retryable one as the static pass had it; `0x04` marks a
  silent slot. The `0x08` the synthetic fixtures used did not appear.

#### D3. Slot search from cold, and the fix of #200 (2026-10-29 and 2026-10-30)

- Method: part 1, `bin/keyvo --dump probe` repeated with the Dialpad
  awake and from sleep, `[timing].slot_probe` 300 ms; one raw
  `getFeature(0x1004)` to slot 3. Part 2, the same on the build of #201
  (`slot_probe` 1000 ms, a software id per search request).
- Result, part 1 (first reply to `getFeature`):

  | slot | device | cold first reply | warm reply |
  |---|---|---|---|
  | 1 | paired device, no 0x4610 | 734 ms, >300 ms | 11-16 ms |
  | 2 | paired device, no 0x4610 | 302-541 ms | 17 ms |
  | 3 | Dialpad | none within 300 ms, 294 ms, 180 ms | 16-17 ms |

  The raw `getFeature(0x1004)` to the sleeping Dialpad took 771 ms, the
  next requests 21-22 ms. The receiver delivered a slow reply twice, and
  with one software id the copy of the `0x4610` reply answered the
  following `getFeature(0x0003)`: seen twice on slot 3 and once on slot 2.
- Result, part 2: the slow slots answer inside the 1000 ms (slot 1 at
  791 ms, slot 2 at 819 ms on the first run); the search requests go out
  with software ids `0f`, `01` and `02`; no copy was matched to a request
  in any run.
- Settles: spec §5.1, the `slot_probe` default of 1000 ms and the
  software ids of the search (#200). The firmware entity reads after the
  search share one software id; a copy there was not seen on an awake
  device and is an open follow-up.

#### D4. Buttons (2026-10-30)

- Method: `bin/keyvo dial` and `--dump dial`: each button pressed once,
  one held, two held together.
- Result: each of the four buttons gives exactly one press and one
  release; keyvo's numbers 1 to 4 (CID order `0x53`, `0x56`, `0x59`,
  `0x5a`) are the physical numbering (owner confirmed). A button held for
  3.1 s and 2.6 s sends no repeats. Two held: `button 2 press`, `button 1
  press`, `button 2 release`, `button 1 release`; `0x1B04` reports the
  whole held set, `00 56 00 53`, then `00 53`.
- Settles: spec §5.8, the button numbering. The statement that diverted
  presses repeat while held is corrected: on this firmware they do not.

#### D5. Rollers (2026-10-30)

- Method: `bin/keyvo --dump dial`, the dial and the small roller turned
  slowly in both directions and fast for one turn each.
- Result: roller 0 is the small roller (40 increments per rotation),
  roller 1 the dial (180). Sign: + is clockwise on the dial; on the small
  roller + is the direction the owner turned first as clockwise
  (forward). Reports arrive about every 22 ms (about 45 Hz) and carry a
  rising timestamp: the frames hold it as the static pass's `u32`
  big-endian in payload bytes 2 to 5 (`11 03 0d 00 01 01 00 05 5c a8`:
  roller 1, delta +1, timestamp `0x00055ca8`). Over the 32 reports of the
  slow clockwise dial block it rose by 902 while the dump's clock
  advanced 907 ms; the unit is not settled.

  | block | reports | sum of deltas | largest delta | duration |
  |---|---|---|---|---|
  | dial, slow, clockwise | 32 | +35 | 4 | 0.9 s |
  | dial, slow, counter-clockwise | 31 | -44 | 6 | 0.9 s |
  | roller, slow, forward | 13 | +20 | 3 | 0.5 s |
  | roller, slow, back | 13 | -16 | 2 | 0.7 s |
  | dial, fast, one turn | 36 | -169 (of 180) | 18 | 0.8 s |
  | roller, fast, one turn | 30 | -51 (of 40) | 2 | 0.7 s |

  The dial accelerates in firmware (deltas up to 18 fast, 4 to 6 slow);
  the roller does not (1 to 2 at any speed). After the fast dial turn
  three small opposite deltas (+3 in total) follow: the dial's
  spring-back. `receiver-input.txt` carries every report of the table,
  and its golden test sums them again.
- Settles: spec §5.8, the physical mapping of the roller ids, the sign
  per direction and the acceleration (dial only). Open: the timestamp
  unit, and whether the small roller's sign is the opposite one the
  third party reported (a wheel has no clockwise of its own).

#### D6. Diverts, sleep and wake (2026-10-30)

- Method: `bin/keyvo dial` and `--dump dial` with the Dialpad left alone
  for ten minutes, and started with the Dialpad in deep sleep (overnight);
  Ctrl-C to stop.
- Result, diverts: while `dial` runs nothing scrolls and the buttons do
  nothing in the focused window. After Ctrl-C (`diverts given back`:
  `0a 3e … 02` per CID, `0d 3e 00 00` and `0d 3e 01 00`) the roller
  scrolls again and the buttons act as before. The first restore request
  took 868 ms, because the Dialpad had dozed after 38 s idle; the others
  took 10 to 20 ms. `setMode` echoes the roller and the mode it set.
- Result, ten minutes idle is not deep sleep: `dial` running, the Dialpad
  untouched from 07:58:07 to 08:08:10, not a single frame in between, no
  arrival, no reconnect. The first roller report came at 08:08:10.2,
  under 1 s after the owner's turn at 08:08:10 (second resolution), and
  the diverts held: nothing to re-divert, no turn lost (66 reports, +171,
  one turn of the dial in 1.6 s).
- Result, deep sleep: the receiver answers connect fail at once (D2); the
  host cannot wake the Dialpad. `probe` reports `not found`; `dial` says
  `no Dialpad answers (...); waiting for it` and finds it after a touch.
  No `0x41` arrival appears. The Dialpad announces the wake on its own
  device index:

  ```
  32611 < 11 03 04 00 01 01 01   0x1D4B WIRELESS_DEVICE_STATUS (index 0x04): reconnect
  32626 < 11 03 09 00 5a 08      0x1004 UNIFIED_BATTERY (index 0x09): 90 %, full
  33397 > 11 03 00 0f 46 10      the next periodic search round
  33421 < 11 03 00 0f 0d 00 01   slot 3 answers in 24 ms
  34178                          both rollers diverted
  ```

  `wait_for_arrival` waits for `0x41` only, so the next search round
  finds the Dialpad, 0.8 s after the reconnect report here (1.6 s after
  the replug reconnect of D7); the diverts are in place 1.6 s after the
  first frame. The turn that wakes the Dialpad is lost.
- Settles: spec §5.8, the restore and the diverts in place while `dial`
  runs; deep sleep needs a much longer idle than ten minutes (overnight
  here). The static pass's `0x41` on wake is corrected: the receiver sent
  none, and the `0x1D4B` reconnect marks the wake. Accepting that report
  on a silent slot as an arrival is an open follow-up.

#### D7. Receiver unplug and replug (2026-10-30)

- Method: the receiver unplugged with `dial` running, plugged back, `dial`
  started again; and a replug with `dial` stopped.
- Result: with `dial` running, `Error: device disconnected`, exit code 1.
  Started again after the replug: `no Dialpad answers (... 3 silent ...);
  waiting for it`; after one turn the `0x1D4B` reconnect, the slot found
  1.6 s later, the diverts working (turns and presses arrived). The
  diverts do not stick: after a replug with `dial` stopped, once the
  Dialpad reconnected (a button press), the roller scrolled the terminal
  again; before that reconnect nothing scrolled only because the link was
  down.
- Settles: spec §5.8, the diverts are volatile and the disconnect ends
  `dial` as specified.

#### D8. The receiver's interfaces and what each node carries (2026-10-29 and 2026-10-30)

- Method: part 1, `udevadm` and `xxd` of every receiver node's report
  descriptor. Part 2, `od` read-only on all four nodes at once, `dial`
  stopped, then with only the Dialpad touched.
- Result, part 1 (the node numbers move between boots and replugs; in
  part 2 they were `hidraw0` to `hidraw3`, the HID++ node `hidraw2`):

  | node | bInterfaceNumber | descriptor |
  |---|---|---|
  | hidraw1 | 00 | keyboard (Generic Desktop, Usage 0x06) |
  | hidraw2 | 01 | mouse (Generic Desktop, Usage 0x02) |
  | hidraw3 | 02 | vendor 0xFF00, reports 10/11: HID++ |
  | hidraw4 | 03 | digitizer (Usage Page 0x0D) |

- Result, part 2:

  | node | bInterfaceNumber | carries |
  |---|---|---|
  | hidraw0 | 00 | keyboard: the MX Mechanical's 8-byte boot reports |
  | hidraw1 | 01 | mouse report `02`: the MX Master 4 and the undiverted Dialpad |
  | hidraw2 | 02 | HID++ `10`/`11`: nothing while undiverted; the Dialpad's `0x1B04` and `0x4610` reports while `dial` runs |
  | hidraw3 | 03 | nothing in these tests |

  The kernel does not run the Bolt receiver in DJ mode here: there are no
  per-device hidraw nodes, and every paired device shares the receiver's
  interfaces. Undiverted, the Dialpad's input is on the mouse node as
  report `02` (`02 <buttons 16> <x 16> <y 16> <wheel 8> <pan 8>`): the
  dial as AC Pan (byte 8; forward +2 to +12, back -1 to -6, accelerated),
  the roller as the wheel (byte 7, ±1 to 2), buttons 1 to 4 as the button
  bits `08`, `10`, `20`, `40`, mouse buttons 4 to 7.
- Settles: spec §5.1, the receiver's HID++ node is the one with reports
  `0x10` and `0x11` among four interface nodes; the statement that the
  kernel gives every paired device a node of its own is corrected. Spec
  §5.8, the Dialpad's undiverted input on the mouse node. The descriptors
  are the sysfs fixtures `receiver-only` (all four) and
  `keypad-and-receiver` (the HID++ node).
