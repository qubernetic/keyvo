# logimap — prior-art analysis for keyvo

Source: `https://github.com/abishekmuthian/logimap`, cloned with `--recursive` to
`scratchpad/logimap` at commit `f0833ea` (2026-07-28). Nothing was modified.
All paths below are relative to that checkout. `file:line` references are to the
checked-out revision. Nothing in this document is copied code; it is a description
of mechanisms so keyvo can re-derive its own implementation.

---

## 1. Facts

| Item | Value | Evidence |
|---|---|---|
| License (actual) | **MIT** (`Copyright (c) 2026 Abishek Muthian`) | `LICENSE:1-3`; `pyproject.toml:12` |
| License (claimed elsewhere) | README and GUI About say **GPL-3.0** — inconsistent with the LICENSE file | `README.md:223`; `logimap/gui/app.py:33` |
| Language | Python 3.10+ (`requires-python = ">=3.10"`), KWin JavaScript, bash; C++17 in submodules | `pyproject.toml:10`; `kwin_script/logimap-focus.js`; `logilinux-sdk/setup.py:27` |
| Version | `0.2.4` in pyproject; README badge still says `0.1.4` | `pyproject.toml:7`; `README.md:3` |
| Size — logimap Python package | ~2,218 LOC across 21 files (`logimap/**/*.py`) | `wc -l` (largest: `gui/app.py` 352, `focus/kwin_dbus.py` 199, `device.py` 192, `render.py` 181) |
| Size — tests | 417 LOC, 4 files (`tests/`) | `wc -l` |
| Size — shell + packaging | `install.sh` 328, `uninstall.sh` 58, `scripts/run.sh` 71, 4 packaging files ≤14 lines each | `wc -l` |
| Size — KWin script | 44 lines | `kwin_script/logimap-focus.js` |
| Size — repo total (non-binary text) | 3,222 lines in top-level project (excl. submodules); `demo/*.mp4` ≈10.7 MB committed | `wc -l`; commit `6c4731b` |
| Size — logilinux C++ (submodule) | 4,592 LOC total; core lib ≈2,300 (`mx_keypad_device.cpp` alone is 962) | `wc -l logilinux/**` |
| Runtime deps | `Pillow>=10.0`, `dbus-next>=0.2.3`; Tk + `python3-pillow-tk` from distro; `pybind11` (build) | `pyproject.toml:25-28`; `install.sh:156-172` |
| Submodules | `logilinux` → **author's own fork** `abishekmuthian/logilinux` (branch `master`, pinned `a9943cd`); `logilinux-sdk` → upstream `logilinux/logilinux-sdk` (`dc79207`, `ignore = dirty`); nested `logilinux-sdk/logilinux-driver` → `ron0studios/LogiLinux` (`12035c9`), replaced by a symlink at install time | `.gitmodules`; `logilinux-sdk/.gitmodules`; `git submodule status --recursive`; `install.sh:202-209` |
| Created / last push | 2026-04-25 / 2026-07-28 | `gh repo view` |
| Commits | 14 on `master`, all by the author | `git log` |
| Stars / forks | 1 / 1 | `gh repo view` |
| Issues | 1 open (#1 "Icons option"), 0 closed; 0 PRs | `gh issue list --state all` |
| Upstream logilinux | MIT, 12 stars, C++, last push 2026-05-16, issues #4-#7 (one open: #5 PacketBufferPool race) | `gh repo view logilinux/logilinux`; `gh issue list` |
| Scope declared | MX Creative Keypad `046d:c354` only; Linux ≥5.10 (uinput); **KDE Plasma 6 Wayland only** | `README.md:18-26` |

---

## 2. Architecture summary

```
KWin (compositor) ──[KWin JS script: workspace.windowActivated → callDBus()]──▶
   D-Bus session bus, service org.logimap.Focus1, method Notify(sssi)
      ──▶ Python daemon (thread "kwin-focus", asyncio + dbus-next)
            ──▶ resolve(profiles, focus) → Profile
            ──▶ render.make_profile_images() → 9 × 118×118 JPEG (Pillow)
            ──▶ KeypadHandle.paint_grid() → logilinux (pybind11) → C++ MXKeypadDevice.setKeyImage()
                  ──▶ writev() of 4095-byte HID output reports to /dev/hidrawN
   ◀── C++ monitor thread reads the same hidraw node, parses button reports, calls back into Python
            ──▶ Daemon._on_button → UinputInjector.press(shortcut) → /dev/uinput EV_KEY events
```

- Process model: one user-session process (`logimap run`), five threads: main (1 s idle loop, signal handling), `kwin-focus` (asyncio loop on a thread), `config-watch` (1 Hz mtime poll), `paint-retry` (2 s), plus the native C++ monitor thread that calls back into Python (`logimap/daemon.py:41-67`; `logimap/focus/kwin_dbus.py:63-88`; `logilinux/lib/src/devices/mx_keypad_device.cpp:375-563`).
- Concurrency control: one `threading.Lock` in the daemon around profile state (`daemon.py:35`) and one `RLock` in `KeypadHandle` serialising every native write (`device.py:50`, `device.py:92-106`).
- No socket/IPC API of its own; the only external surface is the D-Bus method the KWin script calls, plus `SIGHUP` for reload (`daemon.py:44`).
- GUI is a separate Tk process (`logimap gui`) that edits the same JSON and can open the device itself for "Preview on device" (`gui/app.py:305-335`) — there is no hand-off between GUI and daemon; both may open the hidraw node (unclear whether that conflicts; not documented).
- Rendering happens in Python; the C++ layer only chunks and writes JPEG bytes. The device does not scale images (`render.py:11-13`).
- Injection is the daemon's own uinput device; wtype/ydotool/xdotool are probed as fallbacks (`inject/__init__.py:60-91`).
- Config is a single JSON file, hot-reloaded by mtime polling (`daemon.py:76-82`).
- Packaging is a shell installer that builds C++ + pybind11 in a venv and drops udev rules, a launcher, a `.desktop` file and a `systemd --user` unit (`install.sh`).

---

## 3. Protocol facts relied on (from the logilinux submodule and logimap's use of it)

All constants come from the author's fork of logilinux (`logilinux/`), i.e. the LauzHack 2025 reverse-engineering by ron0studios plus the fork's reconnect fixes. logimap itself contains no HID code; it only calls `set_key_image(idx, jpeg_bytes)`, `initialize()`, `has_lcd()`, `start_monitoring()` (`logimap/device.py:59-70,90`).

### Identification and discovery
- Vendor `0x046d`; MX Keypad product `0xc354`; MX Dialpad `0xbc00` — `logilinux/lib/src/core/device_manager.cpp:18-21`, and again hard-coded in `mx_keypad_device.cpp:327`.
- Discovery scans `/dev/input/event*` with `EVIOCGID`/`EVIOCGNAME` (`device_manager.cpp:157-191`) **and** `/dev/hidraw*` with `HIDIOCGRAWINFO`/`HIDIOCGRAWNAME` (`device_manager.cpp:194-231`). hidraw nodes are enumerated by `readdir("/dev")` and sorted numerically (`device_manager.cpp:25-72`; fork commit `9d26fe3` replaced a fixed `hidraw0..19` loop).
- For `MX_KEYPAD`, `Library::findDevice` prefers a hidraw node over an evdev node and **re-scans on every call** (fork commit `9d26fe3`/`de353f1`, `logilinux/lib/src/core/library.cpp`).
- **HID interface / usage page selection: none.** The first `/dev/hidraw*` whose `HIDIOCGRAWINFO` matches VID/PID wins (`mx_keypad_device.cpp:322-334`; `device_manager.cpp:120-138`). Which USB interface that is, and its report descriptor, is **unclear** from this code base.

### Initialisation
- Two 20-byte output reports, written with `write()` and `usleep(10000)` between them (`mx_keypad_device.cpp:130-135`, `594-598`):
  - `11 ff 0b 3b 01 a1 03 00 …` and `11 ff 0b 3b 01 a2 03 00 …` (zero-padded to 20 bytes).
  - Interpretation (mine, not stated in the code): this is the HID++ long-report shape (report ID `0x11`, device index `0xff`), with `0x0b` looking like a feature index and `0x3b` a function/software-id byte, targeting `0xa1`/`0xa2` (the page-button codes, see below). Treat as unverified.
- `hidraw_fd` is opened `O_RDWR` and kept for writes (`mx_keypad_device.cpp:589`). The monitor thread opens the same path separately `O_RDONLY|O_NONBLOCK` (`mx_keypad_device.cpp:395`).
- `initialize()` returns `false` if no hidraw path was found; `hasLCD()` is simply "hidraw path non-empty" (`mx_keypad_device.cpp:585-592`, `659`).

### Image upload (per key)
- `MAX_PACKET_SIZE = 4095` bytes per output report; `LCD_SIZE = 118` (`mx_keypad_device.cpp:23-24`).
- Every packet starts with `14 ff 02 2b` (`PACKET_BASE_HEADER`, `mx_keypad_device.cpp:80`).
- Byte 4 is a part/flag byte: `index | 0x20`, plus `0x80` if first part, plus `0x40` if last part; index is 1-based (`generateWritePacketByte`, `mx_keypad_device.cpp:225-232`).
- **First packet header is 20 bytes** (`PACKET1_HEADER = 20`, `mx_keypad_device.cpp:140`), laid out as (`mx_keypad_device.cpp:164-178`):
  - `[0..3]` = `14 ff 02 2b`
  - `[4]` = flag byte
  - `[5..8]` = `01 00 01 00` (from `PACKET1_GEOMETRY`, `mx_keypad_device.cpp:81`; its last two bytes are overwritten by x)
  - `[9..10]` = x (big-endian u16), `[11..12]` = y, `[13..14]` = width (118), `[15..16]` = height (118)
  - `[17]` = 0
  - `[18..19]` = JPEG byte length, **big-endian u16** → implicit ≤65,535-byte JPEG limit.
  - Payload: up to `4095-20 = 4075` JPEG bytes.
- **Subsequent packets**: 5-byte header (`14 ff 02 2b <flag>`) + up to `4090` bytes (`SUBSEQUENT_HEADER = 5`, `mx_keypad_device.cpp:141,200-220`).
- All packets are zero-padded to exactly 4095 bytes (`memset`, `mx_keypad_device.cpp:182,204`) and sent with a single `writev()`; success is `totalWritten == packet_count * 4095` (`mx_keypad_device.cpp:616-645`). The fd is toggled to `O_NONBLOCK` for the attempt and falls back to a blocking `writev` on `EAGAIN` (`626-641`).
- **Key geometry** (row-major 3×3): `x = 23 + col*(118+40)`, `y = 6 + row*(118+40)` (`mx_keypad_device.cpp:158-161`). Full-screen canvas is 434×434 at origin (23, 6) = `118*3 + 40*2` (`mx_keypad_device.h:41-44`, `mx_keypad_device.cpp:661-665`); `setRawImage(x,y,w,h,jpeg)` exists for arbitrary rectangles (`mx_keypad_device.cpp:667-706`).
- **Image format**: baseline JPEG; the device does **not** scale, so a 90×90 image only fills the top-left of the 118×118 destination — that was logimap bug fixed in `6f9697e` (`render.py:11-14`; commit `6f9697e`). logimap renders RGB 118×118 JPEG quality 85 with Pillow (`render.py:14-15,144-146`).
- `setKeyColor` is a stub returning `false` (`mx_keypad_device.cpp:648-657`). GIF animation is implemented by re-uploading JPEG frames from a per-key thread (`mx_keypad_device.cpp:708-762`).

### Keep-alive, brightness, sleep
- **No keep-alive/heartbeat and no brightness control exist anywhere** in logimap or the logilinux fork (grep for `bright|keep.alive|heartbeat|idle|wake` across `logilinux/lib`, `logilinux/tools`, `logimap/` returns only poll-timeout comments). Whether the keypad dims/sleeps without traffic is **unclear** from this code base.

### Input reports (button events)
- Read loop: `poll()` with 100 ms timeout on the hidraw fd, 256-byte buffer (`mx_keypad_device.cpp:401-429`). The fork added `POLLERR|POLLHUP|POLLNVAL`, `read()==0` and non-`EAGAIN` error handling that breaks out of the loop (`mx_keypad_device.cpp:421-440`; commit `9d26fe3`), which fixed a 100 % CPU spin after unplug.
- **Page buttons (P1/P2)** — checked first (`mx_keypad_device.cpp:442-485`):
  - Press: `11 ff 0b 00 01 a1` (P1, left) or `11 ff 0b 00 01 a2` (P2, right).
  - Release: `11 ff 0b 00 00 00`; the library remembers `last_p_button` to know which one released (`mx_keypad_device.cpp:122,455,468-484`).
  - Enum: `P1_LEFT = 0xa1`, `P2_RIGHT = 0xa2` (`logilinux/lib/include/logilinux/events.h:32-33`).
  - Comment warns that P-button packets carry **spurious grid data at byte 6**, so grid parsing is skipped for those packets (`mx_keypad_device.cpp:444-446,488`).
- **Grid keys** (`mx_keypad_device.cpp:487-555`):
  - Report shape `13 ff 02 00 xx 01 [codes…]` — matched on `[0]==0x13, [1]==0xff, [2]==0x02, [3]==0x00, [5]==0x01`; byte 4 is ignored.
  - Bytes 6+ list **all currently held** key codes `1..9`, terminated by `0`; the library converts to `0..8` and diffs against the previous set to synthesise per-key press/release events (so multi-key chords on the pad are observable).
  - Enum `GRID_0..GRID_8 = 0..8` (`events.h:23-31`); helper `getMXKeypadButton(code)` maps `0..8`, `0xa1`, `0xa2` (`events.h:116-128`).
- Timestamps are `steady_clock` milliseconds (`mx_keypad_device.cpp:460-463`).
- logimap uses only `pressed == True` events and ignores releases and P1/P2 entirely (`logimap/device.py:141-153`; `device.py:85-89` refuses to paint indices outside 0..8).

### Known protocol-layer bug they had to work around
- `PacketBufferPool`: a static 16-slot ring with `fetch_add % 16` and **no release** (`mx_keypad_device.cpp:83-103`). Packets are copied out on `push_back`, but the pool itself is written by any thread without locking, so two concurrent `setKeyImage` calls can corrupt a buffer between `memset` and copy. Reported upstream as `logilinux/logilinux#5` (by GarThor, comment by logimap's author); logimap's mitigation is to hold an `RLock` around the whole 9-key repaint (`logimap/device.py:92-106`; commit `1fae018`).

---

## 4. Focus detection (KDE Plasma 6 / KWin)

### Mechanism
- A KWin **JavaScript** script (`kwin_script/logimap-focus.js`) connects to `workspace.windowActivated` (Plasma 6 API name; `.js:37-39`) and, for each activated window, calls **a D-Bus method on the daemon** (not a signal): `callDBus("org.logimap.Focus1", "/org/logimap/Focus1", "org.logimap.Focus1", "Notify", resourceClass, resourceName, caption, pid)` (`.js:23-29`). Signature `ssss i` → Python `Notify(s, s, s, i)` (`logimap/focus/kwin_dbus.py:34-41`).
- Fields used from the KWin `window` object: `resourceClass`, `resourceName`, `caption`, `pid` (`.js:17-20`). `resourceClass` on Plasma 6 Wayland is the **reverse-DNS app id** (e.g. `org.mozilla.firefox`), which README explicitly warns users about (`README.md:86-88`, `198`).
- On script load it emits once for `workspace.activeWindow` so the daemon paints the right profile at startup (`.js:41-44`).
- The daemon **owns** the bus name `org.logimap.Focus1` on the session bus and exports the object with `dbus-next` (`kwin_dbus.py:21-23,93-102`). If the daemon is absent, the script's `callDBus` is a no-op / caught exception (`.js:12-13,30-32`).
- The script prints to KWin's stdout (`.js:21,35`); where that lands in the journal is not documented (unclear).

### Installation of the script
- Loaded dynamically through KWin's scripting D-Bus API, not as a kpackage (`kwin_dbus.py:114-161`):
  1. `org.kde.KWin` `/Scripting` `org.kde.kwin.Scripting.isScriptLoaded(s)` with plugin name `logimap-focus`;
  2. if not loaded: `loadScript(ss)` with the absolute path of the `.js` file (resolved from the Python package location, `kwin_dbus.py:25`) → returns an integer script id;
  3. `org.kde.KWin` `/Scripting/Script<id>` `org.kde.kwin.Script.run()`.
- This is done by the daemon on start (`kwin_dbus.py:104-107`) and also exposed as `logimap install-kwin` (`kwin_dbus.py:185-199`; `__main__.py:19,43-45`). README calls this "one-time per Plasma session" (`README.md:34`), i.e. the script does **not** persist across KWin restarts; there is no watch on KWin name-owner changes to re-load it (unclear whether a KWin crash silently kills focus tracking — nothing handles it).
- Uninstall tells users to disable it via System Settings → KWin Scripts (`uninstall.sh:56-57`).

### Debounce / dedup / edge cases
- **No debounce.** Every activation triggers `Notify` immediately. Dedup happens downstream by comparing the *resolved profile name* to the currently painted one (`daemon.py:93-103`), so alt-tabbing between two windows of the same profile does not repaint.
- `pid <= 0` → `None` (`kwin_dbus.py:40`); null window guarded (`.js:16`).
- If the daemon cannot become primary owner of the bus name it logs a warning and keeps running (`kwin_dbus.py:98-100`); README's troubleshooting maps `RequestNameReply.IN_QUEUE` to "a second daemon is running" (`README.md:203`).
- Focus source start-up has a 5 s readiness timeout (`kwin_dbus.py:69-70`).
- The package docstring mentions an "xprop poll fallback" (`logimap/focus/__init__.py:1`) but **no such implementation exists**; only `KWinFocusSource` is present.
- The environment gate in the installer is `XDG_SESSION_TYPE == wayland` and `XDG_CURRENT_DESKTOP` containing `KDE` (`install.sh:66-72`).

---

## 5. Input injection

### Chord grammar (`logimap/inject/_parse.py`)
- Tokens joined by `+`, case-insensitive, whitespace-tolerant; the **last token is the key**, all earlier tokens must be modifiers (`_parse.py:41-75`).
- Modifiers: `ctrl`, `control`, `shift`, `alt`, `meta`, `super`, `logo` (`_parse.py:7`); aliases `control→ctrl`, `super→logo`, `meta→logo` (`_parse.py:9-13`). Canonical set is therefore `ctrl/shift/alt/logo`.
- Duplicate modifiers are de-duplicated (`_parse.py:63-65`); a chord consisting only of modifiers is an error (`_parse.py:67-68`); unknown modifier → error.
- **Shifted US-keyboard symbols are rewritten at parse time**: `? → shift+/`, `" → shift+'`, `{ → shift+[`, `! → shift+1`, etc. (table `_parse.py:19-24`, applied at `70-73`). Rationale: `?` has no evdev keycode.
- **Multi-chord sequences** use `,` as separator: `ctrl+b,"` (tmux) → two chords (`_parse.py:78-101`); empty chord / leading / trailing comma → error. Added in `a2bd6e0` specifically for tmux.
- Tests cover all of the above (`tests/test_shortcut_parse.py`).

### Timing
- 50 ms sleep between chords, in every backend (`uinput.py:131-133`; `wtype.py:70-71`; `ydotool.py:39-40`; `xdotool.py:38-39`). No configurable delay, no hold/repeat, no per-key press duration.
- 50 ms settle after `UI_DEV_CREATE` "so udev/Wayland have routed the new device" (`uinput.py:111-113`).

### uinput device (`logimap/inject/uinput.py`)
- Opens `/dev/uinput` `O_WRONLY|O_NONBLOCK` (`uinput.py:93`), sets `EV_KEY` + `EV_SYN` evbits, registers **every** keycode from its tables via `UI_SET_KEYBIT` (`uinput.py:95-98`).
- `uinput_setup`: `BUS_VIRTUAL (0x06)`, vendor `0x1209` (pid.codes), product `0xCAFE`, version 1, name `"logimap virtual keyboard"` (`uinput.py:100-108`).
- ioctl numbers and struct sizes are hand-computed and asserted (`input_event` = 24 B, `uinput_setup` = 92 B on x86_64; `uinput.py:37-59`, `tests/test_uinput.py:25-27`) — portable only to LP64 archs.
- Event order per chord: each modifier down + `SYN_REPORT`, key down + SYN, key up + SYN, modifiers up in reverse + SYN (`uinput.py:64-83`).
- Keycode table (`_keycodes.py`): left-variant modifiers only (`KEY_LEFTCTRL 29`, `KEY_LEFTSHIFT 42`, `KEY_LEFTALT 56`, `KEY_LEFTMETA 125`; `_keycodes.py:10-15`); letters, digits, Return/Esc/Tab/Space/Backspace/Delete/Home/End/PgUp/PgDn/arrows/Insert/Menu, US punctuation, F1-F12 (`_keycodes.py:17-38`). **Missing**: keypad, F13+, media/consumer keys, Print/ScrollLock/Pause, right-hand modifiers, AltGr, non-US keys.
- Device is destroyed via `UI_DEV_DESTROY` in `close()` registered with `atexit` (`uinput.py:90,137-148`).

### Backend probe order and rationale (`logimap/inject/__init__.py`)
1. uinput if `/dev/uinput` opens writable (`:37-43,60-68`).
2. `wtype` only if `wtype ""` exits 0 — used as a live probe of `zwp_virtual_keyboard_v1` availability; **KWin 6 does not expose it to non-IME clients**, so wtype is dead on Plasma (`:46-57`, `:8-9`; `uinput.py:3-4`).
3. `ydotool` (needs root daemon; flagged unmaintained), 4. `xdotool` (XWayland only) (`:77-85`).
- If nothing works the daemon still runs and logs "no injector available" on every press (`daemon.py:28-32,148-150`).

### Permissions and onboarding for injection
- udev rule: `KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"` (`packaging/99-logimap-uinput.rules:8`).
- `sudo usermod -aG input $USER` (`install.sh:247-249`), plus an immediate **ACL bridge** `setfacl -m u:$USER:rw /dev/uinput` so the user can test before re-login (`install.sh:250-255`); a clear end-of-install warning explains that re-login is required for persistence (`install.sh:308-315`; `README.md:39-41`).
- README's troubleshooting gives the same one-shot `setfacl` as a fix for `No usable keystroke injector found` (`README.md:200`).
- Rules are reloaded with `udevadm control --reload` and `udevadm trigger --name-match=uinput` (`install.sh:237-239`).

---

## 6. Profile model

### File and schema (`logimap/config.py`, `logimap/profiles.py`)
- Path: `$XDG_CONFIG_HOME/logimap/profiles.json` (default `~/.config/logimap/profiles.json`) (`config.py:17-18`).
- Top level: `{"version": 1, "default_profile": "default", "profiles": {name: Profile}}` (`config.py:19,56-60`).
- Profile: `{"match": {"wm_class": str, "title_regex": str|null}, "bg_color": [r,g,b], "fg_color": [r,g,b], "keys": {"GRID_0".."GRID_8": {"title": str, "shortcut": str}}}` (`profiles.py:76-95`); defaults `bg (30,30,30)`, `fg (255,255,255)` (`profiles.py:69-70`); key names fixed to the 9 grid keys (`profiles.py:9-13`). Page buttons have no binding slot.
- Only `title` + `shortcut` per key. No icons (open issue #1), no per-key colours, no actions other than a keystroke, no pages (README roadmap item 1, `README.md:227`).

### Matching
- `wm_class`: `"*"` wildcard, otherwise **case-insensitive exact equality** with KWin's `resourceClass` (`profiles.py:45-48`). No globbing, no regex, no list.
- `title_regex`: optional, `re.search(..., caption, IGNORECASE)` (`profiles.py:49-51`).
- `resolve()`: iterate `profiles` in dict order, skip the default, return the **first** match; else the default (`profiles.py:98-105`). Because `save()` writes with `sort_keys=True` (`config.py:61`), after a round-trip the effective priority is **alphabetical by profile name** — an implicit, undocumented rule.
- Fallback: a `default` profile is always present — created on first load, injected if missing, protected from deletion in the GUI (`config.py:33,46-47`; `gui/app.py:202-204`). The GUI defaults a new profile's `wm_class` to the profile name (`gui/app.py:168`).

### Persistence
- Atomic save: temp file in the same dir + `os.replace` (`config.py:62-68`).
- Invalid JSON → logs an exception and proceeds with an in-memory empty config (does not overwrite the file until the user saves) (`config.py:36-40`).
- Missing/unknown fields tolerated via `.get()` defaults; no schema validation, no migration logic despite the `version` field.

### Hot reload
- `config-watch` thread polls `st_mtime` once per second and calls `_reload()` (`daemon.py:76-82`); `SIGHUP` triggers the same (`daemon.py:44`).
- On reload it re-resolves the **last seen focus** and repaints if the profile changed (`daemon.py:69-74`).
- A generic `config.watch()` helper exists (`config.py:78-94`) but the daemon uses its own copy of the loop — dead code.

---

## 7. UX and onboarding ideas worth adopting

**install.sh** (`install.sh`)
- Environment probe with explicit override: refuse unless Wayland + KDE, `--force` to proceed (`:64-72`).
- Version gate: compares installed `logimap --version` vs `pyproject` with `sort -V`; `eq` skips the heavy rebuild, `gt` requires `--downgrade`, `--force` reinstalls (`:44-97`). Runs the version probe from `/` so stale egg-info in the checkout cannot shadow the install (`:48-53`, commit `e8755f6`).
- Refreshes package metadata **before** failure-prone steps so `--version` never lies even if a later step aborts under `set -e` (`:99-134`).
- Picks the Python interpreter whose ABI tag matches the already-compiled `.so`, else the newest available (`:106-125`; mirrored in `scripts/run.sh:22-40`).
- Distro detection via `ID`/`ID_LIKE` for Fedora/Debian/Arch with a "patches welcome" pointer for others (`:136-149`).
- Idempotent, re-runnable; restarts an active systemd unit after upgrade and warns about a foreground `logimap run` that must be restarted manually (`:294-302`).
- Warns if `~/.local/bin` is not on `PATH` (`:273-276`).
- Ends with a **"Next steps" block** listing the exact commands, the log path, the config path and the uninstall command (`:316-327`).
- `uninstall.sh` explicitly lists what it did *not* remove (distro packages, group membership, config/logs, the KWin script) with the manual commands (`uninstall.sh:45-58`).
- udev `udevadm trigger --subsystem-match=hidraw` after installing rules so no replug is needed (`:238`).

**Discovering the window class**
- The single best onboarding trick: the daemon logs every focus change as `focus -> wm_class='…' resource_name='…' pid=… caption='…'` (`daemon.py:85-88`), and README tells users to `tail -F ~/.local/state/logimap/logimap.log | grep 'focus ->'` while alt-tabbing to copy the class verbatim (`README.md:78-88`).

**Troubleshooting table** (`README.md:195-205`) — symptom → first check, including:
- `busctl --user status org.logimap.Focus1` to verify the daemon owns the bus name (`README.md:191-192`);
- `lsusb | grep 046d:c354` and checking `/dev/hidraw*` mode `crw-rw-rw-` to detect a udev rule that did not apply (`README.md:202`);
- `journalctl --user -u logimap -n 50` for unit failures (`README.md:205`);
- the `ModuleNotFoundError: _logilinux_native` → Python ABI mismatch mapping (`README.md:204`).

**Logging** (`logimap/_logging.py`)
- stderr + `RotatingFileHandler` 1 MiB × 3 at `$XDG_STATE_HOME/logimap/logimap.log` (`:10-11,39`); `-v` for DEBUG and `--no-log-file` (`__main__.py:22-27`); the log path is printed at start (`__main__.py:34-35`). Log lines are designed to be grep targets: `focus ->`, `painting profile:`, `press GRID_N has no binding in profile '…'`, `firing GRID_N -> shortcut`, `using built-in uinput injector` (`daemon.py:85,115,146,151`; `inject/__init__.py:65`).

**CLI** (`logimap/__main__.py`)
- Sub-commands `gui`, `run`, `install-kwin`, `paint-test` (`:17-20`). `paint-test` paints 9 hard-coded titles and logs presses for 30 s — a hardware smoke test without the daemon (`device.py:156-192`).

**GUI** (`logimap/gui/`)
- 3×3 preview grid renders the **same JPEG bytes the device will get**, scaled to 72 px, so what you see is what the LCD shows (`keypad_grid.py:27,108-116`).
- Shortcut capture widget: click "Capture", press a chord, get the canonical string; the entry stays editable for chords the compositor grabs (Meta+L etc.) and a hint line documents the comma syntax (`shortcut_capture.py:65-71,91-95`; `README.md:196-199`). Modifier detection uses Tk `event.state` masks including `0x00040000` for Mod4/logo (`shortcut_capture.py:9-14`).
- "Preview on device" pushes the current profile to the hardware from the editor (`gui/app.py:305-335`).
- Duplicate profile, delete-protection for `default`, About dialog with version read from `pyproject.toml` (`gui/app.py:174-212,283-293`).

**Rendering** (`logimap/render.py`)
- Auto font sizing: try sizes 20→9, accept the first that fits in ≤3 lines without breaking a word mid-character (`:18,70-89`); greedy word-wrap with char-wrap fallback flag (`:34-67`).
- **Shared font size across a whole profile** so keys do not look like a "salad of sizes" (`:152-174`; `tests/test_render.py:95-114`).
- Optional subtitle in an accent colour (`:126-142`) — implemented but unused by the daemon.
- Font discovery from three distro-specific DejaVu paths with PIL default as fallback (`:20-31`).

**systemd / desktop**
- `systemd --user` unit tied to `graphical-session.target` (`After`, `PartOf`, `WantedBy`), `Restart=on-failure`, `RestartSec=2` (`packaging/logimap.service`); installed but **not enabled** — the user opts in (`install.sh:285-292`, `README.md:69-70`).
- `.desktop` entry under `Categories=Settings;HardwareSettings;` with `Keywords` (`packaging/logimap.desktop`).

**Resilience**
- Paint deferral: if the device is absent, the wanted profile becomes `pending` and a 2 s retry thread paints it once the device returns; it also detects a dead monitor thread and re-queues the current profile (`daemon.py:105-136`).
- Reconnect-on-failure wrapper around every device op (`device.py:108-130`).

---

## 8. Problems they hit (design around these)

From the git log (14 commits), issues, and code comments:

1. **LCD image size wrong** — rendered 90×90, device does not scale, only the top-left of each 118×118 key was filled. Fixed in `6f9697e` (0.2.2). → keyvo must render exactly 118×118 (or use `setRawImage`-style geometry) and test image dimensions.
2. **Race in the C++ packet pool** — `logilinux/logilinux#5`; mitigated in `1fae018` by serialising all repaints under an `RLock` (`device.py:92-106`). → keyvo: single writer task/actor for the device; never share static buffers.
3. **100 % CPU spin after unplug** — upstream monitor loop ignored `POLLHUP`/read errors. Fixed in the fork (`9d26fe3`, 2026-07-09) and the daemon gained paint-retry/pending logic (`e8755f6`). → keyvo: treat `POLLHUP`/`EIO`/`ENODEV` as disconnect; use a udev/netlink monitor for hot-plug instead of polling.
4. **`std::terminate` / `SIGABRT` on reconnect after a KVM switch** — reassigning a still-joinable `std::thread`; fixed with a mutex + join and a regression test (`de353f1`; `logilinux/tests/mx_keypad_monitor_restart_test.cpp`). → keyvo: model device lifetime explicitly (Rust ownership makes this a compile-time concern).
5. **Stale device objects after reconnect** — `findDevice` cached results; fork now re-scans on every call and enumerates `/dev/hidraw*` dynamically instead of `hidraw0..19` (`9d26fe3`, `de353f1`; fork README "Fork-Specific Reconnect Fixes").
6. **Reconnect blocks the focus thread** — `_with_retry` sleeps 2 s while holding the device lock (`device.py:122-123`), so a focus change during an unplug stalls the D-Bus handler thread. → keyvo: never sleep under a lock; reconnect asynchronously.
7. **Multi-chord shortcuts (tmux) impossible** — added comma syntax and 50 ms inter-chord delay in `a2bd6e0`; users could not type `"` because it is not a keycap → shifted-symbol table. → keyvo: design the action grammar for sequences, delays, holds and text from day one.
8. **Compositor-grabbed shortcuts cannot be captured** in the GUI (Meta+L, Meta+D) → manual entry path (`shortcut_capture.py:66-71`; `README.md:199`). → keyvo GUI must always offer a text form and validate it.
9. **Submodule/stub hell** — the SDK's nested `logilinux-driver` submodule must be replaced by a symlink, which makes `git submodule update --recursive` fail on re-install; the installer therefore runs non-recursive init and hand-fixes it (`6c4731b`, `7d34ac2`; `install.sh:175-209`). Python ABI mismatch between the compiled `.so` and the interpreter produced `ModuleNotFoundError: _logilinux_native` (`README.md:204`). → keyvo: one static Rust binary, no build-from-source install path for users.
10. **Version drift / stale metadata** — `logimap --version` reported the old version after an upgrade because egg-info in the checkout shadowed the venv; installer now runs the probe from `/` and refreshes metadata first (`e8755f6`). README badge (`0.1.4`) and license text (GPL vs MIT) are still inconsistent (`README.md:3,223` vs `LICENSE:1`, `pyproject.toml:12`).
11. **Launcher hard-codes the checkout path** (`install.sh:263-269` writes `LOGIMAP_ROOT="<clone dir>"` and `LD_LIBRARY_PATH` into `~/.local/bin/logimap`); moving or deleting the clone breaks the install.
12. **udev rule grants world-writable hidraw** (`MODE="0666"`, `packaging/99-logitech-creator.rules:9-10`), acknowledged in a comment; the SDK's own script uses `TAG+="uaccess"` (`logilinux-sdk/scripts/setup_permissions.sh:9-12`).
13. **No interface selection** — first hidraw node with matching VID/PID (`mx_keypad_device.cpp:322-334`). Fragile if the keypad exposes multiple HID interfaces (unclear whether it does).
14. **16-bit JPEG length field** in the first packet header (`mx_keypad_device.cpp:177-178`) — large/high-quality images would silently wrap; nothing checks the size.
15. **Alphabetical profile priority** after save (`config.py:61` + `profiles.py:100-104`) — surprising for overlapping matches (e.g. a `title_regex` profile vs a plain-class profile).
16. **P1/P2 page buttons parsed but unused**; only 9 bindings per app; pagination is the sole roadmap item (`README.md:225-227`).
17. **Icons requested** (issue #1, open since 2026-05-14, unanswered).
18. **KWin script is session-scoped** — loaded through `loadScript` each daemon start, lost on KWin restart; no re-load trigger (`kwin_dbus.py:104-107`; `README.md:34`).
19. **Dead/misleading bits**: xprop fallback mentioned but absent (`focus/__init__.py:1`); `config.watch()` unused (`config.py:78`); commit `b496ded` is titled "fixed keycode errors" but only touches `gui/app.py`, `pyproject.toml` and `FUNDING.yml`.
20. **GUI and daemon may both open the device** ("Preview on device") with no coordination (`gui/app.py:319-334`) — behaviour unclear/undocumented.

---

## 9. What keyvo should do differently or better

- **Single static Rust binary**; own hidraw layer (no C++ submodules, no venv, no ABI pinning). Enumerate via udev/sysfs, select the hidraw node by **interface number / report descriptor**, not first-match on VID/PID.
- **One device actor** owning the fd with an mpsc queue; coalesce repaints (drop superseded frames), never sleep under a lock, no shared static buffers. Verify `writev` total against `n × 4095` like logilinux does, but also **reject JPEGs > 65,535 bytes** up front (16-bit length field).
- **Hot-plug via udev netlink** (or `inotify` on `/dev`) instead of a 2 s retry loop; treat `POLLHUP/EIO/ENODEV` as disconnect immediately; keep the wanted frame buffer and replay it on reconnect (logimap's "pending profile" idea, done event-driven).
- **Investigate keep-alive and brightness separately** — nothing in this code base knows about either; needs a USB capture of Logi Options+ (mark as unknown in keyvo's protocol doc rather than assume).
- **Use the page buttons**: `0xa1/0xa2` press/release are already decodable; keyvo pages per profile is exactly what logimap's roadmap wanted. Handle the "P-button packets contain spurious grid data" quirk.
- **Use key release + multi-key state**: the grid report carries the full held-set; keyvo can offer hold/long-press/chord actions cheaply.
- **Focus source**: keep the KWin-script→D-Bus method-call approach (it is the only thing that works on Plasma 6 Wayland), but (a) ship the script as an installable KWin script package *and* support dynamic `loadScript`; (b) watch `NameOwnerChanged` for `org.kde.KWin` and re-load after a KWin restart; (c) add a small **debounce (~30-100 ms)** to collapse alt-tab bursts; (d) keep logimap's "log the class on every focus change" trick and expose it as `keyvo focus --watch` / socket event stream; (e) abstract the source so GNOME/Hyprland/sway can be added.
- **Matching**: explicit priority (`priority = N` or list order preserved in TOML), glob/regex on class, optional `resource_name` and `pid`-based matching, plus a `title_regex`. Document priority.
- **Profiles in TOML** with schema version *and* migration; validate on load with precise error positions; keep atomic writes; hot-reload with inotify (fallback to mtime poll) plus `SIGHUP` and a socket `reload` command.
- **Action grammar**: chords, sequences, per-step delays, hold/release, text typing, shell commands, socket calls; keep logimap's alias set (`ctrl/control`, `super/meta/logo`) and the shifted-symbol rewrite because users type `ctrl+?` naturally; full evdev keycode table (keypad, F13-F24, media/consumer keys, right-side modifiers).
- **uinput device** with a stable name/vendor/product (use a keyvo-specific product id), created once at start; consider also exposing a consumer-control (media) capability. Keep the 50 ms post-create settle as a documented constant, but make inter-chord delay configurable.
- **Permissions**: prefer `TAG+="uaccess"` (systemd-logind ACLs) over `MODE="0666"` for hidraw; keep the `input` group + `setfacl` bridge idea for `/dev/uinput`, and make `keyvo doctor` report group membership, ACL state, hidraw mode, uinput writability, KWin script loaded state, and bus-name ownership (all the checks logimap spreads across README, install.sh and log lines).
- **Rendering**: adopt the shared-font-size-per-page idea and the "no mid-word break" fitting loop; add icons (issue #1), cache rendered tiles by content hash, render off the device thread.
- **Onboarding**: keep the "Next steps" epilogue, the symptom → check table, a `paint-test`-style hardware smoke test, and a version-gated idempotent installer — but as `keyvo install`/`keyvo doctor` subcommands, not a bash script that depends on a git checkout.
- **Packaging**: systemd user unit tied to `graphical-session.target`, installed-not-enabled; do not hard-code source paths; single license and consistent version metadata.
- **GUI**: keep "preview shows the real bytes" and text-first shortcut entry with capture as a helper, but route device access **through the daemon socket** instead of opening the hidraw node from the GUI.

---

## 10. Credit line for the README "Prior art" table

| [logimap](https://github.com/abishekmuthian/logimap) (Abishek Muthian, MIT) | Python per-app profile daemon for the MX Creative Keypad on Plasma 6 Wayland; keyvo re-implements, without copying code, its KWin-script→D-Bus focus feed, uinput chord-injection design, paint-retry/reconnect behaviour and installer/troubleshooting onboarding, and relies on the HID report layout reverse-engineered by [LogiLinux](https://github.com/logilinux/logilinux) (ron0studios et al., MIT) that logimap builds on. |
