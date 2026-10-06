//! 3690 preset tile: 100ms border/pressed overlay, immediate white selection ring.
use super::studio_gradient_data::{Stop, bar};
use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use std::time::Duration;

struct Interaction {
    hovered: bool,
    pressed: bool,
    _activation: Subscription,
}
type Activate = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(super) struct GradientPreset {
    id: ElementId,
    label: String,
    stops: Vec<Stop>,
    selected: bool,
    enabled: bool,
    on_activate: Activate,
}
impl GradientPreset {
    pub(super) fn new(
        id: ElementId,
        label: String,
        stops: Vec<Stop>,
        selected: bool,
        enabled: bool,
        on_activate: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id,
            label,
            stops,
            selected,
            enabled,
            on_activate: Box::new(on_activate),
        }
    }
}
impl RenderOnce for GradientPreset {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "interaction"), cx, |window, cx| {
            let activation =
                cx.observe_window_activation(window, |state: &mut Interaction, window, cx| {
                    if !window.is_window_active() {
                        state.hovered = false;
                        state.pressed = false;
                        cx.notify();
                    }
                });
            Interaction {
                hovered: false,
                pressed: false,
                _activation: activation,
            }
        });
        let pointer = state.read(cx);
        let hovered = self.enabled && pointer.hovered;
        let pressed = self.enabled && pointer.pressed;
        let border = motion::transition(
            (self.id.clone(), "border"),
            if hovered || pressed { 1_f32 } else { 0.3 },
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        let overlay = motion::transition(
            (self.id.clone(), "overlay"),
            if pressed { 0.3_f32 } else { 0. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
            window,
            cx,
        );
        let ring = vec![BoxShadow {
            inset: false,
            color: Colors::white(),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: surface::css(2.).to_pixels(window.rem_size()),
        }];
        BaseButton::new(self.id)
            .accessibility_label(self.label)
            .disabled(!self.enabled)
            .p_0()
            .relative()
            .flex_shrink_0()
            .w(surface::css(25.))
            .h(surface::css(16.))
            .rounded(surface::css(3.))
            .border_1()
            .border_color(Colors::black().opacity(border))
            .when(self.selected || hovered || pressed, |view| {
                view.shadow(ring.clone())
            })
            .focus_visible(move |style| style.shadow(ring.clone()))
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                state.hovered = *hovered;
                cx.notify();
            }))
            .capture_any_mouse_down(window.listener_for(
                &state,
                |state, event: &MouseDownEvent, _, cx| {
                    if event.button == MouseButton::Left {
                        state.pressed = true;
                        cx.notify();
                    }
                },
            ))
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
            .child(
                div()
                    .relative()
                    .size_full()
                    .rounded(surface::css(2.))
                    .overflow_hidden()
                    .child(super::studio_color::checkered())
                    .child(div().absolute().inset_0().child(bar(self.stops)))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(Colors::black())
                            .opacity(overlay),
                    ),
            )
            .on_click(move |event, window, cx| {
                if self.enabled {
                    (self.on_activate)(event, window, cx);
                }
            })
    }
}
