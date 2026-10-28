# `keyvo doctor` fixture roots

Each directory is a fake root for `keyvo doctor --root <dir>`
(`tests/doctor.rs`), copied into a temporary directory first:

| Root | What differs from `all-good` | Expected |
|---|---|---|
| `all-good` | nothing: the Keypad's node, `/dev/uinput`, the shipped rule, focus stealing prevention Low, the unit active, the daemon answering, no Solaar, the Flatpak app installed | nine `ok`, exit 0 |
| `no-rule` | no `/etc/udev/rules.d/70-keyvo.rules`; the nodes are accessible anyway (the hand-made pre-alpha setup) | `rule` warns, exit 0 |
| `no-uinput` | no `/dev/uinput` | `uinput` fails, exit 1 |
| `level-2` | `kwinrc` `[Windows]` `FocusStealingPreventionLevel=2` | `focus` warns, exit 0 |
| `solaar-running` | `flatpak ps` lists `io.github.pwr_solaar.solaar` | `solaar` warns, exit 0 |

What the test adds to every copy:

- `sys/class/hidraw/`: the `keypad-and-receiver` sysfs fixture of
  `keyvo-hid` (`crates/keyvo-hid/tests/fixtures/sysfs/`), each
  `report_descriptor.hex` turned into the binary file sysfs has. The empty
  files `dev/hidraw2` (the Keypad) and `dev/hidraw9` (the receiver) stand
  for the nodes.
- `etc/udev/rules.d/70-keyvo.rules.shipped`, an empty placeholder, becomes
  `70-keyvo.rules` with the contents of `contrib/udev/70-keyvo.rules`, so
  the fixtures do not carry a second copy of the rule.
- `bin/systemctl` and `bin/flatpak`, links to `fake-command`, first on
  `PATH`. The fake prints `commands/<name>_<arguments joined by _>.out`
  and exits with the number in the `.status` file next to it (0 without
  one), or exits 1 when there is no `.out` file: `flatpak info` of an app
  that is not installed, `systemctl --user is-enabled` of a unit that is
  not installed.
- `run/keyvo/keyvo.sock` (mode `0600` in a `0700` directory): a fake daemon
  that answers `status` with the first line of
  `tests/protocol-fixtures/socket/responses.ndjson`.
- The environment: only `PATH`, `HOME` (`home/`), `XDG_RUNTIME_DIR`
  (`run/`), `XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=KDE` and
  `FAKE_COMMANDS` (`commands/`).
