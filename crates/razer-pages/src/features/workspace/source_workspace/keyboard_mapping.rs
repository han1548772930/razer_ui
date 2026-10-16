//! Supply the explicitly local profile library to current keyboard mapping UI.
use super::*;
use crate::features::keyboard_products::{
    KeyboardMappingAiObservation, KeyboardMappingInterDeviceObservation,
    KeyboardMappingMacroObservation, KeyboardMappingProfilesObservation,
};

fn source_guid(profile: &Profile) -> Option<&str> {
    profile
        .source_settings
        .as_ref()
        .and_then(|value| value["guid"].as_str())
        .filter(|guid| !guid.is_empty())
        .or_else(|| (!profile.guid.is_empty()).then_some(profile.guid.as_str()))
}

impl SourceProductWorkspace {
    pub fn observe_running_games(&mut self, count: Option<usize>, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.observe_running_games(count, cx));
        }
    }

    pub fn consume_interdevice_mapping(&mut self, raw: Value, cx: &mut Context<Self>) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => {
                body.update(cx, |body, cx| body.consume_interdevice_mapping(raw, cx))
            }
            FamilyBody::Keyboard(body) => {
                body.update(cx, |body, cx| body.consume_interdevice_mapping(raw, cx))
            }
            _ => false,
        }
    }
    pub fn set_local_mapping_targets(
        &mut self,
        devices: &[Device],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use razer_model::model::DeviceConnectionObservation;
        let rows: Vec<Value> = devices
            .iter()
            .filter(|device| {
                let serial = device.serial_number.to_ascii_uppercase();
                !serial.starts_with("PREVIEW-")
                    && !serial.starts_with("DEMO-")
                    && matches!(
                        device.dashboard.connection_observation,
                        Some(
                            DeviceConnectionObservation::UsbPresent
                                | DeviceConnectionObservation::HidPresent
                        )
                    )
            })
            .map(|device| {
                let mut row = super::super::mouse_products::mapping_target_info(device.product_id)
                    .or_else(|| {
                        super::super::keyboard_products::mapping_target_info(device.product_id)
                    })
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                row["productId"] = serde_json::json!(device.product_id);
                row["productName"] =
                    serde_json::to_value(&device.product_name).unwrap_or(Value::Null);
                if row.get("category").is_none() {
                    row["category"] = serde_json::json!(match device.category {
                        razer_model::model::DeviceCategory::Controller => "GAMEPAD".to_owned(),
                        _ => serde_json::to_value(device.category)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_owned))
                            .unwrap_or_default(),
                    });
                }
                row["isSingleProfile"] = serde_json::json!(device.is_single_profile);
                row["deviceContainerId"] = serde_json::json!(device.device_container_id);
                row["UIWindowName"] = serde_json::json!(device.ui_window_name);
                row["MWWindowName"] = serde_json::json!(device.mw_window_name);
                row["subDevices"] = serde_json::json!(device.sub_devices);
                if let Some(min) = device.min_dpi {
                    row["minDPI"] = serde_json::json!(min);
                }
                if let Some(max) = device.max_dpi {
                    row["maxDPI"] = serde_json::json!(max);
                }
                if let Some(step) = device.dpi_step {
                    row["dpiStep"] = serde_json::json!(step);
                }
                row["supportXYDPI"] = serde_json::json!(device.support_xy_dpi);
                row["activeProfile"] = device
                    .profiles
                    .iter()
                    .find(|profile| profile.id == device.active_profile)
                    .and_then(source_guid)
                    .map(|guid| serde_json::json!(guid))
                    .unwrap_or(Value::Null);
                row["profiles"] = serde_json::json!(
                    device
                        .profiles
                        .iter()
                        .filter_map(|profile| {
                            let guid = source_guid(profile)?;
                            let mut value = profile
                                .source_settings
                                .clone()
                                .unwrap_or(serde_json::json!({}));
                            value["guid"] = serde_json::json!(guid);
                            value["name"] = serde_json::json!(profile.name);
                            if device.product_id == 70 && value.get("DPIStages").is_some() {
                                // Current St publishes lower-case DPIStages including each
                                // source Active flag as visible; the model projection loses it.
                                if let Ok(normalized) =
                                    razer_device::mouse_legacy_mapping::normalize_profile(&value)
                                {
                                    value = normalized;
                                }
                            }
                            if value.get("dpiStages").is_none() {
                                if let Some(dpi) = &profile.dpi_stages {
                                    value["dpiStages"] =
                                        serde_json::to_value(dpi).unwrap_or(Value::Null);
                                }
                            }
                            Some(value)
                        })
                        .collect::<Vec<_>>()
                );
                row
            })
            .collect();
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                body.observe_local_mapping_targets(rows, window, cx)
            }),
            FamilyBody::Keyboard(body) => {
                if let Some(guid) = body.read(cx).lighting_owner().profile_guid {
                    body.update(cx, |body, cx| {
                        body.observe_mapping_inter_device(
                            KeyboardMappingInterDeviceObservation::new(
                                self.device.product_id,
                                guid,
                                rows,
                                None,
                            ),
                            window,
                            cx,
                        )
                    });
                }
            }
            _ => {}
        }
    }
    pub fn set_mapping_macro_library(
        &mut self,
        file: &crate::features::macro_library::MacroLibraryFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.local_mapping_macros = file.mapping_catalog();
        self.sync_local_mapping_macros(window, cx);
    }

    pub(super) fn sync_local_mapping_macros(&self, window: &mut Window, cx: &mut Context<Self>) {
        let catalog = self.local_mapping_macros.clone();
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                body.observe_local_mapping_macros(catalog, window, cx)
            }),
            FamilyBody::Keyboard(body) => {
                if let Some(profile_guid) = body.read(cx).lighting_owner().profile_guid {
                    body.update(cx, |body, cx| {
                        body.observe_mapping_macros(
                            KeyboardMappingMacroObservation {
                                product_id: self.device.product_id,
                                profile_guid,
                                macro_available: !catalog.is_empty(),
                                macro_list: catalog,
                                is_macro_installed: true,
                            },
                            window,
                            cx,
                        )
                    });
                }
            }
            _ => {}
        }
    }
    pub fn accepts_keyboard_mapping_brightness(
        &self,
        request: &crate::features::keyboard_products::KeyboardMappingBrightnessReadRequested,
        cx: &App,
    ) -> bool {
        if request.product_id != self.device.product_id {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        body.read(cx).accepts_mapping_brightness(request)
    }
    pub fn observe_keyboard_mapping_brightness(
        &mut self,
        owner: &Device,
        observation: crate::features::keyboard_products::KeyboardMappingBrightnessObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if owner.product_id != self.device.product_id
            || owner.serial_number != self.device.serial_number
            || owner.device_container_id != self.device.device_container_id
            || observation.product_id != owner.product_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        let request = crate::features::keyboard_products::KeyboardMappingBrightnessReadRequested {
            product_id: observation.product_id,
            profile_guid: observation.profile_guid.clone(),
            owner: observation.owner,
            generation: observation.generation,
        };
        if !body.read(cx).accepts_mapping_brightness(&request) {
            return false;
        }
        body.update(cx, |body, cx| {
            body.observe_mapping_brightness(observation, window, cx)
        });
        true
    }
    pub fn accepts_keyboard_mapping_dial(
        &self,
        request: &crate::features::keyboard_products::KeyboardMappingDialReadRequested,
        cx: &App,
    ) -> bool {
        if request.product_id != self.device.product_id {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        body.read(cx).accepts_mapping_dial(request)
    }
    pub fn observe_keyboard_mapping_dial(
        &mut self,
        owner: &Device,
        observation: crate::features::keyboard_products::KeyboardMappingDialObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if owner.product_id != self.device.product_id
            || owner.serial_number != self.device.serial_number
            || owner.device_container_id != self.device.device_container_id
            || observation.product_id != owner.product_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        let request = crate::features::keyboard_products::KeyboardMappingDialReadRequested {
            product_id: observation.product_id,
            profile_guid: observation.profile_guid.clone(),
            owner: observation.owner,
            generation: observation.generation,
        };
        if !body.read(cx).accepts_mapping_dial(&request) {
            return false;
        }
        body.update(cx, |body, cx| {
            body.observe_mapping_dial(observation, window, cx)
        });
        true
    }
    pub fn observe_keyboard_mapping_sensitivity_warning(
        &mut self,
        owner: &Device,
        preference: Option<Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if owner.product_id != self.device.product_id
            || owner.serial_number != self.device.serial_number
            || owner.device_container_id != self.device.device_container_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.observe_mapping_sensitivity_warning(preference, cx)
        });
        true
    }
    /// The layout is the source UI's resolved system-keyboard layout ID.
    /// A native Windows KLID is a different value and must be translated by
    /// the source-audited host adapter before reaching this observation.
    pub fn observe_keyboard_mapping_system_keyboard_layout(
        &mut self,
        owner: &Device,
        layout: u32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.observe_mapping_system_keyboard_layout(layout, cx)
        });
        true
    }

    pub fn consume_keyboard_mapping_input_redirect(
        &mut self,
        owner: &Device,
        input_product_id: u32,
        source_input_json: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.consume_mapping_input_redirect(input_product_id, source_input_json, cx)
        })
    }

    pub fn observe_keyboard_mapping_login_warning_hidden(
        &mut self,
        hidden: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_mapping_login_warning_hidden(hidden, cx)
            });
        }
    }
    pub fn accepts_keyboard_mapping_inter_device_configure(
        &self,
        request: &crate::features::keyboard_products::KeyboardMappingInterDeviceConfigureRequested,
        cx: &App,
    ) -> bool {
        if self.device.product_id != request.source_product_id() {
            return false;
        }
        match &self.body {
            FamilyBody::Keyboard(body) => body
                .read(cx)
                .accepts_mapping_inter_device_configure(request),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_inter_device(
        &mut self,
        owner: &Device,
        observation: KeyboardMappingInterDeviceObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
            || observation.product_id() != owner.product_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        if body.read(cx).lighting_owner().profile_guid.as_deref()
            != Some(observation.profile_guid())
        {
            return false;
        }
        body.update(cx, |body, cx| {
            body.observe_mapping_inter_device(observation, window, cx)
        });
        true
    }
    pub fn observe_keyboard_mapping_ai(
        &mut self,
        owner: &Device,
        observation: KeyboardMappingAiObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
            || observation.product_id() != owner.product_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        if body.read(cx).lighting_owner().profile_guid.as_deref()
            != Some(observation.profile_guid())
        {
            return false;
        }
        body.update(cx, |body, cx| {
            body.observe_mapping_ai(observation, window, cx)
        })
    }
    pub fn observe_keyboard_mapping_macros(
        &mut self,
        owner: &Device,
        observation: KeyboardMappingMacroObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
            || observation.product_id != owner.product_id
        {
            return false;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return false;
        };
        if body.read(cx).lighting_owner().profile_guid.as_deref()
            != Some(observation.profile_guid.as_str())
        {
            return false;
        }
        body.update(cx, |body, cx| {
            body.observe_mapping_macros(observation, window, cx)
        });
        true
    }
    pub(super) fn seed_keyboard_profile_identity(&self, value: &mut Option<Value>) {
        if !matches!(&self.body, FamilyBody::Keyboard(_)) {
            return;
        }
        let Some(guid) = self
            .device
            .profiles
            .iter()
            .find(|profile| profile.id == self.device.active_profile)
            .and_then(source_guid)
        else {
            return;
        };
        // This is the local library identity, never a synthesized device read.
        let target = value.get_or_insert_with(|| serde_json::json!({}));
        if let Some(target) = target.as_object_mut() {
            target.insert("guid".into(), Value::String(guid.to_owned()));
        }
    }

    pub(super) fn sync_keyboard_mapping_profiles(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_local_mapping_macros(window, cx);
        if let FamilyBody::Mouse(body) = &self.body {
            let profiles = self
                .device
                .profiles
                .iter()
                .filter_map(|profile| {
                    (!profile.guid.is_empty())
                        .then(|| serde_json::json!({"guid":profile.guid,"name":profile.name}))
                })
                .collect();
            if let Some(active) = self
                .device
                .profiles
                .iter()
                .find(|profile| profile.id == self.device.active_profile)
                .map(|profile| profile.guid.as_str())
            {
                body.update(cx, |body, cx| {
                    body.observe_local_mapping_profiles(profiles, active, window, cx)
                });
            }
            return;
        }
        let FamilyBody::Keyboard(body) = &self.body else {
            return;
        };
        let Some(profile_guid) = body.read(cx).lighting_owner().profile_guid else {
            return;
        };
        let observation = KeyboardMappingProfilesObservation {
            product_id: self.device.product_id,
            profile_guid,
            profiles: self
                .device
                .profiles
                .iter()
                .filter_map(|profile| {
                    Some(serde_json::json!({"guid": source_guid(profile)?, "name": profile.name}))
                })
                .collect(),
            is_basic_lighting_device: body.read(cx).source_is_basic_lighting_device(),
        };
        body.update(cx, |body, cx| {
            body.observe_mapping_profiles(observation, window, cx)
        });
    }
}
