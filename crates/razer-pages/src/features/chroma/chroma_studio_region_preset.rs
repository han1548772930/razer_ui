//! Current 958 Ambient region presets: 30x20, 4px white regions and a
//! 100ms ease-in-out border transition. Selection belongs to StudioProperties.
use super::{Colors, label};
use gpui_kit::base::{
    Button,
    motion::{self, Easing, Interpolate, Transition},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface::css;
use std::time::Duration;

#[derive(Clone, PartialEq)]
struct Border(Rgba);

impl Interpolate for Border {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        // Premultiplied RGBA preserves the source green as its alpha changes;
        // interpolating HSL would introduce unrelated hues between gray/green.
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        let alpha = mix(self.0.a, target.0.a);
        let channel = |a: f32, b: f32| {
            if alpha > 0. {
                mix(a * self.0.a, b * target.0.a) / alpha
            } else {
                0.
            }
        };
        Self(Rgba {
            r: channel(self.0.r, target.0.r),
            g: channel(self.0.g, target.0.g),
            b: channel(self.0.b, target.0.b),
            a: alpha,
        })
    }
}

struct Interaction {
    hovered: bool,
    pressed: bool,
    _activation: Subscription,
}

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(super) struct RegionPreset {
    name: &'static str,
    selected: bool,
    enabled: bool,
    on_click: Click,
}

impl RegionPreset {
    pub(super) fn new(
        name: &'static str,
        selected: bool,
        enabled: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            name,
            selected,
            enabled,
            on_click: Box::new(on_click),
        }
    }
}

impl RenderOnce for RegionPreset {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::from((ElementId::from("studio-region"), self.name));
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
                hovered: false,
                pressed: false,
                _activation: activation,
            }
        });
        let enabled = self.enabled;
        if !enabled && (interaction.read(cx).hovered || interaction.read(cx).pressed) {
            interaction.update(cx, |state, _| {
                state.hovered = false;
                state.pressed = false;
            });
        }
        let pointer = interaction.read(cx);
        let target = if enabled && pointer.pressed {
            Colors::selected_pressed()
        } else if self.selected || (enabled && pointer.hovered) {
            Colors::selected()
        } else {
            Colors::helper()
        };
        let border = motion::transition(
            (id.clone(), "border"),
            Border(target.into()),
            Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
            window,
            cx,
        )
        .0;
        let region = div().absolute().bg(Colors::white());
        let region = match self.name {
            "full" => region.inset_0(),
            "left" => region.left_0().top_0().h_full().w(css(4.)),
            "right" => region.right_0().top_0().h_full().w(css(4.)),
            "top" => region.left_0().top_0().w_full().h(css(4.)),
            "bottom" => region.left_0().bottom_0().w_full().h(css(4.)),
            _ => region.size_0(),
        };
        Button::new(id)
            .accessibility_label(format!("{}: {}", label("TEXT_REGION"), self.name))
            .selected(self.selected)
            .disabled(!enabled)
            // The enclosing effects-custom element owns the source opacity .3.
            .styles(|style| style.disabled(|style| style.opacity(1.)))
            .p_0()
            .w(css(30.))
            .h(css(20.))
            .when(self.name != "bottom", |button| button.mr(css(10.)))
            .border_1()
            .border_color(border)
            .bg(Colors::region_background())
            .when(enabled, |button| {
                button.focus_visible(|style| style.border_color(Colors::selected()))
            })
            .on_hover(
                window.listener_for(&interaction, move |state, hovered, _, cx| {
                    state.hovered = enabled && *hovered;
                    if !state.hovered {
                        state.pressed = false;
                    }
                    cx.notify();
                }),
            )
            .capture_any_mouse_down(window.listener_for(
                &interaction,
                move |state, event: &MouseDownEvent, _, cx| {
                    if enabled && event.button == MouseButton::Left {
                        state.pressed = true;
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
            )
            .child(div().relative().size_full().child(region))
            .on_click(move |event, window, cx| {
                if enabled {
                    (self.on_click)(event, window, cx);
                }
            })
    }
}
