//! Actual mounted MapSensitivity factories; raw child payload is separate from
//! the parent local draft and from the still unconnected device submission.
use super::mapping_current::Action;
use super::*;
use gpui_kit::component::IndexPath;
use gpui_kit::component::select::{SelectEvent, SelectState};
use razer_widgets::theme::{MouseCustomizeColors, MouseMappingColors};

#[derive(Deserialize)]
struct SensitivityAction {
    id: String,
    content: String,
    payload: String,
}
#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    actions: Vec<SensitivityAction>,
    required: Vec<String>,
    exemptions: Vec<String>,
    exempt_by_id: bool,
    allow_cycle_turbo: bool,
    minimum: u32,
    maximum: u32,
    step: u32,
    support_xy: bool,
    group: String,
    labels: BTreeMap<String, String>,
    target: String,
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| crate::data::mouse_mapping_sensitivity_current_data!())
        .iter()
        .find(|p| p.product_id == product_id)
}
#[derive(Default)]
pub(super) struct State {
    values: [i64; 2],
    synced_values: [i64; 2],
    independent: bool,
    inputs: Vec<Entity<InputState>>,
    sliders: Vec<Entity<SliderState>>,
    syncing: bool,
    assignment_restored: bool,
}
// 1628 uses parseInt rather than Number. Invalid input is rejected, an empty
// draft remains empty until blur, and typing alone does not dirty MapSensitivity.
pub(super) fn parse_integer_prefix(text: &str) -> Option<i64> {
    let text = text.trim_start();
    let (sign, unsigned) = if let Some(text) = text.strip_prefix('-') {
        (-1, text)
    } else {
        (1, text.strip_prefix('+').unwrap_or(text))
    };
    if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        let end = hex
            .char_indices()
            .find_map(|(index, ch)| (!ch.is_ascii_hexdigit()).then_some(index))
            .unwrap_or(hex.len());
        return i64::from_str_radix(&hex[..end], 16)
            .ok()
            .map(|value| value * sign);
    }
    let prefix = text
        .char_indices()
        .find_map(|(index, ch)| {
            (!(ch.is_ascii_digit() || index == 0 && matches!(ch, '-' | '+'))).then_some(index)
        })
        .unwrap_or(text.len());
    text[..prefix].parse().ok()
}
impl MouseProductWorkspace {
    pub(super) fn initialize_current_mouse_sensitivity(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(policy) = policy(self.spec.product_id) else {
            return;
        };
        let cycle = button["buttonKey"] == "CycleUpSensitivityStages" && !policy.allow_cycle_turbo;
        let state = &mut self.mapping_current_state;
        state.choices = policy
            .actions
            .iter()
            .filter(|a| {
                if cycle {
                    !matches!(a.id.as_str(), "DPI_Clutch" | "DPI_OnTheFly")
                } else {
                    !self.hypershift || a.id != "DPI_OnTheFly"
                }
            })
            .map(|a| Action {
                id: a.id.clone(),
                content: a.content.clone(),
            })
            .collect();
        let restored = button["assignment"] == "SENSITIVITY"
            && button.get("assignmentGroup").is_some_and(|g| !g.is_null());
        state.selected = if restored {
            state
                .choices
                .iter()
                .position(|a| button["assignmentValue"] == a.content)
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        state.sensitivity.values = [800, 800];
        state.sensitivity.assignment_restored = restored;
        if restored && state.selected == 0 {
            let group = &button["assignmentGroup"];
            state.sensitivity.values = [
                group["x"].as_i64().unwrap_or(800),
                group["y"].as_i64().unwrap_or(800),
            ];
            state.sensitivity.independent = group["independent"] == true;
        }
        let selected = (state.selected != usize::MAX).then_some(IndexPath::new(state.selected));
        state.sensitivity.synced_values = state.sensitivity.values;
        let select = cx.new(|cx| SelectState::new(state.choice_items(), selected, window, cx));
        state
            .subscriptions
            .push(cx.subscribe_in(&select, window, |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let original = this
                    .mapping_input
                    .as_deref()
                    .and_then(|input| this.current_mapping_source_button(input));
                let state = &mut this.mapping_current_state;
                let Some(index) = state.choices.iter().position(|a| &a.id == id) else {
                    return;
                };
                let changed = state.selected != index
                    && original
                        .as_ref()
                        .is_none_or(|b| b["assignmentGroup"]["sensitivityAssignment"] != *id);
                state.selected = index;
                state.changed = changed;
                state.can_save = changed;
                this.refresh_current_sensitivity_requirement();
                cx.notify();
            }));
        state.select = Some(select);
        for axis in 0..2 {
            let value = state.sensitivity.values[axis];
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .mask_pattern(MaskPattern::None)
                    .default_value(value.to_string())
                    .validate(|text, _| {
                        text.encode_utf16().count() <= 5
                            && (text.is_empty() || parse_integer_prefix(text).is_some())
                    })
            });
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(policy.minimum as f32)
                    .max(policy.maximum as f32)
                    .step(policy.step as f32)
                    .default_value(value as f32)
            });
            state.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, event, window, cx| {
                    if this.mapping_current_state.sensitivity.syncing {
                        return;
                    }
                    if matches!(event, InputEvent::Blur) {
                        let text = input.read(cx).value().to_string();
                        let value = if text.is_empty() {
                            this.mapping_current_state.sensitivity.values[axis]
                        } else {
                            parse_integer_prefix(&text)
                                .unwrap_or(1800)
                                .clamp(policy.minimum as i64, policy.maximum as i64)
                        };
                        // The mounted consumer supplies no stepDPI to 1628. Its
                        // blur clamp must not round the text to the slider's step.
                        this.change_current_sensitivity_value(axis, value, window, cx);
                        input.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx)
                        });
                    } else if matches!(event, InputEvent::Change) {
                        let text = input.read(cx).value().to_string();
                        if let Some(value) = parse_integer_prefix(&text) {
                            let canonical = value.to_string();
                            if canonical != text {
                                this.mapping_current_state.sensitivity.syncing = true;
                                input
                                    .update(cx, |input, cx| input.set_value(canonical, window, cx));
                                this.mapping_current_state.sensitivity.syncing = false;
                            }
                        }
                    } else if matches!(event, InputEvent::PressEnter { .. }) {
                        window.blur(cx);
                    }
                },
            ));
            state.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if this.mapping_current_state.sensitivity.syncing {
                        return;
                    }
                    let value = match event {
                        SliderEvent::Change(value) | SliderEvent::Release(value) => {
                            value.start() as i64
                        }
                    };
                    this.change_current_sensitivity_value(axis, value, window, cx);
                },
            ));
            state.sensitivity.inputs.push(input);
            state.sensitivity.sliders.push(slider);
        }
        self.refresh_current_sensitivity_requirement();
        self.current_mouse_sensitivity_mount_save_guard();
    }
    fn refresh_current_sensitivity_requirement(&mut self) {
        let Some(policy) = policy(self.spec.product_id) else {
            return;
        };
        let state = &mut self.mapping_current_state;
        state.basic_requires_synapse = state.choices.get(state.selected).is_some_and(|a| {
            if policy.exemptions.is_empty() {
                policy.required.contains(&a.id)
            } else {
                !(policy.exemptions.contains(&a.content)
                    || policy.exempt_by_id && policy.exemptions.contains(&a.id))
            }
        });
    }
    pub(super) fn current_mouse_sensitivity_mount_save_guard(&mut self) {
        let state = &mut self.mapping_current_state;
        if state.function == "SENSITIVITY" && state.sensitivity.assignment_restored {
            state.can_save = false;
            state.changed = false;
        }
    }
    fn change_current_sensitivity_value(
        &mut self,
        axis: usize,
        value: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.mapping_current_state;
        if state.sensitivity.values[axis] == value {
            return;
        }
        state.sensitivity.values[axis] = value;
        if axis == 0 && !state.sensitivity.independent {
            state.sensitivity.values[1] = value;
        }
        state.changed = true;
        state.can_save = true;
        state.basic_requires_synapse = false;
        self.sync_current_sensitivity_controls(window, cx);
        cx.notify();
    }
    fn sync_current_sensitivity_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.mapping_current_state.sensitivity;
        state.syncing = true;
        for axis in 0..2 {
            let value = state.values[axis];
            if state.synced_values[axis] != value
                && let Some(input) = state.inputs.get(axis)
            {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
            }
            if let Some(slider) = state.sliders.get(axis) {
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
            }
            state.synced_values[axis] = value;
        }
        state.syncing = false;
    }
    pub(super) fn current_mouse_sensitivity_body(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_current_state;
        let mut body = v_flex().w_full().child(
            razer_widgets::synapse_select::select(state.select.as_ref()?)
                .items(state.choice_items())
                .accessibility_label(t("SENSITIVITY"))
                .w_full(),
        );
        let Some(action) = state.choices.get(state.selected) else {
            return Some(body.into_any_element());
        };
        if action.id == "DPI_Clutch" {
            let controls = &state.sensitivity;
            let number = |axis: usize| {
                let focus = controls.inputs[axis].focus_handle(cx);
                let focused = focus.is_focused(window);
                div()
                    .w(surface::css(60.))
                    .h(surface::css(26.))
                    .mr(surface::css(if axis == 0 { 5. } else { 0. }))
                    .border_1()
                    .track_focus(&focus)
                    .border_color(if focused {
                        MouseCustomizeColors::selected()
                    } else {
                        MouseCustomizeColors::line()
                    })
                    .bg(MouseCustomizeColors::selected_surface())
                    .text_color(MouseCustomizeColors::text())
                    .text_size(surface::css(14.))
                    .line_height(surface::css(19.))
                    .hover(|style| style.border_color(MouseCustomizeColors::selected()))
                    .child(
                        Input::new(&controls.inputs[axis])
                            .appearance(false)
                            .w_full()
                            .h_full()
                            .text_center(),
                    )
                    .on_key_down(|event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "escape" | "enter") {
                            window.blur(cx);
                            cx.stop_propagation();
                        }
                        if event.keystroke.key == "tab" {
                            cx.stop_propagation();
                        }
                    })
            };
            let mut numbers = h_flex()
                .items_center()
                .child(div().mr(surface::css(10.)).child("DPI"));
            if policy.support_xy && controls.independent {
                numbers = numbers.child(div().mr(surface::css(10.)).child("X"));
            }
            numbers = numbers.child(number(0));
            if controls.independent {
                numbers = numbers
                    .child(div().flex_1())
                    .child(div().mr(surface::css(10.)).child("Y"))
                    .child(number(1));
            }
            let mut clutch = v_flex()
                .mt(surface::css(10.))
                .mb(surface::css(20.))
                .child(numbers);
            if policy.support_xy {
                clutch = clutch.child(
                    Checkbox::new("mouse-current-clutch-xy")
                        .mt(surface::css(20.))
                        .label(t(&policy.labels["xy"]))
                        .checked(controls.independent)
                        .on_click(cx.listener(|this, _, window, cx| {
                            let state = &mut this.mapping_current_state;
                            if state.sensitivity.independent {
                                state.sensitivity.values[1] = state.sensitivity.values[0];
                            }
                            state.sensitivity.independent = !state.sensitivity.independent;
                            state.changed = true;
                            state.can_save = true;
                            state.basic_requires_synapse = false;
                            this.sync_current_sensitivity_controls(window, cx);
                            cx.notify();
                        })),
                );
            } else {
                clutch = clutch.child(div().h(surface::css(15.)));
            }
            for axis in 0..if controls.independent { 2 } else { 1 } {
                clutch = clutch.child(
                    super::mapping_sensitivity_slider_current::SensitivitySlider::new(
                        &controls.sliders[axis],
                        policy.minimum,
                        policy.maximum,
                        controls
                            .independent
                            .then_some(if axis == 0 { "X" } else { "Y" }),
                        !(controls.independent && axis == 0),
                    ),
                );
            }
            body = body.child(clutch);
        } else if action.id == "DPI_OnTheFly" {
            body = body.child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(MouseMappingColors::hypershift_description())
                    .mb(surface::css(20.))
                    .child(t(&policy.labels["on_the_fly"])),
            );
        } else {
            let rows = self.current_mouse_sensitivity_stages();
            let target = policy.target.clone();
            body = body.child(
                v_flex()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(MouseMappingColors::hypershift_description())
                    .mb(surface::css(20.))
                    .children(
                        rows.into_iter()
                            .flatten()
                            .filter(|row| row["visible"] == true)
                            .enumerate()
                            .map(|(index, row)| {
                                let value = if row["x"] == row["y"] {
                                    row["x"].to_string()
                                } else {
                                    format!("X:{} Y:{}", row["x"], row["y"])
                                };
                                div().child(format!(
                                    "{} {} - {}",
                                    t(&policy.labels["stage"]),
                                    index + 1,
                                    value
                                ))
                            }),
                    )
                    .child(
                        gpui_kit::base::Button::new("mouse-current-sensitivity-configure")
                            .p_0()
                            .mt(surface::css(10.))
                            .mb(surface::css(20.))
                            .text_color(MouseCustomizeColors::text())
                            .child(div().underline().child(t(&policy.labels["configure"])))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.request_current_mapping_page(&target, window, cx);
                            })),
                    ),
            );
        }
        Some(body.into_any_element())
    }
    pub(super) fn current_mouse_sensitivity_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let policy = policy(self.spec.product_id)?;
        let state = &self.mapping_current_state;
        let action = state.choices.get(state.selected)?;
        let action = policy.actions.iter().find(|a| a.id == action.id)?;
        let mut payload = json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,"outputType":policy.group});
        payload[&policy.group] = json!({"sensitivityAssignment":action.payload,"x":state.sensitivity.values[0],"y":state.sensitivity.values[1],"independent":state.sensitivity.independent});
        Some(vec![payload])
    }
}
