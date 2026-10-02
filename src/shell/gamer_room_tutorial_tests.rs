use super::GamerRoomPage;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[gpui_kit::test]
fn adding_a_device_suspends_the_tutorial_and_restores_its_step(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1280.), px(1200.)), |window, cx| {
        let view = cx.new(|_| GamerRoomPage::new());
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("gamer-room-tutorial", cx);
        window.click("gr-tour-next", cx);
        window.click("gamer-room-add", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("gamer-room-tour").is_none());
        assert!(window.try_find("gamer-room-add-dialog").is_some());
        assert_eq!(page.read(cx).tour_step, Some(1));
        window.within("iot-preview-scene").click("input", cx);
        window.click("option-general", cx);
        assert_eq!(window.find("iot-preview-scene").expanded(), Some(false));
        window.click("gr-add-close", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("gamer-room-add-dialog").is_none());
        assert!(window.try_find("gamer-room-tour").is_some());
        assert_eq!(page.read(cx).tour_step, Some(1));
        assert_eq!(
            window.find("gr-tour-next").label(),
            Some(crate::i18n::t("DONE").to_uppercase().as_str())
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn gamer_room_steps_keep_source_controls_indicator_anchors_and_completion(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1280.), px(1200.)), |window, cx| {
        let view = cx.new(|_| GamerRoomPage::new());
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    let page = page.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("gamer-room-add").bounds().size,
            size(px(186.), px(176.))
        );
        for id in [
            "gr-hotspot-AETHER_LIGHT_BULBS",
            "gr-hotspot-AETHER_LIGHT_STRIP",
            "gr-hotspot-AETHER_LAMP_PRO",
        ] {
            assert_eq!(window.find(id).bounds().size.height, px(33.));
        }
        window.click("gamer-room-tutorial", cx);
        let first = window.find("gamer-room-tour").bounds();
        let indicator = window.find("gamer-room-tutorial-indicator").bounds();
        assert!(
            (indicator.center().x - (first.left() + px(1.) - (first.size.width - px(2.)) * 0.09))
                .abs()
                < px(0.5)
        );
        assert!((indicator.top() - first.top() - px(1.)).abs() < px(0.5));
        for id in ["gr-tour-back", "gr-tour-next"] {
            assert_eq!(window.find(id).bounds().size, size(px(100.), px(27.)));
        }
        assert_eq!(
            window.find("gr-tour-next").bounds().left()
                - window.find("gr-tour-back").bounds().right(),
            px(10.)
        );
        window.click("gr-tour-back", cx);
        assert_eq!(page.read(cx).tour_step, Some(0));
        window.click("gr-tour-next", cx);
        assert_eq!(page.read(cx).tour_step, Some(1));
        let second = window.find("gamer-room-tour").bounds();
        let indicator = window.find("gamer-room-tutorial-indicator").bounds();
        assert!(
            (indicator.center().x - (second.left() + px(1.) - (second.size.width - px(2.)) * 0.1))
                .abs()
                < px(0.5)
        );
        assert!((indicator.center().y - second.center().y).abs() < px(0.5));
        assert!((second.left() - first.left() + px(7.)).abs() < px(0.5));
        assert!((second.top() - first.top() + px(47.)).abs() < px(0.5));
        assert_eq!(
            window.find("gr-tour-next").label(),
            Some(crate::i18n::t("DONE").to_uppercase().as_str())
        );
        window.click("gr-tour-back", cx);
        assert_eq!(page.read(cx).tour_step, Some(0));
        window.click("gr-tour-next", cx);
        window.click("gr-tour-next", cx);
        assert_eq!(page.read(cx).tour_step, None);
        assert_eq!(page.read(cx).stored_seen, Some(true));
        assert!(window.try_find("gamer-room-tour").is_none());
        window.click("gamer-room-tutorial", cx);
        window.click("gr-tour-skip", cx);
        assert_eq!(page.read(cx).tour_step, None);
        assert!(window.try_find("gamer-room-tutorial-indicator").is_none());
    })
    .unwrap();
}
