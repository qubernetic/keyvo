# Logi Options+ device-communication inventory (macOS offline installer)

Date: 2026-10-24. Source: the public macOS offline installer of Logi Options+
(`logioptionsplus_installer_offline.zip` from Logitech's download CDN, 2026-09-30 build),
which identifies itself as Options+ **2.8.981479** (branch `staging_2.8`) bundling
**Logi Plugin Service 6.4.2.3414** (`LogiPluginService.app`). Purpose: interoperability of
keyvo with the MX Creative Keypad (USB 046d:c354) and MX Creative Dialpad (Bolt WPID BC00).

Method: static analysis only, inside an isolated distrobox with Ghidra 12.1.4 and the REA
CLI 6.7.0: file inventories, string tables, symbol names (both native binaries ship
unstripped C++ symbols), data-file schemas, and sixteen targeted decompiles of the display,
keep-alive and image-send path in the Plugin Service helper. No Logitech binary was
executed and no Logitech server was contacted. Boundary and rules:
[ADR-0008](../adr/0008-static-analysis-boundary.md).

This document carries protocol facts only: names, IDs, constants, layouts and where they
were found. Decompiler output and the raw dumps stay in the spike working directory on the
owner's machine; `analysis/...` paths below refer to that directory, not to the repository.
Everything is regenerable from the public installer with the method above.

## Package format

- The online installers (Windows 50 MB, macOS 20 MB) fetch "depots" through a keyed
  pipeline (`depot_download_job`, `keymaster_download_job`, protobuf
  `logi.protocol.manifest`); the offline installers (`logioptionsplus_installer_offline.exe`,
  724 MB, and `.zip`, 767 MB, same CDN directory) carry a `depots.zip` with 218 `.depot`
  containers.
- Plain `.depot` container: magic `10 01 17 20`, u32 LE manifest length, JSON manifest
  `{"files": [{name, mode, link?}]}`, then one block per manifest entry in order: u32 LE
  length plus raw bytes (symlink entries carry a zero-length block). The main payload
  (`logioptionsplus.depot`, 1062 entries) unpacks into `logioptionsplus.app` (Electron
  front end), `logioptionsplus_agent.app` (Qt agent), the driver installer bundle and
  `data/`; `logi_plugin_service.depot` holds the Plugin Service `.pkg`.
- Encrypted containers use magic `06 05 21 20` (depots) or `21 05 21 20` (files inside the
  payload) with a JSON header (`key-id` plus `file-sha` or `header-sha`); see "Data files
  (encrypted)".
## Summary

1. Options+ itself (Qt agent `logioptionsplus_agent`, 171 MB) does **not** drive the Creative
   Console display. Its `devio` HID++ library has 147 feature classes but no 0x19A1 class.
2. The Keypad and Dialpad are driven by the bundled **Logi Plugin Service** (Loupedeck
   heritage): a .NET host plus a native helper `LogiPluginServiceExt` (24 MB Mach-O) that
   contains `devio::Feature19a1ContextualDisplay`, `DeviceC354`, `DeviceBC00`,
   `Feature4610MultiRoller`, keep-alive and the VLP transport. That binary is the one that
   matters for keyvo, and it is small enough for a targeted Ghidra pass (done, ~3 min).
3. Feature 0x19A1 is named **ContextualDisplay**; its eight functions, their response field
   names, the device-state names (Streaming, Standby, Splash ...), the image header layout
   (3 + 12 bytes per image) and the capability field names were recovered from log format
   strings and three decompiled request builders (section 4).
4. Names for the unknown features: 0x1602 PasswordAuthentication, 0x1807
   ConfigurableProperties, 0x1E02 ManageDeactivatableFeatures. 0x92E2, 0x1EB0 and 0x180B
   appear in neither binary. 0x4610 MultiRoller has getCapabilities/getRollerCapabilities/
   getMode/setMode/getRollerSettings/setRollerSettings and a RotationEvent.
5. Device descriptor JSON for the Creative Console is shipped **encrypted** (custom
   `!\x05! ` container, key ids in `offline.json`); the plaintext `devices_initial.json`
   (118 devices) does not contain c354/BC00. Not decrypted in this spike (legal log).
6. The Electron front end is obfuscated JS with no HID access; all protocol hits in it are
   obfuscator index noise.

## Component map

| Component | Runtime | Talks to devices? | Evidence (kind, location, confidence) |
|---|---|---|---|
| `logioptionsplus_agent.app/Contents/MacOS/logioptionsplus_agent` (universal, x86_64 slice at 16384/89.8 MB, arm64 at 89866240/81.4 MB) | Qt 6 C++ agent, LaunchAgent `com.logi.optionsplus` | Yes: classic HID++ 2.0 over hidraw-equivalent (IOKit), Bolt/Unifying, BLE (`devio::UnifyingBus`, `BleppBus`, `MacOSBus`, `BtSerialBus`, `AccessoryBus`), UVC/audio for webcams | symbol names, `analysis/strings/demangled_symbols.txt` (182 119 symbols; namespaces `logi` 42k, `devio` 6.7k, `logi::hidio` 572, `logi::dfu` 547); high |
| `devio` library inside the agent | C++ HID++ feature layer, 147 `devio::FeatureXXXX*` classes | Yes | `analysis/strings/devio_feature_classes.txt`, `devio_feature_methods.txt`; high |
| `logi::lps` endpoint inside the agent | protobuf/websocket client of the Plugin Service (`lps.proto`, `multiroller.proto`, `crown.proto`) | No (IPC only); it launches/supervises the Plugin Service when an `activationDevices` PID appears | protobuf type names `logi::protocol::lps::*` (HandshakeRequest, GetServiceStateRequest, ButtonEvent, EncoderEvent, ControlAssignment ...), `analysis/strings/protocol_types.txt`; `depots_unpacked/logi_plugin_service/config.json`; high |
| `LogiPluginService.app` (.NET 8 / CoreCLR bundle, 64 files in `MonoBundle`) | .NET service `com.logi.pluginservice`, Loupedeck 5 code base (`LoupedeckService.dll`, `LoupedeckShared.dll`, `PluginApi.dll`, `HidSharp.dll`) | Indirectly: it owns the device model (`Loupedeck.Service.Devices.*`, `MxCreativeKeypad`, `MxCreativeDialpad`, `LogitechCreativeFamily` in `PluginApi.dll`) and talks to the native helper over a local JSON connection (`[LC]` log tag) | `analysis/lps_pkg/`, `analysis/lps/pluginapi_ascii.txt`; high |
| `LogiPluginService.app/Contents/MonoBundle/LogiPluginServiceExt` (universal; arm64 slice 11.9 MB) | native C++ helper, same `devio` library plus Creative-Console-specific classes | **Yes, this is the Creative Console driver**: `DeviceC354`, `DeviceBC00`, `Feature19A1` wrapper, `devio::Feature19a1ContextualDisplay`, `Feature4610MultiRoller`, `Feature0008SwitchAndKeepAlive`, `Feature8040BrightnessControl`, `Feature1B04SpecialKeys`, VLP root 0x0102 / feature set 0x0103 | `analysis/lps/ext_demangled.txt` (31 139 symbols), `ext_nonsymbol.txt`; Ghidra snapshot `analysis/ghidra/ext.snapshot` (26 556 procedures); high |
| `LogiPluginService.app/.../ChromeNativeMessaging.app`, `LogiPluginServiceTool.app`, `LoupeDiagMac.app`, `Loupedeck.app` stub | .NET helpers | No | file inventory `analysis/lps_pkg/`; high |
| `logioptionsplus.app` (Electron, `app.asar` 62 MB, 581 entries, 13 JS bundles) | UI | No: `package.json` has no HID/USB dependency, `main.js` has no HID strings; bundles are obfuscated (`_0x....` string arrays) | `analysis/asar/`, `analysis/asar_filelist.txt`; high |
| `logioptionsplus_updater.app`, `logi_crashpad_handler`, Driver Installer (DriverKit dext) | updater / crash / HID dext installer | dext: kernel-side HID access on macOS only | not analysed (out of scope); n/a |
| Data files `data/devices/*.json`, `data/card_presets`, `Resources/data/*.json`, `data/strings/*.yaml` | JSON/YAML | No (descriptors and UI data) | section 3; high |
| Depots (217 entries in `offline.json`; 122 plain containers unpacked, 95 encrypted containers) | per-device assets, plugins, strings | No | `analysis/depot_index.tsv`, `analysis/depots_unpacked/_survey.json`, `analysis/uuid_depot_headers.json`; high |

## Where the device knowledge lives

### Native code (agent and Plugin Service helper)

- Both binaries ship full C++ symbol tables (unstripped mangled names), so feature IDs,
  function names and nested enum/struct names are readable with `strings` + `c++filt`.
- Agent `devio` feature table (147 classes, file `analysis/strings/devio_feature_classes.txt`):
  0000 Root, 0001 FeatureSet, 0003 FirmwareInfo, 0005 DeviceNameType, 0007 DeviceFriendlyName,
  **0008 SwitchAndKeepAlive**, 0011 PropertyAccess, 0020 ConfigChange, 0021 CryptoIdentifier,
  00C0..00C3 DFUControl, 00D0 DFU, 00D1 ResumableDFU, **0102 Root (VLP)**, **0103 FeatureSet (VLP)**,
  0104 BatterySOC, 1000/1001/1004 battery, 1300 LEDControl, 1802 DeviceReset, 1805 OOBState,
  1806 ConfigurableDeviceProperties, **1807 ConfigurableProperties**, 1814 ChangeHost,
  1815 HostsInfos, 1816 BleProPrepairing, 18A1 LEDState, 1981/1982/1983 backlight,
  1990 IlluminationLight, 19B0 HapticFeedback, 19C0 ForceSensingButton, 1A00 PresenterControl,
  1B00/1B03/**1B04** SpecialKeys, 1B06 SpecialKeysAndButtons, 1B07 SpecialMouse, 1B10 ControlList,
  1BC0 ReportHIDUsages, 1C00 PersistentRemappableAction, 1D4B WirelessStatus,
  1E00 EnableHiddenFeatures, 1F1F FirmwareProperties, 1F20 ADCMeasurement, 2005..2400 mouse,
  40A0/40A2/40A3 FnInversion, 4220 LockKeyState, 4303 LuxReport, 4521/4522 DisableKeys,
  4530/4531 platform, 4540 KBLayout, **4600 Crown**, **4610 MultiRoller**, 6010/6012/6500/6501 Gestures,
  6100 TouchPadRawXY, 8010..8139 gaming, **8040 BrightnessControl**, 8300..8370 audio, 9001 TestPMW3816.
  Not present in the agent: 0x19A1, 0x92E2, 0x1602, 0x1E02, 0x1EB0, 0x180B.
- Plugin Service helper `devio` table (24 classes, subset tuned for the Creative Console):
  0000, 0001, 0003 (CentPPBridge, FirmwareInfo), **0008**, 0020, 00C3, 00D0, **0102**, **0103**,
  **1602 PasswordAuthentication**, 1802, **19a1 ContextualDisplay**, **1B04**, 1D4B, 1E00,
  **1E02 ManageDeactivatableFeatures**, 2121, 2130, 2150, **4610**, **8040**.
- Device classes in the helper: `DeviceC354` (Keypad: onButtonPressed/Released,
  onSpecialKeyPressed/Released, isControlIdValid, fixSpecialKeyId, onDeviceStateChanged,
  startKeepAlive(u16,u16), stopKeepAlive, onDfuTimeout) and `DeviceBC00` (Dialpad:
  onRollerRotated, onSpecialKeyPressed/Released, fixSpecialKeyId). Log tags `[dC354]`, `[dBC00]`,
  `[DC]` (device controller: "Device is not C354", "Disable VlpWindow4"), `[fXXXX]` per feature,
  `[LC]` local connection to the .NET side, `[LR]` local request.
- Wrapper `Feature19A1` (helper): addImage, addJpegImage, addRgb565Image, clearScreen,
  getButtonAreas, getDisplayArea, getMaxImageFps, getMaxImageSize, getDeviceAdopted,
  setDeviceAdopted, getDeviceState, setDeviceState, onButtonPressed/Released, onKeyEvent,
  onDeviceStateEvent, sendImages, sendImageThread (queue of `Feature19a1ImageQueueItem`),
  `s_emptyDisplayJpegImage`, image logging to `image_%03dx%03d_%03dx%03d/image_%07d.jpg`.
- VLP transport facts (strings in both binaries): "ExtendedVLP" / "NormalVLP" / "NotVLP"
  protocol classes; `ConfigItem VlpWindow` (per-PID window size, "Disable VlpWindow4");
  "command size %zu with report payload size %d, vlp payload is %d and packet count is %d";
  "vlp packet #%d, total packets %d"; "VLP response too short"; `VLP_LOOPBACK` test mode;
  `devio::DeviceRequest::set_vlp_response`; `devio::simulate_vlp_device` test double.
- VLP root 0x0102 (log format strings, both binaries): `GetFeature(id) -> id, index, hidden,
  version, maxMemory`; `ProtocolCapabilities() -> major, minor, availableMemory`; `Ping`.
  VLP feature set 0x0103: `GetCount`, `GetFeatureID(index) -> id, hidden, version, maxMemory`,
  `GetAllFeatureIDs -> count, record_size`.

### Data files (plaintext)

- `data/devices/devices_initial.json` (536 KB, 118 devices). Schema per device: `modelId`,
  `displayName`, `extendedDisplayName`, `slotPrefix`, `type` (MOUSE, KEYBOARD, TRACKBALL,
  RECEIVER, ILLUMINATION_LIGHT, CAMERA, PRESENTER, VIRTUAL_DEVICE), `depot`, `comboDevices`,
  `thumbnail`, `supportPageId`, `modes[].interfaces[] {type: DEVIO|HIDIO|UVC|AUDIO|DJ_RCVR_DFU|
  VIRTUAL_DEVICE, id: "046d_<pid>", derivatives[] {extendedModel, modelId "<id>_extN"}}`,
  `capabilities {hasBatteryStatus, specialKeys, unified_battery, pointerSpeed,
  isActionRingSupportedDevice, scroll_wheel_capabilities, hostInfos, fnInversion, disableKeys,
  webcam, flow, ...}`, `supportedApps`, `useDisplayNameFromFw`, `disableFwUpdate`.
  **modelId scheme** (derived from interface ids): `<mask><pid>` where the leading hex digit
  is a connection bitmask: 2 = Bluetooth/BLE PID, 4 = receiver WPID, 8 = USB wired PID,
  6 = BT+receiver, e = all three; four-digit ids are receivers/plain PIDs. Hence
  **`8c354` = Keypad over USB, `2bc00` = Dialpad over BLE** (medium confidence; consistent
  with `DEVIO 046d_4082` + `046d_b023` under `6b023`).
- `data/devices.json`: ordered list of 53 descriptor files (`devices_initial` plus
  `devices_0000001..0000058`); all but the initial one are encrypted (see below).
- `logioptionsplus_agent.app/Contents/Resources/data/`: `applications.json` (app profiles),
  `options_devices.json` and `guru_devices.json` (legacy modelId lists), `DeviceCompatibility.json`
  (receiver/firmware pairing rules keyed by PID, e.g. `bb00 requires c548 16`),
  `defaults/defaults_{control,slot}_{osx,win}.json`, `rap/*.json` (card recommendations),
  `macros/*.json`, `migration/*.json`, `overlay/osd_resources.json`, `firmware.pem` and
  `logitech-lap-public.pem` (signature keys for DFU/LAP; not analysed).
- `data/strings/*.yaml` (19 locales): UI strings; `WHATSNEW_MX_CREATIVE_CONSOLE` is the only
  Creative Console string, no protocol content.
- `depots_unpacked/logi_plugin_service/config.json`: `activationDevices` =
  `["2bc00", "8c354", "2b042", "2b048", "6b350"]`, watchdog (`baseTimeout` 1,
  `timeoutModificator` 2, `retriesMax` 5, `keepalive` true), per platform; this is how the
  agent decides to start the Plugin Service.
- `depots_unpacked/features_config/features.json`: six region/flag gated feature configurations.
- `depots_unpacked/predefined_profiles/`: `profile_photoshop_essentials.json`,
  `profile_premiere_pro.json`, `card_presets_*.json` (action card presets).

### Data files (encrypted)

- 242 files in the main payload (`analysis/payload_encrypted_files.txt`): all
  `data/devices/devices_00000NN.json` (52 files, 1.3 to 45 KB), `data/card_presets/card_presets_001.json`,
  and 189 thumbnails named by UUID. Container: magic `21 05 21 20`, u32 LE header length,
  JSON header `{"key-id": "<uuid>", "file-sha": "<sha256>"}`, then ciphertext.
- 95 of the 217 depots (all UUID-named ones and all `*_strings.depot`) use the sibling
  container magic `06 05 21 20` with header `{"header-sha", "key-id"}` (`analysis/uuid_depot_headers.json`).
- `extract/depots/offline.json` carries a `keys` block: 42 entries `{name: <key-id uuid>,
  version, lastModified, key: <24-char base64>}` covering 39 of the 51 payload key ids and
  41 of the 54 depot key ids. The Creative Console descriptors, thumbnails and strings are in
  this encrypted set (the plaintext set has no c354/BC00 entry and no Creative thumbnails).
  Not decrypted in this spike; see legal log.

### Electron JS bundle

- `app.asar`: 581 entries; JS bundles `app.min.js` (11.7 MB), `cc.min.js` (10.9 MB),
  `520.min.js`, `46.min.js`, `main.js`, `preload.bundle.js`, `vendors.min.js`. All large
  bundles are javascript-obfuscator output (hex string-array lookups `_0x....(0x19a1)`), which
  is why token greps for 0x19A1, vlp, c354, bc00 hit (427/91/18/51 times). Manual context
  check of every `0x19a1` hit shows only lookup indices and base64. `package.json` dependencies
  are UI libraries (`@rjsf/*`, `electron-store`, `lottie-web`, `@sentry/*`); no `node-hid`/`usb`.
  Conclusion: no protocol logic in JS (high confidence).

### Depots

- 217 depot entries; named ones are per-device asset packs (`mx_master_3`, `k600`, ...:
  webp/svg/json), plugin packs (`plugin_photoshop`, `plugin_cep`, ...), `logi_plugin_service`
  (the LPS installer pkg, 130 MB), `logioptionsplus` (the main payload, 850 MB),
  `right_sight`, `logi_ai_portal`. No firmware blobs anywhere (extension census:
  webp 1140, json 480, svg 117, webm 65, strings 40, yaml 19, lua 10, dylib 5; the only
  `.bin` files are V8 snapshots). Firmware is fetched online by the DFU path, not bundled.
- The 96 UUID-named depots (encrypted) are per-device packs for newer devices; their names
  match the encrypted thumbnail UUIDs in the main payload.

## Findings on keyvo's open questions

Evidence kinds: FS = log format string, SYM = symbol name, DC = decompiled request builder
(Ghidra, `analysis/ghidra/decomp/<addr>.json`), DATA = data file, CAP = cross-check with
`hcooper-captures.md`.

### 1. VLP display feature 0x19A1

- Name: `ContextualDisplay` (SYM `devio::Feature19a1ContextualDisplay`, Ext; high).
- Function table (SYM method list + DC function numbers + CAP ordering):

| fn | name | request | response fields (FS) | confidence |
|---|---|---|---|---|
| 0 | getCapabilities | none | `deviceScreenCount`, `maxImageSize`, `maxImageFPS`; flags `rgb565`, `rgb888`, `jpeg`; `calibrated`, `origin`; `deferrableDisplayUpdate` (FS prints them in this grouping) | names high; byte widths see note A |
| 1 | getDisplayInfo | `index` (u8, DC third arg = 1) | `displayShape`, `dimension`, `resHorizontal`, `resVertical`; then `visible area i of n: shape, x, y, w, h`; then `button i of n: shape, x, y, w, h` | high (DC + FS) |
| 2 | setImage | `deferDisplayUpdate`, `count`, images[] (DC: fn = 2, payload = 3 header bytes + per image 12 header bytes + data, trailing arg 4 selects the long VLP report) | `resultCode` (u8), `count` | high |
| 3 | getSupportedDeviceStates | none | list of `supportedDeviceState` (u8 each) | high (CAP: 13 ids) |
| 4 | setDeviceState | `deviceState` (u8) | `deviceState` | high (CAP: `a0` echo + event) |
| 5 | getDeviceState | none | `deviceState` | high (DC third arg = 5; CAP: `a1` at boot) |
| 6 | setConfig | `deviceAdopted` (bool) | `deviceAdopted` | medium (ordering) |
| 7 | getConfig | none | `deviceAdopted` | medium (CAP step 6 `c0 00 01`) |
| event | onFeatureReport | `Event` with old/new device state (`[f19A1] onDeviceStateEvent 0x%02X '%s' -> 0x%02X '%s'`), key events (`onKeyEvent %d %d`) | | high |

- Device state names (FS string table, binary order): host states `Streaming`, `Standby`,
  `StandbyBright`, `Splash`, `SplashBright`, `OnboardingBright`; device-internal states
  `Unknown`, `USB Preconfiguration`, `Idle`, `Low Power`, `TDE embedded image`,
  `TDE embedded animation`, `TDE pattern`. Mapping to the captured ids
  `a0 a1 a2 a4 a5 a6 | d0 d1 d2 d3 d4 d5 d6`: `a0` = Streaming (host-painted; CAP: Options+
  sets `a0` before painting), `a1` = Standby (boot state; CAP), `a2` StandbyBright,
  `a4` Splash, `a5` SplashBright, `a6` OnboardingBright, `d0..d6` = the seven internal states
  in the order above (medium: six host names for six `aN` ids and seven internal names for
  seven `dN` ids, ordering inferred). `a3` is absent from the list; probably an unsupported
  "Onboarding" (non-bright) state.
- Image record layout (DC of setImage + CAP bytes `a0 | 01 00 01 | 00 | 00 17 | 00 06 |
  01 b3 | 01 b2 | 00 11 0a | ff d8 ...`): 3-byte command header (`deferDisplayUpdate` u8,
  `count` u16 BE, reading `01 00 01` as defer=1, count=1; order of the two is medium
  confidence), then per image 12 bytes: `format` u8 (0 = JPEG in the capture; `Rgb565`
  and `rgb888` are the other named formats), `x` u16, `y` u16, `w` u16, `h` u16, `length`
  u24, then the pixel data. Confirms the 24-bit length.
- Capability flags: `rgb565`, `rgb888`, `jpeg` are separate booleans, so the device may
  accept raw RGB565 (`Feature19A1::addRgb565Image` exists). `deferrableDisplayUpdate`
  advertises whether `deferDisplayUpdate` is honoured (batch several setImage calls, then
  flush).
- Note A, fn0 byte widths: from CAP `01 00 7a 1e 00 00 00 1b 01` a plausible reading is
  `deviceScreenCount` = 1, `maxImageSize` = u24 `00 7a 1e` = 31 262 bytes (or u16 `7a 1e`
  preceded by a pad), `maxImageFPS` = `00`?, then format/calibration flags in `1b` and
  `deferrableDisplayUpdate` = `01`. Low confidence. Three Ghidra batches (16 functions)
  reached only the request builders and the future/continuation boilerplate around the
  decode lambdas, not the lambda bodies themselves (they are unnamed local functions
  behind an xref); see section "deeper pass".
- fn1 (CAP first packet `01 00 60 01 e0 01 e0 09 00 | 01 00 18 00 06 00 76 00 76 | 01 00 b6 ...`):
  `displayShape` = 1, `dimension` = 0x0060, `resHorizontal` = 480, `resVertical` = 480, then
  counts, then 9-byte records `shape(1) x(2) y(2) w(2) h(2)`: key 1 at (24, 6) 118 x 118,
  key 2 at (182, 7) 118 x 118. The response is paged over several VLP packets (CAP: four
  packets); which of the two count bytes is visible-area count and which is button count
  needs the hardware re-decode (medium).
- VLP root fn1 `ProtocolCapabilities` (CAP `c0 01 00 00 01 e8 20`): major 1, minor 0,
  availableMemory 0x0001e820 = 124 960 bytes (medium). VLP root fn0 `GetFeature(0x19A1)`
  (CAP `c0 19 a1 02 00 00 00 01 e8 03`): index 2, hidden 0, version 0, maxMemory
  0x0001e803 (medium). VLP root fn2 = `Ping` with echoed payload (CAP step 4: the
  `5e..65` bytes are echoed, i.e. a ping, not a registration; medium).

### 2. Hidden Keypad-only feature 0x92E2

- Not referenced by name or constant in the agent, the Plugin Service helper, PluginApi.dll
  or any data file (strings grep: 0 hits for `92E2`/`Feature92E2` in `ext_strings.txt`; the
  68 lowercase `92e2` hits in the agent are hex noise inside base64/symbol hashes). The
  0x9xxx range in the agent holds only `9001 TestPMW3816` (engineering test). Conclusion:
  an engineering/manufacturing-test feature that Options+ never uses; safe to ignore (high
  that it is unused by this software; its meaning stays unknown).

### 3. Unknown feature names

| id | name | methods (SYM) | where | confidence |
|---|---|---|---|---|
| 0x1602 | PasswordAuthentication | startSession, passwd0, passwd1, endSession; FS "Session started: long=%d, full=%d, const=%d" | Ext only | high |
| 0x1807 | ConfigurableProperties | getPropertyInfo, selectProperty, readProperty, writeProperty, deleteProperty | agent | high |
| 0x1E02 | ManageDeactivatableFeatures | getInfo, getReactInfo, enableFeatures, disableFeatures; FS "[f1E02] Features enabled/disabled" | Ext only | high |
| 0x1EB0 | not present in either binary | | | n/a |
| 0x180B | not present in either binary | | | n/a |
| 0x4610 | MultiRoller (Dialpad dial/roller) | getCapabilities, getRollerCapabilities, getMode, setMode, getRollerSettings, setRollerSettings; nested `Roller`, `ReportingMode`, `RotationEvent`, `RollerId`; FS "[f4610] %d rollers:", "%d. roller %d", "Reporting mode set to (%d): %d", "Failed to divert special keys"; protobuf `multiroller.proto` | agent + Ext | high |
| 0x4600 | Crown (Craft dial; not on these devices) | `logi::devices::feature_4600_crown`, `crown.proto` | agent | high |

The helper enables hidden features at start (`[f1E00] Features enabled %d`, 1E02
enable/disable, 1602 session), which explains why Options+ captures show more than Solaar.

### 4. VLP image transfer, keep-alive, idle-dim

- Length field: 24-bit per image record (DC + CAP; high). Fragmentation is done by the
  `devio` VLP layer ("vlp payload is %d and packet count is %d"); the per-PID `VlpWindow`
  config item sets how many packets are in flight before waiting for ACKs (name only; the
  default value is a runtime constant, not a string; medium that it is a window size).
- Flow control strings: ACK-driven ("buffered response detected with timeout",
  "VLP response too short"), matching the per-fragment `c<n>` ACKs in the captures.
- Size limit: `[f19A1] Image is too big: %d or more, limit %d` with `getMaxImageSize()` from
  fn0 `maxImageSize`; frame pacing `[f19A1] Sleep after image sending for %d ms` derived from
  fn0 `maxImageFPS` (names high). DC of `Feature19A1::sendImageThread`: the size check
  compares against the runtime `maxImageSize` value, no literal limit in code (high).
- Keep-alive: `Feature0008SwitchAndKeepAlive::getTimeoutRange(*timeoutMin, *timeoutMax)`,
  `keepAlive(*requestedTimeout, *finalTimeout)`, `terminate`, event `onKeepAliveTimeout`
  (FS, both binaries). `DeviceC354::startKeepAlive(u16, u16)` logs "Keep-alive timer started
  %d %d" (requested timeout and timer period); CAP shows 3000 ms requested, ping every
  ~1 s. The JSON model to the .NET side carries `keepAliveTimeoutMin`/`keepAliveTimeoutMax`
  (FS). DC of `DeviceC354::startKeepAlive(u16, u16)` and its timer lambda: both arguments
  are stored on the device object and handed to `Feature0008::keepAlive` on every timer
  tick; there is no numeric literal in the helper, so the 3000 ms / ~1 s pair seen in the
  captures is chosen by the .NET service side (high that it is not hard-coded here).
- Idle dim: no "dim"/"idle timeout" string on the host side; the device-internal states
  `Idle` and `Low Power` (d-range) are reported by the device through the 0x19A1 event, so
  dimming is firmware-driven and only observable, not configurable from the host (medium).
- Brightness: `Feature8040BrightnessControl::getInfo`, `getBrightness`, `setBrightness`,
  `getIllumination`, `setIllumination`, nested `Info`, `Report`; `[LC] SetBrightness %d`
  (SYM/FS; high).

### 5. Device descriptors for c354 and BC00

- Plaintext descriptors: none (grep of every plaintext JSON/YAML: 0 hits for c354/bc00;
  `devices_initial.json` has 118 devices, none Creative). The Creative Console entries are in
  the encrypted `devices_00000NN.json` set and the encrypted UUID depots (DATA; high).
- What is known from plaintext: LPS `config.json` activation list `2bc00`, `8c354`
  (DATA; high); modelId mask scheme (section 3; medium); `PluginApi.dll` enum members
  `MxCreativeKeypad` ("MX Creative Keypad"), `MxCreativeDialpad` ("MX Creative Dialpad"),
  `LogitechCreativeFamily` ("Logitech Creative Family") (SYM; high).
- Device model exposed by the helper to .NET (JSON keys, FS): `displayName`, `serialNumber`,
  `firmwareVersion`, `firmwareInfo {versionString, buildString, prefixString, version, entity}`,
  `maxImageFps`, `maxImageSize`, `displayArea`, `buttonsAreas[]` (`AreaModel`:
  `location {x, y}`, `width`, `height`), `keepAliveTimeoutMin`, `keepAliveTimeoutMax`,
  `deviceAdopted`, `isActive`, `deviceState`, `imageFormat`, `brightness`; commands
  `SetDisplayImage`, `ClearScreen`, `SetBrightness`, `GetBrightness`, `SetDeviceState`,
  `GetDeviceState`, `WakeUp`, `GoSleep`, `EnterDfuMode`, `EnableLogging`; callbacks
  `DeviceConnected`, `DeviceDisconnected`, `onButtonPressedCallback`,
  `onButtonReleaseCallback`, `onEncoderRotatedCallback '%s' %d %d`,
  `onDeviceStateChangedCallback`, `onKeepAliveTimeoutCallback`, `onDfuTimeoutCallback`.
- Dialpad path: the helper links `devio::UnifyingBus` (Bolt receiver) and `BleppBus`
  (BLE); `DeviceBC00::onRollerRotated` consumes 0x4610 `RotationEvent`s and
  `onSpecialKeyPressed` the 0x1B04 diverted CIDs (SYM; high).

## What a deeper pass would cost and whether it is worth it

- Done in this spike: Ghidra auto-analysis of the arm64 slice of `LogiPluginServiceExt`
  (3 min) and 16 single-function decompiles at about 2.5 min each (REA reloads the
  project per call): the request builders of getCapabilities/getDisplayInfo/setImage/
  getDeviceState/getSupportedDeviceStates (function numbers 0, 1, 2, 5, 3 and the setImage
  payload layout came from these), `DeviceC354::startKeepAlive` plus its timer lambda,
  `Feature19A1::sendImageThread`, and eight std::function/continuation wrappers around
  the response lambdas that turned out to be boilerplate. Summaries:
  `analysis/ghidra/decomp_batch{2,3,4}.txt`, raw output `analysis/ghidra/decomp/`.
- Remaining concrete gaps and their cost:
  1. Exact byte widths of fn0, fn1 and fn7 responses: the `$_0`/`$_1`/`$_7` lambda bodies
     are unnamed local functions; reaching them needs an xref hop from the wrappers at
     `0x1003bb97c` / `0x1003bd404` / `0x1003c79d0` (Ghidra headless script or the GUI,
     about 15 min) plus 3 decompiles. Worth it only if the M0 hardware re-decode leaves
     ambiguity; the field names already pin the semantics.
  2. Keep-alive and FPS numeric defaults: 1 to 2 decompiles (5 min). Nice to have; keyvo
     measures these on hardware anyway and keeps them in `[timing]`.
  3. 0x4610 request/response layouts for the Dialpad roller (getCapabilities,
     getRollerCapabilities, setMode): 4 decompiles (10 min) plus a capture on the Bolt
     receiver. Worth doing before the Dialpad milestone; nothing in the public HID++ docs.
  4. VLP fragmentation rules (window, sequence byte semantics `e0/a0/61`, ACK timeout):
     the `devio` VLP layer in the helper, maybe 6 to 8 decompiles (20 min). Medium value:
     the captures plus ACK-driven flow already give a working model.
  5. Decrypting the device descriptor JSON: technically trivial (keys ship in
     `offline.json`, likely AES-128 given 16-byte keys), but it is circumvention of a
     protection measure on data, not decompilation for interoperability; needs an explicit
     owner decision and probably is not needed, since the device publishes its geometry.
- Not worth it: the 171 MB agent (nothing Creative-specific), the Electron bundle, the
  .NET service internals (UI/plugin logic), firmware (not bundled).

## Legal-boundary log

- Purpose and scope: interoperability of keyvo with the Keypad and Dialpad only. Analysed:
  file inventories, data-file schemas, string tables, symbol names, protobuf type names,
  and 16 decompiled functions, all in `LogiPluginServiceExt` and all belonging to the
  0x19A1 display feature, the 0x0008 keep-alive timer or the image send thread.
  Decompiler output stays in
  `analysis/ghidra/decomp/` and is not reproduced here; the report carries names, numbers
  and layouts only.
- Skipped on purpose (out of scope for interoperability): licensing/account code
  (`logi::accounts`, `accounts.proto`, Loupedeck5Auth/Cloud DLLs), telemetry
  (`logi::scarif`, `analytics.proto`, `device_analytics_config.json`, Sentry in Electron),
  updater (`logioptionsplus_updater`, `updates.proto`), crash reporting (`logi_crashpad_handler`,
  `crash_reporting.proto`), cloud/marketplace (`cloud.proto`, `marketplace.bundle.js`), UI.
- Encrypted data files and depots were inventoried (container format, header fields, key-id
  coverage) but **not decrypted**, although `offline.json` ships the keys. Reason: this is a
  protection measure on data rather than program code; Art. 6 covers decompilation for
  interoperability, and the device geometry is obtainable from the device itself. Owner
  decision required before going further.
- No Options+ or Plugin Service binary was executed; no network access; nothing written to
  the keyvo repository. All work on the host used `python3 -I`, `strings`, `grep`,
  `c++filt`; Ghidra/REA ran inside the `keyvo-re` distrobox.
- Third-party components noticed, for the licence ledger: HidSharp (.NET), SkiaSharp,
  HarfBuzzSharp, nlohmann::json, spdlog, websocketpp, asio, boost, protobuf, cpr, pugixml,
  yaml-cpp, crashpad, Qt 6. No code was copied.

## Reference paths

- Agent strings and symbols: `analysis/strings/` (`agent_strings_n6.txt`, `agent_strings_text.txt`,
  `mangled_symbols.txt`, `demangled_symbols.txt`, `devio_feature_classes.txt`,
  `devio_feature_methods.txt`, `feature_class_names.txt`, `protocol_types.txt`).
- Plugin Service: `analysis/lps_pkg/` (unpacked pkg), `analysis/lps/` (`ext_strings.txt`,
  `ext_demangled.txt`, `ext_nonsymbol.txt`, `pluginapi_ascii.txt`, `pluginapi_utf16.txt`).
- Depots: `analysis/depot_index.tsv`, `analysis/depots_unpacked/` (`_survey.json`,
  `_other_magic.txt`), `analysis/uuid_depot_headers.json`, `analysis/payload_encrypted_files.txt`.
- Electron: `analysis/asar/`, `analysis/asar_filelist.txt`.
- Ghidra: `analysis/ghidra/` (`LogiPluginServiceExt_arm64`, `ext.snapshot`, `analyze_ext.json`,
  `search_19a1.json`, `search_ka.json`, `decomp/`, `decomp_batch2.txt`, `decomp_summary.py`).
