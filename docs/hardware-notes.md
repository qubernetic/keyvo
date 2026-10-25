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

## First-light checklist (pre-alpha slice 6)

The owner runs these on the host with the binary from `just build`. Before
the first step: stop Solaar (`flatpak kill io.github.pwr_solaar.solaar`),
make sure no Solaar divert is set on the Keypad, and keep the raw output of
every command. Items refer to list A of the project plan §5; items 9
(fps), 10 (JPEG limits) and 11 (brightness) are deferred past the
pre-alpha. Slice 6b turns the output into records in the format above.

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
5. **Display mode.** `just run -- probe` right after plugging in (mode at
   boot, expected `a1`), right after a `fill --hold 10` (expected `a0`), and
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
    to be sent again (it is, on every start). Note the raw event bytes if
    a debug dump is available.
13. **Own-write ACK against key 1.** Start `just run -- keys` in one
    terminal and run `just run -- fill '#ffffff' --key 1` in another: `keys`
    must print nothing for the paint. Then press key 1: one press, one
    release.

## Measurements

None yet. Slice 6b (keyvo#41) adds the first-light records.
