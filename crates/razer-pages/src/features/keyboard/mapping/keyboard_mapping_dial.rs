//! Actual MapDialFunction forms and ordered multi-button save recipes.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::{
    IndexPath,
    select::{Select, SelectEvent, SelectState},
};
#[derive(Deserialize)]
struct Source {
    product_id: u32,
    advanced: bool,
    brightness_in_development: bool,
    recipes: Vec<Recipe>,
    body_labels: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Recipe {
    id: String,
    content: String,
    group: String,
    field: String,
    values: Vec<String>,
}
fn source(pid: u32) -> Option<&'static Source> {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_dial_catalog())
        .iter()
        .find(|row| row.product_id == pid)
}
#[derive(Default, Clone, Deserialize, Serialize)]
pub(super) struct Draft {
    selected: usize,
    selected_eq: usize,
    pub(super) changed: bool,
    #[serde(default)]
    can_save: Option<bool>,
}
#[derive(Clone)]
pub struct KeyboardMappingDialObservation {
    pub product_id: u32,
    pub profile_guid: String,
    pub owner: EntityId,
    pub generation: u64,
    /// Parsed simpleEnumerateAudioDevices().deviceList; never synthetic devices.
    pub playback_devices: Vec<Value>,
    /// Current commonReducer.validDevices, unfiltered.
    pub valid_devices: Vec<Value>,
    /// Parsed MEMORY_STORAGE_LIGHTING_ENGINE_CHROMA_DEVICES_KEY payload.devices.
    pub lighting_engine_devices: Option<Vec<Value>>,
}
#[derive(Clone)]
pub struct KeyboardMappingDialReadRequested {
    pub product_id: u32,
    pub profile_guid: String,
    pub owner: EntityId,
    pub generation: u64,
}
impl EventEmitter<KeyboardMappingDialReadRequested> for KeyboardProductWorkspace {}
#[derive(Default)]
pub(super) struct State {
    owner: Option<EntityId>,
    generation: u64,
    observation: Option<KeyboardMappingDialObservation>,
    selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    speaker_selectors: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    subscriptions: [Vec<Subscription>; 2],
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.observation = None;
        self.selectors = Default::default();
        self.speaker_selectors = Default::default();
        self.subscriptions = Default::default()
    }
}
fn speakers(workspace: &KeyboardProductWorkspace) -> Vec<&Value> {
    let Some(observation) = &workspace.analog_gamepad.mapping.dial.observation else {
        return Vec::new();
    };
    let containers = observation
        .valid_devices
        .iter()
        .filter(|device| {
            device["category"] == "AUDIO"
                && matches!(
                    device["subCategory"].as_str(),
                    Some("SPEAKER" | "SPEAKER_HEAD_CUSHION")
                )
        })
        .map(|device| &device["deviceContainerId"])
        .collect::<Vec<_>>();
    observation
        .playback_devices
        .iter()
        .filter(|device| {
            device["type"] == "speaker" && containers.contains(&&device["containerId"])
        })
        .collect()
}
pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    if slot.dial.can_save == Some(false) {
        return false;
    }
    let Some(metadata) = source(workspace.spec.product_id) else {
        return false;
    };
    let Some(recipe) = metadata.recipes.get(slot.dial.selected) else {
        return false;
    };
    (recipe.id != "SwitchAudioEq" || !speakers(workspace).is_empty())
        && (recipe.id != "GlobalBrightness" || !metadata.brightness_in_development)
}
pub(super) fn merge(
    workspace: &KeyboardProductWorkspace,
    edit: &Edit,
    existing: &[Value],
) -> Vec<Value> {
    let Some(metadata) = source(workspace.spec.product_id) else {
        return existing.to_vec();
    };
    let Some(recipe) = metadata.recipes.get(edit.slots[0].dial.selected) else {
        return existing.to_vec();
    };
    let mut buttons = workspace
        .mapping_buttons()
        .iter()
        .filter(|button| {
            button["functionList"]
                .as_array()
                .is_some_and(|functions| functions.contains(&json!("DIAL_MULTI_FUNCTION")))
        })
        .collect::<Vec<_>>();
    if buttons
        .first()
        .is_some_and(|button| button["defaultValue"] == "VOLUME_UP")
    {
        buttons.reverse()
    }
    let mut result = existing.to_vec();
    for (index, button) in buttons.into_iter().enumerate() {
        let Some(value) = recipe.values.get(index).filter(|value| !value.is_empty()) else {
            continue;
        };
        let mut group = json!({});
        group[&recipe.field] = json!(value);
        if recipe.group == "audioGroup" {
            group["audioMode"] = json!(if recipe.id == "SwitchAudioEq" {
                "SwitchAudioEq"
            } else {
                "SwitchPlaybackDevice"
            });
            if recipe.id == "SwitchAudioEq" {
                let rows = speakers(workspace);
                let selected = rows.get(edit.slots[0].dial.selected_eq).copied();
                let observation = workspace.analog_gamepad.mapping.dial.observation.as_ref();
                let ids = observation
                    .map(|o| {
                        o.valid_devices
                            .iter()
                            .filter(|device| {
                                device["category"] == "AUDIO"
                                    && matches!(
                                        device["subCategory"].as_str(),
                                        Some("SPEAKER" | "SPEAKER_HEAD_CUSHION")
                                    )
                            })
                            .map(|device| device["deviceContainerId"].clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                group["equalizerPayload"] = json!({"razerAudioSpeakerIds":ids});
                if let Some(selected) = selected {
                    group["equalizerPayload"]["selectedPlaybackId"] = selected["id"].clone();
                    if let Some(device) = observation.and_then(|o| {
                        o.valid_devices
                            .iter()
                            .find(|device| device["deviceContainerId"] == selected["containerId"])
                    }) {
                        group["equalizerPayload"]["selectedSpeakerPID"] =
                            device["productId"].clone()
                    }
                }
            }
        }
        let mut payload = json!({"outputType":recipe.group});
        payload[&recipe.group] = group;
        let mut row = edit.clone();
        row.button = (*button).clone();
        row.secondary_enabled = false;
        row.slots[0] = Slot::new(payload);
        result = two_tap::merge(workspace, &row, &result);
    }
    result
}
impl KeyboardProductWorkspace {
    pub fn accepts_mapping_dial(&self, request: &KeyboardMappingDialReadRequested) -> bool {
        request.product_id == self.spec.product_id
            && self.analog_gamepad.mapping.dial.owner == Some(request.owner)
            && self.draft["guid"] == request.profile_guid
            && request.generation == self.analog_gamepad.mapping.dial.generation
            && self
                .analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(|edit| {
                    edit.slots
                        .iter()
                        .any(|slot| slot.category == "DIAL_MULTI_FUNCTION")
                })
    }
    pub fn observe_mapping_dial(
        &mut self,
        observation: KeyboardMappingDialObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if observation.product_id != self.spec.product_id
            || self.draft["guid"] != observation.profile_guid
            || observation.owner != cx.entity_id()
            || observation.generation != self.analog_gamepad.mapping.dial.generation
            || !self
                .analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(|edit| {
                    edit.slots
                        .iter()
                        .any(|slot| slot.category == "DIAL_MULTI_FUNCTION")
                })
        {
            return;
        }
        let previous = self
            .analog_gamepad
            .mapping
            .dial
            .observation
            .as_ref()
            .and_then(|o| o.lighting_engine_devices.as_ref())
            .map(|rows| !rows.is_empty())
            .unwrap_or(false);
        let led = observation
            .lighting_engine_devices
            .as_ref()
            .map(|rows| !rows.is_empty());
        self.analog_gamepad.mapping.dial.observation = Some(observation);
        if led.is_some_and(|led| led != previous) {
            if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
                for slot in &mut edit.slots {
                    if slot.category == "DIAL_MULTI_FUNCTION" {
                        slot.dial.changed = source(self.spec.product_id)
                            .is_some_and(|s| !s.brightness_in_development);
                        slot.dial.can_save = Some(
                            led == Some(false)
                                && source(self.spec.product_id)
                                    .is_some_and(|s| !s.brightness_in_development),
                        );
                    }
                }
            }
            // Original memory callback enableSave(!hasLed,true,false) is retained.
            self.analog_gamepad.mapping.can_save_after_prompt = led == Some(false)
                && source(self.spec.product_id).is_some_and(|s| !s.brightness_in_development);
        }
        self.init_mapping_dial(0, false, window, cx);
        self.init_mapping_dial(1, false, window, cx);
        cx.notify();
    }
    pub(super) fn init_mapping_dial(
        &mut self,
        slot: usize,
        request: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let Some(metadata) = source(self.spec.product_id)
            .filter(|_| edit.slots[slot].category == "DIAL_MULTI_FUNCTION")
        else {
            self.analog_gamepad.mapping.dial.selectors[slot] = None;
            self.analog_gamepad.mapping.dial.speaker_selectors[slot] = None;
            self.analog_gamepad.mapping.dial.subscriptions[slot].clear();
            return;
        };
        let selected = edit.slots[slot].dial.selected;
        let selected_eq = edit.slots[slot].dial.selected_eq;
        self.analog_gamepad.mapping.dial.owner = Some(cx.entity_id());
        if request && metadata.advanced {
            self.analog_gamepad.mapping.dial.generation =
                self.analog_gamepad.mapping.dial.generation.wrapping_add(1);
            self.analog_gamepad.mapping.dial.observation = None;
        }
        let choices = metadata
            .recipes
            .iter()
            .enumerate()
            .map(|(index, recipe)| Choice::localized(index.to_string(), recipe.content.clone()))
            .collect::<Vec<_>>();
        let speaker_choices = speakers(self)
            .iter()
            .enumerate()
            .map(|(index, device)| Choice::new(index.to_string(), text(&device["name"])))
            .collect::<Vec<_>>();
        if let (Some(selector), Some(speaker)) = (
            &self.analog_gamepad.mapping.dial.selectors[slot],
            &self.analog_gamepad.mapping.dial.speaker_selectors[slot],
        ) {
            selector.update(cx, |state, cx| {
                state.set_items(choices, window, cx);
                state.set_selected_index(Some(IndexPath::new(selected)), window, cx)
            });
            speaker.update(cx, |state, cx| {
                state.set_items(speaker_choices, window, cx);
                state.set_selected_index(Some(IndexPath::new(selected_eq)), window, cx)
            });
            return;
        }
        let selector = cx.new(|cx| {
            SelectState::new(choices, Some(IndexPath::new(selected)), window, cx).searchable(false)
        });
        let speaker = cx.new(|cx| {
            SelectState::new(
                speaker_choices,
                Some(IndexPath::new(selected_eq)),
                window,
                cx,
            )
            .searchable(false)
        });
        let change = cx.subscribe_in(&selector, window, move |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(id)) = event else {
                return;
            };
            let Ok(index) = id.parse::<usize>() else {
                return;
            };
            let can_save = source(this.spec.product_id)
                .and_then(|metadata| metadata.recipes.get(index).map(|recipe| (metadata, recipe)))
                .is_some_and(|(metadata, recipe)| {
                    (recipe.id != "SwitchAudioEq" || !speakers(this).is_empty())
                        && (recipe.id != "GlobalBrightness" || !metadata.brightness_in_development)
                });
            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                edit.slots[slot].dial.selected = index;
                edit.slots[slot].dial.changed = true;
                edit.slots[slot].dial.can_save = Some(can_save);
            }
            this.init_mapping_dial(slot, false, window, cx);
            cx.emit(KeyboardProductChanged);
            cx.notify()
        });
        let eq = cx.subscribe_in(&speaker, window, move |this, _, event, _, cx| {
            let SelectEvent::Confirm(Some(id)) = event else {
                return;
            };
            if let Ok(index) = id.parse::<usize>() {
                if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                    edit.slots[slot].dial.selected_eq = index
                }
                cx.emit(KeyboardProductChanged);
                cx.notify()
            }
        });
        self.analog_gamepad.mapping.dial.selectors[slot] = Some(selector);
        self.analog_gamepad.mapping.dial.speaker_selectors[slot] = Some(speaker);
        self.analog_gamepad.mapping.dial.subscriptions[slot] = vec![change, eq];
        if request && metadata.advanced {
            cx.emit(KeyboardMappingDialReadRequested {
                product_id: self.spec.product_id,
                profile_guid: text(&self.draft["guid"]),
                owner: cx.entity_id(),
                generation: self.analog_gamepad.mapping.dial.generation,
            });
        }
    }
    pub(super) fn mapping_dial_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let Some(metadata) = source(self.spec.product_id) else {
            return div().into_any_element();
        };
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return div().into_any_element();
        };
        let Some(recipe) = metadata.recipes.get(edit.slots[slot].dial.selected) else {
            return div().into_any_element();
        };
        let mut body = v_flex()
            .mx(surface::css(20.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc));
        if let Some(selector) = &self.analog_gamepad.mapping.dial.selectors[slot] {
            body = body.child(
                Select::new(selector)
                    .w(surface::css(210.))
                    .h(surface::css(27.))
                    .menu_max_h(surface::css(180.)),
            )
        }
        if !metadata.advanced {
            return body.into_any_element();
        }
        let label = |alias: &str| {
            metadata
                .body_labels
                .get(alias)
                .map(|key| t(key))
                .unwrap_or_default()
        };
        match recipe.id.as_str() {
            "SwitchAudioEq" => {
                let rows = speakers(self);
                body = body.child(
                    v_flex()
                        .my(surface::css(20.))
                        .child(if rows.is_empty() {
                            div()
                                .mb(surface::css(20.))
                                .text_color(rgb(0xfd4949))
                                .child(label("diY"))
                                .into_any_element()
                        } else {
                            v_flex()
                                .mb(surface::css(20.))
                                .child(
                                    div()
                                        .mb(surface::css(10.))
                                        .line_height(surface::css(14.))
                                        .child(label("fYN").to_uppercase()),
                                )
                                .children(
                                    self.analog_gamepad.mapping.dial.speaker_selectors[slot]
                                        .as_ref()
                                        .map(|state| {
                                            Select::new(state)
                                                .w(surface::css(210.))
                                                .h(surface::css(27.))
                                                .menu_max_h(surface::css(180.))
                                        }),
                                )
                                .into_any_element()
                        })
                        .child(
                            BaseButton::new("keyboard-map-dial-compatible")
                                .mb(surface::css(20.))
                                .text_size(surface::css(14.))
                                .underline()
                                .child(label("RFv"))
                                .on_click(|_, _, cx| {
                                    cx.open_url(
                                        "https://mysupport.razer.com/app/answers/detail/a_id/13138",
                                    )
                                }),
                        ),
                );
            }
            "SwitchPlaybackDevice" => {
                body = body.child(
                    div()
                        .my(surface::css(20.))
                        .text_color(rgb(0x999999))
                        .child(label("uTW")),
                )
            }
            "GlobalBrightness" => {
                let led = self
                    .analog_gamepad
                    .mapping
                    .dial
                    .observation
                    .as_ref()
                    .and_then(|o| o.lighting_engine_devices.as_ref())
                    .is_some_and(|rows| !rows.is_empty());
                body = body.child(
                    div()
                        .my(surface::css(20.))
                        .text_color(rgb(if led { 0x999999 } else { 0xfd4949 }))
                        .child(label(if led { "a21" } else { "Chc" })),
                );
            }
            "SwitchChromaEffect" => {
                let installed = self
                    .lighting_observation
                    .as_ref()
                    .is_some_and(|o| o.chroma_installed_module == Some(true));
                body = body.child(v_flex().my(surface::css(20.)).child(if installed {
                    v_flex()
                        .child(
                            div()
                                .my(surface::css(20.))
                                .text_color(rgb(0x999999))
                                .child(label("Ssd")),
                        )
                        .child(
                            BaseButton::new("keyboard-map-dial-configure-chroma")
                                .mb(surface::css(20.))
                                .text_size(surface::css(14.))
                                .underline()
                                .child(label("jrk"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request_lighting_command(
                                        KeyboardLightingCommand::OpenMappingChromaStudio,
                                        cx,
                                    )
                                })),
                        )
                        .into_any_element()
                } else {
                    self.mapping_chroma_install_element(slot, cx)
                }));
            }
            _ => {}
        }
        body.into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_dial_data.
fn read_keyboard_mapping_dial_catalog() -> Vec<Source> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Source>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                product_id: r.child(decode_2)?,
                advanced: r.child(decode_3)?,
                brightness_in_development: r.child(decode_3)?,
                recipes: r.child(decode_4)?,
                body_labels: r.child(decode_8)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Recipe>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Recipe> {
        let value = match r.tag()? {
            0 => Recipe {
                id: r.child(decode_6)?,
                content: r.child(decode_6)?,
                group: r.child(decode_6)?,
                field: r.child(decode_6)?,
                values: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_6, decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_DIAL_DATA,
        0,
        "a8c112eb13cc9f7ad5ad0129b9b08c40ca0d76019ccaa76d4384c0ac018d4736",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_dial_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_dial_data_decodes() {
        let _ = super::read_keyboard_mapping_dial_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_dial_data.
