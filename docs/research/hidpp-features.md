# HID++ feature inventory of the author's devices (2026-09-09)

Source: Solaar 1.1.20 (Flathub `io.github.pwr_solaar.solaar`) on Aurora 44 / KDE Wayland.
Raw dumps: `solaar-show.txt` (`solaar show`, crashes on the Dialpad, Solaar bug in
`settings_templates.check_feature_settings`), `hidpp-features-dump.txt` (raw
`FEATURE_SET.getFeatureID` walk via Solaar's `logitech_receiver` library),
`hidpp-reprog-controls-dump.txt` (`REPROG_CONTROLS_V4.getCidInfo` walk).
Feature names come from Solaar's `SupportedFeature` table; "unknown" means Solaar has no
name for it, not that it is undocumented elsewhere.

## MX Creative Keypad (USB 046d:C354, HID++ 4.5, hidraw direct)

Firmware: `U1 66.00.B0017`, bootloader `BL2 20.00.B0017`, unit ID `33005400`.

| idx | feature | ver | flags | relevance for keyvo |
|---|---|---|---|---|
| 0 | 0x0000 ROOT | 0 | | |
| 1 | 0x0001 FEATURE_SET | 0 | | |
| 2 | 0x0003 DEVICE_FW_VERSION | 7 | | `keyvo probe` |
| 3 | 0x0005 DEVICE_NAME | 5 | | |
| 4 | **0x0008 KEEP_ALIVE** | 1 | | Standard HID++ keep-alive feature exists. Spec Q2 ("keys revert to logo") is probably answered by this feature: `keepAlive()` / `getTimeout()`. Measure. |
| 5 | 0x0011 PROPERTY_ACCESS | 0 | | |
| 6 | 0x00C3 DFUCONTROL | 1 | | firmware update, out of scope |
| 7 | 0x1602 unknown | 0 | | present on all four devices |
| 8 | 0x1802 DEVICE_RESET | 0 | hidden | |
| 9 | 0x1805 OOBSTATE | 0 | hidden | |
| 10 | 0x1807 unknown | 0 | hidden | |
| 11 | **0x1B04 REPROG_CONTROLS_V4** | 6 | | 2 controls: CID `0x01A1` and `0x01A2` = the page buttons (matches logilinux `P1_LEFT=0xa1`, `P2_RIGHT=0xa2`). Divertable. |
| 12 | 0x1E00 ENABLE_HIDDEN_FEATURES | 0 | hidden | |
| 13 | 0x1E02 unknown | 0 | hidden | |
| 14 | 0x1EB0 unknown | 0 | hidden | |
| 15 | **0x8040 BRIGHTNESS_CONTROL** | 1 | | LCD brightness. Spec §5 brightness command comes from here, not from a vendor report. |
| 16 | 0x92E2 unknown | 0 | hidden | Keypad-only, hidden. Candidate for the LCD image channel or display config. Capture under Options+ to find out. |
| 17 | 0x180B unknown | 0 | hidden | |

Note: `solaar show` prints unknown feature IDs byte-swapped in braces (`unknown:1602 {0216}`);
the raw walk confirms the real IDs are as listed above.

## MX Creative Dialpad (Bolt WPID BC00, HID++ 4.5, kind reported as "headset")

Firmware: `RBO 07.00.B0016`, bootloader `BL2 18.00.B0016`, unit ID `B978A829`. Bolt receiver
046d:C548 at `/dev/hidraw9`.

| idx | feature | ver | flags | relevance for keyvo |
|---|---|---|---|---|
| 2 | 0x0003 DEVICE_FW_VERSION | 7 | | |
| 4 | 0x1D4B WIRELESS_DEVICE_STATUS | 0 | | connect/disconnect notifications |
| 5 | 0x0020 CONFIG_CHANGE | 0 | | |
| 6 | 0x0021 CRYPTO_ID | 1 | | |
| 8 | 0x0011 PROPERTY_ACCESS | 0 | | |
| 9 | 0x1004 UNIFIED_BATTERY | 5 | | battery badge |
| 10 | **0x1B04 REPROG_CONTROLS_V4** | 6 | | 4 divertable controls: CID `0x0053`, `0x0056`, `0x0059`, `0x005A` (all group 1, mouse-type, no raw_wheel flag). These are the Dialpad's buttons. |
| 11 | 0x1814 CHANGE_HOST | 1 | | |
| 12 | 0x1815 HOSTS_INFO | 2 | | |
| 13 | **0x4610 unknown** | 1 | | Dialpad-only, not hidden. Neither the dial nor the roller appear in 0x1B04, so this is the prime candidate for dial/roller configuration and events. Capture Options+ traffic to this feature index. |
| 14 | 0x00C3 DFUCONTROL | 1 | | |
| 15-29 | hidden engineering features (0x1802, 0x1803, 0x1807, 0x1816, 0x1805, 0x1830, 0x1891, 0x18A1, 0x1E00, 0x1E02, 0x1602, 0x1EB0, 0x1861, 0x18B1, 0x9205) | | hidden | ignore |

Solaar maintainer's statement (issue #3100) that "only some keys/buttons can be diverted" is
consistent: 4 buttons via 0x1B04, dial/roller elsewhere (0x4610 or plain HID reports).

## MX Master 4 (Bolt WPID B042)

Relevant: 0x19B0 HAPTIC v0 (haptics, deferred past v1), 0x19C0 FORCE_SENSING_BUTTON,
0x2111 SMART_SHIFT_ENHANCED, 0x2121 HIRES_WHEEL, 0x2150 THUMB_WHEEL, 0x2201 ADJUSTABLE_DPI,
0x1B04 with 9 controls (thumb button CID 0x00C3, gesture button 0x00C4, etc.).

## MX Mechanical (Bolt WPID B366)

Relevant: 0x1982 BACKLIGHT2 v3, 0x40A3 K375S_FN_INVERSION, 0x4531 MULTIPLATFORM,
0x1B04 with 41 controls.

## Consequences for the plan

1. **Keep-alive**: the Keypad exposes HID++ 0x0008 KEEP_ALIVE. The TS quick-test must
   measure (a) whether the LCD reverts to logo without traffic, (b) whether calling
   `keepAlive` (0x0008 fn 1) prevents it, (c) the `getTimeout` value (0x0008 fn 0).
2. **Brightness**: 0x8040 BRIGHTNESS_CONTROL v1 (`getInfo`, `getBrightness`, `setBrightness`,
   possibly `getIllumination`). Implement in `keyvo-hid` from the public HID++ feature
   docs plus capture, not from a vendor report.
3. **Page buttons**: standard 0x1B04 divert. logilinux's `11 ff 0b 00 01 a1|a2` report is the
   diverted-button notification of 0x1B04 at feature index 0x0b (matches index 11 above).
4. **Dialpad buttons**: 0x1B04 divert on the Bolt receiver's hidraw (device index of the
   Dialpad on the receiver, currently slot 3). Dial and roller: capture 0x4610 under Options+.
5. **Solaar coexistence**: Solaar enumerates the Keypad and may poll it; stop Solaar during
   keep-alive measurements. Do not enable diverts in Solaar for these devices.
6. **Solaar bug**: `solaar show` crashes on the Dialpad in 1.1.20 (`UnboundLocalError` in
   `settings_templates.py:4703`). Worth reporting upstream with the dump above.
