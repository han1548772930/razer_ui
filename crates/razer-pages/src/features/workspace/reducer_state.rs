//! Existing product roots mount their controls independently of profileReducer.
//! Only a selected profile carrying settings can receive edited UI data.
use razer_model::{model::Device, settings::ProfileSettings};

pub(super) struct ReducerState {
    pub(super) settings: ProfileSettings,
    profile: Option<String>,
}

impl ReducerState {
    pub(super) fn new(pid: u32) -> Self {
        // Current UI brightnessReducer, lightingEffectsReducer and
        // switchOffLightingReducer initialize before MW_SET_PROFILE_TO_UI.
        // These values are presentation state and never create a local profile.
        let mut settings = ProfileSettings::default();
        settings.lighting.enabled = false;
        settings.lighting.brightness = 0;
        settings.lighting.display_off = false;
        settings.lighting.idle_enabled = false;
        settings.lighting.idle_minutes = 0;
        settings.lighting.advanced = false;
        settings.lighting.colors.clear();
        settings.lighting.parameters.clear();
        if pid == 653 {
            // 653 ta pollingRate and bn powerReducer are separate UI reducers.
            settings.polling = 125;
            settings.idle_minutes = 15;
            settings.keyboard.disable_alt_tab = false;
            settings.keyboard.disable_alt_f4 = false;
        } else if pid == 777 {
            // 777 oI/dr EQ reducers have no bands until their MW update; tO
            // playbackReducer and jT powerReducer retain independent values.
            settings.audio.bands.clear();
            settings.audio.custom.clear();
            settings.mic.bands.clear();
            settings.mic.custom.clear();
            settings.volume = 0;
            settings.playback_enabled = true;
            settings.idle_minutes = 15;
        }
        Self {
            settings,
            profile: None,
        }
    }

    pub(super) fn restore(&mut self, device: &Device) {
        self.profile = None;
        if let Some(profile) = device.active_profile_obj()
            && let Some(settings) = &profile.settings
        {
            self.settings = settings.clone();
            self.profile = Some(profile.id.clone());
        }
    }

    pub(super) fn capture(&self, device: &mut Device) {
        let Some(id) = &self.profile else {
            return;
        };
        if id != &device.active_profile {
            return;
        }
        if let Some(profile) = device.profiles.iter_mut().find(|profile| &profile.id == id)
            && let Some(settings) = &mut profile.settings
        {
            *settings = self.settings.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_model::model::Profile;

    fn profile(id: &str, volume: u8) -> Profile {
        Profile {
            id: id.into(),
            guid: format!("guid-{id}"),
            name: id.into(),
            settings: Some(ProfileSettings {
                volume,
                ..Default::default()
            }),
            source_settings: None,
            dpi_stages: None,
        }
    }

    #[test]
    fn empty_existing_products_keep_ui_initial_values_out_of_profile_storage() {
        for pid in [653, 777, 3072, 3073, 3074, 3076, 3077, 3078, 3080] {
            let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
            let before = serde_json::to_value(&device).unwrap();
            let mut state = ReducerState::new(pid);
            state.restore(&device);
            assert!(!state.settings.lighting.enabled);
            assert_eq!(state.settings.lighting.brightness, 0);
            assert_eq!(state.settings.lighting.idle_minutes, 0);
            state.settings.lighting.brightness = 73;
            state.capture(&mut device);
            assert_eq!(serde_json::to_value(&device).unwrap(), before);
        }
    }

    #[test]
    fn missing_settings_or_selected_identity_never_receive_another_profiles_ui_state() {
        let mut device = razer_model::product_preview::registered_preview(653).unwrap();
        device.profiles = vec![profile("first", 29), profile("selected", 71)];
        device.active_profile = "selected".into();
        let mut state = ReducerState::new(653);
        state.restore(&device);
        state.settings.volume = 42;
        device.active_profile = "missing".into();
        state.capture(&mut device);
        assert_eq!(device.profiles[0].settings.as_ref().unwrap().volume, 29);
        assert_eq!(device.profiles[1].settings.as_ref().unwrap().volume, 71);
        device.active_profile = "first".into();
        device.profiles[0].settings = None;
        state.restore(&device);
        state.settings.volume = 51;
        state.capture(&mut device);
        assert!(device.profiles[0].settings.is_none());
        assert_eq!(device.profiles[1].settings.as_ref().unwrap().volume, 71);
    }

    #[test]
    fn selected_profile_restore_and_capture_preserve_unselected_profiles_and_source_data() {
        let mut device = razer_model::product_preview::registered_preview(777).unwrap();
        device.profiles = vec![profile("first", 29), profile("selected", 71)];
        device.profiles[1].source_settings = Some(serde_json::json!({"extension":42}));
        device.active_profile = "selected".into();
        let first = device.profiles[0].clone();
        let mut state = ReducerState::new(777);
        assert!(state.settings.audio.bands.is_empty());
        state.restore(&device);
        assert_eq!(state.settings.volume, 71);
        state.settings.volume = 42;
        state.capture(&mut device);
        assert_eq!(device.profiles[0], first);
        assert_eq!(device.profiles[1].settings.as_ref().unwrap().volume, 42);
        assert_eq!(
            device.profiles[1].source_settings,
            Some(serde_json::json!({"extension":42}))
        );
        device.active_profile = "first".into();
        state.restore(&device);
        assert_eq!(state.settings.volume, 29);
    }
}
