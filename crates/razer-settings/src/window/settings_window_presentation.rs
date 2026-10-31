//! Native presentation of current Settings SVG class transitions and tooltips.
use gpui_kit::base::{
    Tooltip,
    motion::{Easing, Presence, Transition},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::keyboard_navigation::Button;
use razer_widgets::surface::css;
use std::time::Duration;
#[path = "settings_window_theme.rs"]
mod theme;

#[derive(Default)]
struct Hover {
    hovered: bool,
}

fn tip(id: ElementId, label: SharedString) -> Tooltip {
    Tooltip::new(id)
        .font_family("Roboto")
        .text_size(css(14.))
        .line_height(css(16.))
        .whitespace_nowrap()
        .px(css(10.))
        .py(css(8.))
        .border_1()
        .border_color(theme::tooltip_border())
        .bg(theme::tooltip_surface())
        .text_color(theme::tooltip_text())
        .child(label)
}

pub fn toolbar_button(
    id: &'static str,
    label: SharedString,
    asset: &'static str,
    disabled: bool,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = window.use_keyed_state((ElementId::from(id), "hover"), cx, |_, _| Hover::default());
    let hovered = state.read(cx).hovered && !disabled;
    let opacity = Presence::new((ElementId::from(id), "tooltip-opacity"), hovered)
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
    let mut wrapper = div()
        .id((ElementId::from(id), "wrapper"))
        .relative()
        .w(css(40.))
        .h(css(38.))
        .flex_shrink_0()
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .child(
            Button::new(id)
                .accessibility_label(label.clone())
                .disabled(disabled)
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_0()
                .when(disabled, |button| button.opacity(0.3))
                .when(!disabled, |button| {
                    button.hover(|s| s.bg(theme::toolbar_hover()))
                })
                .focus_visible(|s| s.bg(theme::toolbar_hover()))
                .child(img(asset).size(css(20.)))
                .on_click(handler),
        );
    if hovered {
        wrapper = wrapper.child(
            razer_assets::deferred_resource(
                tip((ElementId::from(id), "tooltip").into(), label)
                    .absolute()
                    .left_0()
                    .top(css(43.))
                    .opacity(opacity),
            )
            .with_priority(301),
        );
    }
    wrapper.into_any_element()
}
