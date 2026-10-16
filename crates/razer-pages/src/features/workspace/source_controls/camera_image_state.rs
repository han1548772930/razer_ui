//! Current TU/dh/wU image-preset transitions and advancedWebcamReducer,
//! retained as complete owners by camera-local-pages. Only profile draft values
//! change here; a transport result or device observation is never synthesized.
use serde_json::Value;

/// TU's preset click reconstructs a built-in preset from DEFAULT_SETTINGS and
/// DEFAULTPROFILE.userSettings. Returning to custom retains its last dataset.
pub(super) fn select(
    profile: &mut Value,
    defaults: &Value,
    settings: &[Value],
    index: usize,
) -> bool {
    let Some(preset) = defaults.get("image").and_then(|v| v.get(index)) else {
        return false;
    };
    let Some(name) = preset.get("name").and_then(Value::as_str) else {
        return false;
    };
    let Some(mut data_set) = settings
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
        .cloned()
    else {
        return false;
    };
    if profile.get("imageMode").and_then(Value::as_u64) == Some(index as u64) {
        return false;
    }
    let Some(entries) = profile.get_mut("image").and_then(Value::as_array_mut) else {
        return false;
    };
    let Some(target) = entries.get_mut(index) else {
        return false;
    };
    if name != "custom" {
        *target = preset.clone();
        data_set["userSettings"] = preset["dataSet"]["userSettings"].clone();
        target["dataSet"] = data_set;
    }
    profile["imageMode"] = Value::from(index);
    true
}

/// TU.c distinguishes the first adjustment of a built-in preset from later
/// custom edits. The first keeps the built-in userSettings baseline; the auto
/// white-balance checkbox immediately updates userSettings even in that case.
/// The complete whiteBalance value is based on userSettings, as TU.d/checkbox
/// do, rather than on an unrelated getter result or the last displayed value.
pub(super) fn edit(
    profile: &mut Value,
    settings: &[Value],
    field: &str,
    value: &Value,
) -> Option<(usize, usize)> {
    let current = profile.get("imageMode")?.as_u64()? as usize;
    let entries = profile.get("image")?.as_array()?;
    let custom = entries
        .iter()
        .position(|entry| entry.get("name").and_then(Value::as_str) == Some("custom"))?;
    let selected = entries.get(current)?;
    let name = selected.get("name")?.as_str()?;
    let mut next = selected.get("dataSet")?.clone();
    let mut user_settings = next.get("userSettings")?.clone();
    let (property, property_value) = if let Some(part) = field.strip_prefix("whiteBalance/") {
        if !matches!(part, "value" | "isAuto") {
            return None;
        }
        let mut balance = user_settings.get("whiteBalance")?.clone();
        balance.as_object_mut()?.insert(part.into(), value.clone());
        ("whiteBalance", balance)
    } else if matches!(field, "brightness" | "contrast" | "saturation") {
        (field, value.clone())
    } else {
        return None;
    };
    if name == "custom" || field == "whiteBalance/isAuto" {
        user_settings
            .as_object_mut()?
            .insert(property.into(), property_value.clone());
    } else {
        let baseline = settings
            .iter()
            .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))?
            .as_object()?;
        for (key, value) in baseline {
            if key != "userSettings" && key != "name" {
                user_settings
                    .as_object_mut()?
                    .insert(key.clone(), value.clone());
            }
        }
    }
    next.as_object_mut()?
        .insert(property.into(), property_value);
    next.as_object_mut()?
        .insert("userSettings".into(), user_settings);
    next["name"] = "custom".into();
    next["isUpdateCustom"] = true.into();
    if name == "custom" || field == "whiteBalance/isAuto" {
        // TU.t receives {[property]: value}; e.name is undefined and is not
        // serialized by the MW profile writer.
        next.as_object_mut()?.remove("originalName");
    } else {
        next["originalName"] = name.into();
    }
    profile["image"][custom]["dataSet"] = next;
    profile["imageMode"] = Value::from(custom);
    Some((current, custom))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source_spec(product_id: u32) -> Value {
        let all: Value =
            serde_json::from_str(include_str!("../source_controls_data.json")).unwrap();
        all.as_array()
            .unwrap()
            .iter()
            .find(|spec| spec["product_id"] == product_id)
            .unwrap()
            .clone()
    }

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn first_edit_keeps_source_baseline_then_custom_edits_update_user_settings() {
        let spec = source_spec(3594);
        let defaults = &spec["profile"];
        let settings = spec["camera_image_settings"].as_array().unwrap();
        let mut profile = defaults.clone();
        // Current C (102695-102749) and P (102774-102977) deliberately
        // differ; selecting vibrant uses C values with P userSettings.
        assert_eq!(settings[2]["brightness"], 90);
        assert_eq!(defaults["image"][2]["dataSet"]["brightness"], 121);
        assert!(select(&mut profile, defaults, settings, 2));
        assert_eq!(profile["image"][2]["dataSet"]["brightness"], 90);
        assert_eq!(profile["image"][2]["dataSet"]["contrast"], 150);
        assert_eq!(profile["image"][2]["dataSet"]["saturation"], 170);
        assert_eq!(
            profile["image"][2]["dataSet"]["userSettings"]["brightness"],
            121
        );
        assert_eq!(
            edit(&mut profile, settings, "brightness", &140.into()),
            Some((2, 4))
        );
        assert_eq!(profile["image"][4]["dataSet"]["brightness"], 140);
        assert_eq!(
            profile["image"][4]["dataSet"]["userSettings"]["brightness"],
            90
        );
        edit(&mut profile, settings, "brightness", &150.into()).unwrap();
        assert_eq!(
            profile["image"][4]["dataSet"]["userSettings"]["brightness"],
            150
        );
        assert_eq!(profile["image"][2]["dataSet"]["brightness"], 90);
        assert_eq!(defaults["image"][2]["dataSet"]["brightness"], 121);
        select(&mut profile, defaults, settings, 1);
        select(&mut profile, defaults, settings, 4);
        assert_eq!(profile["image"][4]["dataSet"]["brightness"], 150);
    }

    #[test]
    fn auto_checkbox_uses_user_balance_and_updates_custom_baseline_immediately() {
        let spec = source_spec(3594);
        let defaults = &spec["profile"];
        let settings = spec["camera_image_settings"].as_array().unwrap();
        let mut profile = defaults.clone();
        profile["image"][0]["dataSet"]["whiteBalance"]["value"] = 6100.into();
        edit(&mut profile, settings, "whiteBalance/isAuto", &false.into()).unwrap();
        let custom = &profile["image"][4]["dataSet"];
        assert_eq!(
            custom["whiteBalance"],
            serde_json::json!({"isAuto":false,"value":4000})
        );
        assert_eq!(
            custom["userSettings"]["whiteBalance"],
            custom["whiteBalance"]
        );
    }

    #[test]
    fn camera_family_presets_preserve_first_edit_baseline_and_last_custom_value() {
        for product_id in [3595, 3596] {
            let spec = source_spec(product_id);
            let defaults = &spec["profile"];
            let settings = spec["camera_image_settings"].as_array().unwrap();
            let mut profile = defaults.clone();
            assert!(select(&mut profile, defaults, settings, 2));
            let previous_user = profile["image"][2]["dataSet"]["userSettings"].clone();
            assert_eq!(profile["image"][2]["dataSet"]["brightness"], 90);
            edit(&mut profile, settings, "brightness", &141.into()).unwrap();
            assert_eq!(
                profile["image"][4]["dataSet"]["userSettings"]["brightness"],
                90
            );
            assert_eq!(
                profile["image"][2]["dataSet"]["userSettings"],
                previous_user
            );
            edit(&mut profile, settings, "brightness", &151.into()).unwrap();
            assert_eq!(
                profile["image"][4]["dataSet"]["userSettings"]["brightness"],
                151
            );
            assert!(select(&mut profile, defaults, settings, 1));
            assert!(select(&mut profile, defaults, settings, 4));
            assert_eq!(profile["image"][4]["dataSet"]["brightness"], 151);
            assert!(select(&mut profile, defaults, settings, 0));
            profile["image"][0]["dataSet"]["whiteBalance"]["value"] = 6100.into();
            edit(&mut profile, settings, "whiteBalance/isAuto", &false.into()).unwrap();
            let custom = &profile["image"][4]["dataSet"];
            assert_eq!(
                custom["whiteBalance"],
                serde_json::json!({"isAuto":false,"value":4000})
            );
            assert_eq!(
                custom["userSettings"]["whiteBalance"],
                custom["whiteBalance"]
            );
        }
    }
}
