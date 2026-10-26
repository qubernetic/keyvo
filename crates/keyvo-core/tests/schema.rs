//! The committed JSON Schemas in `schemas/` equal the generated ones.
//!
//! After a deliberate model change, rewrite them with
//! `KEYVO_BLESS=1 just test schema` and review the diff.

use std::path::{Path, PathBuf};

use keyvo_core::schema;

fn committed(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(name)
}

fn check(name: &str, generated: &str) {
    let path = committed(name);
    if std::env::var_os("KEYVO_BLESS").is_some() {
        std::fs::write(&path, generated).unwrap();
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        on_disk == generated,
        "{} is stale; run `KEYVO_BLESS=1 just test schema` and review the diff",
        path.display()
    );
}

#[test]
fn schema_profile_snapshot_is_current() {
    check("profile.schema.json", &schema::profile());
}

#[test]
fn schema_config_snapshot_is_current() {
    check("config.schema.json", &schema::config());
}

#[test]
fn schema_recipe_snapshot_is_current() {
    check("recipe.schema.json", &schema::recipe());
}

#[test]
fn schema_recipe_constrains_groups_variants_and_actions() {
    let recipe: serde_json::Value = serde_json::from_str(&schema::recipe()).unwrap();
    let text = recipe.to_string();
    assert_eq!(recipe["properties"]["schema_version"]["const"], 1);
    assert_eq!(recipe["properties"]["variants"]["minItems"], 1);
    for group in ["general", "system", "apps"] {
        assert!(text.contains(&format!(r#""const":"{group}""#)), "{text}");
    }
    assert!(text.contains(r#""const":"exec""#), "{text}");
    assert_eq!(
        recipe["required"],
        serde_json::json!(["schema_version", "recipe", "variants"])
    );
}

#[test]
fn schema_profile_constrains_versions_keys_and_actions() {
    let profile: serde_json::Value = serde_json::from_str(&schema::profile()).unwrap();
    let text = profile.to_string();
    assert_eq!(profile["properties"]["schema_version"]["const"], 1);
    assert!(text.contains(r#""^[1-9]$""#), "{text}");
    assert!(text.contains(r#""^#[0-9a-fA-F]{6}$""#), "{text}");
    assert!(text.contains(r#""const":"chord""#), "{text}");
    assert!(text.contains(r#""const":"exec""#), "{text}");
    assert_eq!(
        profile["required"],
        serde_json::json!(["schema_version", "profile"])
    );
}
