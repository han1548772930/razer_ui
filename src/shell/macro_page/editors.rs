//! Current 58190 `CustomInput` editor contracts.
//!
//! These constants mirror the source's local dropdown/stepper choices. They
//! are deliberately data-only: keyboard capture, device launch dialogs and
//! service dispatch stay outside the service-free MacroPage shell.

use super::*;
use gpui_kit::base::{NumberInput, StepAction};

pub(super) const MOUSE_ACTION_KEYS: [&str; 10] = [
    "TEXT_LEFT_CLICK",
    "TEXT_RIGHT_CLICK",
    "TEXT_SCROLL_CLICK",
    "TEXT_MOUSE_BUTTON_4",
    "TEXT_MOUSE_BUTTON_5",
    "TEXT_DOUBLE_CLICK",
    "TEXT_SCROLL_LEFT",
    "TEXT_SCROLL_RIGHT",
    "TEXT_SCROLL_UP",
    "TEXT_SCROLL_DOWN",
];

pub(super) fn loop_state_key(state: &str) -> &'static str {
    if state == "end" {
        "TEXT_END_LOOP_TEXT"
    } else {
        "TEXT_START_LOOP_TEXT"
    }
}

pub(super) fn valid_randomized_draft(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    integer.len() <= 3
        && integer.chars().all(|character| character.is_ascii_digit())
        && fraction.is_none_or(|fraction| {
            fraction.len() <= 2 && fraction.chars().all(|character| character.is_ascii_digit())
        })
        && parts.next().is_none()
        && value.parse::<f64>().is_ok_and(|value| value <= 5.0)
}

pub(super) fn normalize_randomized_range(
    min: &str,
    max: &str,
    changed: super::DelayBound,
) -> (String, String) {
    fn normalize(value: &str) -> Option<f64> {
        if !valid_randomized_draft(value) || value.is_empty() {
            return None;
        }
        value.parse::<f64>().ok().filter(|value| value.is_finite())
    }
    let min_value = normalize(min).unwrap_or(0.0);
    let max_value = normalize(max).unwrap_or(0.0);
    let changed_value = if changed == super::DelayBound::Min {
        min
    } else {
        max
    };
    if changed_value.is_empty() {
        return (min.to_string(), max.to_string());
    }
    let crossing = if changed == super::DelayBound::Min {
        min_value >= max_value
    } else {
        max_value < min_value
    };
    if crossing {
        return (min.to_string(), (min_value + 1.0).min(5.0).to_string());
    }
    (min.to_string(), max.to_string())
}

impl MacroPage {
    /// St updates a numeric Delay after 150ms while the input remains open.
    /// Invalidate on document/editor changes so a late callback cannot edit
    /// a different row after reorder, undo, save or navigation.
    pub(super) fn schedule_delay_edit(&mut self, cx: &mut Context<Self>) {
        self.numeric_generation = self.numeric_generation.wrapping_add(1);
        let Some(index) = self.editing_action else {
            return;
        };
        if !self
            .actions
            .get(index)
            .is_some_and(|item| item.kind == ActionKind::Delay)
        {
            return;
        }
        let raw = self.action_editor.read(cx).value().to_string();
        if raw.is_empty() {
            return;
        } // St keeps the empty draft until blur.
        let value = format_delay(parse_delay(&raw));
        let generation = self.numeric_generation;
        let document = self.current;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.numeric_generation != generation
                    || this.current != document
                    || this.actions_for != document
                    || this.editing_action != Some(index)
                    || this.recording_busy()
                {
                    return;
                }
                let Some(item) = this.actions.get(index) else {
                    return;
                };
                if item.kind != ActionKind::Delay || item.value == value && item.state == "fixed" {
                    return;
                }
                this.undo.push(this.actions.clone());
                this.actions[index].value = value;
                this.actions[index].state = "fixed".into();
                this.redo.clear();
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn numeric_value_editor(
        &self,
        index: usize,
        kind: ActionKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = cx.entity().downgrade();
        let arrow = |button: BaseButton, resource: &'static str| {
            button
                .w(css(18.))
                .h(css(12.))
                .p_0()
                .hover(|style| style.bg(rgba(0xffffff1a)))
                .active(|style| style.bg(rgba(0x0000001a)))
                .child(img(resource).size(css(8.)))
        };
        div()
            .w(css(88.))
            .h(css(27.))
            .border_1()
            .border_color(rgb(0x44d62c))
            .bg(rgb(0x111111))
            .child(
                NumberInput::new(&self.action_editor)
                    .size_full()
                    .controls_right()
                    .input(
                        Input::new(&self.action_editor)
                            .id(("macro-numeric-input", index))
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .h(css(25.))
                            .p_0()
                            .pl(css(6.))
                            .text_size(css(14.)),
                    )
                    .increment_button(move |button| arrow(button, "synapse/stepper-up.svg"))
                    .decrement_button(move |button| arrow(button, "synapse/stepper-down.svg"))
                    .on_step(move |action, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.step_numeric_input(kind, action, window, cx);
                        });
                    }),
            )
            .into_any_element()
    }

    fn step_numeric_input(
        &mut self,
        kind: ActionKind,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let step = if kind == ActionKind::Loop { 1.0 } else { 0.001 };
        let sign = if action == StepAction::Increment {
            1.0
        } else {
            -1.0
        };
        let value = parse_delay(&self.action_editor.read(cx).value());
        let next = if kind == ActionKind::Loop {
            ((value + sign * step).clamp(1.0, 99_999.0) as u32).to_string()
        } else {
            format_delay(value + sign * step)
        };
        self.action_editor.update(cx, |input, cx| {
            input.set_value(next, window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }
}
