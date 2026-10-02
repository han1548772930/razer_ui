//! Source presentation; Kit Base retains activation, focus and accessibility.
use super::*;
use crate::ui::theme::{PaletteColors, SettingsButtonColors};
use gpui_kit::base::{Switch, motion};

#[derive(Clone, Copy)]
pub(super) enum SourceButtonKind {
    Green,
    Gray,
    Amazon,
}
#[derive(Default)]
struct Interaction {
    hovered: bool,
    pressed: bool,
    keyboard: bool,
}

pub(super) fn source_button(
    id: &'static str,
    label: impl Into<SharedString>,
    kind: SourceButtonKind,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> Button {
    let state = window.use_keyed_state((ElementId::from(id), "interaction"), cx, |_, _| {
        Interaction::default()
    });
    let interaction = state.read(cx);
    let pressed = interaction.keyboard || interaction.pressed && interaction.hovered;
    let hovered = interaction.hovered;
    let opacity = motion::transition(
        (id, "opacity"),
        if disabled {
            0.3
        } else if pressed {
            0.6
        } else if hovered {
            0.8
        } else {
            1.
        },
        Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut),
        window,
        cx,
    );
    let background = match kind {
        SourceButtonKind::Green => cx.theme().primary,
        SourceButtonKind::Gray => SettingsButtonColors::background(),
        SourceButtonKind::Amazon if pressed => AlexaColors::amazon_button_active(),
        SourceButtonKind::Amazon if hovered => AlexaColors::amazon_button_hover(),
        SourceButtonKind::Amazon => AlexaColors::amazon_button(),
    };
    let background = motion::transition(
        (id, "background"),
        background,
        Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut),
        window,
        cx,
    );
    let foreground = match kind {
        SourceButtonKind::Green => cx.theme().button_primary_foreground,
        SourceButtonKind::Gray => PaletteColors.white(),
        SourceButtonKind::Amazon => AlexaColors::amazon_button_text(),
    };
    let label = label.into().to_uppercase();
    Button::new(id)
        .disabled(disabled)
        .accessibility_label(label.clone())
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .min_w(css(90.))
        .px(css(16.))
        .pt(css(7.))
        .pb(css(6.))
        .text_size(css(12.))
        .line_height(css(12.))
        .rounded(css(2.))
        .border_1()
        .border_color(SettingsButtonColors::border())
        .bg(background)
        .text_color(foreground)
        .opacity(opacity)
        .focus_visible(|style| style.border_color(cx.theme().ring))
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
        .on_key_down(
            window.listener_for(&state, move |state, event: &KeyDownEvent, _, cx| {
                if !disabled && matches!(event.keystroke.key.as_str(), "space" | "enter") {
                    state.keyboard = true;
                    cx.notify();
                }
            }),
        )
        .on_key_up(window.listener_for(&state, |state, _: &KeyUpEvent, _, cx| {
            state.keyboard = false;
            cx.notify();
        }))
        .child(label)
}

pub(super) fn text_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Button {
    let label = label.into();
    Button::new(id)
        .accessibility_label(label.clone())
        .p_0()
        .text_size(css(14.))
        .text_color(AlexaColors::skill_text())
        .underline()
        .hover(|style| style.text_color(cx.theme().primary))
        .active(|style| style.opacity(0.7))
        .focus_visible(|style| style.border_1().border_color(cx.theme().ring))
        .child(label)
}

pub(super) fn source_switch(
    id: &'static str,
    label: String,
    checked: bool,
    window: &mut Window,
    cx: &mut App,
) -> Switch {
    let transition = Transition::new(Duration::from_millis(300)).easing(Easing::Ease);
    let mix = motion::transition(
        (id, "background"),
        if checked { 1_f32 } else { 0_f32 },
        transition.clone(),
        window,
        cx,
    );
    let left = motion::transition(
        (id, "handle"),
        if checked { 15_f32 } else { 1_f32 },
        transition,
        window,
        cx,
    );
    Switch::new(id)
        .checked(checked)
        .accessibility_label(label)
        .relative()
        .w(css(32.))
        .h(css(18.))
        .flex_shrink_0()
        .border_1()
        .border_color(SettingsButtonColors::border())
        .rounded(css(9.))
        .bg(cx.theme().switch.blend(cx.theme().primary.opacity(mix)))
        .focus_visible(|style| style.border_color(cx.theme().ring))
        .child(
            div()
                .absolute()
                .left(css(left))
                .top(css(1.))
                .size(css(14.))
                .rounded_full()
                .bg(cx.theme().title_bar),
        )
}

pub(super) fn source_checkbox(
    id: &'static str,
    label: String,
    checked: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> Checkbox {
    let state = window.use_keyed_state((ElementId::from(id), "interaction"), cx, |_, _| {
        Interaction::default()
    });
    let interaction = state.read(cx);
    let hovered = interaction.hovered && !disabled;
    let pressed = interaction.pressed && hovered;
    let background = if pressed {
        AlexaColors::checkbox_active()
    } else if checked && hovered {
        AlexaColors::checkbox_hover()
    } else if checked {
        cx.theme().primary
    } else {
        cx.theme().transparent
    };
    let border = if pressed {
        AlexaColors::checkbox_active()
    } else if checked && hovered {
        AlexaColors::checkbox_hover()
    } else if checked || hovered {
        cx.theme().primary
    } else {
        AlexaColors::checkbox_border()
    };
    let policy = || Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut);
    let background = motion::transition((id, "background"), background, policy(), window, cx);
    let border = motion::transition((id, "border"), border, policy(), window, cx);
    let top = Presence::new((id, "tick-top"), checked)
        .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    let bottom = Presence::new((id, "tick-bottom"), checked)
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    let tick = cx.theme().group_box;
    Checkbox::new(id)
        .checked(checked)
        .disabled(disabled)
        .accessibility_label(label.clone())
        .group(id)
        .flex()
        .items_center()
        .gap(css(10.))
        .mb(css(10.))
        .text_size(css(14.))
        .text_color(cx.theme().foreground)
        .focus_visible(|style| style.border_1().border_color(cx.theme().ring))
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
        .child(
            div()
                .relative()
                .size(css(20.))
                .flex_shrink_0()
                .rounded(css(2.))
                .border_1()
                .border_color(border)
                .bg(background)
                .when(checked, |view| {
                    view.child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let scale = f32::from(bounds.size.width) / 20.;
                                // CSS tickTop/tickBottom are separate 3px rounded bars.
                                for (x, y, angle, length) in [
                                    (8.6_f32, 16.4_f32, -145_f32, 15.4 * top),
                                    (0.6, 10., -50., 9.6 * bottom),
                                ] {
                                    let angle = angle.to_radians();
                                    let start = bounds.origin + point(px(x * scale), px(y * scale));
                                    let mut path = PathBuilder::stroke(px(3. * scale));
                                    path.move_to(start);
                                    path.line_to(
                                        start
                                            + point(
                                                px(-angle.sin() * length * scale),
                                                px(angle.cos() * length * scale),
                                            ),
                                    );
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, tick);
                                    }
                                }
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                }),
        )
        .child(div().pt(css(1.)).child(label))
}

pub(super) fn close_button(patch: bool, window: &mut Window, cx: &mut App) -> Button {
    let id = if patch {
        "alexa-patch-close"
    } else {
        "alexa-logout-close"
    };
    let state = window.use_keyed_state((ElementId::from(id), "interaction"), cx, |_, _| {
        Interaction::default()
    });
    let interaction = state.read(cx);
    let hover = motion::transition(
        (id, "crossfade"),
        if interaction.hovered { 1_f32 } else { 0_f32 },
        Transition::new(Duration::from_millis(if patch { 200 } else { 100 })).easing(if patch {
            Easing::Linear
        } else {
            Easing::EaseInOut
        }),
        window,
        cx,
    );
    let icon_size = if patch { 19. } else { 20. };
    Button::new(id)
        .absolute()
        .top_0()
        .right_0()
        .size(css(if patch { 36. } else { 30. }))
        .accessibility_label(text("TEXT_CLOSE"))
        .flex()
        .items_center()
        .justify_center()
        .focus_visible(|style| style.border_1().border_color(cx.theme().ring))
        .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
            state.hovered = *hovered;
            cx.notify();
        }))
        .child(
            div()
                .relative()
                .size(css(icon_size))
                .child(
                    img("synapse/alexa-close-gray.svg")
                        .size_full()
                        .opacity(if patch { 1. } else { 1. - hover }),
                )
                .child(
                    img(if patch {
                        "synapse/alexa-close-white.svg"
                    } else {
                        "synapse/alexa-close-green.svg"
                    })
                    .absolute()
                    .inset_0()
                    .size_full()
                    .opacity(hover),
                ),
        )
        .when(!patch, |button| button.active(|style| style.opacity(0.7)))
}

#[derive(IntoElement)]
pub(super) struct SourceSpinner {
    size: f32,
}

pub(super) fn spinner(size: f32) -> SourceSpinner {
    SourceSpinner { size }
}

fn spinner_frame(size: f32, phase: f32, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            // spinner.ef2d0235.svg: viewBox 100, r=30, stroke=10. SMIL has
            // keyTimes 0/.5/1, rotation 0/180/720, and dash lengths 10%/50%/10%.
            let scale = f32::from(bounds.size.width) / 100.;
            let center = bounds.center();
            let rotation = if phase <= 0.5 {
                phase * 360.
            } else {
                180. + (phase - 0.5) * 1080.
            };
            let fraction = if phase <= 0.5 {
                0.1 + phase * 0.8
            } else {
                0.5 - (phase - 0.5) * 0.8
            };
            for (start, length, alpha) in [
                (0., std::f32::consts::TAU, 0.3),
                (rotation.to_radians(), fraction * std::f32::consts::TAU, 1.),
            ] {
                let mut path = PathBuilder::stroke(px(10. * scale));
                for step in 0..=96 {
                    let angle = start + length * step as f32 / 96.;
                    let point = center
                        + point(px(angle.cos() * 30. * scale), px(angle.sin() * 30. * scale));
                    if step == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color.opacity(alpha));
                }
            }
        },
    )
    .size(css(size))
}

impl RenderOnce for SourceSpinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = cx.theme().primary;
        if cx.reduce_motion() {
            spinner_frame(self.size, 0.25, color).into_any_element()
        } else {
            div()
                .size(css(self.size))
                .with_animation(
                    "alexa-spinner-motion",
                    Animation::new(Duration::from_secs(2)).repeat(),
                    move |view, phase| view.child(spinner_frame(self.size, phase, color)),
                )
                .into_any_element()
        }
    }
}
