use super::*;
use std::collections::BTreeSet;

#[test]
fn keyboard_local_recipe_schema_versions_follow_actual_deployed_initializers() {
    for product in [515, 600, 678, 679, 688, 708, 719, 747] {
        assert_eq!(schema_version(product), Some(14));
    }
    for product in [691, 721, 756] {
        assert_eq!(schema_version(product), Some(13));
    }
    for product in [739, 752] {
        assert_eq!(schema_version(product), Some(12));
    }
    assert_eq!(schema_version(0), None);
}

#[test]
fn keyboard_local_recipe_selects_real_edition_values_and_only_the_oled_fallback() {
    for (product, expected) in [
        (515, 100),
        (585, 30),
        (600, 30),
        (691, 50),
        (724, 25),
        (742, 70),
    ] {
        let recipe = initial_template(product, 0, "Desktop").unwrap();
        assert_eq!(recipe.device.product_id, product);
        assert_eq!(
            recipe.profiles[0].brightness.as_ref().unwrap().value,
            expected
        );
        assert_eq!(recipe.profiles[0].name, "Desktop-Default");
    }
    assert!(initial_template(515, 255, "Desktop").is_err());
    assert!(initial_template(0, 0, "Desktop").is_err());
    assert_eq!(
        initial_template(691, 255, "Desktop").unwrap().profiles[0]
            .brightness
            .as_ref()
            .unwrap()
            .value,
        50
    );
    assert_eq!(
        initial_template(720, 0, "Desktop").unwrap().profiles[0].continuous_rapid_trigger,
        Some(false)
    );
    assert_eq!(
        initial_template(720, 130, "Desktop").unwrap().profiles[0].continuous_rapid_trigger,
        Some(true)
    );
}

#[test]
fn keyboard_local_recipe_preserves_global_lighting_and_all_application_profiles() {
    let recipe = initial_template(708, 255, "Desktop").unwrap();
    assert_eq!(recipe.profiles.len(), 13);
    assert_eq!(recipe.device.brightness.as_ref().unwrap().value, 50);
    assert_eq!(recipe.profiles[0].name, "Default App Profile");
    assert_eq!(recipe.profiles[12].name, "Davinci Resolve");
    assert!(
        recipe
            .profiles
            .iter()
            .all(|profile| profile.brightness.is_none())
    );
    assert_eq!(
        recipe
            .profiles
            .iter()
            .map(|profile| &profile.guid)
            .collect::<BTreeSet<_>>()
            .len(),
        13
    );
    let dial = recipe.device.command_dial.as_ref().unwrap();
    assert_eq!(dial.modes.len(), 8);
    assert_eq!(dial.current_mode_uid, dial.modes[0].uid);
    assert_eq!(dial.modes[0].name, "KEYBOARD_BRIGHTNESS");
}

#[test]
fn keyboard_local_recipe_keeps_keymaps_and_encoded_actuation_numbers() {
    let recipe = initial_template(747, 0, "Desktop").unwrap();
    let profile = &recipe.profiles[0];
    let keymaps = profile.keymaps.as_ref().unwrap();
    assert_eq!(keymaps.len(), 8);
    assert_eq!(
        profile.active_keymap_id.as_deref(),
        Some(keymaps[0].guid.as_str())
    );
    let stages = keymaps[1].mappings[0].mapping.as_ref().unwrap();
    let ActuationValue::Named(point) = stages[0].actuation_point.as_ref().unwrap() else {
        panic!("Source indexed point")
    };
    assert_eq!(point.make, SourceNumber::Unsigned(19656));
    assert_eq!(point.release, SourceNumber::Unsigned(256));
    assert_eq!(keymaps[3].mappings.len(), 5);
    assert!(profile.gaming_mode.is_none());
}

#[test]
fn keyboard_local_recipe_retains_oled_media_lists_and_dial_identity() {
    let recipe = initial_template(691, 0, "Desktop").unwrap();
    let oled = recipe.device.oled_settings.as_ref().unwrap();
    assert_eq!(oled.brightness, 75);
    assert_eq!(oled.home_screen_settings.animation.list.len(), 6);
    assert_eq!(oled.home_screen_settings.image.list.len(), 10);
    let dial = recipe.profiles[0].command_dial.as_ref().unwrap();
    assert_eq!(dial.modes.len(), 9);
    assert_eq!(dial.current_mode_uid, dial.modes[0].uid);
    assert_eq!(
        dial.modes
            .iter()
            .map(|mode| &mode.uid)
            .collect::<BTreeSet<_>>()
            .len(),
        9
    );
    let another = initial_template(691, 0, "Desktop").unwrap();
    assert_ne!(recipe.profiles[0].guid, another.profiles[0].guid);
}

#[test]
fn keyboard_local_recipe_keeps_explicit_numeric_forms_and_extensions_at_the_file_boundary() {
    let source = serde_json::json!({"0":19656,"1":256,"future":{"mode":7}});
    let point: ActuationValue = serde_json::from_value(source.clone()).unwrap();
    assert_eq!(serde_json::to_value(point).unwrap(), source);
    let curve = serde_json::json!({"x":"87","y":86});
    let point: SensitivityPoint = serde_json::from_value(curve.clone()).unwrap();
    assert_eq!(serde_json::to_value(point).unwrap(), curve);
    let invalid = serde_json::json!({"selectedEffectId":3,"selectedEffectSetting":null});
    assert!(serde_json::from_value::<super::super::QuickEffects>(invalid).is_err());
}

#[test]
#[ignore = "workspace maintenance compares complete Rust recipes with current AST-derived originals"]
fn keyboard_local_recipe_complete_original_comparison() {
    let requests: Vec<(u32, u8)> = serde_json::from_str(
        &std::env::var("RAZER_KEYBOARD_TEMPLATE_REQUESTS")
            .expect("maintenance supplies current product editions"),
    )
    .unwrap();
    assert!(!requests.is_empty());
    let ids = RecipeIds((0..9).map(|index| format!("dial-{index}")).collect());
    for (product, edition) in requests {
        let device = templates::device(product, &ids).expect("current device template");
        let profiles = if product == 708 {
            templates::applications()
        } else {
            vec![templates::profile(product, edition, &ids).expect("current edition template")]
        };
        println!(
            "KEYBOARD_RECIPE_COMPARE {}",
            serde_json::json!({"product_id":product,"edition":edition,"device":device,"profiles":profiles})
        );
    }
}
