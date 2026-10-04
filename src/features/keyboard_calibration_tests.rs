use super::{CalibrationPreview, KeyboardProductWorkspace, Sample};
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{App, AppContext, ElementId, TestAppContext, px, size};

// Compiled by cargo check --all-targets. Repository policy forbids executing
// these integration tests during the source-only reconstruction task.
#[gpui_kit::test]
fn calibration_open_cancel_and_keyboard_never_modify_profile(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for (pid, scale) in [(740, 1.), (746, 1.25)] {
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16. * scale)));
        let mut entity = None;
        let handle = cx.open_window(size(px(1280. * scale), px(1100. * scale)), |window, cx| {
            let view = cx.new(|cx| KeyboardProductWorkspace::new(pid, false, window, cx));
            view.update(cx, |view, cx| view.set_page("TAB_CALIBRATION", window, cx));
            entity = Some(view.clone());
            Root::new(view, window, cx)
        });
        let entity = entity.unwrap();
        let before = cx.update(|cx| entity.read(cx).snapshot());
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("keyboard-calibration-intro-close", cx);
            window.click("keyboard-calibration-start", cx);
            let panel = window.find("keyboard-calibration-modal").bounds();
            assert!((panel.size.width - px(850. * scale)).abs() < px(1.));
            assert!((panel.top() - px(100. * scale)).abs() < px(1.));
            assert!((panel.bottom() - px(1100. * scale)).abs() < px(1.));
            window.press("a", cx);
            window.click("keyboard-calibration-next", cx);
            assert!(window.try_find("keyboard-calibration-modal").is_some());
            window.press("escape", cx);
            assert!(window.try_find("keyboard-calibration-modal").is_none());
            assert_eq!(
                window.find("keyboard-calibration-start").focused(),
                Some(true)
            );
            window.click("keyboard-calibration-start", cx);
            window.click("keyboard-calibration-cancel", cx);
            assert!(window.try_find("keyboard-calibration-modal").is_none());
        })
        .unwrap();
        cx.update(|cx| assert_eq!(entity.read(cx).snapshot(), before));
    }
}

#[gpui_kit::test]
fn preview_shows_full_product_page_and_waits_for_explicit_samples(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut entity = None;
    let handle = cx.open_window(size(px(1280.), px(1100.)), |window, cx| {
        let view = cx.new(|cx| CalibrationPreview::new(740, window, cx));
        entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let entity = entity.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("keyboard-calibration-modal").is_none());
        assert!(
            window
                .try_find("keyboard-calibration-intro-close")
                .is_some()
        );
        assert!(window.try_find("keyboard-calibration-start").is_some());
        assert!(window.try_find("source-profile-select").is_some());
        assert!(window.try_find("source-product-navigation").is_some());

        window.click((ElementId::from("calibration-product"), "746"), cx);
        assert_eq!(entity.read(cx).product_id, 746);
        assert!(window.try_find("keyboard-calibration-modal").is_none());
        assert!(
            window
                .try_find("keyboard-calibration-intro-close")
                .is_some()
        );
        let keyboard = entity.read(cx).keyboard.clone();
        let before = keyboard.read(cx).snapshot();
        window.click("keyboard-calibration-start", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::SelectKey));
        window.press("escape", cx);
        assert!(window.try_find("keyboard-calibration-modal").is_none());
        assert!(window.try_find("keyboard-calibration-start").is_some());

        window.click((ElementId::from("calibration-sample"), "selected"), cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::KeySelected));
        window.click("keyboard-calibration-next", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::PressKey));
        window.click("keyboard-calibration-next", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::VerifyBottom));
        window.click("keyboard-calibration-next", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::VerifyBottom));
        window.click("keyboard-calibration-cancel", cx);
        assert!(window.try_find("keyboard-calibration-modal").is_none());

        window.click((ElementId::from("calibration-sample"), "release"), cx);
        window.click("keyboard-calibration-next", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::CalibrateTop));
        window.click("keyboard-calibration-next", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::CalibrateTop));
        window.click("keyboard-calibration-cancel", cx);
        assert!(window.try_find("keyboard-calibration-modal").is_none());
        assert!(window.try_find("keyboard-calibration-start").is_some());

        for (expected, id, _) in Sample::ALL {
            window.click((ElementId::from("calibration-sample"), id), cx);
            assert!(sample(entity.read(cx), cx) == Some(expected));
            window.click("keyboard-calibration-cancel", cx);
        }
        window.click((ElementId::from("calibration-sample"), "failure"), cx);
        window.click("keyboard-calibration-next", cx);
        assert!(sample(entity.read(cx), cx) == Some(Sample::SelectKey));
        window.click("keyboard-calibration-cancel", cx);
        window.click((ElementId::from("calibration-sample"), "success"), cx);
        window.click("keyboard-calibration-next", cx);
        assert!(window.try_find("keyboard-calibration-modal").is_none());
        assert_eq!(keyboard.read(cx).snapshot(), before);

        window.click((ElementId::from("calibration-product"), "740"), cx);
        assert_eq!(entity.read(cx).product_id, 740);
        assert!(window.try_find("keyboard-calibration-modal").is_none());
        assert!(window.try_find("keyboard-calibration-start").is_some());
    })
    .unwrap();
}

fn sample(preview: &CalibrationPreview, cx: &App) -> Option<Sample> {
    preview
        .keyboard
        .read(cx)
        .calibration_modal
        .as_ref()
        .and_then(|modal| {
            let modal = modal.read(cx);
            modal.open.then_some(modal.sample).flatten()
        })
}
