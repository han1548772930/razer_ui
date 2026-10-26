use super::*;
use crate::{
    lighting_engine::{EffectParameters, QuickEffectLevel, SoftwareEffect, WaveDirection},
    receiver_lighting::{self, QuickEffect},
};

fn render(source: &EngineEffect, rows: usize, cols: usize) -> Renderer {
    Renderer::new(source, rows, cols, 0, &mut 1).unwrap()
}
fn frame(renderer: &mut Renderer, count: usize, audio: Option<f32>) -> Vec<[u8; 3]> {
    let mut frame = vec![[0; 3]; count];
    renderer.render(&mut frame, 1., audio).unwrap();
    frame
}
fn quick(id: u16, settings: serde_json::Value) -> EngineEffect {
    QuickEffect::parse(&serde_json::json!({"selectedEffectId":id,"selectedEffectSetting":settings}))
        .unwrap()
        .engine_effect()
        .unwrap()
}

#[test]
#[ignore = "explicit pure Rust snapshot for tools/workspace.py lighting DLL comparison"]
fn explicit_native_lighting_snapshots() {
    use crate::lighting_engine::{LedConfiguration, LedMapping};
    use serde_json::json;
    let geometry = LedConfiguration {
        version: 2,
        vendor_id: 5426,
        product_id: 3942,
        edition: 0,
        layout: 0,
        matrix_max_row: 3,
        matrix_max_col: 3,
        device_max_row: 3,
        device_max_col: 3,
        led_width: 40,
        led_height: 40,
        led_width_offset: 0,
        led_height_offset: 0,
        max_region: 0,
        region_max_row: 0,
        region_max_col: 0,
        led_matrix: (0..3)
            .flat_map(|row| {
                (0..3).map(move |col| LedMapping {
                    matrix_pos: [row, col],
                    device_pos: [row, col],
                })
            })
            .collect(),
        led_regions: Vec::new(),
        led_input_map: Vec::new(),
    };
    let mut scenarios = Vec::new();
    for (name, effect) in [
        (
            "static",
            EngineEffect {
                effect: 6,
                parameters: EffectParameters {
                    color: Some(0x332211),
                    ..Default::default()
                },
            },
        ),
        ("wheel_stationary", wheel(2048, 0., -1)),
    ] {
        let mut stable = render(&effect, 3, 3);
        let stable_packed_colorref = frame(&mut stable, 9, None)
            .into_iter()
            .map(|[r, g, b]| u32::from_le_bytes([r, g, b, 0]))
            .collect::<Vec<_>>();
        let mut engine = Engine::new(0);
        let device = engine.create_device(&geometry).unwrap();
        let instance = engine.create_engine().unwrap();
        engine.add_device(instance, device).unwrap();
        engine.add_effect(instance, &effect).unwrap();
        engine.enable(instance, true, 0).unwrap();
        let mut ticks = Vec::new();
        for logical_ms in (0..=4000).step_by(40) {
            let frames = engine
                .frames(logical_ms, [None; 2])
                .unwrap()
                .into_iter()
                .map(|frame| {
                    json!({
                        "device_handle": frame.device_handle,
                        "packed_colorref": frame.frame.chunks_exact(3)
                            .map(|rgb| u32::from_le_bytes([rgb[0],rgb[1],rgb[2],0]))
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            ticks.push(json!({"logical_ms":logical_ms,"frames":frames}));
        }
        engine.enable(instance, false, 4040).unwrap();
        engine.destroy_engine(instance).unwrap();
        engine.destroy_device(device).unwrap();
        engine.terminate();
        scenarios.push(json!({"name":name,"geometry":geometry,"effect":effect,
            "epoch_seconds":0,"ticks":ticks,"stable_packed_colorref":stable_packed_colorref,
            "cleanup":{"engine_destroyed":true,"device_destroyed":true,"terminated":true}}));
    }
    println!(
        "RAZER_LIGHTING_RESULT {}",
        json!({"schema_version":1,"scenarios":scenarios})
    );
}

fn wheel(mode: u32, speed: f64, cycles: i32) -> EngineEffect {
    EngineEffect {
        effect: 13,
        parameters: EffectParameters {
            mode: Some(mode),
            speed: Some(speed),
            cycles: Some(cycles),
            center_row: Some(1),
            center_col: Some(1),
            color_stops: Some(vec![
                ColorStop {
                    stop: 0,
                    color: 0x0000ff,
                },
                ColorStop {
                    stop: 100,
                    color: 0xff0000,
                },
            ]),
            ..Default::default()
        },
    }
}

#[test]
fn wheel_native_angles_use_row_then_column_and_a_1024_entry_gradient() {
    // 0x66d60/0x66f20 cardinal and diagonal angles select indices
    // [128,256,384,0,512,512,896,768,640]. 0x692e0 truncates f32 channels.
    let mut renderer = render(&wheel(2048, 360., -1), 3, 3);
    assert_eq!(
        frame(&mut renderer, 9, None),
        [
            [223, 0, 31],
            [191, 0, 63],
            [159, 0, 95],
            [255, 0, 0],
            [127, 0, 127],
            [127, 0, 127],
            [31, 0, 223],
            [63, 0, 191],
            [95, 0, 159],
        ]
    );
}

#[test]
fn wheel_direction_preserves_phase_truncation_period_and_cursor_reset() {
    let mut clockwise = render(&wheel(1024, 360., -1), 3, 3);
    let mut counterclockwise = render(&wheel(2048, 360., -1), 3, 3);
    let first = frame(&mut clockwise, 9, None);
    assert_eq!(first, frame(&mut counterclockwise, 9, None));
    // Speed 360 yields 25 steps. Opposite modes truncate phases 983 and 40,
    // producing indices 471 and 552 at the right-hand cardinal position.
    assert_eq!(frame(&mut clockwise, 9, None)[5], [137, 0, 117]);
    assert_eq!(frame(&mut counterclockwise, 9, None)[5], [117, 0, 137]);
    clockwise.reset().unwrap();
    assert_eq!(frame(&mut clockwise, 9, None), first);
    for _ in 1..25 {
        frame(&mut clockwise, 9, None);
    }
    assert_eq!(frame(&mut clockwise, 9, None), first);
    let mut finite = render(&wheel(2048, 360., 1), 3, 3);
    for _ in 0..25 {
        assert!(frame(&mut finite, 9, None).iter().any(|p| *p != [0; 3]));
    }
    assert_eq!(frame(&mut finite, 9, None), [[0; 3]; 9]);
    finite.reset().unwrap();
    assert_eq!(frame(&mut finite, 9, None), [[0; 3]; 9]);
    let mut stationary = render(&wheel(2048, 0., 1), 3, 3);
    for _ in 0..50 {
        assert_eq!(frame(&mut stationary, 9, None), first);
    }
}

#[test]
fn gradient_uses_both_segment_endpoints_and_native_truncation() {
    let values = gradient(
        &[
            ColorStop {
                stop: 0,
                color: 255,
            },
            ColorStop {
                stop: 100,
                color: TRANSPARENT,
            },
        ],
        5,
    )
    .unwrap();
    assert_eq!(
        values,
        [
            [255, 0, 0, 0],
            [191, 0, 0, 25],
            [127, 0, 0, 50],
            [63, 0, 0, 75],
            [0, 0, 0, 100]
        ]
    );
    let values = gradient(
        &[
            ColorStop {
                stop: 0,
                color: 255,
            },
            ColorStop {
                stop: 50,
                color: 65280,
            },
            ColorStop {
                stop: 100,
                color: 16711680,
            },
        ],
        6,
    )
    .unwrap();
    assert_eq!(
        values,
        [
            [255, 0, 0, 0],
            [127, 127, 0, 0],
            [0, 255, 0, 0],
            [0, 255, 0, 0],
            [0, 127, 127, 0],
            [0, 0, 255, 0]
        ]
    );
}

#[test]
fn reactive_requires_input_and_nonblack_keying_preserves_source_alpha_rule() {
    let source = EngineEffect {
        effect: 3,
        parameters: EffectParameters {
            mode: Some(4128),
            duration: Some(200),
            cycles: Some(1),
            color_stops: Some(vec![
                ColorStop {
                    stop: 0,
                    color: 255,
                },
                ColorStop {
                    stop: 100,
                    color: TRANSPARENT,
                },
            ]),
            ..Default::default()
        },
    };
    let mut renderer = render(&source, 1, 2);
    assert_eq!(frame(&mut renderer, 2, None), [[0; 3]; 2]);
    renderer.input(&[1]);
    assert_eq!(frame(&mut renderer, 2, None), [[0, 0, 0], [255, 0, 0]]);
    assert_eq!(frame(&mut renderer, 2, None), [[0, 0, 0], [191, 0, 0]]);
    for _ in 0..4 {
        frame(&mut renderer, 2, None);
    }
    assert_eq!(frame(&mut renderer, 2, None), [[0; 3]; 2]);
}

#[test]
fn wave_axes_and_native_integer_speed_are_preserved() {
    let mut source = SoftwareEffect::Wave {
        direction: WaveDirection::Right,
        speed: Some(QuickEffectLevel::Two),
    }
    .engine_effect();
    let mut renderer = render(&source, 1, 3);
    assert_eq!(
        frame(&mut renderer, 3, None),
        [[255, 0, 0], [0, 255, 0], [0, 0, 255]]
    );
    source.parameters.speed = Some(12.5);
    let mut fractional = render(&source, 5, 3);
    source.parameters.speed = Some(12.);
    let mut truncated = render(&source, 5, 3);
    for _ in 0..60 {
        assert_eq!(
            frame(&mut fractional, 15, None),
            frame(&mut truncated, 15, None)
        );
    }
}

#[test]
fn fire_first_pixel_uses_crt_seed_and_fixed_masks() {
    // CRT seed 1 -> rand 41 -> intensity 73; bottom-left attenuation 8
    // leaves 71. Palette index 64 gives [255,63,0], hence [71,17,0].
    let mut renderer = render(&quick(8, serde_json::json!({})), 1, 1);
    assert_eq!(frame(&mut renderer, 1, None), [[71, 17, 0]]);
}

#[test]
fn audio_requires_actual_peak_and_uses_source_decay() {
    let mut source = quick(12, serde_json::json!({"colorBoost":1.0}));
    source.parameters.color_stops = Some(vec![
        ColorStop { stop: 0, color: 0 },
        ColorStop {
            stop: 100,
            color: 255,
        },
    ]);
    let mut renderer = render(&source, 1, 1);
    assert!(renderer.render(&mut [[0; 3]], 1., None).is_err());
    assert_eq!(frame(&mut renderer, 1, Some(1.)), [[255, 0, 0]]);
    assert_eq!(frame(&mut renderer, 1, Some(0.)), [[244, 0, 0]]);
    assert!(renderer.render(&mut [[0; 3]], 1., Some(f32::NAN)).is_err());
}

#[test]
fn every_mounted_effect_and_direction_renders_with_explicit_audio() {
    let mut effects = vec![];
    for id in [1, 2, 3, 5, 7, 8, 12, 19] {
        effects.push(quick(
            id,
            serde_json::json!({"color1":"#ff8000","colorBoost":1.0}),
        ));
    }
    for id in [2, 7, 19] {
        effects.push(quick(id, serde_json::json!({"isRandom":true})));
    }
    for direction in [1, 2, 3, 4, 5, 6, 11, 12] {
        effects.push(quick(4, serde_json::json!({"direction":direction})));
    }
    for effect in effects {
        let mut renderer = render(&effect, 5, 3);
        renderer.input(&[1, 3, 5, 6, 8, 9, 11, 13]);
        for _ in 0..220 {
            assert_eq!(frame(&mut renderer, 15, Some(0.5)).len(), 15);
        }
        renderer.reset().unwrap();
        frame(&mut renderer, 15, Some(0.25));
    }
}

fn engine_with(effect: &EngineEffect) -> (Engine, u32, u32, u32) {
    let mut engine = Engine::new(0);
    let device = engine
        .create_device(&receiver_lighting::led_configuration(164, 0).unwrap())
        .unwrap();
    let owner = engine.create_engine().unwrap();
    engine.add_device(owner, device).unwrap();
    let effect = engine.add_effect(owner, effect).unwrap();
    engine.enable(owner, true, 0).unwrap();
    (engine, owner, device, effect)
}
#[test]
fn engine_fade_keepalive_and_cleanup_have_one_owner() {
    let (mut engine, owner, device, _) =
        engine_with(&SoftwareEffect::Static { color: [255, 0, 0] }.engine_effect());
    assert_eq!(
        engine.frames(0, [None, None]).unwrap()[0].frame,
        vec![3, 0, 0].repeat(8)
    );
    assert!(engine.frames(39, [None, None]).unwrap().is_empty());
    for tick in 1..=73 {
        engine.frames(tick * 40, [None, None]).unwrap();
    }
    assert_eq!(
        engine.frames(74 * 40, [None, None]).unwrap()[0].frame,
        vec![255, 0, 0].repeat(8)
    );
    for tick in 75..=99 {
        engine.frames(tick * 40, [None, None]).unwrap();
    }
    assert!(engine.frames(100 * 40, [None, None]).unwrap().is_empty());
    assert_eq!(engine.frames(125 * 40, [None, None]).unwrap().len(), 0);
    // Logical ticks, rather than a wall-time jump, determine keepalive.
    for tick in 126..149 {
        engine.frames(tick * 40, [None, None]).unwrap();
    }
    assert_eq!(engine.frames(149 * 40, [None, None]).unwrap().len(), 1);
    assert!(engine.destroy_device(device).is_err());
    engine.enable(owner, false, 6000).unwrap();
    assert!(engine.frames(6000, [None, None]).unwrap().is_empty());
    engine.destroy_engine(owner).unwrap();
    engine.destroy_device(device).unwrap();
    assert!(engine.input(device).is_err());
    engine.terminate();
    assert!(engine.create_engine().is_err());
}

#[test]
fn empty_input_map_activates_all_mapped_leds_and_destroyed_handles_do_not_return() {
    let (mut engine, owner, device, _) = engine_with(
        &SoftwareEffect::Reactive {
            color: [255, 0, 0],
            duration: QuickEffectLevel::Two,
        }
        .engine_effect(),
    );
    assert!(
        engine.frames(0, [None, None]).unwrap()[0]
            .frame
            .iter()
            .all(|v| *v == 0)
    );
    for tick in 1..75 {
        engine.frames(tick * 40, [None, None]).unwrap();
    }
    engine.input(device).unwrap();
    let frame = engine.frames(3000, [None, None]).unwrap().pop().unwrap();
    assert_eq!(frame.frame, vec![255, 0, 0].repeat(8));
    engine.destroy_engine(owner).unwrap();
    engine.destroy_device(device).unwrap();
    assert!(engine.frames(3040, [None, None]).unwrap().is_empty());
    let next = engine
        .create_device(&receiver_lighting::led_configuration(164, 0).unwrap())
        .unwrap();
    assert_ne!(next, device);
}

#[test]
fn effect_replacement_fades_previous_frame_without_reusing_effect_handles() {
    let (mut engine, owner, _, old) =
        engine_with(&SoftwareEffect::Static { color: [255, 0, 0] }.engine_effect());
    for tick in 0..75 {
        engine.frames(tick * 40, [None, None]).unwrap();
    }
    engine.remove_effect(owner, old).unwrap();
    let new = engine
        .add_effect(
            owner,
            &SoftwareEffect::Static { color: [0, 0, 255] }.engine_effect(),
        )
        .unwrap();
    assert_ne!(new, old);
    assert_eq!(
        engine.frames(3000, [None, None]).unwrap()[0].frame,
        vec![251, 0, 0].repeat(8)
    );
    for tick in 76..150 {
        engine.frames(tick * 40, [None, None]).unwrap();
    }
    assert_eq!(
        engine.frames(6000, [None, None]).unwrap()[0].frame,
        vec![0, 0, 255].repeat(8)
    );
}

#[test]
fn all_current_peer_geometry_is_accepted_and_device_frame_sizes_stay_exact() {
    for &(product, edition) in receiver_lighting::peer_identities() {
        let manifest = receiver_lighting::peer_manifest(product, edition, 0).unwrap();
        let mut engine = Engine::new(0);
        let device = engine.create_device(&manifest.led).unwrap();
        let owner = engine.create_engine().unwrap();
        engine.add_device(owner, device).unwrap();
        engine
            .add_effect(
                owner,
                &SoftwareEffect::Static {
                    color: [255, 255, 255],
                }
                .engine_effect(),
            )
            .unwrap();
        engine.enable(owner, true, 0).unwrap();
        let frame = engine.frames(0, [None, None]).unwrap().pop().unwrap();
        assert_eq!(
            frame.frame.len(),
            usize::from(manifest.led.device_max_row) * usize::from(manifest.led.device_max_col) * 3
        );
    }
}
