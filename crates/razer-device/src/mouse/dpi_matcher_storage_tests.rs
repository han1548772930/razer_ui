use super::*;

fn record(names: &[Value]) -> Value {
    json!({
        "version":7,
        "activeProfile":"current",
        "profiles":[{"guid":"current","dpiStages":{
            "active":2,"enable":true,
            "stages":[{"x":800,"y":900},{"x":1600,"y":1700}]
        }}],
        "sensitivityMatcher":{"profiles":names.iter().enumerate().map(|(ix,name)|
            json!({"guid":format!("previous-{ix}"),"name":name})
        ).collect::<Vec<_>>()}
    })
}

#[test]
fn dpi_matcher_confirm_uses_first_free_string_candidate_and_strict_equality() {
    // Current 226 main module55680 S/AM and 190 module76665 d/W$ compare
    // names with === and test the base before suffixes 1..99.
    for (names, base, expected) in [
        (
            vec![json!("Mouse"), json!("Mouse 1"), json!("Mouse 3")],
            "Mouse",
            "Mouse 2",
        ),
        (
            vec![json!("mouse"), json!({"en":"Mouse"})],
            "Mouse",
            "Mouse",
        ),
        (vec![json!("Keymap")], "Keymap", "Keymap 1"),
        (
            vec![json!("Keymap 1"), json!("Keymap 3")],
            "Keymap",
            "Keymap 2",
        ),
        (vec![json!("")], "", " 1"),
    ] {
        let source = record(&names);
        let before = source.clone();
        let result = confirm(&source, 1450., "measured", json!(base)).unwrap();
        assert_eq!(
            result.data["sensitivityMatcher"]["profiles"][names.len()]["name"],
            expected
        );
        assert_eq!(result.response.selected(), Some("measured"));
        assert_eq!(
            result.dpi_stages.as_ref().unwrap()["stages"][1],
            json!({"x":1450,"y":1450,"independent":false,"visible":true})
        );
        assert_eq!(
            result.data["profiles"][0]["dpiStages"]["stages"][0],
            json!({"x":800,"y":900})
        );
        assert_eq!(result.data["version"], 8);
        assert_eq!(source, before);
    }
}

#[test]
fn dpi_matcher_confirm_retains_locale_object_names_and_duplicate_final_suffix() {
    let localized = json!({"en":"Mouse","zh-cn":"Source mouse"});
    let source = record(&[localized.clone(), localized.clone()]);
    let result = confirm(&source, 1200., "measured", localized.clone()).unwrap();
    assert_eq!(
        result.data["sensitivityMatcher"]["profiles"][2]["name"],
        localized
    );

    for base in ["Mouse", "Keymap"] {
        let mut names: Vec<_> = (1..=100)
            .map(|suffix| json!(format!("{base} {suffix}")))
            .collect();
        names.push(json!(base));
        let result = confirm(&record(&names), 1200., "measured", json!(base)).unwrap();
        // Both helpers return the last candidate even when it still collides.
        assert_eq!(
            result.data["sensitivityMatcher"]["profiles"][101]["name"],
            format!("{base} 100")
        );
    }
}

#[test]
fn dpi_matcher_confirm_rejects_invalid_identity_or_measurement_without_mutation() {
    let source = record(&[json!("Mouse")]);
    let before = source.clone();
    for (dpi, guid, name) in [
        (0., "measured", json!("Mouse")),
        (f64::INFINITY, "measured", json!("Mouse")),
        (1200., "", json!("Mouse")),
        (1200., "previous-0", json!("Mouse")),
        (1200., "measured", Value::Null),
    ] {
        assert!(confirm(&source, dpi, guid, name).is_err());
        assert_eq!(source, before);
    }
}

#[test]
fn dpi_matcher_confirm_result_enters_the_real_unsigned_stage_parser() {
    let mut source = record(&[]);
    source["profiles"][0]["dpiStages"]["stages"] = json!([
        {"x":800,"y":900,"independent":true,"visible":true},
        {"x":1600,"y":1700,"independent":true,"visible":true}
    ]);
    let result = confirm(&source, 400., "measured", json!("Mouse")).unwrap();
    let stages = &result.dpi_stages.as_ref().unwrap()["stages"];
    let parsed: Vec<crate::mouse_dpi_stages::DpiStage> =
        serde_json::from_value(stages.clone()).unwrap();
    assert_eq!((parsed[0].x, parsed[0].y), (800, 900));
    assert_eq!((parsed[1].x, parsed[1].y), (400, 400));
    assert!(!parsed[1].independent);
    assert!(parsed[1].visible);
}
