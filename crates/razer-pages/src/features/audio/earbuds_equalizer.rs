use super::*;
use razer_device::{
    earbuds_auto_pause::Operation,
    earbuds_equalizer::{Preset, Setting},
};

impl AudioProductWorkspace {
    pub(super) fn earbuds_equalizer_disabled(&self) -> bool {
        razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id)
            && (!self.earbuds.active
                || !self.earbuds.initialized
                || self.earbuds.eq_persisted.is_none()
                || self.earbuds.eq_profile_error.is_some()
                || self
                    .earbuds
                    .controls
                    .as_ref()
                    .is_none_or(|controls| controls.earbud_disconnected))
    }
    pub(super) fn load_earbuds_equalizer_profile(
        &mut self,
        device: &razer_model::model::Device,
        selected: Option<Setting>,
        cx: &mut Context<Self>,
    ) {
        self.earbuds.eq_revision = self.earbuds.eq_revision.wrapping_add(1);
        let Some(selected) = selected else {
            self.earbuds.eq_presets.clear();
            self.earbuds.eq_defaults.clear();
            if let Some(profile) = self.draft.get_mut("profile").and_then(Value::as_object_mut) {
                profile.remove("audioEqualizer");
            }
            return;
        };
        let Some(eq) = self.spec.equalizers.first() else {
            return;
        };
        if device.profiles.is_empty() && device.active_profile.is_empty() {
            match razer_storage::earbuds_profiles::initial_equalizer_source(device.product_id) {
                Ok(source) => self.draft["profile"]["audioEqualizer"] = source,
                Err(error) => {
                    self.earbuds.eq_profile_error = Some(format!("{error:#}"));
                    return;
                }
            }
        }
        if let Some(source) = device
            .profiles
            .iter()
            .find(|profile| profile.id == device.active_profile)
            .and_then(|profile| profile.source_settings.as_ref())
        {
            let profile = source.get("profile").unwrap_or(source);
            if let Some(equalizer) = profile.get("audioEqualizer") {
                self.draft["profile"]["audioEqualizer"] = equalizer.clone();
            }
        }
        let state = &mut self.draft["equalizers"][&eq.key];
        state["selected"] = json!(selected.preset.key());
        state["bands"] = json!(selected.bands);
        state["mode"] = json!(selected.mode);
        use razer_storage::earbuds_profiles as profiles;
        let presets = if device.active_profile.is_empty() {
            profiles::initial_equalizer_presets(device.product_id)
        } else {
            profiles::read_equalizer_presets(device, &device.active_profile)
        };
        let defaults = if device.active_profile.is_empty() {
            profiles::initial_equalizer_presets(device.product_id)
        } else {
            profiles::equalizer_defaults(device, &device.active_profile)
        };
        match presets.and_then(|entries| defaults.map(|defaults| (entries, defaults))) {
            Ok((entries, defaults)) => {
                state["presets"] = json!({});
                for entry in &entries {
                    state["presets"][entry.preset.key()] = json!(entry.bands);
                }
                state["defaults"] = json!({});
                for entry in &defaults {
                    state["defaults"][entry.preset.key()] = json!(entry.bands);
                }
                self.earbuds.eq_presets = entries;
                self.earbuds.eq_defaults = defaults;
                let revision = self.earbuds.eq_revision;
                let key = eq.key.clone();
                let owner = cx.entity().downgrade();
                let handle = self.earbuds_window;
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| {
                            if view.earbuds.eq_revision != revision {
                                return;
                            }
                            for (index, value) in selected.bands.into_iter().enumerate() {
                                if let Some(slider) = view
                                    .sliders
                                    .get(&format!("/equalizers/{key}/bands/{index}"))
                                {
                                    slider.update(cx, |slider, cx| {
                                        slider.set_value(f32::from(value), window, cx)
                                    });
                                }
                            }
                        });
                    });
                });
            }
            Err(error) => self.earbuds.eq_profile_error = Some(format!("{error:#}")),
        }
    }

    pub(super) fn earbuds_equalizer_collection_changed(
        &self,
        device: &razer_model::model::Device,
    ) -> bool {
        use razer_storage::earbuds_profiles as profiles;
        let actual =
            profiles::read_equalizer_presets(device, &device.active_profile).and_then(|presets| {
                profiles::equalizer_defaults(device, &device.active_profile)
                    .map(|defaults| (presets, defaults))
            });
        match actual {
            Ok((presets, defaults)) => {
                presets != self.earbuds.eq_presets || defaults != self.earbuds.eq_defaults
            }
            Err(_) => self.earbuds.eq_profile_error.is_none(),
        }
    }

    pub(super) fn request_earbuds_equalizer(&mut self, key: &str, cx: &mut Context<Self>) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id) {
            return;
        }
        if self.earbuds_equalizer_disabled() {
            return;
        }
        let state = &self.draft["equalizers"][key];
        let parse = || -> anyhow::Result<Setting> {
            let preset: Preset = serde_json::from_value(state["selected"].clone())?;
            let values: [f64; 10] = serde_json::from_value(state["bands"].clone())?;
            anyhow::ensure!(
                values.iter().all(|value| value.is_finite()
                    && value.fract() == 0.
                    && (-6. ..=6.).contains(value)),
                "Invalid earbud EQ bands"
            );
            let bands = values.map(|value| value as i8);
            let mode = self
                .earbuds
                .eq_persisted
                .map(|value| value.mode)
                .ok_or_else(|| anyhow::anyhow!("Earbud EQ profile is absent"))?;
            let setting = Setting {
                mode,
                preset,
                bands,
            };
            setting.validate()?;
            Ok(setting)
        };
        match parse() {
            Ok(setting) => {
                self.earbuds.eq_result = None;
                self.earbuds.queue_edit(Operation::Equalizer { setting });
                self.start_earbuds_request(cx);
            }
            Err(error) => self.earbuds.eq_result = Some(Err(format!("{error:#}"))),
        }
    }
}
