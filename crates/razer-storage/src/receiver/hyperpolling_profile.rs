//! PID179 indicator persistence is a typed edit in the existing workspace file.
//! The workspace queue publishes the file before a device command is admitted.
use anyhow::{Context as _, ensure};
use razer_model::model::Device;
use serde_json::Value;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct IndicatorEdit {
    profile_id: String,
    profile_guid: String,
    status: u8,
}
impl IndicatorEdit {
    pub fn capture(device: &Device, status: u8) -> anyhow::Result<Self> {
        ensure!(
            device.product_id == 179 && device.real_product_id == 179,
            "Indicator local edit belongs to another product"
        );
        ensure!(
            !device.active_profile.is_empty()
                && device
                    .profiles
                    .iter()
                    .any(|profile| profile.id == device.active_profile),
            "Indicator local profile is absent"
        );
        let edit = Self {
            profile_id: device.active_profile.clone(),
            profile_guid: device
                .profiles
                .iter()
                .find(|profile| profile.id == device.active_profile)
                .unwrap()
                .guid
                .clone(),
            status,
        };
        edit.validate_profile(device)?;
        Ok(edit)
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn rebase(&self, device: &mut Device) -> anyhow::Result<()> {
        self.validate_profile(device)?;
        let profile = device
            .profiles
            .iter()
            .find(|profile| profile.id == self.profile_id)
            .unwrap();
        let mut settings = profile
            .source_settings
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        set_status(&mut settings, self.status)?;
        let mut root = device
            .source_device_settings
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        let fields = root
            .as_object_mut()
            .context("Indicator local device settings are malformed")?;
        let previous = fields
            .get("version")
            .map(|value| value.as_u64().context("Indicator local version is invalid"))
            .transpose()?;
        let version = razer_device::mapping_submission::next_source_version(previous)?;
        fields.insert("version".into(), serde_json::json!(version));
        device
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.profile_id)
            .unwrap()
            .source_settings = Some(settings);
        device.source_device_settings = Some(root);
        Ok(())
    }
    pub fn validate_profile(&self, device: &Device) -> anyhow::Result<()> {
        ensure!(
            device.product_id == 179
                && device.real_product_id == 179
                && device.active_profile == self.profile_id,
            "Indicator active profile changed before local publication"
        );
        let profiles = device
            .profiles
            .iter()
            .filter(|profile| profile.id == self.profile_id)
            .collect::<Vec<_>>();
        ensure!(
            profiles.len() == 1 && profiles[0].guid == self.profile_guid,
            "Indicator profile was removed or replaced before local publication"
        );
        Ok(())
    }
}
fn set_status(settings: &mut Value, status: u8) -> anyhow::Result<()> {
    let fields = settings
        .as_object_mut()
        .context("Indicator local profile is malformed")?;
    ensure!(
        fields.get("runtime").is_none_or(Value::is_object),
        "Indicator UI snapshot is malformed"
    );
    // The status is a source byte, including truthy values absent from the
    // three UI options. Preserve all unrelated fields and extensions.
    fields.insert("indicatorLedStatus".into(), serde_json::json!(status));
    // The application's source UI snapshot is restored at /runtime. It is a
    // projection of this same saved field, without a transport receipt.
    let runtime = fields
        .entry("runtime")
        .or_insert_with(|| serde_json::json!({}));
    runtime
        .as_object_mut()
        .context("Indicator UI snapshot is malformed")?
        .insert("indicatorLedStatus".into(), serde_json::json!(status));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device() -> Device {
        let mut device = razer_model::product_preview::registered_preview(179).unwrap();
        device.source_device_settings = Some(serde_json::json!({"version":4,"extension":{"x":9}}));
        device.active_profile = "indicator-profile".into();
        device.profiles = vec![razer_model::model::Profile {
            id: device.active_profile.clone(),
            guid: "indicator-source-guid".into(),
            name: "Indicator profile".into(),
            settings: None,
            dpi_stages: None,
            source_settings: Some(
                serde_json::json!({"indicatorLedStatus":1,"runtime":{"indicatorLedStatus":1},"extension":{"y":8}}),
            ),
        }];
        device
    }
    #[test]
    fn hyperpolling_profile_retains_unknown_byte_and_extensions_without_hardware_claim() {
        let mut settings = serde_json::json!({"indicatorLedStatus":1,"mappings":[],"extension":{"x":9},"runtime":{"indicatorLedStatus":1}});
        set_status(&mut settings, 254).unwrap();
        assert_eq!(settings["indicatorLedStatus"], 254);
        assert_eq!(settings["extension"]["x"], 9);
        assert_eq!(settings["runtime"]["indicatorLedStatus"], 254);
        assert!(settings.get("acknowledged").is_none());
    }
    #[test]
    fn hyperpolling_profile_rejects_malformed_object_instead_of_replacing_it() {
        let mut settings = serde_json::json!([1, 2]);
        assert!(set_status(&mut settings, 3).is_err());
        assert_eq!(settings, serde_json::json!([1, 2]));
    }
    #[test]
    fn hyperpolling_profile_rebase_uses_latest_local_fields_and_source_version() {
        let mut latest = device();
        let edit = IndicatorEdit::capture(&latest, 3).unwrap();
        latest.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(10);
        latest.source_device_settings.as_mut().unwrap()["extension"]["x"] = serde_json::json!(19);
        latest.profiles[0].source_settings.as_mut().unwrap()["extension"]["y"] =
            serde_json::json!(18);
        edit.rebase(&mut latest).unwrap();
        let fields = latest.profiles[0].source_settings.as_ref().unwrap();
        assert_eq!(fields["indicatorLedStatus"], 3);
        assert_eq!(fields["runtime"]["indicatorLedStatus"], 3);
        assert_eq!(fields["extension"]["y"], 18);
        assert_eq!(
            latest.source_device_settings.as_ref().unwrap()["version"],
            11
        );
        assert_eq!(
            latest.source_device_settings.as_ref().unwrap()["extension"]["x"],
            19
        );
    }
    #[test]
    fn hyperpolling_profile_rejects_replaced_profile_and_malformed_revision_atomically() {
        let original = device();
        let edit = IndicatorEdit::capture(&original, 3).unwrap();
        for case in 0..6 {
            let mut latest = original.clone();
            match case {
                0 => latest.active_profile = "another-profile".into(),
                1 => latest.source_device_settings.as_mut().unwrap()["version"] = Value::Null,
                2 => {
                    latest.profiles[0].source_settings.as_mut().unwrap()["runtime"] =
                        serde_json::json!([])
                }
                3 => latest.real_product_id = 164,
                4 => latest.profiles[0].guid = "replacement-guid".into(),
                5 => latest.profiles.push(latest.profiles[0].clone()),
                _ => unreachable!(),
            }
            let before = serde_json::to_value(&latest).unwrap();
            assert!(edit.rebase(&mut latest).is_err());
            assert_eq!(serde_json::to_value(&latest).unwrap(), before);
        }
    }
}
