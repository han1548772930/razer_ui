use super::*;
use crate::{mapping_native_action::parse_output, mapping_native_lookup::parse_input};

fn register(state: &mut ModeState, guid: &str, product: u16) -> Uuid {
    let caller = Uuid::new_v4();
    state
        .register_device(
            caller,
            DeviceInfo {
                guid: guid.into(),
                vendor_id: 5426,
                product_id: product,
                edition_id: 0,
                manufacturer_name: "Razer".into(),
                product_name: "Keyboard".into(),
                serial_number: "serial".into(),
                container_id: "container".into(),
            },
        )
        .unwrap();
    caller
}

#[test]
fn leases_span_devices_and_last_removal_releases_mode() {
    let mut state = ModeState::default();
    let a = register(&mut state, "a", 515);
    let b = register(&mut state, "b", 679);
    assert!(state.apply_hypershift(a, 0, 10).unwrap());
    assert!(!state.apply_hypershift(a, 0, 11).unwrap());
    assert!(!state.apply_hypershift(b, 0, 12).unwrap());
    assert!(!state.apply_hypershift(a, 1, 13).unwrap());
    state.remove_device(a, 14).unwrap();
    assert!(state.hypershift());
    assert!(state.apply_hypershift(b, 1, 15).unwrap());
    assert!(!state.apply_hypershift(b, 1, 16).unwrap());
    assert_eq!(
        state.take_events(),
        vec![
            Event::Hypershift {
                enabled: true,
                time_tick: 10
            },
            Event::Hypershift {
                enabled: false,
                time_tick: 15
            }
        ]
    );
    state.apply_otfs(b, 0, 17).unwrap();
    state.remove_device(b, 18).unwrap();
    assert!(!state.otfs());
    assert_eq!(state.global_mode_json(), json!({}));
    assert!(state.apply_hypershift(a, 0, 19).is_err());
}

#[test]
fn mode_action_retains_first_value_but_notifies_each_operation() {
    let mut state = ModeState::default();
    let caller = register(&mut state, "a", 515);
    let key = parse_input(&json!({"type":"keyboard","scancode":30,"flag":0})).unwrap();
    let output = parse_output(&json!({"type":"mode","scope":"global","flag":0})).unwrap();
    state.apply_mode_action(caller, &key, &output, 100).unwrap();
    state.apply_mode_action(caller, &key, &output, 200).unwrap();
    assert_eq!(state.global_mode_json()["globalmode"][0]["timeTick"], "100");
    let events = state.take_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].kind(), 2);
    assert_eq!(events[1].time_tick(), 200);
    assert_eq!(
        serde_json::from_str::<Value>(events[0].to_native_json()["input"].as_str().unwrap())
            .unwrap(),
        json!({"type":"keyboard","scancode":30,"flag":0,"extraInformation":0,"modifiers":0})
    );
    let release = parse_output(&json!({"type":"mode","scope":"global","flag":1})).unwrap();
    state
        .apply_mode_action(caller, &key.released(), &release, 300)
        .unwrap();
    assert_eq!(state.global_mode_json(), json!({}));
}

#[test]
fn mode_keys_include_layer_and_device_and_queries_use_distinct_shapes() {
    let mut state = ModeState::default();
    let a = register(&mut state, "a", 515);
    let b = register(&mut state, "b", 679);
    let key = parse_input(&json!({"type":"keyboard","scancode":30,"flag":0})).unwrap();
    let output = parse_output(&json!({"type":"mode","scope":"local","flag":0})).unwrap();
    state.apply_mode_action(a, &key, &output, 1).unwrap();
    state.apply_hypershift(b, 0, 2).unwrap();
    state.apply_mode_action(a, &key, &output, 3).unwrap();
    state.set_game_mode(a, true, 4).unwrap();
    state.set_game_mode(a, true, 5).unwrap();
    assert_eq!(state.global_mode_json(), json!({"hypershift":true}));
    let filtered = state.device_mode_json(Filter {
        product: Some(515),
        ..Default::default()
    });
    assert_eq!(filtered["devicemode"].as_array().unwrap().len(), 1);
    assert_eq!(
        filtered["devicemode"][0]["actions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(filtered.get("hypershift").is_none());
    let events = state.take_events();
    assert_eq!(events.len(), 5);
    assert_eq!(events[3].kind(), 1);
    assert!(events[3].to_native_json()["deviceInfo"].is_string());
    assert_eq!(events[0].kind(), 0);
}

#[test]
fn replacing_registration_rejects_late_owner_and_queue_overflow() {
    let mut state = ModeState::default();
    let old = register(&mut state, "a", 515);
    state.remove_device(old, 1).unwrap();
    let current = register(&mut state, "a", 515);
    assert!(state.set_game_mode(old, true, 2).is_err());
    assert!(!state.gaming(current).unwrap());
    for tick in 0..1024 {
        state.set_game_mode(current, false, tick).unwrap();
    }
    assert!(state.set_game_mode(current, true, 2000).is_err());
    assert!(!state.gaming(current).unwrap());
    assert_eq!(state.take_events().len(), 1024);
    state.set_game_mode(current, true, 2001).unwrap();
    assert!(state.gaming(current).unwrap());
}

#[test]
fn input_serialization_and_stateless_order_use_source_fields() {
    let key=parse_input(&json!({"type":"keyboard","scancode":30,"flag":3,"unitId":4,"delayInMs":9,"interrupts":[{"type":"keyboard","scancode":2,"flag":0}]})).unwrap();
    assert_eq!(key.stateless_id(), "t21s30f2u4");
    assert_eq!(key.stateless_id(), key.released().stateless_id());
    assert_eq!(
        key.to_native_json(),
        json!({"type":"keyboard","scancode":30,"flag":3,"unitId":4,"delayInMs":9,"extraInformation":0,"modifiers":0})
    );
    let mouse = parse_input(&json!({"type":"mouse","id":1})).unwrap();
    assert_eq!(mouse.stateless_id(), "t27i3");
    assert_eq!(mouse.released().stateless_id(), "t27i3");
    let ext = parse_input(&json!({"type":"external","key":3,"flag":0,"modifiers":32})).unwrap();
    assert_eq!(ext.lookup_modifiers(), 0);
    assert_eq!(ext.to_native_json()["modifiers"], 32);
}
