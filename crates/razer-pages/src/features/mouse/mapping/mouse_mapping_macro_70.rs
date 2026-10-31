//! PID70 MapMacro.3627a5cd factory2508 and current main9267 playback sets.
use super::super::Choice;
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::component::{
    IndexPath,
    select::{SelectEvent, SelectState},
};
use razer_widgets::stepper::{Stepper, StepperEvent};

const STANDARD: &[(&str, &str)] = &[
    ("Once", "PLAY_ONCE"),
    ("NTimes", "PLAY_MULTIPLE_TIMES"),
    ("ContinuousToggle", "TOGGLE_CONTINUOUS_PLAYBACK"),
    ("ContinuousHeld", "PLAY_WHILE_KEY_PRESSED"),
    ("Queue", "MACRO_QUEUE"),
];
const SCROLL: &[(&str, &str)] = &[
    ("Once", "PLAY_ONCE"),
    ("NTimes", "PLAY_MULTIPLE_TIMES"),
    ("Queue", "MACRO_QUEUE"),
];
const SEQUENCE: &[(&str, &str)] = &[("Sequence", "SEQUENCE_MACRO")];
const PHASED: &[(&str, &str)] = &[("Phased", "PHASED_MACRO")];

pub struct MouseMappingMacroRequested;
impl EventEmitter<MouseMappingMacroRequested> for MouseProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    catalog: Vec<Value>,
    selected: usize,
    playback: usize,
    options: &'static [(&'static str, &'static str)],
    repeat: Value,
    selector: Option<Entity<SelectState<Vec<Choice>>>>,
    playback_selector: Option<Entity<SelectState<Vec<Choice>>>>,
    stepper: Option<Entity<Stepper>>,
}

fn options(entry: Option<&Value>, input: &str) -> &'static [(&'static str, &'static str)] {
    match entry.and_then(|entry| entry["macroPlaybackOption"].as_str()) {
        Some("sequence") => SEQUENCE,
        Some("phased") => PHASED,
        _ if matches!(input, "ScrollUp" | "ScrollDown") => SCROLL,
        _ => STANDARD,
    }
}

fn sort_catalog(catalog: &mut [Value]) {
    // Current3466.eh mutates the macroSet using this exact two-code-unit
    // alphabet ranking followed by JavaScript's UTF-16 string comparison.
    catalog.sort_by(|left, right| {
        let left = left["name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .encode_utf16()
            .collect::<Vec<_>>();
        let right = right["name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .encode_utf16()
            .collect::<Vec<_>>();
        let rank = |value: Option<&u16>| {
            value.and_then(|value| {
                "*!@_.()#^&%-=+01234567989abcdefghijklmnopqrstuvwxyz"
                    .encode_utf16()
                    .position(|unit| unit == *value)
            })
        };
        let a = rank(left.first());
        let b = rank(right.first());
        if a == b {
            let aa = rank(left.get(1));
            let bb = rank(right.get(1));
            if aa == bb || aa.is_none() || bb.is_none() {
                left.cmp(&right)
            } else {
                bb.cmp(&aa)
            }
        } else if a.is_none() || b.is_none() {
            left.cmp(&right)
        } else {
            a.cmp(&b)
        }
    });
}

impl MouseProductWorkspace {
    pub fn observe_local_mapping_macros(
        &mut self,
        mut catalog: Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        sort_catalog(&mut catalog);
        if self.mouse_customize_observation["macroReducer"]["macroList"] == json!(catalog) {
            return;
        }
        self.mapping_macro_requirements = Some(
            razer_model::mapping_requirements::MacroRequirement::from_source_catalog(&catalog)
                .map_err(|error| format!("{error:#}")),
        );
        self.mouse_customize_observation["macroReducer"] =
            json!({"macroAvailable":!catalog.is_empty(),"macroList":catalog});
        if self.spec.product_id == 70 && self.mapping_current_state.function == "MACRO" {
            let selected = self.mapping_current_state.macros.selected;
            let catalog = self.mouse_customize_observation["macroReducer"]["macroList"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            self.mapping_current_state.macros.catalog = catalog;
            self.mapping_current_state.macros.selected =
                if selected < self.mapping_current_state.macros.catalog.len() {
                    selected
                } else {
                    0
                };
            self.refresh_mouse_macro_70_choices(window, cx);
            self.mapping_current_state.can_save =
                !self.mapping_current_state.macros.catalog.is_empty();
            self.mapping_current_state.changed = false;
        }
        cx.notify();
    }

    pub(super) fn initialize_mouse_macro_70(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let catalog = self.mouse_customize_observation["macroReducer"]["macroList"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let group = &button["assignmentGroup"];
        let mapped = button["assignment"] == "MACRO";
        let selected = if mapped {
            catalog
                .iter()
                .position(|entry| entry["guid"] == group["guid"])
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        let options = options(
            catalog.get(selected),
            button["inputID"].as_str().unwrap_or_default(),
        );
        let playback = if mapped {
            options
                .iter()
                .position(|(id, _)| {
                    group["macroPlaybackOption"]
                        .as_str()
                        .is_some_and(|option| id.eq_ignore_ascii_case(option))
                })
                .unwrap_or(0)
        } else {
            0
        };
        let repeat = if mapped {
            group["repeatCount"].clone()
        } else {
            json!(2)
        };
        let value = repeat
            .as_f64()
            .or_else(|| repeat.as_str().and_then(|value| value.parse().ok()))
            .unwrap_or(2.);
        let stepper = cx.new(|cx| {
            Stepper::new(
                "mouse70-macro-repeat",
                value,
                (1., 99., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_custom_keymapping()
            .live_update()
        });
        let subscription = cx.subscribe(&stepper, |this, _, event: &StepperEvent, cx| {
            this.mapping_current_state.macros.repeat = event
                .draft
                .clone()
                .map_or_else(|| json!(event.value), Value::String);
            if !this.mapping_current_state.macros.catalog.is_empty() {
                this.mapping_current_state.can_save = true;
                this.mapping_current_state.changed = true;
            }
            cx.notify();
        });
        let can_save = catalog
            .get(selected)
            .and_then(|entry| entry["name"].as_str())
            .is_some_and(|name| {
                !name.trim().is_empty()
                    && (button["isMapped"] != true || button["assignmentValue"] != name)
            });
        self.mapping_current_state.macros = State {
            catalog,
            selected,
            playback,
            options,
            repeat,
            stepper: Some(stepper),
            ..Default::default()
        };
        self.mapping_current_state.can_save = can_save;
        self.mapping_current_state.changed = false;
        // Current70 DeviceInfo has no OBMSpecs macroGroup capability;3466.Xq
        // returns true before inspecting any selected macro's event contents.
        self.mapping_current_state.basic_requires_synapse = true;
        self.mapping_current_state.subscriptions.push(subscription);
        self.refresh_mouse_macro_70_choices(window, cx);
        self.mapping_190_state.missing_observation = None;
    }

    fn refresh_mouse_macro_70_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.mapping_current_state.macros;
        let choices = state
            .catalog
            .iter()
            .filter_map(|entry| {
                Some(Choice::new(
                    entry["guid"].as_str()?,
                    entry["name"].as_str()?,
                ))
            })
            .collect::<Vec<_>>();
        state.options = options(
            state.catalog.get(state.selected),
            self.mapping_input.as_deref().unwrap_or_default(),
        );
        if state.playback >= state.options.len() {
            state.playback = 0;
        }
        let playback_choices = state
            .options
            .iter()
            .map(|(id, label)| Choice::localized(*id, *label))
            .collect::<Vec<_>>();
        if let Some(selector) = &state.selector {
            selector.update(cx, |selector, cx| {
                selector.set_items(choices, window, cx);
                selector.set_selected_index(
                    (state.selected < state.catalog.len())
                        .then_some(IndexPath::new(state.selected)),
                    window,
                    cx,
                );
            });
        } else {
            let selector = cx.new(|cx| {
                SelectState::new(
                    choices,
                    (state.selected < state.catalog.len())
                        .then_some(IndexPath::new(state.selected)),
                    window,
                    cx,
                )
            });
            let subscription = cx.subscribe_in(&selector, window, |this, _, event, window, cx| {
                let SelectEvent::Confirm(Some(guid)) = event else {
                    return;
                };
                let state = &mut this.mapping_current_state.macros;
                if let Some(index) = state
                    .catalog
                    .iter()
                    .position(|entry| entry["guid"] == guid.as_str())
                {
                    if index == state.selected {
                        return;
                    }
                    state.selected = index;
                    if matches!(
                        state.catalog[index]["macroPlaybackOption"].as_str(),
                        Some("sequence" | "phased")
                    ) {
                        state.playback = 0;
                    }
                    this.refresh_mouse_macro_70_choices(window, cx);
                    this.mapping_current_state.can_save = true;
                    // componentDidUpdate's selectedMacro branch calls
                    // enableSave(macroAvailable,false,...).
                    this.mapping_current_state.changed = false;
                    cx.notify();
                }
            });
            state.selector = Some(selector);
            self.mapping_current_state.subscriptions.push(subscription);
        }
        let state = &mut self.mapping_current_state.macros;
        if let Some(selector) = &state.playback_selector {
            selector.update(cx, |selector, cx| {
                selector.set_items(playback_choices, window, cx);
                selector.set_selected_index(Some(IndexPath::new(state.playback)), window, cx);
            });
        } else {
            let selector = cx.new(|cx| {
                SelectState::new(
                    playback_choices,
                    Some(IndexPath::new(state.playback)),
                    window,
                    cx,
                )
            });
            let subscription = cx.subscribe_in(&selector, window, |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let state = &mut this.mapping_current_state.macros;
                if let Some(index) = state
                    .options
                    .iter()
                    .position(|(option, _)| *option == id.as_str())
                {
                    state.playback = index;
                    this.mapping_current_state.can_save = true;
                    this.mapping_current_state.changed = true;
                    cx.notify();
                }
            });
            state.playback_selector = Some(selector);
            self.mapping_current_state.subscriptions.push(subscription);
        }
    }

    pub(super) fn mouse_macro_70_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let state = &self.mapping_current_state.macros;
        let entry = state.catalog.get(state.selected)?;
        let option = state.options.get(state.playback)?.0;
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,
            "outputType":"macroGroup","macroGroup":{"name":entry["name"],"guid":entry["guid"],
            "macroPlaybackOption":option,"repeatCount":if option=="NTimes" {state.repeat.clone()}else{json!(2)}}}),
        ])
    }

    pub(super) fn mouse_macro_70_body(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.mapping_current_state.macros;
        let available = !state.catalog.is_empty();
        let mut body = v_flex();
        if let Some(selector) = &state.selector {
            body = body.child(
                razer_widgets::synapse_select::select(selector)
                    .placeholder(" ")
                    .items(
                        state
                            .catalog
                            .iter()
                            .filter_map(|entry| {
                                Some(Choice::new(
                                    entry["guid"].as_str()?,
                                    entry["name"].as_str()?,
                                ))
                            })
                            .collect(),
                    )
                    .mb(surface::css(10.))
                    .w_full(),
            );
        }
        body = body.child(
            div()
                .mb(surface::css(5.))
                .opacity(if available { 1. } else { 0.3 })
                .child(t("PLAYBACK_OPTION")),
        );
        if let Some(selector) = &state.playback_selector {
            body = body.child(
                razer_widgets::synapse_select::select(selector)
                    .items(
                        state
                            .options
                            .iter()
                            .map(|(id, label)| Choice::localized(*id, *label))
                            .collect(),
                    )
                    .disabled(!available)
                    .mb(surface::css(10.))
                    .w_full(),
            );
        }
        if state.playback == 1 {
            body = body
                .child(div().mb(surface::css(5.)).child(t("NUMBER_OF_TIMES")))
                .children(state.stepper.clone());
        }
        if !available {
            body = body.child(div().mb(surface::css(20.)).child(t("CREATE_MACRO_MSG")));
        }
        body.child(
            BaseButton::new("mouse70-configure-macros")
                .underline()
                .mb(surface::css(20.))
                .child(t("CONFIGURE_MACROS"))
                .on_click(cx.listener(|_, _, _, cx| cx.emit(MouseMappingMacroRequested))),
        )
        .into_any_element()
    }
}
