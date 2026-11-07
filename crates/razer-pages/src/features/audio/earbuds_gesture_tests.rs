use super::{
    AudioProductWorkspace, Operation, finish_initial_restores, finish_pending_firmware, mount,
    reading,
};
use gpui_kit::{AppContext, TestAppContext, point, px, test::TestWindowExt};
use razer_device::{
    earbuds_gesture::{TapObservation, TapSpeed},
    earbuds_protocol40::ToggleObservation,
};

#[gpui_kit::test]
fn gesture_switch_preserves_failed_write_draft_and_view_link_stays_active(cx: &mut TestAppContext) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, window, cx| {
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            view.finish_earbuds_request(&request, Ok(reading(2660, false)), cx);
            finish_pending_firmware(view, cx);
            finish_initial_restores(view, view.spec.product_id, cx);
        });
        window.render_frame(cx);
        window.click("earbuds-gestures", cx);
        owner.update(cx, |view, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            assert_eq!(request.operation(), Operation::Gesture { enabled: false });
            assert!(request.needs_persistence());
            assert_eq!(
                view.earbuds.gesture.device,
                ToggleObservation::Observed(true)
            );
            view.mark_earbuds_persisted(&request);
            view.finish_earbuds_request(&request, Err("SET rejected".into()), cx);
            assert!(!view.earbuds.gesture.draft);
            assert_eq!(
                view.earbuds.gesture.device,
                ToggleObservation::Observed(true)
            );
            assert_eq!(view.earbuds.gesture.persisted, Some(false));
        });
        window.render_frame(cx);
        window.click(("earbuds-tap-speed", 30usize), cx);
        assert!(owner.read(cx).earbuds.pending.is_none());
        window.click("earbuds-view-gestures", cx);
        window.render_frame(cx);
        assert!(owner.read(cx).earbuds.gesture.popup.is_some());
        let bounds = window.find("earbuds-gesture-modal").bounds();
        assert_eq!(bounds.size.width, window.rem_size() * 40.);
        window.press("escape", cx);
        assert!(owner.read(cx).earbuds.gesture.popup.is_some());
        window.click("earbuds-gesture-modal", cx);
        assert!(owner.read(cx).earbuds.gesture.popup.is_some());
        window.click("earbuds-gesture-close", cx);
        assert!(owner.read(cx).earbuds.gesture.popup.is_none());
        window.click("earbuds-view-gestures", cx);
        window.click_at("earbuds-gesture-backdrop", point(px(1.), px(1.)), cx);
        assert!(owner.read(cx).earbuds.gesture.popup.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn tap_buttons_preserve_real_2668_omission_and_keep_state_until_ack(cx: &mut TestAppContext) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (window, owner, _) = mount(pid, cx);
        cx.update_window(window, |_, window, cx| {
            owner.update(cx, |view, cx| {
                let request = view.earbuds.pending.clone().unwrap();
                view.finish_earbuds_request(&request, Ok(reading(pid, false)), cx);
                finish_pending_firmware(view, cx);
                finish_initial_restores(view, view.spec.product_id, cx);
            });
            window.render_frame(cx);
            if pid == 2668 {
                assert!(window.try_find(("earbuds-tap-speed", 30usize)).is_none());
                return;
            }
            window.click(("earbuds-tap-speed", 30usize), cx);
            owner.update(cx, |view, cx| {
                let request = view.earbuds.pending.clone().unwrap();
                assert_eq!(
                    request.operation(),
                    Operation::TapSpeed {
                        speed: TapSpeed::Fast
                    }
                );
                assert_eq!(
                    view.earbuds.gesture.tap_device,
                    TapObservation::Observed(60)
                );
                assert_eq!(view.earbuds.gesture.tap_draft, TapSpeed::Fast);
                assert_eq!(view.earbuds.gesture.tap_persisted, Some(TapSpeed::Normal));
                view.mark_earbuds_persisted(&request);
                view.finish_earbuds_request(&request, Err("SET rejected".into()), cx);
                assert_eq!(
                    view.earbuds.gesture.tap_device,
                    TapObservation::Observed(60)
                );
                assert_eq!(view.earbuds.gesture.tap_draft, TapSpeed::Fast);
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn unrelated_response_cannot_satisfy_an_earbud_job(cx: &mut TestAppContext) {
    let (window, owner, _) = mount(2660, cx);
    cx.update_window(window, |_, _, cx| {
        owner.update(cx, |view: &mut AudioProductWorkspace, cx| {
            let request = view.earbuds.pending.clone().unwrap();
            let wrong = reading(2657, true);
            view.finish_earbuds_request(&request, Ok(wrong), cx);
            assert!(matches!(
                view.earbuds.gesture.device,
                ToggleObservation::Unobserved
            ));
            assert!(
                view.earbuds
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("another request"))
            );
            assert!(view.earbuds.pending.is_none());
        })
    })
    .unwrap();
}

#[gpui_kit::test]
fn enhancement_widgets_follow_the_mounted_source_column_order(cx: &mut TestAppContext) {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        let (window, _, _) = mount(pid, cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let left = window.find("earbuds-enhancement-left").bounds();
            let right = window.find("earbuds-enhancement-right").bounds();
            assert!(left.origin.x < right.origin.x);
            let (left_ids, right_ids) = if pid == 2657 {
                (
                    vec!["earbuds-anc", "earbuds-auto-pause", "earbuds-gestures"],
                    vec!["earbuds-do-not-disturb", "earbuds-voice-prompt"],
                )
            } else {
                assert!(window.try_find("earbuds-anc").is_none());
                (
                    vec!["earbuds-do-not-disturb", "earbuds-auto-pause"],
                    vec!["earbuds-gestures", "earbuds-voice-prompt"],
                )
            };
            for (column, ids) in [(left, left_ids), (right, right_ids)] {
                let mut previous = None;
                for id in ids {
                    let bounds = window.find(id).bounds();
                    assert!(
                        bounds.origin.x >= column.origin.x
                            && bounds.origin.x < column.origin.x + column.size.width
                    );
                    if let Some(y) = previous {
                        assert!(bounds.origin.y > y);
                    }
                    previous = Some(bounds.origin.y);
                }
            }
        })
        .unwrap();
    }
}
