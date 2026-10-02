//! 4130's `Si`: one introduction to the second main-navigation destination.
//! Tour is a separate application and must not reuse this tutorial's seen flag.
use crate::{
    i18n,
    ui::{surface, theme::MainPageColors},
};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::*;

pub(in crate::shell) struct DashboardTutorial {
    seen: bool,
}

pub(in crate::shell) enum DashboardTutorialEvent {
    Completed,
}

impl EventEmitter<DashboardTutorialEvent> for DashboardTutorial {}

impl DashboardTutorial {
    pub(in crate::shell) fn new(seen: bool) -> Self {
        Self { seen }
    }

    pub(in crate::shell) fn set_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if self.seen != seen {
            self.seen = seen;
            cx.notify();
        }
    }

    pub(in crate::shell) fn reset(&mut self, cx: &mut Context<Self>) {
        self.set_seen(false, cx);
    }

    fn complete(&mut self, cx: &mut Context<Self>) {
        if !self.seen {
            self.seen = true;
            cx.emit(DashboardTutorialEvent::Completed);
            cx.notify();
        }
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("dashboard-tutorial-content")
            .aria_label(i18n::t("DASHBOARD_TUTORIAL_HEADER"))
            .relative()
            .w(surface::css(290.))
            .p(surface::css(20.))
            .rounded(surface::css(5.))
            .border_1()
            .border_color(MainPageColors.tutorial_accent())
            .bg(cx.theme().group_box)
            .text_color(cx.theme().foreground)
            .whitespace_normal()
            .child(
                img("synapse/gr-tutorial-indicator.svg")
                    .absolute()
                    .left(surface::css(124.))
                    .top(surface::css(-38.))
                    .size(surface::css(36.)),
            )
            .child(
                v_flex()
                    .w_full()
                    .h(surface::css(140.))
                    .items_center()
                    .justify_center()
                    .gap(surface::css(10.))
                    .text_size(surface::css(14.))
                    .bg(MainPageColors.banner_shade())
                    .child("教程视频")
                    .child(
                        gpui_kit::base::Link::new("dashboard-tutorial-video")
                            .href("https://apps.razer.com/synapse/dashboard/static/media/Synapse%20Dashboard%20Tutorial.4d4e2f9c.mp4")
                            .open_with(|url, _, _, cx| cx.open_url(url))
                            .accessibility_label("在浏览器中播放 Dashboard 教程视频")
                            .cursor_pointer()
                            .underline()
                            .text_color(cx.theme().foreground)
                            .hover(|view| view.text_color(cx.theme().primary))
                            .focus_visible(|view| view.text_color(cx.theme().primary))
                            .child("在浏览器中播放"),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .text_size(surface::css(16.))
                    .text_color(MainPageColors.tutorial_accent())
                    .mb(surface::css(5.))
                    .child(i18n::t("DASHBOARD_TUTORIAL_HEADER")),
            )
            .child(
                div()
                    .w_full()
                    .text_size(surface::css(14.))
                    .child(i18n::t("DASHBOARD_TUTORIAL_HEADER_DESC")),
            )
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .justify_center()
                    .child(
                        Button::new("dashboard-tutorial-close")
                            .label(i18n::t("CLOSE").to_uppercase())
                            .xsmall()
                            .w(surface::css(100.))
                            .h(surface::css(27.))
                            .ml(surface::css(5.))
                            .p_0()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .rounded(cx.theme().font_size * (3. / 16.))
                            .border_1()
                            .border_color(MainPageColors.banner_shade())
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(MainPageColors.tutorial_accent())
                                    .foreground(MainPageColors.banner_shade())
                                    .hover(MainPageColors.tutorial_hover())
                                    .active(MainPageColors.tutorial_hover()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.complete(cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for DashboardTutorial {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        // The owner is a full-height wrapper around the second navigation tab.
        // Frontend toolbar 38 + this 70 = source fixed top 108; the native
        // host's 42px title bar is already outside that coordinate system.
        // Indicator center: -92 + 124 + 18 = tab left + 50; 108 - 38 = 70.
        div()
            .absolute()
            .left(surface::css(-92.))
            .top(surface::css(70.))
            .child(
                gpui_kit::base::Popover::new("dashboard-tutorial")
                    .anchor(Anchor::TopLeft)
                    .overlay_closable(false)
                    .open(!self.seen)
                    .trigger_with(|_, _, _| div().size_0().into_any_element())
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        if !*open {
                            this.complete(cx);
                        }
                    }))
                    .content(move |_, _, cx| {
                        owner
                            .update(cx, |this, cx| this.content(cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            )
    }
}
