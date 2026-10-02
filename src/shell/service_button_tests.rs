use super::{module_action, module_detail_action};
use crate::ui::surface;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{Root, Theme, h_flex, v_flex};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, Window, div, px, size,
};
use std::{cell::Cell, rc::Rc};

const LONG_ACTION: &str = "Launch firmware updater";
const DETAIL: &str = "More information";

struct ButtonSamples {
    enabled_clicks: Rc<Cell<usize>>,
    disabled_clicks: Rc<Cell<usize>>,
}

impl Render for ButtonSamples {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled_clicks = self.disabled_clicks.clone();
        let enabled_clicks = self.enabled_clicks.clone();
        v_flex()
            .items_start()
            .gap_4()
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        module_action("short-action", "Open", false, false, cx)
                            .on_click(move |_, _, _| enabled_clicks.set(enabled_clicks.get() + 1)),
                    )
                    .child(module_action("long-action", LONG_ACTION, false, false, cx))
                    .child(
                        div()
                            .id("action-text-measure")
                            .test_support()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(12.))
                            .whitespace_nowrap()
                            .child(LONG_ACTION.to_uppercase()),
                    ),
            )
            .child(
                h_flex()
                    .gap_4()
                    .child(module_detail_action("details-action", DETAIL, cx))
                    .child(
                        div()
                            .id("details-text-measure")
                            .test_support()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .whitespace_nowrap()
                            .child(DETAIL),
                    ),
            )
            .child(
                module_action("disabled-action", "Install", true, true, cx)
                    .on_click(move |_, _, _| disabled_clicks.set(disabled_clicks.get() + 1)),
            )
    }
}

#[gpui_kit::test]
fn module_actions_measure_the_real_source_text_and_keep_disabled_geometry(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_kit::init(cx));
    for scale in [1., 1.25] {
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16. * scale)));
        let disabled_clicks = Rc::new(Cell::new(0));
        let enabled_clicks = Rc::new(Cell::new(0));
        let handle = cx.open_window(size(px(1600.), px(500.)), |window, cx| {
            let view = cx.new(|_| ButtonSamples {
                enabled_clicks: enabled_clicks.clone(),
                disabled_clicks: disabled_clicks.clone(),
            });
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let short = window.find("short-action").bounds();
            let long = window.find("long-action").bounds();
            let text = window.find("action-text-measure").bounds();
            assert_eq!(
                window.find("long-action").label(),
                Some("LAUNCH FIRMWARE UPDATER")
            );
            assert_eq!(short.size, size(px(90. * scale), px(27. * scale)));
            assert_eq!(long.size.height, px(27. * scale));
            // 16px on each side and the two physical one-pixel borders.
            assert!((long.size.width - text.size.width - px(32. * scale + 2.)).abs() < px(0.5));
            assert!(long.size.width > short.size.width);
            let details = window.find("details-action").bounds();
            let details_text = window.find("details-text-measure").bounds();
            assert!((details.size.width - details_text.size.width).abs() < px(0.5));
            assert_eq!(window.find("disabled-action").bounds().size, short.size);
            window.click("short-action", cx);
            window.press("enter", cx);
            window.press("space", cx);
            assert_eq!(enabled_clicks.get(), 3);
            window.hover("disabled-action", cx);
            window.click("disabled-action", cx);
            assert_eq!(disabled_clicks.get(), 0);
            assert_eq!(window.find("disabled-action").bounds().size, short.size);
        })
        .unwrap();
    }
}
