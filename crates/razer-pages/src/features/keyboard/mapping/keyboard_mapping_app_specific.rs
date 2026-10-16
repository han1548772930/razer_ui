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
    DATA.get_or_init(|| read_keyboard_mapping_app_interdevice_catalog())
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

// Generated catalog reader: razer-pages/keyboard_mapping_app_interdevice_data.
fn read_keyboard_mapping_app_interdevice_catalog() -> Vec<Data> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Data>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                product_id: r.child(decode_2)?,
                profiles: r.child(decode_3)?,
                fallback_app_choices: r.child(decode_6)?,
                profile_choices: r.child(decode_6)?,
                sensitivity_choices: r.child(decode_6)?,
                keymap_choices: r.child(decode_6)?,
                messages: r.child(decode_8)?,
                dpi_defaults: r.child(decode_9)?,
                configure_action: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Presets>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Presets> {
        let value = match r.tag()? {
            0 => Presets {
                choices: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, f64>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_10)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f64> {
        let value = match r.tag()? {
            0 => r.f64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_APP_INTERDEVICE_DATA,
        0,
        "1680c3bdcaf2ade40515fea86633391c639b068197aa1bddd6f57eedbad03734",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_mapping_app_interdevice_data_decodes() {
    let _ = read_keyboard_mapping_app_interdevice_catalog();
}
// End generated catalog reader: razer-pages/keyboard_mapping_app_interdevice_data.
