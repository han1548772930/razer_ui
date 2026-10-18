//! Current MapLighting 74278, with the mounted isAdvancedMapLighting branch.
use super::*;
use gpui_kit::component::radio::Radio;

pub(super) fn valid(workspace: &KeyboardProductWorkspace, slot: &Slot) -> bool {
    let Some(observation) = &workspace.lighting_observation else {
        return false;
    };
    let group = &slot.staged["lightPacGroup"];
    observation.chroma_installed_module == Some(true)
        && observation.chroma_profiles.first().is_some_and(|profile| {
            profile["name"]
                .as_str()
                .is_some_and(|name| !name.is_empty())
        })
        && observation
            .chroma_profiles
            .iter()
            .any(|profile| profile["id"] == group["ID"])
        && (workspace.spec.config["DeviceInfo"]["isAdvancedMapLighting"] != true
            || source(workspace.spec.product_id)
                .lighting_choices
                .iter()
                .any(|choice| choice["id"] == group["chromaEffectAssignment"]))
}

impl KeyboardProductWorkspace {
    /// Actual MapLighting componentDidUpdate(chromaProfiles): preserve selected ID,
    /// then current Chroma ID, then index zero; enableSave(available,false,true).
    pub(in crate::features) fn reconcile_mapping_chroma_targets(&mut self) {
        let Some(observation) = &self.lighting_observation else {
            return;
        };
        if observation.chroma_profiles.is_empty() {
            return;
        }
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        let mut refreshed = false;
        for slot in &mut edit.slots {
            if slot.category != "SWITCH_LIGHTING" {
                continue;
            }
            let group = &mut slot.staged["lightPacGroup"];
            let selected = observation
                .chroma_profiles
                .iter()
                .find(|row| row["id"] == group["ID"])
                .or_else(|| {
                    observation.chroma_profiles.iter().find(|row| {
                        row["id"].as_str() == observation.selected_chroma_profile_guid.as_deref()
                    })
                })
                .unwrap_or(&observation.chroma_profiles[0]);
            group["ID"] = selected["id"].clone();
            group["Name"] = selected["name"].clone();
            slot.original = slot.staged.clone();
            refreshed = true;
        }
        if refreshed {
            self.analog_gamepad.mapping.can_save_after_prompt =
                observation.chroma_profiles[0]["name"]
                    .as_str()
                    .is_some_and(|name| !name.is_empty())
        }
    }
    pub(super) fn init_mapping_lighting(&mut self, slot: usize, cx: &mut Context<Self>) {
        let advanced = self.spec.config["DeviceInfo"]["isAdvancedMapLighting"] == true;
        let profile = self
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
            .cloned();
        let default_assignment = source(self.spec.product_id)
            .lighting_choices
            .first()
            .map(|choice| choice["id"].clone());
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            let mut group = json!({});
            if let Some(profile) = profile {
                group["ID"] = profile["id"].clone();
                group["Name"] = profile["name"].clone();
            }
            if advanced {
                group["chromaEffectAssignment"] = default_assignment.unwrap_or(Value::Null);
            }
            edit.slots[slot].staged["lightPacGroup"] = group;
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }

    pub(super) fn mapping_lighting_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let group = &edit.slots[slot].staged["lightPacGroup"];
        let observation = self.lighting_observation.as_ref();
        let installed = observation
            .is_some_and(|observation| observation.chroma_installed_module == Some(true));
        let mut panel = v_flex().mx(surface::css(20.)).gap(surface::css(10.));
        if !installed {
            return panel
                .child(self.mapping_chroma_install_element(slot, cx))
                .into_any_element();
        }
        let observation = observation.expect("installed Chroma observation");
        let advanced = self.spec.config["DeviceInfo"]["isAdvancedMapLighting"] == true;
        if advanced {
            panel = panel.children(source(self.spec.product_id).lighting_choices.iter().map(
                |choice| {
                    let id = text(&choice["id"]);
                    Radio::new(SharedString::from(format!(
                        "keyboard-map-chroma-assignment-{slot}-{id}"
                    )))
                    .label(t(&text(&choice["content"])))
                    .checked(group["chromaEffectAssignment"] == id)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                            edit.slots[slot].staged["lightPacGroup"]["chromaEffectAssignment"] =
                                json!(id);
                        }
                        cx.emit(KeyboardProductChanged);
                        cx.notify();
                    }))
                },
            ));
        }
        let profiles = &observation.chroma_profiles;
        if !advanced || group["chromaEffectAssignment"] == "SpecificChromaEffect" {
            panel =
                panel.children(self.mapping_target_element(slot, targets::Kind::Lighting, false));
        }
        if profiles
            .first()
            .is_none_or(|profile| profile["name"].as_str().is_none_or(str::is_empty))
        {
            panel = panel.child(
                div()
                    .mb(surface::css(20.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(t("CONFIGURE_CHROMA_STUDIO_MSG")),
            );
        }
        panel
            .child(
                BaseButton::new(SharedString::from(format!(
                    "keyboard-map-configure-chroma-{slot}"
                )))
                .mb(surface::css(20.))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .underline()
                .child(t("CONFIGURE_CHROMA_STUDIO"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.request_lighting_command(
                        KeyboardLightingCommand::OpenMappingChromaStudio,
                        cx,
                    );
                })),
            )
            .into_any_element()
    }
}
