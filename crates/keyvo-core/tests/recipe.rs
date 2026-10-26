//! Recipe packs: the fixtures under `tests/fixtures/recipes/`.

use std::path::{Path, PathBuf};

use keyvo_core::chord::Chord;
use keyvo_core::diagnostic::Diagnostic;
use keyvo_core::profile::Action;
use keyvo_core::recipe::{self, Group, Recipe, Source, Variant};

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

/// A configuration directory with `recipes/` holding `files`.
fn config_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let recipes = dir.path().join(recipe::RECIPES);
    std::fs::create_dir(&recipes).unwrap();
    for (name, text) in files {
        std::fs::write(recipes.join(name), text).unwrap();
    }
    dir
}

fn screenshot() -> String {
    std::fs::read_to_string(fixture("valid/screenshot.toml")).unwrap()
}

#[test]
fn the_catalogue_is_the_bundled_packs_without_a_user_directory() {
    let dir = tempfile::tempdir().unwrap();
    let catalogue = recipe::catalogue(dir.path());
    assert!(catalogue.problems.is_empty(), "{:?}", catalogue.problems);
    assert_eq!(catalogue.recipes, recipe::bundled().recipes);
    assert!(
        catalogue
            .recipes
            .iter()
            .all(|r| r.source == Source::Bundled)
    );
}

#[test]
fn a_user_pack_joins_the_catalogue_in_group_and_name_order() {
    let dir = config_dir(&[("screenshot.toml", &screenshot()), ("notes.txt", "x")]);
    let catalogue = recipe::catalogue(dir.path());
    assert!(catalogue.problems.is_empty(), "{:?}", catalogue.problems);
    let ids: Vec<_> = catalogue
        .recipes
        .iter()
        .filter(|r| r.group == Group::Apps)
        .map(|r| (r.id.as_str(), r.source))
        .collect();
    assert_eq!(
        ids,
        [
            ("calculator", Source::Bundled),
            ("konsole", Source::Bundled),
            ("screenshot", Source::User),
            ("spectacle", Source::Bundled),
            ("vscode", Source::Bundled),
        ]
    );
    let groups: Vec<_> = catalogue.recipes.iter().map(|r| r.group).collect();
    assert!(groups.is_sorted(), "{groups:?}");
}

#[test]
fn a_user_pack_replaces_the_bundled_one_of_its_id() {
    let dir = config_dir(&[("spectacle.toml", &screenshot())]);
    let catalogue = recipe::catalogue(dir.path());
    let spectacle: Vec<_> = catalogue
        .recipes
        .iter()
        .filter(|r| r.id == "spectacle")
        .collect();
    assert_eq!(spectacle.len(), 1);
    assert_eq!(spectacle[0].source, Source::User);
    assert_eq!(spectacle[0].name, "Screenshot");
}

#[test]
fn a_broken_user_pack_is_a_problem_and_the_others_stay() {
    let broken = std::fs::read_to_string(fixture("invalid/unknown-action.toml")).unwrap();
    let dir = config_dir(&[("screenshot.toml", &screenshot()), ("volume.toml", &broken)]);
    let catalogue = recipe::catalogue(dir.path());
    assert!(catalogue.recipes.iter().any(|r| r.id == "screenshot"));
    assert!(!catalogue.recipes.iter().any(|r| r.id == "volume"));
    let [problem] = catalogue.problems.as_slice() else {
        panic!("{:?}", catalogue.problems);
    };
    assert!(problem.path.ends_with("recipes/volume.toml"), "{problem}");
    assert_eq!(problem.position.map(|p| p.line), Some(12), "{problem}");
}

#[test]
fn a_user_pack_over_the_size_cap_is_not_read() {
    let big = format!("{}#{}\n", screenshot(), "x".repeat(recipe::PACK_BYTES));
    let dir = config_dir(&[("big.toml", &big)]);
    let catalogue = recipe::catalogue(dir.path());
    assert!(!catalogue.recipes.iter().any(|r| r.id == "big"));
    let [problem] = catalogue.problems.as_slice() else {
        panic!("{:?}", catalogue.problems);
    };
    assert!(problem.message.contains("larger than"), "{problem}");
}

#[test]
fn user_packs_past_the_count_cap_are_left_out_with_one_problem() {
    let text = screenshot();
    let names: Vec<String> = (0..=recipe::USER_PACKS)
        .map(|i| format!("pack-{i:03}.toml"))
        .collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), text.as_str())).collect();
    let dir = config_dir(&files);
    let catalogue = recipe::catalogue(dir.path());
    let users = catalogue
        .recipes
        .iter()
        .filter(|r| r.source == Source::User)
        .count();
    assert_eq!(users, recipe::USER_PACKS);
    let [problem] = catalogue.problems.as_slice() else {
        panic!("{:?}", catalogue.problems);
    };
    assert!(problem.message.contains("pack-064.toml"), "{problem}");
}

#[test]
fn a_file_name_that_is_not_an_id_is_refused() {
    let text = std::fs::read_to_string(fixture("valid/screenshot.toml")).unwrap();
    let error = Recipe::from_toml(&text, Path::new("Screen Shot.toml"), Source::User).unwrap_err();
    assert_eq!(error.position, None);
    assert!(error.message.contains("`Screen Shot`"), "{error}");
}
