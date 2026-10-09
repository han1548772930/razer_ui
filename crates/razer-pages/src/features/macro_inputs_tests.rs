//! Compiled by cargo check; not executed under the source-audit policy.
use super::for_device;

#[test]
fn physical_catalog_retains_disabled_inputs_without_default_mapping_extras() {
    let device = razer_model::demo::registered_preview(182).unwrap();
    let layout = for_device(&device).unwrap();
    assert!(layout.mouse_diagram());
    assert_eq!(layout.inputs().len(), 8);
    let left_click = layout
        .inputs()
        .iter()
        .find(|input| input.id() == "LeftClick")
        .unwrap();
    assert!(!left_click.enabled());
    assert!(layout.inputs().iter().any(|input| input.enabled()));
    assert!(layout.inputs().iter().all(|input| input.shape().is_none()));
}

#[test]
fn prepared_layout_defaults_to_us_but_unknown_layout_never_uses_a_family_fallback() {
    let mut device = razer_model::demo::registered_preview(653).unwrap();
    device.layout_id = 0;
    let default = for_device(&device).unwrap();
    device.layout_id = 1;
    let us = for_device(&device).unwrap();
    assert!(!default.mouse_diagram());
    assert!(!default.inputs().is_empty());
    assert_eq!(default.inputs().len(), us.inputs().len());
    for (default, us) in default.inputs().iter().zip(us.inputs()) {
        assert_eq!(default.id(), us.id());
        assert_eq!(default.enabled(), us.enabled());
        assert_eq!(default.bounds(), us.bounds());
        assert!(std::ptr::eq(default.shape().unwrap(), us.shape().unwrap()));
    }
    device.layout_id = u32::MAX;
    assert!(for_device(&device).is_none());
}
