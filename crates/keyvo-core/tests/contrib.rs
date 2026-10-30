//! The example profiles under `contrib/profiles/`, which
//! `docs/pre-alpha-test.md` installs into `~/.config/keyvo/profiles/`, and
//! the recipe packs under `contrib/recipes/`, which keyvo bundles, and the
//! Flatpak under `contrib/flatpak/`, the systemd user unit under
//! `contrib/systemd/` and the udev rule under `contrib/udev/`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use keyvo_core::config::Focus;
use keyvo_core::focus::WindowContext;
use keyvo_core::layout::{Loaded, load};
use keyvo_core::profile::{Action, Key, KeySlot, ModifierMode, Profile};
use keyvo_core::recipe::{self, Group, Recipe, Source};
use keyvo_core::render::icon;

/// `contrib/`, laid out like a configuration directory: `profiles/*.toml`
/// and no `config.toml`, so the defaults apply as on a fresh install.
fn contrib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contrib")
}

fn loaded() -> Loaded {
    load(&contrib()).unwrap_or_else(|errors| panic!("{errors}"))
}

fn keys(profile: &Profile) -> impl Iterator<Item = &Key> {
    profile.pages.iter().flat_map(|page| page.keys.values())
}

fn all_keys(loaded: &Loaded) -> Vec<&Key> {
    loaded.profiles.iter().flat_map(keys).collect()
}

fn exec(cmd: &str) -> Action {
    Action::Exec { cmd: cmd.into() }
}

#[test]
fn every_example_profile_loads_and_each_file_is_its_own_profile() {
    let loaded = loaded();
    let ids: Vec<_> = loaded
        .profiles
        .iter()
        .map(|p| p.profile.id.as_str())
        .collect();
    assert_eq!(ids, ["global", "konsole", "vscode"]);
    let files = std::fs::read_dir(contrib().join("profiles"))
        .unwrap()
        .filter(|entry| {
            let path = entry.as_ref().unwrap().path();
            path.extension().is_some_and(|ext| ext == "toml")
        })
        .count();
    assert_eq!(files, ids.len(), "no file shadowed by the built-in global");
}

#[test]
fn the_app_profiles_match_their_window_classes_and_nothing_else() {
    let loaded = loaded();
    let shown = |class: &str| {
        loaded
            .profiles
            .select(&WindowContext::new(class, "x"))
            .profile
            .id
            .clone()
    };
    assert_eq!(shown("org.kde.konsole"), "konsole");
    assert_eq!(shown("konsole"), "konsole");
    assert_eq!(shown("code"), "vscode");
    assert_eq!(shown("com.visualstudio.code"), "vscode");
    assert_eq!(shown("org.kde.dolphin"), "global");
}

#[test]
fn every_profile_has_its_own_accent() {
    let loaded = loaded();
    let accents: Vec<_> = loaded.profiles.iter().map(|p| p.profile.accent).collect();
    assert!(accents.iter().all(Option::is_some), "{accents:?}");
    let distinct: BTreeSet<_> = accents.iter().map(|a| format!("{a:?}")).collect();
    assert_eq!(distinct.len(), accents.len(), "{accents:?}");
}

#[test]
fn the_keys_cover_a_chord_and_the_running_success_and_error_looks() {
    let loaded = loaded();
    let actions: Vec<_> = all_keys(&loaded)
        .iter()
        .filter_map(|k| k.action.clone())
        .collect();
    assert!(actions.iter().any(|a| matches!(a, Action::Chord { .. })));
    // Success, error, and a run past `[render].long_run_threshold` (10 s)
    // that still ends before `[timing].exec_timeout` (30 s).
    for cmd in ["true", "exit 1", "sleep 15"] {
        assert!(actions.contains(&exec(cmd)), "no exec key `{cmd}`");
    }
}

#[test]
fn the_error_key_notifies_from_its_on_fail() {
    let loaded = loaded();
    let fail = loaded.profiles.global().pages[0]
        .keys
        .values()
        .find(|key| key.action == Some(exec("exit 1")))
        .expect("the error key");
    assert_eq!(
        fail.on_fail,
        Some(exec("notify-send 'keyvo' 'key 2 failed'"))
    );
}

#[test]
fn the_global_profile_has_a_context_a_general_and_a_third_page() {
    let loaded = loaded();
    let pages: Vec<_> = loaded
        .profiles
        .global()
        .pages
        .iter()
        .map(|p| p.name.as_deref())
        .collect();
    assert_eq!(pages, [Some("context"), Some("general"), Some("third")]);
}

/// The recipe packs under `contrib/recipes/`, which the daemon serves as
/// the bundled ones.
fn packs() -> Vec<Recipe> {
    let catalogue = recipe::load_dir(&contrib().join("recipes"), Source::Bundled);
    assert!(catalogue.problems.is_empty(), "{:?}", catalogue.problems);
    catalogue.recipes
}

#[test]
fn every_recipe_pack_validates_and_is_the_bundled_one() {
    let packs = packs();
    let bundled = recipe::bundled();
    assert!(bundled.problems.is_empty(), "{:?}", bundled.problems);
    assert_eq!(bundled.recipes, packs, "recipe::BUNDLED lists every pack");
}

#[test]
fn the_v1_catalogue_has_its_entries_in_their_groups() {
    let names = |group: Group| -> Vec<String> {
        packs()
            .into_iter()
            .filter(|r| r.group == group)
            .map(|r| r.name)
            .collect()
    };
    assert_eq!(
        names(Group::General),
        ["New email", "Open folder", "Open website"]
    );
    assert_eq!(
        names(Group::System),
        [
            "Brightness down",
            "Brightness up",
            "Lock the desktop",
            "Mute",
            "Open a settings page",
            "Volume down",
            "Volume up"
        ]
    );
    assert_eq!(
        names(Group::Apps),
        ["Calculator", "Konsole", "Spectacle", "VS Code"]
    );
}

#[test]
fn the_app_recipes_offer_what_they_do() {
    let variants = |id: &str| -> Vec<String> {
        let pack = packs().into_iter().find(|r| r.id == id).unwrap();
        pack.variants.into_iter().map(|v| v.id).collect()
    };
    assert_eq!(variants("spectacle"), ["fullscreen", "region", "recording"]);
    assert_eq!(variants("vscode"), ["palette", "build", "run"]);
    assert_eq!(variants("konsole"), ["new-tab", "find"]);
}

#[test]
fn every_recipe_has_a_bundled_icon_and_a_label() {
    for pack in packs() {
        assert!(
            icon::lookup(&pack.icon).is_ok(),
            "{}: {}",
            pack.id,
            pack.icon
        );
        assert!(!pack.label.trim().is_empty(), "{}", pack.id);
    }
}

/// A variant that opens an application or a settings page, or records
/// until it is stopped, stays open past `[timing].exec_timeout`, so it is a
/// `launch` (#155); a command that exits soon stays an `exec`.
#[test]
fn the_variants_that_stay_open_are_launches() {
    let launches: BTreeSet<String> = packs()
        .into_iter()
        .flat_map(|pack| {
            pack.variants
                .into_iter()
                .filter(|v| matches!(v.action, Action::Launch { .. }))
                .map(move |v| format!("{}/{}", pack.id, v.id))
        })
        .collect();
    let expected: BTreeSet<String> = [
        "calculator/flatpak",
        "calculator/kcalc",
        "settings-page/bluetooth",
        "settings-page/display",
        "settings-page/keyboard",
        "settings-page/network",
        "settings-page/power",
        "settings-page/sound",
        "spectacle/recording",
    ]
    .map(str::to_owned)
    .into();
    assert_eq!(launches, expected);
}

#[test]
fn the_third_global_page_has_a_launch_key() {
    let loaded = loaded();
    let third = &loaded.profiles.global().pages[2];
    assert!(
        third.keys.values().any(|key| key.action
            == Some(Action::Launch {
                cmd: "sleep 60".into(),
                // No window comes: the launch claims none.
                class: Some("sleep".into()),
                wait_focus: true,
            })),
        "{third:?}"
    );
}

/// The user test of #119 presses "Volume up" and expects the success look,
/// which only an `exec` shows.
#[test]
fn the_default_volume_variant_shows_feedback() {
    let volume = packs().into_iter().find(|r| r.id == "volume-up").unwrap();
    assert!(volume.variants[0].action.has_feedback());
}

#[test]
fn every_icon_is_bundled_and_every_key_has_a_label() {
    for key in all_keys(&loaded()) {
        if let Some(reference) = &key.icon {
            assert!(icon::lookup(reference).is_ok(), "{reference}");
        }
        assert!(key.label.is_some(), "{key:?}");
    }
}

// ---- contrib/flatpak/ ------------------------------------------------------

/// The application id of the Flatpak, the Tauri identifier and the name of
/// every metadata file (spec §3).
const APP_ID: &str = "io.github.qubernetic.keyvo";

/// `path` from the repository root.
fn repo_file(path: &str) -> String {
    let full = contrib().join("..").join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|error| panic!("{}: {error}", full.display()))
}

fn manifest() -> String {
    repo_file(&format!("contrib/flatpak/{APP_ID}.yml"))
}

/// The value of the manifest's top-level `key: value` line.
fn manifest_value<'a>(manifest: &'a str, key: &str) -> Option<&'a str> {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
        .map(str::trim)
}

/// The value of `key=` in the desktop file's `[Desktop Entry]` group.
fn desktop_value<'a>(desktop: &'a str, key: &str) -> Option<&'a str> {
    desktop
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
}

/// The text between the first `open` tag in `xml` and the `close` after it.
fn xml_text<'a>(xml: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = xml.find(open)? + open.len();
    let end = start + xml[start..].find(close)?;
    Some(&xml[start..end])
}

fn tauri_conf() -> serde_json::Value {
    serde_json::from_str(&repo_file("app/src-tauri/tauri.conf.json")).unwrap()
}

#[test]
fn one_application_id_across_manifest_desktop_metainfo_tauri_and_hold_classes() {
    assert_eq!(manifest_value(&manifest(), "id"), Some(APP_ID));
    assert_eq!(manifest_value(&manifest(), "command"), Some("keyvo-app"));

    let desktop = repo_file(&format!("contrib/flatpak/{APP_ID}.desktop"));
    assert_eq!(desktop_value(&desktop, "Icon"), Some(APP_ID));
    assert_eq!(desktop_value(&desktop, "StartupWMClass"), Some(APP_ID));

    let metainfo = repo_file(&format!("contrib/flatpak/{APP_ID}.metainfo.xml"));
    assert_eq!(xml_text(&metainfo, "<id>", "</id>"), Some(APP_ID));
    let desktop_id = format!("{APP_ID}.desktop");
    assert_eq!(
        xml_text(
            &metainfo,
            "<launchable type=\"desktop-id\">",
            "</launchable>"
        ),
        Some(desktop_id.as_str())
    );

    assert_eq!(tauri_conf()["identifier"], APP_ID);
    // Under Flatpak the window's class is the application id (spec §6.1).
    assert!(Focus::default().holds(APP_ID));
}

/// The items of the manifest's top-level list `key:`, comments left out.
fn manifest_list(manifest: &str, key: &str) -> Vec<String> {
    let header = format!("{key}:");
    manifest
        .lines()
        .skip_while(|line| *line != header)
        .skip(1)
        .take_while(|line| line.starts_with(' '))
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(str::to_owned)
        .collect()
}

/// The sandbox permissions are exactly the documented six (spec §10): the
/// display, the GPU, the daemon's runtime directory and the configuration
/// directory. No device, no `KWin`, no system bus, no `org.freedesktop.Flatpak`.
#[test]
fn the_flatpak_finish_args_are_exactly_the_documented_six() {
    assert_eq!(
        manifest_list(&manifest(), "finish-args"),
        [
            "--share=ipc",
            "--socket=wayland",
            "--socket=fallback-x11",
            "--device=dri",
            "--filesystem=xdg-run/keyvo",
            "--filesystem=xdg-config/keyvo:create",
        ]
    );
}

/// The binaries the manifest's build commands install into `/app/bin`.
fn installed_binaries(manifest: &str) -> BTreeSet<String> {
    manifest
        .split("${FLATPAK_DEST}/bin/")
        .skip(1)
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_desktop_file_and_the_command_start_a_binary_the_manifest_installs() {
    let manifest = manifest();
    let installed = installed_binaries(&manifest);
    assert_eq!(
        installed,
        BTreeSet::from(["keyvo".into(), "keyvo-app".into()])
    );
    let desktop = repo_file(&format!("contrib/flatpak/{APP_ID}.desktop"));
    let exec = desktop_value(&desktop, "Exec").and_then(|e| e.split_whitespace().next());
    assert!(exec.is_some_and(|e| installed.contains(e)), "{exec:?}");
    let command = manifest_value(&manifest, "command").unwrap_or_default();
    assert!(installed.contains(command), "{command}");
}

/// `keyvo` and `keyvo-daemon` (the workspace version), `keyvo-app` (its own
/// Cargo.toml and tauri.conf.json) and the metainfo's newest release are
/// one version.
#[test]
fn the_three_binaries_and_the_metainfo_report_one_version() {
    let app: toml::Table = repo_file("app/src-tauri/Cargo.toml").parse().unwrap();
    assert_eq!(
        app["package"]["version"].as_str(),
        Some(keyvo_core::VERSION)
    );
    assert_eq!(tauri_conf()["version"], keyvo_core::VERSION);
    let metainfo = repo_file(&format!("contrib/flatpak/{APP_ID}.metainfo.xml"));
    assert_eq!(
        xml_text(&metainfo, "<release version=\"", "\""),
        Some(keyvo_core::VERSION)
    );
}

/// The `key=value` lines of an ini file (a systemd unit) by section, comments
/// and blank lines dropped; a key may repeat, so each holds a list.
fn ini(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut entries = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            name.clone_into(&mut section);
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .unwrap_or_else(|| panic!("not key=value: {line}"));
        entries.push((
            section.clone(),
            key.trim().to_owned(),
            value.trim().to_owned(),
        ));
    }
    entries
}

/// The user unit starts the daemon from `~/.local/bin` with the graphical
/// session, restarts it on failure and stops the daemon alone, so the
/// applications it launched survive (spec §8, `KillMode=process`).
#[test]
fn the_user_unit_carries_exactly_the_documented_keys() {
    let unit = ini(&repo_file("contrib/systemd/keyvo-daemon.service"));
    let expected = [
        ("Unit", "Description", "keyvo daemon"),
        ("Unit", "After", "graphical-session.target"),
        ("Unit", "PartOf", "graphical-session.target"),
        ("Service", "Type", "simple"),
        ("Service", "ExecStart", "%h/.local/bin/keyvo-daemon"),
        ("Service", "Restart", "on-failure"),
        ("Service", "RestartSec", "2"),
        ("Service", "KillMode", "process"),
        ("Service", "Slice", "app.slice"),
        ("Install", "WantedBy", "graphical-session.target"),
    ]
    .map(|(s, k, v)| (s.to_owned(), k.to_owned(), v.to_owned()));
    assert_eq!(unit, expected);
}

/// The udev rule: `uaccess` on the Keypad's hidraw nodes, the Bolt
/// receiver's HID++ node (interface `02`, `docs/hardware-notes.md` record
/// D8) and `/dev/uinput`, no mode, no group.
#[test]
fn the_udev_rule_tags_the_three_devices_uaccess_and_nothing_else() {
    let rule = repo_file("contrib/udev/70-keyvo.rules");
    let lines: Vec<_> = rule
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        lines,
        [
            r#"KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c354", TAG+="uaccess""#,
            r#"KERNEL=="hidraw*", SUBSYSTEM=="hidraw", IMPORT{parent}="HID_ID", ENV{HID_ID}=="0003:0000046D:0000C548", ATTRS{bInterfaceNumber}=="02", TAG+="uaccess""#,
            r#"KERNEL=="uinput", SUBSYSTEM=="misc", OPTIONS+="static_node=uinput", TAG+="uaccess""#,
        ]
    );
    for forbidden in ["MODE", "GROUP", "0666"] {
        assert!(!rule.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn the_dialpad_turns_the_volume_switches_tabs_mutes_and_indents_in_vs_code() {
    use keyvo_core::dialpad::{Bindings, ButtonSlot, Roller};
    use keyvo_core::stack::Layer;

    let chord = |keys: &str| {
        Some(Action::Chord {
            keys: keyvo_core::chord::Chord::parse(keys).unwrap(),
        })
    };
    let loaded = loaded();
    let profiles = &loaded.profiles;
    let global = Bindings::new(profiles, profiles.global());
    let (dial, _) = global.rotary(Roller::Dial).unwrap();
    assert_eq!(
        (&dial.cw, &dial.ccw),
        (&chord("KEY_VOLUMEUP"), &chord("KEY_VOLUMEDOWN"))
    );
    let (roller, _) = global.rotary(Roller::Roller).unwrap();
    assert_eq!(
        (&roller.cw, &roller.ccw),
        (&chord("ctrl+tab"), &chord("ctrl+shift+tab"))
    );
    let (mute, _) = global.button(ButtonSlot::new(1).unwrap()).unwrap();
    assert_eq!(mute.action, chord("KEY_MUTE"));
    // VS Code indents on the dial, the rest comes from the global table.
    let vscode = Bindings::new(profiles, profiles.get("vscode").unwrap());
    let (indent, layer) = vscode.rotary(Roller::Dial).unwrap();
    assert_eq!(
        (&indent.cw, &indent.ccw, layer),
        (&chord("ctrl+]"), &chord("ctrl+["), Layer::App)
    );
    assert_eq!(vscode.rotary(Roller::Roller).unwrap().1, Layer::Global);
    // Every binding has a label for the app's panel.
    for profile in profiles.iter() {
        let Some(dialpad) = &profile.dialpad else {
            continue;
        };
        let labels = dialpad.dial.iter().chain(&dialpad.roller).map(|r| &r.label);
        let buttons = dialpad.buttons.values().map(|b| &b.label);
        assert!(
            labels.chain(buttons).all(Option::is_some),
            "{}",
            profile.profile.id
        );
    }
}

#[test]
fn the_global_context_page_has_a_held_and_a_toggled_dial_modifier_no_app_profile_hides() {
    let loaded = loaded();
    let context = &loaded.profiles.global().pages[0];
    let key = |n: u8| &context.keys[&KeySlot::new(n).unwrap()];
    let brightness = key(7);
    assert_eq!(brightness.modifier_mode(), Some(ModifierMode::Held));
    assert!(brightness.action.is_some() && brightness.roller.is_some());
    let zoom = key(8);
    assert_eq!(zoom.modifier_mode(), Some(ModifierMode::Toggle));
    assert_eq!(zoom.action, None);
    // Every app profile leaves keys 7 and 8 to the global layer.
    for profile in loaded.profiles.iter().filter(|p| p.profile.id != "global") {
        for page in &profile.pages {
            for n in [7, 8] {
                assert!(
                    !page.keys.contains_key(&KeySlot::new(n).unwrap()),
                    "{} defines key {n}",
                    profile.profile.id
                );
            }
        }
    }
}
