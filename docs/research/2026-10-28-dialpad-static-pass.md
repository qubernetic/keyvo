# Logi Options+ static pass over the MX Creative Dialpad features

Pass date: 2026-10-28 (issue #185, roadmap M3-0). Follow-up of `options-plus-inventory.md`
(2026-10-24) and `options-plus-display-pipeline.md` (2026-10-25): the inventory's deeper-pass
item 3. Target: the same unpacked macOS offline installer, Options+ **2.8.981479** bundling **Logi
Plugin Service 6.4.2.3414**. Two binaries were analysed: the Plugin Service's native helper
`LogiPluginServiceExt` (arm64 slice, the Creative Console driver: `DeviceBC00`, `Feature4610`,
`Feature1B04`, `devio::UnifyingBus`) and the Options+ agent `logioptionsplus_agent` (arm64 slice at
file offset 89866240, 81387888 bytes), which carries the only `0x4610` settings functions and the
`multiroller.proto` descriptor. Purpose: interoperability of keyvo with the MX Creative Dialpad
(Bolt WPID `BC00`, Bluetooth `046d:bc00`). Boundary: ADR-0008
(`docs/adr/0008-static-analysis-boundary.md`); legal-boundary log at the end.

Method: symbols and strings first (`analysis/lps/ext_demangled.txt`, `ext_strings.txt`,
`analysis/strings/demangled_symbols.txt`); the serialized protobuf `FileDescriptorProto` of
`logi/protocol/multiroller.proto` decoded from the agent's data section by a wire-format reader
(names, numbers, types only); Ghidra 12.1.4 headless with the `DumpFacts.java` script of the
display-pipeline pass for targeted decompiles, in seven passes over the helper (the existing
project) and one over the agent (a new project). Everything ran inside the `keyvo-re` distrobox; no
Logitech binary was executed, no Logitech server contacted, nothing decrypted.

Evidence kinds: SYM = symbol name or demangled signature, FS = log format string, PB = protobuf
descriptor, DC = decompiled native function (Ghidra), STR = string table order, TP = third-party
reading (shensquared, Julusian PR #22, Solaar), already in spec §5.8. Confidence: high, medium, low
per claim.

## Summary

1. The Plugin Service drives the Dialpad with four `0x4610` functions only: fn0
   `getCapabilities`, fn1 `getRollerCapabilities`, fn2 `getMode`, fn3 `setMode`. It never calls
   fn4/fn5 (`getRollerSettings`, `setRollerSettings`); those exist only in the Options+ agent's copy
   of `devio`.
2. `setMode` sends two bytes: `roller & 0x0F` and `mode & 0x01`. `ReportingMode` is one bit:
   `NATIVE = 0`, `DIVERTED = 1` (PB). There is no second mode bit for raw or high-resolution deltas.
3. The rotation event is notification 0 of the feature: byte 0 low nibble = roller, byte 1 = delta
   as a signed 8-bit value, bytes 2..5 = a 32-bit big-endian timestamp, optional (a 2-byte event is
   valid, its timestamp is 0). The helper drops the timestamp and passes the delta on unchanged: no
   host-side acceleration, no sign flip.
4. On start the Dialpad's four `0x1B04` controls are diverted with flags `0x03`, the rollers with
   `setMode(roller, DIVERTED)`; on stop `0x02` and `NATIVE`. Every connect builds a new `DeviceBC00`
   and re-sends all diverts; a disconnect destroys it.
5. Bolt: slots come from the receiver's `0x41` arrival notifications. Enumeration reads register
   `0x02` and writes `0x02` to it, so the receiver replays `0x41` for every paired device. The Bolt
   pairing information is long register `0xB5`, sub-address `0x51 + slot index` (0-based), WPID in
   bytes 2..3 little-endian.

## Facts table

| # | Question | Answer | Evidence | Confidence |
|---|---|---|---|---|
| 1 | fn0 `getCapabilities` | Request: no parameter bytes. Response byte 0, low nibble = roller count (`unsigned char`). No flags are read. | DC (builder, decode lambda) | high |
| 2 | fn1 `getRollerCapabilities(roller)` | Request byte 0 = `roller & 0x0F`. Response byte 0 = increments per rotation, byte 1 = ratchet increments, byte 2 low nibble and bit 4 unnamed; see section 2. Timestamp support or acceleration flags: not named in either binary. | DC, FS | high (bytes 0, 1), n/a (byte 2) |
| 3 | fn2 `getMode`, fn3 `setMode` | fn2 request byte 0 = `roller & 0x0F`; response byte 0 bit 0 = mode. fn3 request byte 0 = `roller & 0x0F`, byte 1 = `mode & 0x01`; empty response accepted. `ReportingMode`: `NATIVE = 0`, `DIVERTED = 1`. The Plugin Service sets `DIVERTED` for every roller on start and `NATIVE` on stop. | DC, PB, FS `[f4610] Reporting mode set to (%d): %d` | high |
| 4 | fn4 `getRollerSettings`, fn5 `setRollerSettings` | Fields: direction (bit 0), increments_per_rotation_conf, ratchets_per_increment_conf; see section 4. The Plugin Service does not call them; whether the Dialpad honours them is measured in M3-2. | DC, FS, SYM | high (layout, absence), medium (names of bytes 2, 3) |
| 5 | RotationEvent | Event 0 only (other event numbers ignored); payload shorter than 2 bytes is logged as `invalid event packet size` and dropped; byte 0 `& 0x0F` = roller, byte 1 = delta (`signed char`), bytes 2..5 = timestamp `u32` big-endian when the payload has at least 6 bytes, else 0. One event per roller. Timestamp unit and wrap: not determinable statically (only `uint64 timestamp` in the proto); measure in M3-2. Whether the firmware accelerates the delta: not determinable statically; measure in M3-2. | DC (`Feature4610MultiRoller::onFeatureReport`), SYM `Feature4610::onRotationEvent(unsigned char, signed char, unsigned int)`, PB `MultiRollerRotationEvent` | high (layout), n/a (unit, acceleration) |
| 6 | RollerId, sign | Roller ids are the enumeration indices 0..count-1 (the helper loops `0..fn0 count` and uses the index echoed into the fn1 result as the id). The helper reports roller `n` to the .NET side as encoder `n + 1` with the delta unchanged. Which id is the dial and which the small roller, and the sign convention: not determinable statically; the helper applies no sign. The agent has `RollerSettings.Direction` (`NORMAL`, `INVERTED`) and `is_invertable()`, so Options+ inverts in firmware settings, not in the delta path. TP (roller 0 = small wheel, opposite sign) stays the working assumption; verify in M3-2. | DC (`Feature4610::divertAllSpecialKeys`, `DeviceBC00::onRollerRotated`), PB, SYM | high (ids, no host sign), n/a (physical mapping) |
| 7 | `0x1B04` controls | `DeviceBC00::fixSpecialKeyId` maps CID `0x0053` to button 1, `0x0056` to 2, `0x0059` to 3, `0x005A` to 4; any other CID logs `[dBC00] Unknown special key %d` and maps to 0. The helper names no control and reads no TID. Divert: `GetCtrlIdReporting` first (logged), then `SetCtrlIdReporting(cid, flags, remap 0)` with flags `0x02 \| divert`, so `0x03` to divert and `0x02` to release; persist and raw-XY bits are never set. Presses are edges of the held set (`onKeyChange`): a control already held is not pressed again, so firmware repeats of a held control produce no new press. | DC (`fixSpecialKeyId`, `Feature1B04::divertSpecialKey`, `onKeyChange`) | high |
| 8 | Bolt slot discovery | See section 8. | DC, FS | high |
| 9 | Sleep and wake | `DeviceBC00` has no sleep logic of its own. A request that fails with a soft error is retried after a ping, a configured number of times (`devio::Configuration` items, values not in the function); a device silent for those tries produces `onUnresponsive` and is quarantined. Soft errors: see section 9. A `0x41` with the link-lost bit, or a disconnect, removes the device; the next connect re-creates it and re-sends every divert (see summary 4). | DC (`wt_process_request_with_retry`, `is_soft_error`, `DeviceCollection::onDeviceConnected`, `addDevice<DeviceBC00>`), FS, STR | high (re-divert), medium (error names), n/a (retry counts) |
| 10 | `multiroller.proto` | Section 10; names the fields `min_speed`, `max_speed`, `current_speed`, `direction`, `num_physical_rollers`, `apply_same_settings_to_all_rollers` the questions above did not. | PB | high |

## 2. fn1 getRollerCapabilities

- Request: byte 0 = `roller & 0x0F`. (DC, high)
- Response, as the helper decodes it (at least 3 bytes): byte 0 = **increments** per rotation,
  byte 1 = **ratchet increments**, byte 2 low nibble = a 4-bit field, byte 2 bit 4 = a flag. The
  agent's newer copy of `devio` requires at least 5 bytes and also keeps byte 3 and byte 4. The
  names of bytes 0 and 1 come from the agent's log line `roller %d: increments=%d,
  ratchet_increments=%d`, whose arguments are bytes 0 and 1 of the decoded `Roller`. The byte 2
  nibble, the bit 4 flag and bytes 3..4 have no name in either binary. (DC, FS; high for bytes 0
  and 1, n/a for the rest)
- Cross-check with the third-party replies `28 00 1f 00` and `b4 00 1f 00`: 40 and 180 increments,
  0 ratchet increments (the Dialpad has no ratchet), byte 2 `0x1F` = nibble `0xF` plus flag set on
  both rollers. (TP, consistent)
- The proto's `RollerCapabilities.min_speed` and `max_speed` are not filled from bytes 0 and 1 in
  any function read here; which response bytes they come from was not determined.

## 4. fn4 and fn5

- Only the Options+ agent has them (`devio::Feature4610MultiRoller::getRollerSettings`,
  `setRollerSettings`); the Plugin Service helper has no symbol for either, so Options+ never
  changes the Dialpad's roller settings through the Plugin Service. Whether the agent's own
  `feature_4610_multi_roller` class attaches to `BC00` was not determined (the Plugin Service owns
  the device; spec §5.8 already says Options+ is not needed). (SYM, high for the absence)
- fn4 `getRollerSettings(roller)`: request byte 0 = `roller & 0x0F`. Response (at least 4 bytes):
  byte 0 low nibble = roller, byte 1 bit 0 = **direction** (`NORMAL = 0`, `INVERTED = 1`), byte 2
  = **increments_per_rotation_conf**, byte 3 = **ratchets_per_increment_conf**. (DC; names from the
  log line `settings cached: direction=%d, increments_per_rotation_conf=%d,
  ratchets_per_increment_conf=%d`, whose arguments follow the struct order; high for the layout,
  medium for the byte 2 and byte 3 names)
- fn5 `setRollerSettings(roller, settings)`: request of 4 bytes, byte 0 = `roller & 0x0F`, byte 1 =
  1 when the direction is inverted else 0, bytes 2..3 = the two configuration bytes unchanged.
  (DC, high)
- Ratchet and sensitivity are therefore firmware settings per roller (increments per rotation,
  ratchets per increment); acceleration is not among them. Whether the Dialpad honours fn5:
  not determinable statically; measure in M3-2 (read fn4, write the inverted direction, read
  back).

## 8. Bolt slot discovery

- Receiver kind (`UnifyingBus::pairing_method`): PID `0xC548` (Bolt) and `0xC546` use method 0,
  the BLE Pro path; `0xC52F` and `0xC534` method 2; every other receiver method 1 (Unifying). (DC,
  high)
- Bolt pairing information (`HPP10Command::ble_pro_read_pairing_info`): short HID++ 1.0 request
  `10 FF 83 B5 <0x51 + index> 00 00`, i.e. read long register `0xB5` at sub-address `0x51..0x56`
  for the 0-based slot index (slot 1 is `0x51`). Long response, after the sub-address echo: byte 1
  low nibble = device kind, bytes 2..3 = WPID little-endian, bytes 4..7 = a 32-bit big-endian value
  (the serial in Solaar's reading), bytes 8..12 decoded into further fields whose names the binary
  does not carry. (DC, high for the request and the WPID offset; TP Solaar agrees)
- Unifying pairing information (`HPP10Command::read_pairing_info`): the same request with
  sub-address `0x20 + index`. Not used for a Bolt receiver. (DC, high)
- Connection state (`HPP10Command::read_receiver_connection_state`): `10 FF 81 02 00 00 00`;
  response byte 5 = connected devices, byte 6 = remaining slots (FS `connectedDevices=%d
  remainingSlots=%d`). `set_receiver_connection_state`: `10 FF 80 02 <v> 00 00` with `v = 0x02`
  when asked to enumerate, else `0x00`. Enumeration (`UnifyingBus::enumerate_devices_attempt`)
  reads the state, then writes `0x02`, which makes the receiver send a `0x41` for every paired
  device (the "fake arrival"; error name `UnifyingFakeArrivalMismatch` exists). (DC, high)
- Arrival notification (`UnifyingBus::onRead`), report of at least 7 bytes with byte 2 `0x41`:
  byte 1 = device index (1..max slots, 0 and above max ignored), byte 3 = protocol (value `0x02`
  is skipped), byte 4 low nibble = device type, byte 4 bit 6 (`0x40`) set = link not established,
  bytes 5..6 = WPID little-endian. It calls `perform_arrival(index - 1, wpid, type, protocol,
  connected)`. A report from a slot the bus holds as disconnected marks it connected
  (`perform_connect`, FS "was in disconnected state but is sending reports"), except an HID++ 1.0
  error report (`0x8F`), which is only logged. (DC, FS, high)
- Device classes are chosen by PID in `DeviceCollection::onDeviceConnected`: `0xBC00` builds
  `DeviceBC00`, `0xC354` builds `DeviceC354`. (DC, high)

## 9. Sleep and wake

- `DeviceBC00` (constructor, `onStart`, `onStop`, the key and roller handlers) contains no sleep,
  retry or reconnect logic; `onStart` diverts and subscribes, `onStop` releases and unsubscribes.
  (DC, high)
- Lifecycle: `DeviceCollection::onDeviceConnected` creates the device object and calls
  `DeviceBase::start`, which runs `onStart`; `onDeviceDisconnected` removes it. A device that
  sleeps and wakes therefore gets a fresh object and a fresh set of diverts on the connect that
  follows the wake. There is no keep-alive for `BC00` (no `0x0008` in the device class, consistent
  with spec §5.8). (DC, high)
- Retry loop (`devio::Device::wt_process_request_with_retry`): on a soft error it pings the device
  and retries ("will ping and retry %d more times"); between attempts it may delay ("delaying by %d
  milliseconds before next communication attempt", with a random component); after the configured
  number of silent tries it produces `onUnresponsive` and quarantines the device "for %d seconds" or
  "until it reconnects". `Hpp20Busy` responses can be ignored by a quirk flag (`IgnoreHpp20Busy`).
  The retry counts and delays are read from `devio::Configuration` items; their values are not in
  the function and were not chased. (DC, FS, high for the policy shape, n/a for the numbers)
- Soft errors (`devio::Device::is_soft_error`): `devio::ErrorCode` values 4, 6, 12, 15, 17, 39, 52,
  61, 69, 100, 106. Names from the order of the error-name string table, numbered from `Failure =
  1`: 4 `NoResponse`, 6 `CannotOpenHandleToDevice`, 12 `Hpp10ConnectFail`, 15 `Hpp10Busy`, 17
  `Hpp10ResourceError`, 39 `WriteTimeout`, 52 `Hpp20Busy`, 61 `SpuriousWriteError`, 69
  `SwidConflict`, 100 `CentppTimeout`, 106 `CentppMalformed`. The numbering is cross-checked twice:
  `UnifyingDevice::pairing_info` raises code 28 next to a `DevioExceptionDeviceRemoved` future (28
  = `DeviceRemoved`), and the `Hpp10*` block starts at 8 = `Hpp10Success`, so HID++ 1.0 error `n`
  is code `8 + n`: the receiver's "device unreachable" error `0x04` (`ERR_CONNECT_FAIL`) is soft,
  `0x08` (`Hpp10UnknownDevice`, code 16) is not. (DC, STR; medium for the names, high for the
  values)

## 10. multiroller.proto

Decoded from the agent's embedded `FileDescriptorProto` (package `logi.protocol.multiroller`;
field name, number and type only). These messages are the Options+ agent's model, not the wire
format; they name what the `0x4610` settings and capabilities mean. (PB, high)

| Message | Fields |
|---|---|
| `MultiRollerCapabilities` | `num_rollers` uint32 = 1, `num_physical_rollers` uint32 = 2, `apply_same_settings_to_all_rollers` bool = 3, `multi_roller_capabilities` repeated `RollerCapabilities` = 4 |
| `RollerCapabilities` | `roller_id` uint32 = 1, `min_speed` uint32 = 2, `max_speed` uint32 = 3 |
| `RollerMode` | `roller_id` uint32 = 1, `mode` enum `ReportingMode` = 2; `ReportingMode`: `NATIVE = 0`, `DIVERTED = 1` |
| `RollerSettings` | `roller_id` uint32 = 1, `direction` enum `Direction` = 2, `current_speed` uint32 = 3; `Direction`: `NORMAL = 0`, `INVERTED = 1` |
| `MultiRollerSettings` | `multi_roller_settings` repeated `RollerSettings` = 1 |
| `MultiRollerRotationEvent` | `device_id` string = 1, `device_prefix` string = 2, `roller_id` uint32 = 3, `delta` int32 = 4, `timestamp` uint64 = 5 |
| `RollerAdapter` | `win_adapter_card_id` string = 1, `osx_adapter_card_id` string = 2 |
| `RollerAdapterOverride` | `base` `RollerAdapter` = 1, `fn` `RollerAdapter` = 2 |

`num_physical_rollers` next to `num_rollers` suggests that a device may report more logical
rollers than it has physical ones (inference, low).

## What changes the plan

For slice 6 (the Dialpad driver, M3-1) and spec §5.8:

1. fn3 `setMode` carries one bit. keyvo sends `[roller & 0x0F, 0x01]` to divert and
   `[roller, 0x00]` to release; there is no raw or high-resolution mode to choose.
2. The rotation event layout is settled: roller in the low nibble of payload byte 0, `i8` delta in
   byte 1, optional `u32` big-endian timestamp in bytes 2..5. The driver must accept a 2-byte
   payload. Options+ ignores the timestamp; keyvo can do the same until M3-2 says what it is.
3. Options+ does no host-side acceleration and no sign flip. If the deltas look accelerated, the
   firmware does it; if one roller turns the "wrong" way, Options+ fixes it with fn5's direction
   bit, which keyvo may use or replace by a sign in the profile. Both stay M3-2 measurements.
4. Read the roller count from fn0 and the per-roller increments from fn1 at runtime instead of
   hard-coding 2 rollers, 40 and 180 (same rule as the Keypad geometry).
5. Diverts: `0x1B04` flags `0x03` (no persist bit), so a device reset or a reconnect loses them;
   re-send every divert on each connect, as Options+ does. Track the held set and report edges
   only, which also absorbs the firmware's repeats of a held button.
6. Bolt: find the Dialpad by WPID `BC00` from `0x41` arrivals; at startup write register `0x02`
   with `0x02` to make the receiver replay arrivals for paired devices; read `0xB5 0x51+n` only
   to confirm the WPID of a slot. Treat a `0x41` with bit 6 of byte 4 set as "asleep or out of
   range" and wait for the next `0x41` without it.
7. Errors: HID++ 1.0 `ERR_CONNECT_FAIL` (`0x04`), `ERR_BUSY` (`0x07`), `ERR_RESOURCE_ERROR`
   (`0x09`), HID++ 2.0 `BUSY` and timeouts are retryable; `ERR_UNKNOWN_DEVICE` (`0x08`) is not.

Open, for M3-2 on hardware: which roller id is the dial; sign per roller; whether the deltas are
accelerated in firmware; timestamp unit and wrap; whether fn5 is honoured and persists; the
meaning of fn1 byte 2.

## Legal-boundary log

- Purpose and scope: interoperability of keyvo with the MX Creative Dialpad only, the questions of
  issue #185. Analysed in the Plugin Service helper: the `devio::Feature4610MultiRoller` builders
  (`getCapabilities`, `getRollerCapabilities`, `getMode`, `setMode`), their four decode lambdas
  and `onFeatureReport`; `Feature4610` (`divertSpecialKey`, `divertAllSpecialKeys`,
  `subscribeToEvents`, `onRotationEvent`, `onRollerRotated`); `DeviceBC00` (constructor,
  `onStart`, `onStop`, `fixSpecialKeyId`, `onSpecialKeyPressed`, `onRollerRotated`);
  `DeviceC354::fixSpecialKeyId`; `Feature1B04` (`divertSpecialKey`, `divertAllSpecialKeys`,
  `onKeyChange`); `DeviceCollection` (`onDeviceConnected`, `onDeviceDisconnected`,
  `addDevice<DeviceBC00>`); `devio::HPP10Command` (`read_pairing_info`,
  `ble_pro_read_pairing_info`, `read_receiver_connection_state`, `set_receiver_connection_state`)
  and three of their decode lambdas; `devio::UnifyingBus` (`onRead` at the `0x41` branch,
  `enumerate_devices_attempt` and one continuation, `pairing_method`, `perform_arrival` signature,
  `get_pairing_info`); `devio::UnifyingDevice::pairing_info`; `devio::Device`
  (`wt_process_request_with_retry` for its strings and callees, `is_soft_error`, `errstring`). In
  the Options+ agent: `devio::Feature4610MultiRoller` (`getRollerCapabilities`,
  `getRollerSettings`, `setRollerSettings`) and their three decode lambdas;
  `logi::devices::feature_4610_multi_roller` (`_initialize_capabilities`, `_initialize_settings`,
  `_get_roller_settings`, for their log lines). The batch script decompiled more functions than
  these because name patterns matched template instantiations (helper passes 1 to 7: 118, 76, 4,
  84, 4, 2, 1; agent passes: 71, 3); only the functions named here were read.
- Data read without decompiling: the serialized `FileDescriptorProto` of
  `logi/protocol/multiroller.proto` in the agent's data section (names, numbers, types), and the
  `devio` error-name string table.
- Skipped on purpose: the agent's UI, profile, analytics, accessibility and gesture code around
  the roller class (`_compare_and_apply_roller_diversion`, `fire_trigger`, delayed termination),
  BLE pairing and discovery, DFU, the .NET side (the device descriptors stay encrypted and were not
  decrypted), licensing, accounts, telemetry, cloud, updater, crash reporting.
- Output discipline: this report carries names, numbers, layouts, enumerations, error names and
  evidence kinds only. Decompiler output stays under the spike's `analysis/dialpad/` and
  `analysis/ghidra/headless/`; no code, pseudocode, function body or copied comment entered the
  report or the repository.
- Hygiene: no Options+ or Plugin Service binary was executed; no network use during the analysis;
  the agent's arm64 slice was cut from the universal binary by its fat header, not by a vendor tool;
  nothing was written to the repository except this report and the spec and roadmap edits of the
  same PR.
- Third-party components noticed: protobuf, spdlog, cpprestsdk (`web::http`), in the agent. No code
  was copied.

## Reference paths

All under the spike root on the owner's machine (`~/.cache/keyvo-re-spike/`), not the repository.

- Previous reports: `report/options-plus-inventory.md`, `report/options-plus-display-pipeline.md`.
- Helper: `analysis/ghidra/LogiPluginServiceExt_arm64`, headless project
  `analysis/ghidra/headless/` (`proj.gpr`, `scripts/DumpFacts.java`).
- This pass: `analysis/dialpad/` (symlinked as `~/keyvo-re-dialpad` because Ghidra refuses
  project paths with a dot-prefixed element): `ext_targets.txt` and `ext_targets2..7.txt` with
  their outputs `ext_out*/` and logs `ext_run*.log`; the agent slice `logioptionsplus_agent_arm64`,
  its project `agentproj/`, `agent_targets.txt`, `agent_out/`, `agent_import.log`; the descriptor
  reader `pbdesc.py` and its output `multiroller_proto.txt`; the fat-header reader `fat.py`.
- Strings and symbols: `analysis/lps/ext_strings.txt`, `ext_demangled.txt`,
  `analysis/strings/demangled_symbols.txt`.
