use super::*;
use serde_json::json;

fn document(defaults: Value, active: Value) -> Value {
    json!({"activeProfile":"p","defaultMappings":{"appEngine":{"mappings":defaults}},"profiles":[{"guid":"p","appEngine":{"mappings":active}}]})
}

#[test]
fn identity_profile_rows_are_removed_before_default_merge() {
    use crate::mapping_native_lookup::{Lookup, Modes, ResolvedOutput};
    let key = json!({"type":"keyboard","scancode":30,"flag":0});
    let defaults = json!([{"input":key,"output":{"type":"disabled"}}]);
    let active = json!([{"input":key,"output":key}]);
    let tables = device_tables(&document(defaults, active), None).unwrap();
    let mut lookup = Lookup::new(tables);
    let found = lookup
        .resolve(
            parse_input(&key).unwrap(),
            Modes {
                mapping_enabled: true,
                hypershift: false,
                gaming: false,
                otfs: false,
                global_shortcuts_enabled: true,
            },
        )
        .unwrap();
    assert_eq!(found.output, ResolvedOutput::Mapped(Output::Disabled));
    let key = json!({"type":"keyboard","scancode":30,"flag":0,"snaptapId":7});
    let tables = device_tables(
        &document(
            json!([]),
            json!([{"input":key,"output":{"type":"keyboard","scancode":30,"flag":0}}]),
        ),
        None,
    )
    .unwrap();
    assert_eq!(tables.hook_inputs().len(), 1);
}

#[test]
fn phased_edge_and_duplicate_identity_are_validated_before_publication() {
    let key = json!({"type":"keyboard","scancode":30,"flag":0});
    let row = json!({"input":key,"output":{"type":"phasedMacro","guid":"m","flag":1}});
    assert!(device_tables(&document(json!([]), json!([row])), None).is_err());
    let row = json!({"input":key,"output":key});
    assert!(device_tables(&document(json!([]), json!([row, row])), None).is_err());
}

#[test]
fn macro_flags_preserve_source_repeat_defaults() {
    for (flag, repeat) in [(0, 0), (1, 0), (2, 0), (3, 0), (4, 1)] {
        assert_eq!(
            parse_output(&json!({"type":"macro","guid":"m","flag":flag})).unwrap(),
            Output::Macro {
                guid: "m".into(),
                flag,
                repeat
            }
        );
    }
    assert_eq!(
        parse_output(&json!({"type":"macro","guid":"m","flag":2,"repeat":9})).unwrap(),
        Output::Macro {
            guid: "m".into(),
            flag: 2,
            repeat: 0
        }
    );
    assert!(parse_output(&json!({"type":"macro","guid":"m","flag":5})).is_err());
    assert!(parse_output(&json!({"type":"macro","guid":"","flag":0})).is_err());
}

#[test]
fn turbo_and_phased_defaults_are_distinct() {
    assert_eq!(
        parse_output(&json!({"type":"turbo","guid":"m","flag":0})).unwrap(),
        Output::Turbo {
            guid: "m".into(),
            flag: 0,
            repeat: 0,
            delay_ms: 1000
        }
    );
    assert_eq!(
        parse_output(&json!({"type":"phasedMacro","guid":"m","flag":7})).unwrap(),
        Output::PhasedMacro {
            guid: "m".into(),
            flag: 7,
            delay_ms: 30
        }
    );
    assert!(parse_output(&json!({"type":"turbo","guid":"m","flag":2})).is_err());
}

#[test]
fn nested_context_preserves_notification_fallback() {
    let delay = json!({"type":"delay","ms":7});
    assert!(matches!(
        parse_output(&delay).unwrap(),
        Output::Forwarded { .. }
    ));
    let result =
        parse_output(&json!({"type":"multi","outputs":[delay,{"type":"disabled"}]})).unwrap();
    let Output::Multi(rows) = result else {
        panic!("multi expected")
    };
    assert_eq!(rows[0], Output::Event(Event::Delay { milliseconds: 7 }));
    assert!(matches!(&rows[1],Output::Forwarded {kind,..} if kind=="disabled"));
    let raw = json!({"type":"profile","guid":"p","flag":0});
    let Output::Forwarded { kind, payload } = parse_output(&raw).unwrap() else {
        panic!("notification expected")
    };
    assert_eq!(kind, "profile");
    assert_eq!(
        serde_json::from_str::<Value>(payload.as_json()).unwrap(),
        raw
    );
}

#[test]
fn mode_and_display_use_native_integer_widths() {
    for (scope, expected) in [
        ("global", ModeScope::Global),
        ("interdevice", ModeScope::Interdevice),
        ("local", ModeScope::Local),
    ] {
        assert!(
            matches!(parse_output(&json!({"type":"mode","scope":scope,"flag":65537})).unwrap(),
            Output::Mode {scope,flag:1,..} if scope==expected)
        );
    }
    assert_eq!(
        parse_output(&json!({"type":"display","id":"brightness","value":257})).unwrap(),
        Output::Display(DisplayAction::Brightness(1))
    );
    assert_eq!(
        parse_output(&json!({"type":"display","id":"driverBrightnessStop"})).unwrap(),
        Output::Display(DisplayAction::DriverBrightnessStop)
    );
    assert!(parse_output(&json!({"type":"mode","scope":"device","flag":0})).is_err());
}

#[test]
fn composite_validation_rejects_ambiguous_or_empty_configuration() {
    for value in [
        json!({"type":"multi","outputs":[]}),
        json!({"type":"cascade","cascade":[]}),
        json!({"type":"dpad","dpad":[]}),
        json!({"type":"triggers","outputs":{}}),
    ] {
        assert!(parse_output(&value).is_err());
    }
    assert!(
        parse_output(&json!({"type":"triggers","outputs":{
            "01":{"actuation":1,"mapping":{"type":"disabled"}},
            "1":{"actuation":2,"mapping":{"type":"disabled"}}
        }}))
        .is_err()
    );
    assert!(parse_output(&json!({"type":"rapidTrigger","actuation":10,"makeSensitivity":65536,"breakSensitivity":1,"mapping":{"type":"disabled"}})).is_err());
    let result = parse_output(
        &json!({"type":"triggers","outputs":{"1":{"actuation":10,"mapping":{"type":"disabled"}}}}),
    )
    .unwrap();
    let Output::Triggers(rows) = result else {
        panic!("triggers expected")
    };
    assert!(matches!(rows[&1].output, Output::Forwarded { .. }));
}
