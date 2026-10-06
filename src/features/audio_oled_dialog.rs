//! Current 1383 `Tn` / choose-a-mat shell. Editors own their draft and body.
use super::theme::CssColor;
use super::*;
use gpui_kit::base::Dialog;
use gpui_kit::base::motion::{self, Easing, Transition};
use std::time::Duration;

#[derive(Default)]
struct ClosePointer {
    hovered: bool,
    pressed: bool,
}

pub(super) struct DialogState {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    open: bool,
    started: Instant,
    max_width: f32,
}

impl DialogState {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            focus,
            return_focus,
            open: true,
            started: Instant::now(),
            max_width: 850.,
        }
    }

    pub(super) fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    pub(super) fn close(&mut self, window: &mut Window, cx: &mut App) {
        self.open = false;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn elapsed(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }

    pub(super) fn render<T: 'static>(
        &self,
        id: &'static str,
        title: String,
        body: AnyElement,
        footer: AnyElement,
        window: &mut Window,
        cx: &mut Context<T>,
        on_close: fn(&mut T, &mut Window, &mut Context<T>),
    ) -> AnyElement {
        if !self.open {
            return div().into_any_element();
        }
        let elapsed = self.elapsed();
        let ease = Easing::Ease.sample((elapsed / 0.3).clamp(0., 1.));
        let source_top =
            window.viewport_size().height * (1. - ease) + window.rem_size() * (100. / 16.) * ease;
        if elapsed < 0.3 {
            window.request_animation_frame();
        }
        let close = close_button(id, window, cx, on_close);
        let panel = v_flex()
            .relative()
            .occlude()
            .w(surface::css(
                if window.viewport_size().width >= window.rem_size() * (1400. / 16.) {
                    self.max_width
                } else {
                    800.
                },
            ))
            .min_w(surface::css(800.))
            .h(
                (window.viewport_size().height - source_top - window.rem_size() * (110. / 16.))
                    .max(px(0.)),
            )
            .bg(Colors::surface())
            .rounded_t(surface::css(5.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::text())
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .h(surface::css(36.))
                    .pt(surface::css(20.))
                    .pb(surface::css(10.))
                    .shadow(vec![BoxShadow {
                        color: Colors::border(),
                        offset: point(px(0.), window.rem_size() / 16.),
                        blur_radius: px(0.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_center()
                    .text_color(Colors::muted())
                    .child(title.to_uppercase())
                    .child(close),
            )
            .child(body)
            .child(footer);
        Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(move |this, _, window, cx| on_close(this, window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(Colors::black().opacity(0.5))
                    .opacity((elapsed / 0.1).clamp(0., 1.)),
            )
            .popup(
                div()
                    .absolute()
                    .left_0()
                    .top(source_top + window.rem_size() * (110. / 16.))
                    .w(window.viewport_size().width)
                    .flex()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}

fn close_button<T: 'static>(
    id: &'static str,
    window: &mut Window,
    cx: &mut Context<T>,
    on_close: fn(&mut T, &mut Window, &mut Context<T>),
) -> AnyElement {
    let pointer = window.use_keyed_state((ElementId::from(id), "close-pointer"), cx, |_, _| {
        ClosePointer::default()
    });
    let state = pointer.read(cx);
    let color = if state.pressed {
        Colors::close_pressed()
    } else if state.hovered {
        Colors::close_hover()
    } else {
        Colors::transparent()
    };
    let color = motion::transition(
        (ElementId::from(id), "close-background"),
        CssColor(color.into()),
        Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
        window,
        cx,
    );
    div()
        .id((ElementId::from(id), "close-pointer"))
        .absolute()
        .right_0()
        .top_0()
        .size(surface::css(36.))
        .bg(color.0)
        .on_hover(window.listener_for(&pointer, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .capture_any_mouse_down(window.listener_for(
            &pointer,
            |state, event: &MouseDownEvent, _, cx| {
                if event.button == MouseButton::Left {
                    state.pressed = true;
                    cx.notify();
                }
            },
        ))
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&pointer, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&pointer, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .child(
            BaseButton::new((ElementId::from(id), "close"))
                .accessibility_label(t("CLOSE"))
                .size_full()
                .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                .on_click(cx.listener(move |this, _, window, cx| on_close(this, window, cx))),
        )
        .into_any_element()
}

pub(super) fn action(id: &'static str, text: String, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(text.clone())
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .rounded(surface::css(3.))
        .bg(if primary {
            Colors::selected()
        } else {
            Colors::disabled()
        })
        .text_color(if primary {
            Colors::surface()
        } else {
            Colors::white()
        })
        .text_size(surface::css(12.))
        .active(|s| s.opacity(0.3))
        .child(text.to_uppercase())
}

pub(super) fn footer(cancel: AnyElement, apply: AnyElement) -> AnyElement {
    h_flex()
        .absolute()
        .bottom(surface::css(35.))
        .left(relative(0.5))
        .ml(surface::css(-95.))
        .w(surface::css(190.))
        .gap(surface::css(10.))
        .child(cancel)
        .child(apply)
        .into_any_element()
}
