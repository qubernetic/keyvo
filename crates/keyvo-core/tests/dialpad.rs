//! The Dialpad's `[dialpad]` table of a profile (spec §7.2): both forms of
//! `cw` and `ccw`, the buttons, the errors with position.

use std::path::Path;

use keyvo_core::chord::Chord;
use keyvo_core::diagnostic::{Diagnostic, Position};
use std::time::SystemTime;

use keyvo_core::dialpad::{
    Bindings, ButtonSlot, Control, Roller, Turns, blocked, press, turn, turn_trigger,
};
use keyvo_core::edit::{ButtonFields, ProfileDocument, RotaryFields};
use keyvo_core::layout::Profiles;
use keyvo_core::profile::{Action, Profile};
use keyvo_core::stack::Layer;
use keyvo_core::trace::{Outcome, Trigger};

const HEAD: &str = "schema_version = 1\n[profile]\nid = \"code\"\nname = \"Code\"\n";

fn parse(dialpad: &str) -> Result<Profile, Diagnostic> {
    Profile::from_toml(&format!("{HEAD}{dialpad}"), Path::new("code.toml"))
}

fn chord(keys: &str) -> Action {
    Action::Chord {
        keys: Chord::parse(keys).unwrap(),
    }
}

fn button(n: u8) -> ButtonSlot {
    ButtonSlot::new(n).unwrap()
}

#[test]
fn a_profile_without_the_table_has_no_dialpad() {
    assert_eq!(parse("").unwrap().dialpad, None);
}

#[test]
fn the_long_form_names_full_actions_a_step_and_a_label() {
    let profile = parse(
        r#"
[dialpad]
dial = { cw = { type = "chord", keys = "ctrl+]" }, ccw = { type = "exec", cmd = "outdent" }, step = 3, label = "Indent" }
"#,
    )
    .unwrap();
    let dialpad = profile.dialpad.unwrap();
    let dial = dialpad.dial.unwrap();
    assert_eq!(dial.cw, Some(chord("ctrl+]")));
    assert_eq!(
        dial.ccw,
        Some(Action::Exec {
            cmd: "outdent".into()
        })
    );
    assert_eq!(dial.step.get(), 3);
    assert_eq!(dial.label.as_deref(), Some("Indent"));
    assert_eq!(dialpad.roller, None);
    assert!(dialpad.buttons.is_empty());
}

#[test]
fn the_short_form_is_a_chord_and_the_step_defaults_to_one() {
    let profile = parse(
        r#"
[dialpad]
roller = { cw = "ctrl+tab", ccw = "ctrl+shift+tab" }
"#,
    )
    .unwrap();
    let roller = profile.dialpad.unwrap().roller.unwrap();
    assert_eq!(roller.cw, Some(chord("ctrl+tab")));
    assert_eq!(roller.ccw, Some(chord("ctrl+shift+tab")));
    assert_eq!(roller.step.get(), 1);
    assert_eq!(roller.label, None);
}

#[test]
fn the_buttons_are_keys_without_an_icon_or_a_colour() {
    let profile = parse(
        r#"
[dialpad.buttons.1]
label = "Mute"
action = { type = "chord", keys = "KEY_MUTE" }
on_fail = { type = "exec", cmd = "notify-send mute" }

[dialpad.buttons.4]
label = "Nothing"
"#,
    )
    .unwrap();
    let buttons = profile.dialpad.unwrap().buttons;
    assert_eq!(
        buttons.keys().copied().collect::<Vec<_>>(),
        [button(1), button(4)]
    );
    let mute = &buttons[&button(1)];
    assert_eq!(mute.label.as_deref(), Some("Mute"));
    assert_eq!(mute.action, Some(chord("KEY_MUTE")));
    assert_eq!(
        mute.on_fail,
        Some(Action::Exec {
            cmd: "notify-send mute".into()
        })
    );
    assert_eq!(buttons[&button(4)].action, None);
}

/// The diagnostic of `dialpad` after the profile head.
fn error(dialpad: &str) -> (Position, String) {
    let error = parse(dialpad).unwrap_err();
    (error.position.unwrap(), error.message)
}

#[test]
fn bad_fields_are_rejected_with_their_position() {
    // Line 5 is the first line of the table text.
    for (text, line, message) in [
        ("[dialpad]\nknob = {}\n", 6, "unknown field `knob`"),
        (
            "[dialpad]\ndial = { cw = \"ctrl+]\", speed = 2 }\n",
            6,
            "unknown field `speed`",
        ),
        (
            "[dialpad]\ndial = { step = 0 }\n",
            6,
            "invalid value: integer `0`",
        ),
        ("[dialpad]\ndial = { cw = \"ctrl+nope\" }\n", 6, "nope"),
        (
            "[dialpad.buttons.5]\nlabel = \"x\"\n",
            5,
            "a Dialpad button from 1 to 4",
        ),
        (
            "[dialpad.buttons.1]\nicon = \"lucide:x\"\n",
            6,
            "unknown field `icon`",
        ),
        (
            "[dialpad.buttons.2]\ncolor = \"#ffffff\"\n",
            6,
            "unknown field `color`",
        ),
    ] {
        let (position, found) = error(text);
        assert_eq!(position.line, line, "{text}: {found}");
        assert!(found.contains(message), "{text}: {found}");
    }
}

/// A profile file with a `[dialpad]` table in both forms and comments.
const EDITED: &str = r#"schema_version = 1

[profile]
id = "code"
name = "Code"

[[pages]]
name = "context"

# The Dialpad.
[dialpad]
dial = { cw = "ctrl+]", ccw = "ctrl+[", label = "Indent" } # indent
roller = { cw = { type = "chord", keys = "ctrl+tab" }, ccw = { type = "chord", keys = "ctrl+shift+tab" }, step = 2 }

[dialpad.buttons.1]
label = "Mute" # the speaker
action = { type = "chord", keys = "KEY_MUTE" }
on_fail = { type = "exec", cmd = "notify-send mute" }
"#;

fn edited() -> ProfileDocument {
    ProfileDocument::parse(EDITED, Path::new("code.toml")).unwrap()
}

#[test]
fn the_editor_reads_the_controls() {
    let doc = edited();
    assert_eq!(
        doc.rotary(Roller::Dial),
        RotaryFields {
            cw: Some(chord("ctrl+]")),
            ccw: Some(chord("ctrl+[")),
            step: None,
            label: Some("Indent".into()),
        }
    );
    assert_eq!(doc.rotary(Roller::Roller).step, Some(2));
    assert_eq!(
        doc.button(button(1)),
        ButtonFields {
            label: Some("Mute".into()),
            action: Some(chord("KEY_MUTE")),
        }
    );
    assert_eq!(doc.button(button(2)), ButtonFields::default());
    assert_eq!(
        doc.button_on_fail(button(1)),
        Some(Action::Exec {
            cmd: "notify-send mute".into()
        })
    );
}

#[test]
fn a_short_form_chord_stays_short_and_every_other_line_stays() {
    let mut doc = edited();
    let before = doc.rotary(Roller::Dial);
    let fields = RotaryFields {
        cw: Some(chord("ctrl+shift+]")),
        ..before.clone()
    };
    assert_eq!(doc.set_rotary(Roller::Dial, &fields).unwrap(), before);
    let text = doc.to_string();
    assert_eq!(
        text,
        EDITED.replace("cw = \"ctrl+]\"", "cw = \"ctrl+shift+]\"")
    );
    // Undo gives the file back byte for byte.
    doc.set_rotary(Roller::Dial, &before).unwrap();
    assert_eq!(doc.to_string(), EDITED);
}

#[test]
fn another_action_type_replaces_the_short_form_with_a_table() {
    let mut doc = edited();
    let fields = RotaryFields {
        ccw: Some(Action::Exec {
            cmd: "outdent".into(),
        }),
        step: Some(5),
        ..doc.rotary(Roller::Dial)
    };
    doc.set_rotary(Roller::Dial, &fields).unwrap();
    let text = doc.to_string();
    assert!(
        text.contains(
            "dial = { cw = \"ctrl+]\", ccw = { type = \"exec\", cmd = \"outdent\" }, label = \"Indent\", step = 5 } # indent"
        ),
        "{text}"
    );
    assert_eq!(doc.rotary(Roller::Dial), fields);
    // Step 1 is the default and is not written.
    doc.set_rotary(
        Roller::Dial,
        &RotaryFields {
            step: Some(1),
            ..fields
        },
    )
    .unwrap();
    assert!(!doc.to_string().contains("step = 5"));
    assert_eq!(doc.rotary(Roller::Dial).step, None);
}

#[test]
fn a_button_edit_keeps_its_on_fail_and_a_new_control_is_added() {
    let mut doc = edited();
    doc.set_button(
        button(1),
        &ButtonFields {
            label: Some("Mic".into()),
            action: Some(chord("KEY_MICMUTE")),
        },
    )
    .unwrap();
    let text = doc.to_string();
    assert!(text.contains("label = \"Mic\" # the speaker\n"), "{text}");
    assert!(text.contains("keys = \"KEY_MICMUTE\""), "{text}");
    assert!(text.contains("on_fail = { type = \"exec\""), "{text}");
    doc.set_button(
        button(3),
        &ButtonFields {
            label: Some("Play".into()),
            action: Some(chord("KEY_PLAYPAUSE")),
        },
    )
    .unwrap();
    assert_eq!(doc.button(button(3)).label.as_deref(), Some("Play"));
    assert!(doc.to_string().contains("[dialpad.buttons.3]"), "{}", doc);
}

#[test]
fn the_first_binding_opens_the_table() {
    let mut doc = ProfileDocument::parse(
        &format!("{HEAD}\n[[pages]]\nname = \"context\"\n"),
        Path::new("code.toml"),
    )
    .unwrap();
    let fields = RotaryFields {
        cw: Some(chord("KEY_VOLUMEUP")),
        ..RotaryFields::default()
    };
    doc.set_rotary(Roller::Dial, &fields).unwrap();
    assert_eq!(doc.rotary(Roller::Dial), fields);
    let text = doc.to_string();
    assert!(
        text.contains("[dialpad]\ndial = { cw = { type = \"chord\", keys = \"KEY_VOLUMEUP\" } }\n"),
        "{text}"
    );
    doc.set_button(
        button(2),
        &ButtonFields {
            label: Some("Two".into()),
            action: None,
        },
    )
    .unwrap();
    assert_eq!(doc.button(button(2)).label.as_deref(), Some("Two"));
}

const GLOBAL: &str = r#"schema_version = 1
[profile]
id = "global"
name = "Global"

[dialpad]
dial = { cw = "KEY_VOLUMEUP", ccw = "KEY_VOLUMEDOWN", label = "Volume" }
roller = { cw = "ctrl+tab", ccw = "ctrl+shift+tab", step = 3 }

[dialpad.buttons.1]
action = { type = "chord", keys = "KEY_MUTE" }
"#;

const CODE: &str = r#"schema_version = 1
[profile]
id = "code"
name = "Code"
match = { class = ["code"] }

[dialpad]
dial = { cw = "ctrl+]", ccw = "ctrl+[", label = "Indent" }

[dialpad.buttons.2]
label = "Nothing"
"#;

fn profiles(texts: &[&str]) -> Profiles {
    Profiles::new(
        texts
            .iter()
            .map(|text| {
                let profile = Profile::from_toml(text, Path::new("p.toml")).unwrap();
                (profile.profile.id.clone(), profile)
            })
            .collect(),
    )
}

fn real() -> (Trigger, SystemTime) {
    (turn_trigger(false), SystemTime::UNIX_EPOCH)
}

#[test]
fn a_control_the_app_profile_leaves_out_comes_from_the_global_table() {
    let profiles = profiles(&[GLOBAL, CODE]);
    let code = Bindings::new(&profiles, profiles.get("code").unwrap());
    let (dial, layer) = code.rotary(Roller::Dial).unwrap();
    assert_eq!((dial.label.as_deref(), layer), (Some("Indent"), Layer::App));
    let (roller, layer) = code.rotary(Roller::Roller).unwrap();
    assert_eq!((roller.step.get(), layer), (3, Layer::Global));
    assert_eq!(code.button(button(1)).unwrap().1, Layer::Global);
    // A button the app profile defines hides the global one, even unbound.
    assert_eq!(code.button(button(2)).unwrap().1, Layer::App);
    assert_eq!(code.button(button(3)), None);
    let global = Bindings::new(&profiles, profiles.global());
    assert_eq!(global.rotary(Roller::Dial).unwrap().1, Layer::Global);
    assert!(global.any());
}

#[test]
fn a_turn_runs_once_per_step_with_the_cap() {
    // (step, the deltas of one direction, runs of each, at most 8 a turn)
    for (step, deltas, runs) in [
        (1, &[1, 3, 20][..], &[1, 3, 8][..]),
        (3, &[1, 3, 20], &[0, 1, 7]),
        (3, &[1, 1, 1, 2], &[0, 0, 1, 0]),
        (3, &[-2, -1, -4], &[0, 1, 1]),
    ] {
        let mut turns = Turns::default();
        let got: Vec<u32> = deltas
            .iter()
            .map(|&delta| turns.turn(Roller::Dial, delta, step, 8))
            .collect();
        assert_eq!(got, runs, "step {step}, deltas {deltas:?}");
    }
}

#[test]
fn a_turn_the_other_way_starts_from_nothing() {
    let mut turns = Turns::default();
    assert_eq!(turns.turn(Roller::Roller, 2, 3, 8), 0);
    assert_eq!(turns.turn(Roller::Roller, -2, 3, 8), 0);
    assert_eq!(turns.turn(Roller::Roller, -1, 3, 8), 1);
    // Each roller carries its own detents.
    assert_eq!(turns.turn(Roller::Dial, 2, 3, 8), 0);
    assert_eq!(turns.turn(Roller::Roller, 2, 3, 8), 0);
    assert_eq!(turns.turn(Roller::Dial, 1, 3, 8), 1);
    turns.clear();
    assert_eq!(turns.turn(Roller::Roller, 1, 3, 8), 0);
}

#[test]
fn a_turn_resolves_the_direction_and_the_layer() {
    let profiles = profiles(&[GLOBAL, CODE]);
    let code = Bindings::new(&profiles, profiles.get("code").unwrap());
    let mut turns = Turns::default();
    let up = turn(&code, 1, Roller::Dial, 2, real(), &mut turns, 8);
    assert_eq!(up.runs, 2);
    assert_eq!(up.record.action, Some(chord("ctrl+]")));
    assert_eq!(up.record.control, Some(Control::Dial));
    assert_eq!(up.record.key, None);
    assert_eq!(up.record.layer, Some(Layer::App));
    assert_eq!(up.record.result, Outcome::Pending);
    let down = turn(&code, 1, Roller::Roller, -4, real(), &mut turns, 8);
    assert_eq!(down.record.action, Some(chord("ctrl+shift+tab")));
    assert_eq!((down.runs, down.record.layer), (1, Some(Layer::Global)));
    // A simulated turn neither reads nor changes the carry.
    let dry = turn(
        &code,
        1,
        Roller::Roller,
        -2,
        (turn_trigger(true), SystemTime::UNIX_EPOCH),
        &mut turns,
        8,
    );
    assert_eq!((dry.runs, dry.record.result), (0, Outcome::DryRun));
    assert_eq!(turns.turn(Roller::Roller, -2, 3, 8), 1);
}

#[test]
fn a_turn_without_a_binding_runs_nothing() {
    let profiles = profiles(&[CODE]);
    let code = Bindings::new(&profiles, profiles.get("code").unwrap());
    let mut turns = Turns::default();
    let unbound = turn(&code, 1, Roller::Roller, 5, real(), &mut turns, 8);
    assert_eq!((unbound.runs, unbound.record.result), (0, Outcome::Unbound));
    assert_eq!(unbound.record.layer, None);
    assert!(!Bindings::new(&profiles, profiles.global()).any());
}

#[test]
fn a_button_press_resolves_like_a_key() {
    let profiles = profiles(&[GLOBAL, CODE]);
    let code = Bindings::new(&profiles, profiles.get("code").unwrap());
    let at = SystemTime::UNIX_EPOCH;
    let mute = press(&code, 1, button(1), turn_trigger(false), at);
    assert_eq!(mute.action, Some(chord("KEY_MUTE")));
    assert_eq!(mute.control, Some(Control::Button(button(1))));
    assert_eq!(mute.result, Outcome::Pending);
    let nothing = press(&code, 1, button(2), turn_trigger(true), at);
    assert_eq!(nothing.result, Outcome::Unbound);
    let blocked = blocked(
        "code",
        1,
        Control::Dial,
        turn_trigger(false),
        at,
        "session locked",
    );
    assert_eq!(
        blocked.result,
        Outcome::Blocked {
            reason: "session locked".into()
        }
    );
}

#[test]
fn a_control_is_written_dial_roller_or_button_n() {
    for (control, text) in [
        (Control::Dial, "dial"),
        (Control::Roller, "roller"),
        (Control::Button(button(3)), "button:3"),
    ] {
        assert_eq!(control.to_string(), text);
        assert_eq!(text.parse::<Control>(), Ok(control));
        assert_eq!(
            serde_json::to_string(&control).unwrap(),
            format!("\"{text}\"")
        );
    }
    for bad in ["button:5", "button:0", "knob", "button:"] {
        assert!(bad.parse::<Control>().is_err(), "{bad}");
    }
    assert_eq!(Roller::from_id(0), Some(Roller::Roller));
    assert_eq!(Roller::from_id(1), Some(Roller::Dial));
    assert_eq!(Roller::from_id(2), None);
    assert_eq!(Control::from(Roller::Dial).roller(), Some(Roller::Dial));
    assert_eq!(Control::Button(button(1)).roller(), None);
}
