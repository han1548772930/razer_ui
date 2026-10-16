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
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_active_button_data.json"))
            .expect("current activeButton producers")
    })
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
        button["isRequireSynapse"] = json!(
            assignment == "secondaryAssignment" || self.mapping_observed_requires_synapse(mapping)
        );
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
    fn mapping_observed_requires_synapse(&self, mapping: &Value) -> bool {
        let info = &self.spec.config["DeviceInfo"];
        let output = mapping["outputType"].as_str().unwrap_or_default();
        if info["isXenaT2Series"] == true && mapping["inputID"] == "DKM_SB_247" {
            return true;
        }
        if info["isOBMDevice"] != true {
            return matches!(info["category"].as_str(), Some("KEYBOARD" | "SYSTEM"));
        }
        if matches!(output, "hyperShiftGroup" | "disableGroup") {
            return false;
        }
        let specs = &info["OBMSpecs"];
        if specs["noFirmwareInputId"]
            .as_array()
            .is_some_and(|ids| ids.contains(&mapping["inputID"]))
        {
            return true;
        }
        let supported = &specs["supportedMappings"];
        if supported["controllerGroup"] == true {
            return false;
        }
        let group = &mapping[output];
        if output == "macroGroup"
            && (supported["macroGroup"] != true
                || self.observed_bluetooth() == Some(true)
                    && info["ble"]["obmMacrosAvailable"] != true
                || group["macroPlaybackOption"] == "Queue")
        {
            return true;
        }
        if output == "controllerGroup" && !info["controllerAssignmentsWithoutSynapse"].is_null() {
            return false;
        }
        if group.is_null() {
            return true;
        }
        match output {
            "keyboardGroup" => false,
            "scrollingGroup" | "sensitivityGroup" | "profileNavigationGroup" => {
                let field = match output {
                    "scrollingGroup" => "scrollingAssignment",
                    "sensitivityGroup" => "sensitivityAssignment",
                    _ => "profileNavigationAssignment",
                };
                !supported[output]
                    .as_array()
                    .is_some_and(|ids| ids.contains(&group[field]))
            }
            "multimediaGroup" => matches!(
                group["multimediaAssignment"].as_str(),
                Some("MicVolumeUp" | "MicVolumeDown" | "MuteAll" | "MuteMic")
            ),
            "mouseGroup" => {
                let turbo = group["turboMode"]["isTurbo"] == true;
                let configured = &self.spec.config[if turbo {
                    "MOUSE_TURBO_FUNC_REQUIRED_SYNAPSE_VALUES"
                } else {
                    "MOUSE_FUNC_REQUIRED_SYNAPSE_VALUES"
                }];
                if group["mouseAssignment"] == "DoubleClick" && turbo {
                    return true;
                }
                if let Some(ids) = configured.as_array().filter(|ids| !ids.is_empty()) {
                    return ids.contains(&group["mouseAssignment"]);
                }
                [
                    "ScrollLeft",
                    "ScrollRight",
                    "RepeatScrollUp",
                    "RepeatScrollDown",
                    "RepeatScrollLeft",
                    "RepeatScrollRight",
                    "BossKey",
                    "ScrollUp",
                    "ScrollDown",
                ]
                .iter()
                .any(|id| {
                    group["mouseAssignment"] == *id
                        && !supported["mouseGroup"]
                            .as_array()
                            .is_some_and(|ids| ids.contains(&json!(id)))
                })
            }
            // The actual macro Xq predicate needs the full macro observation.
            "macroGroup" => self.mapping_macro_requires_synapse(&group["guid"]),
            _ => true,
        }
    }
}
