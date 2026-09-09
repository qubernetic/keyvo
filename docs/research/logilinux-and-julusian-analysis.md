# logilinux (upstream), the LauzHack origin, and Julusian's library: prior-art analysis for keyvo

Companion to `logimap-analysis.md` (which covered the abishekmuthian fork of logilinux as used by logimap). This document only records what is **new** relative to that report: upstream logilinux history/issues/tools/Dialpad, the LauzHack experimentation branch, Julusian's TypeScript library, and a cross-check of every protocol constant between them, plus facts from the other reverse-engineering efforts found on GitHub.

Checkouts (nothing modified) under `scratchpad/`:

| Alias used below | Repository | Commit |
|---|---|---|
| `logilinux/` | https://github.com/logilinux/logilinux (`master`) | `d621d56` (2026-05-16) |
| `feat_lcd/` | same repo, branch `origin/feat_lcd` exported with `git archive` | `17a81ca` (2025-11-22) |
| `fork-logilinux/` | https://github.com/abishekmuthian/logilinux (logimap's submodule) | `fde614a` |
| `logilinux-sdk/` | https://github.com/logilinux/logilinux-sdk | `dc79207` |
| `julusian/` | https://github.com/Julusian/node-logitech-mx-creative-console (`main`) | `53891fa` (2026-08-26) |
| `other-mx-creative-console/` | https://github.com/shensquared/mx-creative-console (macOS, C) | `8c29376` (2026-08-04) |
| `other-mx-creative-console-webhid/` | https://github.com/mario-gutierrez/mx-creative-console-webhid | `ff4f868` (2026-03-20) |
| `other-hcooper/` | https://github.com/hcooper-idealbuilders/mx-creative-controller (Windows, TS, pcaps) | `221c52f` (2026-06-12) |
| `other-creative-console-daemon/` | https://github.com/notno/creative-console-daemon (Windows, Rust) | `26be592` (2026-06-01) |
| `other-kevinschaich/` | https://github.com/kevinschaich/mx-creative-console (Dialpad, node-hid) | `7c6ff0b` (2026-02-03) |
| `other-companion-surface-logitech-mx-creative-console/` | https://github.com/bitfocus/companion-surface-logitech-mx-creative-console | `e872d1b` (2026-09-03) |

`file:line` references are to those revisions. No code was copied; only mechanisms and constants are described. "unclear" marks things no source settles.

---

## 1. Facts per project

### 1.1 logilinux/logilinux (upstream) and ron0studios/LogiLinux

| Item | Value | Evidence |
|---|---|---|
| Identity | `ron0studios/LogiLinux` **is** `logilinux/logilinux`: GitHub redirects (HTTP 301) and `gh api repos/ron0studios/LogiLinux` returns `full_name: logilinux/logilinux`. There is no separate "original" repo; the LauzHack repo was transferred to the `logilinux` org. | `gh api`, `curl -sI` |
| License | **MIT**, added 2026-05-16 in commit `d621d56` after issue #7 ("LICENSE", opened by logimap's author 2026-04-25); the repo had no license for its first 6 months | `logilinux/LICENSE`; `gh issue view 7` |
| Language | C++17, CMake; optional giflib + libjpeg (`HAVE_GIFLIB`) | `logilinux/lib/CMakeLists.txt:17,33-49` |
| Size | 6,485 tracked lines; library 2,205 (`lib/src/**`, `lib/include/**`; `mx_keypad_device.cpp` = 887); tools+src+examples 3,263; `tools/README.md` 513 | `wc -l` |
| Created / last push | 2025-11-22 (LauzHack 2025 weekend) / 2026-05-16 | `gh api` |
| Commits / authors | 23 on `master`; ImArjunJ (Arjun Juneja) and Ron0Studios wrote all protocol code on 22–23 Nov 2025 and 1 Dec 2025; GarThor contributed two arg-validation fixes (PR #2, #3, merged 2026-02-03) | `git log` |
| Branches | `master` (default), `main` (identical), `feat_lcd`, the original 2-commit experimentation branch ("init", "experimentation", both 2025-11-22) | `git branch -a`, `git log master..origin/feat_lcd` |
| Stars / forks | 12 / 6 | `gh api` |
| Issues | #4 giflib not found (closed), #5 PacketBufferPool race (**open**, 1 comment from logimap's author), #6 "adobe apps using wine" (closed, unanswered), #7 LICENSE (closed) | `gh issue list --state all` |
| PRs | #1 "Master spee" (merged), #2/#3 GarThor (merged), **#8 abishekmuthian reconnect fixes (open since 2026-07-09, 223+/24-)**, **#9 cwilling "Generalise installation lib directory" (open since 2026-08-20, lib64 fix)** | `gh pr list --state all`; `gh pr view 8/9` |
| Tests | none upstream (the fork added `tests/mx_keypad_monitor_restart_test.cpp`) | `git ls-files` |
| Protocol docs | none; no captures, no dissectors, no notes beyond code comments and `README.md:33-39` | `git log -p --all \| grep -i 'pcap\|wireshark\|capture'` → no hits |

### 1.2 logilinux/logilinux-sdk

| Item | Value | Evidence |
|---|---|---|
| License | MIT (`setup.py:57` classifier; `LICENSE`) | |
| Language | Python ≥3.7 + pybind11 C++ extension | `setup.py:13-29,52` |
| Size | Python package 1,197 lines; `src/bindings.cpp` 154; examples 1,078 (snake, tic-tac-toe, keypad, plugin) | `wc -l` |
| Activity | created and last pushed **2025-11-22** (one-day project); 2 stars, 3 forks, 0 issues, 0 PRs | `gh api` |
| Submodule | `logilinux-driver` → `https://github.com/ron0studios/LogiLinux` at `12035c9` (2025-11-22, pre-writev, pre-license) | `logilinux-sdk/.gitmodules` |
| Protocol content | none of its own; pybind11 wraps `initialize/setKeyImage/setKeyColor/hasLCD` | `src/bindings.cpp:126-131` |
| Notable | `examples/keypad_example.py:63-64` and `snake_game.py:45` render **90×90** JPEGs ("MX Keypad LCD buttons are 90x90 pixels"), the same wrong assumption logimap later hit (`logimap-analysis.md` §8.1); udev script uses `MODE="0666", TAG+="uaccess"` for both `bc00` and `c354` | `scripts/setup_permissions.sh:9-12` |

### 1.3 Julusian/node-logitech-mx-creative-console

| Item | Value | Evidence |
|---|---|---|
| License | MIT (root and each package) | `LICENSE`, `packages/*/LICENSE` |
| Language | TypeScript (ESM), monorepo: `core`, `node` (node-hid + `@julusian/jpeg-turbo`), `webhid`, `webhid-demo` | `packages/` |
| npm | `@logitech-mx-creative-console/core` and `/node`: published versions **0.2.3 (2025-06-14)** and **0.3.0 (2026-05-12)** only | `npm view … versions time` |
| Size | core src 1,636 lines (excl. tests), node src 232, webhid src 230, tests+mocks 334 | `wc -l` |
| Created / first protocol code / last push | 2024-10-31 / `7b332d7` "wip: initial drawing" 2024-10-31 / 2026-08-26 | `git log` |
| Commits / tags | 74; `v0.2.0`…`v0.3.0` | `git log`, `git tag` |
| Stars / forks | 19 / 2 | `gh api` |
| Origin | `61f3175` "chore: init with renamed elgato stream deck code": the library is a fork of Julusian's own `node-elgato-stream-deck`; several leftovers remain (see §4.9) | `git log`, `git show 7b332d7` |
| Issues (all open) | #9 WebHID "Stream Deck is of unexpected type" (2026-01-30); **#11 "Panels are not updated in sync"** (2026-03-25; author: device "picky about rate of data", added sleeps, ripple remains, Options+ has no ripple); **#14 Windows: report IDs 0x13/0x14 need their own HID collection handles** (2026-05-18, hcooper, with usage table) | `gh issue view 9/11/14` |
| PRs | 22 total; **#22 "Windows & Dialpad Support" (sommersdev, open 2026-09-04, +452/-65, admits "vibe coded" with Claude Code, then hand-edited)**; #21 README fix (merged); rest dependabot/release | `gh pr list`, `gh pr view 22` |
| Tests | only `transformImageBuffer` snapshot tests (pixel-format conversion, flips); **no protocol golden buffers**; `DummyHID` mock exists but is unused for protocol assertions | `packages/core/src/__tests__/util.spec.ts`, `__snapshots__/util.spec.ts.snap`, `__tests__/hid.ts` |
| Downstream | Bitfocus Companion surface module depends on `^0.3.0` (keypad only, "does not support the dial yet"); mx-console-suite (macOS), hcooper (Windows) reuse its `imageWriter`/`imagePacker` classes | `other-companion-…/package.json:28`, `companion/HELP.md`; `other-hcooper/keypad/src/device.ts:6-8` |

---

## 2. How the protocol was reverse-engineered

### 2.1 Julusian (October–November 2024): the earliest public implementation
- The LCD frame writer (`14 ff 02 2b`, 20-byte first header, 4095-byte packets, flag byte `index|0x20|0x80|0x40`, big-endian x/y/w/h, u16 length at offset 18) appears fully formed in `7b332d7` on **2024-10-31**, one month after the product launch, replacing the Stream Deck header generator (`git show 7b332d7 -- packages/core/src/services/imageWriter/imageWriter.ts`).
- The repo contains no captures or notes, but `packages/core/src/models/mx-creative-keypad.ts:44-305` preserves, commented out, a **46-packet transcript of what Logi Options+ writes on connect** (root feature queries `11 ff 00 1b …`, `0x0b` divert calls, `13 ff …` long reports, `11 ff 0f 2b …`, `11 ff 04 1b 0b b8`, etc.) with only the two `11 ff 0b 3b 01 a1/a2 03` writes left active (`:261-264`, `:273-276`). This is the best evidence that Julusian worked from a USB/HID capture of Options+ and then bisected the init sequence down to the two divert writes (commit `0c38e41` "wip: basic arrow keys (once they are streaming)", `5da7b42` "wip: init arrow buttons", 2024-11-01).
- Brightness was found on 2024-11-01 (`63ca601` "wip: implement brightness").

### 2.2 LauzHack 2025 (22–23 November 2025): logilinux
- Timeline from `git log`: `f36ec6d` initial commit, `a9cb38f` "feat: dialpad implementation (holy **** it works)" (Dialpad via evdev + a libhidpp-based debug tool), `34d1ce2` "added keypad support" (LCD + button parsing), then a burst of fixes on 22–23 Nov (`2e4ff70` release, `163536a` multi-button, `6bd4ff1` poll, `12035c9` double-count, `9b53874` random presses, `7326ee5` "fix: holy **** it works", the `report[5]==0x01` grid check), `a168307` giflib, `824c173` tools.
- Method for the Dialpad: `src/find-dialpad-events.cpp` opens every `/dev/hidraw*` in a thread each and hex-dumps whatever arrives (`:28-53,64-85`); `src/input-event-monitor.cpp` does the same for every `/dev/input/event*` with Logitech VID (`:22-96`); `src/dialpad-debug.cpp` uses **cvuchener's `libhidpp`** (vendored under `ref/hidpp/`, removed from master in `d074369`) to read protocol version, product ID and enumerate `IFeatureSet` (`:16-25,52-53,206-221`) and then dumps raw reports classifying `0x10/0x11/0x12` as HID++ short/long/very-long (`:113-121`). Result recorded in `README.md:35-39` (REL_HWHEEL low-res, REL_MISC high-res) and `README.md:105` ("Protocol: HID++ 4.5").
- Method for the Keypad LCD: the `feat_lcd` branch is the raw experimentation. `feat_lcd/hid_monitor.cpp` is a 91-line hidraw hex dumper (default `/dev/hidraw1`, `:24`). `feat_lcd/lcd_display.cpp:24-27` already contains the two `11 ff 0b 3b 01 a1/a2 03` "magic reports to wake/init the device" and `:92-184` the exact same packet layout, constants (`MAX_PACKET_SIZE 4095`, `LCD_SIZE 118`, `PACKET1_HEADER 20`, `headerSize 5`), and `generateWritePacketByte()` bit layout as Julusian's `imageWriter.ts`, with bytes 5–8 annotated `// Unknown` (`:121-122`). The upstream comment in speed commit `5637419` says outright: *"The WebHID implementation sends packets sequentially without delays"*. **Conclusion: the logilinux LCD protocol was derived from Julusian's published WebHID/Node implementation, not from independent captures**; the hackathon's own contribution is the Linux hidraw/evdev plumbing, the input-report parsing (`13 ff 02 00 xx 01 [codes]`, P-button `11 ff 0b 00 01 a1`), GIF/video playback and the tools. No pcap, Wireshark dissector or write-up exists in any of the three logilinux repos or the `ayushr27/Logi-logi` hackathon fork (its `HACKATHON_SUBMISSION.md` is a features pitch, "Genie in a Bottle", Category 1).
- Upstream's own inter-packet timing history: first version wrote packet-by-packet with `usleep(2000)` (`feat_lcd/navigation_grid.cpp:66`; 5 ms in `lcd_display.cpp:86`); `5637419` removed delays (0.1 ms only on `EAGAIN`) → `e0d9c1c` "no speed :(" reverted → `9c55b4e` switched to one `writev()` → `d074369` added the packet pool and `setScreenImage`. So upstream did observe that back-to-back individual `write()`s misbehaved but that a single `writev()` works (why one works and the other not is unclear; possibly kernel batching, see §8 Q8).

### 2.3 hcooper-idealbuilders (May–June 2026, Windows): the only public **USB captures**
- Wireshark 4.6.5 + USBPcap, elevated; captures committed: `captures/02-binding-paint.pcap` (Options+ painting a newly assigned key) and `captures/03-options-plus-startup.pcap` (cold start, ~15 HID++ command types) (`captures/README.md:5-9`). Derived TSV with the five 4095-byte LCD frames (`captures/derived/binding-paint-frames.tsv`).
- Journal `docs/journal/2026-05-13-from-sdk-to-col03.md:47-59`: frame 521 shows plain JFIF at offset 0x30 → "Plain JPEG. No encryption"; Windows exposes 5 HID collections, only `Col03` (usage `0x1a10`) accepts report `0x14` (`:63-77`).
- `docs/journal/2026-05-13b-…md:9-27`: wrote `experiments/decode-hidpp.mjs` to rebuild the feature table from Root queries in the capture (index 0x04 = feature `0x0008` = the 1 Hz heartbeat `11 ff 04 1d 0b b8`).
- Their `docs/protocol.md:44-48` misread byte 3 (`0x2d`) as a transaction counter and bytes 10–17 as "unmapped geometry", corrected later by shensquared (§7.1).

### 2.4 shensquared (August 2026, macOS): verification probes, no Logitech software installed
- Dependency-free IOKit probes, one concern each: `probe.c` parses the HID report descriptor and lists collections; `features.c` enumerates IFeatureSet; `cids.c` enumerates `0x1B04` controls and their live divert bit; `claim.c` tests KeepAlive and divert; `listen.c` is read-only; `writetest.c` proves one handle takes `0x11/0x13/0x14`; `dial.c` probes `0x4610`; `display.c` probes `0x00c3` (`CLAUDE.md:33-45`). `docs/PROTOCOL.md` marks each capability "proven" (`:323-339`).

### 2.5 mario-gutierrez (Nov 2025–Mar 2026, WebHID): HID++ feature-level implementation
- Implements HID++ 2.0 root lookup, `0x0007`, `0x1B04`, `0x4610`, `0x8040` properly (feature index discovered at runtime), but hard-codes `0x19A1` "ContextualDisplay" at index 2 for the LCD (`js/feature19a1.js:11-13`), but shensquared later found `0x19A1` is **not** in the Keypad's feature table (§7.1).

---

## 3. Upstream logilinux findings not in the logimap report

The logimap report described the fork's `mx_keypad_device.cpp` (962 lines). Upstream master differs by exactly the fork's reconnect patch (`git diff up/master HEAD --stat`: 8 files, +256/−24; `device_manager.cpp`, `input_monitor.cpp`, `library.cpp`, `mx_keypad_device.cpp`, `tests/`). Everything below is upstream-only or was not covered before.

### 3.1 Upstream vs fork state
- **PR #8 is the fork's work, offered upstream and unmerged** (`gh pr view 8`): stop monitor loops on `POLLERR|POLLHUP|POLLNVAL`, EOF and non-retryable errors; always join monitor threads; refresh discovery each call; enumerate `/dev/hidraw*` dynamically ("On my system it appeared as `/dev/hidraw22`"); mutex around monitor lifecycle. Upstream still has the `hidraw0..19` loops (`device_manager.cpp:66`, `mx_keypad_device.cpp:266`), the cached `devices_` in `library.cpp:29-31`, and the monitor loop that only breaks on `poll()<0` or non-`EAGAIN` read error (`mx_keypad_device.cpp:349-351,479-481`).
- **Issue #5 (PacketBufferPool) is unfixed upstream** (`mx_keypad_device.cpp:28-47`: static 16-slot ring, `fetch_add % 16`, no release); GarThor's issue text also notes it is a process-wide singleton.
- **PR #9** (open): CMake hard-codes `lib` (`lib/CMakeLists.txt:61-65,80-82`), breaking `/usr/lib64` distros.

### 3.2 Device manager and discovery (upstream)
- `scanDevices()` scans `/dev/input/event*` first (`device_manager.cpp:30-63`) and then `/dev/hidraw0..19` (`:66-85`); both keypad **evdev** nodes and the hidraw node become separate `MXKeypadDevice` objects. An `MXKeypadDevice` built from an event path tries to find a hidraw node by VID/PID (`mx_keypad_device.cpp:256-281,290-295`); `LCD_DISPLAY`/`IMAGE_UPLOAD` capabilities are only added when a hidraw path was found (`:297-300`).
- `probeHidrawDevice` opens `O_RDWR|O_NONBLOCK` just to `HIDIOCGRAWINFO` (`device_manager.cpp:143-149`); no report-descriptor inspection (`HIDIOCGRDESC`) anywhere.
- `findDevice(MX_KEYPAD)` prefers a hidraw-path device, else any (`library.cpp:35-50`); for `DIALPAD` first match (`:51-58`), which is always an evdev node because hidraw scanning only creates keypad objects (`device_manager.cpp:72-79`).
- `Library::discoverDevices()` is not thread-safe and `findDevice` caches (`library.cpp:23-31`).

### 3.3 Dialpad support (upstream)
- Identification: `MX_DIALPAD_PRODUCT_ID = 0xbc00` (`device_manager.cpp:19`). `0xbc00` is a **Bluetooth** PID (all Logitech BT HID PIDs are `0xbXXX`); over a Bolt receiver the USB PID would be the receiver's (`0xc548`) and the device would be a child index, and no source in this set handles that path (shensquared `docs/PROTOCOL.md:310-312`: "untested").
- `DialpadDevice` is **evdev-only**: `InputMonitor` opens the event node `O_RDONLY|O_NONBLOCK` (`input_monitor.cpp:29`), `poll()` 100 ms (`:81`), optional `EVIOCGRAB` (`:47`). Capabilities: `ROTATION`, `BUTTONS`, `HIGH_RES_SCROLL` (`dialpad_device.cpp:10-12`).
- Rotation mapping (`input_monitor.cpp:106-143`): `EV_REL` codes `0x08 REL_WHEEL` / `0x0b REL_WHEEL_HI_RES` → `RotationType::WHEEL`; `0x06 REL_HWHEEL` / `0x0c REL_HWHEEL_HI_RES`, `REL_DIAL`, `REL_MISC` → `RotationType::DIAL`. Low-res codes give `delta` and `delta_high_res = delta*120`; hi-res codes give `delta_high_res` and `delta = value/120` (rounded to ±1 if nonzero). README contradicts the code slightly: `README.md:37-38` says the dial sends `REL_HWHEEL (6)` low-res "1-6 units per tick" and `REL_MISC (12)` high-res "120 units per degree" (code treats `0x0c` as `REL_HWHEEL_HI_RES`, which is also 12: same number, different name). Which physical roller (large dial vs small wheel) maps to which axis is **unclear** from upstream; `dialpad-volume.cpp:212` uses code 6 for "the dial".
- Buttons: evdev key codes **275–278** (`BTN_SIDE`, `BTN_EXTRA`, `BTN_FORWARD`, `BTN_BACK`) named `TOP_LEFT/TOP_RIGHT/BOTTOM_LEFT/BOTTOM_RIGHT` (`events.h:14-20,86-99`). `dialpad-volume.cpp:219-226` treats *any* `EV_KEY` as "dial press" (there is no dial click; see §6).
- HID++: only in the debug tool. `dialpad-debug.cpp` prints protocol version and feature IDs but records none in the repo; README claims "HID++ 4.5" (`README.md:105`). No HID++ writes to the Dialpad exist in the library.
- Tools: `dialpad-monitor` (`--json`, `--grab`, `--device`), `dialpad-grab` (holds `EVIOCGRAB` until Ctrl-C, `tools/dialpad-grab.cpp:123-131`), `logilinux-devices --json` (`tools/README.md:53-68`).
- Permissions: README udev rule covers only `hidraw` for `bc00` (`README.md:90-92`); `tools/README.md:405-411` adds `input` + `hidraw` rules for both PIDs with `MODE="0666"`.

### 3.4 Keypad extras (upstream)
- `setScreenImage()` = `setRawImage(23, 6, 434, 434, jpeg)` (`mx_keypad_device.cpp:586-590`; `SCREEN_WIDTH = 118*3+40*2`, `.h:41-44`). GIF playback per key (`:633-687`, one thread per key, `sleep_for(frame.delay_ms)`) and full-screen GIF (`:771-822`); `examples/video-test.cpp` decodes video with ffmpeg and calls `setScreenImage` per frame at source fps (`:175-178,262-269`).
- `keypad-set-color` shells out to ImageMagick `convert -quality 85` on a 118×118 PPM (`tools/keypad-set-color.cpp:159-168`); `tools/README.md:487` recommends JPEG "< 50KB" (no basis given).
- `grabExclusive()` is a no-op returning false for the keypad (`mx_keypad_device.cpp:500-503`).
- Monitor thread reads up to 256 bytes per `read()` (`:337-338`) and ignores byte 4 of the grid report (documented as `xx`, `:410`).
- Timestamps: keypad events use `steady_clock` ms; Dialpad events use the evdev `tv_sec*1e6+tv_usec` µs (`input_monitor.cpp:112-113`), so the units are inconsistent across device types.

### 3.5 Licensing caveat
`feat_lcd/ref/hidpp/COPYING` and the pre-`d074369` master vendored cvuchener's `hidpp` (GPL-3.0). It is only linked into the `src/dialpad-debug` tool (`src/CMakeLists.txt`), not `liblogilinux`, but the SDK's pinned `logilinux-driver` submodule (`12035c9`) still carries it. keyvo must not lift anything from `ref/`.

---

## 4. Julusian library findings

### 4.1 Enumeration and device model
- `VENDOR_ID = 0x046d` (`packages/core/src/index.ts:18`); models table `DEVICE_MODELS2` with `MX_CREATIVE_KEYPAD: productIds [0xc354]` (`:40-44`); Dialpad `0xbc00` is present only as a commented entry (`:45`; `id.ts:11,16`). `generateButtonsGrid(3,3,{118,118},{23,6},{40,40})` (`models/mx-creative-keypad.ts:19`) yields `hidId = index+1` (1..9) and `pixelPosition = 23+col*158, 6+row*158` (`controlsGenerator.ts:16-17,27-31`). Page buttons: `index 9/10`, `hidId 0x01a1/0x01a2`, `feedbackType 'none'` (`mx-creative-keypad.ts:20-21`). `PANEL_SIZE 480×480` (`:24`).
- **Interface / collection selection: none.** Node: `HID.devicesAsync()` deduplicated by path; the first path whose PID matches is opened (`packages/node/src/index.ts:35-41,87`). WebHID: `requestDevice({filters:[{vendorId}]})` (`packages/webhid/src/index.ts:32`). On Linux `hidapi` exposes one path per hidraw node, so this is equivalent to logilinux's first-match. On Windows this is the root cause of issue #14 (five collection paths, only Col01 opened).
- No hotplug in the library; `examples/device-detection.js:42-48` polls with the `usb` package but filters on vendor **`0x0fd9` (Elgato)**, a Stream Deck leftover, so the example never re-scans for the keypad.

### 4.2 Init sequence
- Two 20-byte writes, `11 ff 0b 3b 01 a1 03 00…` and `… a2 …` (`mx-creative-keypad.ts:261-264,273-276`), sent via `sendReports` immediately after open (`node/src/index.ts:99`; `webhid/src/index.ts:76`). Node writes them back-to-back with one 10 ms sleep after the pair (`node/src/hid-device.ts:48-56`); WebHID has no delay at all (`webhid/src/hid-device.ts:41-47`). Decoded per HID++ (`§5`): feature index `0x0b` = ReprogControlsV4 (`0x1B04`), function 3 `setCidReporting`, software id `0xb`, CID `0x01a1`/`0x01a2`, flags `0x03` (divert + …).
- PR #22 replaces this with `performInitWrites`: 100 ms settle after open, 20 ms between writes, commented "A device handles one request at a time and silently drops any arriving while it is busy, so writing them back to back loses roughly every other one" and "A request made as soon as the device is opened can be dropped, which over bluetooth leaves the control … silently dead" (`julusian-pr22.diff:34-62`).

### 4.3 LCD output report (`packages/core/src/services/imageWriter/imageWriter.ts`)
- `MAX_PACKET_SIZE 4095` (`:6`), first header 20 bytes (`:11`): `[0..3] = 14 ff 02 2b` (`:16-19`), `[4] = generateWritePacketByte(1, true, fitsInOne)` (`:20`), `[5..6] = 0x0100`, `[7..8] = 0x0100` (`:21-22`), `[9..16] = x, y, w, h` big-endian u16 (`:23-26`), `[17]` untouched (0), `[18..19] = byteBuffer.length` u16 (`:27`) → **silent truncation above 65,535 bytes** (`setUint16` masks). Continuation: 5-byte header, part numbers from 2 (`:32-47`). Flag byte: `index | 0b0010_0000`, `|0x80` first, `|0x40` last (`:54-61`). Byte-identical layout to logilinux.
- All packets are exactly 4095 bytes, zero-padded (`new Uint8Array(MAX_PACKET_SIZE)`), same as logilinux.

### 4.4 Image pipeline
- Input pixel formats `rgb|rgba|bgr|bgra`; buffer length must equal `w*h*bpp` exactly (`buttonsLcdDisplay/default.ts:181-184`); converted to RGBA by `transformImageBuffer` and JPEG-encoded (`imagePacker/jpeg.ts:18-27`). Node: `@julusian/jpeg-turbo`, `FORMAT_RGBA`, **quality 95** default, `subsampling` option (`node/src/jpeg.ts:9,30-36`); WebHID: canvas `toBlob('image/jpeg', 0.9)` (`webhid/src/jpeg.ts:26-27`).
- `clearPanel()` paints one **480×480 black JPEG at (0,0)** (`default.ts:82-97`), a different geometry from logilinux's 434×434@(23,6) (see §5). `clearKey()` sends a 118×118 black RGB image (`:127-132`). `fillKeyColor` builds an RGBA solid fill (`:146-168`). `fillPanelBuffer()` slices a `354×354` RGB(A) buffer (3×118 per side, `calculateDimensionsFromGridSpan`, `:61-66`) into nine per-key writes issued with `Promise.all` (`:219-240`), serialised by the p-queue.
- `withPadding` is `throw new Error('Not implemented')` (`:57-59`).

### 4.5 Brightness, "reset to logo", firmware/serial
- `setBrightness(pct)`: range 0–100 checked, then clamped to **≥1 with the comment "0 is not allowed, it resets the device"** (`properties/default.ts:12-16`); report `11 ff 0f 2b 00 <pct> 00…` (`:19-23`) = feature index `0x0f` (hard-coded), function 2, swid `0xb`, 16-bit BE value = pct.
- `resetToLogo()`: **feature report** `03 02 00 … (32 bytes)` via `sendFeatureReport` (`:29-36`). This is the Stream Deck reset command (`git log -S` traces it to `61f3175`/`89fb1ef`, the Stream Deck import). Whether it does anything on the Logitech device is **unclear**; no other source uses it; Bitfocus calls it on close (`other-companion-…/src/instance.ts:63`). A commented alternative in `models/base.ts:119-128` sends `11 ff 04 1b 0b b8` (feature index 4 = KeepAlive fn 1, 3000 ms) as a "finish" write, i.e. Julusian saw the heartbeat in the capture but did not identify it.
- `getFirmwareVersion`/`getSerialNumber` commented out; they used Stream Deck feature reports 5/6 (`default.ts:39-49`).

### 4.6 Input parsing (`packages/core/src/services/input/mx-creative-keypad.ts`)
- The HID layer strips the report id and passes `(reportId, data)` (`node/src/hid-device.ts:33-35`; `webhid/src/hid-device.ts:20-23`).
- `handleInput`: **drops any report whose `data[2] == 0x2b`** ("Ignore acks to drawing", `:28`), then `0x13` → LCD keys, `0x11` → page buttons (`:32-36`).
- LCD keys: require `data[0..2] == ff 02 00` and `data[4] == 0x01` (`:40-46`; raw offsets 1,2,3 and 5, identical to logilinux's check), then read **signed int8** values from `data[5]` (raw 6) until 0, map by `hidId` (1..9), diff against `#pushedButtons` → `down`/`up` events (`:51-79`). Values ≥0x80 would read negative and be ignored.
- Page buttons: require `data[0..2] == ff 0b 00` (`:83`), then a **list of big-endian u16 CIDs from `data[3]` (raw 4) until `0x0000`** (`:88-90`), diffed separately (`#pushedArrowButtons`). Same bytes as logilinux's `11 ff 0b 00 01 a1` but modelled as a pressed-list (so both page buttons held at once are representable; logilinux tracks a single `last_p_button`).
- Byte 4 of the `0x13` report (a sequence/flag byte per shensquared) is ignored, like logilinux.

### 4.7 Write pacing and issue #11
- Node `sendReports`: p-queue concurrency 1; each batch is written packet-by-packet with `await hid.write()`, then **`setTimeout(10)`**: "Small delay to prevent overwhelming the device with back-to-back reports, which can cause it to skip some draws" (`node/src/hid-device.ts:48-57`). Introduced in `c343933` (2026-05-12, shipped in 0.3.0) after issue #11 (2026-03-25). The maintainer: "It seems that the panel is a bit picky about rate of data. If sending too fast, it ignores the latter messages (typically bottom right …)". Ripple across the 9 keys remains; reporter says Options+ shows no ripple. Bitfocus adds another 5 ms after each key and retries a failed write 3× with 20 ms back-off (`other-companion-…/src/instance.ts:92-111`).
- `examples/rapid-fill.js:13-36` fills 9 keys sequentially at 5 Hz (+100 ms pause) as the stress test.

### 4.8 Linux packaging
- udev rules generated by `udev-generator` (`scripts/regenerate-udev.mjs`): `KERNEL=="hidraw*", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c354", MODE:="660", TAG+="uaccess"` (desktop) or `GROUP="plugdev"` (headless), plus a blanket `SUBSYSTEM=="input", GROUP="input", MODE="0660"` (`packages/node/udev/*.rules:1-3`). PR #22 adds `bc00` lines.

### 4.9 Stream Deck leftovers to be aware of
`resetToLogo` feature report (§4.5); WebHID error text "Stream Deck is of unexpected type." (`webhid/src/index.ts:66`, the text in issue #9); `usb` vendor `0x0fd9` in the example; JSDoc "brightness of the keys on the Stream Deck" (`types.ts:106`); `MXConsoleEncoderControlDefinition` with `hasLed`/`ledRingSteps` (`controlDefinition.ts:33-44`), unused until PR #22.

### 4.10 PR #22: Dialpad and Windows (unmerged, 2026-09-04)
- Windows: `collections.ts` groups node-hid paths by stripping `&ColNN#`, keeps only `usagePage ≥ 0xff00`, derives the report ids a collection carries from the **low byte of the collection usage as a bitmask counting from 0x10** (`usage 0x1a02 → bit1 → 0x11`, `0x1a08 → 0x13`, `0x1a10 → 0x14`) (`julusian-pr22.diff:578-662`), opens every collection and routes each write by `data[0]` (`:663-755`). The "primary" path is the one carrying `0x11`.
- Dialpad model (`packages/core/src/models/mx-creative-dialpad.ts` in the diff, `:192-289`): `productIds [0xbc00]`; encoders `hidIndex 0` (row 0 col 2, **`invertRotation: true`**, "reports its rotation the opposite way round to the large dial") and `hidIndex 1` (row 1 col 1); buttons CIDs `0x0053, 0x0056, 0x0059, 0x005a`; feature indices **hard-coded** `DIALPAD_REPROG_CONTROLS_FEATURE_INDEX = 0x0a`, `DIALPAD_DIAL_FEATURE_INDEX = 0x0d` (`:356-357`); init = `setCidReporting(cid, 0x03)` for the 4 buttons and `0x4610 fn3 [rollerIdx, 0x01]` for both rollers (`:264-289`). Input: report `0x11`, `data[0]==0xff`, `data[2]==0x00` (notification), switch on feature index; buttons = u16 CID list from `data[3]`; dial = `hidIndex data[3]`, **signed int8 delta at `data[4]`** (raw byte 5), inverted for roller 0 (`:391-446`). No display/brightness on the Dialpad (`properties`/`buttonsLcd` become optional).

---

## 5. Cross-check table: logilinux upstream vs Julusian core

Legend: AGREE / DISAGREE / ONLY-IN-L (logilinux) / ONLY-IN-J (Julusian). Third-party columns are cited in §6–7 where they break a tie. `L` = `logilinux/lib/src/devices/mx_keypad_device.cpp` unless another file is named; `J` = `julusian/packages/core/src/…`.

| # | Aspect | logilinux | Julusian | Verdict |
|---|---|---|---|---|
| 1 | Vendor / Keypad PID | `0x046d` / `0xc354` (`core/device_manager.cpp:17,20`; L:272) | `0x046d` / `0xc354` (`index.ts:18,41`) | AGREE |
| 2 | Dialpad PID | `0xbc00` (`device_manager.cpp:19`) | `0xbc00` commented (`index.ts:45`), active in PR #22 | AGREE (value) / ONLY-IN-L (support) |
| 3 | Interface / collection selection | first `/dev/hidraw*` (0..19) with matching VID/PID (L:266-279; `device_manager.cpp:66-85`) | first node-hid path with matching PID (`node/src/index.ts:36-51,87`) | AGREE (both none). Windows needs 3 collections (issue #14, PR #22); Linux topology unclear, see Q1 |
| 4 | Report IDs used | out `0x11` (init), `0x14` (image); in `0x11`, `0x13` (L:73-78,24,370,414) | same (`mx-creative-keypad.ts:262`, `imageWriter.ts:16`, `input/…:32-34`) | AGREE |
| 5 | Output packet size | 4095 (L:20), zero-padded (L:125,147) | 4095 (`imageWriter.ts:6`), zero-padded | AGREE |
| 6 | Init writes | `11 ff 0b 3b 01 a1 03`, `11 ff 0b 3b 01 a2 03`, 20 B each (L:73-78) | identical (`mx-creative-keypad.ts:261-276`) | AGREE |
| 7 | Init pacing | `write()` + `usleep(10000)` after each (L:520-523) | both then one 10 ms sleep (node) / none (webhid); PR #22: 100 ms settle + 20 ms gap | DISAGREE (timing), see Q3 |
| 8 | Any keep-alive / heartbeat | none | none in 0.3.0; `11 ff 04 1b 0b b8` sits commented in `base.ts:119-128` | AGREE (absent), but shensquared/hcooper/Catalyst say the display reverts without traffic (§7), see Q4/Q5 |
| 9 | Image header bytes 0–3 | `14 ff 02 2b` (L:24) | `14 ff 02 2b` (`imageWriter.ts:16-19`) | AGREE (Options+ capture uses `14 ff 02 2d`, §7.3) |
| 10 | Flag byte [4] | `index\|0x20`, `\|0x80` first, `\|0x40` last, first packet index **1** (L:109,150,168-175) | identical, index 1 (`imageWriter.ts:20,45,54-61`) | AGREE (Options+ starts at index **0**: `e0`/`a0`+`61`, §7.3), see Q7 |
| 11 | Bytes [5..8] | `01 00 01 00` from `PACKET1_GEOMETRY` (L:25,111) | `setUint16(5,0x0100)`, `setUint16(7,0x0100)` (`:21-22`) | AGREE (meaning: mario reads them as displayIndex=1, defer=0, numImages=1, format=0, §7.2) |
| 12 | Bytes [9..16] | x, y, w, h BE u16 (L:112-119) | same (`:23-26`) | AGREE |
| 13 | Byte [17] | 0 | 0 | AGREE (mario/0x19A1 reading: bits 16–23 of a 24-bit length, see Q11) |
| 14 | Bytes [18..19] | JPEG length BE u16 (L:120-121) | same, masked by `setUint16` (`:27`) | AGREE (both wrap >65535) |
| 15 | First-packet payload / continuation | 4075 / 5-byte header, 4090 payload, parts from 2 (L:83-84,128,143-163) | same (`:13,32-38`) | AGREE |
| 16 | Key geometry | 118×118, origin (23,6), pitch 158 (L:21,101-104) | same (`mx-creative-keypad.ts:19`, `controlsGenerator.ts:29-30`) | AGREE |
| 17 | Full-panel geometry | 434×434 at (23,6) (`.h:41-44`, L:586-590) | `PANEL_SIZE 480×480`, `clearPanel` paints 480×480 at (0,0) (`mx-creative-keypad.ts:24`; `default.ts:82-97`); `fillPanelBuffer` is 354×354 sliced per key | DISAGREE, see Q10 (Options+: 435×434 at (23,6); mario: 457×440) |
| 18 | Image format | baseline JPEG from ImageMagick q85 / libjpeg (tools) | JPEG via jpeg-turbo q95 (RGBA in) / canvas q0.9 | AGREE (JPEG) / DISAGREE (quality, subsampling unspecified), see Q11 |
| 19 | Write mechanism | one `writev()` of all packets, `O_NONBLOCK` first then blocking on `EAGAIN` (L:551-566); no inter-packet delay | per-packet `write()`, 10 ms after each batch (node) | DISAGREE, see Q8/Q9 |
| 20 | Brightness | not implemented | `11 ff 0f 2b 00 <1..100>`; 0 "resets the device" (`properties/default.ts:12-23`) | ONLY-IN-J, see Q5 (mario: 16-bit raw with min/max from `0x8040 fn0`; Options+ capture sends `00 46` = 70) |
| 21 | Reset to logo | none | Stream Deck feature report `03 02 00…` (`:29-36`) | ONLY-IN-J, unverified, see Q19 |
| 22 | LCD key input match | `[0]=13 [1]=ff [2]=02 [3]=00 [5]=01`, byte 4 ignored (L:414-415) | `data[0..2]=ff 02 00`, `data[4]=01` (`input/…:40-46`) | AGREE |
| 23 | LCD key codes | bytes 6+ = `1..9`, 0-terminated, converted to 0..8 (L:420-429) | `data[5..]` int8 until 0, `hidId 1..9` → index 0..8 (`:51-60`) | AGREE |
| 24 | Ack/echo filtering | only via `[3]==0x00` in the grid match; `0x11` P-button match also requires `[3]==0x00` (L:370-371) | explicit `data[2]==0x2b` drop (`:28`) plus `[3]==0` checks | AGREE in effect (shensquared: `13 ff 02 2b c1 00 01 00` echoes ≈ every 5 s must be dropped, see Q12) |
| 25 | Page buttons | `11 ff 0b 00 01 a1/a2` press, `11 ff 0b 00 00 00` release; single `last_p_button` (L:363-406) | `11 ff 0b 00` + u16 CID list until 0 (`:83-97`); per-button set | AGREE (bytes) / DISAGREE (model) |
| 26 | "Spurious grid data" in P-button packets | skips grid parsing for those packets (L:365-368,409) | not needed: report id dispatch separates 0x11 from 0x13 | AGREE (logilinux's comment describes a `0x11` packet; grid data can only be in `0x13`) |
| 27 | Multi-key chords | full held-set diff (L:417-476) | same (`:62-79`) | AGREE |
| 28 | Input read buffer | 256 B (L:337) | node-hid/WebHID report-sized | AGREE (max input report is 32 B per descriptor, §7.1) |
| 29 | Hot-plug / reconnect | none upstream (fork/PR #8 partial) | none (device-detection example broken) | AGREE (absent) |
| 30 | Dialpad transport | evdev `/dev/input/event*` (REL_HWHEEL/REL_MISC…, BTN 275–278) | PR #22: HID++ notifications on report `0x11` (`0x4610` idx `0x0d`, `0x1B04` idx `0x0a`) | ONLY-IN-L vs ONLY-IN-J(PR), see Q16/Q17 |
| 31 | Permissions | `MODE="0666"` (README) | `MODE:="660", TAG+="uaccess"` / `plugdev` | DISAGREE (policy) |
| 32 | Tests with golden buffers | none | none (pixel-transform only) | AGREE (absent) |

**Disagreements that define the hardware quick-test:** rows 7, 8, 10, 17, 18, 19, 20, 21, 25, 30.

---

## 6. Dialpad facts from all sources

| Fact | Source |
|---|---|
| Identity `046d:bc00` over **Bluetooth**; two collections: Generic Desktop **Mouse report `0x02`** (16 buttons, 12-bit X/Y, wheel, AC Pan) and vendor `0xFF43` report `0x11` (HID++ short, 20 B). No USB data path (2×AAA, BT or Logi Bolt). Behind Bolt the HID++ device index would be 1–6 instead of `0xFF`, untested by anyone | shensquared `docs/PROTOCOL.md:16-17,243-256,310-312` |
| Feature table: 29 entries; **no `0x0008` KeepAlive** → no heartbeat needed; `0x4610` MultiRoller at index `0x0d`; `0x1B04` at index `0x0a` ("not `0x0b` as on the Keypad, so the index has to be looked up") | `docs/PROTOCOL.md:254-260,290-292`; PR #22 hard-codes the same indices (`julusian-pr22.diff:356-357`) |
| Rollers (`0x4610 fn0` → 2): roller **0 = small wheel**, fn1 `28 00 1f 00` (0x28 = 40 increments/rotation); roller **1 = large dial**, `b4 00 1f 00` (180 increments/rotation); fn2 get mode, fn3 set mode, only bit 0 stored (divert flag); diverting stops native pointer/scroll | `docs/PROTOCOL.md:258-272`; mario `js/feature4610.js:11-16,53-121` (fn1 = incrementsPerRotation, incrementsPerRatchet, lightbarId/timestamp bit) |
| Rotation notification: `11 ff 0d 00 <roller> <delta s8> 00 <ts…>`; deltas accelerate (1..9, `ff`..`f7`); `-1` arrives as `ff 00` so byte 6 is not a high byte; bytes 7–9 a rising timestamp | `docs/PROTOCOL.md:273-287`; `src/dial.c:28-33`; mario reads a 32-bit BE timestamp at params[2..5] (`feature4610.js:135-139`) |
| PR #22: roller 0 reports the **opposite sign** of roller 1 (`invertRotation: true`) | `julusian-pr22.diff:209-222` |
| Buttons: four `0x1B04` controls, CIDs `0x0053, 0x0056, 0x0059, 0x005a`, TIDs `0x003c, 0x003e, 0x0109, 0x0109`, flags `0x31` (reprogrammable + divertable); diverted presses arrive as `0x1B04` notifications carrying the CID list and **repeat while held** | `docs/PROTOCOL.md:289-305`; PR #22 same CIDs; mario `DIALPAD_KEYS` (`js/mxCreativeConsole.js:32-37`) |
| Natively the four buttons are **mouse buttons 4–7**: node-hid raw mouse report bits `8/16/32/64` in byte 1 = TopLeft/TopRight/BottomLeft/BottomRight, byte 6 = "Scroll" wheel, byte 7 = "Jog" (AC Pan), signed deltas (`1..127` up, `128..255` down) | kevinschaich `lib/hid-reader.js:4-15,91-103` (platform unstated, node-hid) |
| Linux evdev view of the same: keys **275–278**, `REL_HWHEEL(6)`/`REL_HWHEEL_HI_RES(12)`/`REL_MISC` and `REL_WHEEL(8)`/`REL_WHEEL_HI_RES(11)`; hi-res unit 120 per notch/degree | logilinux `input_monitor.cpp:106-143`, `events.h:14-20`, `README.md:35-39` |
| **No dial click**: no source exposes a press control for the large dial; the Options+ profile store lists exactly 4 press slots and 2 rotate slots (`Loupedeck71`) | `docs/PROTOCOL.md:306-308`; `docs/OPTIONS-PLUS.md:98-106` |
| HID++ protocol version "4.5" | logilinux `README.md:105` (from `dialpad-debug`) |
| Divert state should be restored on exit; SIGTERM must unwind, otherwise the dial "goes mute until something restores them or the device power cycles" | `src/helper.c:279-286,661-668` |
| A BT device that sleeps and reconnects comes back as a **new** HID device object; hold no stale handle | `src/helper.c:511-515` |
| Reply matching: the mouse collection floods report `0x02` while turning; match replies on report id + feature index + function/swid byte, never "first inbound report" | `docs/PROTOCOL.md:314-321`; `src/dial.c:41-49` |
| macOS needs Input Monitoring for the Dialpad only (mouse collection); irrelevant on Linux but explains why probes there open by VID match | `docs/PROTOCOL.md:237-241` |
| Bitfocus Companion: "It does not support the dial yet, this is being worked on" (PR #22 is that work) | `companion/HELP.md` |
| Open question raised by shensquared: can rollers report without being diverted (keep native scroll while also seeing ticks)? | `CLAUDE.md:57-58` |

---

## 7. Additional sources and what they add

### 7.1 shensquared/mx-creative-console (MIT, C, macOS, 2,060 LOC + 595 lines of docs; 0 stars; 2026-08-04)
The most rigorous source. Verified on hardware with **no Logitech software installed**:
- Keypad descriptor: vendor usage page `0xFF43`, 140-byte report descriptor, `MaxInputReportSize 32`, `MaxOutputReportSize 4095`; five top-level collections: `0x1A02 → 0x11` (in+out, 19 B), `0x1A08 → 0x13` (in+out, 31 B), `0x1A10 → 0x14` (**out only**, 4094 B), Generic Desktop System Control `0x04` (in, 1 B), Consumer Control `0x05` (in, 1 B) (`docs/PROTOCOL.md:8-30`). One IOKit handle reaches all three vendor collections; Windows needs three handles (`:32-39`).
- **Feature table (17 entries)**: `0x00 Root, 0x01 IFeatureSet, 0x02 0x0003 DeviceInformation, 0x03 0x0005 DeviceNameAndType, 0x04 0x0008 KeepAlive, 0x05 0x0011, 0x06 0x00c3, 0x07 0x1602, 0x08 0x1802 (reset), 0x09 0x1805 (OOB), 0x0a 0x1807, 0x0b 0x1b04 ReprogControlsV4, 0x0c 0x1e00, 0x0d 0x1e02, 0x0e 0x1eb0, 0x0f 0x8040 BrightnessControl, 0x10 0x92e2, 0x11 0x180b` (`:55-78`). **`0x19A1` is absent**: the LCD is a raw fragment protocol on `0x14`, and implementations naming `0x19A1` hard-code an index (`:80-83`, aimed at mario). **`0x4610` absent** on the Keypad (`:85-86`). Cross-checked against the Options+ profile store (`Loupedeck70`: 9 press, 0 rotate controls; `docs/OPTIONS-PLUS.md:98-106`).
- **Corrections to hcooper's notes** (`:172-183`): byte 3 is `(function<<4)|softwareId`, constant, not a transaction counter: `0x2b` (Julusian) and `0x2d` (Options+ capture) both work; sequencing is the byte-4 bitfield; bytes 9–16 are x,y,w,h and the captured `435×434` was a full-panel paint at (23,6).
- **KeepAlive is what holds the display and the input stream**: without a host pinging `0x0008 fn1 (0x0bb8 = 3000 ms)` the Keypad answers requests but emits no heartbeats, key presses or diverted events; painted images revert to the Logi logo when pings stop; pinging at exactly 3.0 s let the firmware reclaim the panel after ~8 s; **1 s pings hold a paint indefinitely with no repainting** (`:186-216`). "Reports of images reverting after roughly 500 ms trace to an unanswered keepalive." Brightness re-assertion or timed repaint also "held" the display but only as traffic and with visible flicker (`:218-221`; `src/helper.c:163-166,597-627`).
- Input: LCD keys on `0x13` are **not `0x1B04` controls**; `cids` finds exactly two controls, `0x01A1`/`0x01A2`, flags `0x20` (divertable, not persistently divertable → set divert on every start) (`:109-118`). **Echo trap**: own paints/brightness writes come back as `13 ff 02 2b c1 00 01 00` roughly every 5 s, whose byte 6 reads as key 1, so filter on byte 3 ≠ 0 (`:101-107`, `src/helper.c:246-251`). `0x1B04` acks carry the sent function/swid (`11 ff 0b 31 …`); notifications carry `0x00` (`:129-132`). The page-key report and the LCD key report both repeat while held (`src/helper.c:227-231`).
- LCD: "JPEG, up to 300 KB. RGB888 works, RGB565 is unsupported"; "no ACK to wait for"; **fragments need ~5 ms spacing** on macOS or multi-packet frames land only intermittently: "Node implementations get this spacing from their event loop" (`:144-166`, `src/helper.c:208-214`). **JPEG must be exactly the size in the header or nothing draws and every write still succeeds**; ImageIO quality below ~0.75 rejected, 0.85 chosen (`:167-170`).
- Brightness `0x8040` at index `0x0f`, ack `11 ff 0f 2b 00 …` (`:223-226`); `paint.c:118-121` writes `11 ff 0f 2b 00 64` ("the console self-dims to zero when idle").
- `0x00c3` answers only function 0; not a display claim (`CLAUDE.md:55-56`).
- Bridge design: long-lived helper with unix socket, `paintfile` by path because large socket writes drop, 10 s ping to detect a dead client; hot-plug via `IOHIDManagerRegisterDeviceMatchingCallback` (`src/helper.c:553-564`).

### 7.2 mario-gutierrez/mx-creative-console-webhid (no license file; JS; 6 stars; Nov 2025–Mar 2026)
- Clean HID++ 2.0 client (`js/hidpp20.js:74-103`: root query `fn0 [featHi, featLo]` → index; 20-byte `0x11` requests; responses matched by feature index only, 5 s timeout).
- `0x8040` BrightnessControl: fn0 `getInfo` → `maxBrightness` (u16), `steps`, `capabilities`, `minBrightness` (u16); fn1 `getBrightness` (u16); fn2 `setBrightness(u16 BE)` (`js/feature8040.js:5-54`). Percent is mapped between min and max (`js/mxCreativeConsole.js:50-75`), i.e. the unit of Julusian's single byte is **unclear** (Q5).
- `0x1B04`: fn0 count, fn1 `getCidInfo` (cid, tid, flags, fpos, group, gmask, additionalFlags), fn2 `getCidReporting`, fn3 `setCidReporting(cid, reporting|0x03, remap, reporting2)`; event 0 = list of four u16 CIDs; snapshots and restores the previous reporting flags on disconnect (`js/feature1b04.js:33-145`).
- `0x0007` DeviceFriendlyName read in 15-byte chunks (`js/feature0007.js`).
- LCD as "0x19A1 ContextualDisplay" **VLP** frames: the same 20-byte header interpreted as `[1]=deviceIndex ff, [2]=featureIndex 02, [3]=(fn 2<<4)|swid b = 0x2b, [4]=VLP status (bit7 first, bit6 last, bit5 always, bits0-3 seq), [5]=displayIndex, [6]=deferDisplayUpdate, [7]=numImages, [8]=imageFormat (0 JPEG, 1 RGB565, 2 RGB888), [9..16]=x,y,w,h, [17]=len>>16, [18..19]=len&0xffff` (`js/feature19a1.js:147-238`). This reading explains the constant `01 00 01 00` (displayIndex 1, defer 0, numImages 1, JPEG) and makes byte 17 the high byte of a **24-bit length**, consistent with shensquared's "up to 300 KB". Their "all" mode sets **`deferDisplayUpdate = 1` for keys 1–8 and 0 for the ninth** to batch one refresh (`js/mxCreativeConsole.js:728-736`), a plausible fix for Julusian issue #11 (Q9). Whether the device honours the defer bit is unverified by anyone else. Also claims `maxImageFPS 10`, display bounding box 457×440 (`feature19a1.js:37-38,70-80`), no source given.
- Device classification: probes `0x4610` vs `0x19A1` to decide dialpad vs keypad (`mxCreativeConsole.js:187-196`).
- Raw `0x13` key parsing matches Julusian (`:866-897`).

### 7.3 hcooper-idealbuilders/mx-creative-controller (MIT, TS, Windows, 2026-05-13 → 06-12)
- The five captured Options+ LCD frames (`captures/derived/binding-paint-frames.tsv`), decoded with the row-9–14 layout: frame 521 `14 ff 02 2d | e0 | 01 00 01 00 | x=0x0017 (23) y=0x0006 (6) w=0x01b3 (**435**) h=0x01b2 (434) | 00 | len=0x05b4 (1460)`, a single-packet **full-panel** paint; the embedded JPEG SOF0 reads 434×435 (`ffc0 0011 08 01b2 01b3`), so the JPEG dimensions equal the header exactly. Frames 639/640 and 647/648: `a0` (first, not last, **index 0**) + `61` (last, **index 1**), length `0x110b` (4363) = 4075 + 288, the same image painted twice 3.2 ms apart; the two fragments are **151 µs and 78 µs apart**, so Options+ on Windows sends fragments back-to-back, contradicting the "5 ms needed" observation on macOS (Q8). Byte 3 `0x2d` → software id `0xd` (Options+), fn 2.
- Options+ heartbeat `11 ff 04 1d 0b b8` at ~1 Hz (`docs/protocol.md:54-64`); `0x0008 fn0` returns `01 f4 27 10` (500 / 10000, plausibly min/max keep-alive ms, unverified); fn0 called twice at startup (`docs/journal/2026-05-13b…:14-29`). Options+ calls `0x8040` once with `00 46` (=70) (`docs/protocol.md:116`).
- Feature indices from the startup capture agree with shensquared (`0x04, 0x06, 0x07, 0x08, 0x0b, 0x0c, 0x0d, 0x0f`) plus a high index `0xc0 → 0xe019` (`docs/protocol.md:102-117`).
- Their shipping `keypad/src/device.ts` opens Col01/02/03, **does not send a keepalive at all**, listens on Col02 for `0x13`, and re-asserts `11 ff 0f 2b 00 64` at open and every 30 s because "the firmware dims the LCDs to black on its own idle timer (observed after USB suspend AND after ~1h of no button presses): image writes then 'succeed' into a dark panel" (`device.ts:55-64,113,202-211`; `docs/journal/2026-06-11…:50-60`). Also: 2 s write timeout because a wedged handle hangs forever (`:46-53`), exception-safe partial opens (`:91-108`), 450 ms long-press (`:44`), and "1000 ms repaint … still reclaims from Logi Options+'s app-focus repaints" (`journal/2026-05-13c…:25`).
- Windows collection table confirmed by issue #14 on Julusian.

### 7.4 notno/creative-console-daemon (no license file; Rust; Windows; 3,190 LOC; 2026-03 → 06)
- Rust `hidapi` implementation that opens the `0x1A02` collection for HID++ (divert init identical bytes, `src/hid/device.rs:19-28`), `0x1A08` for `0x13` input, `0x1A10` for LCD (`src/hid/lcd.rs:265-283`); byte-identical packet builder and unit tests with expected flag bytes `0xE1 / 0xA1 / 0x22 / 0x63` and multi-packet split `0xA1, 0x62` (`lcd.rs:56-115,355-398`), usable as **independent golden values** for keyvo's packetizer tests. JPEG quality 95 (`lcd.rs:21`). Button positions table (`lcd.rs:25-35`).
- Parser drops `0x11` reports with feature index `0x04` ("heartbeat … battery status", mislabelled) and byte 3 `0x2b/0x3b` acks (`protocol.rs:134-144`); claims LCD keys have CIDs `0x5E–0x65` when diverted (`protocol.rs:220-228`, unexplained, and off by one for 9 keys), but shensquared's `cids` found only two controls, so this is **unclear**.
- No keep-alive, no brightness; "Supervisor script: auto-restart on device disconnect" (README).

### 7.5 kevinschaich/mx-creative-console (no license; JS; Dialpad only; 2026-01): raw mouse-report mapping, see §6.

### 7.6 bitfocus/companion-surface-logitech-mx-creative-console (MIT; TS; 2 stars), a thin wrapper over Julusian 0.3.0: `clearPanel` on init/close, `resetToLogo` on close, 5 ms between key draws, 3 retries, `rotate` events wired for the future dial (`src/instance.ts:48-54,57-66,92-111`); USB ids generated from `DEVICE_MODELS2` (`tools/update-usb-ids.mjs`).

### 7.7 CatalystMonish/mx-console-suite (MIT; Node; macOS; built on Julusian): "tiles are re-sent every 2 s so they never revert to the device logo" (`README.md:309-310`; `launcher/launcher.js:229-230,310-312`) and "Quit Logi Options+ if it grabs the keypad", the third independent report of the revert-to-logo behaviour when only Julusian's calls are used.

### 7.8 Others found by search (skimmed only)
`ayushr27/Logi-logi` (fork of logilinux with a C++ daemon + Tauri GUI; LauzHack submission; no new protocol facts), `rshankras/claude-console` (C# Logi Actions plugin, uses the official SDK, no HID), `morpheus-csmith/flowstate-…`, `cinderspire/context-flow`, `AIoOS-67/agenteyes-mx-console` (DevStudio 2026 plugin submissions), `jamesjingyi/mx-creative-console` (profiles), `vallieres/mx-creative-console-bg-maker` (splits an image into 3×3). None add wire-level facts. GitHub code search for `0xc354`/`c354 logitech` surfaces only the repos above plus `ayushr27`.

---

## 8. Open questions the hardware quick-test must answer

Each item states what to measure and which sources disagree.

1. **Linux HID topology.** How many `/dev/hidraw*` nodes does `046d:c354` create, and which report IDs does each descriptor contain (`HIDIOCGRDESCSIZE`/`HIDIOCGRDESC`)? Expected from macOS (§7.1): one USB interface, 140-byte descriptor, five collections, so likely one hidraw node carrying `0x04, 0x05, 0x11, 0x13, 0x14` plus kernel evdev nodes for the System/Consumer collections. Record `lsusb -v` interface count and `hid-generic` binding. Verify a single node accepts `write()` of 20-, 32- and 4095-byte reports (return values).
2. **Silent-device behaviour.** With no Logi software and no writes, open the node and read for 60 s while pressing keys: are `0x13` key reports emitted (logilinux implies yes on Linux; shensquared says the Keypad is mute until a KeepAlive is sent on macOS)? Repeat after (a) the two divert writes only, (b) one `0x0008 fn1 3000` write.
3. **Init pacing.** Send the two divert writes back-to-back with 0, 10, 20 ms gaps and 0/100 ms settle after open (rows 7); check via `0x1B04 fn2 getCidReporting` (bit 0) whether both diverts took, N=20 runs each.
4. **Keep-alive and revert timing.** Paint 9 keys, then: (a) no traffic, measuring the time until the panel reverts to the logo (500 ms? 8 s? never?); (b) `0x0008 fn1 0x0bb8` every 3.0 s; (c) every 1.0 s; (d) brightness re-assert every 30 s without keep-alive. Also read `0x0008 fn0` (expect `01 f4 27 10`) and try `fn1` with other periods (500, 10000). Confirm whether keep-alive responses (`11 ff 04 …`) arrive at ~1 Hz once started.
5. **Brightness semantics.** `0x8040 fn0 getInfo` → max/min/steps; compare `fn2` with `00 64` (Julusian percent) vs raw max; what does value `00 00` do (Julusian: "resets the device"); ack shape; does `fn1 getBrightness` reflect idle dimming; how long until idle dim (hcooper: ~1 h / USB suspend).
6. **Feature table on Linux.** Enumerate IFeatureSet; confirm indices `0x04 = 0x0008`, `0x0b = 0x1B04`, `0x0f = 0x8040`, absence of `0x19A1` and `0x4610`; note firmware version (`0x0003`) so the table can be compared with shensquared's and hcooper's.
7. **Packet index base and swid.** Paint with first-packet index 1/swid `0xb` (libs) and index 0/swid `0xd` (Options+ capture); also index 0 continuation `0x61`. All accepted?
8. **Fragment pacing on Linux.** For a 12 KB JPEG (3 packets), compare success rate of: one `writev()` (logilinux), back-to-back `write()`s (Options+ sends 150 µs apart), `write()`s with 5 ms gaps (shensquared). N=100 each; success = image visible (photo or manual check per batch).
9. **Multi-key update and ripple.** Paint 9 keys with 0/5/10 ms gaps; does the last key get dropped (issue #11)? Test the VLP interpretation: byte `[6]=1` (deferDisplayUpdate) on keys 1–8 and `0` on key 9: does the panel update atomically?
10. **Full-panel geometry.** Which of these render correctly: 434×434@(23,6) (logilinux), 435×434@(23,6) (Options+), 480×480@(0,0) (Julusian clearPanel), 457×440@(0,0) (mario)? Is anything drawn at x<23 or y<6 visible? What happens with w/h not matching the JPEG SOF dimensions (shensquared: nothing drawn, write succeeds)?
11. **JPEG limits and formats.** Max size: >65,535 bytes with byte 17 = len>>16 (24-bit) vs wrapped u16; quality floor (macOS ImageIO <0.75 rejected: is it size, progressive scan, or subsampling?); 4:2:0 vs 4:4:4; grayscale; progressive; and format byte `[8]` = 1 (RGB565) / 2 (RGB888 raw, 118×118×3 = 41,772 bytes).
12. **Echo/ack reports.** Do `13 ff 02 2b …` echoes of paints appear on Linux (~every 5 s per shensquared)? Confirm the parser must gate on byte 3 == 0 (Julusian gates on `0x2b`; logilinux on `[3]==0`).
13. **Input report details.** Repeat rate of `0x13` while a key is held; meaning/increment of byte 4 (`0xce` seen); max simultaneous keys listed; page-button chord (both held) representation; do page buttons work at all without the divert writes (they should not: "controls are not reported to software until these enable them", PR #22).
14. **Divert persistence.** After unplug/replug or suspend, is the page-button divert flag still set (`fn2`)? (shensquared: volatile, capability flags `0x20` only.)
15. **Reset-to-logo.** Does Julusian's feature report `03 02 00…` do anything on this device (`HIDIOCSFEATURE`)? Does `0x1802` "reset" (index 0x08) do what its name suggests (do not run before Q1–Q13 are recorded).
16. **Dialpad on Linux over Bluetooth.** Does `046d:bc00` get a hidraw node with `0x11` reports and an evdev mouse node; does `hid-logitech-hidpp` bind and change anything; does `0x4610 fn3 [roller, 1]` divert work and stop native scrolling; does `0x1B04 fn3 (cid, 0x03)` on the four CIDs deliver notifications; are feature indices `0x0d`/`0x0a` the same; evdev codes for the two rollers (which is `REL_HWHEEL` vs `REL_WHEEL`, hi-res units).
17. **Dialpad roller sign and scale.** Is roller 0 inverted relative to roller 1 (PR #22)? Deltas per detent at slow/fast speed; increments per rotation 40 / 180 (`fn1`).
18. **Dialpad over Bolt receiver.** PID, device index, and whether the same HID++ features answer via the receiver (nobody has tested).
19. **Coexistence.** With Solaar or `hid-logitech-hidpp` active, are keep-alive/divert writes interfered with? With Options+ absent nothing else should claim the device on Linux; verify no second process holds the node (`fuser /dev/hidraw*`).
20. **hidraw write semantics.** Does the kernel accept a 4095-byte output report through `write()`/`writev()` in one call (logilinux says yes; confirm return value equals `n*4095`), and does `O_NONBLOCK` ever return `EAGAIN` for output reports.

---

## 9. Recommendations for the Rust port (primary source per aspect)

| Aspect | Primary source | Rationale |
|---|---|---|
| Enumeration on Linux | own udev/sysfs code; select the hidraw node by report descriptor (presence of `0x14`), not first-match | rows 3; PR #22's usage-bitmask trick (`usage & 0xff` → report ids from `0x10`) as a cross-check |
| Init (divert page buttons) | Julusian bytes (= logilinux), PR #22 pacing (100 ms settle, 20 ms between), then **verify with `0x1B04 fn2`** | rows 6–7; shensquared: divert is volatile, re-send on every start |
| Keep-alive | shensquared (`0x0008 fn1 0x0bb8` every 1 s, started before any read): treat as **required until Q4 says otherwise**; make period configurable | Julusian/logilinux have none and three downstreams report reverts |
| Brightness | mario's `0x8040` fn0/fn1/fn2 with runtime feature-index lookup; expose percent mapped via getInfo; never send 0 until Q5 | Julusian's hard-coded `0x0f` and percent guess |
| Feature index discovery | mario `hidpp20.js` root-query mechanism, shensquared's reply matching (report id + feature index + fn/swid byte) | both libs hard-code indices; Dialpad proves indices differ per device |
| LCD packetizer | Julusian/logilinux layout (identical); add 24-bit length via byte 17 only after Q11; reject oversize JPEGs; **golden values from notno `lcd.rs` tests and hcooper's captured frames** (`e0/a0/61`, 435×434@(23,6), len fields) | rows 9–15; §7.3 |
| Write strategy | single `writev()` per image (logilinux) as default, with a configurable inter-packet gap fallback (shensquared 5 ms) and a per-key gap (Julusian 10 ms) until Q8/Q9 settle; one writer task, no shared buffers (issue #5) | row 19 |
| Full-panel paints | logilinux 434×434@(23,6) as the safe default; test Options+'s 435×434 | row 17 |
| JPEG encoding | quality 85–95, baseline, dimensions exactly the header's; size guard | shensquared/logilinux/Julusian |
| Input parsing | Julusian's model (report-id dispatch, u16 CID list for `0x11`, int8 list for `0x13`, per-source pressed sets) + shensquared's byte-3≠0 echo filter + logilinux's held-set diff | rows 22–27 |
| Hot-plug / reconnect | fork/PR #8 semantics (POLLHUP/EOF = disconnect, rescan) implemented with udev netlink; shensquared's "BT device returns as a new object" | §3.1, §6 |
| Dialpad | shensquared `docs/PROTOCOL.md` + `helper.c` (feature-index lookup, divert both rollers and four CIDs, restore on exit) with PR #22's inversion note; keep logilinux's evdev path as a non-diverted fallback | §6 |
| Permissions | Julusian's `TAG+="uaccess"` rules (both PIDs) | row 31 |
| Things to ignore | `resetToLogo` feature report, Stream Deck leftovers, notno's `0x5E–0x65` CIDs, mario's `0x19A1` index, logilinux `ref/hidpp` (GPL) | §4.5, §4.9, §7.4, §3.5 |

---

## 10. Credit lines for the README prior-art table

| Project | Line |
|---|---|
| [logilinux](https://github.com/logilinux/logilinux) (Arjun Juneja, Ron0Studios et al., LauzHack 2025, MIT) | C++ library and CLI tools for the MX Creative Keypad and Dialpad on Linux: hidraw/evdev discovery, JPEG upload over report `0x14`, button-report parsing, Dialpad rotation via evdev; keyvo re-derives the Linux plumbing and the input-report parsing from it without copying code. |
| [logilinux-sdk](https://github.com/logilinux/logilinux-sdk) (ron0studios, MIT) | Python/pybind11 SDK over logilinux with a plugin model modelled on Logitech's C# SDK; consulted for API shape only. |
| [node-logitech-mx-creative-console](https://github.com/Julusian/node-logitech-mx-creative-console) (Julian Waller / Bitfocus, MIT) | The earliest public implementation (Oct 2024) of the Keypad's LCD frame format, page-button divert writes and brightness command, in TypeScript for Node and WebHID; keyvo's packet layout and input model are checked against it. |
| [mx-creative-console](https://github.com/shensquared/mx-creative-console) (Shen Shen, MIT) | macOS C probes that verified the HID++ feature table, the `0x0008` keep-alive requirement, `0x1B04` diverts, `0x8040` brightness and the Dialpad's `0x4610` MultiRoller over Bluetooth, and corrected earlier public notes; keyvo's keep-alive and Dialpad design follow its findings. |
| [mx-creative-console-webhid](https://github.com/mario-gutierrez/mx-creative-console-webhid) (Mario Gutierrez, no license file) | Browser WebHID client with runtime HID++ feature discovery (`0x1B04`, `0x4610`, `0x8040`, `0x0007`) and a VLP reading of the LCD header; consulted for the HID++ mechanics, not copied. |
| [mx-creative-controller](https://github.com/hcooper-idealbuilders/mx-creative-controller) (Hunter Cooper, MIT) | Windows controller plus the only public USBPcap captures and journal of the reverse-engineering; its captured Options+ frames serve as golden data. |
| [creative-console-daemon](https://github.com/notno/creative-console-daemon) (Nathan Rosquist, no license file) | Rust/hidapi daemon for Windows whose packetizer unit tests provide independent expected values. |
| [companion-surface-logitech-mx-creative-console](https://github.com/bitfocus/companion-surface-logitech-mx-creative-console) (Bitfocus, MIT) | Bitfocus Companion surface module on top of Julusian's library; reference for draw throttling and retry behaviour. |
