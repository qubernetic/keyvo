//! The chord parser (spec §7.2): aliases, shifted symbols, evdev key names
//! and sequences, as table tests.

use keyvo_core::chord::{Chord, ChordError, KeyCode, KeyEvent};

const LEFTCTRL: u16 = 29;
const LEFTSHIFT: u16 = 42;
const LEFTALT: u16 = 56;
const RIGHTALT: u16 = 100;
const LEFTMETA: u16 = 125;

/// Expected steps as `(modifier codes, key code)`.
type Expected = &'static [(&'static [u16], u16)];

/// Each step as `(modifier codes, key code)`.
fn steps(text: &str) -> Vec<(Vec<u16>, u16)> {
    let chord = Chord::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    chord
        .steps()
        .iter()
        .map(|step| {
            (
                step.modifiers().codes().map(KeyCode::get).collect(),
                step.key().get(),
            )
        })
        .collect()
}

#[test]
fn chords_parse_to_modifiers_and_a_key() {
    let table: &[(&str, Expected)] = &[
        ("a", &[(&[], 30)]),
        ("ctrl+s", &[(&[LEFTCTRL], 31)]),
        ("control+s", &[(&[LEFTCTRL], 31)]),
        ("ctrl+shift+g", &[(&[LEFTCTRL, LEFTSHIFT], 34)]),
        // Modifiers always come out in one order.
        ("shift+ctrl+g", &[(&[LEFTCTRL, LEFTSHIFT], 34)]),
        ("alt+tab", &[(&[LEFTALT], 15)]),
        ("altgr+e", &[(&[RIGHTALT], 18)]),
        ("super+e", &[(&[LEFTMETA], 18)]),
        ("meta+e", &[(&[LEFTMETA], 18)]),
        // Case and spaces around the names do not matter.
        ("Ctrl + Shift + P", &[(&[LEFTCTRL, LEFTSHIFT], 25)]),
        ("CTRL+A", &[(&[LEFTCTRL], 30)]),
        // A modifier alone is pressed as its key.
        ("super", &[(&[], LEFTMETA)]),
        ("ctrl+shift", &[(&[LEFTCTRL], LEFTSHIFT)]),
        // Friendly aliases.
        ("escape", &[(&[], 1)]),
        ("return", &[(&[], 28)]),
        ("del", &[(&[], 111)]),
        ("ins", &[(&[], 110)]),
        ("pgup", &[(&[], 104)]),
        ("pgdn", &[(&[], 109)]),
        ("period", &[(&[], 52)]),
        ("quote", &[(&[], 40)]),
        ("backtick", &[(&[], 41)]),
        // Every evdev name, with or without its prefix.
        ("f5", &[(&[], 63)]),
        ("f24", &[(&[], 194)]),
        ("KEY_F13", &[(&[], 183)]),
        ("key_playpause", &[(&[], 164)]),
        ("volumeup", &[(&[], 115)]),
        ("space", &[(&[], 57)]),
        ("ctrl+0", &[(&[LEFTCTRL], 11)]),
        // Unshifted symbols.
        ("ctrl+/", &[(&[LEFTCTRL], 53)]),
        ("ctrl+-", &[(&[LEFTCTRL], 12)]),
        ("ctrl+=", &[(&[LEFTCTRL], 13)]),
        ("ctrl+[", &[(&[LEFTCTRL], 26)]),
        ("ctrl+]", &[(&[LEFTCTRL], 27)]),
        ("ctrl+\\", &[(&[LEFTCTRL], 43)]),
        ("ctrl+;", &[(&[LEFTCTRL], 39)]),
        ("ctrl+'", &[(&[LEFTCTRL], 40)]),
        ("ctrl+`", &[(&[LEFTCTRL], 41)]),
        ("ctrl+.", &[(&[LEFTCTRL], 52)]),
        // `,` and `+` are keys where a key is expected.
        ("ctrl+,", &[(&[LEFTCTRL], 51)]),
        ("ctrl++", &[(&[LEFTCTRL, LEFTSHIFT], 13)]),
        ("+", &[(&[LEFTSHIFT], 13)]),
        (",", &[(&[], 51)]),
        // Shifted symbols add shift (US layout).
        ("ctrl+?", &[(&[LEFTCTRL, LEFTSHIFT], 53)]),
        ("!", &[(&[LEFTSHIFT], 2)]),
        ("@", &[(&[LEFTSHIFT], 3)]),
        ("#", &[(&[LEFTSHIFT], 4)]),
        ("$", &[(&[LEFTSHIFT], 5)]),
        ("%", &[(&[LEFTSHIFT], 6)]),
        ("^", &[(&[LEFTSHIFT], 7)]),
        ("&", &[(&[LEFTSHIFT], 8)]),
        ("*", &[(&[LEFTSHIFT], 9)]),
        ("(", &[(&[LEFTSHIFT], 10)]),
        (")", &[(&[LEFTSHIFT], 11)]),
        ("_", &[(&[LEFTSHIFT], 12)]),
        ("{", &[(&[LEFTSHIFT], 26)]),
        ("}", &[(&[LEFTSHIFT], 27)]),
        ("|", &[(&[LEFTSHIFT], 43)]),
        (":", &[(&[LEFTSHIFT], 39)]),
        ("\"", &[(&[LEFTSHIFT], 40)]),
        ("~", &[(&[LEFTSHIFT], 41)]),
        ("<", &[(&[LEFTSHIFT], 51)]),
        (">", &[(&[LEFTSHIFT], 52)]),
        ("shift+?", &[(&[LEFTSHIFT], 53)]),
        // Sequences: steps separated by `,`.
        ("ctrl+k, ctrl+s", &[(&[LEFTCTRL], 37), (&[LEFTCTRL], 31)]),
        ("ctrl+,,a", &[(&[LEFTCTRL], 51), (&[], 30)]),
        (
            "ctrl+k,ctrl++",
            &[(&[LEFTCTRL], 37), (&[LEFTCTRL, LEFTSHIFT], 13)],
        ),
    ];
    for (text, expected) in table {
        let expected: Vec<(Vec<u16>, u16)> = expected
            .iter()
            .map(|(mods, key)| (mods.to_vec(), *key))
            .collect();
        assert_eq!(steps(text), expected, "{text}");
    }
}

#[test]
fn bad_chords_name_the_problem() {
    let table: &[(&str, ChordError, &str)] = &[
        ("", ChordError::Empty, "empty chord"),
        ("   ", ChordError::Empty, "empty chord"),
        (
            "ctrl+",
            ChordError::MissingKey,
            "a `+` or `,` must be followed by a key",
        ),
        (
            "ctrl+s,",
            ChordError::MissingKey,
            "a `+` or `,` must be followed by a key",
        ),
        (
            "ctrl+nope",
            ChordError::UnknownKey("nope".to_owned()),
            "unknown key `nope`",
        ),
        (
            "page up",
            ChordError::UnknownKey("page up".to_owned()),
            "unknown key `page up`",
        ),
        (
            "a+b",
            ChordError::NotAModifier("a".to_owned()),
            "`a` is not a modifier (ctrl, shift, alt, altgr, super)",
        ),
        (
            "ctrl+,b",
            ChordError::ExpectedSeparator(",".to_owned()),
            "expected `+`, `,` or the end after `,`",
        ),
    ];
    for (text, error, message) in table {
        let got = Chord::parse(text).unwrap_err();
        assert_eq!(&got, error, "{text}");
        assert_eq!(got.to_string(), *message, "{text}");
    }
}

#[test]
fn a_chord_keeps_its_source_text() {
    let chord = Chord::parse("Ctrl + Shift + P").unwrap();
    assert_eq!(chord.as_str(), "Ctrl + Shift + P");
    assert_eq!(chord.to_string(), "Ctrl + Shift + P");
}

#[test]
fn a_step_presses_modifiers_then_the_key_and_releases_in_reverse() {
    let down = |code| KeyEvent {
        code: KeyCode::new(code),
        down: true,
    };
    let up = |code| KeyEvent {
        code: KeyCode::new(code),
        down: false,
    };
    let chord = Chord::parse("ctrl+?, a").unwrap();
    assert_eq!(
        chord.events(),
        vec![
            down(LEFTCTRL),
            down(LEFTSHIFT),
            down(53),
            up(53),
            up(LEFTSHIFT),
            up(LEFTCTRL),
            down(30),
            up(30),
        ]
    );
}

#[test]
fn key_names_resolve_through_the_evdev_table() {
    assert_eq!(KeyCode::from_name("leftctrl"), Some(KeyCode::new(LEFTCTRL)));
    assert_eq!(KeyCode::from_name("KEY_A"), Some(KeyCode::new(30)));
    assert_eq!(KeyCode::from_name("reserved"), None);
    assert_eq!(KeyCode::from_name("max"), None);
    assert_eq!(KeyCode::from_name(""), None);
    // Aliases in the header resolve to their value.
    assert_eq!(KeyCode::from_name("hangeul"), KeyCode::from_name("hanguel"));
}

#[test]
fn every_code_of_the_table_is_listed_once_in_order() {
    let all: Vec<_> = KeyCode::all().collect();
    assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(all.contains(&KeyCode::new(LEFTCTRL)));
    assert!(all.contains(&KeyCode::from_name("f24").unwrap()));
    // Every chord key a parser accepts can be declared on a uinput device.
    for name in ["a", "leftmeta", "playpause", "KEY_F13"] {
        assert!(all.contains(&KeyCode::from_name(name).unwrap()), "{name}");
    }
}
