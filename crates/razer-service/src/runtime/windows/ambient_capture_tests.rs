use super::*;
use std::{cell::Cell, collections::VecDeque};

pub(super) struct Scripted {
    refreshes: VecDeque<Refresh>,
    calls: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
    pixels: DesktopPixels,
}
enum Refresh {
    Captured,
    Unchanged,
    Reconstructed,
    Failed(&'static str),
}
impl Scripted {
    pub(super) fn refresh(&mut self) -> anyhow::Result<bool> {
        self.calls.set(self.calls.get() + 1);
        match self
            .refreshes
            .pop_front()
            .expect("unexpected capture refresh")
        {
            Refresh::Captured => {
                self.pixels = DesktopPixels::Captured {
                    width: 3,
                    height: 3,
                    rgb: vec![[17, 34, 51]; 9],
                };
                Ok(true)
            }
            Refresh::Unchanged => Ok(false),
            Refresh::Reconstructed => {
                self.pixels = DesktopPixels::Unobserved;
                Ok(false)
            }
            Refresh::Failed(error) => Err(anyhow::anyhow!(error)),
        }
    }
    pub(super) fn sample(&self, request: ScreenRequest) -> anyhow::Result<Vec<[u8; 3]>> {
        self.pixels.sample(request)
    }
}
impl Drop for Scripted {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn capture(
    refreshes: Vec<Result<bool, &'static str>>,
) -> (Capture, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    scripted(
        refreshes
            .into_iter()
            .map(|result| match result {
                Ok(true) => Refresh::Captured,
                Ok(false) => Refresh::Unchanged,
                Err(error) => Refresh::Failed(error),
            })
            .collect(),
    )
}
fn scripted(refreshes: Vec<Refresh>) -> (Capture, Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    (
        Capture::new(
            ScreenBackend::DesktopDuplication,
            Backend::Scripted(Scripted {
                refreshes: refreshes.into(),
                calls: calls.clone(),
                drops: drops.clone(),
                pixels: DesktopPixels::Unobserved,
            }),
        ),
        calls,
        drops,
    )
}
fn request() -> ScreenRequest {
    ScreenRequest {
        rect: razer_device::lighting_engine::ScreenRect::FULL,
        rows: 3,
        columns: 3,
    }
}

#[test]
fn ambient_capture_all_effects_share_one_thirty_millisecond_refresh() {
    let (mut capture, calls, drops) = capture(vec![Ok(true), Ok(true)]);
    let requests = [(7, request()), (9, request())];
    let first = capture.observe(&requests, 0);
    assert!(matches!(
        first[&7],
        ScreenObservation::Captured { frame_id: 1, .. }
    ));
    assert_eq!(first[&7], first[&9]);
    assert_eq!(
        capture.observe(&requests, 29)[&7],
        ScreenObservation::Unchanged
    );
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        capture.observe(&requests, 30)[&9],
        ScreenObservation::Captured { frame_id: 2, .. }
    ));
    assert_eq!(calls.get(), 2);
    drop(capture);
    assert_eq!(drops.get(), 1);
}

#[test]
fn ambient_capture_first_timeout_keeps_desktop_unobserved() {
    let (mut capture, calls, _) = capture(vec![Ok(false), Ok(true)]);
    let requests = [(1, request())];
    assert_eq!(
        capture.observe(&requests, 0)[&1],
        ScreenObservation::Unchanged
    );
    assert_eq!(capture.status().frame_id, None);
    assert_eq!(capture.status().refreshed_at_ms, None);
    assert_eq!(
        capture.observe(&requests, 29)[&1],
        ScreenObservation::Unchanged
    );
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        capture.observe(&requests, 30)[&1],
        ScreenObservation::Captured { frame_id: 1, .. }
    ));
    assert_eq!(capture.status().refreshed_at_ms, Some(30));
}

#[test]
fn ambient_capture_failure_recovers_without_faking_observation_and_release_is_owned() {
    let (mut capture, calls, drops) = capture(vec![Ok(true), Err("access lost"), Ok(true)]);
    let requests = [(1, request())];
    capture.observe(&requests, 0);
    assert_eq!(
        capture.observe(&requests, 30)[&1],
        ScreenObservation::Failed {
            frame_id: None,
            error: "access lost".into()
        }
    );
    assert_eq!(capture.status().frame_id, Some(1));
    assert_eq!(capture.status().refreshed_at_ms, Some(0));
    assert_eq!(capture.status().last_error.as_deref(), Some("access lost"));
    assert_eq!(
        capture.observe(&requests, 31)[&1],
        ScreenObservation::Unchanged
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(capture.status().last_error.as_deref(), Some("access lost"));
    capture.observe(&requests, 60);
    assert_eq!(capture.status().frame_id, Some(2));
    assert_eq!(capture.status().last_error, None);
    capture.close().unwrap();
    capture.close().unwrap();
    assert!(capture.status().released);
    assert_eq!(drops.get(), 1);
    assert!(matches!(
        capture.observe(&requests, 90)[&1],
        ScreenObservation::Failed { frame_id: None, .. }
    ));
    assert_eq!(calls.get(), 3);
}

#[test]
fn ambient_capture_pitch_copy_and_source_right_angle_rotation_preserve_rgb() {
    let bytes = [
        3, 2, 1, 255, 6, 5, 4, 255, 0, 0, 0, 0, 9, 8, 7, 255, 12, 11, 10, 255, 0, 0, 0, 0,
    ];
    // The sample has two live rows, each with eight pixels bytes and four
    // bytes of native pitch padding, as in 0x74d70/0x81ba0.
    assert_eq!(
        unsafe { copy_bgra(bytes.as_ptr(), 12, 2, 2) }.unwrap(),
        [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]]
    );
    assert!(unsafe { copy_bgra(bytes.as_ptr(), 7, 2, 2) }.is_err());
    let pixels = (0..6).map(|v| [v, 0, 0]).collect::<Vec<_>>();
    for (angle, width, height, expected) in [
        (0, 3, 2, vec![0, 1, 2, 3, 4, 5]),
        (90, 2, 3, vec![3, 0, 4, 1, 5, 2]),
        (180, 3, 2, vec![5, 4, 3, 2, 1, 0]),
        (270, 2, 3, vec![2, 5, 1, 4, 0, 3]),
    ] {
        let (w, h, actual) = rotate_rgb(3, 2, &pixels, angle).unwrap();
        assert_eq!((w, h), (width, height));
        assert_eq!(actual.iter().map(|p| p[0]).collect::<Vec<_>>(), expected);
    }
    assert!(rotate_rgb(3, 2, &pixels, 45).is_err());
}

#[test]
fn ambient_capture_gdi_consumers_keep_postincrement_order_and_failed_sample_identity() {
    let (mut capture, calls, _) = capture(vec![Ok(true), Ok(true), Ok(true)]);
    capture.status.backend = ScreenBackend::Gdi;
    capture.next_frame_id = 0;
    // The second request is intentionally empty: a native sample failure is
    // committed before returning cached output, and is not retried for its ID.
    let mut invalid = request();
    invalid.rect.right = 0;
    let requests = [(9, request()), (7, invalid)];
    let first = capture.observe(&requests, 0);
    assert!(matches!(
        first[&9],
        ScreenObservation::Captured { frame_id: 0, .. }
    ));
    assert!(matches!(
        first[&7],
        ScreenObservation::Failed {
            frame_id: Some(1),
            ..
        }
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(capture.status().frame_id, Some(0));
    assert_eq!(capture.status().refreshed_at_ms, Some(0));
    assert!(capture.status().last_error.is_some());
    let inside = capture.observe(&requests, 29);
    assert!(matches!(
        inside[&9],
        ScreenObservation::Captured { frame_id: 1, .. }
    ));
    assert_eq!(inside[&7], ScreenObservation::Unchanged);
    assert_eq!(calls.get(), 1);
    assert!(
        capture
            .status()
            .last_error
            .as_ref()
            .unwrap()
            .contains("Effect 7")
    );
    let boundary = capture.observe(&requests, 30);
    assert_eq!(boundary[&9], ScreenObservation::Unchanged);
    assert!(matches!(
        boundary[&7],
        ScreenObservation::Failed {
            frame_id: Some(2),
            ..
        }
    ));
    assert_eq!(calls.get(), 2);
    let repaired = [(9, request()), (7, request())];
    assert_eq!(
        capture.observe(&repaired, 31)[&7],
        ScreenObservation::Unchanged
    );
    assert_eq!(capture.status().frame_id, Some(2));
    assert!(
        capture
            .status()
            .last_error
            .as_ref()
            .unwrap()
            .contains("Effect 7")
    );
    // Reordering consumers changes which one receives the old counter. The
    // official caller's traversal order, not handle sorting, owns this fact.
    let reversed = [(7, request()), (9, request())];
    let next = capture.observe(&reversed, 60);
    assert_eq!(next[&7], ScreenObservation::Unchanged);
    assert!(matches!(
        next[&9],
        ScreenObservation::Captured { frame_id: 3, .. }
    ));
    assert_eq!(calls.get(), 3);
    assert_eq!(capture.status().frame_id, Some(3));
    assert!(
        capture
            .status()
            .last_error
            .as_ref()
            .unwrap()
            .contains("Effect 7")
    );
    let recovered = capture.observe(&reversed, 61);
    assert!(matches!(
        recovered[&7],
        ScreenObservation::Captured { frame_id: 3, .. }
    ));
    assert_eq!(capture.status().last_error, None);
}

#[test]
fn ambient_capture_reconstruction_timeout_preserves_old_output_and_new_unobserved_output() {
    use razer_device::lighting_engine::{
        EffectParameters, EngineEffect, ScreenRect, renderer::Renderer,
    };
    let (mut capture, _, _) = scripted(vec![
        Refresh::Captured,
        Refresh::Reconstructed,
        Refresh::Unchanged,
        Refresh::Captured,
    ]);
    let effect = EngineEffect {
        effect: 11,
        parameters: EffectParameters {
            screen_rect: Some(ScreenRect::FULL),
            blur: Some(0.),
            ..Default::default()
        },
    };
    let mut seed = 0;
    let mut old = Renderer::new(&effect, 1, 1, 0, &mut seed).unwrap();
    let mut new = Renderer::new(&effect, 1, 1, 0, &mut seed).unwrap();
    let mut old_output = vec![[0; 3]];
    let mut new_output = vec![[0; 3]];
    let first = capture.observe(&[(1, request())], 0);
    old.render_with_screen(&mut old_output, 1., None, Some(&first[&1]))
        .unwrap();
    assert_eq!(old_output, [[17, 34, 51]]);
    let requests = [(1, request()), (2, request())];
    let rebuilt = capture.observe(&requests, 30);
    assert_eq!(rebuilt[&1], ScreenObservation::Unchanged);
    assert!(matches!(
        rebuilt[&2],
        ScreenObservation::Failed {
            frame_id: Some(1),
            ..
        }
    ));
    old.render_with_screen(&mut old_output, 1., None, Some(&rebuilt[&1]))
        .unwrap();
    new.render_with_screen(&mut new_output, 1., None, Some(&rebuilt[&2]))
        .unwrap();
    assert_eq!(old_output, [[17, 34, 51]]);
    assert_eq!(new_output, [[0; 3]]);
    assert_eq!(capture.status().frame_id, Some(1));
    assert_eq!(capture.status().refreshed_at_ms, Some(0));
    assert!(
        capture
            .status()
            .last_error
            .as_ref()
            .unwrap()
            .contains("Effect 2")
    );
    assert_eq!(
        capture.observe(&requests, 60)[&2],
        ScreenObservation::Unchanged
    );
    assert!(
        capture
            .status()
            .last_error
            .as_ref()
            .unwrap()
            .contains("Effect 2")
    );
    let observed = capture.observe(&requests, 90);
    for (renderer, output, handle) in [
        (&mut old, &mut old_output, 1),
        (&mut new, &mut new_output, 2),
    ] {
        assert!(matches!(
            observed[&handle],
            ScreenObservation::Captured { frame_id: 2, .. }
        ));
        renderer
            .render_with_screen(output, 1., None, Some(&observed[&handle]))
            .unwrap();
        assert_eq!(*output, [[17, 34, 51]]);
    }
    assert_eq!(capture.status().refreshed_at_ms, Some(90));
    assert_eq!(capture.status().last_error, None);
}

#[test]
fn ambient_capture_backend_and_per_effect_errors_have_independent_recovery_and_retirement() {
    let (mut capture, _, _) = capture(vec![Ok(true), Err("GPU mapping failed"), Ok(true)]);
    let mut invalid = request();
    invalid.rect.right = 0;
    let requests = [(7, invalid), (9, request())];
    capture.observe(&requests, 0);
    assert!(
        capture
            .status()
            .last_error
            .as_ref()
            .unwrap()
            .contains("Effect 7")
    );
    capture.observe(&requests, 30);
    let error = capture.status().last_error.unwrap();
    assert!(error.contains("GPU mapping failed") && error.contains("Effect 7"));
    capture.retain_effects(&[9]);
    assert_eq!(
        capture.status().last_error.as_deref(),
        Some("GPU mapping failed")
    );
    assert_eq!(
        capture.observe(&[(9, request())], 31)[&9],
        ScreenObservation::Unchanged
    );
    assert_eq!(
        capture.status().last_error.as_deref(),
        Some("GPU mapping failed")
    );
    capture.observe(&[(9, request())], 60);
    assert_eq!(capture.status().last_error, None);
}
