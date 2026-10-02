use super::{AlertAction, AlertPlacement, SourceAlert};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use std::{cell::Cell, rc::Rc};

#[gpui_kit::test]
fn source_alert_keeps_product_placement_and_only_commits_explicit_action(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for (placement, center) in [
        (AlertPlacement::AboveCenter, false),
        (AlertPlacement::UpperCenter, true),
    ] {
        let calls = Rc::new(Cell::new(0));
        let called = calls.clone();
        let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
            let alert = SourceAlert::open(
                "Save mapping",
                "Unsaved mapping.\n\nDiscard removes these edits.",
                "alert-keep",
                vec![
                    AlertAction::new("alert-commit", "Save", move |_, _| {
                        called.set(called.get() + 1)
                    })
                    .primary(),
                ],
                placement,
                window,
                cx,
            );
            Root::new(alert, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let panel = window.find("source-save-alert").bounds();
            assert!((f32::from(panel.size.width) - 400.).abs() < 1.);
            let y = if center {
                panel.origin.y + panel.size.height / 2.
            } else {
                panel.bottom()
            };
            assert!((f32::from(y) - if center { 270. } else { 450. }).abs() < 1.);
            window.press("enter", cx);
            assert_eq!(
                calls.get(),
                0,
                "Enter on the modal itself is not an implicit save"
            );
            window.click("alert-commit", cx);
            assert_eq!(calls.get(), 1);
            assert!(window.try_find("source-save-alert").is_none());
        })
        .unwrap();
    }
}
