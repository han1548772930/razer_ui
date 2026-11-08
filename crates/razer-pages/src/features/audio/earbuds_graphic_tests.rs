use super::State;
use razer_device::earbuds_auto_pause::Report;
use serde_json::json;

#[test]
fn independent_observations_preserve_unknown_zero_and_disconnected() {
    let mut state = State::default();
    assert_eq!(state.level(0), None);
    let report: Report = serde_json::from_value(json!({
        "product_id":2657,"operation":"Read","connected":true,
        "left_battery":255,"right_battery":0,"observation":"Unobserved","write":"Skipped"
    }))
    .unwrap();
    state.observe(&report);
    assert_eq!(state.level(0), Some(-1));
    assert_eq!(state.level(1), Some(0));
    let mut partial = report.clone();
    partial.left_battery = None;
    partial.right_battery = Some(78);
    state.observe(&partial);
    assert_eq!(state.level(0), None);
    assert_eq!(state.level(1), Some(78));
    partial.connected = Some(false);
    state.observe(&partial);
    assert_eq!(state.level(0), None);
    assert_eq!(state.level(1), None);
}
