# Logi Options+ display update pipeline (MX Creative Keypad)

Spike date: 2026-10-25. Follow-up of `options-plus-inventory.md` (2026-10-24). Target: the same
unpacked macOS offline installer, Options+ **2.8.981479** bundling **Logi Plugin Service 6.4.2.3414**
(`LogiPluginService.app`): the native helper `LogiPluginServiceExt` (arm64 slice) and the .NET
assemblies `LoupedeckService.dll`, `LoupedeckShared.dll`, `PluginApi.dll`. Purpose: interoperability
of keyvo with the MX Creative Keypad (USB 046d:c354) and Dialpad. Boundary: ADR-0008
(`docs/adr/0008-static-analysis-boundary.md`); legal-boundary log at the end.

Method: strings and symbols first (`analysis/lps/ext_strings.txt`, `ext_demangled.txt`); Ghidra
12.1.4 headless (`analysis/ghidra/headless/`, scripts `DumpFacts.java` and `DumpAsm.java`) for a
batch decompile of the native helper driven by symbol and string-xref patterns, which reached the
unnamed response-decoding lambdas the inventory could not; ILSpy 9.1 (`ilspycmd`) for the .NET
assemblies (`analysis/ilspy/`). Cross-checks: the hcooper capture
(`captures/external/hcooper/derived/binding-paint-frames.tsv`, `docs/research/hcooper-captures.md`)
and the owner's first-light probe logs (`~/keyvo-firstlight-2026-10-25/0[124]-probe-*.log`,
firmware `U1 66.00.B0017`). Everything ran inside the `keyvo-re` distrobox; no Logitech binary was
executed, no Logitech server contacted, nothing decrypted.

Evidence kinds: FS = log format string, SYM = symbol name, DC = decompiled native function (Ghidra),
DCS = decompiled C# (ILSpy), RES = embedded resource file, CAP = capture or hardware-log cross-check.
Confidence: high, medium, low per claim.

Note on the .NET side: the assemblies are obfuscated. Control flow is flattened and most numeric
constants are read from a table that the obfuscator stores as an AES-encrypted embedded resource
(`CryptoStream`/`Rijndael` in the loader class). That table was **not** decrypted (brief and ADR
rule: nothing is decrypted). Constants that are only reachable through it are marked "obfuscated
constant" below; where the same table index is used in two typed contexts the value is inferred and
marked as inferred. Names of classes, methods, enums and string-keyed JSON fields are intact.

## Summary

Options+ does not paint nine keys. On a profile, page or mode switch the .NET service marks all nine
buttons dirty, draws each button as a 116x116 image into a 480x480 black canvas at the button's
device-reported origin, takes the **bounding box of the drawn buttons** (435x434 at 23,6 on this
device), encodes that one rectangle as a baseline JPEG (quality 100, 4:2:0) and sends it as **one**
`SetDisplayImage` message to the native helper, which sends **one** 0x19A1 fn2 `setImage` command
with one image (`displayIndex` 1, `deferDisplayUpdate` 0, `count` 1). The device renders one image,
so there is no wipe between background and keys and no key-by-key appearance. If the encoded
rectangle exceeds `maxImageSize` the canvas crop is split into horizontal bands, each its own
command. On a single-button change (icon, label, pressed feedback) the same path runs with one dirty
button: the bounding box is that button's 118x118 area, one JPEG, one command; the pressed feedback
is the same icon shrunk by a fixed ratio and centred on black. The deferred-update flag exists in
the protocol but Options+ never sets it; there is no multi-image command in practice although the
protocol allows up to 255 images per command. Pacing is not the trick: the helper sends each image
as soon as it is queued (configurable sleep, default 0 ms) and keeps at most two pending images.

## 0. Deferred display update and the fn0 byte widths

- **fn2 `setImage` request layout** (DC of `devio::Feature19a1ContextualDisplay::setImage`, high;
  CAP `01 00 01 | 00 | 00 17 | 00 06 | 01 b3 | 01 b2 | 00 05 b4 | ff d8 ...` in hcooper frame 521):
  - byte 0: `displayIndex` (u8). Options+ always sends **1** (same index as `getDisplayInfo(1)`).
  - byte 1: `deferDisplayUpdate` flag in **bit 0** (u8; only bit 0 is written, the rest is left 0).
    Options+ always sends **0** (DC of `Feature19A1::sendImages`: the call passes index 1, defer 0).
  - byte 2: `count` (u8) = number of images in the command.
  - per image, 12 bytes: `format` (u8, low nibble only), `x` (u16 BE), `y` (u16 BE), `w` (u16 BE),
    `h` (u16 BE), `length` (u24 BE), then `length` bytes of pixel data.
  - The inventory's reading "defer=1, count u16" was wrong; the first byte is the display index.
- **fn2 response**: byte 0 `resultCode`, byte 1 `count` (DC of the setImage decode lambda, high).
  `resultCode` 0xFE means "invalid device state": the helper then reads the device state and logs
  `[f19A1] Invalid device state: 0x%02X` (DC of `sendImages`, high). CAP (medium): the ACK of the
  last fragment of a keyvo write, `c<n> 00 01`, is this response (resultCode 0, count 1); the acks
  of the earlier fragments carry `00 00`.
- **Flush**: there is no separate flush function in the eight-function table (SYM, high). A deferred
  image can only be shown by a later `setImage` with the flag clear (inference, low). How many
  images may be deferred is not visible in the host software because it never defers (high that it
  is unused; the limit itself is unknown).
- **fn0 `getCapabilities` response layout** (DC of the decode lambda at `0x1003bbebc`, high; the
  helper pads the payload to 8 bytes and reads bytes 0, 1, 2, 3 and 7):

  | byte | field | width and unit |
  |---|---|---|
  | 0 | `deviceScreenCount` | u8 |
  | 1..2 | `maxImageSize` | u16 BE **in units of 1024 bytes** (the value is shifted left by 10) |
  | 3 | `maxImageFPS` | u8 |
  | 4..6 | not read by the host | |
  | 7 | flags | bit 0 `deferrableDisplayUpdate`, bit 1 `rgb565`, bit 2 `rgb888`, bit 3 `jpeg`, bit 4 `calibrated`, bits 5..6 `origin` (2-bit) |

  The bit-to-name assignment follows the response struct's field order, which matches the order of
  the log lines (medium; the alternative orders make `jpeg` 0, which the capture refutes).
  CAP (hcooper step 9: `01 00 7a 1e 00 00 00 1b 01`): 1 screen, `maxImageSize` 0x007a x 1024 =
  **124 928 bytes**, `maxImageFPS` **30**, flags 0x1b = deferrable 1, rgb565 1, rgb888 0, jpeg 1,
  calibrated 1, origin 0. The ninth captured byte is beyond what this helper version parses. The
  inventory's "31 262 bytes" was a misread. Observation: VLP root `availableMemory` 124 960 =
  `maxImageSize` + 32 (medium that the two are the same receive buffer).
- **fn1 `getDisplayInfo` response layout** (DC of the decode lambda at `0x1003bd944`, high):
  byte 0 `displayShape` (u8), 1..2 `dimension` (u16 BE), 3..4 `resHorizontal`, 5..6 `resVertical`
  (u16 BE), byte 7 **button count**, byte 8 **visible-area count**, then 9-byte records
  `shape (u8), x, y, w, h (u16 BE each)`, buttons first, visible areas after them. CAP: shape 1,
  dimension 0x60 = 96, 480x480, 9 buttons, 0 visible areas. The owner's unit reports the buttons at
  (24,6) (181,7) (340,6) (23,164) (181,164) (340,164) (23,321) (181,322) (339,321), all 118x118
  (CAP, `02-probe-device.log`).
- fn7 `getConfig` and fn6 `setConfig`: a single `deviceAdopted` byte (FS of the decode lambdas,
  high). The helper sets it to 1 at start when it reads 0 (DC of the `Feature19A1` constructor).

## 1. Pacing

- `maxImageFPS` = 30 on c354 (CAP, see above); there is no constant or default for it in the
  helper, the value comes from fn0 at start and is exported to .NET as `maxImageFps` (DC of
  `LoupedeckConnection::getDeviceInfoModel`, FS, high).
- `[f19A1] Sleep after image sending for %d ms` is **not** derived from `maxImageFPS`. The value is
  read at start from an integer file named `image_sleep_in_ms` in the helper's data directory,
  default **0**; after each image command the send thread sleeps that long only when it is greater
  than 0 (`[f19A1] Sleep %d ms`) (DC of the constructor and `sendImages`, FS, high). The log strings
  `enableimagelogging`, `enableimagefolders`, `images19A1` are the same kind of file-based debug
  switches (image dumps to `image_%03dx%03d_%03dx%03d/image_%07d.jpg`).
- The send thread takes **one** queue item at a time, builds a vector of exactly one `Image`,
  checks its decoded length against `maxImageSize` (`[f19A1] Image is too big: %d or more, limit
  %d`), sends the command and waits for the fn2 response before the next item (DC of
  `sendImageThread` and `sendImages`, high). Images are therefore serialised by the response, not
  by a timer.
- Queue: `ConcurrentQueue<Feature19a1ImageQueueItem>`; before every push the queue is trimmed to
  **2** entries (`limitTo(2)`, DC plus disassembly of `addImage`, high) and the drop is logged as
  `[f19A1] %d images removed from queue`. Stale frames are dropped, not queued.
- `Feature19a1ImageQueueItem` layout (DC of the queue pop and the `Image` fill, high): `format`
  (u32), `x`, `y`, `w`, `h` (u32 each), then the image as a **base64 string** (the JSON payload is
  kept encoded until the send thread decodes it into a buffer of `maxImageSize` + 16 bytes).
  The devio `Image` struct is 32 bytes: format u32 at 0, x at 4, y at 8, w at 12, h at 16, length
  at 20, data pointer at 24.
- .NET side: `DeviceState70DrawEngine` owns a `System.Timers.Timer` with interval
  **1000 / MaxImageFps** ms (33.3 ms at 30 fps; the fallback FPS when the device reports none is an
  obfuscated constant) (DCS, high for the formula). The image sender `DoDrawImages` runs on a
  background task that wakes on **either** a new encoded image or the timer (wait-any semantics of
  `BackgroundTaskBase`, DCS, medium), drains every pending image with one `SetDisplayImage` message
  each and restarts the timer. The timer is a periodic retry, not a frame gate: images leave as
  soon as they are encoded.

## 2. Multi-image command

- The protocol allows several images per fn2 command: `count` is a u8, so structurally up to 255
  images, each with its own 12-byte header (`format, x, y, w, h, length`) and data; the command size
  is 3 + sum(12 + length) (DC of the request builder, high).
- Options+ never uses it: `sendImages` always builds a one-element vector and `setImage` is called
  with that vector (DC, high). CAP: every setImage command in the hcooper capture has `count` 1 and
  one 435x434 image; no multi-image frame exists in the capture.
- Whether a multi-image set is applied atomically by the device is therefore not observable in the
  host software (unknown). The host-side size check is per image against `maxImageSize`; the
  device-side bound is presumably the same receive buffer (`availableMemory` = `maxImageSize` + 32,
  medium).
- VLP transport facts (DC of `devio::DeviceRequest::set_device_interface` and
  `next_command_packet`, medium): report 0x13 is request type 2, report 0x14 type 3; the payload
  per packet is the report payload size minus 4, the packet count is ceil((command size - 4) /
  (payload - 4)) with a minimum of 1; the sequence byte is `0x80 | (index & 0xF)`, with `0x40` set
  on the last packet (`e0` = first and last of a one-packet message, `a0 ... 61` as captured). The
  per-PID `VlpWindow` config item 0x12 is **4** packets in flight when enabled (default) and **1**
  when a debug file `disablevlpwindow4` exists (`[DC] Disable VlpWindow4`) (DC of
  `devio::Device::Device` and `DeviceCollection::onStart`, high).

## 3. Unit of an update

- The local JSON connection (`[LC]`) method **`SetDisplayImage`** carries exactly **one image**:
  parameters `{deviceId}`, data object `{X, Y, Width, Height, ImageFormat, Image}` where
  `ImageFormat` is a string (`"Jpeg"` or `"Rgb565"`, anything else fails with `Unknown image format
  '%s'`) and `Image` is the base64 of the encoded bytes (DCS of
  `ExtendedDevicesMessageHandler.SetDisplayImage` and its nested request model; DC of
  `LoupedeckConnection::onRequest` and `SetDisplayImageModel` `from_json` with keys `width`,
  `height`, `imageFormat`; FS `[LC] SetDisplayImage '%s' %d`; high). No list, no area ids:
  rectangles only. `"Jpeg"` maps to `Feature19A1::addJpegImage`, `"Rgb565"` to `addRgb565Image`.
- The device model sent to .NET on connect (`DeviceConnected` event, JSON keys from FS and DCS of
  `Loupedeck7DeviceInfo`, high): `deviceId`, `pid`, `displayName`, `serialNumber`,
  `firmwareVersion`, `firmwareInfo[]`, `maxImageFps` (uint), `maxImageSize` (uint), `displayArea`
  (`{location {x, y}, width, height}`), `buttonsAreas[]` (same `AreaModel`), `keepAliveTimeoutMin`,
  `keepAliveTimeoutMax`, `deviceAdopted`, `imageWidth`, `imageHeight`.
- On a profile, page or mode change the .NET `DeviceState70` calls `RequestPageRedraw` (reasons:
  profile state changed, plugin language changed, redraw device, keep-alive timeout, state changed
  to Streaming), which marks **all nine** buttons dirty and signals `DoDrawControls` (DCS, high).
  `DoDrawControls` draws every dirty button that has a device-reported area into a list of
  (area, bytes) pairs and hands the whole list to the canvas object in **one** `Draw(list)` call
  (DCS, high).
- The canvas object (obfuscated class `bq`, DCS, high): a 480x480 `BitmapBuilder` cleared to black
  at construction, image format from settings (default inferred Jpeg), size limit = `maxImageSize`.
  For one `Draw(list)` it draws each button image at the button's `Location.X, Location.Y` (top-left
  aligned, no rotation), tracks the bounding box of the drawn areas, crops the canvas to that
  bounding box and encodes it once. The result is queued as one (area, bytes) item for
  `DoDrawImages`, which sends it as one `SetDisplayImage(x, y, w, h, format, bytes)`.
- Therefore a page change is **one display-area image covering the union of the nine button
  areas**, not nine button images and not the full 480x480 panel. CAP: 435x434 at (23,6) is exactly
  the union of the owner's button rectangles (x 23..457, y 6..439).
- Splitting rule (DCS of the canvas `ProcessQueue`, high for the structure, the factor is an
  obfuscated double): n = ceil(encoded length x factor / `maxImageSize`); if n is 1 the single crop
  is queued, otherwise the bounding box is cut into n horizontal bands of equal height (full
  bounding-box width), each encoded and queued separately, top to bottom.
- `Loupedeck70Device.SetDisplayImage(format, bytes)` without a rectangle uses (0, 0, W, W) with an
  obfuscated constant that the 480x480 canvas implies is 480 (DCS, medium); `SetButtonImage(format,
  buttonId, bytes)` uses the device-reported button rectangle (DCS, high). Neither is on the normal
  redraw path.

## 4. Single-button change

- `RequestButtonRedraw(button, pressedBits, reason, app)` sets **one** dirty bit (DCS, high). The
  same `DoDrawControls` then produces a one-item list, the canvas bounding box is that button's
  device-reported area, so the device receives **one image of the button area, 118x118 at the
  reported origin**, not the whole display and not an enlarged rectangle (DCS, high).
- Content of that 118x118 image: the button's action image is built at `PluginImageSize.Width116`
  = **116x116** (DCS; the empty-button resources are 116x116, RES) and drawn at the area origin, so
  the right-most and bottom-most 2 pixel columns and rows of the 118x118 area come from the black
  canvas (DCS, high). This is why Options+ shows no coloured strip beside a key: its canvas
  background is black and every key write covers the full reported rectangle.
- Pressed feedback (DCS of `TryDrawActionImage`, high for the structure): the action image is
  resized by a fixed ratio (two obfuscated integer constants, numerator/denominator) and drawn
  centred on a black 116x116 builder; released buttons draw the image at (0, 0) unscaled. Animated
  images follow the same rule per frame (`OnRedrawAnimatedImages`), again as one `Draw(list)` of
  all animated controls, so several animated buttons share one bounding-box image per frame.
- A button with no action gets the embedded `EmptyButtonImage` resource (RES: PNG, JPEG and RGB565
  variants, 116x116; JPEG 380 bytes, quality-100 tables, 4:2:0; RGB565 26 912 bytes) instead of a
  rendered image (DCS, high).

## 5. Formats

- Image format selection in .NET: `BitmapImageFormat` enum `Jpeg = 3, Png = 4, Wbmp = 5,
  SpecialFormats = 1000, Rgb565 = 1001, Rgb565Swapped = 1002, Rgb888 = 1003` (DCS, high). The
  Keypad path reads the format from a settings key with a default that is the same table index as
  `PluginImageSize.Width116` (= 3), hence **Jpeg** (inferred, medium); the capture confirms JPEG on
  the wire. `Rgb565` is supported end to end (`addRgb565Image`, `ConvertToRgb565` with optional byte
  swap) but is not the default for c354. The devio `ImageFormat` value for JPEG on the wire is
  **0** (CAP); the wire values of the RGB formats were not captured.
- `ClearScreen` (`[LC] ClearScreen`) sends `addJpegImage(0, 0, 480, 480, s_emptyDisplayJpegImage)`
  (DC of `Feature19A1::clearScreen`, high). `s_emptyDisplayJpegImage` is a base64 string literal in
  the helper: a 4 227-byte **480x480** baseline JPEG, JFIF 96 dpi, 4:2:0 subsampling, quantisation
  tables identical to libjpeg quality **94**, all-black (data file analysis of the decoded literal,
  high). In the .NET service `ClearScreen` has only its API wrapper and **no caller** on the redraw
  path (DCS grep, high): a normal session never clears the screen.
- Runtime JPEG encoder settings (CAP of hcooper frames 521, 639, 647; .NET encodes with SkiaSharp
  `SKBitmap.Encode(SKEncodedImageFormat, quality)` whose quality argument is an obfuscated
  constant, DCS): baseline SOF0, 3 components, **4:2:0** (Y 2x2, Cb and Cr 1x1), both quantisation
  tables all ones = **quality 100**, JFIF header, no progressive, no restart markers observed.
  Encoded sizes seen: 1 460 and 4 363 bytes for flat 435x434 content. The size cap is the device's
  `maxImageSize` (124 928 bytes) applied by the canvas band splitting (section 3) and again per
  image by the helper (section 1).
- Resize filter quality for scaled images is an obfuscated `SKFilterQuality` constant (unknown).

## 6. First paint after connect

Sequence (DC of the `Feature19A1` constructor, DCS of `Loupedeck70Device.OnOpen`,
`DeviceState70.OnStart` and `OnDeviceStateChanged`, FS; high unless noted):

1. Helper, on feature init: fn7 `getConfig` (deviceAdopted), fn6 `setConfig(1)` when it read 0,
   fn3 `getSupportedDeviceStates`, fn5 `getDeviceState`, fn0 `getCapabilities`,
   fn1 `getDisplayInfo(1)`; the image thread starts; the device model is sent to .NET.
2. .NET `OnOpen`: logs fps, size, display area, button areas, keep-alive range; starts keep-alive
   (settings default on; the fixed pair is obfuscated, the capture shows 3000 ms requested and a
   1 s period; the fallback is `keepAliveTimeoutMax` with a period of half of it); reads the
   device state.
3. `DeviceState70.OnStart`: the draw engine starts **paused**; if the device is already Streaming
   it resumes; if the device is in Dfu it sends `ExitDfuMode`; if the device is not Streaming it
   sends `SetDeviceState(Streaming)` (0xa0).
4. The device answers with the state-change event; `OnDeviceStateChanged` sees Streaming, calls
   `RequestPageRedraw` (all nine buttons, one bounding-box image, section 3) and resumes the draw
   engine; `Loupedeck70Device` restores brightness on the same event.
5. **No `ClearScreen` precedes the first image**, neither after connect nor after a keep-alive
   timeout; a keep-alive timeout event only triggers `RequestPageRedraw` (DCS, high).

.NET state enum `Loupedeck70DeviceState` (DCS, high; corrects the inventory's mapping):
`Error = -1, None = 0, Streaming = 160 (0xa0), Standby = 161 (0xa1), StandbyBright = 162 (0xa2),
Splash = 163 (0xa3), SplashBright = 164 (0xa4), OnboardingBright = 165 (0xa5), Dfu = 166 (0xa6),
Idle = 210 (0xd2), LowPower = 211 (0xd3)`. The device's supported list `a0 a1 a2 a4 a5 a6` therefore
omits `Splash` (0xa3) and includes `Dfu` (0xa6); `d0` Unknown, `d1` USB Preconfiguration, `d2` Idle,
`d3` Low Power, `d4..d6` TDE states keep the inventory's order. The helper releases every held
button when the state becomes Dfu (DC of `DeviceC354::onDeviceStateChanged`, high).

## 7. Image build pipeline, numbers only

- Canvas per display: **480x480**, black, one per device, kept between updates (DCS, high; the
  `DeviceState70DrawEngine` constants are 480 and 116, `ButtonCount = 9`).
- Canvas per button: **116x116** (`PluginImageSize.Width116`, `Width116Pixels = 116`), background
  black, drawn into the display canvas at the device-reported button origin, top-left aligned; the
  118x118 reported area thus has a 2 px black margin on the right and bottom and no inset on the
  left and top (DCS, high).
- Composition: no gaps are drawn; the background between keys is the black canvas, the bounding-box
  crop carries it to the device. 435x434 at (23,6) is the union of the nine reported rectangles
  (section 0), which is why it is neither 480x480 nor 3x118 + gaps (CAP + DCS, high).
- Fitting of a user picture into the 116x116 button image: done by `ActionImageBuilder`
  (`LoupedeckShared`), which is icon and label drawing and was **skipped** (boundary). Flags that
  exist for it: `HasBorder, UserImage, CropBorders, FillBackground, NoBackground, ForceImageOnly,
  UseOverlayBoundingBoxForVectorImages, ReplaceBackgroundColorWithBlack` (`ActionImageBuilderFlags`
  enum, DCS, high; semantics not analysed). The only fitting rules read on the device path are the
  pressed-state shrink-and-centre (section 4) and a generic border routine that shrinks an image by
  an obfuscated inset and draws it offset by another (DCS, medium, values unknown).
- Per-device limits: `maxImageSize` 124 928 bytes per image (fn0), `maxImageFPS` 30, 24-bit length
  field, VLP window 4 packets, 4095-byte report 0x14 (CAP + DC, high).

## 8. Idle logo

- While the device is not Streaming, the draw engine is **paused** (`ai.Pause()` on any state
  change away from Streaming) and `DoDrawImages` is skipped, so the host sends no images during the
  Standby or Splash animation (DCS of `DeviceState70.OnDeviceStateChanged` and `DoDrawImages`,
  high).
- The transition that stops the logo is the host's `SetDeviceState(Streaming)`: when the state
  event reports Standby or StandbyBright and the service has not itself requested sleep (a flag in
  `DeviceState70`), the service immediately sends `SetDeviceState(Streaming)` again, then on the
  resulting Streaming event redraws the whole page (DCS, high). CAP: Options+ sets `a0` before the
  first paint; keyvo's `ClaimWatch` re-sends `setMode(a0)` on the `a1` event, the same behaviour.
- The idle dim states `Idle` (0xd2) and `Low Power` (0xd3) are reported by the device and only
  logged; no host command reacts to them (FS, DCS, medium).

## What changes the plan

1. **#72 strip beside the keys.** Options+ never sees it because (a) its page paint is one image
   covering the union of the key rectangles and (b) its per-key image is 116x116 on a black
   118x118 area taken from the same black canvas, so key and background always agree. For keyvo:
   render the nine keys and the panel background into one 480x480 canvas, send the **union
   rectangle** (435x434 at 23,6 on this unit, computed from fn1, never hard-coded) as one JPEG on
   profile, page and mode changes, and for a single-key change send the key's reported 118x118
   rectangle cropped from the same canvas. The strip then shows the profile background exactly as
   the panel paint does, which makes the per-key margin question moot.
2. **`keyvo-hid` image writer.** One `setImage` command per update is the model to copy. The
   multi-image `count` field and the `deferDisplayUpdate` bit (byte 1 bit 0, after the
   `displayIndex` byte) exist but Options+ uses neither, so their device behaviour is untested
   by the reference implementation; keep them as hardware experiments, not as the design. Fix the
   header: byte 0 is `displayIndex` (send 1), byte 1 is the defer flag (send 0), byte 2 is `count`.
   Enforce `maxImageSize` per image (124 928 bytes here, read from fn0 in KiB units) and split the
   canvas into horizontal bands when an encoded crop exceeds it, as Options+ does.
3. **Daemon paint sequence.** Replace background-then-nine-keys with one composite image per
   change; coalesce dirty keys into one bounding box; keep at most a couple of pending images and
   drop stale ones (the helper keeps 2); send the next image after the fn2 response, no timer
   needed. Pressed feedback is a shrunken icon centred on the key, drawn as a one-key update.
4. **Probe fn0 fields.** Print `deviceScreenCount` (u8), `maxImageSize` (u16 BE x 1024 bytes),
   `maxImageFPS` (u8), and the flag byte 7 as `deferrableDisplayUpdate`, `rgb565`, `rgb888`,
   `jpeg`, `calibrated` (bits 0..4) and `origin` (bits 5..6); print fn1 with byte 7 = button count
   and byte 8 = visible-area count. Correct the recorded `maxImageSize` from 31 262 to 124 928.
5. **Encoder.** Baseline JPEG, 4:2:0, quality 100 is what the device gets from Options+; keyvo can
   use the same, with a quality or band fallback when a photo-like 435x434 frame exceeds 124 928
   bytes.
6. **Spec state table.** `a3` = Splash (unsupported by this device), `a4` = SplashBright,
   `a5` = OnboardingBright, `a6` = Dfu; `d2` = Idle, `d3` = Low Power. `ClearScreen` is a 480x480
   black JPEG at (0,0), not a separate command; keyvo does not need it.

## Legal-boundary log

- Purpose and scope: interoperability of keyvo with the Keypad (and Dialpad) only. Analysed: the
  0x19A1 request builders and response-decoding lambdas, the `Feature19A1` wrapper (constructor,
  `addImage`, `addJpegImage`, `clearScreen`, `sendImages`, `sendImageThread`, `getDeviceInfoModel`
  through `LoupedeckConnection`, `onRequest` only at the `SetDisplayImage`/`ClearScreen` string
  hits), `DeviceC354::onDeviceStateChanged`, `DeviceCollection::onStart`, the `VlpWindow` and VLP
  packetisation lines of `devio::Device::Device`, `DeviceRequest::set_device_interface` and
  `next_command_packet`, the JSON model serialisers. The headless batch decompiled 331 functions
  into `analysis/ghidra/headless/out1/` because the name patterns matched template instantiations;
  only the functions named here were read. .NET types decompiled by ILSpy (whole assemblies to
  disk) and read: `Loupedeck70Device`, `Loupedeck7DeviceInfo`, `DeviceState70`,
  `DeviceState70DrawEngine`, `ExtendedDevicesMessageHandler`, the canvas classes `bq` and `br`,
  `CanvasArea`, `BackgroundTask`/`BackgroundTaskBase` (signature level), `BitmapBuilder` and
  `BitmapImageExtensions` (signature level and the encode/resize call sites), the enums
  `BitmapImageFormat`, `PluginImageSize`, `Loupedeck70DeviceState`, `CanvasShape`,
  `ActionImageBuilderFlags`, and the obfuscator loader classes `bz`, `by`, `ch` only far enough to
  establish that the constant table is encrypted.
- Skipped on purpose: the editor UI, plugins, accounts, telemetry, cloud, updater, crash reporting,
  and the drawing code of icons and labels (`ActionImageBuilder`, `TryCreateActionImage`,
  `ConfigurationWindowMessageHandler`). The obfuscator's AES-encrypted constant resource was **not**
  decrypted; constants reachable only through it are reported as unknown or as inferences and are
  marked so.
- Output discipline: this report carries names, numbers, layouts, enumerations, state names and
  evidence kinds only. Decompiler output stays under `analysis/ghidra/headless/` and
  `analysis/ilspy/`; no code, pseudocode, function body or copied comment entered the report or the
  repository.
- Hygiene: no Options+ or Plugin Service binary was executed; network use inside the distrobox was
  limited to `dnf install dotnet-sdk-9.0` and `dotnet tool install ilspycmd`; nothing was written
  to the repository except the copy of this report under `docs/research/`. A symlink
  `~/keyvo-re-ghidra-headless` to the spike's headless directory exists because Ghidra refuses
  project paths with a dot-prefixed element.
- Third-party components noticed: SkiaSharp (JPEG encoder), nlohmann::json 3.11.2, HidSharp.
  No code was copied.

## Reference paths

- Brief: `BRIEF-display-pipeline.md`; previous report: `report/options-plus-inventory.md`.
- Native helper: `analysis/ghidra/LogiPluginServiceExt_arm64`; headless project and scripts
  `analysis/ghidra/headless/` (`proj.gpr`, `scripts/DumpFacts.java`, `scripts/DumpAsm.java`,
  `targets.txt`, `out1/index.tsv`, `out1/strings.tsv`, `out1/functions.tsv`, `out1/*.c`,
  `asm1.txt`, `run2.log`, `run3.log`).
- Strings and symbols: `analysis/lps/ext_strings.txt`, `ext_demangled.txt`.
- .NET: `analysis/ilspy/{PluginApi,LoupedeckShared,LoupedeckService}/`, install logs
  `analysis/ilspy/dnf-install.log`, `ilspy-install.log`, `decompile.log`; embedded resources
  `analysis/ilspy/LoupedeckService/Loupedeck.Service.Loupedeck7Devices.Resources.EmptyButtonImage.*`.
- Repository cross-checks: `captures/external/hcooper/derived/binding-paint-frames.tsv`,
  `docs/research/hcooper-captures.md`, `docs/hardware-notes.md`; first-light logs
  `~/keyvo-firstlight-2026-10-25/`.
