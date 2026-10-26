use super::*;

fn ambient(rows: usize, columns: usize, blur: f32) -> Ambient {
    Ambient::new(
        &EffectParameters {
            screen_rect: Some(ScreenRect::FULL),
            blur: Some(blur),
            ..Default::default()
        },
        rows,
        columns,
    )
    .unwrap()
}

#[test]
fn ambient_deriche_matches_order_zero_two_pass_byte_vectors() {
    // 0x5d9d0: sigma=1, alpha=1.6950000524520874, ema=0.1835992187,
    // boundary coefficients 0.7097669244 / 0.2902331352. Each axis truncates.
    for (input, expected) in [
        ([255, 0, 0], [180, 74, 21]),
        ([0, 255, 0], [52, 106, 52]),
        ([0, 0, 255], [21, 74, 180]),
        ([255, 255, 255], [255; 3]),
    ] {
        let mut actual = input;
        deriche(&mut actual, 1.);
        assert_eq!(actual, expected);
    }
    let mut renderer = ambient(3, 3, 1.);
    let mut pixels = vec![[0; 3]; 9];
    pixels[4] = [255, 0, 0];
    let mut output = vec![[0; 4]; 9];
    renderer
        .render(
            &mut output,
            3,
            3,
            Some(&ScreenObservation::Captured {
                frame_id: 1,
                pixels,
            }),
        )
        .unwrap();
    assert_eq!(
        output.iter().map(|p| p[0]).collect::<Vec<_>>(),
        [10, 22, 10, 21, 44, 21, 10, 22, 10]
    );
}

#[test]
fn ambient_frame_identity_failure_and_reset_preserve_only_actual_cached_output() {
    let mut renderer = ambient(1, 1, 0.);
    assert_eq!((renderer.request.rows, renderer.request.columns), (3, 3));
    let mut output = vec![[9; 4]];
    assert!(renderer.render(&mut output, 1, 1, None).is_err());
    renderer
        .render(&mut output, 1, 1, Some(&ScreenObservation::Unchanged))
        .unwrap();
    assert_eq!(output, [[0; 4]]);
    let mut captured = vec![[0; 3]; 9];
    captured[0] = [255, 0, 0];
    captured[4] = [17, 34, 51];
    renderer
        .render(
            &mut output,
            1,
            1,
            Some(&ScreenObservation::Captured {
                frame_id: 7,
                pixels: captured,
            }),
        )
        .unwrap();
    assert_eq!(output, [[17, 34, 51, 0]]);
    renderer
        .render(
            &mut output,
            1,
            1,
            Some(&ScreenObservation::Captured {
                frame_id: 7,
                pixels: vec![[255; 3]; 9],
            }),
        )
        .unwrap();
    assert_eq!(output, [[17, 34, 51, 0]]);
    renderer
        .render(
            &mut output,
            1,
            1,
            Some(&ScreenObservation::Failed {
                frame_id: Some(8),
                error: "sample failed".into(),
            }),
        )
        .unwrap();
    renderer
        .render(
            &mut output,
            1,
            1,
            Some(&ScreenObservation::Captured {
                frame_id: 8,
                pixels: vec![[255; 3]; 9],
            }),
        )
        .unwrap();
    assert_eq!(output, [[17, 34, 51, 0]]);
    renderer
        .render(
            &mut output,
            1,
            1,
            Some(&ScreenObservation::Captured {
                frame_id: 9,
                pixels: vec![[1, 2, 3]; 9],
            }),
        )
        .unwrap();
    assert_eq!(output, [[1, 2, 3, 0]]);
}

#[test]
fn ambient_capture_crop_and_nearest_resize_follow_the_native_integer_boundary() {
    let mut request = ScreenRequest {
        rect: ScreenRect::FULL,
        rows: 3,
        columns: 3,
    };
    let rgb = (0..16).map(|v| [v, 0, 0]).collect::<Vec<_>>();
    assert_eq!(
        request
            .sample_rgb(4, 4, &rgb)
            .unwrap()
            .iter()
            .map(|p| p[0])
            .collect::<Vec<_>>(),
        [0, 1, 2, 4, 5, 6, 8, 9, 10]
    );
    request.rect = ScreenRect {
        left: 32768,
        top: 0,
        right: 65535,
        bottom: 65535,
    };
    assert_eq!(request.pixel_rect(4, 4).unwrap(), [2, 0, 4, 4]);
    assert_eq!(
        request
            .sample_rgb(4, 4, &rgb)
            .unwrap()
            .iter()
            .map(|p| p[0])
            .collect::<Vec<_>>(),
        [2, 2, 3, 6, 6, 7, 10, 10, 11]
    );
    request.rect = ScreenRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    assert_eq!(request.pixel_rect(4, 4).unwrap(), [0, 0, 4, 4]);
    request.rect = ScreenRect {
        left: 0,
        top: 0,
        right: 6553,
        bottom: 65535,
    };
    assert!(request.pixel_rect(4, 4).is_err());
    assert!(request.sample_rgb(4, 4, &rgb[..15]).is_err());
}

#[test]
fn ambient_engine_observation_is_explicit_and_destroy_releases_capture_demand() {
    use super::super::Engine;
    use crate::lighting_engine::{EngineEffect, LedMapping};
    let mut geometry = crate::marci_lighting::led_configuration();
    geometry.matrix_max_row = 3;
    geometry.matrix_max_col = 3;
    geometry.device_max_row = 3;
    geometry.device_max_col = 3;
    geometry.led_matrix = (0..3)
        .flat_map(|r| {
            (0..3).map(move |c| LedMapping {
                matrix_pos: [r, c],
                device_pos: [r, c],
            })
        })
        .collect();
    let mut engine = Engine::new(0);
    let device = engine.create_device(&geometry).unwrap();
    let instance = engine.create_engine().unwrap();
    engine.add_device(instance, device).unwrap();
    let effect = engine
        .add_effect(
            instance,
            &EngineEffect {
                effect: 11,
                parameters: EffectParameters {
                    screen_rect: Some(ScreenRect::FULL),
                    blur: Some(0.),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    assert!(engine.has_ambient());
    engine.enable(instance, true, 0).unwrap();
    assert_eq!(engine.screen_requests(0).len(), 1);
    let samples = std::collections::BTreeMap::from([(
        effect,
        ScreenObservation::Captured {
            frame_id: 1,
            pixels: vec![[255, 0, 0]; 9],
        },
    )]);
    assert_eq!(
        engine.frames_with_screen(0, [None; 2], &samples).unwrap()[0].frame,
        [3, 0, 0].repeat(9)
    );
    assert!(engine.screen_requests(1).is_empty());
    engine.enable(instance, false, 1).unwrap();
    assert!(engine.screen_requests(100).is_empty());
    engine.destroy_engine(instance).unwrap();
    assert!(!engine.has_ambient());
    engine.destroy_device(device).unwrap();
    engine.terminate();
}
