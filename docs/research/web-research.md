# keyvo web research: everything outside logimap / logilinux / node-logitech-mx-creative-console

Date: 2026-09-09. Scope: forums, issue trackers, wikis, docs, product pages, firmware/Options+ notes and
*other* GitHub repos. The three code bases named above are covered by separate reports and are only
referenced here where an external source adds a fact. "unclear" means the sources are silent.

Method: WebSearch, GitHub API (`gh`), direct fetch of docs/wiki pages, HN Algolia API, Lemmy API,
kernel/systemd/Solaar sources. Reddit blocks our crawler (no reddit.com results could be fetched).

---

## 1. Reverse-engineering and protocol knowledge beyond the three known code bases

### 1.1 shensquared/mx-creative-console (macOS, raw IOKit HID + Bluetooth HID++) - richest new source

- USB 046d:c354, vendor usage page 0xFF43, three vendor collections: usage 0x1A02 -> report 0x11 (HID++ short,
  19+1 B), 0x1A08 -> report 0x13 (HID++ long, 31+1 B), 0x1A10 -> report 0x14 (LCD, 4094+1 B, out only); plus
  Generic Desktop System Control (0x04) and Consumer Control (0x05) in-reports.
  https://github.com/shensquared/mx-creative-console/blob/main/docs/PROTOCOL.md
- Feature table (17 features) enumerated via IFeatureSet: 0x0000, 0x0001, 0x0003, 0x0005, **0x0008 KeepAlive (idx 0x04)**,
  0x0011, 0x00c3, 0x1602, 0x1802 reset, 0x1805 OOB, 0x1807, **0x1b04 ReprogControlsV4 (idx 0x0b)**, 0x1e00,
  0x1e02, 0x1eb0, **0x8040 BrightnessControl (idx 0x0f)**, 0x92e2, 0x180b. 0x19A1 is NOT present ("public
  write-ups that name 0x19A1 hardcode an index"); 0x4610 MultiRoller is NOT present (keypad has no rotary). Same URL.
- LCD keys arrive raw on report 0x13 (`13 ff 02 2b`, byte 4 = seq/index bitfield), NOT as 0x1B04 controls;
  the two page buttons ARE 0x1B04 controls, CIDs 0x01A1 / 0x01A2, flags 0x20 divertable; diverted presses
  repeat while held (edge detection needed). Same URL.
- LCD paint: key 118x118 px, gap 40 px, panel 480x480, origin (23,6), key x = 23 + col*158, y = 6 + row*158.
  Format JPEG up to 300 KB, RGB888 works, RGB565 unsupported. Header bytes 9-16 are four big-endian u16
  x,y,w,h (a 435x434 capture was a full-panel paint). Byte 3 (0x2b / 0x2d) is the HID++ function+swid byte,
  not a transaction counter. Same URL.
- **Multi-packet frames need ~5 ms pause between fragments**; the JPEG must be exactly the length in the
  header; JPEG quality below ~0.75 was rejected by the device ("draws nothing and still reports success").
  Same URL.
- **Keepalive semantics** (the most important fact for a daemon): feature 0x0008 function 1 with parameter
  0x0BB8 = 3000 ms. With nobody pinging, the keypad answers requests but emits no key presses, no diverted
  events, no heartbeats. After `keepAlive(3000)` the device streams heartbeats ~1/s. When the host stops,
  the LCDs revert to the firmware idle screen (Logi logo on centre key); reports of images "reverting after
  ~500 ms" trace to an unanswered keepalive. Pinging at exactly 3.0 s still lost the panel after ~8 s;
  **pinging every 1 s holds a paint indefinitely with no repaint**. Re-asserting brightness on a timer also
  "worked" only because it is traffic; it blanks/relights visibly. Same URL.
- Brightness: feature 0x8040 (index 0x0f on this firmware), ack `11 ff 0f 2b 00 ...`. Same URL.
- Dialpad over Bluetooth: its own HID endpoint, HID++ device index 0xFF, 29-30 features, **no 0x0008 KeepAlive**
  so no heartbeat needed. **0x4610 MultiRoller at index 0x0d**: roller 0 = small wheel (0x28 = 40 steps/rev),
  roller 1 = large dial (0xB4 = 180 steps/rev); deltas 1..9 and 0xFF..0xF7 (device accelerates). Diverting the
  rollers stops them driving the system pointer/scroll. 0x1B04 sits at index 0x0a (not 0x0b) so indices must be
  discovered, never assumed. Four buttons CIDs 0x0053, 0x0056, 0x0059, 0x005a, flags 0x31 (reprogrammable +
  divertable). "No source exposes a dial-press control ... the large dial most likely does not click."
  Behind a Bolt receiver the device index would be 1-6 - "untested here". Same URL.
- Reply matching: the Dialpad floods mouse report 0x02 while turning; a request must be matched on report
  0x11 + feature index + function/swid byte, never "first inbound report". Same URL.
- Options+ profile store on macOS: `~/Library/Application Support/Logi/LogiPluginService/Applications/<DeviceType>/<app>/Profiles/<GUID>/ProfileInfo.json`;
  device types `Loupedeck70` = Keypad (9 press, 0 rotate), `Loupedeck71` = Dialpad (4 press, 2 rotate, page "Dial
  Page"), `Loupedeck72` = Actions Ring (8 press, 8 rotate). Chord encoding embeds the macOS keyboard layout id.
  https://github.com/shensquared/mx-creative-console/blob/main/docs/OPTIONS-PLUS.md
- Design decision record: Node Actions-SDK plugins cannot push images (`GetActionImage` returns null, no outbound
  messages); C# can (`GetCommandImage` + `ActionImageChanged`). They chose raw HID ("Path C").
  https://github.com/shensquared/mx-creative-console/blob/main/docs/DECISIONS.md

### 1.2 hcooper-idealbuilders/mx-creative-controller (Windows, Node + Julusian lib, runs beside Options+)

- Windows exposes five HID collections Col01..Col05; the Julusian lib opens only Col01 by default, hence
  "Cannot write to hid device" for 0x13/0x14 writes (filed as Julusian issue #14).
  https://github.com/hcooper-idealbuilders/mx-creative-controller/blob/main/docs/protocol.md ,
  https://github.com/Julusian/node-logitech-mx-creative-console/issues/14
- **"The firmware dims the panels on its own. After an idle period (and after USB suspend) the console sets its
  LCD brightness to zero all by itself. Image writes keep succeeding - into a black screen."** Their fix:
  re-send `0x11 ff 0f 2b 00 <pct>` on Col01 every 30 s. https://github.com/hcooper-idealbuilders/mx-creative-controller#readme
- Options+ coexistence: their 1 Hz repaint "reclaims" keys from Options+ on focus changes; 4 Hz repaint + 1 Hz
  heartbeat visibly flickers against the firmware default. Keepalive fn 0 returned `01 f4 27 10` (500 ms /
  10000 ms, meaning unclear). docs/protocol.md above.

### 1.3 Bitfocus Companion surface module

- Repo `bitfocus/companion-surface-logitech-mx-creative-console` (v1.0.1) is a thin wrapper over
  `@logitech-mx-creative-console/node` ^0.3.0: `openMxCreativeConsole(path, { jpegOptions, brightness: true })`,
  `fillKeyBuffer`, `clearPanel`, `resetToLogo` on close, `setBrightness(percent)`, `rotate` events for
  encoders, image throttle/retry then disconnect on repeated failure. No keepalive or idle code of its own.
  https://github.com/bitfocus/companion-surface-logitech-mx-creative-console/blob/main/src/instance.ts
- Companion 4.1.0 added the console as a surface; the module request says "buttons, though not the wheel".
  https://github.com/bitfocus/companion-module-requests/issues/1854
- Only user-facing bug: on Windows 10 the device is not discovered by Companion (Julusian's WebHID demo works);
  open since 2025-09-29, no maintainer reply. https://github.com/bitfocus/companion-surface-logitech-mx-creative-console/issues/9

### 1.4 notno/creative-console-daemon (Rust, Windows only)

- Rust daemon for MX Creative Keypad + Stream Deck XL over hidapi; OBS websocket, webhooks, media keys via
  Windows SendInput, TOML config with hot reload, tray + editor. Keypad USB only; "The Dialpad (Bluetooth) is not
  supported"; "No per-application profiles". Button ids 1-9 LCD, 10/11 page. Adds no new protocol facts.
  https://github.com/notno/creative-console-daemon

### 1.5 CatalystMonish/mx-console-suite (macOS, Node)

- Keep-alive by repaint: "The console reverts un-refreshed keys to its logo, so both the launcher and the apps
  re-send their tiles on a short interval" - tiles re-sent every 2 s. One process owns the keypad; apps are
  spawned and the launcher releases/reopens the device. Exit combo = both page buttons.
  https://github.com/CatalystMonish/mx-console-suite

### 1.6 kunalbhayana/logikit (macOS, Python) and the .lp5 profile format

- Options+ profiles are plain JSON directories; `ActionIcons/*.ict` are JSON wrapping base64 SVG + text layer;
  every identifier is an uppercase 32-hex GUID; keyboard shortcuts are a 4-field string embedding the macOS
  virtual keycode, Cocoa modifier mask and the keyboard layout id. logikit builds a keypad from a `.keys` text
  file and backs up the store before writing. https://github.com/kunalbhayana/logikit
- MovGP0/mx_creative_keypad ships 20 `.lp5` profiles (KiCad, Krita, Inkscape, VS Code, Rider, Zed ...) and an
  LP5 reference: `.lp5` = ZIP with `ApplicationInfo.json`, `ProfileInfo.json`, `ActionIcons/*.ict`,
  `metadata/LoupedeckPackage.yaml` (`type: Profile5`). `ApplicationInfo.json` binds a profile to a process via
  `processOrBundleName`. Macro action ids look like `$@Generic___@Macro___<ID>`, text `$@Generic___@TypeText___...`,
  sleep `$@Generic___@Sleep___500`, keys `Return___<LAYOUT_ID>___Return___`.
  https://github.com/MovGP0/mx_creative_keypad/blob/main/.agents/Skills/mx-creative-keypad/references/lp5-format.md
- Import feasibility: the format is documented well enough to parse key -> (icon SVG, label, macro steps,
  keyboard chord); chords need a layout-id-aware decoder (macOS keycodes / Windows form differ). Nobody has
  published a converter to another tool; unclear whether Windows `.lp5` uses the same chord encoding.

### 1.7 rshankras/claude-console + keypad-profiles, k12club/neon-deck (C# Actions-SDK plugins)

- claude-console (1 star) is a C# plugin needing Options+ 6.4+ Plugin Service (.NET 10); universal plugin, ships
  `.lp5` layouts per OS; live keys are opt-in and wire hooks into `~/.claude/settings.json`; opening a live key
  in the Options+ icon editor "freezes it into a snapshot". Windows Terminal cannot report the front tab.
  https://github.com/rshankras/claude-console
- keypad-profiles: `.lp5` files are bound to Terminal.app / Windows Terminal; import once, survives plugin
  updates. https://github.com/rshankras/keypad-profiles
- neon-deck: C# plugin with a 1 s ticker for live keys, Clawd idle animation "8 frames, 4 fps"; dev loop via
  `.link` file in `%LOCALAPPDATA%\Logi\LogiPluginService\Plugins\` and `loupedeck://plugin/<name>/reload`.
  https://github.com/k12club/neon-deck

### 1.8 Other repos found (GitHub search, 2026-09-09)

- mario-gutierrez/mx-creative-console-webhid (WebHID, Chrome 89+, keypad + dialpad, brightness read/set,
  dialpad roller events) https://github.com/mario-gutierrez/mx-creative-console-webhid
- ~35 Actions-SDK plugins for MXCC (SolidWorks, Godot, Unity, Litra, Spotify, Claude/Codex usage meters ...);
  all C#/Node via Options+, none touch the device directly.
- AprilNEA/OpenLogi (Rust Options+ alternative, 20k stars) has an open request "Plans to support the MX Creative
  Console?" (2026-05-31) with a user saying "Solaar dont work for me". https://github.com/AprilNEA/OpenLogi/issues/13
- Julusian repo open issues (facts only): #22 "Windows & Dialpad Support", #11 "Panels are not updated in
  sync", #14 collections. https://github.com/Julusian/node-logitech-mx-creative-console/issues
- logilinux: 12 stars, PR #8 "Fix MX Keypad disconnect handling, reconnect discovery" (open since 2026-07-09).
  https://github.com/logilinux/logilinux/pull/8

### 1.9 New hardware: "MX Keypad" (launched 2026-09-08)

- Logitech launched the $99.99 "MX Keypad" for developers/AI workflows on 2026-09-08 (Graphite, Pale Grey),
  nine LCD keys, USB-C, macOS 13+/Windows 10+, 3 months Copilot Pro+, plugins for Copilot/Claude Code/VS Code/
  IntelliJ. Press release says it "builds on" the MX Creative Keypad.
  https://news.logitech.com/press-releases/news-details/2026/Logitech-Unveils-MX-Keypad-for-Developers-The-Customizable-Multi-App-AI-Control-Center/default.aspx ,
  https://9to5mac.com/2026/09/08/logitech-launches-99-mx-keypad-for-coding-and-ai-workflows/
- Logitech's own troubleshooting article is now titled "Troubleshooting - MX Creative Console & MX Keypad".
  https://support.logi.com/hc/en-150/articles/25575060779031-Troubleshooting-Updates-MX-Creative-Console
- Whether the MX Keypad is the same USB PID (c354) / same firmware family: **unclear** (no lsusb output published yet).

## 2. Logitech HID++ 2.0 public documentation

- Official draft spec (2012-06-04) is public; it defines the 4-bit software id, device index, feature index,
  error codes, root feature 0x0000 (getFeature returns 1-based index, 0 if absent), 0x1B00 rev1. It predates
  0x1B04, 0x4610, 0x8040 and 0x0008, so those come only from community sources.
  https://lekensteyn.nl/files/logitech/logitech_hidpp_2.0_specification_draft_2012-06-04.pdf
- logiops wiki lists feature IDs and the 0x0003 "Protocol Support" nibble (USB, Unifying, BTLE, Bluetooth).
  https://github.com/PixlOne/logiops/wiki/HIDPP--2.0
- Solaar's constants name the features we see: KEEP_ALIVE=0x0008, DFUCONTROL=0x00C3, REPROG_CONTROLS_V4=0x1B04,
  BRIGHTNESS_CONTROL=0x8040, ONBOARD_PROFILES=0x8100, HAPTIC=0x19B0 (MX Master 4), no name for 0x4610.
  https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/hidpp20_constants.py
- Solaar `show` output for the Keypad: HID++ 4.5, Model ID C35400000000, firmware "U1 66.00.B0017" +
  bootloader "BL2 20.00.B0017", 18 features, 2 reprogrammable keys 0x01A1/0x01A2, battery unavailable.
  Dialpad: **USB id 046d:BC00 over Bluetooth (btleid BC00)**, HID++ 4.5, firmware "RBO 07.00.B0016", 30 features
  incl. 0x1D4B wireless status, 0x1004 unified battery, 0x1814/0x1815 change-host/hosts-info, 0x4610, four
  reprogrammable keys reported by Solaar as "Back Button, Forward Button, Button 6, Left Scroll As Button 7".
  https://github.com/pwr-Solaar/Solaar/issues/3100
- Solaar maintainer closed #3100 ("not sure if there is anything more here for Solaar", closed 2026-04-14 for no
  response); Solaar has no descriptor for either device. Same URL.
- Solaar rules: "rule processing only fully works under X11 ... process conditions ... on GNOME under Wayland
  need the Solaar Gnome extension". https://github.com/pwr-Solaar/Solaar/blob/master/docs/rules.md
- Linux kernel `hid-logitech-hidpp.c`: implements 0x0003, 0x0005, 0x1000/0x1001/0x1004 battery, 0x1D4B,
  0x1F20, 0x2120/0x2121 hi-res wheel, 0x1B04 (only to map extra mouse buttons), 0x8123 G920. **No entry for
  c354 or bc00**; USB entries are an explicit PID list, so the Keypad binds to hid-generic and hidraw exposes
  every report untouched. Bluetooth entries are also a PID list (0xb0xx). Bolt children are recognised via
  `hidpp_is_bolt_child()` (parent product 0xc548) and get name/serial init.
  https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c
- `hid-logitech-dj.c` handles the Bolt receiver as `recvr_type_bolt` (`USB_DEVICE_ID_LOGITECH_BOLT_RECEIVER
  0xc548`, `HID_QUIRK_ALWAYS_POLL` in hid-quirks.c) and creates one child HID device per paired slot; Solaar
  lists Bolt with max 6 devices. https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-dj.c ,
  https://pwr-solaar.github.io/Solaar/devices/
- Bolt pairing adds an authentication phase (passcode) - relevant if keyvo ever pairs the Dialpad itself.
  https://pwr-solaar.github.io/Solaar/capabilities/
- libratbag hidpp20.h knows 0x1B04 as SPECIAL_KEYS_BUTTONS and 0x8100 ONBOARD_PROFILES; nothing about 0x4610/0x8040.
  https://github.com/libratbag/libratbag/blob/master/src/hidpp20.h
- MX Master 4 haptics = feature 0x19B0 with 16 named waveforms and intensity 0-5 (Solaar master, logiops PR
  #524). Not present on Keypad or Dialpad feature tables. https://github.com/PixlOne/logiops/pull/524 ,
  https://github.com/pwr-Solaar/Solaar/issues/3191
- Solaar udev rule grants `uaccess` to every 046d hidraw node and to `/dev/uinput`
  (`KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess"`). https://github.com/pwr-Solaar/Solaar/blob/master/rules.d/42-logitech-unify-permissions.rules
- Julusian ships two udev variants: desktop (`KERNEL=="hidraw*", ATTRS{idVendor}=="046d",
  ATTRS{idProduct}=="c354", MODE:="660", TAG+="uaccess"`) and headless (`GROUP="plugdev"`).
  https://github.com/Julusian/node-logitech-mx-creative-console/tree/main/packages/node/udev

## 3. Forums and communities

- Lemmy (2024-09-29, 56 upvotes, 9 comments): "Anyone know if the MX creative console from logitech works on
  Linux (and to what degree?" - OP wants to drive it from a Raspberry Pi for home control, "I'm sure it'll
  eventually be reverse engineered". https://lemmy.ca/post/29973362
- Solaar #3100 (Ubuntu 25.10, Wayland): user wants both devices, hits "solaar show" crash, gets logilinux
  examples and the WebHID demo working, complains "I'm really wishing Ubuntu didn't force users to switch to
  Wayland"; another user later posts logimap. https://github.com/pwr-Solaar/Solaar/issues/3100
- Hacker News: one story (3 points, 0 comments, 2024-09-24), zero comments mentioning the device since.
  https://news.ycombinator.com/item?id=41635805
- Logitech: "Logitech software is not supported by Chrome and Linux" (Getting Started); product page footnote:
  Options+ "available for recent versions of Windows and macOS". https://support.logi.com/hc/en-us/articles/24727160439703-Getting-Started-MX-Creative-Console ,
  https://www.logitech.com/en-us/shop/p/mx-keypad
- Options+ v1.96 notes: "Some people complained of a non-responsive dialpad, especially when waking up the
  device or when the computer was coming out of sleep ... refactored the Connection logic".
  https://support.logi.com/hc/en-us/articles/36716539691031-What-s-new-MX-Creative-Console
- Options+ settings: keypad brightness slider "will be saved and reused any time your device wakes up" - i.e.
  the device does sleep/wake under Options+. https://support.logi.com/hc/en-150/articles/25407700552599-Settings-Configuration-MX-Creative-Console
- Firmware updates: "The Firmware Update Tool is no longer supported" - updates come only through Options+.
  https://support.logi.com/hc/en-150/articles/25575060779031-Troubleshooting-Updates-MX-Creative-Console
- Reddit, KDE Discuss, Arch/Fedora/Ubuntu forums, Bitfocus forum, Lobsters: no reachable posts about the
  device (Reddit blocks the crawler; forum searches returned nothing). Arch Wiki has no page mentioning it.
- Windows-side pain points from repos: Options+ "broken and limited" (notno), Windows Terminal cannot report the
  focused tab (claude-console), Options+ freezes dynamic keys when opened in its icon editor (claude-console).

## 4. Logitech official material

- Product/support: Keypad = 9 LCD keys + 2 page keys, USB-C only; Dialpad = large dial + roller + 4 buttons,
  Bluetooth LE or Logi Bolt, 2x AAA. Per-key LCD resolution and refresh rate are not published anywhere
  official; 118x118 px per key / 480x480 panel comes only from reverse engineering (section 1).
  https://support.logi.com/hc/en-us/articles/24727160439703-Getting-Started-MX-Creative-Console
- Options+ release notes: v1.83+ = Actions Ring for MXCC; v1.87 (2025-03-04) cut/paste actions across
  Keypad pages; v1.91 (2025-05-29) MXCC fixes; v1.95 (2025-09-20) Actions Ring opacity + FOLDERS; v1.96
  Dialpad reconnection; v1.98 (2026-01-08); 2.x (2.7.9x current). Actions Ring works with all MX devices since
  Aug 2025. https://marketplace.logi.com/releasenotes/optionsplus/en ,
  https://support.logi.com/hc/en-001/articles/1500005516462-Logi-Options-Release-Notes ,
  https://support.logi.com/hc/en-in/articles/25576274277143-Logi-Actions-Ring-MX-Creative-Console
- Logi Actions SDK: plugins are `.lplug4` packages run by the Logi Plugin Service (part of Options+ /
  Loupedeck); "Designed to work on both Windows and macOS"; no Linux. https://logitech.github.io/actions-sdk-docs/plugin-basics/
- Node.js SDK is beta, "New in Plugin API 6.2.3 (Options+ 1.97)", "macOS support added in Plugin API 6.3
  (Options+ 2.2)". https://logitech.github.io/actions-sdk-docs/nodejs/introduction/
- C# dynamic images: override `GetCommandImage(actionParameter, imageSize)` with `BitmapBuilder`; do not set
  font size ("Plugin Service will define the best font size per device").
  https://logitech.github.io/actions-sdk-docs/csharp/tutorial/change-a-button-image/
- Node images: per shensquared's runtime analysis the Node SDK message set is InitConnection, GetActionList,
  GetActionText, GetActionImage(null), ExecuteCommand, ExecuteAdjustment, StopPlugin -> no dynamic images.
  https://github.com/shensquared/mx-creative-console/blob/main/docs/OPTIONS-PLUS.md
- Plugin classes: "application" plugins (need a foreground app, matched via `applicationPatterns`:
  processNamePattern / bundleNamePattern / displayNamePattern / executablePathPattern, .NET regex) vs
  "universal" plugins. https://logitech.github.io/actions-sdk-docs/csharp/plugin-features/plugin-capabilities/
- Default application profiles ship inside application plugins and act as templates only.
  https://logitech.github.io/actions-sdk-docs/csharp/plugin-features/default-application-profiles/
- Marketplace approval: every version manually reviewed "at the sole discretion of the Logitech Marketplace
  team", no guaranteed time, ping marketplace@logitech.com after 10 working days; open-source code must comply
  with its licences; approval can be withdrawn. Local dev needs no signing/notarization.
  https://logitech.github.io/actions-sdk-docs/marketplace-approval-guidelines/ ,
  https://www.logitech.com/en-us/tos/marketplace-terms-plugin-eula
- Marketplace catalog is Windows/macOS only; an open-source plugin is fine (several exist). Whether Logitech
  would list a plugin that advertises a Linux daemon: unclear.

## 5. Comparable projects (process/UX)

### StreamController (Flathub com.core447.StreamController, 1104 stars)
- Per-app page switching is regex on window title/class; on GNOME it requires its own GNOME Shell extension
  ("Have you installed the Gnome extension?" is the standard support reply), on KDE it shells out to
  `kdotool` (via `flatpak-spawn --host` inside Flatpak), Hyprland uses `.socket2.sock`, Sway polls
  `swaymsg -t get_tree` every 0.2 s. https://github.com/StreamController/StreamController/tree/main/src/backend/WindowGrabber/Integrations
- Flatpak manifest: `--device=all` "Needed to communicate with the decks", `--talk-name=org.freedesktop.Flatpak`
  (plugins run host commands), `--talk-name=org.gnome.Shell`, `--filesystem=home`.
  https://github.com/flathub/com.core447.StreamController/blob/master/com.core447.StreamController.yml
- udev rules are `SUBSYSTEM=="usb" ... TAG+="uaccess"` and must be installed manually even for the Flatpak.
  https://github.com/StreamController/StreamController/blob/main/udev.rules
- Top issue themes (by comments): RAM usage (#123, 53c), migration from streamdeck-ui (#32), "doesn't find
  stream deck" (#50, #370, #459), automatic page switching not working (#215, #601), USB disconnect needs app
  restart (#142), Flatpak tray icon on GNOME (#421), input stopped working on Wayland (#477).
  https://github.com/StreamController/StreamController/issues

### OpenDeck (Rust/Tauri, Flathub me.amankhanna.opendeck, 2146 stars)
- Runs Elgato Stream Deck plugins (Wine/Node), OpenAction API; profiles switch on active window. Flatpak uses
  `--device=all`, `--talk-name=org.freedesktop.Flatpak`; udev rules still manual.
  https://github.com/nekename/OpenDeck , https://github.com/flathub/me.amankhanna.opendeck/blob/master/me.amankhanna.opendeck.yaml
- Active-window detection uses `active-win-pos-rs`; on KDE Wayland it injects a KWin script per poll (250 ms)
  -> issue #425 "Application watcher floods the D-Bus session bus on KDE Wayland, exhausting the user's message
  quota" (open, 2026-08-15). https://github.com/nekename/OpenDeck/issues/425
- Input simulation via `enigo` -> stream of Wayland issues (#120, #154, #211, #287); maintainer: "I would love to
  get rid of it"; a community `uinput-simulation` plugin was written instead (Havner). Boatswain-style
  RemoteDesktop portal "remember decision" was requested (#171).
  https://github.com/nekename/OpenDeck/issues/171 , https://github.com/Havner/uinput-simulation
- Flatpak autostart/background did not work (#67, #195). https://github.com/nekename/OpenDeck/issues/195

### Boatswain (GNOME, Flathub com.feaneron.Boatswain)
- Author got Stream Deck udev rules **upstreamed into systemd 251** so users need no manual rule; Flathub-only
  distribution. https://feaneron.com/2022/04/13/updates-on-boatswain/
- Manifest: `--device=all`, pulseaudio, network, MPRIS. https://github.com/flathub/com.feaneron.Boatswain/blob/master/com.feaneron.Boatswain.json
- Open issues: "Smart Profiles support" (per-app profiles absent), "doesn't detect stream deck after suspend or
  usb replug", Flatpak zombie on KDE. https://gitlab.gnome.org/World/boatswain/-/issues

### streamdeck-linux-gui (436 stars) - Qt; issues are hotkeys/udev/Qt platform; effectively superseded.
  https://github.com/streamdeck-linux-gui/streamdeck-linux-gui

### Loupedeck on Linux - small efforts only: scottlaird/loupedeck (Go, 13 stars), libredeck (0 stars),
  openSUSE hack week project; a PySide6 config app in development. https://github.com/scottlaird/loupedeck ,
  https://hackweek.opensuse.org/projects/support-loupedeck-ct-hardware-on-linux

### Elgato SDK conventions
- Manifest `ApplicationsToMonitor` (bundle id on macOS, exe on Windows) -> `applicationDidLaunch/DidTerminate`;
  plugins ship `.streamDeckProfile` files and can `switchToProfile` only to their own bundled profiles.
  https://docs.elgato.com/streamdeck/sdk/guides/app-monitoring/ , https://docs.elgato.com/streamdeck/sdk/guides/profiles/

## 6. Flatpak/Flathub specifics for HID daemons

- All three Stream Deck apps on Flathub use `--device=all`; none uses `--device=input` or the USB portal.
  (manifests above)
- `--device=input` exists since Flatpak 1.15.6 (joysticks/evdev); Flatpak 1.17 conditional permissions allow
  `--device-if=all:!has-input-device` + `--device=input` for backwards compatibility. No dedicated
  "hidraw" permission exists. https://docs.flatpak.org/en/latest/sandbox-permissions.html ,
  https://github.com/flathub/com.carpeludum.KegaFusion/issues/6
- USB portal (`org.freedesktop.portal.Usb`, Flatpak 1.16) hands out `/dev/bus/usb` fds "meant to be used with
  the USB library of your choice" (libusb / hidapi-libusb) - not hidraw, so the kernel hid-generic driver would
  have to be detached; unsuitable for a device that must stay a HID keyboard/consumer device.
  https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Usb.html
- `/dev/uinput` is a misc char device that Flatpak does not share even with `--device=all` ("Not sharing
  '/dev/uinput' with sandbox: File has unsupported type", antimicrox #741); ydotool's daemon needs uinput and
  "usually requires root". https://github.com/AntiMicroX/antimicrox/issues/741 , https://github.com/ReimuNotMoe/ydotool
- udev rules cannot be installed from inside a sandbox; every project documents `sudo cp ... /etc/udev/rules.d/`
  or an install script; Boatswain's answer was upstreaming to systemd. (section 5)
- Background portal: `RequestBackground(autostart=true, commandline=..., dbus-activatable)`; `SetStatus` for a
  status line. OpenDeck reports it unreliable in Flatpak (#67/#195).
  https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html
- KWin scripting from a sandbox: StreamController does it by `flatpak-spawn --host kdotool` (needs
  `--talk-name=org.freedesktop.Flatpak`, i.e. full host escape); a direct `--talk-name=org.kde.KWin` would let a
  sandboxed app call `org.kde.kwin.Scripting.loadScript(path)` but the script file must be host-readable -
  unclear whether Flathub reviewers accept `org.kde.KWin` talk-name.
- Flathub inclusion policy is case-by-case; `--device=all` is tolerated in practice for hardware apps (three
  precedents). https://docs.flathub.org/docs/for-app-authors/requirements

## 7. Wayland focus detection

- KWin scripting API: `workspace.windowActivated(KWin::Window*)` signal, `workspace.activeWindow`, window
  `resourceClass`; scripts are loaded via D-Bus `org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript` and
  die when KWin restarts. https://develop.kde.org/docs/plasma/kwin/api/
- Push model (logimap): a persistent KWin script calls back over D-Bus on `windowActivated` - no polling.
  Poll model (OpenDeck/active-win-pos-rs, kdotool per call): script per query, journal scraping, 1% CPU at
  50 ms and a D-Bus-quota flood at 250 ms on KDE. https://github.com/nekename/OpenDeck/issues/425 ,
  https://dev.to/plexescor/how-i-built-native-wayland-window-tracking-across-hyprland-gnome-and-kde-in-c23-2lk8
- kdotool (389 stars): generates a KWin script per invocation; Plasma 6 only; window ids are KWin UUIDs.
  https://github.com/jinliu/kdotool
- GNOME: no compositor API; extensions expose D-Bus: "Focused Window D-Bus"
  (`org.gnome.shell.extensions.FocusedWindow.Get` -> JSON with title/wm_class/pid, plus a signal for
  subscriptions), "Window Calls" / "Window Calls Extended" (`org.gnome.Shell.Extensions.WindowsExt.FocusClass`
  etc.; newer versions emit D-Bus signals). StreamController and Solaar each ship their own extension.
  https://github.com/flexagoon/focused-window-dbus , https://github.com/hseliger/window-calls-extended
- Hyprland: `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock` streams `activewindow>>CLASS,TITLE`
  and `activewindowv2>>ADDRESS`; `hyprctl activewindow -j` for state. https://wiki.hypr.land/IPC/
- Sway: `swaymsg -t subscribe '["window"]'` (StreamController currently polls `get_tree` instead).
- Protocol coverage (wayland.app, 2026-09): `wlr-foreign-toplevel-management-unstable-v1` v3 is implemented by
  Sway, Hyprland, niri, river, labwc, Wayfire, phoc, Cage, Louvre, Treeland, Mir(v2) - **not KWin 6.7, not
  Mutter 51, not COSMIC**. `ext-foreign-toplevel-list-v1` (identifier/title/app_id only, no "activated" state)
  is implemented by Sway, Hyprland, niri, river, labwc, phoc, COSMIC, Mir, Louvre, Treeland - **not KWin, not
  Mutter, not Wayfire**. https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1 ,
  https://wayland.app/protocols/ext-foreign-toplevel-list-v1
- Conclusion: no single protocol covers KDE + GNOME; a backend-per-compositor design is unavoidable (KWin
  script, GNOME extension, wlr-foreign-toplevel for wlroots-family, Hyprland socket as a cheaper special case).
- KDE's own `org_kde_plasma_window_management` exposes app ids/active state to Plasma taskbars; whether
  third-party clients may bind it under current KWin is unclear (older design described an authorization step).
  https://blog.martin-graesslin.com/blog/2016/06/a-task-manager-for-the-plasma-wayland-session/

## 8. Input injection on Wayland

- uinput: works everywhere (kernel level); needs write access to `/dev/uinput` -> udev `uaccess` rule (Solaar
  ships one; Havner's plugin ships `42-uinput-uaccess.rules`); virtual keyboard must be created with the right
  keymap semantics (evdev keycodes, layout-independent); logimap needed the `input` group or setfacl.
  https://github.com/Havner/uinput-simulation , https://github.com/abishekmuthian/logimap
- ydotool: daemon + client over a socket, daemon needs root/uinput; non-ASCII and layout issues reported
  (OpenWhispr #240). https://github.com/ReimuNotMoe/ydotool , https://github.com/OpenWhispr/openwhispr/issues/240
- dotool: uinput, no daemon, xkb-aware typing; needs `input` group/udev rule. https://git.sr.ht/~geb/dotool
- wtype: `zwp_virtual_keyboard_v1`; GNOME never implemented it, KWin 6 hides it from non-IME clients (logimap
  troubleshooting). https://github.com/abishekmuthian/logimap
- enigo (Rust): X11 default; Wayland/libei backends "experimental" behind feature flags; double-input bug when
  several protocols are available (#464, closed); restore-token support merged (#514/#532).
  https://github.com/enigo-rs/enigo , https://github.com/enigo-rs/enigo/issues/464
- RemoteDesktop portal: `CreateSession -> SelectDevices(types, persist_mode 0/1/2, restore_token) -> Start ->
  NotifyKeyboardKeycode/NotifyKeyboardKeysym` over D-Bus, or `ConnectToEIS` for libei (then Notify* methods
  error). No root, no udev; one user prompt per session unless persisted.
  https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
- KDE implementation: xdg-desktop-portal-kde implements persist_mode/restore_data and NotifyKeyboardKeysym via
  KWin; libei/EIS integration present (`org.kde.KWin.EIS.RemoteDesktop`). Known bug: "Persistence in the remote
  desktop portal does not work across reboots" - still reproduced on Plasma 6.6.3 (2026-04).
  https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/blob/master/src/remotedesktop.cpp ,
  https://bugs.kde.org/show_bug.cgi?id=480235
- KDE workaround: pre-authorization table `kde-authorized` (Plasma 6.3+): `flatpak permission-set kde-authorized
  remote-desktop <app_id> yes` (empty app_id for host apps; app_id from XDP Registry or `app-<id>.service`
  unit name). Plasma 6.5 adds a settings page to manage portal permissions.
  https://develop.kde.org/docs/administration/portal-permissions/ ,
  https://planet.kde.org/david-redondo-2025-09-04-configuring-portal-permissions-in-plasma/
- Keysym-based portal typing has a KDE layout bug (Bug 489021: NotifyKeyboardKeysym uses the wrong layout to
  compute the keycode). https://www.mail-archive.com/kde-bugs-dist@kde.org/msg1011909.html
- Boatswain and OpenDeck (via enigo/libei) already use the portal path and users hit the per-login prompt;
  OpenDeck users asked for "remember the decision" like Boatswain shows. https://github.com/nekename/OpenDeck/issues/171

## Implications for keyvo

Facts that change the design
- Drive the keypad with a host keepalive (0x0008 fn1, 3000 ms requested) at **1 s**, from the moment the
  device opens, before any read; without it there are no key events at all and paints revert to the logo.
  Do not "keep alive by repainting" (mx-console-suite, hcooper) - it flickers.
- Expect the firmware to zero LCD brightness after idle/USB suspend while accepting writes; re-assert 0x8040
  after resume/idle (hcooper re-sends every 30 s; better: on wake events and after each reconnect).
- Pace multi-packet LCD frames (~5 ms between report-0x14 fragments), keep JPEG quality >= ~0.85, make the
  header length exactly match. Use RGB888 JPEG; RGB565 is rejected.
- Never hardcode HID++ feature indices: 0x1B04 is 0x0b on the Keypad and 0x0a on the Dialpad; discover via root.
- Dialpad: no keepalive, 0x4610 MultiRoller (40 / 180 detents per rev, accelerated deltas), four divertable
  CIDs, presses repeat while held. Match replies by report id + feature index + swid; ignore the 0x02 mouse
  flood. Over Bolt the device index is 1-6 (untested by anyone) - M0 must measure this on the user's c548.
- Kernel binds hid-generic to c354 (no HID++ driver quirks) so hidraw is clean; the Dialpad via Bolt appears as a
  hid-logitech-dj child with kernel HID++ init already talking to it (battery, name) - share politely.
- Per-app focus: no Wayland protocol spans KDE and GNOME. Ship a KWin script that pushes `windowActivated`
  over D-Bus (logimap model, avoids OpenDeck #425's poll flood); GNOME via an extension D-Bus interface
  (Focused Window D-Bus / Window Calls Extended or our own); wlroots family via wlr-foreign-toplevel; Hyprland
  socket2 optional.
- Input injection: uinput as primary (layout-independent evdev codes, works on every compositor, needs a udev
  uaccess rule) with the RemoteDesktop portal as the no-root fallback; document `kde-authorized` pre-auth and the
  KDE persistence bug so users understand repeated prompts.
- Options+ profile import is realistic: `.lp5` = ZIP of documented JSON (Loupedeck70/71 layouts, `.ict` SVG
  icons, macro steps, chords with layout id). Plan an importer for icons + labels + simple chords; treat
  layout-id chords as best effort.
- Windows path: only the C# Actions SDK can render dynamic key images; Node cannot. A Windows keyvo bridge
  would be a C# universal plugin or raw HID (both proven by others).
- Watch the new "MX Keypad" (2026-09-08): confirm its PID/feature table as soon as an lsusb/`solaar show`
  appears; the same daemon should cover it if it is c354-compatible.

Things to measure in M0
- Keepalive cadence vs. paint persistence (1 s vs 3 s vs none), heartbeat rate, and time-to-logo when stopped.
- Idle dim: how long until brightness drops to 0 with keepalive running; whether keepalive alone prevents it.
- Fragment pacing threshold and max sustainable full-panel fps at 118x118x9 JPEG (nobody has published fps).
- Dialpad over Bolt: device index, whether 0x4610 divert works through the receiver, wake latency after sleep
  (Options+ 1.96 had to fix reconnection - expect trouble).
- Reconnect after USB suspend/replug (logilinux PR #8, StreamController #142/#459, Boatswain #42 all struggled).

Things to copy
- Julusian's dual udev rules (uaccess for desktops, group for headless); Solaar's `uinput` uaccess line.
- Boatswain: upstream a udev rule for 046d:c354 into systemd's rules/hwdb so Flathub users need no manual step.
- Elgato/Options+ convention: per-app profiles keyed by app identifier (wm_class on Wayland), plugin-bundled
  default profiles that act as templates and never overwrite user edits.
- claude-console: live/opt-in features that edit user config only after explicit confirmation, with backups.
- mx-console-suite: "both page buttons together = back to launcher" as an always-available escape chord.
- Companion: `resetToLogo` on close so the device shows a sane state when the daemon exits.

Things to avoid
- Polling compositors for the active window (OpenDeck #425); journal scraping (kdotool/dev.to method).
- enigo-style multi-protocol injection (double inputs, layout bugs).
- Flatpak-first before the udev story is solved; `--device=all` plus `--talk-name=org.freedesktop.Flatpak` is what
  everyone ships and it is effectively no sandbox.
- Depending on the Options+ icon editor / Actions SDK for dynamic content on Windows (freezes live keys).

## Demand signals

- Linux support is officially absent: Logitech states Options+ is Windows/macOS only and firmware updates are
  only via Options+. (support.logi.com Getting Started / Troubleshooting)
- Community asks: Lemmy thread with 56 upvotes (2024); Solaar #3100 (closed for lack of a path, user "It'd be
  nice if we could add functionality for this device"); OpenLogi #13 "Plans to support the MX Creative
  Console?" with a follow-up user wanting Keypad + Dialpad on Windows and Linux because "Solaar dont work for me"
  (OpenLogi itself has 20k stars, showing appetite for Options+ replacements).
- Five independent Linux/macOS/Windows attempts at driving the device without Options+ in 12 months
  (Julusian 19 stars, logilinux 12, notno, shensquared, hcooper, logimap, mx-console-suite, webhid lib),
  plus Bitfocus Companion shipping a surface module - the demand is from tinkerers and streaming/AV users.
- The 2026-09-08 "MX Keypad for developers" launch targets exactly the Claude Code / Copilot / terminal
  audience that runs Linux; Logitech's own plugins are Windows/macOS only, so the gap widens.
- Comparable Linux Stream Deck apps are healthy (OpenDeck 2146 stars, StreamController 1104), and their issue
  trackers show per-app switching on Wayland and reliable input injection as the top recurring pain - the two
  things keyvo is designed around.
- Counter-signal: HN has zero discussion; Reddit is unreachable for this survey; total GitHub stars across all
  MXCC repos are low (< 60). The audience is real but small; distribution (Flathub, AUR, Fedora COPR) and a
  frictionless udev story matter more than features.
