//! Compile-checked only: execution is prohibited by the workspace policy.
use super::{CalibrationObservation, GamepadProductWorkspace, state::CalibrationAction};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};

#[gpui_kit::test]
fn local_start_cancel_and_retry_keep_observations_scoped(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut view = None;
    let handle = cx.open_window(size(px(1100.), px(800.)), |window, cx| {
        let body = cx.new(|cx| {
            let mut body = GamepadProductWorkspace::new(2636, 0, window, cx);
            body.set_page("TAB_CALIBRATION", window, cx);
            body
        });
        view = Some(body.clone());
        Root::new(body, window, cx)
    });
    let view = view.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let initial = view.read(cx).snapshot();
        window.click("gamepad-calibrate-left", cx);
        let generation = view.read(cx).calibration_generation().unwrap();
        let state = &view.read(cx).calibration_state;
        assert_eq!(state.step, 1);
        assert_eq!(state.positions, [None, None]);
        assert!(!state.is_complete());
        assert_eq!(view.read(cx).snapshot(), initial);
        window.click("gamepad-calibration-stop", cx);
        assert_eq!(
            view.read(cx).calibration_intent().unwrap().action(),
            CalibrationAction::Stop
        );
        assert!(!view.update(cx, |body, cx| body.observe_calibration(
            CalibrationObservation::progress(generation, 1, 1, 6, true).unwrap(),
            window,
            cx
        )));
        window.click("gamepad-calibrate-right", cx);
        let generation = view.read(cx).calibration_generation().unwrap();
        assert!(view.update(cx, |body, cx| body.observe_calibration(
            CalibrationObservation::progress(generation, 1, 2, -1, false).unwrap(),
            window,
            cx
        )));
        window.render_frame(cx);
        assert!(window.find("gamepad-calibration-error").visible());
        window.click("gamepad-calibration-error-retry", cx);
        let intent = view.read(cx).calibration_intent().unwrap();
        assert_eq!(intent.action(), CalibrationAction::Start);
        assert_eq!(intent.part(), 2);
        assert!(intent.generation() > generation);
        assert_eq!(view.read(cx).calibration_state.step, 1);
        assert!(!view.read(cx).calibration_state.is_complete());
        assert!(window.try_find("gamepad-calibration-error").is_none());
    })
    .unwrap();
}

#[test]
fn valid_step_without_real_rotations_is_not_done() {
    use super::state::CalibrationState;
    let mut state = CalibrationState::default();
    let generation = state
        .request(CalibrationAction::Start, 1)
        .unwrap()
        .generation();
    assert!(state.observe(CalibrationObservation::progress(generation, 1, 1, 5, true).unwrap()));
    assert!(!state.is_complete());
    assert!(!state.observe(CalibrationObservation::position(generation, 1, 1, 1000., 0.).unwrap()));
    assert!(!state.observe(CalibrationObservation::progress(generation, 2, 2, 6, true).unwrap()));
    let mut sequence = 2;
    // Genuine-observation contract fixture, never seeded into production state.
    for _ in 0..3 {
        for segment in 0..=64 {
            let angle = f64::from(segment) * std::f64::consts::TAU / 64.;
            assert!(
                state.observe(
                    CalibrationObservation::position(
                        generation,
                        sequence,
                        1,
                        1000. * angle.cos(),
                        1000. * angle.sin()
                    )
                    .unwrap()
                )
            );
            sequence += 1;
        }
    }
    assert_eq!(state.rotations, 3);
    assert!(state.is_complete());
    assert!(state.observe(CalibrationObservation::unavailable(generation, sequence)));
    assert!(!state.is_complete());
    assert_eq!(state.positions, [None, None]);
    assert!(
        !state.observe(
            CalibrationObservation::progress(generation, sequence + 1, 1, 6, true).unwrap()
        )
    );
}
