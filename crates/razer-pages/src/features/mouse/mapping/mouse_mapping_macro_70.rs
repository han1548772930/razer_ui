//! Current MapMacro editor shared by all 75 mouse deployments. The actual
//! DeviceInfo inputs select playback restrictions and firmware requirements.
use super::super::Choice;
use super::*;
use gpui_kit::component::{
    IndexPath,
    select::{SelectEvent, SelectItem, SelectState},
};
use razer_widgets::keyboard_navigation::Button as BaseButton;
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
const WITHOUT_HELD: &[(&str, &str)] = &[
    ("Once", "PLAY_ONCE"),
    ("NTimes", "PLAY_MULTIPLE_TIMES"),
    ("ContinuousToggle", "TOGGLE_CONTINUOUS_PLAYBACK"),
    ("Queue", "MACRO_QUEUE"),
];
const SEQUENCE: &[(&str, &str)] = &[("Sequence", "SEQUENCE_MACRO")];
const PHASED: &[(&str, &str)] = &[("Phased", "PHASED_MACRO")];

pub struct MouseMappingMacroRequested;
impl EventEmitter<MouseMappingMacroRequested> for MouseProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    catalog: Vec<MacroEntry>,
    selected: usize,
    playback: usize,
    options: &'static [(&'static str, &'static str)],
    repeat: Value,
    selector: Option<Entity<SelectState<Vec<MacroChoice>>>>,
    playback_selector: Option<Entity<SelectState<Vec<Choice>>>>,
    stepper: Option<Entity<Stepper>>,
}

#[derive(Clone, Debug, Deserialize)]
struct MacroEntry {
    guid: String,
    name: String,
    #[serde(rename = "macroPlaybackOption")]
    mode: Option<String>,
}

#[derive(Clone)]
struct MacroChoice {
    guid: String,
    name: SharedString,
    disabled: bool,
}

impl SelectItem for MacroChoice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.name.clone()
    }
    fn value(&self) -> &String {
        &self.guid
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

fn is_scroll(input: &str) -> bool {
    matches!(input, "ScrollUp" | "ScrollDown")
}

fn options(
    entry: Option<&MacroEntry>,
    input: &str,
    product: u32,
    mount: bool,
) -> &'static [(&'static str, &'static str)] {
    match entry.and_then(|entry| entry.mode.as_deref()) {
        Some("sequence") => SEQUENCE,
        Some("phased") => PHASED,
        _ if is_scroll(input)
            || mount
                && matches!(product, 199 | 208)
                && matches!(input, "DKM_SB_210" | "DKM_AI_PROMPT" | "DKM_SB_03") =>
        {
            SCROLL
        }
        _ if input == "DKM_SB_03"
            && matches!(
                product,
                218 | 222 | 196 | 192 | 190 | 229 | 182 | 208 | 199 | 158
            ) =>
        {
            WITHOUT_HELD
        }
        _ => STANDARD,
    }
}

fn eligible(entry: Option<&MacroEntry>, button: &Value) -> bool {
    let Some(entry) = entry.filter(|entry| !entry.name.trim().is_empty()) else {
        return false;
    };
    if !entry.guid.is_empty() && button["assignmentGroup"]["guid"] == entry.guid {
        return false;
    }
    let previous = button["assignmentGroup"]["name"]
        .as_str()
        .filter(|name| !name.is_empty())
        .or_else(|| button["assignmentValue"].as_str())
        .unwrap_or("");
    if previous.is_empty() {
        button["isMapped"] != true
    } else {
        entry.name.trim() != previous.trim()
    }
}

impl State {
    fn choices(&self, input: &str) -> Vec<MacroChoice> {
        if self.catalog.is_empty() {
            return vec![MacroChoice {
                guid: String::new(),
                name: " ".into(),
                disabled: false,
            }];
        }
        self.catalog
            .iter()
            .map(|entry| MacroChoice {
                guid: entry.guid.clone(),
                name: entry.name.clone().into(),
                disabled: is_scroll(input) && entry.mode.as_deref() == Some("phased"),
            })
            .collect()
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
        if self.mapping_current_state.function == "MACRO" {
            let selected = self.mapping_current_state.macros.selected;
            let Ok(catalog) = serde_json::from_value::<Vec<MacroEntry>>(
                self.mouse_customize_observation["macroReducer"]["macroList"].clone(),
            ) else {
                self.mapping_current_state.can_save = false;
                self.mapping_190_state.missing_observation = Some("macroReducer.macroList");
                cx.notify();
                return;
            };
            let next_selected = if selected < catalog.len() { selected } else { 0 };
            let next_entry = catalog.get(next_selected);
            let previous_mode = next_entry.and_then(|entry| {
                self.mapping_current_state
                    .macros
                    .catalog
                    .iter()
                    .find(|previous| previous.guid == entry.guid)
                    .and_then(|previous| previous.mode.as_deref())
            });
            let mode_changed = previous_mode != next_entry.and_then(|entry| entry.mode.as_deref());
            if selected != next_selected || mode_changed {
                self.mapping_current_state.macros.options = options(
                    next_entry,
                    self.mapping_input.as_deref().unwrap_or_default(),
                    self.mapping_source_product_id(),
                    false,
                );
                if matches!(
                    next_entry.and_then(|entry| entry.mode.as_deref()),
                    Some("sequence" | "phased")
                ) {
                    self.mapping_current_state.macros.playback = 0;
                }
            }
            self.mapping_current_state.macros.catalog = catalog;
            self.mapping_current_state.macros.selected = next_selected;
            self.mapping_190_state.missing_observation = None;
            self.refresh_mouse_macro_70_choices(window, cx);
            let button = self
                .mapping_input
                .as_deref()
                .and_then(|input| self.current_mapping_source_button(input));
            self.mapping_current_state.can_save = button.as_ref().is_some_and(|button| {
                eligible(
                    self.mapping_current_state
                        .macros
                        .catalog
                        .get(self.mapping_current_state.macros.selected),
                    button,
                ) && (!is_scroll(button["inputID"].as_str().unwrap_or_default())
                    || self
                        .mapping_current_state
                        .macros
                        .catalog
                        .get(next_selected)
                        .is_some_and(|entry| entry.mode.as_deref() != Some("phased")))
            });
            self.update_mouse_macro_requirement(false);
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
        let Ok(catalog) = serde_json::from_value::<Vec<MacroEntry>>(
            self.mouse_customize_observation["macroReducer"]["macroList"].clone(),
        ) else {
            self.mapping_current_state.can_save = false;
            self.mapping_190_state.missing_observation = Some("macroReducer.macroList");
            return;
        };
        let group = &button["assignmentGroup"];
        let mapped = button["assignment"] == "MACRO";
        let mut selected = if mapped {
            catalog
                .iter()
                .position(|entry| group["guid"] == entry.guid)
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        if is_scroll(button["inputID"].as_str().unwrap_or_default()) {
            if let Some(index) = catalog
                .iter()
                .position(|entry| entry.mode.as_deref() != Some("phased"))
            {
                selected = index;
            }
        }
        let options = options(
            catalog.get(selected),
            button["inputID"].as_str().unwrap_or_default(),
            self.mapping_source_product_id(),
            true,
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
                this.update_mouse_macro_requirement(false);
            }
            cx.notify();
        });
        let can_save = eligible(catalog.get(selected), button)
            && (!is_scroll(button["inputID"].as_str().unwrap_or_default())
                || catalog
                    .iter()
                    .any(|entry| entry.mode.as_deref() != Some("phased")));
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
        self.update_mouse_macro_requirement(true);
        self.mapping_current_state.subscriptions.push(subscription);
        self.refresh_mouse_macro_70_choices(window, cx);
        self.mapping_190_state.missing_observation = None;
    }

    fn refresh_mouse_macro_70_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.mapping_current_state.macros;
        let choices = state.choices(self.mapping_input.as_deref().unwrap_or_default());
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
                    (state.catalog.is_empty() || state.selected < state.catalog.len())
                        .then_some(IndexPath::new(state.selected)),
                    window,
                    cx,
                );
            });
        } else {
            let selector = cx.new(|cx| {
                SelectState::new(
                    choices,
                    (state.catalog.is_empty() || state.selected < state.catalog.len())
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
                    .position(|entry| entry.guid == guid.as_str())
                {
                    if index == state.selected
                        || is_scroll(this.mapping_input.as_deref().unwrap_or_default())
                            && state.catalog[index].mode.as_deref() == Some("phased")
                    {
                        return;
                    }
                    state.selected = index;
                    if matches!(
                        state.catalog[index].mode.as_deref(),
                        Some("sequence" | "phased")
                    ) {
                        state.playback = 0;
                    }
                    state.options = options(
                        state.catalog.get(index),
                        this.mapping_input.as_deref().unwrap_or_default(),
                        this.spec.product_id,
                        false,
                    );
                    this.refresh_mouse_macro_70_choices(window, cx);
                    let button = this
                        .mapping_input
                        .as_deref()
                        .and_then(|input| this.current_mapping_source_button(input));
                    this.mapping_current_state.can_save = button.as_ref().is_some_and(|button| {
                        eligible(this.mapping_current_state.macros.catalog.get(index), button)
                    });
                    this.update_mouse_macro_requirement(false);
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
                    this.update_mouse_macro_requirement(true);
                    cx.notify();
                }
            });
            state.playback_selector = Some(selector);
            self.mapping_current_state.subscriptions.push(subscription);
        }
    }

    fn update_mouse_macro_requirement(&mut self, include_active: bool) {
        let state = &self.mapping_current_state.macros;
        let guid = state
            .catalog
            .get(state.selected)
            .map(|entry| entry.guid.as_str());
        let macros = self
            .mapping_macro_requirements
            .as_ref()
            .and_then(|value| value.as_ref().ok());
        let required = razer_model::mapping_requirements::current_mouse_mapping_capabilities(
            self.spec.product_id,
        )
        .zip(macros)
        .is_none_or(|(capabilities, macros)| capabilities.macro_requires_synapse(guid, macros));
        let active = include_active
            && self
                .mapping_input
                .as_deref()
                .and_then(|input| self.current_mapping_source_button(input))
                .is_some_and(|button| button["isRequireSynapse"] == true);
        self.mapping_current_state.basic_requires_synapse = required
            || active
            || state
                .options
                .get(state.playback)
                .is_some_and(|(id, _)| *id == "Queue");
    }

    pub(super) fn mouse_macro_70_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let state = &self.mapping_current_state.macros;
        let entry = state.catalog.get(state.selected)?;
        let option = state.options.get(state.playback)?.0;
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,
            "outputType":"macroGroup","macroGroup":{"name":entry.name,"guid":entry.guid,
            "macroPlaybackOption":option,"repeatCount":if option=="NTimes" {state.repeat.clone()}else{json!(2)}}}),
        ])
    }

    pub(super) fn mouse_macro_70_body(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.mapping_current_state.macros;
        let available = !state.catalog.is_empty();
        let mut body = v_flex();
        if let Some(selector) = &state.selector {
            let choices = state.choices(self.mapping_input.as_deref().unwrap_or_default());
            let tips = choices
                .iter()
                .filter(|choice| choice.disabled)
                .map(|choice| {
                    (
                        choice.guid.clone(),
                        razer_widgets::synapse_select::SelectOptionDecoration::new()
                            .tooltip(t("DISABLED_PHASED_MACRO")),
                    )
                })
                .collect();
            let only_phased = choices.len() == 1 && choices[0].disabled;
            body = body.child(
                razer_widgets::synapse_select::select_pointer(selector)
                    .placeholder(" ")
                    .id("mouse-macro-choice")
                    .items(choices)
                    .pointer_option_decorations(tips)
                    .when(only_phased, |select| {
                        select.disabled_tooltip(t("DISABLED_PHASED_MACRO"))
                    })
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
                razer_widgets::synapse_select::select_pointer(selector)
                    .id("mouse-macro-playback")
                    .selected_tooltip(true)
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

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;

    const PRODUCTS: &[u32] = &[
        70, 80, 83, 89, 92, 96, 98, 99, 100, 101, 103, 104, 105, 106, 107, 108, 110, 112, 113, 115,
        116, 117, 120, 122, 124, 128, 131, 132, 133, 134, 138, 140, 141, 143, 145, 147, 148, 150,
        152, 153, 154, 156, 158, 161, 163, 165, 167, 170, 175, 178, 180, 182, 184, 185, 190, 192,
        194, 196, 199, 203, 204, 208, 211, 214, 218, 221, 222, 224, 226, 229, 231, 235, 239, 2594,
        45066,
    ];
    const FIRMWARE_MACROS: &[u32] = &[
        89, 96, 100, 101, 103, 104, 105, 108, 112, 115, 117, 122, 124, 128, 132, 133, 134, 141,
        143, 145, 147, 150, 153, 167, 170, 175, 178, 203, 204, 211, 214, 218, 226, 231, 235, 2594,
        45066,
    ];

    fn catalog() -> Vec<Value> {
        vec![
            json!({"guid":"macro-a","name":"Macro A","macroPlaybackOption":"standard",
            "appEngine":{},"macroEvents":[]}),
        ]
    }

    #[gpui_kit::test]
    fn mouse_macro_current_all_products_mount_save_and_use_source_requirement(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        for &pid in PRODUCTS {
            let mut view = None;
            let mut input = String::new();
            let handle = cx.open_window(size(px(1400.), px(950.)), |window, cx| {
                let owner = cx.new(|cx| MouseProductWorkspace::new(pid, window, cx));
                owner.update(cx, |owner, cx| {
                    owner.observe_local_mapping_macros(catalog(), window, cx);
                    input = owner
                        .customize_source_buttons()
                        .iter()
                        .find(|button| button["defaultValue"] == "RIGHT_CLICK")
                        .and_then(|button| button["inputID"].as_str())
                        .unwrap()
                        .to_owned();
                    if pid == 190 {
                        owner.mapping_190_request(
                            super::super::customize_190::source_mapping::Next::Open(input.clone()),
                            window,
                            cx,
                        );
                    } else {
                        owner.open_current_mouse_mapping(input.clone(), window, cx);
                    }
                    assert!(owner.mapping_input.is_some(), "PID{pid} input {input}");
                });
                view = Some(owner.clone());
                Root::new(owner, window, cx)
            });
            let view = view.unwrap();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let category = if pid == 190 { "mouse-190-function-MACRO" } else { "mouse-current-menu-MACRO-0" };
                window.click(category, cx);
                assert_eq!(view.read(cx).mapping_current_state.function, "MACRO", "PID{pid} category selection");
                assert!(window.find("mouse-macro-choice").visible(), "PID{pid}");
                assert!(view.read(cx).mapping_current_state.can_save, "PID{pid}");
                assert_eq!(view.read(cx).mapping_current_state.basic_requires_synapse,
                    !FIRMWARE_MACROS.contains(&pid), "PID{pid}");
                window.click(if pid == 190 { "mouse-190-mapping-save" } else { "mouse-mapping-save" }, cx);
                let saved = view.read(cx).snapshot();
                let rows = saved["mappings"].as_array().unwrap_or_else(|| panic!("PID{pid} missing mappings"));
                let row = rows.iter().find(|row| row["inputID"] == input && row["isHyperShift"] == false)
                    .unwrap_or_else(|| panic!("PID{pid} missing {input}; editor {:?}, mappings {rows:?}",view.read(cx).mapping_input));
                assert_eq!(row["macroGroup"], json!({"guid":"macro-a","name":"Macro A","macroPlaybackOption":"Once","repeatCount":2}), "PID{pid}");
                assert!(view.read(cx).mapping_input.is_none(), "PID{pid}");
                window.remove_window();
            }).unwrap();
            cx.run_until_parked();
        }
    }

    #[::core::prelude::v1::test]
    fn mouse_macro_current_source_guard_and_product_playback_branches() {
        let entry = MacroEntry {
            guid: "a".into(),
            name: " Macro ".into(),
            mode: Some("standard".into()),
        };
        assert!(!eligible(
            Some(&entry),
            &json!({"assignmentGroup":{"guid":"a","name":"Different"}})
        ));
        assert!(!eligible(
            Some(&entry),
            &json!({"assignmentValue":"Macro","isMapped":true})
        ));
        assert!(!eligible(Some(&entry), &json!({"isMapped":true})));
        assert!(eligible(Some(&entry), &json!({"isMapped":false})));
        for &pid in PRODUCTS {
            assert_eq!(options(Some(&entry), "ScrollUp", pid, true), SCROLL);
            let cycle = options(Some(&entry), "DKM_SB_03", pid, true);
            if matches!(pid, 199 | 208) {
                assert_eq!(cycle, SCROLL);
            } else if [158, 182, 190, 192, 196, 218, 222, 229].contains(&pid) {
                assert_eq!(cycle, WITHOUT_HELD);
            } else {
                assert_eq!(cycle, STANDARD);
            }
        }
        assert_eq!(options(Some(&entry), "DKM_SB_210", 199, true), SCROLL);
        assert_eq!(options(Some(&entry), "DKM_SB_210", 199, false), STANDARD);
    }

    #[gpui_kit::test]
    fn mouse_macro_current_attachment_views_save_to_their_side_and_cancel_releases_editor(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        for pid in [80, 83, 103, 141, 143, 147, 150, 167, 180, 231] {
            let mut owner = None;
            let handle = cx.open_window(size(px(1400.), px(950.)), |window, cx| {
                let view = cx.new(|cx| MouseProductWorkspace::new(pid, window, cx));
                view.update(cx, |view, cx| {
                    view.observe_local_mapping_macros(catalog(), window, cx)
                });
                owner = Some(view.clone());
                Root::new(view, window, cx)
            });
            let owner = owner.unwrap();
            cx.update_window(handle.into(), |_,window,cx| {
                let main = owner.read(cx).snapshot()["mappings"].clone();
                for view_index in 1..owner.read(cx).spec.groups.len() {
                    window.render_frame(cx);
                    window.click(format!("mouse-source-view-{view_index}"),cx);
                    assert_eq!(owner.read(cx).group_ix,view_index,"PID{pid}");
                    let input = owner.read(cx).customize_source_buttons().iter()
                        .find_map(|row| row["inputID"].as_str().map(str::to_owned)).unwrap();
                    let before = owner.read(cx).snapshot();
                    window.click(format!("mouse-{pid}-input-{input}"),cx);
                    window.click("mouse-current-menu-MACRO-0",cx);
                    assert!(owner.read(cx).mapping_current_state.can_save,"PID{pid} view{view_index}");
                    window.click("mouse-mapping-cancel",cx);
                    assert!(owner.read(cx).mapping_input.is_none());
                    assert!(owner.read(cx).mapping_current_state.subscriptions.is_empty());
                    assert_eq!(owner.read(cx).snapshot(),before);
                    window.click(format!("mouse-{pid}-input-{input}"),cx);
                    window.click("mouse-current-menu-MACRO-0",cx);
                    window.click("mouse-mapping-save",cx);
                    let saved=owner.read(cx).snapshot();
                    let side=super::super::button_state_current::side_panel_name(pid,view_index as i32).unwrap();
                    let rows=saved["sidePanelMappings"][side].as_array().unwrap();
                    let row=rows.iter().find(|row|row["inputID"]==input).unwrap_or_else(||panic!("PID{pid} view{view_index}"));
                    assert_eq!(row["macroGroup"],json!({"guid":"macro-a","name":"Macro A","macroPlaybackOption":"Once","repeatCount":2}));
                    assert_eq!(saved["mappings"],main);
                    assert!(owner.read(cx).mapping_input.is_none());
                }
                window.remove_window();
            }).unwrap();
            cx.run_until_parked();
        }
    }

    #[gpui_kit::test]
    fn mouse_macro_current_scroll_rejects_phased_and_catalog_changes_do_not_save(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut view = None;
        let handle = cx.open_window(size(px(1400.), px(950.)), |window, cx| {
            let owner = cx.new(|cx| MouseProductWorkspace::new(222, window, cx));
            owner.update(cx, |owner,cx| {
                let mut macros = catalog();
                macros.insert(0,json!({"guid":"phased","name":"A Phased","macroPlaybackOption":"phased","appEngine":{},"macroEvents":[]}));
                owner.observe_local_mapping_macros(macros,window,cx);
                owner.open_current_mouse_mapping("ScrollUp".into(),window,cx);
            });
            view = Some(owner.clone()); Root::new(owner,window,cx)
        });
        let view = view.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("mouse-current-menu-MACRO-0", cx);
            assert_eq!(
                view.read(cx).mapping_current_state.macros.selected,
                1,
                "input {:?}, entries {:?}, function {}",
                view.read(cx).mapping_input,
                view.read(cx).mapping_current_state.macros.catalog,
                view.read(cx).mapping_current_state.function
            );
            assert!(
                view.read(cx)
                    .mapping_current_state
                    .macros
                    .choices("ScrollUp")[0]
                    .disabled
            );
            assert_eq!(view.read(cx).mapping_current_state.macros.options, SCROLL);
            window.click("mouse-macro-choice", cx);
            window.hover((ElementId::from("source-pointer-option-tip-phased"), "tip-trigger"), cx);
            assert!(window.find((ElementId::from("source-pointer-option-tip-phased"), "tip-popup")).visible());
            assert_eq!(view.read(cx).mapping_current_state.macros.selected, 1);
            window.click("mouse-macro-choice", cx);
            let before = view.read(cx).snapshot();
            view.update(cx, |owner, cx| {
                owner.observe_local_mapping_macros(Vec::new(), window, cx)
            });
            window.render_frame(cx);
            assert!(!view.read(cx).mapping_current_state.can_save);
            window.click("mouse-mapping-save", cx);
            assert!(view.read(cx).mapping_input.is_some());
            assert_eq!(view.read(cx).snapshot(), before);
            for (mode, expected, can_save) in [
                ("sequence", SEQUENCE, true),
                ("standard", SCROLL, true),
                ("phased", PHASED, false),
                ("standard", SCROLL, true),
            ] {
                let mut macros = catalog();
                macros[0]["macroPlaybackOption"] = json!(mode);
                view.update(cx, |owner, cx| {
                    owner.observe_local_mapping_macros(macros, window, cx)
                });
                assert_eq!(view.read(cx).mapping_current_state.macros.options, expected);
                assert_eq!(view.read(cx).mapping_current_state.can_save, can_save);
                assert_eq!(view.read(cx).snapshot(), before);
                if mode == "phased" {
                    window.render_frame(cx);
                    window.hover((ElementId::from("source-pointer-disabled-tip"), "tip-trigger"), cx);
                    assert!(window.find((ElementId::from("source-pointer-disabled-tip"), "tip-popup")).visible());
                }
            }
            view.update(cx, |owner, cx| {
                owner.observe_local_mapping_macros(vec![json!({"guid":"invalid"})], window, cx)
            });
            assert!(!view.read(cx).mapping_current_state.can_save);
            view.update(cx, |owner, cx| {
                owner.observe_local_mapping_macros(catalog(), window, cx)
            });
            assert!(view.read(cx).mapping_current_state.can_save);
            assert!(
                view.read(cx)
                    .mapping_190_state
                    .missing_observation
                    .is_none()
            );
        })
        .unwrap();
    }
}
