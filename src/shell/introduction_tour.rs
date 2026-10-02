//! Introduction Tour's Fn/Cn/Dn/Pn, from main.bf769e69.js and main.adb3bb78.css.
use crate::{
    i18n,
    ui::{scroll::SourceScrollable as _, surface, theme::TourColors, tutorial_media},
};
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

struct Step {
    header: &'static str,
    paragraphs: &'static [&'static str],
    media: &'static str,
}

const STEPS: &[Step] = &[
    Step {
        header: "QUICK_EFFECTS_AND_ADVANCED_EFFECT_HEADER",
        paragraphs: &[
            "QUICK_EFFECTS_AND_ADVANCED_EFFECT_CONTENT_1",
            "QUICK_EFFECTS_AND_ADVANCED_EFFECT_CONTENT_2",
            "QUICK_EFFECTS_AND_ADVANCED_EFFECT_CONTENT_3",
        ],
        media: "synapse/tour-quick-effects.webp",
    },
    Step {
        header: "THE_DEVICES_AND_MODULES_TAB_HEADER",
        paragraphs: &[
            "THE_DEVICES_AND_MODULES_TAB_CONTENT_1",
            "THE_DEVICES_AND_MODULES_TAB_CONTENT_2",
        ],
        media: "synapse/tour-devices-modules.webp",
    },
    Step {
        header: "MORE_RAZER_APPLICATIONS_HEADER",
        paragraphs: &["CLICK_ON_THE_ICON"],
        media: "synapse/tour-razer-apps.webp",
    },
    Step {
        header: "USE_MACROS_TO_IMPROVE_EFFECIENCY_HEADER",
        paragraphs: &[
            "USE_MACROS_TO_IMPROVE_EFFECIENCY_CONTENT_1",
            "USE_MACROS_TO_IMPROVE_EFFECIENCY_CONTENT_2",
        ],
        media: "synapse/tour-macros.webp",
    },
    Step {
        header: "LINKED_GAMES_TOUR_HEADER",
        paragraphs: &["LINKED_GAMES_TOUR_CONTENT_1", "LINKED_GAMES_TOUR_CONTENT_2"],
        media: "synapse/tour-linked-games.webp",
    },
];

pub(super) struct IntroductionTour {
    selected_ix: usize,
    next_focus: FocusHandle,
    scroll: ScrollHandle,
}

pub(super) struct CloseRequested;
impl EventEmitter<CloseRequested> for IntroductionTour {}

impl IntroductionTour {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            selected_ix: 0,
            next_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
        }
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.next_focus.focus(window, cx);
    }

    fn previous(&mut self, cx: &mut Context<Self>) {
        if self.selected_ix > 0 {
            self.selected_ix -= 1;
            cx.notify();
        }
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if self.selected_ix + 1 == STEPS.len() {
            cx.emit(CloseRequested);
        } else {
            self.selected_ix += 1;
            cx.notify();
        }
    }

    fn navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let last = self.selected_ix + 1 == STEPS.len();
        v_flex()
            .id("introduction-tour-controls")
            .test_support()
            .min_h(surface::css(46.))
            .flex_shrink_0()
            .child(
                h_flex()
                    .gap(surface::css(12.))
                    .mb(surface::css(14.))
                    .children(STEPS.iter().enumerate().map(|(ix, step)| {
                        div()
                            .id(step.header)
                            .size(surface::css(4.))
                            .rounded_full()
                            .bg(if ix == self.selected_ix {
                                TourColors::button_text()
                            } else {
                                TourColors::secondary()
                            })
                    })),
            )
            .child(
                h_flex()
                    .child(
                        tour_button(
                            "introduction-tour-back",
                            "BACK",
                            false,
                            self.selected_ix == 0,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.previous(cx))),
                    )
                    .child(
                        tour_button(
                            "introduction-tour-next",
                            if last { "GET_STARTED" } else { "NEXT" },
                            true,
                            false,
                            window,
                            cx,
                        )
                        .track_focus(&self.next_focus)
                        .mx(surface::css(10.))
                        .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
                    )
                    // The source renders an underlined div command. Use a semantic
                    // Button so Tab/Enter retain the same close action as a click.
                    .child(
                        gpui_kit::base::Button::new("introduction-tour-skip")
                            .accessibility_label(i18n::t("SKIP").to_uppercase())
                            .child(div().underline().child(i18n::t("SKIP").to_uppercase()))
                            .flex()
                            .items_center()
                            .text_color(TourColors::paragraph())
                            .focus_visible(|style| style.text_color(cx.theme().primary))
                            .ml(surface::css(10.))
                            .p_0()
                            .h(surface::css(27.))
                            .text_size(surface::css(12.))
                            .disabled(last)
                            .when(last, |view| view.opacity(0.3))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.selected_ix + 1 < STEPS.len() {
                                    cx.emit(CloseRequested);
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn tour_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::base::Button {
    let state = window.use_keyed_state((ElementId::from(id), "opacity-state"), cx, |_, _| {
        TourButtonState::default()
    });
    let current = state.read(cx);
    let target = if disabled || current.pressed {
        0.6
    } else if current.hovered {
        0.8
    } else {
        1.
    };
    let opacity = motion::transition(
        (id, "opacity"),
        target,
        Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut),
        window,
        cx,
    );
    let background = if primary {
        cx.theme().primary
    } else {
        TourColors::secondary()
    };
    gpui_kit::base::Button::new(id)
        .accessibility_label(i18n::t(label).to_uppercase())
        .child(i18n::t(label).to_uppercase())
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .min_w(surface::css(100.))
        .h_auto()
        .min_h(surface::css(27.))
        .px(surface::css(16.))
        .py(surface::css(6.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .rounded(surface::css(3.))
        .border_1()
        .border_color(TourColors::border())
        .bg(background)
        .text_color(if primary {
            TourColors::primary_text()
        } else {
            TourColors::button_text()
        })
        .focus_visible(|style| style.border_color(cx.theme().primary))
        .disabled(disabled)
        .opacity(opacity)
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            if !hovered {
                state.pressed = false;
            }
            cx.notify();
        }))
        .when(!disabled, |view| {
            view.on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
        })
}

#[derive(Default)]
struct TourButtonState {
    hovered: bool,
    pressed: bool,
}

fn paragraph(key: &'static str, window: &Window) -> AnyElement {
    let content = i18n::t(key);
    let body = if key == "CLICK_ON_THE_ICON" {
        // Preserve the locale's complete sentence and exact inline source icon.
        // HTML only accepts px lengths: resolve its 16px size using the rem base.
        let icon = f32::from(window.rem_size());
        let html = content.replace(
            "<i class=\"icon-app u-icon-size-16 u-icon u-ml-2 u-mr-2\"></i>",
            &format!(
                "<img src=\"tour-app-icon\" width=\"{}\" height=\"{icon}\" />",
                icon * 2.
            ),
        );
        gpui_kit::base::TextView::html("introduction-tour-apps-paragraph", html)
            .image_source(|_| "synapse/tour-app-inline.svg".into())
            .scrollable(false)
            .selectable(false)
            .into_any_element()
    } else {
        div().child(content).into_any_element()
    };
    div()
        .mt(surface::css(16.))
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .text_color(TourColors::paragraph())
        .whitespace_normal()
        .child(body)
        .into_any_element()
}

impl Render for IntroductionTour {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let step = &STEPS[self.selected_ix];
        div()
            .id("introduction-tour-scroll")
            .test_support()
            .size_full()
            .bg(TourColors::background())
            .scrollable_both()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .relative()
                    .w_full()
                    .min_w(surface::css(1160.))
                    .min_h(surface::css(700.))
                    .child(
                        img("synapse/tour-background.png")
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(surface::css(640.))
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(surface::css(700.))
                            .overflow_hidden()
                            .child(
                                img("synapse/tour-background-fade.png")
                                    .absolute()
                                    .left_0()
                                    .top(surface::css(500.))
                                    .w_full()
                                    .h(surface::css(250.))
                                    .object_fit(ObjectFit::Fill),
                            ),
                    )
                    // The source's final !important rules fix both margins at 20px.
                    // Keep content in flow so longer translations and short windows scroll.
                    .child(
                        h_flex()
                            .relative()
                            .items_start()
                            .w(surface::css(1120.))
                            .mx(surface::css(20.))
                            .pt(surface::css(80.))
                            .pb(surface::css(40.))
                            .child(
                                v_flex()
                                    .w(surface::css(450.))
                                    .flex_shrink_0()
                                    .mr(surface::css(30.))
                                    .child(
                                        v_flex()
                                            .min_h(surface::css(314.))
                                            .child(
                                                div()
                                                    .id("introduction-tour-heading")
                                                    .test_support()
                                                    .font_family("RazerF5")
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(surface::css(24.))
                                                    .line_height(surface::css(23.))
                                                    .text_color(cx.theme().primary)
                                                    .child(i18n::t(step.header).to_uppercase()),
                                            )
                                            .children(
                                                step.paragraphs
                                                    .iter()
                                                    .map(|key| paragraph(key, window)),
                                            ),
                                    )
                                    .child(self.navigation(window, cx)),
                            )
                            .child(
                                tutorial_media::clip(step.media, i18n::t(step.header), 640. / 360.)
                                    .w(surface::css(640.)),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
#[path = "introduction_tour_tests.rs"]
mod tests;
