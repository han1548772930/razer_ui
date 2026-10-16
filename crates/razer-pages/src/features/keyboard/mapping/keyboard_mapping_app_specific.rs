//! Current MapAppSpecific: profile-name presets and custom key recording.
use super::*;
use razer_widgets::theme::KeyboardAppSpecificColors as Colors;

#[derive(Deserialize)]
pub(super) struct Data {
    pub product_id: u32,
    pub profiles: BTreeMap<String, Presets>,
    pub fallback_app_choices: Vec<Value>,
    pub profile_choices: Vec<Value>,
    pub sensitivity_choices: Vec<Value>,
    pub keymap_choices: Vec<Value>,
    pub messages: BTreeMap<String, String>,
    pub dpi_defaults: BTreeMap<String, f64>,
    pub configure_action: String,
}
#[derive(Deserialize)]
pub(super) struct Presets {
    pub choices: Vec<Value>,
}
pub(super) fn data(pid: u32) -> &'static Data {
    static DATA: OnceLock<Vec<Data>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_app_interdevice_data.json"))
            .expect("current app/inter-device metadata")
    })
    .iter()
    .find(|row| row.product_id == pid)
    .expect("current keyboard product")
}
#[derive(Default)]
pub(super) struct State {
    custom: [bool; 2],
    recorded: [Value; 2],
    cleared: [bool; 2],
    selected: [usize; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.custom = [false; 2];
        self.recorded = Default::default();
        self.cleared = [false; 2];
        self.selected = [0; 2];
    }
}
fn choices(workspace: &KeyboardProductWorkspace) -> &'static [Value] {
    let data = data(workspace.spec.product_id);
    data.profiles
        .get(workspace.draft["name"].as_str().unwrap_or_default())
        .map(|presets| presets.choices.as_slice())
        .unwrap_or(&data.fallback_app_choices)
}
pub(super) fn custom(workspace: &KeyboardProductWorkspace, slot: usize) -> bool {
    workspace.analog_gamepad.mapping.app_specific.custom[slot]
}
pub(super) fn recording_group(workspace: &KeyboardProductWorkspace, slot: usize) -> &Value {
    &workspace.analog_gamepad.mapping.app_specific.recorded[slot]
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    if workspace
        .analog_gamepad
        .mapping
        .app_specific
        .cleared
        .iter()
        .any(|cleared| *cleared)
    {
        return false;
    }
    if slot.staged["outputTypeSubset"] != "appSpecific" {
        return false;
    }
    let group = &slot.staged["keyboardGroup"];
    if choices(workspace)
        .iter()
        .any(|choice| choice["keyboardGroup"].is_object() && choice["keyboardGroup"] == *group)
    {
        return true;
    }
    group["key"].as_str().is_some_and(|key| {
        source(workspace.spec.product_id)
            .key_targets
            .iter()
            .any(|target| target["inputID"] == key)
    }) && group["modifiers"]
        .as_array()
        .is_some_and(|modifiers| modifiers.iter().all(|modifier| modifier.is_string()))
}
impl KeyboardProductWorkspace {
    pub(super) fn init_mapping_app_specific(&mut self, slot: usize, cx: &mut Context<Self>) {
        if !self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| edit.slots[slot].category == "APP_SPECIFIC")
        {
            self.analog_gamepad.mapping.app_specific.custom[slot] = false;
            self.analog_gamepad.mapping.app_specific.recorded[slot] = Value::Null;
            self.analog_gamepad.mapping.app_specific.cleared[slot] = false;
            self.analog_gamepad.mapping.app_specific.selected[slot] = 0;
            return;
        }
        self.init_mapping_keyboard_recording(slot, cx);
        self.analog_gamepad.mapping.app_specific.custom[slot] = false;
        self.analog_gamepad.mapping.app_specific.selected[slot] = 0;
        let selected = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .map(|edit| &edit.slots[slot]);
        if selected.is_some_and(|slot| {
            slot.category == "APP_SPECIFIC"
                && slot.staged["keyboardGroup"]["key"]
                    .as_str()
                    .is_none_or(str::is_empty)
        }) {
            let choice = choices(self).first().cloned();
            if let Some(choice) = choice {
                self.stage_mapping_app_preset(slot, &choice, cx);
            }
        }
    }
    pub(super) fn activate_mapping_app_custom(&mut self, slot: usize, cx: &mut Context<Self>) {
        let recorded = self.analog_gamepad.mapping.app_specific.recorded[slot].clone();
        let fallback = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .map(|edit| text(&edit.button["buttonKey"]))
            .unwrap_or_default();
        let fallback = source(self.spec.product_id)
            .key_targets
            .iter()
            .find(|target| target["inputID"] == fallback)
            .map(|target| target["inputID"].clone())
            .unwrap_or(Value::Null);
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let group = if recorded.is_object() {
                recorded
            } else {
                json!({"keyGroup":0,"key":fallback,"modifiers":[]})
            };
            two_tap::replace_slot(
                &mut edit.slots[slot],
                json!({"outputType":"keyboardGroup","outputTypeSubset":"appSpecific","keyboardGroup":group}),
            );
            edit.slots[slot].category = "APP_SPECIFIC".into();
        }
        self.analog_gamepad.mapping.app_specific.custom[slot] = true;
        self.analog_gamepad.mapping.app_specific.cleared[slot] = false;
        cx.notify();
    }
    pub(super) fn retain_mapping_app_recording(&mut self, slot: usize) {
        if let Some(edit) = &self.analog_gamepad.mapping.edit {
            if edit.slots[slot].category == "APP_SPECIFIC"
                && self.analog_gamepad.mapping.app_specific.custom[slot]
            {
                self.analog_gamepad.mapping.app_specific.recorded[slot] =
                    edit.slots[slot].staged["keyboardGroup"].clone();
                self.analog_gamepad.mapping.app_specific.cleared[slot] = false;
            }
        }
    }
    pub(super) fn clear_mapping_app_recording(&mut self, slot: usize, cx: &mut Context<Self>) {
        self.analog_gamepad.mapping.app_specific.recorded[slot] = Value::Null;
        self.analog_gamepad.mapping.app_specific.cleared[slot] = true;
        if self.analog_gamepad.mapping.app_specific.custom[slot] {
            if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
                edit.slots[slot].staged["keyboardGroup"] =
                    json!({"keyGroup":0,"key":"","modifiers":[]});
            }
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn stage_mapping_app_preset(&mut self, slot: usize, choice: &Value, cx: &mut Context<Self>) {
        if !choice["keyboardGroup"].is_object() {
            return;
        }
        self.analog_gamepad.mapping.app_specific.selected[slot] = choices(self)
            .iter()
            .position(|row| row == choice)
            .unwrap_or_default();
        self.analog_gamepad.mapping.app_specific.cleared[slot] = false;
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let payload = json!({"outputType":"keyboardGroup","outputTypeSubset":"appSpecific","keyboardGroup":choice["keyboardGroup"]});
            two_tap::replace_slot(&mut edit.slots[slot], payload);
            edit.slots[slot].category = "APP_SPECIFIC".into();
            self.analog_gamepad.mapping.app_specific.custom[slot] = false;
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn mapping_app_specific_element(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(_) = &self.analog_gamepad.mapping.edit else {
            return div().into_any_element();
        };
        let presets = choices(self);
        let custom = custom(self, slot);
        let custom_active = self.mapping_keyboard_recording_active(slot);
        let name = self.draft["name"]
            .as_str()
            .filter(|name| data(self.spec.product_id).profiles.contains_key(*name))
            .unwrap_or("Custom Application");
        v_flex()
            .w(surface::css(210.))
            .mb(surface::css(10.))
            .text_size(surface::css(14.))
            .child(
                div()
                    .relative()
                    .top(surface::css(-10.))
                    .child(name.to_uppercase()),
            )
            .child(
                v_flex()
                    .id(SharedString::from(format!("mapping-app-scroll-{slot}")))
                    .h(surface::css(350.))
                    .mt(surface::css(1.))
                    .mb(surface::css(20.))
                    .overflow_y_scroll()
                    .overflow_x_hidden()
                    .bg(Colors::list())
                    .children(presets.iter().enumerate().map(|(index, choice)| {
                        if choice["name"] == "divider" {
                            return div()
                                .mx(surface::css(6.))
                                .my(surface::css(4.))
                                .h(surface::css(1.))
                                .flex_shrink_0()
                                .bg(Colors::divider())
                                .into_any_element();
                        }
                        let choice = choice.clone();
                        let selected = !custom
                            && self.analog_gamepad.mapping.app_specific.selected[slot] == index;
                        let primary = choice["name"]
                            .as_str()
                            .or_else(|| choice["content"].as_str())
                            .unwrap_or_default();
                        let secondary = choice["name"]
                            .as_str()
                            .or_else(|| choice["secondContent"].as_str())
                            .unwrap_or_default();
                        BaseButton::new(SharedString::from(format!(
                            "mapping-app-preset-{slot}-{index}"
                        )))
                        .w_full()
                        .flex_shrink_0()
                        .px(surface::css(5.))
                        .py(surface::css(4.))
                        .mb(surface::css(4.))
                        .min_h(surface::css(25.))
                        .rounded(surface::css(3.))
                        .border_1()
                        .border_color(if selected {
                            Colors::selected_border()
                        } else {
                            cx.theme().transparent
                        })
                        .selected(selected)
                        .disabled(choice["disabled"] == true)
                        .bg(if selected { Colors::active() } else { Colors::row() })
                        .text_color(Colors::text())
                        .child(
                            v_flex()
                                .w_full()
                                .child(div().line_height(surface::css(17.)).child(t(primary)))
                                .when(choice["disabled"] != true, |row| {
                                    row.child(
                                        div()
                                            .text_size(surface::css(12.))
                                            .line_height(surface::css(14.))
                                            .text_color(Colors::secondary())
                                            .child(secondary.to_owned()),
                                    )
                                }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.stage_mapping_app_preset(slot, &choice, cx)
                        }))
                        .into_any_element()
                    }))
                    .child(
                        v_flex()
                            .id(SharedString::from(format!("mapping-app-custom-{slot}")))
                            .flex_shrink_0()
                            .rounded(surface::css(3.))
                            .border_1()
                            .border_color(if custom_active {
                                Colors::selected_border()
                            } else {
                                cx.theme().transparent
                            })
                            .bg(if custom_active {
                                Colors::active()
                            } else {
                                Colors::custom()
                            })
                            .text_color(Colors::text())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.activate_mapping_app_custom(slot, cx);
                            }))
                            .child(
                                div()
                                    .px(surface::css(5.))
                                    .pt(surface::css(5.))
                                    .pb(surface::css(3.))
                                    .line_height(surface::css(17.))
                                    .child(t(data(self.spec.product_id)
                                        .messages
                                        .get("ZGg")
                                        .map(String::as_str)
                                        .unwrap_or("CUSTOM_SHORTCUT"))),
                            )
                            .child(
                                div()
                                    .p(surface::css(5.))
                                    .child(self.mapping_keyboard_recording_element(slot, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}
