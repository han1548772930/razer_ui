//! Current lazy MapMouse implementations, audited individually per product.
//! The host submission chain is still a separate, explicit gap: edits here
//! update the local profile draft and never acknowledge a device write.
use super::super::Choice;
use super::*;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::IndexPath;
use gpui_kit::component::select::{SelectEvent, SelectState};
use razer_widgets::{
    source_slider::SourceSlider,
    stepper::{Stepper, StepperEvent},
    theme::{MouseCustomizeColors, MouseMappingColors},
};

#[derive(Clone, Deserialize)]
pub(super) struct Action {
    pub(super) id: String,
    pub(super) content: String,
}
#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    base: Vec<Action>,
    extended: Vec<Action>,
    boss: Vec<Action>,
    scroll_mode: bool,
    scroll_mode_label: String,
    no_turbo: Vec<String>,
    scroll_mode_disables_turbo: bool,
    no_scroll_click_repeat: bool,
    secondary_assignment_override: bool,
    required_kind: String,
    exempt_by_id: bool,
    is_obm: bool,
    required_defaults: Vec<String>,
    required_click: String,
    required: BTreeMap<String, Vec<String>>,
}
fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| crate::data::mouse_mapping_actions_current_data!())
        .iter()
        .find(|policy| policy.product_id == product_id)
}

#[derive(Default)]
pub(super) struct State {
    pub(super) profile: super::mapping_profile_70::State,
    pub(super) macros: super::mapping_macro_70::State,
    pub(super) keyboard: super::mapping_keyboard_70::State,
    pub(super) lighting: super::mapping_lighting_70::State,
    pub(super) inter: super::mapping_inter_70::State,
    pub(super) input: Option<String>,
    pub(super) function: String,
    // Parent state, retained across child function/button changes. Closing the
    // actual popup clears it; index-zero search does not assign a new value.
    pub(super) search_value: Option<usize>,
    pub(super) choices: Vec<Action>,
    pub(super) select: Option<Entity<SelectState<Vec<Choice>>>>,
    slider: Option<Entity<SliderState>>,
    stepper: Option<Entity<Stepper>>,
    pub(super) subscriptions: Vec<Subscription>,
    pub(super) selected: usize,
    turbo: bool,
    speed: Value,
    syncing: bool,
    pub(super) changed: bool,
    pub(super) can_save: bool,
    pub(super) basic_requires_synapse: bool,
    pub(super) text_value: String,
    pub(super) text_editor: Option<Entity<crate::features::mapping_text_editor::MappingTextEditor>>,
    pub(super) sensitivity: super::mapping_sensitivity_current::State,
    pub(super) launch: super::mapping_launch_current::State,
    reset: bool,
    requirement_event: RequirementEvent,
}
#[derive(Default, Clone, Copy, PartialEq)]
enum RequirementEvent {
    #[default]
    Mount,
    Option,
    Turbo,
}
impl State {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    fn speed_number(&self) -> i64 {
        self.speed
            .as_i64()
            .or_else(|| self.speed.as_str().and_then(|text| text.parse().ok()))
            .unwrap_or(0)
            .clamp(1, 20)
    }
    pub(super) fn choice_items(&self) -> Vec<Choice> {
        self.choices
            .iter()
            .map(|action| Choice::localized(action.id.clone(), action.content.clone()))
            .collect()
    }
}

impl MouseProductWorkspace {
    pub(super) fn current_mouse_mapping_changed(&self) -> bool {
        self.mapping_current_state.changed
    }

    /// Confirms that the local editor closed; this does not acknowledge a
    /// device submission. Unsupported shapes retain the editor and pending UI.
    pub(super) fn save_current_mouse_mapping_for_navigation(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.mapping_input.is_none() {
            return true;
        }
        self.save_current_mouse_mapping(cx);
        self.mapping_input.is_none()
    }

    pub(super) fn discard_current_mouse_mapping(&mut self, cx: &mut Context<Self>) {
        self.set_current_customize_input(None, false);
        self.mapping_current_state.clear();
        cx.notify();
    }

    pub(super) fn change_current_mouse_mapping_function(
        &mut self,
        function: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if function == "AI_LAUNCHER" {
            return;
        }
        let Some(input) = self.mapping_input.clone() else {
            return;
        };
        let Some(button) = self.current_mapping_source_button(&input) else {
            return;
        };
        let mapped = button["isMapped"] == true;
        let can_save = !matches!(
            function,
            "TEXT_FUNCTION" | "KEYBOARD_FUNCTION" | "LAUNCH_PROGRAM" | "INTERDEVICE"
        ) && (function != "DEFAULT" || mapped)
            && (!mapped || button["assignment"] != function);
        let remounted = self.mapping_current_state.function != function;
        if remounted {
            // Changing the React child type unmounts MapMouse. Returning to it
            // reads the original activeButton, rather than retaining unsaved
            // slider/select values from the previous child instance.
            self.initialize_current_mouse_mapping(input, Some(function), window, cx);
        }
        let local_child_flags = (self.spec.product_id == 70
            && matches!(
                function,
                "MACRO" | "SWITCH_PROFILE" | "SWITCH_LIGHTING" | "INTERDEVICE"
            )
            && remounted)
            .then_some((
                self.mapping_current_state.can_save,
                self.mapping_current_state.changed,
            ));
        self.mapping_current_state.reset = function == "DEFAULT";
        self.mapping_current_state.changed = false;
        self.mapping_current_state.can_save = can_save;
        self.mapping_current_state.requirement_event = RequirementEvent::Mount;
        if let Some((can_save, changed)) = local_child_flags {
            self.mapping_current_state.can_save = can_save;
            self.mapping_current_state.changed = changed;
        }
        if remounted {
            self.current_mouse_basic_mount_save_guard(&button);
            self.current_mouse_sensitivity_mount_save_guard();
            self.apply_current_mouse_search_on_mount(window, cx);
        }
        cx.notify();
    }

    pub(super) fn current_mapping_source_button(&self, input: &str) -> Option<Value> {
        let originals = self.spec.groups.get(self.group_ix)?["buttonList"].as_array()?;
        let all = self
            .spec
            .groups
            .iter()
            .flat_map(|group| {
                group["buttonList"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned()
            })
            .collect::<Vec<_>>();
        let buttons = button_state_current::derive(
            self.spec.product_id,
            originals,
            &all,
            self.draft["mappings"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            self.hypershift,
            self.group_ix as i32,
        )
        .ok()?;
        buttons
            .into_iter()
            .find(|button| button["inputID"] == input)
    }

    fn current_mapping_turbo_disabled(&self, button: &Value) -> bool {
        let Some(policy) = policy(self.spec.product_id) else {
            return true;
        };
        policy
            .no_turbo
            .iter()
            .any(|key| button["buttonKey"] == *key)
            || button["inputType"] == "ControllerInput"
            || button["disableTurboInAssignment"]
                .as_array()
                .is_some_and(|list| list.iter().any(|function| function == "MOUSE_FUNCTION"))
            || policy.scroll_mode_disables_turbo
                && self
                    .mapping_current_state
                    .choices
                    .get(self.mapping_current_state.selected)
                    .is_some_and(|action| action.id == "ScrollModeSwitch")
    }

    pub(super) fn current_mapping_requires_synapse(&self) -> bool {
        if self.mapping_current_state.function != "MOUSE_FUNCTION" {
            return self.mapping_current_state.basic_requires_synapse;
        }
        let Some(policy) = policy(self.spec.product_id) else {
            return false;
        };
        let state = &self.mapping_current_state;
        let Some(action) = state.choices.get(state.selected) else {
            return false;
        };
        if state.reset {
            return false;
        }
        if policy.required_kind == "event_inline" {
            let required = policy.required_defaults.contains(&action.content);
            if state.requirement_event == RequirementEvent::Option {
                return required
                    && !(policy.is_obm
                        && matches!(
                            action.content.as_str(),
                            "REPEAT_SCROLL_RIGHT"
                                | "REPEAT_SCROLL_LEFT"
                                | "REPEAT_SCROLL_DOWN"
                                | "REPEAT_SCROLL_UP"
                                | "SCROLL_UP"
                                | "SCROLL_DOWN"
                        ));
            }
            return required || action.id == policy.required_click && state.turbo;
        }
        if policy.required_kind == "check" {
            if action.id == policy.required_click && state.turbo {
                return true;
            }
            let name = if state.turbo {
                "MOUSE_TURBO_FUNC_REQUIRED_SYNAPSE_KEYS"
            } else {
                "MOUSE_FUNC_REQUIRED_SYNAPSE_KEYS"
            };
            if let Some(override_keys) = policy.required.get(name).filter(|keys| !keys.is_empty()) {
                return override_keys.contains(&action.content);
            }
            return policy.required_defaults.contains(&action.content)
                && !(policy.is_obm
                    && matches!(
                        action.content.as_str(),
                        "REPEAT_SCROLL_RIGHT"
                            | "REPEAT_SCROLL_LEFT"
                            | "REPEAT_SCROLL_DOWN"
                            | "REPEAT_SCROLL_UP"
                            | "SCROLL_UP"
                            | "SCROLL_DOWN"
                    ));
        }
        let exempt = policy.required.get("SYNAPSE_NOT_REQUIRED_FUNCTIONS");
        if exempt.is_some_and(|keys| {
            keys.contains(&action.content) || policy.exempt_by_id && keys.contains(&action.id)
        }) {
            return false;
        }
        policy.required_defaults.contains(&action.content)
            || action.id == policy.required_click && state.turbo
    }

    pub(super) fn open_current_mouse_mapping(
        &mut self,
        input: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_function = self.mapping_current_state.function.clone();
        let had_child = self.mapping_current_state.input.is_some();
        self.initialize_current_mouse_mapping(input, None, window, cx);
        if !had_child || previous_function != self.mapping_current_state.function {
            self.apply_current_mouse_search_on_mount(window, cx);
        }
    }

    /// Actual 199/208 handleOpenFunction; deliberately distinct from the
    /// ordinary category click because positive numeric indices are passed to
    /// the source parent's changeFunction before the named function is set.
    pub(super) fn apply_current_mouse_remap_search(
        &mut self,
        function: &str,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 199 | 208) {
            return;
        }
        let Some(input) = self.mapping_input.clone() else {
            return;
        };
        let Some(button) = self.current_mapping_source_button(&input) else {
            return;
        };
        let previous_search = self.mapping_current_state.search_value;
        let remounted = self.mapping_current_state.function != function;
        let previous_gate = (
            self.mapping_current_state.can_save,
            self.mapping_current_state.changed,
        );
        if remounted {
            self.initialize_current_mouse_mapping(input, Some(function), window, cx);
        }
        if index != 0 {
            // All imported parent function enums are strings. A positive
            // numeric index passes checkSave and no explicit-selection gate;
            // parent changeFunction clears dirty before child lifecycle runs.
            self.mapping_assignment = None;
            self.mapping_current_state.search_value = Some(index);
            self.mapping_current_state.can_save = true;
            self.mapping_current_state.changed = false;
            self.mapping_current_state.basic_requires_synapse = false;
            self.collapse_current_mouse_mapping_menu();
        } else if remounted {
            // setState({currentFunction}) alone leaves parent save flags intact.
            self.mapping_current_state.can_save = previous_gate.0;
            self.mapping_current_state.changed = previous_gate.1;
        }
        if remounted {
            self.current_mouse_basic_mount_save_guard(&button);
            self.current_mouse_sensitivity_mount_save_guard();
            self.apply_current_mouse_search_on_mount(window, cx);
        } else if index != 0 && previous_search != Some(index) {
            self.apply_current_mouse_search_option(index, window, cx);
        }
        cx.notify();
    }

    fn apply_current_mouse_search_on_mount(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .mapping_current_state
            .search_value
            .filter(|index| *index != 0)
        {
            self.apply_current_mouse_search_option(index, window, cx);
        }
    }

    fn apply_current_mouse_search_option(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 199 | 208) {
            return;
        }
        if self.mapping_current_state.function != "MOUSE_FUNCTION" {
            // KeyboardCombineMouse remains an explicit form gap. Sensitivity,
            // Text and the other children have no source searchValue consumer.
            self.apply_current_mouse_basic_search_option(index, window, cx);
            return;
        }
        let state = &mut self.mapping_current_state;
        state.selected = index;
        state.changed = true;
        state.can_save = true;
        state.reset = false;
        state.requirement_event = RequirementEvent::Option;
        if let Some(select) = &state.select {
            select.update(cx, |select, cx| {
                select.set_selected_index(Some(IndexPath::new(index)), window, cx)
            });
        }
    }

    fn initialize_current_mouse_mapping(
        &mut self,
        input: String,
        function_override: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mapping_input.as_ref() == Some(&input) && function_override.is_none() {
            return;
        }
        let Some(button) = self.current_mapping_source_button(&input) else {
            return;
        };
        if button["isEnabled"] == false || button["disabled"] == true {
            return;
        }
        if function_override.is_none() {
            self.set_current_customize_input(Some(input.clone()), true);
        }
        let search_value = self.mapping_current_state.search_value;
        let retain_search =
            self.mapping_current_state.input.is_some() && matches!(self.spec.product_id, 199 | 208);
        self.mapping_assignment = None;
        self.mapping_current_state.clear();
        if retain_search {
            self.mapping_current_state.search_value = search_value;
        }
        let function = function_override.unwrap_or_else(|| {
            if button["isMapped"] == true {
                button["assignment"].as_str().unwrap_or("DEFAULT")
            } else {
                "DEFAULT"
            }
        });
        self.mapping_current_state.input = Some(input.clone());
        self.mapping_current_state.function = function.to_owned();
        self.mapping_current_state.reset = function == "DEFAULT";
        if function == "DEFAULT" {
            cx.notify();
            return;
        }
        if function == "TEXT_FUNCTION" {
            self.initialize_current_mouse_text(&button, window, cx);
            cx.notify();
            return;
        }
        if function == "SENSITIVITY" {
            self.initialize_current_mouse_sensitivity(&button, window, cx);
            cx.notify();
            return;
        }
        if function == "LAUNCH_PROGRAM" {
            self.initialize_current_mouse_launch(&button, window, cx);
            cx.notify();
            return;
        }
        if self.spec.product_id == 70 && function == "SWITCH_PROFILE" {
            self.initialize_mouse_profile_70(&button, window, cx);
            cx.notify();
            return;
        }
        if self.spec.product_id == 70 && function == "MACRO" {
            self.initialize_mouse_macro_70(&button, window, cx);
            cx.notify();
            return;
        }
        if self.spec.product_id == 70 && function == "KEYBOARD_FUNCTION" {
            self.initialize_mouse_keyboard_70(&button, window, cx);
            cx.notify();
            return;
        }
        if self.spec.product_id == 70 && function == "SWITCH_LIGHTING" {
            self.initialize_mouse_mapping_lighting_70(&button, window, cx);
            cx.notify();
            return;
        }
        if self.spec.product_id == 70 && function == "INTERDEVICE" {
            self.initialize_mouse_inter_70(&button, window, cx);
            cx.notify();
            return;
        }
        if function != "MOUSE_FUNCTION" {
            self.initialize_current_mouse_basic(&button, window, cx);
            cx.notify();
            return;
        }
        let Some(policy) = policy(self.spec.product_id) else {
            cx.notify();
            return;
        };
        let key = button["buttonKey"].as_str().unwrap_or("");
        let no_turbo = policy.no_turbo.iter().any(|value| value == key)
            || button["inputType"] == "ControllerInput"
            || button["disableTurboInAssignment"]
                .as_array()
                .is_some_and(|list| list.iter().any(|function| function == "MOUSE_FUNCTION"));
        let mut actions = policy.base.clone();
        // N(activeButton) removes repeat actions for ScrollClick on the newer
        // branch. _checkDisableTurbo also removes extended actions per input.
        if !(policy.no_scroll_click_repeat && key == "ScrollClick") {
            actions.extend(policy.extended.clone());
        }
        actions.extend(policy.boss.clone());
        if policy.scroll_mode && !matches!(key, "ScrollUp" | "ScrollDown") {
            actions.push(Action {
                id: "ScrollModeSwitch".into(),
                content: policy.scroll_mode_label.clone(),
            });
        }
        // assignDefaultValues consumes the projected activeButton. Looking up
        // only a flat profile mapping loses the original secondary/nested value.
        let current =
            (button["assignment"] == "MOUSE_FUNCTION").then_some(&button["assignmentGroup"]);
        let content = if policy.secondary_assignment_override
            && button["secondaryAssignment"] == "MOUSE_FUNCTION"
            && button["secondaryAssignmentValue"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        {
            &button["secondaryAssignmentValue"]
        } else {
            &button["assignmentValue"]
        };
        let selected = if current.is_some() {
            actions
                .iter()
                .position(|action| *content == action.content)
                .unwrap_or(0)
        } else {
            0
        };
        // assignDefaultValues selects from the constructor's N(activeButton)
        // first; _checkDisableTurbo then removes extended rows without
        // translating selectedOption. Preserve even that source ordering.
        if no_turbo {
            actions.retain(|action| {
                !policy
                    .extended
                    .iter()
                    .any(|extended| extended.id == action.id)
            });
        }
        let turbo = current.is_some_and(|group| group["turboMode"]["isTurbo"] == true);
        let speed = current
            .and_then(|group| group["turboMode"].get("keysPerSecond"))
            .cloned()
            .unwrap_or(json!(7));
        let state = &mut self.mapping_current_state;
        state.input = Some(input);
        state.choices = actions;
        state.selected = selected;
        state.turbo = turbo;
        state.speed = speed;
        state.reset = false;
        let items = state.choice_items();
        let select =
            cx.new(|cx| SelectState::new(items, Some(IndexPath::new(selected)), window, cx));
        let slider = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(20.)
                .step(1.)
                .default_value(state.speed_number() as f32)
        });
        let stepper = cx.new(|cx| {
            Stepper::new(
                "mouse-current-map-turbo-stepper",
                state.speed_number() as f64,
                (1., 20., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_custom_keymapping()
            .live_update()
        });
        state
            .subscriptions
            .push(cx.subscribe_in(&select, window, |this, _, event, _, cx| {
                if this.mapping_current_state.syncing {
                    return;
                }
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let state = &mut this.mapping_current_state;
                if let Some(index) = state.choices.iter().position(|action| &action.id == id) {
                    state.selected = index;
                    state.changed = true;
                    state.can_save = true;
                    state.reset = false;
                    state.requirement_event = RequirementEvent::Option;
                    cx.notify();
                }
            }));
        state.subscriptions.push(cx.subscribe_in(
            &stepper,
            window,
            |this, _, event: &StepperEvent, window, cx| {
                if this.mapping_current_state.syncing {
                    return;
                }
                let state = &mut this.mapping_current_state;
                state.speed = event
                    .draft
                    .as_ref()
                    .map_or_else(|| json!(event.value as i64), |draft| json!(draft));
                state.changed = true;
                state.can_save = true;
                state.reset = false;
                state.requirement_event = RequirementEvent::Turbo;
                state.syncing = true;
                if let Some(slider) = &state.slider {
                    let numeric = event
                        .draft
                        .as_ref()
                        .and_then(|draft| draft.parse::<f32>().ok())
                        .unwrap_or(event.value as f32);
                    slider.update(cx, |slider, cx| slider.set_value(numeric, window, cx));
                }
                state.syncing = false;
                cx.notify();
            },
        ));
        state.subscriptions.push(
            cx.subscribe_in(&slider, window, |this, _, event, window, cx| {
                if this.mapping_current_state.syncing {
                    return;
                }
                match event {
                    SliderEvent::Change(value) => {
                        this.mapping_current_state.syncing = true;
                        if let Some(stepper) = &this.mapping_current_state.stepper {
                            stepper.update(cx, |stepper, cx| {
                                stepper.sync_value(
                                    value.start().round() as f64,
                                    1.,
                                    false,
                                    window,
                                    cx,
                                )
                            });
                        }
                        this.mapping_current_state.syncing = false;
                        cx.notify();
                    }
                    SliderEvent::Release(value) => {
                        this.update_current_mouse_turbo(value.start().round() as i64, window, cx)
                    }
                }
            }),
        );
        state.select = Some(select);
        state.slider = Some(slider);
        state.stepper = Some(stepper);
        cx.notify();
    }

    fn update_current_mouse_turbo(
        &mut self,
        value: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.clamp(1, 20);
        let state = &mut self.mapping_current_state;
        state.speed = json!(value);
        state.changed = true;
        state.can_save = true;
        state.reset = false;
        state.requirement_event = RequirementEvent::Turbo;
        state.syncing = true;
        if let Some(stepper) = &state.stepper {
            stepper.update(cx, |stepper, cx| {
                stepper.sync_value(value as f64, 1., false, window, cx)
            });
        }
        if let Some(slider) = &state.slider {
            slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
        }
        state.syncing = false;
        cx.notify();
    }

    fn save_current_mouse_mapping(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.mapping_input.clone() else {
            return;
        };
        let Some(button) = self.current_mapping_source_button(&input) else {
            return;
        };
        let state = &self.mapping_current_state;
        if !state.can_save
            || state.input.as_ref() != Some(&input)
            || button["isEnabled"] == false
            || button["disabled"] == true
        {
            return;
        }
        let mut mappings = self.draft["mappings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let reset_both_layers = !self.hypershift
            && (state.function == "RAZER_HYPERSHIFT"
                || button["assignment"] == "RAZER_HYPERSHIFT"
                || state.reset && button["assignment"] == "DISABLE");
        let payloads = if state.reset {
            Vec::new()
        } else if state.function == "MOUSE_FUNCTION" {
            let Some(action) = state.choices.get(state.selected) else {
                return;
            };
            let mut group = json!({"mouseAssignment":action.id});
            if state.turbo {
                group["turboMode"] = json!({"isTurbo":true,"keysPerSecond":state.speed});
            }
            vec![
                json!({"inputID":input,"inputType":button["inputType"],"isHyperShift":self.hypershift,"outputType":"mouseGroup","mouseGroup":group}),
            ]
        } else if state.function == "TEXT_FUNCTION" {
            let Some(payloads) = self.current_mouse_text_payload(&button) else {
                return;
            };
            payloads
        } else if state.function == "SENSITIVITY" {
            let Some(payloads) = self.current_mouse_sensitivity_payload(&button) else {
                return;
            };
            payloads
        } else if state.function == "LAUNCH_PROGRAM" {
            let Some(payloads) = self.current_mouse_launch_payload(&button) else {
                return;
            };
            payloads
        } else if self.spec.product_id == 70 && state.function == "SWITCH_PROFILE" {
            let Some(payloads) = self.mouse_profile_70_payload(&button) else {
                return;
            };
            payloads
        } else if self.spec.product_id == 70 && state.function == "MACRO" {
            let Some(payloads) = self.mouse_macro_70_payload(&button) else {
                return;
            };
            payloads
        } else if self.spec.product_id == 70 && state.function == "KEYBOARD_FUNCTION" {
            let Some(payloads) = self.mouse_keyboard_70_payload(&button) else {
                return;
            };
            payloads
        } else if self.spec.product_id == 70 && state.function == "SWITCH_LIGHTING" {
            let Some(payloads) = self.mouse_mapping_lighting_70_payload(&button) else {
                return;
            };
            payloads
        } else if self.spec.product_id == 70 && state.function == "INTERDEVICE" {
            let Some(payloads) = self.mouse_inter_70_payload(&button) else {
                return;
            };
            payloads
        } else if let Some(payloads) = self.current_mouse_basic_payloads(&button) {
            payloads
        } else {
            cx.emit(MouseMappingSaveRejected::UnsupportedFunction);
            return;
        };
        if mappings.iter().any(|mapping| {
            mapping["inputID"] == input
                && (reset_both_layers
                    || mapping["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift)
                && mapping.get("mapping").is_some()
        }) {
            // The parent packing/submission callback is not yet reconstructed
            // for this shape. Retain the editor and both assignments, rather
            // than projecting a flattened local save as if it were verified.
            cx.emit(MouseMappingSaveRejected::UnsupportedFunction);
            return;
        }
        if payloads.is_empty() {
            self.current_mouse_restore_primary(
                &mut mappings,
                &button,
                &json!({"outputType":"defaultGroup"}),
            );
        } else {
            for payload in &payloads {
                self.current_mouse_restore_primary(&mut mappings, &button, payload);
            }
        }
        mappings.retain(|mapping| {
            !(mapping["inputID"] == input
                && (reset_both_layers
                    || mapping["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift))
        });
        mappings.extend(payloads);
        self.draft["mappings"] = json!(mappings);
        self.set_current_customize_input(None, false);
        self.mapping_current_state.clear();
        cx.emit(MouseProductChanged);
        if self.spec.product_id == 70 {
            cx.emit(super::dpi_70::MouseLegacyMappingCommitted);
        }
        cx.notify();
    }

    pub(super) fn current_mouse_mapping_body(
        &self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = &self.mapping_current_state;
        let input = button["inputID"].as_str()?;
        if state.input.as_deref() != Some(input) {
            return None;
        }
        let mut body = v_flex().w(surface::css(210.)).mb(surface::css(20.)).child(
            div()
                .mb(surface::css(20.))
                .font_family("RazerF5")
                .text_size(surface::css(16.))
                .line_height(surface::css(19.))
                .text_color(MouseCustomizeColors::selected())
                .child(t(&state.function).to_uppercase()),
        );
        if state.function == "MOUSE_FUNCTION" {
            let select = state.select.as_ref()?;
            body = body.child(
                razer_widgets::synapse_select::select(select)
                    .items(state.choice_items())
                    .accessibility_label(t("MOUSE_FUNCTION"))
                    .w_full(),
            );
        } else if state.reset {
            body = body.child(self.current_mouse_default_body(button, cx));
        } else if state.function == "TEXT_FUNCTION" {
            body = body.children(self.current_mouse_text_body());
        } else if state.function == "SENSITIVITY" {
            body = body.children(self.current_mouse_sensitivity_body(window, cx));
        } else if state.function == "LAUNCH_PROGRAM" {
            body = body.children(self.current_mouse_launch_body(cx));
        } else if self.spec.product_id == 70 && state.function == "SWITCH_PROFILE" {
            body = body.child(self.mouse_profile_70_body(cx));
        } else if self.spec.product_id == 70 && state.function == "MACRO" {
            body = body.child(self.mouse_macro_70_body(cx));
        } else if self.spec.product_id == 70 && state.function == "KEYBOARD_FUNCTION" {
            body = body.child(self.mouse_keyboard_70_body(window, cx));
        } else if self.spec.product_id == 70 && state.function == "SWITCH_LIGHTING" {
            body = body.child(self.mouse_mapping_lighting_70_body(cx));
        } else if self.spec.product_id == 70 && state.function == "INTERDEVICE" {
            body = body.child(self.mouse_inter_70_body(window, cx));
        } else if let Some(basic) = self.current_mouse_basic_body() {
            body = body.child(basic);
        }
        if state.function == "MOUSE_FUNCTION" && !self.current_mapping_turbo_disabled(button) {
            body = body.child(
                h_flex()
                    .mt(surface::css(10.))
                    .gap(surface::css(5.))
                    .child(
                        Checkbox::new("mouse-current-map-turbo")
                            .label(t("ENABLE_TURBO"))
                            .checked(state.turbo)
                            .on_click(cx.listener(|this, on, _, cx| {
                                this.mapping_current_state.turbo = *on;
                                this.mapping_current_state.changed = true;
                                this.mapping_current_state.can_save = true;
                                this.mapping_current_state.reset = false;
                                this.mapping_current_state.requirement_event =
                                    RequirementEvent::Turbo;
                                cx.notify();
                            })),
                    )
                    .child(
                        razer_widgets::source_tooltip::SourceTooltip::new(
                            "mouse-current-map-turbo-help",
                            t("MAPPING_TURBO_CONTROL_TIP"),
                            210.,
                        )
                        .hovered_priority(9999)
                        .trigger(|hovered, window, cx| {
                            BaseButton::new("mouse-current-map-turbo-help-icon")
                                .size(surface::css(14.))
                                .p_0()
                                .rounded_full()
                                .bg(surface::fade_color(
                                    "mouse-current-map-turbo-help-color",
                                    if hovered {
                                        MouseMappingColors::help_hover()
                                    } else {
                                        MouseMappingColors::help()
                                    },
                                    300,
                                    window,
                                    cx,
                                ))
                                .child(
                                    img("synapse/automation-tooltip_questionmark.svg").size_full(),
                                )
                                .into_any_element()
                        }),
                    ),
            );
            body = body.child(
                div()
                    .mt(surface::css(10.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(MouseCustomizeColors::text())
                    .opacity(surface::fade_opacity(
                        ("mouse-current-turbo-label", cx.entity_id()),
                        if state.turbo { 1. } else { 0.3 },
                        200,
                        window,
                        cx,
                    ))
                    .child(t("SPEED_PER_SEC")),
            );
            if state.turbo {
                let stepper = state.stepper.as_ref()?;
                let slider = state.slider.as_ref()?;
                body = body
                    .child(stepper.clone())
                    .child(SourceSlider::new(
                        slider,
                        (slider.read(cx).value().start() - 1.) / 19.,
                    ))
                    .child(
                        h_flex()
                            .justify_between()
                            .text_size(surface::css(12.))
                            .child("1")
                            .child("20"),
                    );
            }
        }
        if let Some(prerequisite) = self.current_mouse_prerequisite(button, state.reset) {
            body = body.child(prerequisite);
        }
        Some(
            body.child(
                h_flex()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .child(
                        Button::new("mouse-mapping-cancel")
                            .label(t("CANCEL"))
                            .outline()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_current_customize_input(None, false);
                                this.mapping_current_state.clear();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("mouse-mapping-save")
                            .label(t("SAVE"))
                            .primary()
                            .disabled(!state.can_save)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.save_current_mouse_mapping(cx)),
                            ),
                    ),
            )
            .into_any_element(),
        )
    }
}
