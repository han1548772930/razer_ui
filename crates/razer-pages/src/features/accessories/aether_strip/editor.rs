use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{NumberInput, StepAction};
use std::time::Duration;

impl AetherStrip {
    pub(super) fn commit_side(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.inputs[id as usize]
            .read(cx)
            .value()
            .parse::<u32>()
            .unwrap_or(0);
        self.set_side(id, value, window, cx);
    }
    fn set_side(&mut self, id: u32, value: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.observation.enabled() {
            return;
        }
        let total = configured(&self.bends);
        let Some(bend) = self.bends.iter_mut().find(|b| b.id == id) else {
            return;
        };
        let max = bend
            .value
            .saturating_add(self.observation.detected.unwrap_or(0).saturating_sub(total));
        if max == 0 {
            return;
        }
        let value = value.clamp(1, max);
        let changed = value != bend.value;
        bend.value = value;
        self.sync_inputs(window, cx);
        if changed {
            self.request(
                "ON_SET_CHROMA_LED_NUMBER",
                json!({"bendData":self.bends}),
                cx,
            );
            cx.emit(AetherStripChanged);
        }
        cx.notify();
    }
    fn step_side(
        &mut self,
        id: u32,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = self.inputs[id as usize]
            .read(cx)
            .value()
            .parse::<u32>()
            .unwrap_or(0);
        let value = match action {
            StepAction::Increment => value.saturating_add(1),
            StepAction::Decrement => value.saturating_sub(1),
        };
        self.set_side(id, value, window, cx);
    }
}

#[derive(IntoElement)]
pub(super) struct LedInput {
    pub(super) input: Entity<InputState>,
    pub(super) owner: WeakEntity<AetherStrip>,
    pub(super) id: u32,
    pub(super) enabled: bool,
    pub(super) value: u32,
    pub(super) max: u32,
}
impl RenderOnce for LedInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = self.input.focus_handle(cx);
        let focused = focus.is_focused(window);
        let hover = razer_widgets::theme::DropdownColors::new().hover();
        let active = Colors::dialog().opacity(0.1);
        let arrow = move |button: gpui_kit::base::Button, path, limit| {
            button
                .w(surface::css(14.))
                .h(surface::css(12.))
                .p_0()
                .flex()
                .items_center()
                .justify_center()
                .opacity(if limit { 0.3 } else { 1. })
                .hover(move |v| v.bg(hover))
                .active(move |v| v.bg(active))
                .child(img(path).size(surface::css(8.)))
        };
        let owner = self.owner;
        let id = self.id;
        let state = window.use_keyed_state(
            (ElementId::from(("aether-led-input", id)), "hover"),
            cx,
            |_, _| false,
        );
        let border = motion::transition(
            (ElementId::from(("aether-led-input", id)), "border"),
            if self.enabled && (focused || *state.read(cx)) {
                cx.theme().primary
            } else {
                Colors::border()
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        div()
            .id(("aether-led-input", id))
            .track_focus(&focus)
            .w(surface::css(62.))
            .h(surface::css(26.))
            .mb(surface::css(10.))
            .border_1()
            .border_color(border)
            .bg(Colors::background())
            .on_hover(window.listener_for(&state, |hover, next, _, cx| {
                *hover = *next;
                cx.notify();
            }))
            .on_key_down(|event, window, cx| {
                // Current QG commits both Enter and Escape by blurring its input.
                if matches!(event.keystroke.key.as_str(), "enter" | "escape") {
                    window.blur(cx);
                }
            })
            .child(
                NumberInput::new(&self.input)
                    .disabled(!self.enabled)
                    .controls_right()
                    .size_full()
                    .input(
                        Input::new(&self.input)
                            .id(("aether-led-text", id))
                            .disabled(!self.enabled)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .p_0()
                            .pl(surface::css(5.))
                            .h(surface::css(24.))
                            .text_size(surface::css(14.)),
                    )
                    .increment_button(move |b| {
                        arrow(b, "synapse/stepper-up.svg", self.value >= self.max)
                    })
                    .decrement_button(move |b| {
                        arrow(b, "synapse/stepper-down.svg", self.value <= 1)
                    })
                    .on_step(move |action, window, cx| {
                        let _ = owner.update(cx, |view, cx| view.step_side(id, action, window, cx));
                    }),
            )
    }
}
