//! JSON Schemas of the configuration files, generated from the types with
//! `schemars` (spec §7.1). The committed copies in `schemas/` back editor
//! validation; a test keeps them equal to the generated text.

use schemars::generate::SchemaSettings;
use schemars::transform::RecursiveTransform;
use schemars::{JsonSchema, Schema};
use serde_json::Value;

use crate::config::Config;
use crate::profile::Profile;

/// The schema of `profiles/<id>.toml`, pretty JSON with a final newline.
#[must_use]
pub fn profile() -> String {
    render::<Profile>()
}

/// The schema of `config.toml`, pretty JSON with a final newline.
#[must_use]
pub fn config() -> String {
    render::<Config>()
}

fn render<T: JsonSchema>() -> String {
    let generator = SchemaSettings::draft2020_12()
        .with_transform(RecursiveTransform(drop_null))
        .into_generator();
    let schema = Value::from(generator.into_root_schema_for::<T>());
    format!("{schema:#}\n")
}

/// TOML has no null: an optional key is one that may be left out, so the
/// `null` alternative `schemars` adds for every `Option` is removed.
fn drop_null(schema: &mut Schema) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    if let Some(Value::Array(types)) = object.get_mut("type") {
        types.retain(|t| t != "null");
        if let [only] = types.as_slice() {
            let only = only.clone();
            object.insert("type".to_owned(), only);
        }
    }
    let Some(Value::Array(any_of)) = object.get("anyOf") else {
        return;
    };
    let null = serde_json::json!({ "type": "null" });
    let rest: Vec<&Value> = any_of.iter().filter(|s| **s != null).collect();
    if let [Value::Object(only)] = rest.as_slice() {
        let only = only.clone();
        object.remove("anyOf");
        for (key, value) in only {
            object.entry(key).or_insert(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dropped(value: Value) -> Value {
        let mut schema = Schema::try_from(value).unwrap();
        drop_null(&mut schema);
        Value::from(schema)
    }

    #[test]
    fn null_type_and_null_alternative_are_removed() {
        assert_eq!(
            dropped(serde_json::json!({ "type": ["string", "null"] })),
            serde_json::json!({ "type": "string" })
        );
        assert_eq!(
            dropped(serde_json::json!({
                "description": "d",
                "anyOf": [{ "$ref": "#/$defs/Color" }, { "type": "null" }]
            })),
            serde_json::json!({ "description": "d", "$ref": "#/$defs/Color" })
        );
    }

    #[test]
    fn other_schemas_are_kept() {
        for value in [
            serde_json::json!(true),
            serde_json::json!({ "type": ["string", "integer"] }),
            serde_json::json!({ "anyOf": [{ "type": "string" }, { "type": "integer" }] }),
        ] {
            assert_eq!(dropped(value.clone()), value);
        }
    }
}
