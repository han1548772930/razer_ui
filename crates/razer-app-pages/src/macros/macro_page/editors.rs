//! Current 58190 `CustomInput` editor contracts.
//!
//! These constants mirror the source's local dropdown/stepper choices. They
//! are deliberately data-only: keyboard capture, device launch dialogs and
//! service dispatch stay outside the service-free MacroPage shell.

use super::*;
use gpui_kit::base::{NumberInput, StepAction};

pub use razer_model::macro_library::MOUSE_ACTION_KEYS;

/// Number/isNaN acceptance is distinct from parseFloat submission.
pub(super) fn source_number(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if value.is_empty() {
        return Some(0.);
    }
    let radix = if value.starts_with("0x") || value.starts_with("0X") {
        Some(16)
    } else if value.starts_with("0b") || value.starts_with("0B") {
        Some(2)
    } else if value.starts_with("0o") || value.starts_with("0O") {
        Some(8)
    } else {
        None
    };
    let number = if let Some(radix) = radix {
        if value.len() == 2 {
            return None;
        }
        value[2..].chars().try_fold(0., |n, c| {
            c.to_digit(radix).map(|d| n * radix as f64 + d as f64)
        })?
    } else {
        value.parse::<f64>().ok()?
    };
    number.is_finite().then_some(number)
}

pub(super) fn source_float(raw: &str) -> Option<f64> {
    let value = raw.trim_start();
    let bytes = value.as_bytes();
    let mut end = usize::from(bytes.first().is_some_and(|c| matches!(c, b'+' | b'-')));
    if value[end..].starts_with("Infinity") {
        return Some(if bytes.first() == Some(&b'-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    let start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let mut digits = end - start;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        digits += end - start;
    }
    if digits == 0 {
        return None;
    }
    if bytes.get(end).is_some_and(|c| matches!(c, b'e' | b'E')) {
        let exponent = end;
        end += 1;
        if bytes.get(end).is_some_and(|c| matches!(c, b'+' | b'-')) {
            end += 1;
        }
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == start {
            end = exponent;
        }
    }
    value[..end].parse::<f64>().ok()
}

/// St.w and yt._ coerce numeric editors; Ot retains the complete command.
pub(super) fn committed_value(kind: ActionKind, raw: &str) -> String {
    match kind {
        ActionKind::Delay => format_delay(source_float(raw).unwrap_or(0.)),
        ActionKind::Loop => source_float(raw)
            .unwrap_or(1.)
            .clamp(1., 99_999.)
            .to_string(),
        ActionKind::Command => raw.to_string(),
        _ => raw.trim().to_string(),
    }
}

pub fn loop_state_key(state: &str) -> &'static str {
    if state == "end" {
        "TEXT_END_LOOP_TEXT"
    } else {
        "TEXT_START_LOOP_TEXT"
    }
}

pub fn valid_randomized_draft(value: &str) -> bool {
    randomized_draft(value).is_some()
}

fn randomized_draft(value: &str) -> Option<String> {
    if value.is_empty() {
        return Some(String::new());
    }
    source_number(value)?;
    let (integer, fraction) = value
        .split_once('.')
        .map_or((value, None), |(integer, fraction)| {
            (integer, Some(fraction))
        });
    let integer: String = integer.chars().take(3).collect();
    let normalized = if let Some(fraction) = fraction {
        format!("{integer}.{}", fraction.chars().take(2).collect::<String>())
    } else {
        integer
    };
    (source_float(&normalized)? <= 5.).then_some(normalized)
}

pub fn normalize_randomized_range(
    min: &str,
    max: &str,
    changed: super::DelayBound,
) -> (String, String) {
    let changed_value = if changed == super::DelayBound::Min {
        min
    } else {
        max
    };
    let Some(value) = randomized_draft(changed_value).filter(|value| !value.is_empty()) else {
        return (min.to_string(), max.to_string());
    };
    let number = source_float(&value).expect("Accepted randomized draft is numeric");
    let min_value = source_number(min);
    let max_value = source_number(max);
    if changed == super::DelayBound::Min {
        if max_value.is_some_and(|max| number >= max) {
            let min = number.max(0.);
            (min.to_string(), (min + 1.).min(5.).to_string())
        } else {
            (value, max.to_string())
        }
    } else if let Some(min_value) = min_value.filter(|min| number < *min) {
        (min.to_string(), (min_value + 1.).min(5.).to_string())
    } else {
        (min.to_string(), value)
    }
}

impl MacroPage {
    /// St, yt and Ot update their action after 150ms while the input is open.
    /// Invalidate on document/editor changes so a late callback cannot edit
    /// a different row after reorder, undo, save or navigation.
    pub fn action_input_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.editing_action else {
            return;
        };
        let Some(kind) = self.actions.get(index).map(|item| item.kind) else {
            return;
        };
        if !matches!(
            kind,
            ActionKind::Delay | ActionKind::Loop | ActionKind::Command
        ) {
            return;
        }
        let raw = self.action_editor.read(cx).value().to_string();
        if kind == ActionKind::Delay && raw.is_empty() {
            // St returns before clearing its previous numeric timer.
            return;
        }
        if kind == ActionKind::Delay {
            self.action_edit_tasks.clear();
            self.numeric_generation = self.numeric_generation.wrapping_add(1);
        } else {
            // yt captures each normalized count; Ot reads the current command
            // when its timer fires. Only St retains and cancels its timer.
            self.action_edit_tasks.retain(|task| !task.is_ready());
        }
        let value = committed_value(kind, &raw);
        let draft = if kind == ActionKind::Delay && raw.ends_with('.') {
            raw
        } else {
            value.clone()
        };
        if self.action_editor.read(cx).value().as_ref() != draft {
            self.action_editor
                .update(cx, |input, cx| input.set_value(draft, window, cx));
        }
        let generation = self.numeric_generation;
        let document = self.current;
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
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
                let value = if kind == ActionKind::Command {
                    this.action_editor.read(cx).value().to_string()
                } else {
                    value
                };
                if item.kind != kind
                    || item.value == value && (kind != ActionKind::Delay || item.state == "fixed")
                {
                    return;
                }
                this.undo.push(this.actions.clone());
                if kind == ActionKind::Loop && this.action_editor.read(cx).value().as_ref() != value
                {
                    // yt's item effect consumes each submitted numeric count.
                    this.action_editor.update(cx, |input, cx| {
                        input.set_value(value.clone(), window, cx);
                    });
                }
                this.actions[index].value = value;
                if kind == ActionKind::Delay {
                    this.actions[index].state = "fixed".into();
                }
                this.sync_loop_value(index);
                this.redo.clear();
                cx.notify();
            });
        });
        self.action_edit_tasks.push(task);
    }

    pub fn numeric_value_editor(
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
        let sign = if action == StepAction::Increment { 1.0 } else { -1.0 };
        let value = source_float(&self.action_editor.read(cx).value()).unwrap_or(0.);
        let next = if kind == ActionKind::Loop {
            (value + sign * step).clamp(1.0, 99_999.0).to_string()
        } else {
            format_delay(value + sign * step)
        };
        self.action_editor.update(cx, |input, cx| {
            input.set_value(next, window, cx);
            input.select_all(window, cx);
        });
        if kind == ActionKind::Delay {
            self.action_input_changed(window, cx);
        }
        cx.notify();
    }
}
