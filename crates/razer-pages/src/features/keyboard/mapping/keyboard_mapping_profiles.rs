//! Current MapSwitchProfile 58361; profile/chroma lists remain observations.
use super::*;
use gpui_kit::component::radio::Radio;

#[derive(Clone)]
pub struct KeyboardMappingProfilesObservation {
    pub product_id: u32,
    pub profile_guid: String,
    /// Current profileReducer.profiles GUID/name rows, or the explicitly local
    /// profile library supplied by the owning workspace. Never device success.
    pub profiles: Vec<Value>,
    pub is_basic_lighting_device: bool,
}
#[derive(Default)]
pub(super) struct State {
    observation: Option<KeyboardMappingProfilesObservation>,
}

pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    let profiles = workspace.mapping_profile_rows();
    if profiles.len() < 2 {
        return false;
    }
    let group = &slot.staged["profileNavigationGroup"];
    if !source(workspace.spec.product_id)
        .profile_choices
        .iter()
        .any(|choice| choice["id"] == group["profileNavigationAssignment"])
    {
        return false;
    }
    if group["profileNavigationAssignment"] != "Specific" {
        return true;
    }
    profiles.iter().any(|profile| {
        profile["guid"] == group["guid"] && profile["guid"] != workspace.draft["guid"]
    }) && (group["lightpacId"].is_null()
        || workspace
            .lighting_observation
            .as_ref()
            .is_some_and(|observation| {
                observation
                    .chroma_profiles
                    .iter()
                    .any(|profile| profile["id"] == group["lightpacId"])
            }))
}

impl KeyboardProductWorkspace {
    pub fn source_is_basic_lighting_device(&self) -> bool {
        if let Some(observation) = self
            .analog_gamepad
            .mapping
            .profiles
            .observation
            .as_ref()
            .filter(|observation| {
                observation.product_id == self.spec.product_id
                    && self.draft["guid"] == observation.profile_guid
            })
        {
            return observation.is_basic_lighting_device;
        }
        self.spec.config["DeviceInfo"]["isBasicLightingDevice"]
            .as_bool()
            .unwrap_or(false)
    }

    pub fn observe_mapping_profiles(
        &mut self,
        observation: KeyboardMappingProfilesObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if observation.product_id != self.spec.product_id
            || self.draft["guid"] != observation.profile_guid
        {
            return;
        }
        self.analog_gamepad.mapping.profiles.observation = Some(observation);
        self.init_mapping_targets(0, window, cx);
        self.init_mapping_targets(1, window, cx);
        cx.notify();
    }

    pub(super) fn mapping_profile_rows(&self) -> &[Value] {
        self.analog_gamepad
            .mapping
            .profiles
            .observation
            .as_ref()
            .filter(|observation| self.draft["guid"] == observation.profile_guid)
            .map(|observation| observation.profiles.as_slice())
            .unwrap_or(&[])
    }

    fn stage_mapping_profile_option(
        &mut self,
        slot: usize,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .mapping_profile_rows()
            .iter()
            .find(|profile| profile["guid"] != self.draft["guid"])
            .map(|profile| profile["guid"].clone());
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let mut group = json!({"profileNavigationAssignment": id});
            if id == "Specific" {
                group["guid"] = target.unwrap_or(Value::Null);
            }
            edit.slots[slot].staged["profileNavigationGroup"] = group;
        }
        self.init_mapping_targets(slot, window, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn mapping_profiles_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let group = &edit.slots[slot].staged["profileNavigationGroup"];
        let profiles = self.mapping_profile_rows();
        let disabled = profiles.len() < 2;
        let mut panel = v_flex()
            .mx(surface::css(20.))
            .gap(surface::css(10.))
            .when(disabled, |panel| panel.opacity(0.3));
        for choice in &source(self.spec.product_id).profile_choices {
            let id = text(&choice["id"]);
            let label = text(&choice["content"]);
            panel = panel.child(
                Radio::new(SharedString::from(format!(
                    "keyboard-map-profile-{slot}-{id}"
                )))
                .label(t(&label))
                .checked(group["profileNavigationAssignment"] == id)
                .disabled(disabled)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.stage_mapping_profile_option(slot, &id, window, cx);
                })),
            );
        }
        if group["profileNavigationAssignment"] == "Specific" {
            panel =
                panel.children(self.mapping_target_element(slot, targets::Kind::Profile, disabled));
            let chroma = self.lighting_observation.as_ref();
            let chroma_profiles = chroma
                .map(|observation| observation.chroma_profiles.as_slice())
                .unwrap_or(&[]);
            let basic = self.source_is_basic_lighting_device();
            let chroma_disabled = basic || chroma_profiles.is_empty();
            let enabled = !group["lightpacId"].is_null();
            panel = panel.child(
                Checkbox::new(SharedString::from(format!(
                    "keyboard-map-profile-lighting-{slot}"
                )))
                .label(t("SWITCH_LIGHTING_PROFILE"))
                .checked(enabled)
                .disabled(disabled || chroma_disabled)
                .on_click(cx.listener(
                    move |this, enabled: &bool, window, cx| {
                        let profile = this
                            .lighting_observation
                            .as_ref()
                            .and_then(|observation| {
                                observation
                                    .chroma_profiles
                                    .iter()
                                    .find(|profile| {
                                        profile["id"].as_str()
                                            == observation.selected_chroma_profile_guid.as_deref()
                                    })
                                    .or_else(|| observation.chroma_profiles.first())
                            })
                            .map(|profile| profile["id"].clone());
                        if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                            let group = &mut edit.slots[slot].staged["profileNavigationGroup"];
                            if *enabled {
                                if let Some(id) = profile {
                                    group["lightpacId"] = id;
                                }
                            } else if let Some(group) = group.as_object_mut() {
                                group.remove("lightpacId");
                            }
                        }
                        this.init_mapping_targets(slot, window, cx);
                        cx.emit(KeyboardProductChanged);
                        cx.notify();
                    },
                )),
            );
            if enabled && !chroma_disabled {
                panel = panel.children(self.mapping_target_element(
                    slot,
                    targets::Kind::ProfileChroma,
                    disabled,
                ));
            }
        }
        panel.into_any_element()
    }
}
