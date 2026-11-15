use super::DeviceWorkspace;
use crate::nav::Tab;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[gpui_kit::test]
fn key_tip_653_uses_real_workspace_mapping_layers_and_source_geometry(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1280.), px(960.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::test_devices::keyboard(), true, window, cx));
        view.update(cx, |view, _| {
            view.settings_mut().hypershift_bindings.insert(
                "KEY_A".into(),
                r#"local-mapping:v1:{"kind":"keyboard","key":"KEY_B","modifiers":[],"turbo":null}"#
                    .into(),
            );
        });
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("keyboard-hit-KEY_A", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("keyboard-key-tip-name").label(), Some("A"));
        let key = window.find("keyboard-hit-KEY_A").bounds();
        let tip = window.find("keyboard-mapping-key-tip").bounds();
        assert_eq!(tip.bottom(), key.top() - px(5.));
        window.hover("keyboard-hit-KEY_FN", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("keyboard-key-tip-notice").label(),
            Some(razer_i18n::t("SECOND_KEY_TIP").as_str())
        );
        window.click("mapping-hypershift", cx);
        window.hover("keyboard-hit-KEY_A", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("keyboard-key-tip-name").label(), Some("B"));
        page.update(cx, |view, cx| view.set_page(Tab::Lighting, window, cx));
        window.render_frame(cx);
        assert!(window.try_find("keyboard-mapping-key-tip").is_none());
        window.remove_window();
    })
    .unwrap();
}

#[gpui_kit::test]
fn key_tip_653_invalidates_pending_hover_on_leave_and_connection_change(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1280.), px(960.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::test_devices::keyboard(), true, window, cx));
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("keyboard-hit-KEY_A", cx);
        window.hover("mapping-hypershift", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("keyboard-mapping-key-tip").is_none());
        page.update(cx, |view, cx| {
            view.observe_connection(
                Some(razer_model::model::DeviceConnectionObservation::HidPresent),
                cx,
            );
            view.hover_keyboard_key_tip("KEY_A", true, cx);
            assert!(view.keyboard_key_tip.pending.is_some());
            view.observe_connection(None, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("keyboard-mapping-key-tip").is_none());
        assert!(page.read(cx).keyboard_key_tip.pending.is_none());
        window.remove_window();
    })
    .unwrap();
}

#[gpui_kit::test]
fn key_tip_653_media_popup_keeps_direction_order_text_anchor_and_dirty_cancel(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1280.), px(960.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::test_devices::keyboard(), true, window, cx));
        view.update(cx, |view, _| {
            view.settings_mut().bindings.insert(
                "ScrollUp".into(),
                r#"local-mapping:v1:{"kind":"keyboard","key":"KEY_C","modifiers":[],"turbo":null}"#
                    .into(),
            );
        });
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("keyboard-hit-DIAL_CLICK", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let popup = window.find("keyboard-media-popup").bounds();
        let anchor = window.find("keyboard-hit-DIAL_CLICK").bounds();
        assert_eq!(popup.size, size(px(234.), px(143.)));
        assert_eq!(popup.left(), anchor.left() - px(10.));
        assert_eq!(
            popup.top(),
            page.read(cx).customize_drawer.body_scroll().bounds().top() + px(95.)
        );
        assert!(window.try_find("keyboard-mapping-key-tip").is_none());
        assert!(
            window.find("keyboard-media-ScrollUp").bounds().top()
                < window.find("keyboard-media-ScrollDown").bounds().top()
        );
        window.hover("keyboard-media-label-ScrollUp", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("keyboard-key-tip-name").label(), Some("C"));
        let label = window.find("keyboard-media-label-ScrollUp").bounds();
        let tip = window.find("keyboard-mapping-key-tip").bounds();
        assert_eq!(tip.bottom(), label.top() - px(5.));
        window.click("keyboard-media-ScrollUp", cx);
        assert_eq!(page.read(cx).mapping.as_ref().unwrap().input, "ScrollUp");
        assert!(page.read(cx).keyboard_key_tip.media_shown);
        window.click("mapping-category-mouse", cx);
        assert!(page.read(cx).keyboard_key_tip.media_shown);
        assert!(page.read(cx).mapping_dirty());
        window.hover("mapping-hypershift", cx);
        assert!(window.find("keyboard-media-popup").visible());
        window.click("mapping-cancel", cx);
        window.click("mapping-keep-editing", cx);
        assert_eq!(page.read(cx).mapping.as_ref().unwrap().input, "ScrollUp");
        window.click("mapping-cancel", cx);
        window.click("mapping-discard", cx);
        assert!(page.read(cx).mapping.is_none());
        assert!(page.read(cx).settings().bindings["ScrollUp"].contains("KEY_C"));
        window.hover("mapping-hypershift", cx);
        window.render_frame(cx);
        assert!(window.try_find("keyboard-media-popup").is_none());
        window.click("customize-drawer-toggle", cx);
        window.hover("keyboard-hit-DIAL_CLICK", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("keyboard-media-popup").bounds().top(),
            window.find("keyboard-hit-DIAL_CLICK").bounds().top() - px(20.)
        );
        page.update(cx, |view, cx| view.observe_keyboard_metadata(None, cx));
        window.render_frame(cx);
        assert!(window.try_find("keyboard-media-popup").is_none());
        window.remove_window();
    })
    .unwrap();
}
