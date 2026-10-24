# sysfs fixtures

Trees shaped like `/sys/class/hidraw` for the node search in
`crates/keyvo-hid/src/sysfs.rs`: `class/hidraw/<node>/device/uevent` as the
kernel writes it, and the report descriptor as `report_descriptor.hex`
(space-separated hex, `#` comments). The tests copy a tree into a temporary
directory and turn each `.hex` file into the binary `report_descriptor`
sysfs has.

| Scenario | Nodes | Expected |
|---|---|---|
| `keypad-and-receiver` | `hidraw2` Keypad (reports 04 05 11 13 14), `hidraw9` Bolt receiver (10 11) | `hidraw2` |
| `keypad-split` | `hidraw4` Keypad (04 05), `hidraw5` Keypad (11 13 14) | `hidraw5`, not the first match |
| `keypad-without-bulk` | `hidraw2` Keypad (11 13) | refused: no node carries `0x14` |
| `receiver-only` | `hidraw9` Bolt receiver | not found |

The descriptors are synthetic. The Keypad's follows the five top-level
collections shensquared decoded on macOS
(`docs/research/logilinux-and-julusian-analysis.md` §7.1); the real Linux
topology and descriptor bytes come from the hardware check (plan §5 item 1)
and replace these files then.
