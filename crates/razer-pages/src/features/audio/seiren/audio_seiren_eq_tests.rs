use super::*;

#[core::prelude::v1::test]
fn seiren_eq_dynamic_ids_add_restore_and_disable_removed_band() {
    let mut model = Model::initial();
    let preset = model.active_mut().unwrap();
    preset.selected = 3;
    let removed = preset.remove_selected().unwrap();
    assert_eq!(removed.id, 3);
    assert_eq!(removed.payload()["bandStatus"], 0);
    assert_eq!(preset.selected, 4);
    let writes = preset.add().unwrap();
    assert_eq!(preset.selected, 3);
    assert_eq!(writes.len(), 7);
    let restored = &preset.bands[3];
    assert_eq!(restored.frequency, 1500.);
    assert_eq!(restored.gain, 0.);
    assert_eq!(restored.q, 2.);
    preset.add().unwrap();
    assert_eq!(preset.selected, 7);
    assert_eq!(preset.bands.len(), 8);
    assert!(preset.add().is_none());
    let value = serde_json::to_value(&model).unwrap();
    assert_eq!(Model::restore(&value).unwrap(), model);
}
#[core::prelude::v1::test]
fn seiren_eq_flat_preserves_highpass_and_reset_writes_inactive_slot() {
    let mut model = Model::initial();
    let preset = model.active_mut().unwrap();
    let highpass = preset.high_pass.clone();
    let selected = preset.selected;
    let writes = preset.flatten();
    assert_eq!(writes.len(), 8);
    assert_eq!(writes[7].status, Some(0));
    assert_eq!(preset.high_pass, highpass);
    assert_eq!(preset.selected, selected);
    assert!(!preset.is_default());
    let reset = preset.reset();
    assert_eq!(reset.len(), 8);
    assert_eq!(preset.bands.len(), 7);
    assert!(preset.is_default());
}
#[core::prelude::v1::test]
fn seiren_eq_drag_filter_constraints_and_logarithmic_q_handles() {
    let mut band = Model::initial().active().unwrap().bands[1].clone();
    let next = drag_band(&band, 0.5, 0.2);
    assert_eq!(next.frequency, 632.);
    assert_eq!(next.gain, 7.2);
    assert_eq!(next.q, band.q);
    band.update("type", &json!("highpass"));
    assert_eq!(band.gain, 0.);
    band.update("gain", &json!(10.));
    assert_eq!(band.gain, 0.);
    let next = drag_band(&band, -1., 2.);
    assert_eq!(next.frequency, 20.);
    assert_eq!(next.q, 0.3);
    assert_eq!(next.gain, 0.);
    assert_eq!(q_from_handle_distance(20.), 15.);
    assert_eq!(q_from_handle_distance(80.), 0.3);
    assert!((q_from_handle_distance(q_handle_distance(2.)) - 2.).abs() < 1e-10);
}
#[core::prelude::v1::test]
fn seiren_eq_gain_interpolation_preserves_source_display_offset() {
    for percent in [0., 14., 40., 50., 60., 70., 80., 84., 87., 100.] {
        assert!((gain_percent(gain_db(percent)) - percent).abs() < 1e-10);
    }
    assert_eq!(gain_db(0.), 4.);
    assert_eq!(gain_db(100.), 40.);
}
#[core::prelude::v1::test]
fn seiren_eq_preset_payload_uses_stable_band_ids_and_current_effects() {
    let model = Model::initial();
    let effects = json!({"vocal_bass":{"isEnabled":false,"value":27},"vocal_exciter":{"isEnabled":true,"value":63}});
    let payload = model.active().unwrap().payload(&effects);
    assert_eq!(payload["eqName"], "podcast");
    assert_eq!(payload["parametricEqSettings"][0]["filterType"], 3);
    assert_eq!(payload["parametricEqSettings"][6]["band"], 6);
    assert_eq!(payload["vocalBass"]["value"], 27);
    assert_eq!(payload["vocalExciter"]["value"], 63);
}
