use super::*;
use serde_json::{Value, json};

fn convert(id: u16, settings: Option<Value>) -> anyhow::Result<EngineEffect> {
    let mut source = json!({"selectedEffectId":id});
    if let Some(settings) = settings {
        source["selectedEffectSetting"] = settings;
    }
    QuickEffect::parse(&source)?.engine_effect()
}

#[test]
fn wheel_expands_source_palette_direction_and_manifest_matrix_center() {
    let effect = QuickEffect::parse(&json!({
        "selectedEffectId":13,"selectedEffectSetting":{"direction":2}
    }))
    .unwrap()
    .engine_effect_for_geometry(&crate::marci_lighting::led_configuration())
    .unwrap();
    assert_eq!(effect.effect, 13);
    let p = effect.parameters;
    assert_eq!(p.mode, Some(2048));
    assert_eq!(p.speed, Some(360.));
    assert_eq!(p.cycles, Some(-1));
    assert_eq!((p.center_row, p.center_col), (Some(15), Some(16)));
    assert_eq!(
        p.color_stops.unwrap(),
        vec![
            ColorStop {
                stop: 0,
                color: 0xff18ff
            },
            ColorStop {
                stop: 18,
                color: 0x1010ff
            },
            ColorStop {
                stop: 28,
                color: 0x0080ff
            },
            ColorStop {
                stop: 35,
                color: 0x00ffef
            },
            ColorStop {
                stop: 42,
                color: 0x00ffdf
            },
            ColorStop {
                stop: 49,
                color: 0x00ff00
            },
            ColorStop {
                stop: 52,
                color: 0xffff00
            },
            ColorStop {
                stop: 64,
                color: 0xff5818
            },
            ColorStop {
                stop: 77,
                color: 0xff5818
            },
            ColorStop {
                stop: 100,
                color: 0xff18ff
            },
        ]
    );
    assert_eq!(convert(13, None).unwrap().parameters.mode, Some(1024));
    assert!(convert(13, Some(json!({"direction":3}))).is_err());
}

#[test]
fn static_keeps_unobserved_color_distinct_from_the_ui_default() {
    assert_eq!(convert(1, None).unwrap().parameters.color, Some(NO_COLOR));
    let effect = convert(1, Some(json!({"color1":"#112233"}))).unwrap();
    assert_eq!(effect.effect, 6);
    assert_eq!(effect.parameters.color, Some(0x332211));
    assert!(convert(1, Some(json!({"color1":"green"}))).is_err());
    assert!(convert(1, Some(json!({"color1":42}))).is_err());
    assert!(convert(1, Some(Value::Null)).is_err());
}

#[test]
fn breathing_preserves_missing_first_color_and_accessory_timing() {
    let missing = convert(2, Some(json!({"color2":"#112233","duration":1}))).unwrap();
    let shifted = convert(
        2,
        Some(json!({"color1":"no-color","color2":"#112233","duration":3})),
    )
    .unwrap();
    let missing = missing.parameters;
    let shifted = shifted.parameters;
    assert_eq!(missing.duration, Some(14000));
    assert_eq!(missing.color_stops.as_ref().unwrap().len(), 9);
    assert_eq!(missing.color_stops.as_ref().unwrap()[2].color, NO_COLOR);
    assert_eq!(missing.color_stops.as_ref().unwrap()[6].color, 0x332211);
    assert_eq!(shifted.duration, Some(7000));
    assert_eq!(shifted.color_stops.as_ref().unwrap().len(), 5);
    assert_eq!(shifted.color_stops.as_ref().unwrap()[2].color, 0x332211);
    let random = convert(2, Some(json!({"isRandom":true})))
        .unwrap()
        .parameters;
    assert_eq!(random.mode, Some(1));
    assert_eq!(random.color_stops, Some(Vec::new()));
    assert_eq!(random.static_color, Some(NO_COLOR));
}

#[test]
fn spectrum_uses_accessory_preset_even_if_a_profile_carries_gamepad_duration() {
    let default = convert(3, None).unwrap();
    assert_eq!(default.effect, 4);
    assert_eq!(default.parameters.duration, Some(37740));
    assert_eq!(default.parameters.color_stops.as_ref().unwrap().len(), 10);
    assert_eq!(default, convert(3, Some(json!({"duration":1}))).unwrap());
}

#[test]
fn accessory_wave_preserves_rotation_split_imports_and_accessory_speed() {
    for (direction, mode, angle) in [
        (1, 0, 270),
        (2, 0, 90),
        (3, 0, 0),
        (4, 0, 180),
        (5, 2, 90),
        (6, 2, 270),
        (11, 1024, 0),
        (12, 2048, 0),
    ] {
        let effect = convert(4, Some(json!({"direction":direction,"speed":1}))).unwrap();
        assert_eq!(effect.effect, 2);
        assert_eq!(effect.parameters.mode, Some(mode));
        assert_eq!(effect.parameters.angle, Some(angle));
        assert_eq!(effect.parameters.speed, Some(15.));
        assert_eq!(
            effect.parameters.color_stops.as_ref().unwrap()[0].color,
            255
        );
    }
    let default = convert(4, None).unwrap().parameters;
    assert_eq!(default.angle, Some(180));
    assert_eq!(default.speed, Some(25.));
    assert_eq!(
        convert(4, Some(json!({"direction":12,"speed":3})))
            .unwrap()
            .parameters
            .speed,
        Some(35.)
    );
    assert!(convert(4, Some(json!({"direction":7}))).is_err());
    assert!(convert(4, Some(json!({"speed":4}))).is_err());
}

#[test]
fn reactive_and_starlight_use_distinct_duration_and_native_density_rules() {
    let reactive = convert(5, Some(json!({"color1":"#112233","duration":2}))).unwrap();
    assert_eq!(reactive.effect, 3);
    assert_eq!(reactive.parameters.mode, Some(4128));
    assert_eq!(reactive.parameters.duration, Some(1500));
    assert_eq!(
        reactive.parameters.color_stops.as_ref().unwrap()[1].color,
        NO_COLOR
    );
    let star = convert(
        7,
        Some(json!({"color1":"#112233","color2":"#000000","duration":2})),
    )
    .unwrap();
    assert_eq!(star.effect, 7);
    assert_eq!(star.parameters.duration, Some(1700));
    assert_eq!(star.parameters.max_star, Some(20.));
    assert_eq!(star.parameters.regen, Some(10));
    assert_eq!(star.parameters.color_stops.as_ref().unwrap()[2].color, 0);
    let random = convert(7, Some(json!({"isRandom":true,"duration":3})))
        .unwrap()
        .parameters;
    assert_eq!(random.mode, Some(1));
    assert_eq!(random.duration, Some(2000));
    assert_eq!(random.color_stops, Some(Vec::new()));
}

#[test]
fn fire_expands_the_complete_fixed_native_preset() {
    let effect = convert(8, None).unwrap();
    assert_eq!(effect.effect, 8);
    assert_eq!(
        serde_json::to_value(effect.parameters).unwrap(),
        json!({
            "Mode":0,"ColorStops":[{"Stop":0,"Color":32767},{"Stop":50,"Color":255},{"Stop":100,"Color":0}],
            "Rate":160,"Seed":100,"Cycles":-1
        })
    );
}

#[test]
fn audio_keeps_fractional_boost_and_absent_input_distinct() {
    let effect = convert(12, Some(json!({"colorBoost":1.25,"color1":"#112233"}))).unwrap();
    assert_eq!(effect.effect, 12);
    assert_eq!(
        serde_json::to_value(effect.parameters).unwrap(),
        json!({
            "Mode":0,"ColorStops":[{"Stop":0,"Color":65280},{"Stop":50,"Color":65535},{"Stop":100,"Color":255}],
            "BoostLevel":1.25,"AutoBoost":0,"Decay":1000,"Auto":0,"Flow":0
        })
    );
    assert_eq!(convert(12, None).unwrap().parameters.boost_level, None);
    assert!(convert(12, Some(json!({"colorBoost":0.}))).is_err());
    assert!(convert(12, Some(json!({"colorBoost":"1.25"}))).is_err());
}

#[test]
fn tidal_uses_manifest_centers_and_source_random_color_indices() {
    let effect = convert(19, Some(json!({"centerPointX":999,"centerPointY":999}))).unwrap();
    assert_eq!(effect.effect, 19);
    let params = effect.parameters;
    assert_eq!(params.mode, Some(2));
    assert_eq!(params.direction, Some(1));
    assert_eq!(params.center_col, Some(1));
    assert_eq!(params.center_row, Some(2));
    assert_eq!(params.color_stops.as_ref().unwrap()[2].color, 65280);
    assert_eq!(params.color_stops.as_ref().unwrap()[5].color, 16711680);
    let random = convert(19, Some(json!({"isRandom":true,"direction":0})))
        .unwrap()
        .parameters;
    assert_eq!(random.mode, Some(3));
    assert_eq!(random.direction, Some(0));
    assert_eq!(random.color_stops.as_ref().unwrap()[2].color, 65536);
    assert_eq!(random.color_stops.as_ref().unwrap()[5].color, 131072);
    assert!(convert(19, Some(json!({"direction":2}))).is_err());
}

#[test]
fn battery_uses_its_monitor_and_external_source_fields_round_trip() {
    let source = json!({"selectedEffectId":14,"selectedEffectSetting":{"extension":7},"owner":"serial","regionId":0});
    let effect = QuickEffect::parse(&source).unwrap();
    assert_eq!(effect.source_id(), 14);
    assert!(effect.engine_effect().is_err());
    assert_eq!(serde_json::to_value(effect).unwrap(), source);
    for id in [0, 6, 17, 194] {
        assert!(convert(id, None).is_err());
    }
}

#[test]
fn ambient_uses_current_screen_choices_and_native_truncated_crop_units() {
    use crate::lighting_engine::ScreenRect;
    assert_eq!(
        convert(11, None).unwrap().parameters.screen_rect,
        Some(ScreenRect::FULL)
    );
    for (screen, rect) in [
        ("full", ScreenRect::FULL),
        (
            "left",
            ScreenRect {
                right: 6553,
                ..ScreenRect::FULL
            },
        ),
        (
            "top",
            ScreenRect {
                bottom: 6553,
                ..ScreenRect::FULL
            },
        ),
        (
            "right",
            ScreenRect {
                left: 58981,
                ..ScreenRect::FULL
            },
        ),
        (
            "bottom",
            ScreenRect {
                top: 58981,
                ..ScreenRect::FULL
            },
        ),
    ] {
        let effect = convert(11, Some(json!({"screen":screen}))).unwrap();
        assert_eq!(effect.effect, 11);
        assert_eq!(effect.parameters.screen_rect, Some(rect));
        assert_eq!(effect.parameters.blur, Some(1.));
    }
    assert!(convert(11, Some(json!({"screen":"custom"}))).is_err());
}

#[test]
fn wheel_shared_defaults_do_not_expand_mounted_dock_capabilities() {
    let effect = QuickEffect::parse(&json!({"selectedEffectId":13})).unwrap();
    let native = effect.engine_effect().unwrap();
    assert_eq!(native.effect, 13);
    assert_eq!(native.parameters.mode, Some(1024));
    assert_eq!(native.parameters.speed, Some(360.));
    assert_eq!(native.parameters.cycles, Some(-1));
    assert_eq!(
        (native.parameters.center_col, native.parameters.center_row),
        (Some(1), Some(2))
    );
    assert_eq!(native, convert(13, Some(json!({"direction":1}))).unwrap());
    for product_id in [164, 241] {
        let geometry = crate::receiver_lighting::led_configuration(product_id, 0).unwrap();
        assert_eq!(
            effect.engine_effect_for_geometry(&geometry).unwrap(),
            native
        );
        let configuration = crate::receiver_lighting::Configuration {
            product_id,
            edition_id: 0,
            effect: effect.clone(),
            selected_docking_mouse: None,
        };
        let error = configuration.validate().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("absent from the mounted dock page")
        );
    }
}

#[test]
fn peer_tidal_uses_its_matrix_center_instead_of_the_dock_center() {
    let effect = QuickEffect::parse(&json!({
        "selectedEffectId":19,
        "selectedEffectSetting":{"centerPointX":0,"centerPointY":0}
    }))
    .unwrap();
    for (product, center_col, center_row) in [(215, 2, 3), (221, 3, 3), (234, 0, 0)] {
        let geometry = crate::receiver_lighting::peer_manifest(product, 0, 0)
            .unwrap()
            .led;
        let native = effect.engine_effect_for_geometry(&geometry).unwrap();
        assert_eq!(native.parameters.center_col, Some(center_col));
        assert_eq!(native.parameters.center_row, Some(center_row));
        assert_eq!(native.effect, 19);
    }
    for product in [164, 241] {
        let geometry = crate::receiver_lighting::led_configuration(product, 0).unwrap();
        assert_eq!(
            effect.engine_effect_for_geometry(&geometry).unwrap(),
            effect.engine_effect().unwrap()
        );
    }
}
