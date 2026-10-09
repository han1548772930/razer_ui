//! 4130's `Si`: one introduction to the second main-navigation destination.
//! Tour is a separate application and must not reuse this tutorial's seen flag.
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use razer_widgets::theme::MainPageColors;
use razer_widgets::tutorial::TutorialIndicator;
use razer_widgets::tutorial::tutorial_button;
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "dashboard_tutorial_tests.rs"]
mod tests;

pub struct DashboardTutorial {
    seen: bool,
}

pub enum DashboardTutorialEvent {
    Completed,
}

impl EventEmitter<DashboardTutorialEvent> for DashboardTutorial {}

impl DashboardTutorial {
    pub fn new(seen: bool) -> Self {
        Self { seen }
    }

    pub fn set_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if self.seen != seen {
            self.seen = seen;
            cx.notify();
        }
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.set_seen(false, cx);
    }

    fn complete(&mut self, cx: &mut Context<Self>) {
        if !self.seen {
            self.seen = true;
            cx.emit(DashboardTutorialEvent::Completed);
            cx.notify();
        }
    }

    fn content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("dashboard-tutorial-content")
            .test_support()
            .aria_label(i18n::t("DASHBOARD_TUTORIAL_HEADER"))
            .relative()
            .w(surface::css(290.))
            .rounded(surface::css(5.))
            .border_1()
            .border_color(MainPageColors.tutorial_accent())
            .bg(cx.theme().group_box)
            .text_color(cx.theme().foreground)
            .whitespace_normal()
            .child(
                div()
                    .id("dashboard-tutorial-scroll")
                    .w_full()
                    .max_h((window.viewport_size().height - window.rem_size() * 2.5).max(px(1.)))
                    .scrollable_y()
                    .child(
                        v_flex()
                            .p(surface::css(20.))
                            .child(razer_widgets::tutorial_media::clip(
                                "synapse/tutorial-dashboard.webp",
                                i18n::t("DASHBOARD_TUTORIAL_HEADER"),
                                250. / 190.,
                            ))
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
                                h_flex().mt(surface::css(20.)).justify_center().child(
                                    tutorial_button(
                                        "dashboard-tutorial-close",
                                        i18n::t("CLOSE"),
                                        true,
                                        false,
                                        window,
                                        cx,
                                    )
                                    .ml(surface::css(5.))
                                    .on_click(cx.listener(|this, _, _, cx| this.complete(cx))),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for DashboardTutorial {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        // Si.v() first sets the measured left coordinates after 100ms. Until
        // then display:none hides both the panel and its fixed-position marker.
        let now = cx.background_executor().now();
        let started =
            window.use_keyed_state(
                "dashboard-tutorial-layout-delay",
                cx,
                |_, _| None::<Instant>,
            );
        let ready = started.update(cx, |started, _| {
            if self.seen {
                *started = None;
                false
            } else {
                now.saturating_duration_since(*started.get_or_insert(now))
                    >= Duration::from_millis(100)
            }
        });
        if !self.seen && !ready {
            window.request_animation_frame();
        }
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
                    .open(ready)
                    .trigger_with(|_, _, _| div().size_0().into_any_element())
                    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                        if !*open {
                            this.complete(cx);
                        }
                    }))
                    .content(move |_, window, cx| {
                        owner
                            .update(cx, |this, cx| this.content(window, cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    }),
            )
            .when(ready, |view| {
                // Source uses position:fixed for this marker, independently of
                // the panel. Keep it on the navigation target when Base clamps
                // the panel to a small viewport; escape ancestor clipping only.
                view.child(
                    deferred(
                        TutorialIndicator::new("dashboard-tutorial-indicator")
                            .absolute()
                            .left(surface::css(124.))
                            .top(surface::css(-38.)),
                    )
                    .priority(gpui_kit::base::POPUP_PRIORITY + 1),
                )
            })
    }
}
