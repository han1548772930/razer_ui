//! Ordinary forms actually rendered after audioGroup/brightnessGlobalGroup mappings.
//! Source flags exclude advanced Audio on all currently audited keyboard products.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};

#[derive(Deserialize)]
struct Metadata {
    product_id: u32,
    audio: Vec<Value>,
    brightness: Vec<Value>,
    is_xbox: bool,
    brightness_in_development: bool,
    audio_spaced: bool,
    labels: BTreeMap<String, String>,
}
fn metadata(pid: u32) -> Option<&'static Metadata> {
    static DATA: OnceLock<Vec<Metadata>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_audio_brightness_catalog())
        .iter()
        .find(|row| row.product_id == pid)
}
#[derive(Clone, Default, Deserialize, Serialize)]
pub(super) struct Draft {
    #[serde(default)]
    initialized: bool,
    pub(super) selected: usize,
    pub(super) original_index: usize,
    pub(super) can_save: bool,
    pub(super) changed: bool,
}
#[derive(Default)]
pub(super) struct State {
    owner: Option<EntityId>,
    generation: u64,
    devices: Option<bool>,
    selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    subscriptions: [Option<Subscription>; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.owner = None;
        self.devices = None;
        self.selectors = Default::default();
        self.subscriptions = Default::default();
    }
    fn has_led(&self) -> bool {
        self.devices == Some(true)
    }
}
#[derive(Clone)]
pub struct KeyboardMappingBrightnessReadRequested {
    pub product_id: u32,
    pub profile_guid: String,
    pub owner: EntityId,
    pub generation: u64,
}
pub struct KeyboardMappingBrightnessObservation {
    pub product_id: u32,
    pub profile_guid: String,
    pub owner: EntityId,
    pub generation: u64,
    /// Original getMemoryStorageItem first item's parsed value.payload.devices.
    pub has_led_devices: bool,
}
impl EventEmitter<KeyboardMappingBrightnessReadRequested> for KeyboardProductWorkspace {}
fn rows<'a>(
    metadata: &'a Metadata,
    category: &str,
) -> Option<(&'static str, &'static str, &'a Vec<Value>)> {
    match category {
        "AUDIO_FUNCTION" => Some(("audioGroup", "audioAssignment", &metadata.audio)),
        "GLOBAL_BRIGHTNESS" => Some((
            "brightnessGlobalGroup",
            "brightnessGlobalAssignment",
            &metadata.brightness,
        )),
        _ => None,
    }
}
pub(super) fn staged(pid: u32, slot: &Slot) -> Option<Value> {
    let (group, field, choices) = rows(metadata(pid)?, &slot.category)?;
    let id = choices.get(slot.audio_brightness.selected)?["id"].as_str()?;
    let mut result = json!({"outputType":group});
    result[group] = json!({});
    result[group][field] = json!(id);
    Some(result)
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    slot.audio_brightness.can_save && staged(workspace.spec.product_id, slot).is_some()
}
impl KeyboardProductWorkspace {
    pub fn accepts_mapping_brightness(
        &self,
        request: &KeyboardMappingBrightnessReadRequested,
    ) -> bool {
        request.product_id == self.spec.product_id
            && self.draft["guid"] == request.profile_guid
            && self.analog_gamepad.mapping.audio_brightness.owner == Some(request.owner)
            && self.analog_gamepad.mapping.audio_brightness.generation == request.generation
            && self
                .analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(|edit| {
                    edit.slots
                        .iter()
                        .any(|slot| slot.category == "GLOBAL_BRIGHTNESS")
                })
    }
    pub fn observe_mapping_brightness(
        &mut self,
        observation: KeyboardMappingBrightnessObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let request = KeyboardMappingBrightnessReadRequested {
            product_id: observation.product_id,
            profile_guid: observation.profile_guid,
            owner: observation.owner,
            generation: observation.generation,
        };
        if request.owner != cx.entity_id() || !self.accepts_mapping_brightness(&request) {
            return;
        }
        let previous = self.analog_gamepad.mapping.audio_brightness.has_led();
        self.analog_gamepad.mapping.audio_brightness.devices = Some(observation.has_led_devices);
        let has_led = self.analog_gamepad.mapping.audio_brightness.has_led();
        // C first calls enableSave(!newLed,true,false); the [A] effect follows
        // with enableSave(newLed,true,false). Preserve the resulting state.
        if previous != has_led {
            let development =
                metadata(self.spec.product_id).is_some_and(|m| m.brightness_in_development);
            if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
                for slot in &mut edit.slots {
                    if slot.category == "GLOBAL_BRIGHTNESS" {
                        slot.audio_brightness.can_save = has_led && !development;
                        slot.audio_brightness.changed = !development;
                    }
                }
            }
            self.analog_gamepad.mapping.can_save_after_prompt = has_led && !development;
        }
        self.init_mapping_audio_brightness(0, false, window, cx);
        self.init_mapping_audio_brightness(1, false, window, cx);
        cx.notify();
    }
    pub(super) fn init_mapping_audio_brightness(
        &mut self,
        slot: usize,
        request: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let category = edit.slots[slot].category.clone();
        let Some(m) = metadata(self.spec.product_id) else {
            return;
        };
        let Some((group, field, choices)) = rows(m, &category) else {
            self.analog_gamepad.mapping.audio_brightness.selectors[slot] = None;
            self.analog_gamepad.mapping.audio_brightness.subscriptions[slot] = None;
            return;
        };
        if request {
            let index = choices
                .iter()
                .position(|choice| choice["id"] == edit.slots[slot].staged[group][field]);
            let draft = &mut self.analog_gamepad.mapping.edit.as_mut().unwrap().slots[slot]
                .audio_brightness;
            if !draft.initialized && category == "AUDIO_FUNCTION" {
                // Ordinary source _e only restores values in advanced/GameChat
                // branches. Existing Audio therefore retains useState(0),
                // enables Save and does not select its previous assignment.
                draft.selected = 0;
                draft.original_index = index.unwrap_or(0);
                draft.can_save = true;
            } else if !draft.initialized {
                draft.selected = index.unwrap_or(0);
                draft.original_index = index.unwrap_or(0);
                draft.can_save = index.is_none() && !m.brightness_in_development;
            }
            if !draft.initialized {
                draft.changed = false;
                draft.initialized = true;
            }
            if slot == 0 {
                self.analog_gamepad.mapping.can_save_after_prompt = draft.can_save;
            }
            self.analog_gamepad.mapping.audio_brightness.owner = Some(cx.entity_id());
            if category == "GLOBAL_BRIGHTNESS" {
                self.analog_gamepad.mapping.audio_brightness.generation = self
                    .analog_gamepad
                    .mapping
                    .audio_brightness
                    .generation
                    .wrapping_add(1);
                self.analog_gamepad.mapping.audio_brightness.devices = None;
                cx.emit(KeyboardMappingBrightnessReadRequested {
                    product_id: self.spec.product_id,
                    profile_guid: text(&self.draft["guid"]),
                    owner: cx.entity_id(),
                    generation: self.analog_gamepad.mapping.audio_brightness.generation,
                });
            }
        }
        let selected = self.analog_gamepad.mapping.edit.as_ref().unwrap().slots[slot]
            .audio_brightness
            .selected;
        let choices = choices
            .iter()
            .enumerate()
            .map(|(index, choice)| Choice::localized(index.to_string(), text(&choice["content"])))
            .collect::<Vec<_>>();
        if !request
            && let Some(selector) = &self.analog_gamepad.mapping.audio_brightness.selectors[slot]
        {
            selector.update(cx, |state, cx| {
                state.set_items(choices, window, cx);
                state.set_selected_index(Some(IndexPath::new(selected)), window, cx);
            });
            return;
        }
        let selector = cx.new(|cx| {
            SelectState::new(choices, Some(IndexPath::new(selected)), window, cx).searchable(false)
        });
        let category_for_change = category.clone();
        let subscription = cx.subscribe_in(&selector, window, move |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(id)) = event else {
                return;
            };
            let Ok(index) = id.parse::<usize>() else {
                return;
            };
            let Some(m) = metadata(this.spec.product_id) else {
                return;
            };
            let has_led = this.analog_gamepad.mapping.audio_brightness.has_led();
            let Some(edit) = &mut this.analog_gamepad.mapping.edit else {
                return;
            };
            let staged_slot = &mut edit.slots[slot];
            if staged_slot.category != category_for_change {
                return;
            }
            if category_for_change == "GLOBAL_BRIGHTNESS" && m.brightness_in_development {
                staged_slot.audio_brightness.can_save = false;
                this.init_mapping_audio_brightness(slot, false, window, cx);
                cx.notify();
                return;
            }
            if !rows(m, &category_for_change).is_some_and(|(_, _, rows)| index < rows.len()) {
                return;
            }
            staged_slot.audio_brightness.selected = index;
            staged_slot.audio_brightness.changed = true;
            staged_slot.audio_brightness.can_save = category_for_change == "AUDIO_FUNCTION"
                || has_led && index != staged_slot.audio_brightness.original_index;
            this.analog_gamepad.mapping.can_save_after_prompt =
                staged_slot.audio_brightness.can_save;
            if let Some(value) = staged(this.spec.product_id, staged_slot) {
                two_tap::replace_slot(staged_slot, value);
            }
            cx.emit(KeyboardProductChanged);
            cx.notify();
        });
        self.analog_gamepad.mapping.audio_brightness.selectors[slot] = Some(selector);
        self.analog_gamepad.mapping.audio_brightness.subscriptions[slot] = Some(subscription);
    }
    pub(super) fn mapping_audio_brightness_element(
        &self,
        slot: usize,
        _cx: &Context<Self>,
    ) -> AnyElement {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return div().into_any_element();
        };
        let Some(m) = metadata(self.spec.product_id) else {
            return div().into_any_element();
        };
        let audio = edit.slots[slot].category == "AUDIO_FUNCTION";
        let mut body = v_flex()
            .mx(surface::css(20.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc))
            .when(audio && m.audio_spaced, |body| body.mb(surface::css(30.)));
        if let Some(selector) = &self.analog_gamepad.mapping.audio_brightness.selectors[slot] {
            body = body.child(
                div()
                    .when(audio && m.audio_spaced, |body| body.mb(surface::css(20.)))
                    .child(
                        Select::new(selector)
                            .w(surface::css(210.))
                            .h(surface::css(27.))
                            .menu_max_h(surface::css(180.)),
                    ),
            );
        }
        if audio {
            if !m.audio_spaced {
                body = body.child(div().h(surface::css(17.)));
            }
            if m.is_xbox {
                body = body.child(
                    div()
                        .when(m.audio_spaced, |body| body.mt(surface::css(20.)))
                        .child(t(&m.labels["xbox"]))
                        .child(
                            div()
                                .id(("keyboard-audio-xbox-tooltip", slot))
                                .child(
                                    img("synapse/keyboard-mapping-notices/help.svg")
                                        .size(surface::css(16.)),
                                )
                                .tooltip(move |window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(t(
                                        &m.labels["xbox_tooltip"],
                                    ))
                                    .build(window, cx)
                                }),
                        ),
                );
            }
        } else {
            let led = self.analog_gamepad.mapping.audio_brightness.has_led();
            body = body.child(
                div()
                    .my(surface::css(20.))
                    .text_color(rgb(if led { 0x999999 } else { 0xfd4949 }))
                    .child(t(&m.labels[if led { "available" } else { "unavailable" }])),
            );
        }
        body.into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_audio_brightness_data.
fn read_keyboard_mapping_audio_brightness_catalog() -> Vec<Metadata> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Metadata>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Metadata> {
        let value = match r.tag()? {
            0 => Metadata {
                product_id: r.child(decode_2)?,
                audio: r.child(decode_3)?,
                brightness: r.child(decode_3)?,
                is_xbox: r.child(decode_5)?,
                brightness_in_development: r.child(decode_5)?,
                audio_spaced: r.child(decode_5)?,
                labels: r.child(decode_6)?,
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
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_7, decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_AUDIO_BRIGHTNESS_DATA,
        0,
        "1182e7979a7eafb0c5269bddbb6722798b9ec61c949c328717cdd310e42a215d",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_audio_brightness_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_audio_brightness_data_decodes() {
        let _ = super::read_keyboard_mapping_audio_brightness_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_audio_brightness_data.
