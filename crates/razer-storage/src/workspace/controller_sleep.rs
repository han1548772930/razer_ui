//! Device-global controller sleep preferences and migration of older page drafts.
use anyhow::{Context as _, ensure};
use razer_device::controller_sleep::{PowerSaving, capability};
use razer_model::model::{Device, Profile};

fn legacy_preference(profile: &Profile) -> anyhow::Result<Option<PowerSaving>> {
    let Some(settings) = profile.source_settings.as_ref() else {
        return Ok(None);
    };
    let settings = settings
        .as_object()
        .context("Invalid legacy controller workspace")?;
    let Some(controller) = settings.get("controller") else {
        return Ok(None);
    };
    let controller = controller
        .as_object()
        .context("Invalid legacy controller controls")?;
    let Some(power) = controller.get("power") else {
        return Ok(None);
    };
    let power = power
        .as_object()
        .context("Invalid legacy controller power settings")?;
    power
        .get("powerSaving")
        .map(|value| {
            serde_json::from_value(value.clone())
                .context("Invalid legacy controller sleep preference")
        })
        .transpose()
}

pub(super) fn migrate(device: &mut Device) -> anyhow::Result<()> {
    if capability(device.product_id).is_none() {
        ensure!(
            device.controller_power_saving.is_none(),
            "Controller sleep preference belongs to another product"
        );
        return Ok(());
    }
    let mut selected = device.controller_power_saving;
    if let Some(setting) = selected {
        setting.seconds()?;
    }
    for profile in &device.profiles {
        if let Some(setting) = legacy_preference(profile)? {
            ensure!(
                selected.is_none_or(|previous| previous == setting),
                "Conflicting controller sleep preferences require an explicit selection"
            );
            selected = Some(setting);
        }
    }
    device.controller_power_saving = selected;
    for profile in &mut device.profiles {
        if let Some(controller) = profile
            .source_settings
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|settings| settings.get_mut("controller"))
            .and_then(serde_json::Value::as_object_mut)
        {
            if let Some(power) = controller
                .get_mut("power")
                .and_then(serde_json::Value::as_object_mut)
            {
                if power.remove("powerSaving").is_some() && power.is_empty() {
                    controller.remove("power");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct TestDocument(PathBuf);
    impl TestDocument {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            loop {
                let path = std::env::temp_dir().join(format!(
                    "razer-ui-controller-sleep-{}-{}.json",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                if path.with_extension("json.bak").exists() {
                    continue;
                }
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                {
                    Ok(_) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Could not create test document: {error}"),
                }
            }
        }
    }
    impl Drop for TestDocument {
        fn drop(&mut self) {
            for path in [&self.0, &self.0.with_extension("json.bak")] {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    fn device() -> Device {
        let mut device = razer_model::product_preview::registered_preview(2636).unwrap();
        device.profiles = vec![Profile {
            id: "first".into(),
            guid: "first".into(),
            name: "First".into(),
            settings: None,
            dpi_stages: None,
            source_settings: Some(json!({"profile":{"brightness":41},"controller":{"power":{
                "powerSaving":{"value":15,"isEnabled":true},"extension":"retained"
            }}})),
        }];
        device
    }

    #[test]
    fn controller_sleep_migration_moves_only_the_global_preference() {
        let mut device = device();
        device.profiles.push(device.profiles[0].clone());
        migrate(&mut device).unwrap();
        assert_eq!(
            device.controller_power_saving,
            Some(PowerSaving::new(15, true).unwrap())
        );
        for profile in &device.profiles {
            let settings = profile.source_settings.as_ref().unwrap();
            assert_eq!(settings["profile"]["brightness"], 41);
            assert_eq!(settings["controller"]["power"]["extension"], "retained");
            assert!(settings.pointer("/controller/power/powerSaving").is_none());
        }
        migrate(&mut device).unwrap();
    }

    #[test]
    fn controller_sleep_migration_rejects_conflicts_without_removing_legacy_values() {
        let mut device = device();
        device.controller_power_saving = Some(PowerSaving::new(30, true).unwrap());
        let before = serde_json::to_value(&device).unwrap();
        assert!(migrate(&mut device).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
        device.controller_power_saving = None;
        let mut second = device.profiles[0].clone();
        second.source_settings.as_mut().unwrap()["controller"]["power"]["powerSaving"]["value"] =
            json!(45);
        device.profiles.push(second);
        let before = serde_json::to_value(&device).unwrap();
        assert!(migrate(&mut device).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
    }

    #[test]
    fn controller_sleep_migration_rejects_invalid_shape_and_unrelated_product() {
        let mut device = device();
        device.profiles[0].source_settings.as_mut().unwrap()["controller"]["power"]["powerSaving"]
            ["value"] = json!("15");
        assert!(migrate(&mut device).is_err());
        device.product_id = 2684;
        device.controller_power_saving = Some(PowerSaving::new(15, true).unwrap());
        assert!(migrate(&mut device).is_err());
    }

    #[test]
    fn controller_sleep_migration_preserves_documents_without_legacy_preferences() {
        for settings in [
            json!({}),
            json!({"controller": {}}),
            json!({"controller": {"power": {}}}),
            json!({"controller": {"power": {"extension": []}}}),
            json!({"profile": {"extension": null}}),
        ] {
            let mut device = device();
            device.profiles[0].source_settings = Some(settings);
            let before = serde_json::to_value(&device).unwrap();
            migrate(&mut device).unwrap();
            assert_eq!(serde_json::to_value(&device).unwrap(), before);
        }
        // Other products retain their legacy workspace until their own protocol
        // and migration owner have been established.
        let mut unrelated = device();
        unrelated.product_id = 2684;
        let before = serde_json::to_value(&unrelated).unwrap();
        migrate(&mut unrelated).unwrap();
        assert_eq!(serde_json::to_value(&unrelated).unwrap(), before);
    }

    #[test]
    fn controller_sleep_migration_rejects_malformed_legacy_levels_atomically() {
        for settings in [
            json!(false),
            json!([]),
            json!({"controller": null}),
            json!({"controller": []}),
            json!({"controller": {"power": null}}),
            json!({"controller": {"power": 15}}),
            json!({"controller": {"power": {"powerSaving": null}}}),
            json!({"controller": {"power": {"powerSaving": {"value": 15}}}}),
            json!({"controller": {"power": {"powerSaving": {
                "value": 15, "isEnabled": true, "extension": 42
            }}}}),
            json!({"controller": {"power": {"powerSaving": {
                "value": 1093, "isEnabled": true
            }}}}),
        ] {
            let mut device = device();
            let mut second = device.profiles[0].clone();
            second.source_settings = Some(settings);
            device.profiles.push(second);
            let before = serde_json::to_value(&device).unwrap();
            assert!(migrate(&mut device).is_err());
            assert_eq!(serde_json::to_value(&device).unwrap(), before);
        }
    }

    #[test]
    fn controller_sleep_workspace_loading_is_read_only_and_saving_publishes_migration() {
        use super::super::{WorkspaceFile, read_workspace, write_workspace};

        for product_id in [2636, 2676] {
            for legacy_vector in [true, false] {
                let path = TestDocument::new();
                let mut device = device();
                device.product_id = product_id;
                let original = if legacy_vector {
                    serde_json::to_string(&vec![device]).unwrap()
                } else {
                    serde_json::to_string(&WorkspaceFile::new(vec![device], false)).unwrap()
                };
                std::fs::write(&path.0, &original).unwrap();
                let loaded = read_workspace(&path.0).unwrap().unwrap();
                assert_eq!(std::fs::read_to_string(&path.0).unwrap(), original);
                assert_eq!(
                    loaded.devices[0].controller_power_saving,
                    Some(PowerSaving::new(15, true).unwrap())
                );
                let settings = loaded.devices[0].profiles[0]
                    .source_settings
                    .as_ref()
                    .unwrap();
                assert!(settings.pointer("/controller/power/powerSaving").is_none());
                assert_eq!(settings["controller"]["power"]["extension"], "retained");
                write_workspace(&path.0, &loaded).unwrap();
                assert_eq!(
                    std::fs::read_to_string(path.0.with_extension("json.bak")).unwrap(),
                    original
                );
                let saved = read_workspace(&path.0).unwrap().unwrap();
                assert_eq!(
                    serde_json::to_value(saved.devices).unwrap(),
                    serde_json::to_value(loaded.devices).unwrap()
                );
            }
        }
    }

    #[test]
    fn controller_sleep_workspace_conflicts_preserve_existing_bytes_on_read_and_write() {
        use super::super::{WorkspaceFile, read_workspace, write_workspace};

        let path = TestDocument::new();
        let valid = WorkspaceFile::new(vec![device()], false);
        let mut conflicting = valid.clone();
        conflicting.devices[0].controller_power_saving = Some(PowerSaving::new(30, true).unwrap());
        let original = serde_json::to_string(&valid).unwrap();
        std::fs::write(&path.0, &original).unwrap();
        assert!(write_workspace(&path.0, &conflicting).is_err());
        assert_eq!(std::fs::read_to_string(&path.0).unwrap(), original);
        assert!(!path.0.with_extension("json.bak").exists());

        let original = serde_json::to_string(&conflicting).unwrap();
        std::fs::write(&path.0, &original).unwrap();
        assert!(read_workspace(&path.0).is_err());
        assert!(write_workspace(&path.0, &valid).is_err());
        assert_eq!(std::fs::read_to_string(&path.0).unwrap(), original);
        assert!(!path.0.with_extension("json.bak").exists());
    }
}
