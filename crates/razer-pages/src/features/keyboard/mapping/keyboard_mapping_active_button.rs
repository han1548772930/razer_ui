//! Current updateButtonWithMappings and assignment value producers.
use super::*;
#[derive(Deserialize)]
struct Source {
    product_id: u32,
    assignments: BTreeMap<String, Vec<String>>,
    tables: BTreeMap<String, Vec<Value>>,
}
fn source(pid: u32) -> &'static Source {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_active_button_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited current activeButton product")
}
fn label(metadata: &Source, helper: &str, id: &Value) -> Value {
    if let Some(category) = match helper {
        "eK" => Some("MOUSE_FUNCTION"),
        "u3" => Some("WINDOWS_SHORTCUT"),
        _ => None,
    } {
        if let Some(row) = super::source(metadata.product_id)
            .options
            .get(category)
            .and_then(|options| options.choices.iter().find(|row| row["id"] == *id))
        {
            return row["content"].clone();
        }
    }
    metadata
        .tables
        .get(helper)
        .and_then(|rows| rows.iter().find(|row| row["id"] == *id))
        .map(|row| row["content"].clone())
        .unwrap_or_else(|| id.clone())
}
impl KeyboardProductWorkspace {
    /// The physical SVG reads the same `isMapped` producer as its drawer.
    /// A persisted mapping entry may contain only default/actuation data;
    /// its existence alone must not paint the key as customized.
    pub(in super::super::super) fn keyboard_physical_mapped(&self, input: &str) -> bool {
        self.mapping_buttons()
            .iter()
            .find(|button| {
                button["inputID"].as_str() == Some(input)
                    || button["buttonKey"].as_str() == Some(input)
            })
            .is_some_and(|button| self.mapping_active_button(button)["isMapped"] == true)
    }

    pub(in super::super::super) fn keyboard_physical_enabled(&self, input: &str) -> bool {
        self.mapping_buttons()
            .iter()
            .find(|button| {
                button["inputID"].as_str() == Some(input)
                    || button["buttonKey"].as_str() == Some(input)
            })
            .is_none_or(|button| {
                let button = self.mapping_active_button(button);
                button["isEnabled"] != false && button["disabled"] != true
            })
    }

    pub(in super::super::super) fn mapping_active_button(&self, raw: &Value) -> Value {
        let mut button = raw.clone();
        let Some(mapping) =
            self.current_mapping(raw["inputID"].as_str().unwrap_or_default(), self.hypershift)
        else {
            return button;
        };
        if let Some(rows) = mapping["mapping"].as_array() {
            for (index, row) in rows.iter().enumerate() {
                let mut row = row.clone();
                row["inputID"] = mapping["inputID"].clone();
                self.assign_mapping_button(
                    &mut button,
                    &row,
                    if index == 0 {
                        "assignment"
                    } else {
                        "secondaryAssignment"
                    },
                );
            }
        } else {
            self.assign_mapping_button(&mut button, mapping, "assignment")
        }
        button
    }
    fn assign_mapping_button(&self, button: &mut Value, mapping: &Value, assignment: &str) {
        let output = mapping["outputType"].as_str().unwrap_or_default();
        if output == "hyperShiftGroup" && self.hypershift {
            button["isEnabled"] = json!(false)
        }
        if output == "defaultGroup" {
            return;
        }
        match self.mapping_observed_requires_synapse(mapping, assignment == "secondaryAssignment") {
            Ok(requires) => button["isRequireSynapse"] = json!(requires),
            Err(_) => {
                if let Some(button) = button.as_object_mut() {
                    button.remove("isRequireSynapse");
                }
                return;
            }
        }
        let group = &mapping[output];
        let metadata = source(self.spec.product_id);
        let inter_device = (output == "interDeviceGroup")
            .then(|| {
                self.mapping_inter_observed_devices().and_then(|rows| {
                    rows.iter()
                        .find(|row| row["productId"] == group["interPID"])
                })
            })
            .flatten();
        let (method, value) = match output {
            "keyboardGroup" => {
                let mut names = group["modifiers"]
                    .as_array()
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|id| {
                                super::source(self.spec.product_id)
                                    .key_targets
                                    .iter()
                                    .find(|row| row["inputID"] == *id)
                                    .map(|row| text(&row["name"]))
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if let Some(key) = group["key"].as_str().filter(|key| !key.is_empty()) {
                    names.push(self.recording_key_name(key))
                } else {
                    names.clear()
                }
                if let Some(mouse) = group.get("mouseGroup").filter(|_| !names.is_empty()) {
                    names.push(t(&text(&label(metadata, "eK", &mouse["mouseAssignment"]))))
                }
                ("assignKeyboardGroup", json!(names.join(" + ")))
            }
            "dynamicKeyStrokeGroup" => ("assignDynamicKeyStrokeGroup", json!("DYNAMIC_KEY_STROKE")),
            "controllerGroup" => (
                "assignControllerGroup",
                group["controllerAssignment"].clone(),
            ),
            "joystickGroup" => (
                "assignJoystickGroup",
                joystick::assignment(group)
                    .map(|value| json!(value))
                    .unwrap_or(Value::Null),
            ),
            "keymapGroup" => (
                "assignKeymapGroup",
                match group["type"].as_str() {
                    Some("specificKeymap") => group["name"].clone(),
                    Some("nextKeymap") => json!("Next Keymap"),
                    Some("previousKeymap") => json!("Previous Keymap"),
                    Some("cycleUpKeymap") => json!("Cycle Up Keymap"),
                    Some("cycleDownKeymap") => json!("Cycle Down Keymap"),
                    _ => Value::Null,
                },
            ),
            "mouseGroup" => (
                "assignMouseGroup",
                label(metadata, "eK", &group["mouseAssignment"]),
            ),
            "scrollingGroup" => (
                "assignScrollingGroup",
                label(metadata, "eO", &group["scrollingAssignment"]),
            ),
            "sensitivityGroup" => (
                "assignSensitivityGroup",
                label(metadata, "Cz", &group["sensitivityAssignment"]),
            ),
            "macroGroup" => (
                "assignMacroGroup",
                group
                    .get("name")
                    .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                    .cloned()
                    .unwrap_or(json!(" ")),
            ),
            "hyperShiftGroup" => ("assignHyperShiftGroup", json!("Razer Hypershift")),
            "launchGroup" => (
                "assignLaunchGroup",
                group.get("path").unwrap_or(&group["url"]).clone(),
            ),
            "aiLauncherGroup" => ("assignAILauncherGroup", group["assignment"].clone()),
            "multimediaGroup" => (
                "assignMultimediaGroup",
                label(
                    metadata,
                    if group["isAdvancedMultiDialAudiol"] == true {
                        "pO"
                    } else {
                        "vB"
                    },
                    &group["multimediaAssignment"],
                ),
            ),
            "win8ShortcutsGroup" => (
                "assignWindowsShortcutGroup",
                label(metadata, "u3", &group["windowsShortcutAssignment"]),
            ),
            "textBlockGroup" => ("assignTextGroup", group["text"].clone()),
            "disableGroup" => ("assignDisableGroup", json!("DISABLE")),
            "backlightGroup" => (
                "assignBacklightGroup",
                label(metadata, "A6", &group["backlightAssignment"]),
            ),
            "brightnessGlobalGroup" => (
                "assignBrightnessGlobalGroup",
                label(metadata, "Pq", &group["brightnessGlobalAssignment"]),
            ),
            "audioGroup" => (
                "assignAudioGroup",
                label(
                    metadata,
                    if group["audioMode"] == true { "zi" } else { "z4" },
                    &group["audioAssignment"],
                ),
            ),
            "knobGroup" => (
                "assignKnobGroup",
                label(metadata, "H0", &group["controlKnobAssignment"]),
            ),
            "interDeviceGroup" => {
                if group.is_null() {
                    button["isRequireSynapse"] = json!(false);
                    return;
                }
                let value = match group["interDeviceType"].as_str() {
                    Some("profileNavigationGroup")
                        if group["profileNavigationGroup"]["profileNavigationAssignment"]
                            == "Specific" =>
                    {
                        if let Some(device) = inter_device.filter(|device| {
                            device["profiles"]
                                .as_array()
                                .is_some_and(|rows| !rows.is_empty())
                        }) {
                            let profile = device["profiles"].as_array().and_then(|rows| {
                                rows.iter().find(|row| {
                                    row["guid"] == group["profileNavigationGroup"]["guid"]
                                })
                            });
                            let lighting = &group["profileNavigationGroup"]["lightpacId"];
                            if profile.is_none()
                                || (!lighting.is_null()
                                    && lighting.as_str() != Some("")
                                    && !self.lighting_observation.as_ref().is_some_and(|o| {
                                        o.chroma_profiles.iter().any(|row| row["id"] == *lighting)
                                    }))
                            {
                                button["isRequireSynapse"] = json!(false);
                                return;
                            }
                            profile
                                .map(|profile| {
                                    profile
                                        .get("name")
                                        .filter(|v| !v.is_null())
                                        .unwrap_or(&group["profileNavigationGroup"]["name"])
                                        .clone()
                                })
                                .unwrap_or(Value::Null)
                        } else {
                            button[format!("{assignment}Value")].clone()
                        }
                    }
                    Some("profileNavigationGroup") => label(
                        metadata,
                        "bS",
                        &group["profileNavigationGroup"]["profileNavigationAssignment"],
                    ),
                    Some("sensitivityGroup") => label(
                        metadata,
                        "Cz",
                        &group["sensitivityGroup"]["sensitivityAssignment"],
                    ),
                    Some("keymapGroup") => super::source(self.spec.product_id)
                        .keymap_choices
                        .iter()
                        .find(|row| row["id"] == group["keymapGroup"]["type"])
                        .map(|row| row["content"].clone())
                        .unwrap_or(Value::Null),
                    _ => button[format!("{assignment}Value")].clone(),
                };
                ("assignInterDeviceGroup", value)
            }
            "profileNavigationGroup" => {
                let value = if group["profileNavigationAssignment"] == "Specific" {
                    let Some(profile) = self
                        .mapping_profile_rows()
                        .iter()
                        .find(|profile| profile["guid"] == group["guid"])
                    else {
                        return;
                    };
                    profile["name"].clone()
                } else {
                    label(metadata, "bS", &group["profileNavigationAssignment"])
                };
                ("assignProfileGroup", value)
            }
            "lightPacGroup" => {
                let value = if !group["chromaEffectAssignment"].is_null() {
                    label(metadata, "gW", &group["chromaEffectAssignment"])
                } else {
                    let Some(profile) = self.lighting_observation.as_ref().and_then(|o| {
                        o.chroma_profiles
                            .iter()
                            .find(|row| row["id"] == group["ID"])
                    }) else {
                        return;
                    };
                    profile["name"].clone()
                };
                ("assignLightpacGroup", value)
            }
            _ => return,
        };
        let Some(categories) = metadata.assignments.get(method) else {
            return;
        };
        let Some(category) = categories
            .get(usize::from(
                output == "multimediaGroup" && group["isAdvancedMultiDialAudiol"] == true,
            ))
            .or(categories.first())
        else {
            return;
        };
        button["isMapped"] = json!(true);
        button[assignment] = json!(category);
        button[format!("{assignment}Value")] = value;
        if output != "disableGroup" {
            button[format!("{assignment}Group")] = group.clone();
            if output == "interDeviceGroup" {
                button[format!("{assignment}Group")]["isInterDeviceConnected"] =
                    json!(inter_device.is_some())
            }
        }
    }
    pub fn mapping_requirement_diagnostic(&self) -> Option<String> {
        if let Err(error) = &self.mapping_capabilities {
            return Some(error.clone());
        }
        if let Err(error) = self.mapping_macro_requirements() {
            return Some(format!("{error:#}"));
        }
        for mapping in self
            .draft
            .get("mappings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(input) = mapping.get("inputID").and_then(Value::as_str) else {
                return Some("Mapping inputID must be a string".into());
            };
            let parts = mapping.get("mapping").and_then(Value::as_array);
            for part in parts
                .map(Vec::as_slice)
                .unwrap_or(std::slice::from_ref(mapping))
            {
                if part.get("outputType").and_then(Value::as_str) == Some("defaultGroup") {
                    continue;
                }
                if let Err(error) =
                    razer_model::mapping_requirements::MappingRequirement::from_source_mapping(
                        part, input,
                    )
                {
                    return Some(format!("{error:#}"));
                }
            }
        }
        None
    }

    fn mapping_observed_requires_synapse(
        &self,
        mapping: &Value,
        secondary: bool,
    ) -> anyhow::Result<bool> {
        use razer_model::mapping_requirements::{MappingObservations, MappingRequirement};
        let capabilities = self
            .mapping_capabilities
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let input_id = mapping
            .get("inputID")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Mapping inputID must be a string"))?;
        let requirement = MappingRequirement::from_source_mapping(mapping, input_id)?;
        let observations = MappingObservations::new(
            self.observed_bluetooth(),
            self.mapping_macro_requirements()?,
        );
        Ok(capabilities.requires_synapse(&requirement, &observations, secondary))
    }
}

#[cfg(test)]
mod requirement_tests {
    #[::core::prelude::v1::test]
    fn current_keyboard_capability_boundaries_accept_every_packaged_product() {
        let products = super::read_keyboard_products_catalog();
        assert!(!products.is_empty());
        for (product_id, factory) in products {
            let product = factory();
            razer_model::mapping_requirements::MappingCapabilities::from_source_config(
                &product.config,
            )
            .unwrap_or_else(|error| panic!("PID{product_id}: {error:#}"));
        }
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_active_button_data.
fn read_keyboard_mapping_active_button_catalog() -> Vec<Source> {
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
                assignments: r.child(decode_3)?,
                tables: r.child(decode_6)?,
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
    ) -> anyhow::Result<std::collections::BTreeMap<String, std::vec::Vec<String>>> {
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
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, std::vec::Vec<serde_json::Value>>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_ACTIVE_BUTTON_DATA,
        0,
        "dfff20c2bf0e6bece30321ac0fe76624cc42a4e38c35dd99758d46b71c0fc98a",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_active_button_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_active_button_data_decodes() {
        let _ = super::read_keyboard_mapping_active_button_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_active_button_data.
