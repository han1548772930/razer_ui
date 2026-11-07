//! The current source-profile 13 to 14 file migration. This boundary preserves
//! source property presence and Object.assign order before typed decoding.
use anyhow::{Context as _, Result, ensure};
use serde_json::{Map, Value};

pub(crate) fn migrate_13_to_14(value: Value) -> Result<Value> {
    migrate_with(value, || uuid::Uuid::new_v4().to_string())
}

fn migrate_with(value: Value, fresh_guid: impl FnOnce() -> String) -> Result<Value> {
    let Value::Object(mut root) = value else {
        anyhow::bail!("Source profile migration requires an object");
    };
    let schema = root
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .context("Source profile schemaVersion must be an unsigned integer")?;
    if schema == 14 {
        return Ok(Value::Object(root));
    }
    ensure!(
        schema == 13,
        "The source profile 13 to 14 migration requires schema 13"
    );
    if let Some(intensity) = root
        .get("audioHapticIntensity")
        .filter(|value| !value.is_array())
        .cloned()
    {
        let selected = root
            .get("activeHapticIntensityGuid")
            .filter(|value| !value.is_null())
            .cloned()
            .unwrap_or_else(|| Value::String(fresh_guid()));
        let mut item = Map::new();
        item.insert("guid".into(), selected.clone());
        match intensity {
            Value::Object(fields) => item.extend(fields),
            Value::String(text) => {
                // JSON strings cannot represent the isolated UTF-16 surrogate
                // properties produced by Object.assign for supplementary text.
                ensure!(
                    text.chars().all(|character| character.len_utf16() == 1),
                    "Haptic intensity string cannot preserve source UTF-16 properties"
                );
                item.extend(text.chars().enumerate().map(|(index, character)| {
                    (index.to_string(), Value::String(character.to_string()))
                }));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
            Value::Array(_) => unreachable!("Array migration is excluded above"),
        }
        root.insert(
            "audioHapticIntensity".into(),
            Value::Array(vec![Value::Object(item)]),
        );
        // An explicit null selects a generated item GUID but remains an
        // existing root property in the actual source.
        root.entry("activeHapticIntensityGuid").or_insert(selected);
    }
    root.insert("schemaVersion".into(), 14.into());
    Ok(Value::Object(root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_schema14_keeps_absence_existing_arrays_and_other_fields() {
        for mut input in [
            json!({"schemaVersion":13,"profiles":[],"future":7}),
            json!({"schemaVersion":13,"audioHapticIntensity":[{"guid":"existing"}],"activeHapticIntensityGuid":null}),
        ] {
            let output =
                migrate_with(input.clone(), || panic!("No new GUID on this path")).unwrap();
            input["schemaVersion"] = json!(14);
            assert_eq!(output, input);
        }
    }

    #[test]
    fn source_schema14_preserves_guid_presence_and_object_override_order() {
        let output = migrate_with(json!({"schemaVersion":13,"audioHapticIntensity":{"value":42,"guid":"item"},"activeHapticIntensityGuid":"selected"}), || panic!("Existing GUID")).unwrap();
        assert_eq!(
            output["audioHapticIntensity"],
            json!([{"guid":"item","value":42}])
        );
        assert_eq!(output["activeHapticIntensityGuid"], "selected");
        let output = migrate_with(json!({"schemaVersion":13,"audioHapticIntensity":{"value":42},"activeHapticIntensityGuid":null}), || "fresh".into()).unwrap();
        assert_eq!(output["audioHapticIntensity"][0]["guid"], "fresh");
        assert!(output["activeHapticIntensityGuid"].is_null());
        let output = migrate_with(
            json!({"schemaVersion":13,"audioHapticIntensity":{"value":42}}),
            || "fresh".into(),
        )
        .unwrap();
        assert_eq!(output["activeHapticIntensityGuid"], "fresh");
    }

    #[test]
    fn source_schema14_uses_nullish_guid_selection_and_defined_primitive_spread() {
        for value in [Value::Null, json!(false), json!(17)] {
            let output = migrate_with(json!({"schemaVersion":13,"audioHapticIntensity":value,"activeHapticIntensityGuid":""}), || panic!("Empty string is defined")).unwrap();
            assert_eq!(output["audioHapticIntensity"], json!([{"guid":""}]));
        }
        let output = migrate_with(
            json!({"schemaVersion":13,"audioHapticIntensity":"ab"}),
            || "fresh".into(),
        )
        .unwrap();
        assert_eq!(
            output["audioHapticIntensity"],
            json!([{"guid":"fresh","0":"a","1":"b"}])
        );
        assert!(
            migrate_with(
                json!({"schemaVersion":13,"audioHapticIntensity":"😀"}),
                || "fresh".into()
            )
            .is_err()
        );
    }

    #[test]
    fn source_schema14_rejects_other_versions_and_leaves_current_documents_unchanged() {
        for value in [
            json!(null),
            json!({}),
            json!({"schemaVersion":null}),
            json!({"schemaVersion":12}),
            json!({"schemaVersion":15}),
        ] {
            assert!(migrate_with(value, || panic!("Invalid version")).is_err());
        }
        let input = json!({"schemaVersion":14,"audioHapticIntensity":{"unexpected":1}});
        assert_eq!(
            migrate_with(input.clone(), || panic!("No repeated migration")).unwrap(),
            input
        );
    }
}
