use super::{DeviceKind, open};
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, TestAppContext, point, px, size};

#[gpui_kit::test]
fn upper_right_close_dismisses_each_wifi_flow_at_small_window_size(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    for kind in [
        DeviceKind::General,
        DeviceKind::GamerRoom,
        DeviceKind::KeyLight,
    ] {
        let mut popup = None;
        let handle = cx.open_window(size(px(700.), px(480.)), |window, cx| {
            let view = open(kind, window, cx);
            popup = Some(view.clone());
            Root::new(view, window, cx)
        });
        let popup = popup.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let panel = window.find("gamer-room-add-dialog").bounds();
            let close = window.find("gr-add-close").bounds();
            assert_eq!(close.size, size(px(36.), px(36.)));
            assert_eq!(close.right(), panel.right());
            assert_eq!(close.top(), panel.top());
            assert!(close.bottom() <= px(480.));
            window.click_at("gr-add-close", point(px(34.), px(2.)), cx);
            assert!(!popup.read(cx).is_open());
            assert!(window.try_find("gamer-room-add-dialog").is_none());
        })
        .unwrap();
    }
}
