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
| `receiver-only` | `hidraw9` Bolt receiver | Keypad not found, receiver `hidraw9` |

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
descriptor. The Bolt receiver's descriptor (`hidraw9`) is synthetic: a
short `0x10` and a long `0x11` vendor report; only its VID/PID matters.
