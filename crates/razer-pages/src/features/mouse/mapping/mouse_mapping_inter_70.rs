//! Current PID70 MapInterDevice.b32db296 factories 8792/9515/489/421.
use super::super::Choice;
use super::mapping_sensitivity_slider_current::SensitivitySlider;
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::component::{
    IndexPath,
    radio::Radio,
    select::{SelectEvent, SelectState},
};
use razer_widgets::theme::MouseCustomizeColors as Colors;

const PROFILES: &[(&str, &str)] = &[
    ("NextProfile", "NEXT_PROFILE"),
    ("PreviousProfile", "PREVIOUS_PROFILE"),
    ("CycleUp", "CYCLE_UP_PROFILE"),
    ("CycleDown", "CYCLE_DOWN_PROFILE"),
    ("Specific", "SPECIFIC_PROFILE"),
];
const KEYMAPS: &[(&str, &str)] = &[
    ("nextKeymap", "NEXT_KEYMAP"),
    ("previousKeymap", "PREVIOUS_KEYMAP"),
    ("cycleUpKeymap", "CYCLE_UP_KEYMAP"),
    ("cycleDownKeymap", "CYCLE_DOWN_KEYMAP"),
];
const SENSITIVITY: &[(&str, &str)] = &[
    ("DPI_Clutch", "SENSITIVITY_CLUTCH"),
    ("DPI_Up", "SENSITIVITY_STAGE_UP"),
    ("DPI_Down", "SENSITIVITY_STAGE_DOWN"),
    ("DPI_OnTheFly", "ON_THE_FLY_SENSITIVITY"),
    ("DPI_CycleUp", "CYCLE_UP_SENSITIVITY"),
    ("DPI_CycleDown", "CYCLE_DOWN_SENSITIVITY"),
];

#[derive(Clone)]
pub struct MouseMappingTargetRequested {
    target: Value,
}
impl MouseMappingTargetRequested {
    pub fn target(&self) -> &Value {
        &self.target
    }
}
impl EventEmitter<MouseMappingTargetRequested> for MouseProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    rows: Vec<Value>,
    selected: usize,
    function: String,
    option: usize,
    profiles: Vec<Value>,
    profile_guid: String,
    chroma: Vec<Value>,
    chroma_index: usize,
    switch_lighting: bool,
    dpi: [i64; 2],
    independent: Option<bool>,
    selects: Vec<Entity<SelectState<Vec<Choice>>>>,
    choice_lists: Vec<Vec<Choice>>,
    inputs: Vec<Entity<InputState>>,
    sliders: Vec<Entity<SliderState>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}
fn choices(rows: &[Value], id: &str, label: &str) -> Vec<Choice> {
    rows.iter()
        .map(|row| {
            Choice::new(
                row[id]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| row[id].to_string()),
                row[label].as_str().unwrap_or_default(),
            )
        })
        .collect()
}
fn connected_rows(rows: &[Value], locale: &str) -> Vec<Value> {
    let locale = locale.to_ascii_lowercase();
    rows.iter()
        .filter(|row| {
            row["productId"] != 70
                && row["isSingleProfile"] != true
                && row["interDeviceMappingConfig"]["isSupported"] != false
                && row["category"] != "GAMEPAD"
                && !(row["isMouseConnected"] == true && row["productId"] == row["dongleId"])
        })
        .map(|row| {
            let mut row = row.clone();
            // Current MapInterDevice.b32db296/8792 loadDevices uses the
            // lower-case language key, then English for a falsy translation.
            row["name"] = json!(
                row["productName"][&locale]
                    .as_str()
                    .filter(|name| !name.is_empty())
                    .or_else(|| row["productName"]["en"].as_str())
                    .unwrap_or_default()
            );
            row["isNotConnected"] = json!(false);
            row
        })
        .collect()
}

#[cfg(test)]
mod locale_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn language_reload_reprojects_names_without_changing_observed_profiles() {
        let devices = vec![json!({
            "productId":182,
            "productName":{"en":"Razer Deathadder V3 Pro","zh-cn":"Razer 炼狱蝰蛇 V3专业版"},
            "name":"Previously rendered name",
            "profiles":[{"guid":"profile-1","name":"CANCEL"}],
            "activeProfile":"profile-1",
            "category":"MOUSE"
        })];
        let original = devices.clone();
        let english = connected_rows(&devices, "en");
        let chinese = connected_rows(&devices, "zh-CN");
        assert_eq!(english[0]["name"], "Razer Deathadder V3 Pro");
        assert_eq!(chinese[0]["name"], "Razer 炼狱蝰蛇 V3专业版");
        assert_eq!(chinese[0]["profiles"], original[0]["profiles"]);
        assert_eq!(chinese[0]["activeProfile"], "profile-1");
        assert_eq!(chinese[0]["isNotConnected"], false);
        assert_eq!(devices, original);
    }

    #[::core::prelude::v1::test]
    fn language_reload_keeps_source_target_filter_and_empty_translation_fallback() {
        let devices = vec![
            json!({"productId":70,"productName":{"en":"Source device"}}),
            json!({"productId":182,"productName":{"en":"English fallback","zh-cn":""},"category":"MOUSE"}),
            json!({"productId":653,"isSingleProfile":true}),
            json!({"productId":756,"interDeviceMappingConfig":{"isSupported":false}}),
            json!({"productId":2676,"category":"GAMEPAD"}),
            json!({"productId":207,"isMouseConnected":true,"dongleId":207}),
            json!({"productId":9999,"category":"KEYPAD"}),
        ];
        let rows = connected_rows(&devices, "zh-CN");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["productId"], 182);
        assert_eq!(rows[0]["name"], "English fallback");
        assert_eq!(rows[1]["productId"], 9999);
        assert_eq!(rows[1]["name"], "");
        assert_eq!(
            connected_rows(&devices, "de")[0]["name"],
            "English fallback"
        );
    }
}
fn functions(row: &Value) -> Vec<(&'static str, &'static str)> {
    let mut result = vec![("profileNavigationGroup", "SWITCH_PROFILE")];
    match row["category"].as_str() {
        Some("MOUSE" | "MOUSEPLUSMAT") => result.push(("sensitivityGroup", "SENSITIVITY")),
        Some("KEYPAD") => result.push(("keymapGroup", "SWITCH_KEYMAP")),
        _ => {}
    }
    result
}
impl State {
    fn row(&self) -> Option<&Value> {
        self.rows.get(self.selected)
    }
    fn stages(&self) -> Vec<Value> {
        let Some(row) = self.row() else {
            return Vec::new();
        };
        row["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|profile| profile["guid"] == row["activeProfile"])
            .and_then(|profile| profile["dpiStages"]["stages"].as_array())
            .cloned()
            .unwrap_or_default()
    }
    fn disabled(&self) -> bool {
        self.row().is_none_or(|row| row["isNotConnected"] == true)
            || if self.function == "sensitivityGroup" {
                self.stages().is_empty()
            } else {
                self.profiles.is_empty()
            }
    }
}
impl MouseProductWorkspace {
    pub fn observe_local_mapping_targets(
        &mut self,
        rows: Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_customize_observation["validDevices"] == json!(rows) {
            return;
        }
        self.mouse_customize_observation["validDevices"] = json!(rows);
        self.reload_mouse_inter_70(window, cx);
        cx.notify();
    }
    pub(super) fn reload_mouse_inter_70(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Current 8792 componentDidUpdate reloads from activeButton for both
        // lang and validDevices changes. This also restores the source save
        // flags and function selection; translating cached titles is not enough.
        if self.spec.product_id == 70 && self.mapping_current_state.function == "INTERDEVICE" {
            if let Some(button) = self
                .mapping_input
                .as_deref()
                .and_then(|input| self.current_mapping_source_button(input))
            {
                self.initialize_mouse_inter_70(&button, window, cx);
            }
        }
    }
    pub(super) fn initialize_mouse_inter_70(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut rows = connected_rows(
            self.mouse_customize_observation["validDevices"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            &razer_i18n::locale(),
        );
        let group = &button["assignmentGroup"];
        let mapped = button["assignment"] == "INTERDEVICE";
        let selected = if mapped {
            rows.iter().position(|row|row["productId"]==group["interPID"]).unwrap_or_else(||{
                rows.insert(0,json!({"productId":group["interPID"],"name":t("DEVICE_NOT_CONNECTED_MSG"),"isNotConnected":true,"isUnknownMappedDevice":true,"category":"unknown","profiles":[]}));0
            })
        } else {
            0
        };
        self.mapping_current_state.inter = State {
            rows,
            selected,
            function: if mapped {
                group["interDeviceType"]
                    .as_str()
                    .unwrap_or("profileNavigationGroup")
                    .into()
            } else {
                "profileNavigationGroup".into()
            },
            ..Default::default()
        };
        self.mount_mouse_inter_70(mapped.then_some(group), window, cx);
        self.mapping_current_state.can_save = !self.mapping_current_state.inter.rows.is_empty();
        self.mapping_current_state.changed = false;
        self.mapping_current_state.basic_requires_synapse = true;
    }
    fn mount_mouse_inter_70(
        &mut self,
        restore: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chroma = self.lighting_observation()["chromaProfiles"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let state = &mut self.mapping_current_state.inter;
        state.selects.clear();
        state.choice_lists.clear();
        state.inputs.clear();
        state.sliders.clear();
        state.subscriptions.clear();
        let row = state.row().cloned().unwrap_or(json!({}));
        state.profiles = row["profiles"].as_array().cloned().unwrap_or_default();
        state.profiles.sort_by_cached_key(|profile| {
            profile["name"]
                .as_str()
                .unwrap_or_default()
                .to_uppercase()
                .encode_utf16()
                .collect::<Vec<_>>()
        });
        state.chroma = chroma;
        let group = restore.map(|group| &group[&state.function]);
        let options = match state.function.as_str() {
            "sensitivityGroup" => SENSITIVITY,
            "keymapGroup" => KEYMAPS,
            _ => PROFILES,
        };
        let key = match state.function.as_str() {
            "sensitivityGroup" => "sensitivityAssignment",
            "keymapGroup" => "type",
            _ => "profileNavigationAssignment",
        };
        state.option = group
            .and_then(|group| options.iter().position(|(id, _)| group[key] == *id))
            .unwrap_or(0);
        state.profile_guid = group
            .and_then(|group| {
                state
                    .profiles
                    .iter()
                    .find(|profile| profile["guid"] == group["guid"])
            })
            .or_else(|| state.profiles.first())
            .and_then(|profile| profile["guid"].as_str())
            .unwrap_or_default()
            .into();
        state.chroma_index = group
            .and_then(|group| {
                state
                    .chroma
                    .iter()
                    .position(|profile| profile["id"] == group["lightpacId"])
            })
            .unwrap_or(0);
        state.switch_lighting = group.is_some_and(|group| {
            state
                .chroma
                .iter()
                .any(|profile| profile["id"] == group["lightpacId"])
        });
        state.dpi = [
            group.and_then(|g| g["x"].as_i64()).unwrap_or(800),
            group.and_then(|g| g["y"].as_i64()).unwrap_or(800),
        ];
        // Source constructor uses independent, but getter reads XYEnabled.
        state.independent = group.map(|g| g["independent"] == true);
        let fs = functions(&row);
        let all_options = [
            choices(&state.rows, "productId", "name"),
            fs.iter()
                .map(|(id, label)| Choice::localized(*id, *label))
                .collect(),
            options
                .iter()
                .filter(|(id, _)| !self.hypershift || *id != "DPI_OnTheFly")
                .map(|(id, label)| Choice::localized(*id, *label))
                .collect(),
            choices(&state.profiles, "guid", "name"),
            choices(&state.chroma, "id", "name"),
        ];
        let indexes = [
            Some(state.selected),
            fs.iter().position(|(id, _)| *id == state.function),
            Some(
                if self.hypershift && state.function == "sensitivityGroup" && state.option > 3 {
                    state.option - 1
                } else {
                    state.option
                },
            ),
            state
                .profiles
                .iter()
                .position(|p| p["guid"] == state.profile_guid),
            Some(state.chroma_index),
        ];
        for (slot, (items, index)) in all_options.into_iter().zip(indexes).enumerate() {
            state.choice_lists.push(items.clone());
            let select =
                cx.new(|cx| SelectState::new(items, index.map(IndexPath::new), window, cx));
            state.subscriptions.push(cx.subscribe_in(
                &select,
                window,
                move |this, _, event, window, cx| {
                    let SelectEvent::Confirm(Some(id)) = event else {
                        return;
                    };
                    let state = &mut this.mapping_current_state.inter;
                    match slot {
                        0 => {
                            if let Some(index) = state
                                .rows
                                .iter()
                                .position(|row| row["productId"].to_string() == id.as_str())
                            {
                                state.selected = index;
                                state.function = "profileNavigationGroup".into();
                                this.mount_mouse_inter_70(None, window, cx);
                            }
                        }
                        1 => {
                            state.function = id.to_string();
                            this.mount_mouse_inter_70(None, window, cx);
                        }
                        2 => {
                            state.option = SENSITIVITY
                                .iter()
                                .position(|(value, _)| *value == id.as_str())
                                .unwrap_or(0);
                        }
                        3 => {
                            state.profile_guid = id.to_string();
                        }
                        4 => {
                            state.chroma_index = state
                                .chroma
                                .iter()
                                .position(|row| row["id"] == id.as_str())
                                .unwrap_or(0);
                        }
                        _ => {}
                    }
                    this.mapping_current_state.changed = slot != 1
                        || this.mapping_current_state.inter.function == "sensitivityGroup";
                    this.mapping_current_state.can_save = true;
                    this.mapping_current_state.basic_requires_synapse =
                        this.mapping_current_state.inter.function != "sensitivityGroup"
                            || this.mapping_current_state.inter.option >= 3;
                    cx.notify();
                },
            ));
            state.selects.push(select);
        }
        let minimum = row["minDPI"].as_i64().unwrap_or(100);
        let maximum = row["maxDPI"].as_i64().unwrap_or(16000);
        let step = row["dpiStep"].as_i64().unwrap_or(100).max(1);
        for axis in 0..2 {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(state.dpi[axis].to_string())
                    .validate(|value, _| {
                        value.encode_utf16().count() <= 5
                            && (value.is_empty()
                                || super::mapping_sensitivity_current::parse_integer_prefix(value)
                                    .is_some())
                    })
            });
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(minimum as f32)
                    .max(maximum as f32)
                    .step(step as f32)
                    .default_value(state.dpi[axis] as f32)
            });
            state.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, input, event, window, cx| {
                    if this.mapping_current_state.inter.syncing {
                        return;
                    }
                    match event {
                        InputEvent::Change => {
                            let value = input.read(cx).value().to_string();
                            if let Some(value) =
                                super::mapping_sensitivity_current::parse_integer_prefix(&value)
                            {
                                this.mapping_current_state.inter.syncing = true;
                                input.update(cx, |input, cx| {
                                    input.set_value(value.to_string(), window, cx)
                                });
                                this.mapping_current_state.inter.syncing = false;
                            }
                        }
                        InputEvent::Blur => {
                            let text = input.read(cx).value().to_string();
                            let value = if text.is_empty() {
                                this.mapping_current_state.inter.dpi[axis]
                            } else {
                                let value =
                                    super::mapping_sensitivity_current::parse_integer_prefix(&text)
                                        .unwrap_or(1800);
                                (if value % step != 0 {
                                    value - value % step + step
                                } else {
                                    value
                                })
                                .clamp(minimum, maximum)
                            };
                            this.change_mouse_inter_dpi(axis, value, window, cx);
                        }
                        InputEvent::PressEnter { .. } => window.blur(cx),
                        _ => {}
                    }
                },
            ));
            state.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if this.mapping_current_state.inter.syncing {
                        return;
                    }
                    let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
                    this.change_mouse_inter_dpi(axis, value.start() as i64, window, cx);
                },
            ));
            state.inputs.push(input);
            state.sliders.push(slider);
        }
    }
    fn change_mouse_inter_dpi(
        &mut self,
        axis: usize,
        value: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.mapping_current_state.inter;
        let changed = state.dpi[axis] != value;
        state.dpi[axis] = value;
        if axis == 0 && state.independent != Some(true) {
            state.dpi[1] = value;
        }
        state.syncing = true;
        for axis in 0..2 {
            let value = state.dpi[axis];
            state.inputs[axis].update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
            state.sliders[axis].update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
        }
        state.syncing = false;
        if changed {
            self.mapping_current_state.changed = true;
            self.mapping_current_state.can_save = true;
            self.mapping_current_state.basic_requires_synapse = false;
        }
        cx.notify();
    }
    pub(super) fn mouse_inter_70_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let state = &self.mapping_current_state.inter;
        let row = state.row()?;
        if row["isUnknownMappedDevice"] == true {
            return None;
        }
        let mut value = match state.function.as_str() {
            "profileNavigationGroup" => {
                let mut group =
                    json!({"profileNavigationAssignment":PROFILES.get(state.option)?.0});
                if state.option == 4 {
                    let profile = state
                        .profiles
                        .iter()
                        .find(|p| p["guid"] == state.profile_guid)
                        .or_else(|| state.profiles.first())?;
                    group["guid"] = profile["guid"].clone();
                    group["name"] = profile["name"].clone();
                    if state.switch_lighting {
                        if let Some(chroma) = state.chroma.get(state.chroma_index) {
                            group["lightpacId"] = chroma["id"].clone();
                        }
                    }
                }
                group
            }
            "keymapGroup" => json!({"type":KEYMAPS.get(state.option)?.0}),
            "sensitivityGroup" => {
                let mut group = json!({"sensitivityAssignment":SENSITIVITY.get(state.option)?.0,"x":state.dpi[0],"y":state.dpi[1]});
                if let Some(independent) = state.independent {
                    group["independent"] = json!(independent);
                }
                group
            }
            _ => return None,
        };
        let mut group = json!({"interPID":row["productId"],"name":row["name"],"interDeviceType":state.function});
        group[&state.function] = value.take();
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,"outputType":"interDeviceGroup","interDeviceGroup":group}),
        ])
    }
    pub(super) fn mouse_inter_70_body(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let state = &self.mapping_current_state.inter;
        let Some(row) = state.row() else {
            return div().child(t("INTERDEVICE_WARNING")).into_any_element();
        };
        let select = |slot: usize, disabled: bool| {
            razer_widgets::synapse_select::select(&state.selects[slot])
                .items(state.choice_lists[slot].clone())
                .w_full()
                .disabled(disabled)
        };
        let mut body = v_flex()
            .mb(surface::css(10.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(select(0, false));
        if row["subDevices"]
            .as_array()
            .is_some_and(|rows| rows.len() > 1)
        {
            body = body.child(
                v_flex()
                    .w(surface::css(210.))
                    .my(surface::css(15.))
                    .p(surface::css(10.))
                    .border_1()
                    .border_color(rgb(0xffa500))
                    .rounded(surface::css(3.))
                    .items_center()
                    .justify_center()
                    .child(img("synapse/mouse70-mapping-warning.svg").size(surface::css(20.)))
                    .child(
                        div()
                            .text_center()
                            .child(t("MULTIPLE_SIMILAR_DEVICE_WARNING")),
                    ),
            );
        }
        if row["isNotConnected"] == true {
            body = body.child(
                div()
                    .text_color(
                        razer_widgets::theme::KeyboardInterDeviceColors::disconnected_text(),
                    )
                    .child(t("DEVICE_NOT_CONNECTED_MSG")),
            );
        }
        if row["isUnknownMappedDevice"] == true {
            return body.into_any_element();
        }
        let disabled = state.disabled();
        let mut child = v_flex()
            .opacity(if disabled { 0.3 } else { 1. })
            .child(select(1, row["isNotConnected"] == true));
        if state.function == "sensitivityGroup" {
            child = child.child(select(2, disabled));
            if state.option == 0 {
                let mut inputs = h_flex()
                    .items_center()
                    .mt(surface::css(10.))
                    .child(div().mr(surface::css(10.)).child("DPI"));
                for axis in 0..if state.independent == Some(true) { 2 } else { 1 } {
                    if state.independent == Some(true) {
                        if axis == 1 {
                            inputs = inputs.child(div().flex_1());
                        }
                        inputs = inputs.child(div().mr(surface::css(10.)).child(if axis == 0 {
                            "X"
                        } else {
                            "Y"
                        }));
                    }
                    let focus = state.inputs[axis].focus_handle(cx);
                    inputs = inputs.child(
                        div()
                            .w(surface::css(60.))
                            .h(surface::css(26.))
                            .mr(surface::css(5.))
                            .border_1()
                            .border_color(if focus.is_focused(window) {
                                Colors::selected()
                            } else {
                                Colors::line()
                            })
                            .track_focus(&focus)
                            .bg(Colors::selected_surface())
                            .text_color(Colors::text())
                            .text_size(surface::css(14.))
                            .line_height(surface::css(19.))
                            .hover(|style| style.border_color(Colors::selected()))
                            .child(
                                Input::new(&state.inputs[axis])
                                    .appearance(false)
                                    .w_full()
                                    .h_full()
                                    .text_center()
                                    .disabled(disabled),
                            )
                            .on_key_down(|event, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "escape" | "enter") {
                                    window.blur(cx);
                                    cx.stop_propagation();
                                }
                                if event.keystroke.key == "tab" {
                                    cx.stop_propagation();
                                }
                            }),
                    );
                }
                child = child.child(inputs);
                if row["supportXYDPI"] == true {
                    child = child.child(
                        Checkbox::new("mouse70-inter-xy")
                            .mt(surface::css(20.))
                            .label(t("ENABLE_XY"))
                            .checked(state.independent == Some(true))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.mapping_current_state.inter.independent = Some(*checked);
                                this.mapping_current_state.changed = true;
                                this.mapping_current_state.can_save = true;
                                this.mapping_current_state.basic_requires_synapse = false;
                                cx.notify();
                            })),
                    );
                }
                for axis in 0..if state.independent == Some(true) { 2 } else { 1 } {
                    child = child.child(
                        SensitivitySlider::new(
                            &state.sliders[axis],
                            row["minDPI"].as_u64().unwrap_or(100) as u32,
                            row["maxDPI"].as_u64().unwrap_or(16000) as u32,
                            if state.independent == Some(true) {
                                Some(if axis == 0 { "X" } else { "Y" })
                            } else {
                                None
                            },
                            state.independent != Some(true) || axis == 1,
                        )
                        .enabled(!disabled),
                    );
                }
            } else if SENSITIVITY
                .get(state.option)
                .is_some_and(|(id, _)| *id == "DPI_OnTheFly")
                && !self.hypershift
            {
                child = child.child(
                    div()
                        .mb(surface::css(20.))
                        .text_color(razer_widgets::theme::MouseSidebarColors::muted())
                        .child(t("ON_THE_FLY_SENSITIVITY_MSG")),
                );
            } else {
                for (index, stage) in state
                    .stages()
                    .iter()
                    .filter(|stage| stage["visible"] == true)
                    .enumerate()
                {
                    let dpi = if stage["x"] == stage["y"] {
                        stage["x"].to_string()
                    } else {
                        format!("X:{} Y:{}", stage["x"], stage["y"])
                    };
                    child =
                        child.child(div().child(format!("{} {} - {dpi}", t("STAGE"), index + 1)));
                }
                child = child.child(
                    BaseButton::new("mouse70-inter-configure")
                        .p_0()
                        .mt(surface::css(10.))
                        .mb(surface::css(20.))
                        .disabled(disabled)
                        .child(div().underline().child(t("CONFIGURE_SENSITIVITY_SETTINGS")))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(target) = this.mapping_current_state.inter.row() {
                                cx.emit(MouseMappingTargetRequested {
                                    target: target.clone(),
                                });
                            }
                        })),
                );
            }
        } else {
            let options = if state.function == "keymapGroup" {
                KEYMAPS
            } else {
                PROFILES
            };
            for (index, (id, label)) in options.iter().enumerate() {
                child = child.child(
                    Radio::new(SharedString::from(format!("mouse70-inter-{id}")))
                        .label(t(label))
                        .checked(state.option == index)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.mapping_current_state.inter.option = index;
                            this.mapping_current_state.changed = true;
                            this.mapping_current_state.can_save = true;
                            this.mapping_current_state.basic_requires_synapse = true;
                            cx.notify();
                        })),
                );
            }
            if state.function == "profileNavigationGroup" && state.option == 4 {
                let chroma_disabled = !state.switch_lighting
                    && (self.lighting_observation()["isBasicLightingDevice"] == true
                        || state.chroma.is_empty());
                child = child.child(
                    div()
                        .ml(surface::css(30.))
                        .w(surface::css(180.))
                        .mb(surface::css(20.))
                        .child(select(3, disabled))
                        .child(
                            Checkbox::new("mouse70-inter-lighting")
                                .w(surface::css(170.))
                                .mb(surface::css(10.))
                                .label(t("SWITCH_LIGHTING_PROFILE"))
                                .checked(state.switch_lighting)
                                .disabled(disabled || chroma_disabled)
                                .on_click(cx.listener(|this, value, _, cx| {
                                    this.mapping_current_state.inter.switch_lighting = *value;
                                    this.mapping_current_state.changed = true;
                                    this.mapping_current_state.can_save = true;
                                    cx.notify();
                                })),
                        )
                        .when(state.switch_lighting && !chroma_disabled, |body| {
                            body.child(select(4, disabled))
                        }),
                );
            }
        }
        body.child(child).into_any_element()
    }
}
