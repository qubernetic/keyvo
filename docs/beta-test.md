# Beta test

The owner's sessions on the host for the beta epic (#180, roadmap
"Beta"): the preconditions, the build and install, the profiles, the
Dialpad over the Bolt receiver, and the User test checklists of every
slice merged into `epic/180-beta`. Every step names the command and what
it should show. Where a result differs, note the step number and what you
saw; section 6 says what goes back on the issues. The pre-alpha guide
(`docs/pre-alpha-test.md`) stays as the record of the pre-alpha and is not
extended.

**Binaries.** The daemon and the CLI are built in the devcontainer and run
on the host: the CLI from `bin/` after `just build`, the daemon from
`~/.local/bin` under the systemd user unit (section 2, #184). The app comes from the Flatpak
bundle of the CI `flatpak` job, or from `just flatpak` on the host
(section 2).

**Flatpak id.** The app's application id is `io.github.qubernetic.keyvo`
(ADR-0005). The Flatpak carries the app and the `keyvo` CLI, not the
daemon. Under the Flatpak the app's window class is the same id.

**Terminals.** Keep three terminals open in the repository checkout
(called T1, T2 and T3 below): T1 for the daemon's log (`journalctl
--user -u keyvo-daemon -f`) or, where an item says so, the daemon itself,
T2 for `keyvo ctl` and the CLI, T3 for the app.

## 1. Preconditions

Run these in T1 from the repository root, on the `epic/180-beta` branch:

```bash
git switch epic/180-beta && git pull
git rev-parse --short HEAD      # note it: the commit under test
```

1. **Solaar stopped.** It polls the same hidraw nodes and breaks the
   keep-alive measurements.

   ```bash
   flatpak kill io.github.pwr_solaar.solaar
   flatpak ps
   ```

   Expected: `flatpak ps` lists no `io.github.pwr_solaar.solaar`. Solaar
   must also have no divert set on the Keypad or the Dialpad (never set
   one there; it conflicts with keyvo's own `0x1B04` diverts).

2. **hidraw and uinput access: the udev rule.** Build first (section 2,
   `just build`), then check what is there, install the rule
   ([`contrib/udev/70-keyvo.rules`](../contrib/udev/70-keyvo.rules)),
   replug the Keypad and check again:

   ```bash
   just test-hw                     # runs bin/keyvo doctor
   sudo install -m 644 contrib/udev/70-keyvo.rules /etc/udev/rules.d/
   sudo udevadm control --reload && sudo udevadm trigger
   # unplug the Keypad and plug it in again
   just test-hw
   getfacl /dev/uinput
   ```

   Expected: after the install, the `keypad`, `uinput` and `rule` lines of
   `keyvo doctor` read `ok`, and `getfacl /dev/uinput` lists
   `user:<you>:rw-`. Before it, a machine with the pre-alpha's hand-made
   setup (mode `0666`, the `input` group) shows `keypad` and `uinput`
   `ok` and `rule` `warn`; a fresh one shows `fail` on both with the
   install lines. The rule needs no group and no mode: it tags the nodes
   `uaccess`, so the user at the seat gets an ACL. Without `/dev/uinput`
   access the chord keys fail and the daemon logs `cannot create the
   uinput keyboard: ...; chord keys will fail`. `keyvo doctor` exits 1
   while any line is `fail`; the other lines (session, focus stealing
   prevention, unit, daemon, Solaar, Flatpak) come back in section 2.

3. **`AGENT_HORDE` unset.** With `AGENT_HORDE=1` the justfile runs the
   toolchain natively and refuses `just run` and `just test-hw`.

   ```bash
   unset AGENT_HORDE
   echo "AGENT_HORDE=${AGENT_HORDE:-unset}"
   ```

   Expected: `AGENT_HORDE=unset`. If `just test-hw` above printed `not
   applicable in native mode`, it was set: unset it and run step 2 again.

4. **KDE Plasma 6 on Wayland**, `qdbus6` on the path (`qdbus6
   org.kde.KWin` lists `/Scripting`), and `notify-send` available
   (`command -v notify-send` prints a path). Fedora and Aurora ship the
   tool as `qdbus-qt6`, not `qdbus6`: use that name wherever `qdbus6`
   stands.

## 2. Build and install

The build runs in the devcontainer; the binaries run on the host.

```bash
just dev          # start the dev container (just build starts it too)
just build        # cargo build in the container
```

Expected:

- `just build` ends with `binaries in bin/: keyvo, keyvo-daemon (debug)`.
- `just run -- --version` and `bin/keyvo-daemon --version` each print one
  version line.

The app is the Flatpak `io.github.qubernetic.keyvo` (#182): the app and
the `keyvo` CLI on `org.gnome.Platform//51`, without the daemon. Take the
bundle from the CI run of the commit under test (the `keyvo.flatpak`
artifact of the `flatpak` job: Actions, the run, Artifacts; unzip it):

```bash
flatpak install --user keyvo.flatpak
flatpak list --app --columns=application,runtime | grep keyvo
```

Expected: `io.github.qubernetic.keyvo  org.gnome.Platform/x86_64/51`
(the install pulls the runtime from Flathub if it is missing). Or build
and install the same app from the checkout on the host (the first run
downloads the GNOME SDK, about 2 GB):

```bash
just flatpak
```

**The daemon under the systemd user unit** (#184). Install the binary
and the unit
([`contrib/systemd/keyvo-daemon.service`](../contrib/systemd/keyvo-daemon.service)),
start it, follow its log in T1:

```bash
install -Dm755 bin/keyvo-daemon ~/.local/bin/keyvo-daemon
install -Dm644 contrib/systemd/keyvo-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload && systemctl --user enable --now keyvo-daemon
systemctl --user status keyvo-daemon
journalctl --user -u keyvo-daemon -f      # T1
```

Expected: `status` reads `active (running)`, the pad paints, and the log
shows the KWin script loaded. The unit starts the daemon with the
graphical session, restarts it 2 s after a crash, and a stop ends the
daemon alone: applications it launched keep running (`KillMode=process`).
After every `just build` that changes the daemon, install the binary
again and `systemctl --user restart keyvo-daemon`. A user service does
not see the shell's `PATH`: a `launch` or `exec` command that names a
binary outside the systemd manager's `PATH` (`systemctl --user
show-environment`) needs its full path, or a `PATH=` line in a file under
`~/.config/environment.d/` and a new login. An item that runs
`bin/keyvo-daemon` in T1 itself stops the unit first (`systemctl --user
stop keyvo-daemon`) and starts it again afterwards; two daemons cannot
share the socket.

Then the whole check:

```bash
just test-hw
```

Expected: `keypad`, `uinput`, `rule`, `session`, `unit` and `daemon`
read `ok`; `focus` reads `ok` at KWin's default level (Low); `solaar`
reads `ok` with Solaar stopped (section 1, step 1); `flatpak` says
whether the app is installed (it is after the install above). The exit
status is 0 (`echo $?`).

With the daemon running, in T3:

```bash
flatpak run io.github.qubernetic.keyvo
flatpak run --command=keyvo io.github.qubernetic.keyvo ctl status
```

Expected: the app opens on the Pad screen; the `ctl status` line has
`"connected":true`. Inside the Flatpak the CLI reaches only the daemon:
`keyvo probe`, `keys`, `fill`, `preview` and `doctor` run from
`bin/keyvo` on the host.

## 3. Profiles

The example profiles live in [`contrib/profiles/`](../contrib/profiles/);
their keys are listed in `docs/pre-alpha-test.md` section 3. If
`~/.config/keyvo` exists already, move it aside first
(`mv ~/.config/keyvo ~/.config/keyvo.before-test`) and move it back after
the session.

```bash
mkdir -p ~/.config/keyvo/profiles
cp contrib/profiles/*.toml ~/.config/keyvo/profiles/
just run -- preview global -o /tmp/keyvo-global.png
```

Expected: the `preview` line prints `global page 1 of 3: 480x480, 9 keys
-> /tmp/keyvo-global.png` with nothing on stderr, and the PNG shows the
global profile's labels and icons. Slices that change the example
profiles (#183, #188, #189) say so in their section 5 block; copy the
files again after pulling them.

## 4. The Dialpad over Bolt

The Dialpad (WPID `BC00`) is paired with the Logi Bolt receiver
(`046d:c548`), next to the MX Mechanical and the MX Master 4. Find the
receiver's node (the node number can change):

```bash
grep -l 'HID_ID=0003:0000046D:0000C548' /sys/class/hidraw/*/device/uevent
```

Expected: one or more `/sys/class/hidraw/hidrawN/device/uevent` paths.
Do not use `solaar show` for the Dialpad: it crashes on it (upstream bug
in `settings_templates.py`, Solaar 1.1.20); the raw feature walk is in
`docs/research/hidpp-features-dump.txt`.

The driver and `keyvo dial` came with #186 (PR #199). The hardware check
of slice 7 (#187, part 1 on 2026-10-29, part 2 on 2026-10-30) ran steps 1
to 5 and 7 over Bolt; what it measured is in `docs/hardware-notes.md`,
section "Dialpad over Bolt" (records D1 to D8), and since #202 the
fixtures replay its frames. Step 6, Bluetooth, is not measured yet. Read
a step's expected lines against those records: ten minutes idle is not
deep sleep there (record D6), and a wake from deep sleep sends no
arrival, so a `dial` started while the Dialpad sleeps finds it in its
next search round.
Stop Solaar and the daemon for these steps (`flatpak kill
io.github.pwr_solaar.solaar`, `systemctl --user stop keyvo-daemon`); the
Dialpad must be awake (turn the dial once).

1. `bin/keyvo probe`: after the Keypad's lines, an empty line and the
   receiver block: `receiver` names the receiver's node (`046d:c548`,
   reports `10 11` among others), `slots` lists every slot asked up to the
   Dialpad's (`3 dialpad` on the owner's receiver, where Solaar shows it as
   device 3), `features` names `0x1b04` and `0x4610` with their indices
   (`0x0a` and `0x0d` in the feature walk), `firmware` the Dialpad's
   entities (Solaar shows `RBO 07.00.B0016`), `buttons` four CIDs `0x0053
   0x0056 0x0059 0x005a`, `rollers` two lines with the increments per
   rotation (40 and 180, measured in record D1).
2. `bin/keyvo dial`: stderr says `Dialpad on slot N: 4 buttons, ...;
   Ctrl-C to stop`. Each button prints `button K press` once while held
   and `button K release` on letting go; turning the dial and the roller
   prints `dial +N` / `roller -N` lines, one per notification, and the
   desktop does not scroll while they are diverted. Note which physical
   control is `dial` and which `roller`, the sign per direction, and
   whether fast turns give larger deltas.
3. Ctrl-C: stderr says `diverts given back`; the dial and the roller
   scroll the desktop again, the buttons do what they did before.
4. Let the Dialpad sleep (leave it alone until it goes dark) while `dial`
   runs: stderr says `Dialpad link lost`; touch it: `Dialpad back, diverts
   sent again`, and the events print again. Start `dial` with the Dialpad
   asleep: `no Dialpad answers (...); waiting for it` once, then the
   start line after it wakes.
5. `bin/keyvo --dump dial`: every report on stderr; a turn shows `< 11 0N
   0d 00 ...`, the HID++ 1.0 error replies of empty slots `< 10 0K 8f
   ...`.
6. Over Bluetooth (pair the Dialpad on its Bluetooth channel), with its
   node from `grep -l 'HID_ID=0005:0000046D:0000BC00'
   /sys/class/hidraw/*/device/uevent`: `bin/keyvo --device /dev/hidrawN
   probe` prints the `dialpad` block with `slots  direct dialpad`;
   `bin/keyvo --device /dev/hidrawN dial` works as in step 2.
7. From sleep (the fix of #200, PR #201): leave the Dialpad alone for ten
   minutes, then `bin/keyvo probe` over Bolt without touching it, three
   times. Each run prints the receiver block with `3 dialpad` in its
   `slots` line; the search waits up to `[timing].slot_probe` (1000 ms)
   per slot, so the block can take a few seconds. A `3 silent` instead
   means the receiver answered connect fail (`--dump` shows `< 10 03 8f
   00 0f 04`): post the `--dump probe` lines.

## 5. Slice User test checklists

Every `## User test` item of the slice PRs merged into `epic/180-beta`,
PR number first, one block per slice. Items sections 1 to 4 already
covered say so; tick them from that result. Unless an item says
otherwise: Solaar stopped, the binaries from section 2, the profiles from
section 3, the daemon running under the user unit (section 2) when the
item needs it and stopped (`systemctl --user stop keyvo-daemon`) when the
item uses `keyvo fill`, `keys`, `probe` or `dial`. An item that starts
`bin/keyvo-daemon` in T1 (the blocks before #196) stops the unit first
and starts it again afterwards. Where an item's original text used its own test
profile, it is rewritten here for the example profiles.

### #192: the Flatpak (#182)

- [ ] #192 Download `keyvo.flatpak` from the PR's `flatpak` job and `flatpak install --user keyvo.flatpak`: `flatpak list --app` shows `io.github.qubernetic.keyvo` on `org.gnome.Platform//51` (section 2).
- [ ] #192 With the daemon running (`bin/keyvo-daemon` in T1): `flatpak run io.github.qubernetic.keyvo` opens the app; the Pad shows the mirror and the real tiles; the Inspector edits a label and the pad follows; the colour chooser opens without the #169 abort.
- [ ] #192 `flatpak run --command=keyvo io.github.qubernetic.keyvo ctl status` prints the status line with `"connected":true`.
- [ ] #192 With the app focused, the pad keeps the previous window's profile (the id is in `[focus].hold_classes`); `bin/keyvo ctl status` shows `held: true`.
- [ ] #192 "Open in editor" on a profile opens the file in the desktop's editor through the portal.
- [ ] #192 `just flatpak` on the host builds and installs the same app from the checkout (the first run downloads the SDK, about 2 GB) and `flatpak run io.github.qubernetic.keyvo` works the same.
- [ ] #192 `flatpak info --show-permissions io.github.qubernetic.keyvo` lists only the six `finish-args`: `ipc`, `wayland`, `fallback-x11`, `dri`, `xdg-run/keyvo` and `xdg-config/keyvo:create`.

### #194: a launch keeps the global page (#179)

The owner's own profiles (the twelve `launch` keys of #155), with
`~/.config/keyvo/config.toml` absent (defaults: `launch_focus` 15 s,
`page_return_global` 10 s); the evidence is the log of
`bin/keyvo ctl subscribe` in T2. On a global page the view names the
global profile (spec §6.3), so a window that keeps the page brings a
`context` event and no `profile` or `page` event; the switch to the new
profile shows at the return.

- [ ] #194 Brave, left twice to `apps`, press KCalc: KCalc comes up and the pad stays on `apps` (the `apps` tiles and accent stay); 10 s after KCalc appeared the pad shows the `KCalc test` context page. The subscriber shows KCalc's `context` event with no `profile` or `page` event after it, then 10 s later the `profile` and `page` events of the `KCalc test` context page.
- [ ] #194 Brave, left twice to `apps`, press Spotify, then KCalc within 2 s: both come up, the pad stays on `apps` throughout; 10 s after the last window the context page of the profile in front.
- [ ] #194 Brave, left twice to `apps`, press Mailspring (a slow cold start), then click Konsole with the mouse before Mailspring is up: the pad shows Konsole's context page at once; when Mailspring comes up and takes the focus, the pad shows the global profile's context page (index 1, as the index was already 1).
- [ ] #194 KCalc already open, Brave, left twice to `apps`, press KCalc again: the launch exits at once (hand-over, the trace says `exited 0`), KCalc comes in front, the pad stays on `apps`; the `KCalc test` context page after 10 s.
- [ ] #194 The KCalc key with `wait_focus = false` in `global.toml`: KCalc comes up and the pad shows the `KCalc test` context page at once. Restore the key.
- [ ] #194 `config.toml` with `[timing] launch_focus = 0`: every launch key behaves as before this slice (the context page at once). Remove the file.
- [ ] #194 The app on the GNOME runtime: the Inspector's Launch editor shows `Window class` with its hint and `Wait for the window` switched on; set the class of the `KCalc test` key 6 (`NoApp`) to `org.does.not.Exist`: the file carries `class = "org.does.not.Exist"`, the daemon reloads without a warning, the tile stays error on a press as in #155. Switching `Wait for the window` off writes `wait_focus = false`; on again removes it.
- [ ] #194 `kreadconfig6 --file kwinrc --group Windows --key FocusStealingPreventionLevel` prints nothing (the default, Low), as `docs/hardware-notes.md` records.

### #195: a key's on_fail (#183)

This slice changes the example profiles: copy `contrib/profiles/global.toml`
again (section 3). The daemon runs from `bin/keyvo-daemon` in T1, `bin/keyvo
ctl subscribe '{"events":["trace"]}'` runs in T2. Key 2 of the global
profile's context page (`Fail`, `exit 1`) now carries `on_fail = { type =
"exec", cmd = "notify-send 'keyvo' 'key 2 failed'" }`.

- [ ] #195 Press key 2 on the global context page: the key shows the red x badge and the notification "keyvo / key 2 failed" appears; T2 shows two `trace` events for the press, the first `exited` with code 1 and no `branch`, the second with `"branch":"on_fail"` and `exited` code 0. The badge stays after the notification.
- [ ] #195 Change the `on_fail` command in `~/.config/keyvo/profiles/global.toml` to `keyvo-no-such-command`, save, press key 2: the badge shows again, the second trace record reads `exited` with code 127 (the shell's missing command), no notification, and no third record follows.
- [ ] #195 Lock the session (Super+L), press key 2, unlock: T2 shows one `blocked` record with `session locked`, no `on_fail` record, no notification.
- [ ] #195 Restore the `on_fail` command. In the app, select key 2 on the Pad: the Inspector shows the line "Runs `notify-send 'keyvo' 'key 2 failed'` when this fails. Edit it in the file." under the action. Copy the file (`cp ~/.config/keyvo/profiles/global.toml /tmp/global.before`), change the label to `Broken` in the Inspector, then `diff /tmp/global.before ~/.config/keyvo/profiles/global.toml`: only the `label` line differs.
- [ ] #195 In the app, the trace strip shows the press of key 2 as one line ending in `; on_fail: exec notify-send 'keyvo' 'key 2 failed' -> exited 0 after <n> ms`; opening it, the drawer's Result section has an `on_fail` row with the same text.

### #196: the user unit, the udev rule and keyvo doctor (#184)

The rule and the unit are installed as in sections 1 and 2. `bin/keyvo`
is the CLI of the commit under test (`just build`).

- [ ] #196 Remove the hand-made permissions for the test (or on a fresh boot): `bin/keyvo doctor` reports `fail` on the Keypad node and `/dev/uinput` with the install line; install the rule, replug the Keypad: both `ok`; `getfacl /dev/uinput` shows the user's ACL. (Section 1, step 2.)
- [ ] #196 Install the unit as in the guide: `systemctl --user status keyvo-daemon` is `active`; the pad paints within two seconds of login; `journalctl --user -u keyvo-daemon` shows the KWin script loaded. (Section 2.)
- [ ] #196 `systemctl --user stop keyvo-daemon` while a launched KCalc is open: the pad goes to the logo, KCalc stays. `start` again: the pad paints.
- [ ] #196 Kill the daemon (`kill -9 $(systemctl --user show -p MainPID --value keyvo-daemon)`): the unit restarts it within about 2 s (`journalctl --user -u keyvo-daemon` shows the restart, the pad paints again).
- [ ] #196 `bin/keyvo doctor` with Solaar running (`flatpak run io.github.pwr_solaar.solaar --window=hide &`) warns on `solaar`; with the session level set to 2 (`kwriteconfig6 --file kwinrc --group Windows --key FocusStealingPreventionLevel 2`) it warns on `focus`; then reset (`kwriteconfig6 --file kwinrc --group Windows --key FocusStealingPreventionLevel --delete`) and stop Solaar (`flatpak kill io.github.pwr_solaar.solaar`). Both warnings leave the exit status 0.
- [ ] #196 Lock and unlock the session with the daemon under the unit: the lock page shows and goes (logind over the system bus works from a user service).
- [ ] #196 The receiver's rule reaches its own node only: find the receiver's node and a paired device's child node (`for n in /dev/hidraw*; do echo "$n $(udevadm info -q property -n "$n" | grep '^HID_ID=')"; done` shows the receiver's own nodes with `HID_ID` ending in `0000C548` (from its parent), the keyboard's with its wireless product id), then `udevadm info -a -n <node>` on both and `getfacl` on both: the receiver's node lists `user:<you>`, the keyboard's child node does not. Note what `getfacl` shows in the PR.
- [ ] #196 `bin/keyvo doctor --json`: nine lines, each one JSON object with `check`, `status`, `message` and `fix`.

### #199: the Dialpad driver over Bolt and keyvo dial (#186)

The hardware check of this driver is #187 (slice 7); these items are its
starting point, and its results over Bolt are in `docs/hardware-notes.md`,
"Dialpad over Bolt". Solaar and the daemon stopped.

- [ ] #199 `bin/keyvo probe` prints the receiver block with the Dialpad's slot, the `0x1b04` and `0x4610` indices, the firmware, four buttons and two rollers (section 4, step 1). Post the block.
- [ ] #199 `bin/keyvo dial` and `bin/keyvo dial --json` print one line per button edge and turn, a held button once (section 4, step 2); the desktop does not scroll while diverted. Post which control is `dial` and which `roller`, and the sign per direction.
- [ ] #199 Ctrl-C gives the diverts back: scrolling and the buttons' own actions return (section 4, step 3).
- [ ] #199 Sleep and wake while `dial` runs, and a start while asleep (section 4, step 4).
- [ ] #199 `bin/keyvo --dump dial` with a few turns and presses: post the `--dump` lines of the start (slot search and diverts), one turn per roller, one button press and the Ctrl-C; they replace the synthetic fixtures.
- [ ] #199 Over Bluetooth with `--device`: `probe` and `dial` (section 4, step 6).

### #205: the receiver's rule on the HID++ node only (#197)

The rule now tags only the receiver's HID++ node, USB interface `02`
(`docs/hardware-notes.md`, record D8). Install the rule of this PR as in
section 1, step 2 (until then `keyvo doctor` reads `rule` `warn`), then
replug the receiver. List the receiver's nodes with their interface:

```bash
for n in $(grep -l 'HID_ID=0003:0000046D:0000C548' /sys/class/hidraw/*/device/uevent); do
  d=/dev/$(basename "$(dirname "$(dirname "$n")")")
  echo "$d $(udevadm info -a -n "$d" | grep -m1 'bInterfaceNumber')"
done
```

- [ ] #205 `getfacl` on each `046d:c548` node of that list: only the node with `bInterfaceNumber` `02` lists `user:<you>`; the nodes of `00`, `01` and `03` do not. Post the four `getfacl` outputs.
- [ ] #205 `bin/keyvo dial` runs as the user (no root) and prints events on a turn and a press (section 4, step 2).

## 6. What to post back

Every slice block of section 5: on the slice's issue, the commit under
test (section 1), the host (distribution, kernel, Plasma version), and
the PR number and item text of every unticked item, with what you saw.
Every failure or contradiction becomes a new slice on the beta epic, or a
`fix(...)` issue on `develop` when it is a finding of the pre-alpha
re-test (roadmap, Beta, Rules).

### #201: a sleeping Dialpad over Bolt in the slot search (#200)

The fix of the cold-run defects of #187, part 1. Solaar and the daemon
stopped, `bin/keyvo` from `just build` of this PR.

- [ ] #201 With the Dialpad asleep (left alone for ten minutes) over Bolt, `bin/keyvo probe` three times: each run prints the receiver block with `3 dialpad` in its `slots` line (section 4, step 7). The remaining #199 items follow on this build.
