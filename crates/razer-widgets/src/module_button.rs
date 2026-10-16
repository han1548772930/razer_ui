//! Current module-card actions, shared by dashboard and module views.
use crate::{surface, theme::MainPageColors};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Transition},
};
use gpui_kit::{component::*, prelude::FluentBuilder as _, *};
use std::time::Duration;
/// 6505's `.item-action.btn`: max-content width, 90px minimum, 27px border box.
/// Base owns activation; a direct text child keeps the source 12px measurement
/// independent of Component Button's full-size label slot and default type size.
pub fn module_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
    disabled: bool,
    cx: &App,
) -> ModuleAction {
    let id = id.into();
    let label = label.into().to_uppercase();
    let button = BaseButton::new(id.clone())
        .accessibility_label(label.clone())
        .disabled(disabled)
        .w_auto()
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .flex_shrink_0()
        .px(surface::css(16.))
        .pt(surface::css(7.))
        .pb(surface::css(6.))
        .border_1()
        .border_color(crate::theme::PaletteColors.swatch_border())
        .rounded(surface::css(2.))
        .text_size(surface::css(12.))
        .line_height(surface::css(12.))
        .whitespace_nowrap()
        .bg(if primary {
            cx.theme().primary
        } else {
            MainPageColors.module_action_gray()
        })
        .text_color(if primary {
            MainPageColors.banner_shade()
        } else {
            MainPageColors.banner_heading()
        })
        .cursor_default()
        .when(!disabled, |button| {
            button.focus_visible(|style| style.border_color(cx.theme().foreground))
        })
        .child(label);
    ModuleAction {
        id,
        button,
        disabled,
    }
}

/// Keeps the source opacity transition on the whole button, including its label
/// and border. Rendering supplies Window only for retained presentation state;
/// command activation, keyboard handling and focus remain owned by Base Button.
#[derive(IntoElement)]
pub struct ModuleAction {
    id: ElementId,
    button: BaseButton,
    disabled: bool,
}

impl ModuleAction {
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.button = self.button.on_click(handler);
        self
    }
}

impl Styled for ModuleAction {
    fn style(&mut self) -> &mut StyleRefinement {
        self.button.style()
    }
}

impl InteractiveElement for ModuleAction {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.button.interactivity()
    }
}

impl StatefulInteractiveElement for ModuleAction {}

#[derive(Default)]
struct ModuleActionInteraction {
    hovered: bool,
    pressed: bool,
}

impl RenderOnce for ModuleAction {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled;
        let state = window.use_keyed_state(
            (self.id.clone(), "module-button-interaction"),
            cx,
            |_, _| ModuleActionInteraction::default(),
        );
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
            (self.id, "module-button-opacity"),
            target,
            Transition::new(Duration::from_millis(200)).easing(Easing::EaseOut),
            window,
            cx,
        );
        self.button
            .opacity(opacity)
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
    }
}

/// The source's underlined `.info-text.link` is a text-sized in-app command.
pub fn module_detail_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> BaseButton {
    let label = label.into();
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .w_auto()
        .h_auto()
        .flex_shrink_0()
        .p_0()
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .whitespace_nowrap()
        .text_color(MainPageColors.card_caption())
        .underline()
        .cursor_default()
        .hover(|style| style.text_color(cx.theme().primary))
        .active(|style| style.text_color(cx.theme().primary.opacity(0.7)))
        .focus_visible(|style| style.text_color(cx.theme().primary))
        .child(label)
}
