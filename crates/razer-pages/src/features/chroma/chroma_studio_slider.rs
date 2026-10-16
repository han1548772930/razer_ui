//! Studio 2132 no-tip range: 958.eb094e3a.chunk.css `.slider > .range`.
//! The fill uses p*W; the HTML range thumb travels inside the 8px end insets.
use gpui_kit::base::motion::{self, Easing, Interpolate, Transition};
use gpui_kit::base::{Slider, SliderIndicator, SliderThumb, SliderTrack, slider::SliderState};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface;
use std::time::Duration;

struct Palette;
impl Palette {
    fn green() -> Rgba {
        rgb(0x44d62c)
    }
    fn track() -> Rgba {
        rgba(0x44d62c4d)
    }
    fn hover() -> Rgba {
        rgb(0x5d5d5d)
    }
    fn pressed() -> Rgba {
        rgb(0x404040)
    }
}

// CSS interpolates color channels rather than HSL hue across green/gray.
#[derive(Clone, PartialEq)]
struct Background(Rgba);
impl Interpolate for Background {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        Self(Rgba {
            r: mix(self.0.r, target.0.r),
            g: mix(self.0.g, target.0.g),
            b: mix(self.0.b, target.0.b),
            a: mix(self.0.a, target.0.a),
        })
    }
}

struct Interaction {
    focus: FocusHandle,
    hovered: bool,
    pressed: bool,
    _activation: Subscription,
}

type KeyChange = Box<dyn Fn(f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(super) struct StudioSlider {
    state: Entity<SliderState>,
    enabled: bool,
    label: Option<SharedString>,
    on_key_change: Option<KeyChange>,
}
impl StudioSlider {
    pub(super) fn new(state: &Entity<SliderState>, enabled: bool) -> Self {
        Self {
            state: state.clone(),
            enabled,
            label: None,
            on_key_change: None,
        }
    }
    pub(super) fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
    /// Base emits Change/Release for pointer input. Keyboard changes use this
    /// owner callback because SliderState::set_value intentionally emits neither.
    pub(super) fn on_key_change(
        mut self,
        callback: impl Fn(f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_key_change = Some(Box::new(callback));
        self
    }
}

impl RenderOnce for StudioSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::from(("studio-slider", self.state.entity_id()));
        let interaction = window.use_keyed_state((id.clone(), "interaction"), cx, |window, cx| {
            let activation =
                cx.observe_window_activation(window, |state: &mut Interaction, window, cx| {
                    if !window.is_window_active() && (state.hovered || state.pressed) {
                        state.hovered = false;
                        state.pressed = false;
                        cx.notify();
                    }
                });
            Interaction {
                focus: cx.focus_handle().tab_stop(true),
                hovered: false,
                pressed: false,
                _activation: activation,
            }
        });
        let pointer = interaction.read(cx);
        let focus = pointer.focus.clone();
        let enabled = self.enabled;
        let background = if enabled && pointer.pressed {
            Palette::pressed()
        } else if enabled && pointer.hovered {
            Palette::hover()
        } else {
            Palette::green()
        };
        let background = motion::transition(
            (id.clone(), "thumb-background"),
            Background(background),
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        )
        .0;
        let progress = self.state.read(cx).percentage().end.clamp(0., 1.);
        let thumb = SliderThumb::new(&self.state)
            .disabled(!enabled)
            .absolute()
            .left(relative(progress))
            .ml(surface::css(-8.))
            .size(surface::css(16.))
            .rounded_full()
            .border_1()
            .border_color(Palette::green())
            .bg(background)
            .on_hover(window.listener_for(&interaction, |state, hovered, _, cx| {
                state.hovered = *hovered;
                cx.notify();
            }))
            .capture_any_mouse_down(window.listener_for(
                &interaction,
                move |state, event: &MouseDownEvent, window, cx| {
                    if enabled && event.button == MouseButton::Left {
                        state.pressed = true;
                        state.focus.focus(window, cx);
                        cx.notify();
                    }
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&interaction, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&interaction, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            );

        let slider = Slider::new(&self.state)
            .disabled(!enabled)
            .relative()
            .w_full()
            .h(surface::css(16.))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(surface::css(6.))
                    .w_full()
                    .h(surface::css(5.))
                    .rounded(surface::css(3.))
                    .bg(Palette::track()),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(surface::css(6.))
                    .w(relative(progress))
                    .h(surface::css(5.))
                    .rounded(surface::css(3.))
                    .bg(Palette::green()),
            )
            .child(
                SliderTrack::new(&self.state)
                    .disabled(!enabled)
                    .absolute()
                    .inset_0()
                    .child(
                        SliderIndicator::new(&self.state)
                            .absolute()
                            .left(surface::css(8.))
                            .right(surface::css(8.))
                            .h_full()
                            .child(thumb),
                    ),
            );
        let state = self.state.clone();
        let callback = self.on_key_change;
        div()
            .id(id)
            .relative()
            .w_full()
            .h(surface::css(16.))
            .when_some(self.label, |view, label| view.aria_label(label))
            // A tool switch can disable the range while the button is held.
            // Release cleanup stays mounted even though new presses are inert.
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&interaction, |state, _, _, cx| {
                    if state.pressed {
                        state.pressed = false;
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&interaction, |state, _, _, cx| {
                    if state.pressed {
                        state.pressed = false;
                        cx.notify();
                    }
                }),
            )
            .when(enabled, |view| {
                view.track_focus(&focus)
                    .tab_index(0)
                    .focus_visible(|style| style.border_1().border_color(Palette::green()))
                    .capture_any_mouse_down(window.listener_for(
                        &interaction,
                        |state, event: &MouseDownEvent, window, cx| {
                            if event.button == MouseButton::Left {
                                state.pressed = true;
                                state.focus.focus(window, cx);
                                cx.notify();
                            }
                        },
                    ))
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let slider = state.read(cx);
                        let old = slider.value().end();
                        let next = match event.keystroke.key.as_str() {
                            "left" | "down" => old - slider.step_value(),
                            "right" | "up" => old + slider.step_value(),
                            "home" => slider.min_value(),
                            "end" => slider.max_value(),
                            _ => return,
                        }
                        .clamp(slider.min_value(), slider.max_value());
                        if next != old {
                            state.update(cx, |state, cx| state.set_value(next, window, cx));
                            if let Some(callback) = &callback {
                                callback(next, window, cx);
                            }
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                    })
            })
            .child(slider)
    }
}
