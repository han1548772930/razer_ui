use super::*;
use serde_json::json;

const EXISTING: &str = "00000000-0000-4000-8000-000000000001";
const FIRST: &str = "00000000-0000-4000-8000-000000000002";
const SECOND: &str = "00000000-0000-4000-8000-000000000003";

fn existing() -> Vec<Value> {
    vec![json!({"guid":EXISTING,"name":"Work","localExtension":{"keep":7}})]
}

fn row(profile: Value) -> Value {
    json!({"profile":profile,"listItems":[],"hasWarning":false})
}

#[test]
fn import_1398_keeps_extensions_and_uses_a_shallow_current_default_merge() {
    let existing = existing();
    let settings = json!({"version":7,"activeProfile":EXISTING,"deviceExtension":{"keep":42}});
    let selected = [row(json!({
        "guid":"file-guid","name":"Imported","bassBoost":{"value":81},
        "applicationPresets":[{"app":"game.exe","preset":"custom"}],
        "extension":{"opaque":[1,null,{"nested":true}]}
    }))];
    let before = (existing.clone(), settings.clone(), selected.clone());
    let prepared =
        prepare_global_import_1398(&selected, &existing, Some(&settings), || FIRST.into()).unwrap();
    assert_eq!(prepared.profiles[0], existing[0]);
    assert_eq!(prepared.profiles[1]["guid"], FIRST);
    assert_eq!(prepared.profiles[1]["name"], "Imported");
    assert_eq!(prepared.profiles[1]["bassBoost"], json!({"value":81}));
    assert_eq!(
        prepared.profiles[1]["applicationPresets"],
        selected[0]["profile"]["applicationPresets"]
    );
    assert_eq!(
        prepared.profiles[1]["extension"],
        selected[0]["profile"]["extension"]
    );
    let fixture: Value = serde_json::from_str(include_str!(
        "../../testdata/re/blackshark-reset-current-source.json"
    ))
    .unwrap();
    for (field, value) in fixture["profile_defaults"].as_object().unwrap() {
        if !matches!(field.as_str(), "bassBoost" | "applicationPresets") {
            assert_eq!(&prepared.profiles[1][field], value, "{field}");
        }
    }
    assert_eq!(
        prepared.device_settings,
        json!({"version":8,"activeProfile":EXISTING,"deviceExtension":{"keep":42}})
    );
    assert_eq!((existing, settings, selected), before);
}

#[test]
fn import_1398_preserves_suffixes_case_and_batch_order_without_selecting_a_profile() {
    let existing = existing();
    let rows = [
        row(json!({"name":"Work"})),
        row(json!({"name":"Work"})),
        row(json!({"name":"work"})),
    ];
    let mut guids = [FIRST, SECOND, "00000000-0000-4000-8000-000000000004"].into_iter();
    let prepared =
        prepare_global_import_1398(&rows, &existing, None, || guids.next().unwrap().into())
            .unwrap();
    assert_eq!(
        prepared
            .profiles
            .iter()
            .map(|profile| profile["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Work", "Work (1)", "Work (2)", "work"]
    );
    assert_eq!(prepared.device_settings, json!({"version":1}));
    let rows = [
        row(json!({"name":"Work (1)"})),
        row(json!({"name":"Work (1)"})),
    ];
    let mut guids = [FIRST, SECOND].into_iter();
    let prepared =
        prepare_global_import_1398(&rows, &existing, None, || guids.next().unwrap().into())
            .unwrap();
    assert_eq!(prepared.profiles[1]["name"], "Work (1)");
    assert_eq!(prepared.profiles[2]["name"], "Work (1) (1)");
}

#[test]
fn import_1398_rejects_the_current_sources_unavailable_quick_effects_branch() {
    let existing = existing();
    for quick_effects in [
        json!({"selectedEffectId":3}),
        json!({}),
        json!([]),
        json!(1),
        json!(true),
        json!("lighting"),
    ] {
        let rows = [
            row(json!({"name":"Safe"})),
            row(json!({"name":"Foreign","quickEffects":quick_effects})),
        ];
        assert!(
            prepare_global_import_1398(&rows, &existing, None, || FIRST.into())
                .unwrap_err()
                .to_string()
                .contains("QUICK_EFFECTS")
        );
        assert_eq!(existing, self::existing());
    }
    for quick_effects in [Value::Null, json!(false), json!(0), json!("")] {
        let prepared = prepare_global_import_1398(
            &[row(json!({"name":"Imported","quickEffects":quick_effects}))],
            &existing,
            None,
            || FIRST.into(),
        )
        .unwrap();
        assert_eq!(prepared.profiles[1]["quickEffects"], quick_effects);
    }
}

#[test]
fn import_1398_checks_versions_rows_and_fresh_identity_before_producing_a_candidate() {
    let existing = existing();
    let rows = [row(json!({"name":"Imported"}))];
    for settings in [
        json!([]),
        json!({"version":null}),
        json!({"version":"7"}),
        json!({"version":1.5}),
        json!({"version":1u64 << 53}),
    ] {
        assert!(
            prepare_global_import_1398(&rows, &existing, Some(&settings), || FIRST.into()).is_err()
        );
    }
    let wrap = prepare_global_import_1398(
        &rows,
        &existing,
        Some(&json!({"version":(1u64 << 53)-1})),
        || FIRST.into(),
    )
    .unwrap();
    assert_eq!(wrap.device_settings["version"], 0);
    for guid in ["not-a-guid", EXISTING] {
        assert!(prepare_global_import_1398(&rows, &existing, None, || guid.into()).is_err());
    }
    let duplicate = [row(json!({"name":"One"})), row(json!({"name":"Two"}))];
    assert!(prepare_global_import_1398(&duplicate, &existing, None, || FIRST.into()).is_err());
    for value in [
        json!({"profile":{"name":"Missing list"}}),
        row(json!({})),
        row(Value::Null),
        json!({"profile":{"name":"Bad list"},"listItems":[null]}),
    ] {
        assert!(prepare_global_import_1398(&[value], &existing, None, || FIRST.into()).is_err());
    }
    assert!(prepare_global_import_1398(&[], &existing, None, || FIRST.into()).is_err());
}
