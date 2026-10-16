//! Settings 720: `.setting-block .thx-btn.test`, not the Component outline button.
use crate::surface::css;
use crate::theme::SettingsButtonColors;
use gpui_kit::base::{
    Button,
    motion::{self, Easing, Transition},
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

#[derive(Default)]
struct Interaction {
    hovered: bool,
    pressed: bool,
}

pub fn settings_button(
    id: &'static str,
    label: impl Into<SharedString>,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> Button {
    let label = label.into().to_uppercase();
    let state = window.use_keyed_state((ElementId::from(id), "settings-button"), cx, |_, _| {
        Interaction::default()
    });
    let interaction = state.read(cx);
    let target = if disabled {
        0.3
    } else if interaction.pressed && interaction.hovered {
        0.6
    } else if interaction.hovered {
        0.8
    } else {
        1.
    };
    let opacity = motion::transition(
        (id, "opacity"),
        target,
        Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
        window,
        cx,
    );
    Button::new(id)
        .accessibility_label(label.clone())
        .disabled(disabled)
        .min_w(css(100.))
        .h(css(27.))
        .flex_shrink_0()
        .px(css(10.))
        .py_0()
        .text_size(css(12.))
        .whitespace_nowrap()
        .border_1()
        .border_color(SettingsButtonColors::border())
        .rounded(css(3.))
        .bg(SettingsButtonColors::background())
        .text_color(SettingsButtonColors::foreground())
        .opacity(opacity)
        .when(!disabled, |button| button.cursor_pointer())
        .when(disabled, |button| button.cursor_default())
        .focus_visible(|style| style.border_color(cx.theme().primary))
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            if !hovered {
                state.pressed = false;
            }
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&state, move |state, _, _, cx| {
                state.pressed = !disabled;
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
        .child(label)
}
