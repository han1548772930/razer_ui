//! Local keyboard lighting fields and source-version rebasing. These functions
//! do not read, initialize or write a physical device.
use anyhow::{Context as _, ensure};
use razer_device::{
    keyboard_current_profile::{Brightness, LightingOff},
    keyboard_settings::{BrightnessSubmission, power},
};
use razer_model::model::Device;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    brightness: Brightness,
    switch_off_lighting: LightingOff,
}
impl Configuration {
    pub fn brightness(&self) -> &Brightness {
        &self.brightness
    }
    pub fn switch_off_lighting(&self) -> &LightingOff {
        &self.switch_off_lighting
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        power::validate(&self.switch_off_lighting, &self.brightness)
    }
}

#[derive(Clone, Debug)]
pub enum Change {
    SwitchOff(LightingOff),
    Brightness(BrightnessSubmission),
}

pub fn supported(product_id: u32) -> bool {
    razer_device::keyboard_metadata::hid_capability(product_id, product_id).is_some()
        || razer_device::keyboard_metadata::ble_product_id(product_id).is_some()
}

pub fn device_fields(product_id: u32) -> &'static [&'static str] {
    if product_id == 708 {
        &["brightness", "switchOffLighting"]
    } else {
        &[]
    }
}

fn settings<'a>(device: &'a Device, profile_id: &str) -> anyhow::Result<&'a Value> {
    let profile = device
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .context("Keyboard lighting profile is absent")?;
    if device.product_id == 708 {
        device
            .source_device_settings
            .as_ref()
            .context("Keyboard global lighting fields are absent")
    } else {
        profile
            .source_settings
            .as_ref()
            .context("Keyboard profile lighting fields are absent")
    }
}

fn settings_mut<'a>(
    device: &'a mut Device,
    profile_id: &str,
) -> anyhow::Result<&'a mut Map<String, Value>> {
    if device.product_id == 708 {
        device
            .source_device_settings
            .as_mut()
            .and_then(Value::as_object_mut)
            .context("Keyboard global lighting fields are not an object")
    } else {
        device
            .profiles
            .iter_mut()
            .find(|profile| profile.id == profile_id)
            .and_then(|profile| profile.source_settings.as_mut())
            .and_then(Value::as_object_mut)
            .context("Keyboard profile lighting fields are not an object")
    }
}

pub fn read(device: &Device, profile_id: &str) -> anyhow::Result<Configuration> {
    ensure!(
        supported(device.product_id),
        "No current keyboard lighting profile identity"
    );
    let configuration: Configuration =
        serde_json::from_value(settings(device, profile_id)?.clone())?;
    configuration.validate()?;
    Ok(configuration)
}

fn replace(
    device: &mut Device,
    profile_id: &str,
    configuration: &Configuration,
) -> anyhow::Result<()> {
    let settings = settings_mut(device, profile_id)?;
    settings.insert(
        "brightness".into(),
        serde_json::to_value(&configuration.brightness)?,
    );
    settings.insert(
        "switchOffLighting".into(),
        serde_json::to_value(&configuration.switch_off_lighting)?,
    );
    Ok(())
}

pub fn merge(
    device: &mut Device,
    profile_id: &str,
    change: &Change,
) -> anyhow::Result<Configuration> {
    let mut configuration = read(device, profile_id)?;
    match change {
        Change::Brightness(submission) => {
            submission.validate()?;
            configuration.brightness.value = submission.value();
            configuration.brightness.is_enabled = submission.enabled();
        }
        Change::SwitchOff(settings) => {
            power::validate(settings, &configuration.brightness)?;
            configuration.switch_off_lighting.is_display_on = settings.is_display_on;
            configuration.switch_off_lighting.is_idle_enabled = settings.is_idle_enabled;
            configuration.switch_off_lighting.idle_minutes = settings.idle_minutes;
        }
    }
    let version = device
        .source_device_settings
        .as_ref()
        .and_then(|root| root.get("version"))
        .map(|version| {
            version
                .as_u64()
                .context("Keyboard lighting version is not an unsigned integer")
        })
        .transpose()?;
    let next = razer_device::mapping_submission::next_source_version(version)?;
    let mut candidate = device.clone();
    replace(&mut candidate, profile_id, &configuration)?;
    let root = candidate
        .source_device_settings
        .get_or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("Keyboard local device record is not an object")?;
    root.insert("version".into(), next.into());
    *device = candidate;
    Ok(configuration)
}

/// Adopt only fields whose local draft still equals the preceding saved value.
pub fn accept_saved(current: &mut Device, previous: &Device, saved: &Device) -> anyhow::Result<()> {
    ensure!(
        current.product_id == previous.product_id && current.product_id == saved.product_id,
        "Keyboard lighting saved product identity differs"
    );
    for profile in &saved.profiles {
        let (Ok(old), Ok(mut now), Ok(committed)) = (
            read(previous, &profile.id),
            read(current, &profile.id),
            read(saved, &profile.id),
        ) else {
            continue;
        };
        if now.brightness == old.brightness {
            now.brightness = committed.brightness;
        }
        if now.switch_off_lighting == old.switch_off_lighting {
            now.switch_off_lighting = committed.switch_off_lighting;
        }
        replace(current, &profile.id, &now)?;
        if current.product_id == 708 {
            break;
        }
    }
    let old = previous
        .source_device_settings
        .as_ref()
        .and_then(|root| root.get("version"));
    let now = current
        .source_device_settings
        .as_ref()
        .and_then(|root| root.get("version"));
    if now == old {
        if let Some(version) = saved
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get("version"))
        {
            let root = current
                .source_device_settings
                .get_or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .context("Keyboard local device record is not an object")?;
            root.insert("version".into(), version.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::keyboard_settings::BrightnessIntent;
    use serde_json::json;
    fn device(product: u32) -> Device {
        let mut device = razer_model::product_preview::registered_preview(product).unwrap();
        device.active_profile = "selected".into();
        device.profiles = ["selected", "other"].into_iter().map(|id| razer_model::model::Profile {
            id:id.into(),guid:id.into(),name:id.into(),settings:None,dpi_stages:None,
            source_settings:Some(json!({"brightness":{"isEnabled":true,"value":73,"unknown":7},
                "switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":5,"keep":2},
                "mappings":["unchanged"]})),
        }).collect();
        device.source_device_settings = Some(json!({"version":4,"keep":"root"}));
        if product == 708 {
            device
                .source_device_settings
                .as_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .extend(
                    device.profiles[0]
                        .source_settings
                        .as_ref()
                        .unwrap()
                        .as_object()
                        .unwrap()
                        .clone(),
                );
        }
        device
    }
    #[test]
    fn keyboard_lighting_save_preserves_other_profiles_and_extensions() {
        let mut device = device(679);
        let other = device.profiles[1].clone();
        let mut setting = read(&device, "selected").unwrap().switch_off_lighting;
        setting.is_idle_enabled = true;
        merge(&mut device, "selected", &Change::SwitchOff(setting)).unwrap();
        assert_eq!(device.profiles[1], other);
        let config = read(&device, "selected").unwrap();
        assert_eq!(config.brightness.value, 73);
        assert_eq!(config.switch_off_lighting.extensions["keep"], 2);
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["keep"],
            "root"
        );
    }
    #[test]
    fn keyboard_lighting_global_708_does_not_create_per_profile_brightness() {
        let mut device = device(708);
        let profiles = device.profiles.clone();
        let change = Change::Brightness(
            BrightnessSubmission::new(42, false, BrightnessIntent::Enabled).unwrap(),
        );
        merge(&mut device, "selected", &change).unwrap();
        assert_eq!(device.profiles, profiles);
        assert_eq!(read(&device, "other").unwrap().brightness.value, 42);
        assert!(!read(&device, "other").unwrap().brightness.is_enabled);
    }
    #[test]
    fn keyboard_lighting_missing_fields_and_invalid_versions_do_not_partially_save() {
        let mut device = device(679);
        device.source_device_settings = Some(json!({"version":"wrong"}));
        let before = serde_json::to_value(&device).unwrap();
        let change = Change::Brightness(
            BrightnessSubmission::new(42, true, BrightnessIntent::Value).unwrap(),
        );
        assert!(merge(&mut device, "selected", &change).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
        device.profiles[0]
            .source_settings
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("brightness");
        assert!(read(&device, "selected").is_err());
    }
    #[test]
    fn keyboard_lighting_save_receipt_does_not_overwrite_a_newer_independent_draft() {
        let previous = device(679);
        let mut saved = previous.clone();
        let mut current = previous.clone();
        let mut setting = read(&saved, "selected").unwrap().switch_off_lighting;
        setting.is_display_on = true;
        merge(&mut saved, "selected", &Change::SwitchOff(setting)).unwrap();
        merge(
            &mut current,
            "selected",
            &Change::Brightness(
                BrightnessSubmission::new(42, true, BrightnessIntent::Value).unwrap(),
            ),
        )
        .unwrap();
        accept_saved(&mut current, &previous, &saved).unwrap();
        let config = read(&current, "selected").unwrap();
        assert_eq!(config.brightness.value, 42);
        assert!(config.switch_off_lighting.is_display_on);
    }
}
