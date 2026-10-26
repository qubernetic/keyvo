//! Recipe packs: the fixtures under `tests/fixtures/recipes/`.

use std::path::{Path, PathBuf};

use keyvo_core::chord::Chord;
use keyvo_core::diagnostic::Diagnostic;
use keyvo_core::profile::Action;
use keyvo_core::recipe::{Group, Recipe, Source, Variant};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/recipes")
        .join(name)
}

fn parse(name: &str) -> Result<Recipe, Diagnostic> {
    let path = fixture(name);
    let text = std::fs::read_to_string(&path).unwrap();
    Recipe::from_toml(&text, &path, Source::User)
}

#[test]
fn a_pack_parses_every_field_and_takes_its_id_from_the_file_name() {
    let recipe = parse("valid/screenshot.toml").unwrap();
    assert_eq!(
        recipe,
        Recipe {
            id: "screenshot".into(),
            source: Source::User,
            group: Group::Apps,
            name: "Screenshot".into(),
            icon: "lucide:camera".into(),
            label: "Shot".into(),
            variants: vec![
                Variant {
                    id: "full".into(),
                    name: "Full screen".into(),
                    action: Action::Exec {
                        cmd: "spectacle --fullscreen".into()
                    },
                },
                Variant {
                    id: "key".into(),
                    name: "Print key".into(),
                    action: Action::Chord {
                        keys: Chord::parse("print").unwrap()
                    },
                },
            ],
        }
    );
}

#[test]
fn an_unknown_action_kind_names_the_file_and_line() {
    let error = parse("invalid/unknown-action.toml").unwrap_err();
    assert_eq!(error.path, fixture("invalid/unknown-action.toml"));
    assert_eq!(error.position.map(|p| p.line), Some(12), "{error}");
    assert!(error.message.contains("unknown variant `dbus`"), "{error}");
    assert!(
        error.to_string().contains("unknown-action.toml:12:"),
        "{error}"
    );
}

/// Small invalid packs, each with the line of its problem and a part of
/// the message.
const INVALID: &[(&str, Option<usize>, &str)] = &[
    (
        "schema_version = 1\n[recipe]\ngroup = \"apps\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n",
        Some(1),
        "missing field `variants`",
    ),
    (
        "schema_version = 1\nvariants = []\n[recipe]\ngroup = \"apps\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n",
        Some(2),
        "at least one variant",
    ),
    (
        "schema_version = 1\n[recipe]\ngroup = \"games\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n\
         [[variants]]\nid = \"a\"\nname = \"A\"\naction = { type = \"exec\", cmd = \"true\" }\n",
        Some(3),
        "unknown variant `games`",
    ),
    (
        "schema_version = 1\n[recipe]\ngroup = \"apps\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n\
         [[variants]]\nid = \"a\"\nname = \"A\"\nacton = { type = \"exec\", cmd = \"true\" }\n",
        Some(10),
        "unknown field `acton`",
    ),
    (
        "schema_version = 1\n[recipe]\ngroup = \"apps\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n\
         [[variants]]\nid = \"Big\"\nname = \"A\"\naction = { type = \"exec\", cmd = \"true\" }\n",
        Some(8),
        "lowercase letters",
    ),
    (
        "schema_version = 1\n[recipe]\ngroup = \"apps\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n\
         [[variants]]\nid = \"a\"\nname = \"A\"\naction = { type = \"exec\", cmd = \"true\" }\n\
         [[variants]]\nid = \"a\"\nname = \"B\"\naction = { type = \"exec\", cmd = \"false\" }\n",
        Some(12),
        "variant `a` twice",
    ),
    (
        "schema_version = 1\n[recipe]\ngroup = \"apps\"\nname = \"A\"\nicon = \"lucide:x\"\nlabel = \"A\"\n\
         [[variants]]\nid = \"a\"\nname = \"A\"\naction = { type = \"chord\", keys = \"ctrl+nope\" }\n",
        Some(10),
        "nope",
    ),
    (
        "schema_version = 2\n",
        Some(1),
        "unsupported schema_version 2",
    ),
];

#[test]
fn every_invalid_pack_names_its_line() {
    for (text, line, message) in INVALID {
        let error = Recipe::from_toml(text, Path::new("p.toml"), Source::User).unwrap_err();
        assert_eq!(error.position.map(|p| p.line), *line, "{error}\n{text}");
        assert!(error.message.contains(message), "{error}\n{text}");
    }
}

#[test]
fn a_file_name_that_is_not_an_id_is_refused() {
    let text = std::fs::read_to_string(fixture("valid/screenshot.toml")).unwrap();
    let error = Recipe::from_toml(&text, Path::new("Screen Shot.toml"), Source::User).unwrap_err();
    assert_eq!(error.position, None);
    assert!(error.message.contains("`Screen Shot`"), "{error}");
}
