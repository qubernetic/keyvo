# sysfs fixtures

Trees shaped like `/sys/class/hidraw` for the node search in
`crates/keyvo-hid/src/sysfs.rs`: `class/hidraw/<node>/device/uevent` as the
kernel writes it, and the report descriptor as `report_descriptor.hex`
(space-separated hex, `#` comments). The tests copy a tree into a temporary
directory and turn each `.hex` file into the binary `report_descriptor`
sysfs has.

| Scenario | Nodes | Expected |
|---|---|---|
| `keypad-and-receiver` | `hidraw2` Keypad (reports 04 05 11 13 14), `hidraw9` Bolt receiver (10 11) | Keypad `hidraw2`, receiver `hidraw9` |
| `keypad-split` | `hidraw4` Keypad (04 05), `hidraw5` Keypad (11 13 14) | Keypad `hidraw5`, not the first match; no receiver |
| `keypad-without-bulk` | `hidraw2` Keypad (11 13) | refused: no node carries `0x14` |
| `receiver-only` | Bolt receiver: `hidraw1` keyboard (no report ids), `hidraw2` mouse (02 03 04 0b 3c 3d), `hidraw3` HID++ (10 11), `hidraw4` digitizer (28 29 2a) | Keypad not found, receiver `hidraw3` |

`keyvo_hid::sysfs::find_receiver` takes the `046d:c548` node that carries
both HID++ reports, `0x10` and `0x11`; the unit tests build the
receiver's other interface nodes and a Bluetooth Dialpad (`0005:046d:bc00`)
in a temporary directory.

The Keypad descriptor of `keypad-and-receiver` is the real one: first
light on 2026-10-25, firmware `U1 66.00.B0017`, `xxd
/sys/class/hidraw/hidraw6/device/report_descriptor` on the owner's Keypad
(`00-item1-report-descriptor.hex` of the first-light logs on
qubernetic/keyvo#40), 140 bytes, five top-level collections. Linux creates
one USB interface and one hidraw node for `046d:c354`, so this is the
topology keyvo meets. The node keeps the name `hidraw2` (the node number
changes between boots and plugs; first light had `hidraw6`); its `uevent`
carries the `HID_PHYS` path of the first-light report on #40 and is
otherwise in the kernel's format.

`keypad-split` and `keypad-without-bulk` are hypothetical topologies the
node search must still handle; their collections are cut from the captured
descriptor.

The Bolt receiver's descriptors are the real ones: the owner's hardware
check of slice 7 (#187, part 1, 2026-10-29), `xxd
/sys/class/hidraw/<node>/device/report_descriptor` on the receiver's four
interfaces, `bInterfaceNumber` 00 to 03 on `hidraw1` to `hidraw4`
(`docs/hardware-notes.md`, "Dialpad over Bolt"). `receiver-only` carries
all four under their measured node names; `keypad-and-receiver` carries the
HID++ interface (02) alone, as `hidraw9`. Their `uevent` files are in the
kernel's format with the `HID_ID` and `HID_NAME` the check printed and the
interface number in `HID_PHYS`; the check recorded neither the USB path
nor the driver, so `HID_PHYS` keeps the earlier path and `DRIVER` is left
out.
