//! Current 1342 nl() publishes the device document's preset library. Profile
//! navigation never owns or initializes that library.
use serde_json::{Value, json};

const LIBRARY: &str = "_audioMixerPresets";

pub(super) fn capture(
    product: u32,
    settings: &mut Option<Value>,
    snapshot: &mut Value,
) -> anyhow::Result<()> {
    if product != 1342 {
        return Ok(());
    }
    anyhow::ensure!(
        settings.as_ref().is_none_or(Value::is_object),
        "Audio Mixer device settings are not an object"
    );
    let Some(library) = snapshot
        .as_object_mut()
        .and_then(|root| root.remove(LIBRARY))
    else {
        return Ok(());
    };
    let settings = settings.get_or_insert_with(|| json!({}));
    if let Some(settings) = settings.as_object_mut() {
        settings.insert(LIBRARY.into(), library);
    }
    Ok(())
}

pub(super) fn migrate_profile(product: u32, settings: &mut Option<Value>, snapshot: &mut Value) {
    if product != 1342
        || settings
            .as_ref()
            .is_some_and(|settings| !settings.is_object())
    {
        return;
    }
    let Some(library) = snapshot
        .as_object_mut()
        .and_then(|root| root.remove(LIBRARY))
    else {
        return;
    };
    let settings = settings.get_or_insert_with(|| json!({}));
    if let Some(settings) = settings.as_object_mut() {
        settings.entry(LIBRARY).or_insert(library);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn absent_library_does_not_create_device_configuration() {
        let mut settings = None;
        let mut snapshot = json!({"device":{"micMonitor":{"value":31}}});
        capture(1342, &mut settings, &mut snapshot).unwrap();
        assert!(settings.is_none());
        assert_eq!(snapshot["device"]["micMonitor"]["value"], 31);
    }

    #[test]
    fn library_has_one_device_owner_and_migration_preserves_that_owner() {
        let library = json!({"presets":[],"active_preset":"","version":7});
        let mut snapshot = json!({"_audioMixerPresets":library,"profile":{"brightness":[]}});
        let mut settings = None;
        capture(1342, &mut settings, &mut snapshot).unwrap();
        assert_eq!(settings.as_ref().unwrap()[LIBRARY], library);
        assert!(snapshot.get(LIBRARY).is_none());
        let mut old = json!({"_audioMixerPresets":{"version":2}});
        migrate_profile(1342, &mut settings, &mut old);
        assert_eq!(settings.as_ref().unwrap()[LIBRARY], library);
        assert!(old.get(LIBRARY).is_none());
    }

    #[test]
    fn malformed_device_settings_are_rejected_without_dropping_the_library() {
        let mut settings = Some(json!([1, 2]));
        let mut snapshot = json!({"_audioMixerPresets":{"version":7}});
        let before = snapshot.clone();
        assert!(capture(1342, &mut settings, &mut snapshot).is_err());
        assert_eq!(snapshot, before);
        assert_eq!(settings, Some(json!([1, 2])));
        migrate_profile(1342, &mut settings, &mut snapshot);
        assert_eq!(snapshot, before);
    }
}
