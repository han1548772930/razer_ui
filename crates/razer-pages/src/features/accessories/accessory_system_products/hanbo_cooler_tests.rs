use super::*;

fn advanced(channel: CoolerChannel) -> HanboCooler {
    let mut state = HanboCooler::default();
    state.set_mode(channel, ThermalMode::Advanced);
    state
}

fn press(state: &mut HanboCooler, channel: CoolerChannel, index: usize) -> ThermalCommit {
    let geometry = ChartGeometry::new(channel);
    let y = geometry.y(state.curve_values(channel)[index]);
    state.press_curve(channel, geometry.x(index), y).unwrap()
}

#[::core::prelude::v1::test]
fn hanbo_chart_preserves_the_source_axis_anchor_and_channel_grid_extents() {
    let fans = ChartGeometry::new(CoolerChannel::Fans);
    let pump = ChartGeometry::new(CoolerChannel::Pump);
    assert_eq!(fans.right, 510.);
    assert_eq!(pump.right, 512.);
    assert_eq!(fans.top, 9.);
    assert_eq!(pump.top, 9.);
    // `translate(0,Fg-c/2);rotate(-Math.PI/2)` in current `useYg`.
    assert_eq!(fans.title_anchor(), 164.5);
    assert_eq!(pump.title_anchor(), 164.5);
}

#[::core::prelude::v1::test]
fn hanbo_mode_presentation_preserves_source_selected_hover_and_disabled_precedence() {
    assert_eq!(
        mode_presentation(false, false, false, false),
        (ModeBackground::Normal, false)
    );
    assert_eq!(
        mode_presentation(false, false, true, false),
        (ModeBackground::Hover, true)
    );
    assert_eq!(
        mode_presentation(false, false, true, true),
        (ModeBackground::Selected, true)
    );
    assert_eq!(
        mode_presentation(true, false, true, false),
        (ModeBackground::Selected, true)
    );
    assert_eq!(
        mode_presentation(true, true, true, true),
        (ModeBackground::Selected, true)
    );
    assert_eq!(
        mode_presentation(false, true, true, true),
        (ModeBackground::Normal, false)
    );
    assert_eq!(
        mode_presentation(false, false, false, true),
        (ModeBackground::Normal, false)
    );
}

#[::core::prelude::v1::test]
fn hanbo_curve_cleanup_commits_previous_points_then_release_flushes_last_points() {
    let channel = CoolerChannel::Fans;
    let mut state = advanced(channel);
    let geometry = ChartGeometry::new(channel);
    let pressed = press(&mut state, channel, 2);
    assert_eq!(pressed.thermal.advanced_setting[2], 40.);

    let first = state.move_curve(channel, geometry.y(46.)).unwrap();
    assert_eq!(first.thermal.advanced_setting[2], 40.);
    assert_eq!(state.profile.fans.advanced_setting[2], 40.);
    assert!((state.curve_values(channel)[2] - 46.).abs() < 0.001);

    let second = state.move_curve(channel, geometry.y(48.)).unwrap();
    assert_eq!(second.thermal.advanced_setting[2], 46.);
    let released = state.release_curve().unwrap();
    assert_eq!(released.thermal.advanced_setting[2], 48.);
    assert_eq!(state.profile.fans.advanced_setting[2], 48.);
    assert!(state.drag.is_none());
    assert!(state.release_curve().is_none());
}

#[::core::prelude::v1::test]
fn hanbo_curve_drag_remains_monotonic_outside_plot_bounds() {
    let channel = CoolerChannel::Pump;
    let mut state = advanced(channel);
    press(&mut state, channel, 2);
    state.move_curve(channel, -1_000.).unwrap();
    assert!((state.curve_values(channel)[2] - 80.).abs() < 0.001);
    state.move_curve(channel, 10_000.).unwrap();
    assert!((state.curve_values(channel)[2] - 40.).abs() < 0.001);
    assert_eq!(
        state.release_curve().unwrap().thermal.advanced_setting[2],
        40.
    );
}

#[::core::prelude::v1::test]
fn hanbo_page_exit_flushes_both_advanced_editors_once_in_source_order() {
    let mut state = advanced(CoolerChannel::Fans);
    state.set_mode(CoolerChannel::Pump, ThermalMode::Advanced);
    press(&mut state, CoolerChannel::Pump, 2);
    state
        .move_curve(
            CoolerChannel::Pump,
            ChartGeometry::new(CoolerChannel::Pump).y(66.),
        )
        .unwrap();
    let cleanup = state.unmount_curves();
    assert_eq!(cleanup.len(), 2);
    assert_eq!(cleanup[0].channel, CoolerChannel::Fans);
    assert_eq!(cleanup[1].channel, CoolerChannel::Pump);
    assert_eq!(cleanup[1].thermal.advanced_setting[2], 66.);
    assert!(state.drag.is_none());
    assert!(state.unmount_curves().is_empty());
}

#[::core::prelude::v1::test]
fn hanbo_help_leave_rejects_late_drag_and_reentry_uses_the_final_saved_points() {
    let channel = CoolerChannel::Fans;
    let mut state = advanced(channel);
    state.set_mode(CoolerChannel::Pump, ThermalMode::Advanced);
    let geometry = ChartGeometry::new(channel);
    press(&mut state, channel, 2);
    state.move_curve(channel, geometry.y(47.36)).unwrap();

    let cleanup = state.unmount_curves();
    assert_eq!(cleanup.len(), 2);
    assert_eq!(cleanup[0].channel, channel);
    assert_eq!(cleanup[0].thermal.advanced_setting[2], 47.4);
    assert_eq!(cleanup[1].channel, CoolerChannel::Pump);
    assert!(state.editor(channel).is_none());
    assert!(state.editor(CoolerChannel::Pump).is_none());
    assert!(state.move_curve(channel, geometry.y(70.)).is_none());
    assert!(state.release_curve().is_none());

    state.mount_curves();
    assert_eq!(state.curve_values(channel)[2], 47.4);
    assert!(state.drag.is_none());
    assert!(state.release_curve().is_none());
}

#[::core::prelude::v1::test]
fn hanbo_locale_cleanup_submits_visible_points_and_keeps_the_active_drag() {
    let mut state = advanced(CoolerChannel::Fans);
    state.set_mode(CoolerChannel::Pump, ThermalMode::Advanced);
    press(&mut state, CoolerChannel::Fans, 2);
    let geometry = ChartGeometry::new(CoolerChannel::Fans);
    state
        .move_curve(CoolerChannel::Fans, geometry.y(47.36))
        .unwrap();
    let visible = state.curve_values(CoolerChannel::Fans).to_vec();
    let locale = format!("{}-changed", i18n::locale());

    let cleanup = state.refresh_curve_locale(&locale);
    assert_eq!(cleanup.len(), 2);
    assert_eq!(cleanup[0].channel, CoolerChannel::Fans);
    assert_eq!(cleanup[1].channel, CoolerChannel::Pump);
    assert_eq!(cleanup[0].thermal.advanced_setting[2], 47.4);
    assert_eq!(state.curve_values(CoolerChannel::Fans), visible);
    assert!(state.drag.is_some());
    assert!(state.refresh_curve_locale(&locale).is_empty());

    state
        .move_curve(CoolerChannel::Fans, geometry.y(49.))
        .unwrap();
    assert_eq!(
        state.release_curve().unwrap().thermal.advanced_setting[2],
        49.
    );
    assert_eq!(state.unmount_curves().len(), 2);
}

#[::core::prelude::v1::test]
fn hanbo_locale_change_cleans_only_mounted_advanced_charts() {
    let mut state = HanboCooler::default();
    assert!(state.refresh_curve_locale("new-language").is_empty());
    state.set_mode(CoolerChannel::Pump, ThermalMode::Advanced);
    let locale = format!("{}-changed", i18n::locale());
    let cleanup = state.refresh_curve_locale(&locale);
    assert_eq!(cleanup.len(), 1);
    assert_eq!(cleanup[0].channel, CoolerChannel::Pump);
    state.set_mode(CoolerChannel::Pump, ThermalMode::Quiet);
    assert!(state.refresh_curve_locale("another-language").is_empty());
}

#[::core::prelude::v1::test]
fn hanbo_mode_dispatch_precedes_unmount_cleanup_of_pending_curve() {
    let channel = CoolerChannel::Fans;
    let mut state = advanced(channel);
    press(&mut state, channel, 2);
    state
        .move_curve(channel, ChartGeometry::new(channel).y(47.))
        .unwrap();
    let commits = state.set_mode(channel, ThermalMode::Performance);
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].thermal.advanced_setting[2], 40.);
    assert_eq!(commits[1].thermal.advanced_setting[2], 47.);
    assert!(commits.iter().all(
        |commit| commit.thermal.active_thermal_indicator.mode_selected == ThermalMode::Performance
    ));
    assert!(state.editor(channel).is_none());
    assert!(state.drag.is_none());
}

#[::core::prelude::v1::test]
fn hanbo_same_value_pump_and_curve_cleanup_preserve_source_save_interactions() {
    let mut state = HanboCooler::default();
    assert!(
        state
            .set_mode(CoolerChannel::Fans, ThermalMode::Normal)
            .is_empty()
    );
    assert_eq!(
        state
            .set_mode(CoolerChannel::Pump, ThermalMode::Normal)
            .len(),
        1
    );
    state.set_mode(CoolerChannel::Fans, ThermalMode::Advanced);
    let pressed = press(&mut state, CoolerChannel::Fans, 2);
    let released = state.release_curve().unwrap();
    assert_eq!(pressed.thermal, released.thermal);
    assert_eq!(state.unmount_curves().len(), 1);
}

#[::core::prelude::v1::test]
fn hanbo_snapshot_keeps_all_source_ports_and_extension_fields() {
    let mut state = HanboCooler::default();
    state.set_mode(CoolerChannel::Fans, ThermalMode::Performance);
    let mut original = razer_device::hanbo_settings::default_ports();
    original[0]["brightness"]["value"] = json!(71);
    original[1]["thermal"]["extension"] = json!("retained");
    original[1]["thermal"]["activeThermalIndicator"]["serviceFlag"] = json!(3);
    original
        .as_array_mut()
        .unwrap()
        .push(json!({"id":99,"name":"Extra","extension":{"key":1}}));
    let snapshot = state.snapshot_ports(Some(&original));
    assert_eq!(snapshot.as_array().unwrap().len(), 7);
    assert_eq!(snapshot[0], original[0]);
    assert_eq!(snapshot[1]["thermal"]["extension"], "retained");
    assert_eq!(
        snapshot[1]["thermal"]["activeThermalIndicator"]["serviceFlag"],
        3
    );
    assert_eq!(
        snapshot[1]["thermal"]["activeThermalIndicator"]["modeSelected"],
        "PERFORMANCE"
    );
    for index in 2..7 {
        assert_eq!(snapshot[index], original[index]);
    }
    assert_eq!(state.snapshot_ports(None).as_array().unwrap().len(), 6);
}

#[::core::prelude::v1::test]
fn hanbo_profile_restore_discards_editor_drafts_without_resetting_runtime() {
    let mut state = advanced(CoolerChannel::Fans);
    let sample = serde_json::from_value(json!({
        "fan":{"channel_number":1,"presence":1,"set_mode":2,"actual_mode":2,"rpm":1234,"set_duty_cycle":50,"actual_duty_cycle":50},
        "pump":{"presence":1,"set_mode":2,"actual_mode":2,"temperature_tenths_celsius":355,"rpm":2345,"set_duty_cycle":50,"actual_duty_cycle":50}
    })).unwrap();
    state.runtime.observe_telemetry(&sample);
    let runtime = state.runtime;
    press(&mut state, CoolerChannel::Fans, 2);
    state
        .move_curve(
            CoolerChannel::Fans,
            ChartGeometry::new(CoolerChannel::Fans).y(47.),
        )
        .unwrap();
    state.restore_ports(None);
    assert_eq!(state.profile, ThermalProfile::default());
    assert!(state.editor(CoolerChannel::Fans).is_none());
    assert!(state.drag.is_none());
    assert_eq!(state.runtime, runtime);
}
