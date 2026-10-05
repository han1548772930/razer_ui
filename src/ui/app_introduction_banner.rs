//! Shared current Synapse `22534/ji` and Chroma `62296/Pn` presentation.
//! Their selectors, declarations and language keys match after resolving asset
//! URL roots. See app-introduction-banner-current-evidence.json. Both local
//! applications have a tour, so their compiled capability selects Start Tour.
use crate::{i18n, ui::surface::css};
use gpui_kit::base::{
    Button,
    motion::{self, Easing, Transition},
};
use gpui_kit::*;
use std::{rc::Rc, time::Duration};

type Action = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct AppIntroductionBanner {
    id: &'static str,
    namespace: &'static str,
    close: Action,
    synapse_tour: Action,
    chroma_tour: Action,
}

impl AppIntroductionBanner {
    pub(crate) fn new(
        id: &'static str,
        namespace: &'static str,
        close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        synapse_tour: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        chroma_tour: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            namespace,
            close: Rc::new(close),
            synapse_tour: Rc::new(synapse_tour),
            chroma_tour: Rc::new(chroma_tour),
        }
    }
    fn text(&self, suffix: &str) -> String {
        i18n::t(&format!("{}INTRODUCTION_BANNER_{suffix}", self.namespace))
    }
}

#[derive(Default)]
struct ButtonState {
    hovered: bool,
    pressed: bool,
}

fn tour_button(
    id: ElementId,
    label: String,
    action: Action,
    window: &mut Window,
    cx: &mut App,
) -> Button {
    let state = window.use_keyed_state((id.clone(), "interaction"), cx, |_, _| {
        ButtonState::default()
    });
    let current = state.read(cx);
    let opacity = if current.pressed && current.hovered {
        0.6
    } else if current.hovered {
        0.8
    } else {
        1.
    };
    let opacity = motion::transition(
        (id.clone(), "opacity"),
        opacity,
        Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut),
        window,
        cx,
    );
    Button::new(id)
        .h(css(27.))
        .min_w(css(100.))
        .px(css(5.))
        .pt(css(7.))
        .pb(css(6.))
        .mt(css(30.))
        .border_1()
        .border_color(rgba(0x0000004d))
        .rounded(css(2.))
        .bg(rgb(0x44d62c))
        .text_color(rgb(0x000000))
        .text_size(css(12.))
        .line_height(css(12.))
        .opacity(opacity)
        .cursor_default()
        .child(label.to_uppercase())
        .on_hover(window.listener_for(&state, |s, hovered, _, cx| {
            s.hovered = *hovered;
            if !hovered {
                s.pressed = false;
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&state, |s, _, _, cx| {
                s.pressed = true;
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&state, |s, _, _, cx| {
                s.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&state, |s, _, _, cx| {
                s.pressed = false;
                cx.notify();
            }),
        )
        .on_click(move |e, w, cx| action(e, w, cx))
}

impl RenderOnce for AppIntroductionBanner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut body = div()
            .relative()
            .flex()
            .justify_around()
            .w_full()
            .max_w(css(1020.))
            .mx_auto()
            .pb(css(30.));
        for (name, icon, action) in [
            (
                "SYNAPSE",
                "synapse/chroma-big_synapse_4.svg",
                self.synapse_tour.clone(),
            ),
            (
                "CHROMA_APP",
                "synapse/chroma-introduction-logo.png",
                self.chroma_tour.clone(),
            ),
        ] {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        img(icon)
                            .size(css(100.))
                            .mt(css(27.))
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        div()
                            .mt(css(16.))
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .text_color(rgb(0xffffff))
                            .child(self.text(&format!("{name}_BODY_1")).to_uppercase()),
                    )
                    .child(
                        div()
                            .mt(css(10.))
                            .mb(css(10.))
                            .text_size(css(14.))
                            .font_weight(FontWeight::NORMAL)
                            .child(self.text(&format!("{name}_BODY_2")).to_uppercase()),
                    )
                    .child(
                        div()
                            .w(css(360.))
                            .text_size(css(14.))
                            .font_weight(FontWeight::NORMAL)
                            .text_center()
                            .child(self.text(&format!("{name}_BODY_3"))),
                    )
                    .child(tour_button(
                        (ElementId::from(self.id), SharedString::from(name)).into(),
                        self.text("START_TOUR"),
                        action,
                        window,
                        cx,
                    )),
            );
        }
        body = body.child(
            div()
                .absolute()
                .top(css(-30.))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    img("synapse/chroma-split_arrow.svg")
                        .size(css(140.))
                        .object_fit(ObjectFit::Contain),
                ),
        );
        div()
            .id(self.id)
            .relative()
            .w_full()
            .min_w(css(1220.))
            .max_w(css(2500.))
            .min_h(css(531.))
            .mt(css(10.))
            .rounded(css(5.))
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .child(
                img("synapse/chroma-introduction_background.png")
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .rounded(css(5.)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .mt(css(20.))
                            .font_family("RazerF5")
                            .text_size(css(42.))
                            .font_weight(FontWeight::BOLD)
                            .text_center()
                            .text_color(rgb(0x44d62c))
                            .child(self.text("HEADING_1").to_uppercase()),
                    )
                    .child(
                        div()
                            .m(css(16.))
                            .font_family("RazerF5")
                            .text_size(css(24.))
                            .child(self.text("HEADING_2")),
                    )
                    .child(
                        div()
                            .mb(css(16.))
                            .text_size(css(14.))
                            .child(self.text("HEADING_3")),
                    ),
            )
            .child(body)
            .child(
                Button::new((ElementId::from(self.id), "close"))
                    .absolute()
                    .top(css(10.))
                    .right(css(10.))
                    .size(css(24.))
                    .p_0()
                    .accessibility_label(i18n::t("CLOSE"))
                    .child(img("synapse/mapping-close.svg").size_full())
                    .on_click(move |e, w, cx| (self.close)(e, w, cx)),
            )
    }
}
