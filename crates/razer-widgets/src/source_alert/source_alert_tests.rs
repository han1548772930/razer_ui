use super::{AlertAction, AlertPlacement, SourceAlert};
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, point, px, size};
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

#[gpui_kit::test]
fn alert_corner_close_keeps_edits_without_invoking_actions(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let calls = Rc::new(Cell::new(0));
    let called = calls.clone();
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let alert = SourceAlert::open(
            "Save mapping",
            "Unsaved changes",
            "keep-editing",
            vec![AlertAction::new("commit", "Save", move |_, _| {
                called.set(called.get() + 1)
            })],
            AlertPlacement::AboveCenter,
            window,
            cx,
        );
        Root::new(alert, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let panel = window.find("source-save-alert").bounds();
        let close = window.find("keep-editing").bounds();
        assert_eq!(close.size, size(px(20.), px(20.)));
        assert!((panel.right() - close.right() - px(9.)).abs() < px(0.5));
        assert!((close.top() - panel.top() - px(9.)).abs() < px(0.5));
        window.click_at("keep-editing", point(px(18.), px(2.)), cx);
        assert!(window.try_find("source-save-alert").is_none());
        assert_eq!(calls.get(), 0);
    })
    .unwrap();
}
