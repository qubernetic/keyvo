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
on the host: from `bin/` after `just build`, and from `~/.local/bin` under
the systemd user unit once #184 is merged. The app comes from the Flatpak
bundle of the CI `flatpak` job, or from `just flatpak` on the host
(section 2).

**Flatpak id.** The app's application id is `io.github.qubernetic.keyvo`
(ADR-0005). The Flatpak carries the app and the `keyvo` CLI, not the
daemon. Under the Flatpak the app's window class is the same id.

**Terminals.** Keep three terminals open in the repository checkout
(called T1, T2 and T3 below): T1 for the daemon, T2 for `keyvo ctl` and
the CLI, T3 for the app.

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

2. **hidraw and uinput permissions.**

   ```bash
   just test-hw
   ```

   Expected: every `/dev/hidraw*` line reads `crw-rw-rw-` (mode `0666`),
   `/dev/uinput` is group `input`, and the `groups of <you>` line contains
   `input`. Then `test -w /dev/uinput && echo uinput ok` prints
   `uinput ok`; without it the chord keys fail and the daemon logs
   `cannot create the uinput keyboard: ...; chord keys will fail`. The
   udev rule and `keyvo doctor` of #184 replace this step when they land.

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

With the daemon running in T1 (`bin/keyvo-daemon`), in T3:

```bash
flatpak run io.github.qubernetic.keyvo
flatpak run --command=keyvo io.github.qubernetic.keyvo ctl status
```

Expected: the app opens on the Pad screen; the `ctl status` line has
`"connected":true`. Inside the Flatpak the CLI reaches only the daemon:
`keyvo probe`, `keys`, `fill` and `preview` run from `bin/keyvo` on the
host. #184 adds the install of the daemon under the user unit.

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

The driver and the `keyvo dial` command arrive with #186, the hardware
check with #187; their steps go here when they land.

## 5. Slice User test checklists

Every `## User test` item of the slice PRs merged into `epic/180-beta`,
PR number first, one block per slice. Items sections 1 to 4 already
covered say so; tick them from that result. Unless an item says
otherwise: Solaar stopped, the binaries from section 2, the profiles from
section 3, the daemon started with `bin/keyvo-daemon` in T1 when the item
needs it and stopped (Ctrl-C) when the item uses `keyvo fill`, `keys`,
`probe` or `dial`. Where an item's original text used its own test
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

## 6. What to post back

Every slice block of section 5: on the slice's issue, the commit under
test (section 1), the host (distribution, kernel, Plasma version), and
the PR number and item text of every unticked item, with what you saw.
Every failure or contradiction becomes a new slice on the beta epic, or a
`fix(...)` issue on `develop` when it is a finding of the pre-alpha
re-test (roadmap, Beta, Rules).
