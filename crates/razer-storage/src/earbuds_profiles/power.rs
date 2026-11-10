//! The source PowerSaving event stores one device-global preference.
use anyhow::{Context as _, Result, ensure};
use razer_device::earbuds_power::Setting;
use razer_model::model::Device;
use serde::Deserialize;
use serde_json::Value;

const FIELD: &str = "powerSaving";

#[derive(Deserialize)]
struct Stored {
    #[serde(rename = "isEnabled")]
    enabled: bool,
    value: f64,
}
impl Stored {
    fn setting(self) -> Result<Setting> {
        let seconds = self.value * 60.;
        ensure!(
            seconds.is_finite() && (0.0..=f64::from(u16::MAX)).contains(&seconds),
            "Earbud stored sleep time is outside the protocol range"
        );
        ensure!(
            (seconds - seconds.round()).abs() < 0.000001,
            "Earbud stored sleep time is not a whole second"
        );
        Ok(Setting::new(self.enabled, seconds.round() as u16))
    }
}

pub fn read_power(device: &Device) -> Result<Option<Setting>> {
    ensure!(
        razer_device::earbuds_protocol40::supports_auto_pause(device.product_id),
        "Product has no reviewed earbud power settings"
    );
    let Some(root) = &device.source_device_settings else {
        return Ok(None);
    };
    let root = root
        .as_object()
        .context("Earbud global settings must be an object")?;
    root.get(FIELD)
        .map(|value| -> Result<Setting> {
            serde_json::from_value::<Stored>(value.clone())
                .context("Invalid earbud global power settings")?
                .setting()
        })
        .transpose()
}

/// The official restore hook reads an explicitly present active-profile field,
/// while ordinary PowerSaving edits remain device-global. Defaults omit it.
pub fn read_power_profile(device: &Device) -> Result<Option<(String, Setting)>> {
    ensure!(
        razer_device::earbuds_protocol40::supports_auto_pause(device.product_id),
        "Product has no reviewed earbud power settings"
    );
    let profile = if device.active_profile.is_empty() {
        device.profiles.first()
    } else {
        device
            .profiles
            .iter()
            .find(|profile| profile.id == device.active_profile)
    };
    let Some(profile) = profile else {
        return Ok(None);
    };
    let Some(control) = super::workspace_control(profile.source_settings.as_ref(), FIELD)? else {
        return Ok(None);
    };
    let setting = serde_json::from_value::<Stored>(Value::Object(control.clone()))
        .context("Invalid explicit profile power settings")?
        .setting()?;
    Ok(Some((profile.id.clone(), setting)))
}

pub fn merge_power(device: &mut Device, setting: Setting) -> Result<()> {
    read_power(device)?;
    let root = device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    let root = root
        .as_object_mut()
        .context("Earbud global settings must be an object")?;
    let field = root.entry(FIELD).or_insert_with(|| serde_json::json!({}));
    let field = field
        .as_object_mut()
        .context("Earbud power settings must be an object")?;
    field.insert("isEnabled".into(), Value::Bool(setting.enabled()));
    field.insert("value".into(), Value::from(setting.minutes()));
    Ok(())
}

/// A completed save may advance the local preference only if it did not race
/// another device-global edit. It never changes a hardware observation.
pub fn accept_power_saved(device: &mut Device, previous: &Device, saved: &Device) -> Result<()> {
    if !razer_device::earbuds_protocol40::supports_auto_pause(device.product_id) {
        return Ok(());
    }
    if read_power(device)? == read_power(previous)? {
        if let Some(setting) = read_power(saved)? {
            merge_power(device, setting)?;
        }
    }
    Ok(())
}
