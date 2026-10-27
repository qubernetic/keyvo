# Pre-alpha home test

One session on the host with the Keypad attached: the no-strip check (#70,
once #72 is merged), the end-to-end check (#52), and the User test
checklists of every slice merged into `epic/34-pre-alpha`. Every step
names the command and what it should show. Where a result differs, note
the step number and what you saw; section 7 says what goes back on the
issues.

Plan about two hours. Keep three terminals open in the repository
checkout (called T1, T2 and T3 below).

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
   (`command -v notify-send` prints a path). Fedora and Aurora ship the
   tool as `qdbus-qt6`, not `qdbus6`: use that name here and in section 5
   and the #64 checklist wherever `qdbus6` stands.

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

On an atomic host (Aurora, Fedora Kinoite) that does not layer
`webkit2gtk`, the app runs inside the installed `org.gnome.Platform//51`
Flatpak runtime instead, until the Flatpak manifest of M4b-1 exists (the
owner's comment on #52 of 2026-10-25; the runtime's glibc covers the
binary's). Wherever the guide says `bin/keyvo-app`, run, from the
repository root:

```bash
flatpak run --filesystem=host --filesystem=xdg-run/keyvo \
  --socket=wayland --socket=fallback-x11 --share=ipc --device=dri \
  --command="$PWD/bin/keyvo-app" org.gnome.Platform//51
```

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

## 4. No strip beside the keys (#70, after #72)

Run this section on a build with slice 6l (composite paint, #72). Before
it the daemon painted the panel background and then the nine keys as
separate images, and the strip this section looks for was expected.

On first light a key painted over a panel paint left a strip of the
panel colour at its bottom and right edge (`docs/hardware-notes.md` item
7). Options+ never shows it, because it paints the background and all
keys as one composite image and a single key as a crop of the same
canvas (`docs/research/options-plus-display-pipeline.md`, "What changes
the plan" item 1); #72 makes keyvo paint the same way, which makes a
per-key margin unnecessary. Magenta and green instead of white and black,
so a one-pixel strip stands out (the owner's method on #52). Photos do
not settle it: the domed key caps act as a lens, and a strip appears or
disappears with the viewing angle, so look at the pad from several angles
(straight on, from each side, from above and below).

First note the firmware: `just run -- probe` and copy the `firmware` line
marked `application, active` (first light: `U1 66.00.B0017`).

1. **Control: separate images.** The daemon is stopped. One command line,
   so the second claim follows the first before the keep-alive expires:

   ```bash
   just run -- fill '#ff00ff' --panel && just run -- fill '#00ff00' --all --hold 20
   ```

   Expected: both commands print one line per write with `acked` equal to
   `fragments`; the keys are green with a magenta strip at their bottom
   and right edge, as on first light. This shows the strip is visible
   from where you look.

2. **Composite paint on a magenta leftover.** With the magenta panel and
   green keys of step 1 still on the pad, start `bin/keyvo-daemon` in T1
   with a window focused that no profile matches (Dolphin), and leave it
   running for 20 s.

   Expected: the global profile appears in one update, not key by key;
   no magenta and no green anywhere on or beside a key from any angle,
   only the profile's dark blue background around and between the keys.

3. **One key, then a profile switch.** With the daemon running, press
   key 5 (OK) and focus a Konsole window, then Dolphin again. Expected:
   the pressed key and each profile switch leave no strip of the earlier
   colour beside any key. Stop the daemon with Ctrl-C.

4. **Post** the result on #70 (section 7): no photos.

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

   On Fedora and Aurora the first command is `qdbus-qt6` instead of
   `qdbus6`. Expected: `true`, then `700` and `600`.

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
   `trace` event), switch windows (`context` and `profile` events), write
   a profile file again unchanged with
   `cp contrib/profiles/konsole.toml ~/.config/keyvo/profiles/` in T3 (a
   `config` event; a bare `touch` changes no content and is not seen). Ctrl-C ends it. Leave it running for steps 9 to 13.

9. **GUI mirror.** In T3:

   ```bash
   bin/keyvo-app
   ```

   Expected: a window titled `keyvo`, 1200 x 760, with a header (`keyvo`,
   a device selector showing `Keypad`, a `...` menu button), the
   navigation rail on the left with Pad selected, and the Pad beside it:
   the Keypad drawing with nine tiles matching the pad in its cutouts,
   under it the page tabs of the profile (`context`, `general`, `third`
   on the global profile, the pad's page selected) and `Accent` with a
   blue swatch and its `#rrggbb`. The app window itself is held
   (`[focus].hold_classes`, #121), so the profile of the window before it
   stays and a line reads `keyvo-app focused, holding <profile>`. Switch
   the page with a page button: the drawing and the selected tab follow.

10. **Dry-run press and its trace entry.** In the app, click key 2
    (Fail): it gets the accent outline and nothing is pressed. Press
    `Space`. Expected: the result line reads `Key 2, simulated, global
    layer: <action>, dry run, nothing executed` followed by `Show the
    trace entry`, a line appears at the top of Recent events, the pad's
    key 2 does not change, and the `subscribe` in T2 prints a `trace`
    event whose `trigger` has `"simulated":true` and whose `result` is
    `{"status":"dry_run"}`. The same from the CLI:

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
    Then pick `rocket` in the inspector's Icon field and `#336699` in its
    Color swatch: the pad's key 5 repaints with the rocket on that colour,
    the inspector's header reads `Key 5: Exec` with the file and line, and
    the diff shows the `icon = "lucide:rocket"` and `color` lines added.
    Restore it: `cp contrib/profiles/global.toml ~/.config/keyvo/profiles/`.

12. **Shell and readable dropdowns.** Still in the app:

    - The rail lists Pad, Profiles, Behaviors. `Ctrl+2` shows Profiles
      (step 14), `Ctrl+1` brings the Pad back with the mirror as it was.
      Behaviors is greyed: hovering it shows `Coming soon`; `Ctrl+3` or
      Tab onto it shows `Coming soon` and the sentence naming M4a-5, and
      the screen does not change. `Ctrl+4` does nothing.
    - The header's Profile dropdown lists Global first, then the
      profiles by priority, the one on the pad as `<name>, on the pad`.
      `Sandbox` is a greyed switch whose focus names M4a-4. The gear
      opens Preferences on the right with the tabs General, Devices,
      Integrations, Projects, each one sentence naming its roadmap item;
      the arrows move between the tabs, `Esc` closes it and the gear has
      the focus again. The `...` menu's `Keyboard shortcuts` opens the
      keyboard map in the same place (one drawer at a time); `Esc`
      closes it.
    - Narrow the window: below 1200 px the Pad's Actions column
      collapses into the inspector's Action select, below 1100 px the
      rail shows icons only, below 1000 px the Profiles list moves above
      the editor. The window does not shrink below 960 x 640.
    - Open the device selector in the header and the Action selector of
      the inspector, and the `...` menu: every option and entry is
      readable (light text on a dark background). The menu's entries
      but `Keyboard shortcuts` are greyed with `Coming soon`. Switch the desktop between
      the light and the dark theme (System Settings, Colors and Themes,
      Global Theme) and open them again: still readable in both.
    - Unplug the Keypad: the device selector shows `Simulator` only.
      Plug it back: `Keypad` returns and is selected.

13. **Pad states and the trace drawer.** Still in the app, on the Pad:

    - Click `Show the trace entry` on the result line of step 10. A
      drawer opens on the right with Trigger (`simulated`), Context
      snapshot, Resolved action and Block reason `Dry run: resolved,
      nothing executed.`; `Esc` closes it. Press key 5 (OK) on the pad,
      then click its line in Recent events: the drawer shows Result with
      `exited`, `Exit code 0` and the duration. The close button closes
      it too.
    - In the drawer's header, `Debug log` is a greyed switch with
      `Coming soon`; clicking it changes nothing.
    - With the global profile on the pad (it has three pages), select
      key 5, then click the `general` tab: the key editor edits key 5 of
      page 2 while the drawing stays on the pad's page 1, and a line
      says `Editing Global, key 5; the pad shows global, page 1.` with
      `Follow`; key 5 of the drawing has no accent outline. Click
      `Follow`: the line goes and the editor is on key 5 of page 1.
    - Break a profile: put `label = 7` on a key of
      `~/.config/keyvo/profiles/global.toml`. An error notice shows
      `Schema error in <path of global.toml>, line <n>, column <n>: ...`.
      Restore the file
      (`cp contrib/profiles/global.toml ~/.config/keyvo/profiles/`): the
      error goes.
    - Unplug the Keypad: a note says `The Keypad is missing; last seen at
      <time>.` and the mirror keeps its tiles. Plug it back: the note
      goes.
    - Stop the daemon (Ctrl-C in T1): an error notice says `Daemon
      unreachable` with the socket path and `Start it with
      keyvo-daemon.`, the mirror keeps its tiles. Start it again and
      click Retry: the replayed entries return.

14. **Profiles.** Focus Konsole, then press `Ctrl+2` in the app:

    - The list shows Global first, then Konsole and VS Code (all three
      have priority 0, so by id), each name followed by a small square
      of its accent; Konsole says `on the pad` and is selected. Focus
      another window and come back: the mark follows the pad.
    - The heading row shows `Konsole` and `Open in editor`, the file
      `~/.config/keyvo/profiles/konsole.toml` under it, once. The form
      shows the name, the accent `#3fae5a` beside its swatch, priority
      `0` with `higher wins the window`, and Enabled reading `On` with
      the sentence that switching a profile off arrives with M4a-4,
      each label above its field. Change the name to `Terminal` and press Enter.
      Expected: the heading and the list show `Terminal` at once, and

      ```bash
      diff contrib/profiles/konsole.toml ~/.config/keyvo/profiles/konsole.toml
      ```

      shows only the `name` line, alignment intact. Change the accent to
      `#d6884e` and press Enter: the pad's accent bar turns orange within
      about a second and the diff shows the `accent` line too, its
      `# green` comment kept. Type `teal` as the accent: the line under
      the field says it is not `#rrggbb` and nothing is written. Restore
      the file:
      `cp contrib/profiles/konsole.toml ~/.config/keyvo/profiles/`.
    - `Open in editor` opens `konsole.toml` in the desktop's application
      for TOML files.
    - Match rules show `Window class` once with `org.kde.konsole` and
      `konsole` under it; `+ rule` and each row's `Edit` are greyed with
      `Coming soon`. Select Global in the list: it says the global
      profile is under every window.
    - On Global the Pages are three thumbnails, `context`, `general` and
      `third`, each the page's tiles as the pad would show them. Click
      `general`, then `Edit keys on the Pad`: the Pad opens with the
      Profile dropdown on Global and the `general` tab selected, and the
      line starts `Editing Global` (with the key selected earlier, if any)
      and ends `the pad shows konsole, page 1.`; `Follow` returns to it.
    - `+` above the list opens Empty profile, Copy of the active profile
      and Starter pack, all greyed: hovering shows `Coming soon`, Tab
      onto one adds the sentence with its roadmap item, and a click does
      nothing.
    - Unplug the Keypad: the thumbnails keep their names and a note says
      the pictures appear once a Keypad has reported its geometry. Plug
      it back.

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
- [ ] #64 `qdbus6 ... isScriptLoaded keyvo-focus` (`qdbus-qt6` on Fedora and Aurora) prints `true`; `stat -c %a "$XDG_RUNTIME_DIR/keyvo"` prints `700` (section 5 step 2). `head -20 "$XDG_RUNTIME_DIR/keyvo/keyvo-focus.js"` shows the header, and the `service` line holds the daemon's unique name (`:1.<n>`), not `@SERVICE@`.
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

### Panel background before the keys (#79)

Since #72 (#93) the background and the keys come as one image, so where
an item below says "first, then the keys", expect both at once; the
no-strip expectations stay.

- [ ] #79 Daemon stopped: `just run -- fill '#ffffff' --panel`, then `bin/keyvo-daemon` in T1 with Dolphin focused: the panel turns dark blue first, then the nine global keys paint on it. No white strip beside any key; around and between the keys only the dark blue profile background.
- [ ] #79 With the daemon running, focus Konsole: the background turns dark green, then the Konsole keys; VS Code: dark orange, then its keys; Dolphin again: dark blue. No strip of the previous profile's background beside a key (section 5 steps 3 to 5).
- [ ] #79 With the daemon painting the global profile, `kill -STOP $(pgrep -x keyvo-daemon)`, wait 5 s, `kill -CONT $(pgrep -x keyvo-daemon)`: after one `claim lost, display reclaimed` the dark blue background comes back first, then the keys; no logo strip beside a key (together with #73 item 3).
- [ ] #79 `just run -- preview global -o /tmp/keyvo-global.png` shows the same dark blue between the keys as the pad.

### Display fields in probe and fill --full (#91)

- [ ] #91 `just run -- probe`: under `display`, after the mode line, read `deviceScreenCount 1`, `maxImageSize 124928 bytes`, `maxImageFPS 30` and `deferrableDisplayUpdate 1  rgb565 1  rgb888 0  jpeg 1  calibrated 1  origin 0` (the captured values, spec §5.10); the `geometry` line reads `480x480, 9 buttons, 0 visible areas`. Post any value that differs on #89.
- [ ] #91 Daemon stopped: `just run -- fill '#ff00ff' --full` prints one line `full  480x480 at 0,0  ... acked` and the whole panel turns magenta, the border outside the keys included; `just run -- fill '#ff00ff' --panel` for comparison prints `panel  435x434 at 23,6`.
- [ ] #91 `just run -- fill '#ff00ff' --full --panel` (and `--full --key 1`, `--full --all`) refuses with `cannot be used with` and exit code 2, before it touches the device.

### Composite paint (#93)

- [ ] #93 The #52 video method (240 fps, section 5): start `bin/keyvo-daemon` with Dolphin focused, then focus Konsole and Dolphin again. Each profile switch is one wipe of the whole key area; no key appears after another and no background shows before its keys.
- [ ] #93 Section 4 steps 1 to 3 with the magenta panel and green keys: after step 2 and after every key press and profile switch of step 3, no magenta, green or earlier background strip beside any key, from any angle.
- [ ] #93 With the daemon painting the global profile, press key 2 (exits 1): its running, then error look changes only key 2, and the keys around it do not flicker.
- [ ] #93 `kill -STOP $(pgrep -x keyvo-daemon)`, wait 5 s, `kill -CONT $(pgrep -x keyvo-daemon)`: after one `claim lost, display reclaimed` the whole page comes back in one update, no logo strip beside a key.
- [ ] #93 The daemon's output in T1 has no `display capabilities:` warning (the daemon read `maxImageSize` from fn0) and no `panel: not painted` line during the steps above.

### Global key fallback and reported geometry (#99)

- [ ] #99 With the example profiles of section 3 (`konsole.toml` has no key 3), start `bin/keyvo-daemon` and focus Konsole: key 3 on the pad shows the global "Type" key with the green Konsole accent, and pressing it types "Key" into Konsole. The daemon's `press:` log line has `layer: Some(Global)` and `profile: "konsole"`.
- [ ] #99 `bin/keyvo-app` with Konsole focused: key 3 of the mirror shows the same tile with a "global" mark, its screen-reader name ends in "global layer", and a click on it adds an event line `Key 3, simulated, global layer: chord ...`. Keys 1 and 2 have no mark.
- [ ] #99 The mirror's nine keys have the size and the 3x3 layout of the Keypad without any key count or size in the app; unplug the Keypad: the mirror keeps its layout.
- [ ] #99 `bin/keyvo-daemon --config-dir /tmp/kv-other` (a copy of `~/.config/keyvo` with one label changed): `keyvo ctl status` shows `config_dir` `/tmp/kv-other`, and the app's mirror and key editor show and edit that copy, not `~/.config/keyvo`.
- [ ] #99 `KEYVO_SOCKET=/tmp/kv.sock bin/keyvo-daemon`, then `KEYVO_SOCKET=/tmp/kv.sock keyvo ctl status` answers; a key with `cmd = "keyvo ctl status > /tmp/kv-status.json"` writes the status (the child got `KEYVO_SOCKET`).

### App shell with rail, header and tokens (#100)

- [ ] #100 In the light and in the dark desktop theme, open every dropdown of the app (the header's device selector, the key editor's Action selector, the `...` menu) and read every option: no white text on a white background, no dark text on a dark background.
- [ ] #100 Section 5 step 12: rail order and `Ctrl+1` to `Ctrl+7`, the five "Coming soon" screens with their roadmap sentence on keyboard focus, the column collapse order when narrowing the window, the 960 x 640 minimum.
- [ ] #100 With the desktop's reduced animations on (System Settings, Animation speed at Instant), nothing in the app animates.

### Home with status strip and trace drawer (#101)

- [ ] #101 Section 5 step 9: the page selector and the accent bar above the mirror, the status strip with `Overlay none` and `unknown project`.
- [ ] #101 Section 5 step 13: the drawer from the result line and from an event line, `Esc` closes it; Debug log greyed with `Coming soon`; the profile link opens Profiles; a picked page that the pad does not show; the schema error with file and line; the missing Keypad with its last-seen time; the skeleton rows after Retry.
- [ ] #101 The look of Home in the light and the dark desktop theme: every text readable, the links visible as links.

### Profiles screen with tabs and key card (#102)

- [ ] #102 Section 5 step 14: list order with Global pinned and Konsole marked active, name and accent written with only their line changed, Open in editor, the tabs, a key into the key editor, the key card's layer origin, the greyed "Coming soon" controls.
- [ ] #102 The look of Profiles in the light and the dark desktop theme: every text readable, the selected profile, tab and key visible.

### Socket peer check in a sandbox (#104)

Both items run by the owner on 88bb0b1 (2026-10-26).

- [x] #104 Daemon on the host (`bin/keyvo-daemon`): `bin/keyvo ctl status` inside `flatpak run --filesystem=host --filesystem=xdg-run/keyvo --command=sh org.gnome.Platform//51` prints the status reply against the host daemon, not `socket: Unknown error -1000`.
- [x] #104 Daemon on the host, app started with the GNOME runtime wrapper (`~/keyvo-prealpha-2026-10-25/keyvo-app-flatpak.sh`): Home shows the device connected, the mirror and the key editor.

### Mock dev server and themes (#113)

No Keypad and no daemon needed; a host browser.

- [x] #113 `just app-dev`, open `http://localhost:1420`: Home shows the mirror with tiles and a trace entry arrives every two seconds; the profile switches every ten seconds (Konsole, VS Code, Global).
- [x] #113 `?theme=breeze-light` renders light, `?theme=adwaita-dark` dark with the GNOME accent.
- [x] #113 Profiles lists the three profiles; editing the Global accent changes the preview; a reload resets it.
- [x] #113 `?mock-error=unreachable` shows the Home error state.

### Plug-in splash in the display claim (#114)

- [ ] #114 Daemon running and painting the global profile (`bin/keyvo-daemon`, `bin/keyvo ctl subscribe` in a second terminal): unplug the Keypad, wait 5 s, plug it back in. The keys come back; the daemon log shows the `hid i/o error` and `cannot open the keypad` lines of the unplug and no `display stayed in mode` line (the splash wait is logged at `debug`, which the daemon does not print yet); `ctl subscribe` shows the `device` event with the error of the unplug and no `device` event with an error after the plug-in.
- [ ] #114 Daemon stopped, right after a replug: `just run -- fill '#00ff00' --panel --hold 5` paints green instead of failing with `display stayed in mode 0xa4 after setMode(a0)`.
- [ ] #114 `printf '[timing]\nclaim_splash_retry = 2000\nclaim_splash_timeout = 2000\n' > ~/.config/keyvo/config.toml`, then `bin/keyvo-daemon`: it logs `configuration not loaded:` and `.../config.toml:2:22: `claim_splash_retry` (2000 ms) must be shorter than `claim_splash_timeout` (2000 ms); ...`. Then Ctrl-C and `rm ~/.config/keyvo/config.toml`.

### Held window classes (#121)

The daemon with the profiles from section 3 and `bin/keyvo-app` running (section 2: the GNOME runtime wrapper where `bin/keyvo-app` alone fails); no `~/.config/keyvo/config.toml`, so `[focus].hold_classes` is `["keyvo-app"]`.

- [ ] #121 Focus Konsole: the pad shows the Konsole profile. Click the keyvo window: the pad stays on Konsole, Home's Context fact reads "keyvo-app focused, holding org.kde.konsole", and `bin/keyvo ctl subscribe` shows the `context` event with `"held":true` and no `profile`, `page` or `mode` event after it.
- [ ] #121 Focus VS Code: the pad switches to the VS Code profile after the hysteresis as before.
- [ ] #121 `printf '[focus]\nhold_classes = []\n' > ~/.config/keyvo/config.toml`, restart the daemon: clicking the keyvo window switches the pad to the global profile as before #121. Then `rm ~/.config/keyvo/config.toml`.

### Design critique (#106)

No Keypad needed; the mock dev server of #113 in a host browser.

- [x] #106 Read `docs/design/critique-2026-10.md`: the score table, the six priority issues and the priorities for slices 4 to 6 match what the Profiles and Home screens show on `just app-dev`; the priorities are approved in the PR.
- [x] #106 With the browser detector injected as section 2 of the critique describes, the `[Human]` overlay marks the trace rows (cramped padding) on Home in `breeze-dark` and the muted labels (low contrast) on Profiles in `?theme=breeze-light`.
- [x] #106 On Profiles with no profile picked in the list, wait for the mock's focus cycle (10 s): the editor switches profile on its own; this is the P1 finding the pass fixes in #108.

### Pad screen brief (#107)

No Keypad needed.

- [ ] #107 Read `docs/plans/2026-10-26-pad-screen-brief.md` and `docs/design/gui-ux.md` §2 to §3.2: the eleven decisions of the 2026-10-26 grill are recorded as agreed; the brief is approved in the PR.

### The keypad drawing (#124)

No Keypad and no daemon needed; the mock dev server of #113 in a host browser.

- [ ] #124 `just app-dev`, open `http://localhost:1420` with `?theme=breeze-dark`, `?theme=breeze-light`, `?theme=adwaita-dark` and `?theme=adwaita-light` in turn: Home shows the Keypad drawing with the mock's nine tiles in its key cutouts, scaled with the drawing, in all four themes.
- [ ] #124 In the devtools, set `--kv-device: #b9bcc0` (the pale grey edition) on `:root`: the body and the key and page-button rims recolour to pale grey.

### Recipe packs and the recipes request (#131)

The daemon running; no app needed. The drag-and-drop user test of #119 runs in #130 once #129 is merged.

- [ ] #131 `just run -- ctl recipes` prints one line with the fourteen bundled packs (`"source":"bundled"`), General, System and Apps in that order, and no `problems`.
- [ ] #131 `mkdir -p ~/.config/keyvo/recipes && cp contrib/recipes/spectacle.toml ~/.config/keyvo/recipes/` and change its `label` to `Shot`: `just run -- ctl recipes` lists `spectacle` once, with `"source":"user"` and the label `Shot`. Write `x = 1` into a second file `~/.config/keyvo/recipes/bad.toml`: the answer carries a `problems` line naming `bad.toml:1:1`. Remove both files afterwards.
- [ ] #131 Copy the `exec` of `contrib/recipes/volume-up.toml` (variant `plasma`) and run it in a terminal: the volume rises and the Plasma OSD shows. The same for `brightness-up.toml` and `lock.toml` (variant `screensaver`); note on #119 any command that fails on Plasma 6.

### The shell of three screens (#132)

No Keypad needed; the mock dev server of #113 in a host browser, then the app. Run by the owner in #130.

- [ ] #132 Section 5 step 12: the rail of Pad, Profiles, Behaviors on `Ctrl+1` to `Ctrl+3`; the gear opens Preferences with its four tabs and `Esc` gives the focus back to the gear; `Keyboard shortcuts` in the `...` menu opens the keyboard map, one drawer at a time.
- [ ] #132 `just app-dev` with `?theme=breeze-light` and `?theme=breeze-dark`: open the Profile dropdown; Global first, the pad's profile marked `on the pad`, every option readable; pick VS Code: the dropdown keeps it while the mock's focus cycle moves the pad, and the `on the pad` mark moves.
- [ ] #132 `?mock-error=unreachable`: the notice has a full 1 px border in the error colour and starts with `Error`; no coloured stripe on its left.

### The pad screen with the mirror and the page tabs (#133)

No Keypad needed for the first three; the mock dev server of #113 in a host browser, then the app on the GNOME runtime. Run by the owner in #130.

- [ ] #133 `just app-dev` at 1363x798 and at 960 px wide, with `?theme=breeze-light` and `?theme=breeze-dark`: the Keypad drawing scales with the window, never above its 1:1 size and never clipped; the tiles sit in the cutouts; the page tabs and the accent swatch are readable; no element under a scroll bar.
- [ ] #133 Wait for the mock's focus cycle to reach `keyvo-app` (after Konsole): a line reads `keyvo-app focused, holding konsole` and the drawing stays on Konsole; the next step (VS Code) removes the line.
- [ ] #133 Click a tile (it gets the accent outline, nothing is pressed), then wait for the focus cycle to move the pad: the key editor stays on the key, the drawing follows the pad, and `Editing <pinned profile>, key <n>; the pad shows <profile>, page <n>.` appears with `Follow`; `Follow` removes it and the editor shows the same key number on the pad's profile. `?mock-profiles=empty` and `Empty` in the header's dropdown: no tabs, only the greyed `+`.
- [ ] #133 With the daemon and the Keypad: section 5 steps 9, 10 and 13.

### The pinned inspector with the icon picker (#134)

No Keypad needed for the first three; the mock dev server of #113 in a host browser, then the app. Run by the owner in #130.

- [ ] #134 `just app-dev`, click key 1 on Konsole: the inspector reads `Key 1: Chord` with `konsole.toml line <n>` under it and the tile beside Label, Icon and Color. Change the label, type `rock` in Icon and pick `rocket` with the arrows and Enter, pick a colour in the swatch: each change repaints the tile, the swatch and its `#rrggbb` text agree, and the result line reads `Wrote key 1 to ... at <hh:mm:ss>`. `Ctrl+Z` three times outside a field takes the three changes back.
- [ ] #134 Click a dimmed key with the `global` mark on Konsole: the inspector says `from global` with the global file and line; `Override here` writes the global key into Konsole's key (the mark goes), `Clear` empties it again (the global key shows through), and both undo.
- [ ] #134 `?mock-error=write`: change a label; an error notice shows the file, line and column of the key and the message, the typed label stays in the field, and Undo stays disabled. Type `teal` into Color: the message stands under the field and nothing is written.
- [ ] #134 With the daemon and the Keypad: section 5 step 11, the icon and colour included: the file round-trips (only the edited lines change).

### The actions catalogue column and the trace strip (#135)

The mock dev server of #113 in a host browser for the first three, then the app. Run by the owner in #130.

- [ ] #135 `just app-dev` in a window at least 1200 px wide: the Actions column at the right lists General, System and Apps; a heading collapses and expands its group. Type `ko` into Search actions: only Konsole is left; `zzz` says `No action matches`. Press `/` with the focus outside a field: the search takes it. Focus Spectacle: `Coming soon` with the #119 sentence, and a click does nothing.
- [ ] #135 Drag Exec onto key 7: key 7 gets the outline, the inspector reads `Key 7` with Exec in the Action select and an empty Command, and nothing is written yet; type a command and leave the field: the result line says it was written. Select key 2 and click Chord in the column: the same with an empty Chord field.
- [ ] #135 Narrow the window below 1200 px: the column goes, and the Action select lists the three groups with the presets as `(coming soon)` options. Once the mock has sent a few trace entries, the trace strip under the inspector shows the three newest with their 24 h times, the chevron expands it to every entry and back, and an entry opens the drawer on itself.
- [ ] #135 With the daemon and the Keypad, at 1363x798 and at 960 px: drag Exec onto key 7 and set the command: the file round-trips (only key 7's lines change) and the pad repaints.

### The catalogue entries from the recipe packs (#136)

The mock dev server of #113 in a host browser for the first two, then the app with the daemon and the Keypad. Run by the owner in #130.

- [ ] #136 `just app-dev` in a window at least 1200 px wide: the Actions column lists the packs of `contrib/recipes/` (System: Brightness down to Volume up; Apps: Calculator, Konsole, Screenshot, Spectacle, VS Code) with Chord and Exec at the end of General. Type `region` into Search actions: only Spectacle is left.
- [ ] #136 Drag Spectacle onto key 7: the result line says it was written, the tile shows the camera and `Capture`, and the inspector shows `What it does` set to `Full screen` above the Action; choose `Region`: the Command reads `spectacle --region`. Undo takes it back to `Full screen`. Narrow the window below 1200 px: the Action select lists the presets, and choosing `Volume up` there writes it into the selected key.
- [ ] #136 With the daemon and the Keypad (#119's user tests): Spectacle with `Region` on key 7, then press key 7 on the pad: Spectacle opens the region capture. Drag Volume up onto key 8 and press it: the volume rises and Plasma's volume OSD shows. The profile file round-trips (only the lines of keys 7 and 8 change).

### The profiles screen as the profile manager (#137)

The app with the daemon in the GNOME runtime. Run by the owner in #130.

- [ ] #137 At 1363x798 and at 960 px, light and dark: the list, the header form and the rules with no control under a scroll bar; the thumbnails render the pages.
- [ ] #137 Section 5 step 14: name and accent edits round-trip (the file byte-identical outside the field); `Edit keys on the Pad` opens the Pad pinned to the profile and the selected page.
- [ ] #137 Every dropdown opened and read in the light and the dark desktop theme (carried from #52).

### Muted text and focus ring that pass in every theme set (#144)

The mock dev server of #113 in a host browser. Run by the owner in #130.

- [ ] #144 `just app-dev`, open `?theme=breeze-light`, then `?theme=adwaita-light`: on the Profiles screen the file path, and on the Pad the hints and the empty-state sentences, read as muted but dark enough; disabled controls stay legible. Tab through the header: the focus ring is a clearly visible blue on both surfaces in `breeze-light`.
- [ ] #144 Run the browser detector of `docs/design/critique-2026-10.md` on both light themes: it reports no `low-contrast` on body text.
- [ ] #144 `?theme=breeze-dark` and `?theme=adwaita-dark` look as before (their measured values are kept).

### The profiles screen after the design pass (#139)

The mock dev server of #113 in a host browser. Run by the owner in #130.

- [ ] #139 `just app-dev`, Profiles, at 1363x798 and at 960 px, in a light and a dark theme: every label of the header form (Name, Accent, Priority, Enabled) sits above its field, muted and smaller, as in the Pad's Inspector; the accent swatch and its `#rrggbb` text share one row under the label.
- [ ] #139 Same sizes: `Open in editor` sits at the end of the heading row and `+ rule` at the end of the `Match rules` row, both where the form ends (`48ch`), not at the column's edge; the rule list and the thumbnails keep their widths.
- [ ] #139 Enabled reads as a value: `On` with the sentence that switching a profile off arrives with M4a-4, like Priority's `0  higher wins the window`; nothing there looks or acts like a switch.

### No selection outline while the pad is away (#143)

The mock dev server of #113 in a host browser. Run by the owner in #130.

- [ ] #143 `just app-dev`: while the pad shows Konsole, click key 3: it gets the accent outline. Wait for the focus cycle to move the pad: the outline goes, the drawing shows the new profile's tiles, the inspector stays on key 3 and the line under the tabs reads `Editing Konsole, key 3; the pad shows <profile>, page 1.` with `Follow`.
- [ ] #143 Click any tile while that line shows: the line goes, the inspector edits that key on the profile the pad shows, and the tile has the accent outline again.
- [ ] #143 Profiles, pick a profile the pad does not show, `Edit keys on the Pad`: the line reads `Editing <name>, page 1; the pad shows <profile>, page 1.` (`key <n>` instead of `page 1` when a key was selected before); click a tile: the inspector edits that key number on the chosen profile, the line names it, and the drawing draws no outline.

### The whole lucide set (#142)

The mock dev server of #113 in a host browser, then the daemon with the Keypad. Run by the owner in #130.

- [ ] #142 `just app-dev`, click a key, type `hou` in Icon: about a dozen rows with their glyphs, `house` among them; pick it: the tile shows the house.
- [ ] #142 Type `arrow` in Icon: 60 rows and under them `60 of 95, keep typing`; the arrows move over the 60 rows and wrap. Empty the field and press the down arrow: the first 60 names and `60 of <about 1900>, keep typing`. (`plus` matches 51 names, so it shows no count line.)
- [ ] #142 The Actions column still opens within a second.
- [ ] #142 With the daemon and the Keypad: set a key to `icon = "lucide:house"` in its profile by hand, save: the pad shows the house; `lucide:home` draws the same tile.

### The pad screen polished after the design pass (#141)

No Keypad needed for the first four; the mock dev server of #113 in a host browser, then the app on the GNOME runtime. Run by the owner in #130.

- [ ] #141 `just app-dev` at 1363x798 and at 960 px, `?theme=breeze-dark`, `?theme=breeze-light`, `?theme=adwaita-light`: under Recent events the newest entry reads in full (`07:54:58 Key 1, app layer: chord ctrl+p, sent`), the older ones take what is left; the chevron still opens every entry.
- [ ] #141 `?mock-error=unreachable`: the Error notice, "No pad connected." under Inspector, "Connect the daemon to assign actions." above the Actions column, and no "Select a key" anywhere; Retry keeps the notice (the mock stays unreachable). With the app on the GNOME runtime and no daemon: the same notice; start `keyvo-daemon`, Retry: the three prompts are back.
- [ ] #141 Hover over a catalogue entry, a rail item, a page tab, a field and a mirror tile: the border turns to the text colour and nothing else moves; the current rail item and the selected tile keep their accent border under the cursor; with the desktop's reduced motion on, the change is instant.
- [ ] #141 In the four themes, in the mirror and in the Profiles thumbnails: every tile sits inside the rim of its cutout with the rim's corner, the same shape in Breeze and in Adwaita and at both window sizes; no light square shows in the corners of a tile; a cutout without a tile shows its number in the drawing's glyph colour.
- [ ] #141 With the daemon and the Keypad on the GNOME runtime: the same four checks against the real tiles, then section 5 step 11 (the inspector writes a label, icon and colour; the lucide id under Icon is not monospace).

### The icon search kept through a refresh (#149)

The mock dev server of #113 in a host browser. Run by the owner in #130.

- [ ] #149 `just app-dev`, click a key, type `hou` in Icon, wait fifteen seconds (the focus cycle moves the pad at least once): the field still reads `hou` and the list its fourteen matches; pick `house`: the tile shows the house.
- [ ] #149 Type a Label, Tab into Icon, type `arr`: the search survives the write of the label.
- [ ] #149 Type `hou` in Icon, click another tile: the Icon field shows that key's icon and the list is closed.

### First-light records (#75)

- [ ] #75 Read `docs/hardware-notes.md`, "Measurements", against the first-light report on #40 and its logs: every number and log citation matches.
- [ ] #75 The spec §5.4 key origin table matches the `geometry` lines of `just run -- probe`.

## 7. What to post back

Clean up first: `rm -f grid.png x.png /tmp/demo.toml
/tmp/keyvo-*.png`, and move a saved `~/.config/keyvo.before-test` back if
you made one.

**On #70**, one comment:

```markdown
## No strip beside the keys (<YYYY-MM-DD>)

- Device: MX Creative Keypad 046d:c354, firmware <application, active line of probe>
- Commit: <git rev-parse --short HEAD from section 1, with #72 merged>
- Method: docs/pre-alpha-test.md section 4 (magenta panel, green keys, then the daemon's composite paint), several viewing angles; Solaar stopped
- Control (step 1): <magenta strip at bottom and right of each key / other: ...>
- Composite (step 2): <one update, no strip / strip on key ..., edge ...>
- Key press and profile switch (step 3): <no strip / strip on ...>
```

**On #52**, one comment:

```markdown
## Pre-alpha end to end (<YYYY-MM-DD>)

- Device: MX Creative Keypad 046d:c354, firmware <application, active line of probe>
- Commit: <git rev-parse --short HEAD>
- Host: <distribution, kernel (uname -r), Plasma version, Wayland or X11>
- Method: docs/pre-alpha-test.md sections 5 and 6
- Section 5: step 1 <pass / fail: what you saw> ... step 14 <...>
- Section 6: <PR number and item text of every unticked item, with what you saw>
- Logs: <the daemon log of section 5, attached>
```

Every failure or contradiction becomes a new slice on the pre-alpha epic.
