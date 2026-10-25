# Pre-alpha home test

One session on the host with the Keypad attached: the visible key window
measurement (#70), the end-to-end check (#52), and the User test
checklists of every slice merged into `epic/34-pre-alpha`. Every step
names the command and what it should show. Where a result differs, note
the step number and what you saw; section 7 says what goes back on the
issues.

Plan about two hours. Keep three terminals open in the repository
checkout (called T1, T2 and T3 below) and a phone for the photographs.

## 1. Preconditions

Run these in T1 from the repository root, on the `epic/34-pre-alpha`
branch:

```bash
git switch epic/34-pre-alpha && git pull
git rev-parse --short HEAD      # note it: the commit under test
```

1. **Solaar stopped.** It polls the same hidraw nodes and breaks the
   keep-alive measurements.

   ```bash
   flatpak kill io.github.pwr_solaar.solaar
   flatpak ps
   ```

   Expected: `flatpak ps` lists no `io.github.pwr_solaar.solaar`. Solaar
   must also have no divert set on the Keypad (never set one there; it
   conflicts with keyvo's own `0x1B04` divert).

2. **hidraw and uinput permissions.**

   ```bash
   just test-hw
   ```

   Expected: every `/dev/hidraw*` line reads `crw-rw-rw-` (mode `0666`),
   `/dev/uinput` is group `input`, and the `groups of <you>` line contains
   `input`. Then `test -w /dev/uinput && echo uinput ok` prints
   `uinput ok`; without it the chord keys fail and the daemon logs
   `cannot create the uinput keyboard: ...; chord keys will fail`.

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
   (`command -v notify-send` prints a path).

## 2. Build

The build runs in the devcontainer; the binaries run on the host.

```bash
just dev          # start the dev container (just build starts it too)
just build        # cargo build in the container
just app-build    # the Svelte frontend and the Tauri shell
```

Expected:

- `just build` ends with `binaries in bin/: keyvo, keyvo-daemon (debug)`.
- `just app-build` ends with `binary in bin/: keyvo-app (debug)`.
- `bin/` holds `keyvo`, `keyvo-daemon` and `keyvo-app`.
  `just run -- --version` and `bin/keyvo-daemon --version` each print one
  version line.

If `just app-build` fails on missing webkit2gtk or Tauri packages, the
image predates #66: run `just rebuild`, then `just dev`, then the two
builds again. If `bin/keyvo-app` later fails to start on a missing
`libwebkit2gtk-4.1` library, note it for #52 (the host lacks the runtime)
and skip the GUI steps.

## 3. Install the example profiles

The profiles live in [`contrib/profiles/`](../contrib/profiles/):

| File | Matches | Accent | Keys (context page) |
|---|---|---|---|
| `global.toml` | no app profile matches | blue `#4e8bd6` | 1 Notify (exec, `notify-send`), 2 Fail (exec, `exit 1`), 3 Type (chord `shift+k, e, y`), 4 Slow (exec, `sleep 15`), 5 OK (exec, `true`); page `general` key 1 General, page `third` key 1 Third |
| `konsole.toml` | `org.kde.konsole`, `konsole` | green `#3fae5a` | 1 New tab (chord `ctrl+shift+t`), 2 Find (chord `ctrl+shift+f`), 4 OK, 5 Fail, 6 Slow |
| `vscode.toml` | `code`, `com.visualstudio.code` | orange `#e0a83b` | 1 Palette (chord `ctrl+shift+p`), 2 Save (chord `ctrl+s`), 7 OK, 8 Fail, 9 Slow |

If `~/.config/keyvo` exists already, move it aside first
(`mv ~/.config/keyvo ~/.config/keyvo.before-test`) and move it back after
the session.

```bash
mkdir -p ~/.config/keyvo/profiles
cp contrib/profiles/*.toml ~/.config/keyvo/profiles/
just run -- preview global -o /tmp/keyvo-global.png
just run -- preview konsole -o /tmp/keyvo-konsole.png
just run -- preview vscode -o /tmp/keyvo-vscode.png
```

Expected: the three `preview` lines print `global page 1 of 3: 480x480, 9
keys -> /tmp/keyvo-global.png`, `konsole page 1 of 1: ...` and `vscode
page 1 of 1: ...`, with nothing on stderr. Open the PNGs: the labels and
icons of the table above, an accent bar in the profile's colour under
every key. The same files serve as test images in section 6.

## 4. Visible key window (#70)

The daemon must not run during this section; only `keyvo fill` drives
the pad. Photograph the pad straight on, from above, in even light, the
whole panel in the frame. Name the photos `70-<step>.jpg`.

First note the firmware: `just run -- probe` and copy the `firmware` line
marked `application, active` (first light: `U1 66.00.B0017`).

1. **White panel, black keys.** One command line, so the second claim
   follows the first before the keep-alive expires:

   ```bash
   just run -- fill '#ffffff' --panel && just run -- fill '#000000' --all --hold 30
   ```

   Expected: both print one line per write with `acked` equal to
   `fragments`; the panel turns white, then the nine keys black. During
   the 30 s hold, photograph (`70-1.jpg`). On first light a white strip
   stayed at the bottom and the right edge of each key.

2. **Black panel, white keys.**

   ```bash
   just run -- fill '#000000' --panel && just run -- fill '#ffffff' --all --hold 30
   ```

   Photograph during the hold (`70-2.jpg`).

3. **Keys on the logo, without a panel paint.** Unplug the Keypad, wait
   5 s, plug it back in, wait until the logo shows, then:

   ```bash
   just run -- probe | grep display     # expected: mode a2
   just run -- fill '#ffffff' --all --hold 30
   ```

   Photograph during the hold (`70-3.jpg`). Note what fills the strip:
   the logo, the previous image, or nothing visible.

4. **Per key, from the photos.** For each key 1 to 9 note: the edges on
   which a strip shows (top, bottom, left, right), and its estimated width
   in pixels. The key is 118 px wide, so a strip a tenth of the key wide
   is about 12 px. Note whether all nine keys behave the same.

A table for the notes:

| Key | Edges with a strip | Width (px) | Same in 70-1, 70-2, 70-3? |
|---|---|---|---|
| 1 | | | |
| 2 | | | |
| 3 | | | |
| 4 | | | |
| 5 | | | |
| 6 | | | |
| 7 | | | |
| 8 | | | |
| 9 | | | |

## 5. End to end (#52)

The daemon runs in T1 for the whole section; leave its log visible. T2
is for `keyvo ctl`, T3 for the app.

1. **Daemon start.** In T1:

   ```bash
   bin/keyvo-daemon
   ```

   Expected log lines, within about a second: `keyvo-daemon <version>
   with /home/<you>/.config/keyvo`, `control socket at
   /run/user/<uid>/keyvo/keyvo.sock`, and `KWin script loaded as script
   <n>; focus switching on`. No `WARN` line: in particular no
   `configuration not loaded`, no `cannot create the uinput keyboard` and
   no `keypad: cannot open the keypad`.

2. **KWin script loaded.** In T2:

   ```bash
   qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.isScriptLoaded keyvo-focus
   stat -c %a "$XDG_RUNTIME_DIR/keyvo" "$XDG_RUNTIME_DIR/keyvo/keyvo.sock"
   ```

   Expected: `true`, then `700` and `600`.

3. **Global profile painted.** Focus a window no profile matches (for
   example Dolphin). Expected: the pad shows Notify, Fail, Type, Slow and
   OK on keys 1 to 5 with a blue accent bar, keys 6 to 9 blank with the
   blue bar; it matches `/tmp/keyvo-global.png`.

4. **Konsole switch.** Focus a Konsole window. Expected: within a
   fraction of a second the pad shows the Konsole profile (green bars, New
   tab, Find, OK, Fail, Slow), and T1 logs `showing profile konsole page
   1 in auto mode`.

5. **Editor switch.** Focus VS Code. Expected: orange bars, Palette,
   Save, and OK, Fail, Slow on keys 7 to 9; the log names `vscode`. Focus
   Dolphin again: back to the blue global profile.

6. **Chord key.** Focus Konsole and press key 1 (New tab): a new tab
   opens. Focus a text editor no profile matches (Kate) and press key 3
   (Type): `Key` is typed.

7. **Exec keys and their looks.** On the global profile (Kate or Dolphin
   focused):

   - Key 5 (OK): a green check badge for about 2 s, then the plain key.
   - Key 2 (Fail): a red x badge that stays. Press it again: the badge
     goes while the command runs and comes back.
   - Key 4 (Slow): the icon dims and an arc appears (running). After 10 s
     an orange refresh badge appears (long run), after 15 s the green
     check (success).
   - Key 1 (Notify), short: a notification `key 1 short` and the green
     check. Hold it for over 1 s: the notification reads `key 1 long`
     before you let go.

   Repeat key 6 (Slow) and key 5 (Fail) on the Konsole profile: the same
   looks with the green accent.

8. **Status and subscribe.** In T2:

   ```bash
   just run -- ctl status
   just run -- ctl subscribe
   ```

   Expected: `status` prints one JSON line with `"connected":true,"keys":9`
   and the shown profile, page and mode. `subscribe` prints its response
   line and then keeps running: press a pad key (a `key` event and a
   `trace` event), switch windows (`context` and `profile` events), save a
   profile file unchanged with `touch ~/.config/keyvo/profiles/konsole.toml`
   (a `config` event). Ctrl-C ends it. Leave it running for steps 9 to 11.

9. **GUI mirror.** In T3:

   ```bash
   bin/keyvo-app
   ```

   Expected: a window titled `keyvo` with nine tiles matching the pad, a
   status strip with `Keypad connected`, the profile, page, mode and the
   focused window. The app window itself matches no profile, so the
   global profile shows. Switch the page with a page button: the grid and
   the strip follow.

10. **Dry-run press and its trace entry.** In the app, click key 2
    (Fail). Expected: the result line reads `Key 2, simulated: <action>,
    dry run, nothing executed`, a line appears at the top of Recent
    events, the pad's key 2 does not change, and the `subscribe` in T2
    prints a `trace` event whose `trigger` has `"simulated":true` and
    whose `result` is `{"status":"dry_run"}`. The same from the CLI:

    ```bash
    just run -- ctl press '{"key":4,"dry_run":true}'
    ```

    prints one entry with `"simulated":true` and `"status":"dry_run"`, and
    key 4 does not start running.

11. **Key edit in the GUI repaints the pad.** Select key 5 on the grid
    (click it or press `5`), change its label to `Done` and press Enter.
    Expected: within about a second the pad's key 5 and the mirror show
    `Done`, T2 prints a `config` event, and

    ```bash
    diff contrib/profiles/global.toml ~/.config/keyvo/profiles/global.toml
    ```

    shows only the `label` line of key 5, comments and alignment intact.
    Restore it: `cp contrib/profiles/global.toml ~/.config/keyvo/profiles/`.

Stop the app and the daemon (Ctrl-C) before section 6; it restarts them
as each item needs.

## 6. Slice User test checklists

Every `## User test` item of the slice PRs merged into
`epic/34-pre-alpha`, PR number first. Items section 4 or 5 already
covered say so; tick them from that result. Unless an item says
otherwise: Solaar stopped, the binaries from section 2, the profiles from
section 3, the daemon started with `bin/keyvo-daemon` in T1 when the item
needs it and stopped (Ctrl-C) when the item uses `keyvo fill`, `keys` or
`probe`. Where an item's original text used its own test profile, it is
rewritten here for the example profiles; the key numbers are the same.

Items 54 to 58 were first run on first light (#40, records in
[hardware-notes.md](hardware-notes.md), "Measurements"); run them again
and note any difference from the record named.

### Process (#53)

- [ ] #53 Read `CONTRIBUTING.md` "Sandbox-complete and hardware-bound issues" and "Branch types", `docs/roadmap.md` (intro, S definition, Pre-alpha), `docs/plans/2026-10-24-gui-ux-design.md` 4.1 rule 3 and `CLAUDE.md` step 2 and Git Rules: all five say the same about the Proof block location and the epic branch.

### HID++ and VLP framing (#54)

- [ ] #54 `just run -- probe` reports `hid++` `4.5` from the classic ROOT ping.
- [ ] #54 Probe resolves `0x0008 KEEP_ALIVE` through `ROOT.getFeature` (index 0x04 on the author's unit) and prints the timeout range `500..10000 ms` (record 4).
- [ ] #54 Probe resolves VLP `getFeature(0x19A1)` to VLP index 2; hidden, version and maxMemory are printed (capture: 0, 0, 0x0001e803).
- [ ] #54 Probe resolves `0x1B04` to index 0x0b on the Keypad (`just run -- probe --feature 0x1b04`); nothing is hard-coded, so another index is no failure.
- [ ] #54 `just run -- probe --feature 0x0020` reports the feature absent, not an error.
- [ ] #54 Provoke one VLP error reply and post the raw bytes on #71: `just run -- --dump probe --feature 0x1234` (see #74 below).

### Claim, geometry and keep-alive (#55)

- [ ] #55 `just run -- probe` prints the VLP protocol capabilities (capture: major 1, minor 0, available memory 0x0001e820).
- [ ] #55 Probe prints the supported display modes (capture: `a0 a1 a2 a4 a5 a6 d0..d6`) and the mode before the claim (`a2` after plugging in, record 5).
- [ ] #55 Probe prints the geometry: panel 480x480 and nine 118x118 keys; compare the nine origins with spec §5.4 and record 6. Post the raw fn1 reply (`just run -- --dump probe`) if any value differs.
- [ ] #55 After a claim the display reports mode `a0`: `just run -- fill '#00ff00' --panel --hold 5`, then at once `just run -- probe | grep display` reads `mode a0`. Whether the mode-change event arrives: right after a replug (mode `a2`), `just run -- --dump fill '#00ff00' --panel 2>&1 | grep '13 ff 02 10'` shows the event `13 ff 02 10 c<n> a0` (record 5).
- [ ] #55 With the keep-alive running, a paint stays for at least 60 s (`just run -- fill '#00ff00' --panel --hold 60`); after the command ends the panel returns to the logo (about 3 s, record 3).
- [ ] #55 Keep-alive replies echo the requested 3000 ms and the pings go out about once per second: `just run -- --dump fill '#00ff00' --panel --hold 5 2>&1 | grep '11 ff 04'`; `just run -- keys` in a second terminal still receives presses.
- [ ] #55 The claim works without VLP root fn2, which keyvo does not send (record 5: every paint appeared).

### VLP image writer (#56)

- [ ] #56 `just run -- fill '#ff0000' --panel` paints the whole panel red in one write (435x434 at 23,6); no key stays on its old image.
- [ ] #56 `just run -- fill '#0000ff' --key 5` paints only the centre key; the other eight keep their image.
- [ ] #56 Repeat both with `--index-base 1`, with `--swid 0xb`, and with both; note which combinations paint (record 7: all four).
- [ ] #56 Every `fill` line shows `acked` equal to `fragments`.
- [ ] #56 Press key 1 right after a paint (`just run -- keys` running in T2): exactly one press, never a phantom press from the write's ACK (record 13).
- [ ] #56 A JPEG whose size differs from the target is refused before anything is written. `fill` scales every PNG to its target, so no CLI command can send a wrong size; this is covered by the writer's unit tests. Tick it as "not reachable from the CLI".

### Key input and page-button divert (#57)

- [ ] #57 After `just run -- keys` starts, both page buttons are diverted; `just run -- --dump keys` shows the `setCidReporting` and `getCidReporting` replies for `0x01A1` and `0x01A2` with the divert bit (record 12).
- [ ] #57 Pressing and releasing each LCD key 1 to 9 gives exactly one press and one release for that key, numbered as in the geometry.
- [ ] #57 Holding two keys and releasing them in either order gives press, press, release, release with the right keys; no report repeats while held (record 13).
- [ ] #57 Each page button gives one press and one release and switches nothing on the device itself; holding both gives both presses.
- [ ] #57 A panel paint (`just run -- fill '#ffffff' --panel` in T3 while `keys` runs in T2), with no key touched, gives no key event.
- [ ] #57 Divert with settle 0 and spacing 0: there is no CLI flag for it (record 12). Tick it as "not reachable from the CLI".
- [ ] #57 Unplug and replug, start `keys` again: the divert is sent again and both page buttons report.

### Probe, keys and fill (#58)

- [ ] #58 `just run -- probe` finds the Keypad's node without `--device`: the `device` line names a `/dev/hidraw*` node with report `14`; `hid++` is `4.5`; `firmware` lists `U1 66.00.B0017` as `application, active`; the keep-alive range is `500..10000 ms`; `display` is `mode a2` after plugging in (record 5; the PR said `a1`); `geometry` shows 480x480 with nine 118x118 keys.
- [ ] #58 `just run -- probe --device /dev/hidrawN`, with the node from the `device` line, gives the same output.
- [ ] #58 `just run -- keys` prints one press and one release per LCD key 1 to 9 and one each per page button (`page left`, `page right`); Ctrl-C ends it. `just run -- keys --json` prints the same events as one JSON object per line.
- [ ] #58 `just run -- fill '#ff0000' --key 1` paints key 1 red; `just run -- fill '#0000ff' --panel` paints the whole panel; `just run -- fill /tmp/keyvo-global.png` paints every key. Each line shows `acked` equal to `fragments`.
- [ ] #58 `just run -- fill '#00ff00' --all --hold 10` keeps the paint for the whole hold; note the time until the logo comes back after the command ends.
- [ ] #58 With `just run -- keys` in T2, `just run -- fill '#ffffff' --key 1` in T3 prints no event in T2.
- [ ] #58 Unplugging the Keypad while `keys` runs ends it with `device disconnected`.

### Profile model and TOML loading (#59)

- [ ] #59 `just test profile config layout schema` passes.
- [ ] #59 Open `crates/keyvo-core/tests/fixtures/profiles/valid/vscode.toml` in VS Code with Even Better TOML after adding a first line `#:schema ../../../../../../schemas/profile.schema.json`: hovering a key shows its description; `color = "blue"` or a `colour = ...` line under a key is flagged. Undo the change afterwards (`git checkout -- crates/keyvo-core/tests/fixtures/profiles/valid/vscode.toml`).
- [ ] #59 `schemas/config.schema.json` shows the defaults as millisecond integers (`"default": 150` for `focus_hysteresis`).

### Key renderer and preview (#60)

- [ ] #60 `just run -- preview crates/keyvo-core/tests/fixtures/profiles/valid/demo.toml -o grid.png` prints `demo page 1 of 2: 480x480, 9 keys -> grid.png` and on stderr `key 8: no bundled icon `lucide:no-such-icon`; drawn as lucide:circle-question-mark`. `grid.png` shows nine dark rounded keys with a blue bar, a green and a yellow key (black ink on the yellow one), "Run every int..." cut with an ellipsis, a question-mark icon on key 8.
- [ ] #60 The same with `--page 2` shows only key 5 (Settings); `--page 3` fails with `profile `demo` has no page 3; it has 2`.
- [ ] #60 `just run -- preview global -o grid.png` renders the installed global profile (section 3); `just run -- preview nosuch -o grid.png` lists the profiles found.
- [ ] #60 On the pad: note on #43 any colour fringing on label edges (section 5 step 3).

### Chord parser, press resolution and feedback looks (#61)

- [ ] #61 Open the five `crates/keyvo-core/tests/snapshots/render__looks__*.snap.png` files. Running: a grey hammer with a blue arc from 12 to 3 o'clock. Success: a green check badge at the top right. Error: a red x badge. Long run: an orange refresh badge. Override: a dark red key with a bug icon, "Failed" and a red `!` badge.
- [ ] #61 In a copy of `crates/keyvo-core/tests/fixtures/profiles/valid/demo.toml` (`cp` it to `/tmp/demo.toml`), change key 2's chord to `ctrl+nope`. `just run -- preview /tmp/demo.toml -o x.png` exits 1 with ``Error: /tmp/demo.toml:23:35: invalid chord: unknown key `nope` ``.

### Daemon on the active profile (#62)

The original test profile had the same keys 1 to 4 as the example
`global.toml`.

- [ ] #62 `bin/keyvo-daemon` (Dolphin focused): within about a second keys 1 to 5 show their icons and labels with the blue accent, keys 6 to 9 are blank. No warning in the log (section 5 steps 1 and 3).
- [ ] #62 Key 1 short: a notification `key 1 short` and a green check badge for about 2 s. Held over 1 s: `key 1 long` before you let go (section 5 step 7).
- [ ] #62 Key 2: a red x badge that stays. Again: the badge goes and comes back (section 5 step 7).
- [ ] #62 Kate focused, key 3: `Key` is typed (section 5 step 6).
- [ ] #62 Key 4: the icon dims and an arc appears; after 10 s an orange badge, after 15 s a green check (section 5 step 7).
- [ ] #62 Change key 1's `label` in `~/.config/keyvo/profiles/global.toml` and save: only key 1 changes, within a fraction of a second.
- [ ] #62 Set `label = 7` on key 1 and save: the log shows `global.toml:<line>:<column>` with the error and the pad does not change. Undo and save: the pad follows again.
- [ ] #62 Leave the pad alone for a minute: it stays painted and presses still work (keep-alive).
- [ ] #62 Unplug the Keypad: the log says the device is gone. Plug it back in: within about 2 s the keys are painted again and presses work.
- [ ] #62 Ctrl-C in T1: the daemon exits at once.

### Focus context and page buttons (#63)

The global profile has the pages context, general and third; key 1 shows
Notify, General and Third on them. Keep a window no profile matches
focused (Dolphin). The daemon now has a focus source (#64), so the
original first item's `focus switching off` warning does not appear.

- [ ] #63 `bin/keyvo-daemon`: key 1 shows Notify.
- [ ] #63 Tap either page button: key 1 shows General and the log says `showing profile global page 2 in auto mode`. Tap again: back to Notify.
- [ ] #63 Hold a page button for over 1 s: before you let go, the log says `... in manual mode` and the pad does not change.
- [ ] #63 In manual mode, tap right repeatedly: Notify, General, Third, Notify. Tap left: it pages backwards.
- [ ] #63 Hold both page buttons together: key 1 shows Notify and the log says `showing profile global page 1 in auto mode`. Releasing them does nothing more.
- [ ] #63 Tap the right button within 1 s of letting go of a long press: a short tap still pages.
- [ ] #63 Ctrl-C: the daemon exits at once.

### KWin focus watcher (#64)

- [ ] #64 `bin/keyvo-daemon` logs `KWin script loaded as script <n>; focus switching on` (section 5 step 1).
- [ ] #64 `qdbus6 ... isScriptLoaded keyvo-focus` prints `true`; `stat -c %a "$XDG_RUNTIME_DIR/keyvo"` prints `700` (section 5 step 2). `head -20 "$XDG_RUNTIME_DIR/keyvo/keyvo-focus.js"` shows the header, and the `service` line holds the daemon's unique name (`:1.<n>`), not `@SERVICE@`.
- [ ] #64 Focusing Konsole switches to the Konsole profile, focusing another window goes back to global, one info log line per switch (section 5 steps 4 and 5).
- [ ] #64 Title rule: create `~/.config/keyvo/profiles/titletest.toml` with the content below, then run `printf '\e]0;keyvo-test\a'` in a focused Konsole. The pad switches to the red `Title` profile without a focus change. Delete the file afterwards.

  ```toml
  schema_version = 1
  [profile]
  id     = "titletest"
  name   = "Title"
  match  = { title = "keyvo-test" }
  accent = "#d64e4e"
  priority = 10
  [[pages]]
  [pages.keys.5]
  label = "Title"
  ```

- [ ] #64 `kwin_wayland --replace &` restarts KWin: the daemon logs `KWin is gone; focus switching waits for it`, then `KWin script loaded ...` again, and focus switching still works. If the session ends instead, note it on #52.
- [ ] #64 Ctrl-C and start the daemon again: it loads the script once, and one focus change gives one switch.
- [ ] #64 `DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent bin/keyvo-daemon` logs `focus switching off: D-Bus: ...`; the pad still paints and the page buttons still page.

### Socket API and keyvo ctl (#65)

- [ ] #65 `stat -c %a "$XDG_RUNTIME_DIR/keyvo/keyvo.sock" "$XDG_RUNTIME_DIR/keyvo"` prints `600` and `700`; the log shows `control socket at ...` (section 5 steps 1 and 2).
- [ ] #65 `just run -- ctl status` prints one line with `"connected":true,"keys":9` and the shown profile (section 5 step 8).
- [ ] #65 `just run -- ctl press '{"key":4,"dry_run":true}'` prints an entry with `"simulated":true` and `"status":"dry_run"`; the command does not run and the pad does not change (section 5 step 10).
- [ ] #65 `just run -- ctl press '{"key":5}'` runs the command: the key shows success. `'{"key":2}'`: error.
- [ ] #65 `just run -- ctl subscribe`: `key` and `trace` events on a press, `context` and `profile` on a window switch, `config` on a saved profile; Ctrl-C ends it (section 5 step 8).
- [ ] #65 `just run -- ctl notify_context '{"class":"org.kde.konsole"}'` switches to the Konsole profile without a focus change.
- [ ] #65 A second `bin/keyvo-daemon` in T3 refuses to start with `another keyvo daemon serves ...`. After the first one stops (Ctrl-C), `ls "$XDG_RUNTIME_DIR/keyvo/keyvo.sock"` finds no file.

### Tauri and Svelte scaffold (#66)

- [ ] #66 `just app-build` ends with `binary in bin/: keyvo-app (debug)` (section 2).
- [ ] #66 With the daemon running, `bin/keyvo-app` opens a window titled `keyvo` showing `Keypad connected` and `profile global, page 1, auto`.
- [ ] #66 With the daemon stopped, a fresh `bin/keyvo-app` shows `Daemon unreachable: cannot reach the keyvo daemon at .../keyvo.sock: ...`.
- [ ] #66 Tab moves focus visibly (accent outline) and the window follows the dark token defaults.

### Home mirror and virtual pad (#67)

- [ ] #67 Nine tiles matching the pad, the status strip with device, profile, page, mode and focused window (section 5 step 9).
- [ ] #67 The page buttons switch the page; the grid and the strip follow (section 5 step 9).
- [ ] #67 Clicking a bound key reads `Key N, simulated: <action>, dry run, nothing executed`, adds a Recent events line, runs nothing (section 5 step 10).
- [ ] #67 Right-click a key: the result says `long`.
- [ ] #67 Press `5`, then `Space`: key 5 is focused and dry-run pressed.
- [ ] #67 Press a key on the device: its tile is outlined while held, and the press shows in Recent events.
- [ ] #67 Stop the daemon: the alert reads `Daemon unreachable: the daemon closed the connection` and the last frame stays. Start it and click Retry: the mirror is live again.

### Key editor (#68)

- [ ] #68 The editor beside the mirror shows key 1 of the active profile and the path of its file.
- [ ] #68 Select key 2, change its label, press Enter: within about a second the pad and the mirror show it; `diff contrib/profiles/global.toml ~/.config/keyvo/profiles/global.toml` (or against the newest file in `~/.local/state/keyvo/snapshots/`) shows only that line, comments and alignment intact (section 5 step 11 does the same on key 5).
- [ ] #68 Set the color to `#aa3300`, then switch Action to exec and enter `notify-send hi`: pressing key 2 on the pad sends the notification.
- [ ] #68 Click outside the fields and press `Ctrl+Z` twice: the action and the color come back, and the file matches its version from before those two edits.
- [ ] #68 Break the profile by hand (`label = 7`): after the reload the editor shows `.../profiles/global.toml:<line>:<column>: ...` and the pad keeps the last good state.

After this block: `cp contrib/profiles/*.toml ~/.config/keyvo/profiles/`.

### Display reclaim after a keep-alive expiry (#73)

- [ ] #73 `just run -- fill '#00ff00' --panel --hold 20 --keep-alive-period 3000 --keep-alive-timeout 3000` is refused before it touches the device: nothing is painted, and the error reads `--keep-alive-period (3000 ms) must be shorter than --keep-alive-timeout (3000 ms); ...`.
- [ ] #73 `printf '[timing]\nkeep_alive_period = 3000\n' > ~/.config/keyvo/config.toml`, then `bin/keyvo-daemon`: it refuses the configuration, logging `configuration not loaded:` and `.../config.toml:2:21: `keep_alive_period` (3000 ms) must be shorter than `keep_alive_timeout` (3000 ms); ...`. Then Ctrl-C and `rm ~/.config/keyvo/config.toml`.
- [ ] #73 With the daemon painting the global profile, `kill -STOP $(pgrep -x keyvo-daemon)`, wait 5 s, `kill -CONT $(pgrep -x keyvo-daemon)`: the logo appears while it is stopped, the keys come back within one keep-alive period after `CONT`, and the log shows one `claim lost, display reclaimed`.
- [ ] #73 Daemon stopped: `just run -- fill '#00ff00' --panel --hold 30`, and the same `kill -STOP`/`kill -CONT` on `$(pgrep -x keyvo)`: after `CONT`, `display reclaimed after a lost claim; painting again` is printed once and the green panel is back within one period.

### Raw frame dump and feature probe (#74)

- [ ] #74 `just run -- --dump probe`: stdout is the usual report; stderr has one line per frame (`<ms> > 11 ff ...`, `<ms> < 11 ff ...`, then `13 ff` frames for the VLP part), with rising milliseconds.
- [ ] #74 `just run -- --dump probe --feature 0x1234`: the `classic` and `vlp` lines each read `absent` or `error: ...`, and the dump shows the two requests and replies. Paste the `vlp` reply bytes into #71.
- [ ] #74 `just run -- probe --feature 0x1b04` prints `classic 0x1b04 index 0x0b version 6`.
- [ ] #74 `just run -- keys --json --dump 2>frames.txt | jq .` while pressing key 5: `jq` prints `{"edge":"press","key":5}` and the release, no parse error; `frames.txt` has the `13 ff 02 00 ..` key reports. Delete `frames.txt` afterwards.
- [ ] #74 `just run -- --dump fill '#ff0000' --key 1`: each image fragment prints one line `> 14 ff 02 <fn|swid> <seq> .. (4094 bytes)` followed by its `< 13 ff 02 ..` ACK.

### First-light records (#75)

- [ ] #75 Read `docs/hardware-notes.md`, "Measurements", against the first-light report on #40 and its logs: every number and log citation matches.
- [ ] #75 The spec §5.4 key origin table matches the `geometry` lines of `just run -- probe`.

## 7. What to post back

Clean up first: `rm -f grid.png x.png /tmp/demo.toml
/tmp/keyvo-*.png`, and move a saved `~/.config/keyvo.before-test` back if
you made one.

**On #70**, one comment:

```markdown
## Visible key window (<YYYY-MM-DD>)

- Device: MX Creative Keypad 046d:c354, firmware <application, active line of probe>
- Commit: <git rev-parse --short HEAD from section 1>
- Method: docs/pre-alpha-test.md section 4, steps 1 to 3; Solaar stopped
- Photos: 70-1.jpg (white panel, black keys), 70-2.jpg (black panel, white keys), 70-3.jpg (keys on the logo)
- Step 3: the strip shows <the logo / the previous image / nothing visible>
- Per key: <the table from section 4>
- All nine keys the same: <yes / no, which differ>
```

**On #52**, one comment:

```markdown
## Pre-alpha end to end (<YYYY-MM-DD>)

- Device: MX Creative Keypad 046d:c354, firmware <application, active line of probe>
- Commit: <git rev-parse --short HEAD>
- Host: <distribution, kernel (uname -r), Plasma version, Wayland or X11>
- Method: docs/pre-alpha-test.md sections 5 and 6
- Section 5: step 1 <pass / fail: what you saw> ... step 11 <...>
- Section 6: <PR number and item text of every unticked item, with what you saw>
- Logs: <the daemon log of section 5, attached>
```

Every failure or contradiction becomes a new slice on the pre-alpha epic.
