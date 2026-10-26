//! Queued device drafts merge against their captured local save revision.
use razer_model::model::Device;
use std::collections::BTreeMap;

pub struct DeviceSaveDraft {
    baseline: Vec<Device>,
    proposed: Vec<Device>,
}

impl DeviceSaveDraft {
    pub fn new(baseline: Vec<Device>, proposed: Vec<Device>) -> Self {
        Self { baseline, proposed }
    }

    pub fn has_changes(&self, identity: &str) -> bool {
        let previous = self
            .baseline
            .iter()
            .find(|device| device.workspace_identity() == identity);
        let proposed = self
            .proposed
            .iter()
            .find(|device| device.workspace_identity() == identity);
        match (previous, proposed) {
            (Some(previous), Some(proposed)) => fields(previous) != fields(proposed),
            (None, None) => false,
            _ => true,
        }
    }

    /// Replay accepted edits in queue order after the caller checks the device
    /// owner and connection epoch. Unedited fields keep intervening local saves;
    /// this is not a merge policy for independent external writers.
    pub fn rebase(&self, latest: &[Device]) -> Result<Vec<Device>, String> {
        let baseline = indexed(&self.baseline)?;
        let proposed = indexed(&self.proposed)?;
        let current = indexed(latest)?;
        for identity in baseline.keys() {
            if !proposed.contains_key(identity) {
                return Err(format!("Queued save lost device draft {identity}"));
            }
            if self.has_changes(identity) && !current.contains_key(identity) {
                return Err(format!("Queued save device owner is absent: {identity}"));
            }
        }
        for identity in proposed.keys() {
            if !baseline.contains_key(identity) {
                return Err(format!(
                    "Queued save has no captured device baseline: {identity}"
                ));
            }
        }
        let mut rebased = latest.to_vec();
        for device in &mut rebased {
            let identity = device.workspace_identity();
            let (Some(previous), Some(proposed)) =
                (baseline.get(&identity), proposed.get(&identity))
            else {
                continue;
            };
            merge_profiles(
                &identity,
                &previous.profiles,
                &proposed.profiles,
                &mut device.profiles,
            )?;
            apply_changed(
                &previous.active_profile,
                &proposed.active_profile,
                &mut device.active_profile,
            );
            if proposed.active_profile != previous.active_profile
                && !device
                    .profiles
                    .iter()
                    .any(|profile| profile.id == device.active_profile)
            {
                return Err(format!(
                    "Queued save selected a removed profile for {identity}"
                ));
            }
            merge_device_settings(
                &identity,
                &previous.source_device_settings,
                &proposed.source_device_settings,
                &mut device.source_device_settings,
            )?;
            apply_changed(
                &previous.controller_power_saving,
                &proposed.controller_power_saving,
                &mut device.controller_power_saving,
            );
        }
        Ok(rebased)
    }
}

fn fields(
    device: &Device,
) -> (
    &[razer_model::model::Profile],
    &str,
    &Option<serde_json::Value>,
    &Option<razer_device::controller_sleep::PowerSaving>,
) {
    (
        &device.profiles,
        &device.active_profile,
        &device.source_device_settings,
        &device.controller_power_saving,
    )
}

fn indexed(devices: &[Device]) -> Result<BTreeMap<String, &Device>, String> {
    let mut indexed = BTreeMap::new();
    for device in devices {
        let identity = device.workspace_identity();
        if indexed.insert(identity.clone(), device).is_some() {
            return Err(format!(
                "Queued save has a repeated device owner: {identity}"
            ));
        }
    }
    Ok(indexed)
}

fn apply_changed<T: PartialEq + Clone>(baseline: &T, proposed: &T, latest: &mut T) {
    if proposed != baseline {
        *latest = proposed.clone();
    }
}

/// Vendor documents remain JSON only at this persistence boundary. Replay
/// object-field deltas without replacing unrelated extension fields.
fn merge_source_value(
    baseline: Option<&serde_json::Value>,
    proposed: Option<&serde_json::Value>,
    latest: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    if proposed == baseline {
        return latest.cloned();
    }
    if let Some(serde_json::Value::Object(proposed)) = proposed
        && baseline.is_none_or(serde_json::Value::is_object)
        && latest.is_none_or(serde_json::Value::is_object)
    {
        let baseline = baseline.and_then(serde_json::Value::as_object);
        let mut merged = latest
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        let keys = proposed
            .keys()
            .chain(baseline.into_iter().flat_map(|fields| fields.keys()))
            .collect::<std::collections::BTreeSet<_>>();
        for key in keys {
            match merge_source_value(
                baseline.and_then(|fields| fields.get(key)),
                proposed.get(key),
                merged.get(key),
            ) {
                Some(value) => {
                    merged.insert(key.clone(), value);
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        return Some(serde_json::Value::Object(merged));
    }
    proposed.cloned()
}

fn merge_device_settings(
    identity: &str,
    baseline: &Option<serde_json::Value>,
    proposed: &Option<serde_json::Value>,
    latest: &mut Option<serde_json::Value>,
) -> Result<(), String> {
    let fields =
        |value: &Option<serde_json::Value>| value.as_ref().is_none_or(serde_json::Value::is_object);
    if !fields(baseline) || !fields(proposed) || !fields(latest) {
        return Err(format!(
            "Queued save has an invalid device settings document for {identity}"
        ));
    }
    let mut before = baseline.clone();
    let mut after = proposed.clone();
    for document in [&mut before, &mut after] {
        if let Some(fields) = document.as_mut().and_then(serde_json::Value::as_object_mut) {
            fields.retain(|field, _| {
                !razer_device::mapping_document::MappingProduct::owns_storage_field(field)
            });
        }
    }
    if let Some(fields) = before.as_ref().and_then(serde_json::Value::as_object) {
        if fields.is_empty() {
            before = None;
        }
    }
    if let Some(fields) = after.as_ref().and_then(serde_json::Value::as_object) {
        if fields.is_empty() {
            after = None;
        }
    }
    // Compiled projections belong to their compiler. Keep the last saved
    // projection as compiler input; the caller regenerates it after replay.
    let projections = latest
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .map(|fields| {
            fields
                .iter()
                .filter(|(field, _)| {
                    razer_device::mapping_document::MappingProduct::owns_storage_field(field)
                })
                .map(|(field, value)| (field.clone(), value.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    *latest = merge_source_value(before.as_ref(), after.as_ref(), latest.as_ref());
    if !projections.is_empty() {
        let fields = latest
            .get_or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .expect("validated device settings object");
        fields.extend(projections);
    }
    Ok(())
}

fn merge_profile_settings(
    baseline: &Option<razer_model::settings::ProfileSettings>,
    proposed: &Option<razer_model::settings::ProfileSettings>,
    latest: &mut Option<razer_model::settings::ProfileSettings>,
) {
    let (Some(before), Some(after), Some(current)) = (baseline, proposed, latest.as_mut()) else {
        apply_changed(baseline, proposed, latest);
        return;
    };
    let razer_model::settings::ProfileSettings {
        audio,
        mic,
        playback_enabled,
        volume,
        tracking,
        sensitivity,
        polling,
        polling_wired,
        polling_wireless,
        idle_minutes,
        power_enabled,
        low_power,
        lighting,
        keyboard,
        bindings,
        hypershift_bindings,
        linked_games,
    } = after;
    apply_changed(&before.audio, audio, &mut current.audio);
    apply_changed(&before.mic, mic, &mut current.mic);
    apply_changed(
        &before.playback_enabled,
        playback_enabled,
        &mut current.playback_enabled,
    );
    apply_changed(&before.volume, volume, &mut current.volume);
    apply_changed(&before.tracking, tracking, &mut current.tracking);
    apply_changed(&before.sensitivity, sensitivity, &mut current.sensitivity);
    apply_changed(&before.polling, polling, &mut current.polling);
    apply_changed(
        &before.polling_wired,
        polling_wired,
        &mut current.polling_wired,
    );
    apply_changed(
        &before.polling_wireless,
        polling_wireless,
        &mut current.polling_wireless,
    );
    apply_changed(
        &before.idle_minutes,
        idle_minutes,
        &mut current.idle_minutes,
    );
    apply_changed(
        &before.power_enabled,
        power_enabled,
        &mut current.power_enabled,
    );
    apply_changed(&before.low_power, low_power, &mut current.low_power);
    apply_changed(&before.lighting, lighting, &mut current.lighting);
    apply_changed(&before.keyboard, keyboard, &mut current.keyboard);
    apply_changed(&before.bindings, bindings, &mut current.bindings);
    apply_changed(
        &before.hypershift_bindings,
        hypershift_bindings,
        &mut current.hypershift_bindings,
    );
    apply_changed(
        &before.linked_games,
        linked_games,
        &mut current.linked_games,
    );
}

fn merge_profiles(
    identity: &str,
    baseline: &[razer_model::model::Profile],
    proposed: &[razer_model::model::Profile],
    latest: &mut Vec<razer_model::model::Profile>,
) -> Result<(), String> {
    fn indexed<'a>(
        identity: &str,
        profiles: &'a [razer_model::model::Profile],
    ) -> Result<BTreeMap<&'a str, &'a razer_model::model::Profile>, String> {
        let mut indexed = BTreeMap::new();
        for profile in profiles {
            if profile.id.is_empty() || indexed.insert(profile.id.as_str(), profile).is_some() {
                return Err(format!(
                    "Queued save has an invalid profile identity for {identity}"
                ));
            }
        }
        Ok(indexed)
    }
    let before = indexed(identity, baseline)?;
    let after = indexed(identity, proposed)?;
    indexed(identity, latest)?;
    if baseline == proposed {
        return Ok(());
    }
    if latest == baseline || latest == proposed {
        *latest = proposed.to_vec();
        return Ok(());
    }
    for previous in baseline {
        let current_index = latest.iter().position(|profile| profile.id == previous.id);
        match (after.get(previous.id.as_str()), current_index) {
            (Some(proposed), _) if **proposed == *previous => (),
            (None, None) => (),
            (None, Some(index)) if latest[index].guid == previous.guid => {
                latest.remove(index);
            }
            (None, Some(_)) => {
                return Err(format!(
                    "Queued save cannot delete a replaced profile identity for {identity}"
                ));
            }
            (Some(_), None) => {
                return Err(format!(
                    "Queued save edited a removed profile for {identity}"
                ));
            }
            (Some(proposed), Some(index)) => {
                let current = &mut latest[index];
                let owner = format!("{identity}/{}", previous.id);
                if current.guid != previous.guid && current.guid != proposed.guid {
                    return Err(format!(
                        "Queued save profile identity was replaced for {owner}"
                    ));
                }
                apply_changed(&previous.guid, &proposed.guid, &mut current.guid);
                apply_changed(&previous.name, &proposed.name, &mut current.name);
                current.source_settings = merge_source_value(
                    previous.source_settings.as_ref(),
                    proposed.source_settings.as_ref(),
                    current.source_settings.as_ref(),
                );
                merge_profile_settings(
                    &previous.settings,
                    &proposed.settings,
                    &mut current.settings,
                );
                apply_changed(
                    &previous.dpi_stages,
                    &proposed.dpi_stages,
                    &mut current.dpi_stages,
                );
            }
        }
    }
    for profile in proposed
        .iter()
        .filter(|profile| !before.contains_key(profile.id.as_str()))
    {
        match latest.iter().find(|current| current.id == profile.id) {
            Some(current) if current == profile => (),
            Some(_) => {
                return Err(format!(
                    "Queued save has a conflicting new profile for {identity}"
                ));
            }
            None => latest.push(profile.clone()),
        }
    }
    let before_order = baseline
        .iter()
        .filter(|profile| after.contains_key(profile.id.as_str()))
        .map(|profile| profile.id.as_str())
        .collect::<Vec<_>>();
    let after_order = proposed
        .iter()
        .filter(|profile| before.contains_key(profile.id.as_str()))
        .map(|profile| profile.id.as_str())
        .collect::<Vec<_>>();
    if before_order != after_order {
        let mut remaining = std::mem::take(latest);
        for profile in proposed {
            if let Some(index) = remaining
                .iter()
                .position(|current| current.id == profile.id)
            {
                latest.push(remaining.remove(index));
            }
        }
        latest.extend(remaining);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device_for_product(pid: u32) -> Device {
        let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
        device.active_profile = "test-profile".into();
        device.profiles = vec![razer_model::model::Profile {
            id: device.active_profile.clone(),
            guid: device.active_profile.clone(),
            name: "Test profile".into(),
            settings: None,
            source_settings: None,
            dpi_stages: None,
        }];
        device.source_device_settings = Some(serde_json::json!({"version": 1}));
        device
    }

    fn device() -> Device {
        device_for_product(756)
    }

    #[test]
    fn queued_controller_sleep_save_changes_only_its_device_global_field() {
        use razer_device::controller_sleep::PowerSaving;
        let previous = device_for_product(2636);
        let mut proposed = previous.clone();
        proposed.controller_power_saving = Some(PowerSaving::new(15, true).unwrap());
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed]);
        assert!(draft.has_changes(&previous.workspace_identity()));
        let mut latest = previous.clone();
        latest.profiles[0].name = "Saved intervening edit".into();
        latest.source_device_settings = Some(serde_json::json!({"extension": 42}));
        let actual = draft.rebase(&[latest.clone()]).unwrap().remove(0);
        assert_eq!(
            actual.controller_power_saving,
            Some(PowerSaving::new(15, true).unwrap())
        );
        assert_eq!(actual.profiles, latest.profiles);
        assert_eq!(actual.source_device_settings, latest.source_device_settings);
        let unchanged = DeviceSaveDraft::new(vec![previous.clone()], vec![previous]);
        assert_eq!(
            unchanged.rebase(&[actual.clone()]).unwrap()[0].controller_power_saving,
            actual.controller_power_saving
        );
    }

    #[test]
    fn queued_unchanged_draft_preserves_a_preceding_import_or_reset() {
        let previous = device();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![previous.clone()]);
        let mut latest = previous.clone();
        let mut imported = latest.profiles[0].clone();
        imported.id = "imported".into();
        imported.guid = "imported".into();
        latest.profiles.push(imported);
        latest.active_profile = "imported".into();
        latest.source_device_settings = Some(serde_json::json!({"version": 2, "extension": 42}));
        let rebased = draft.rebase(&[latest.clone()]).unwrap();
        assert_eq!(fields(&rebased[0]), fields(&latest));
        assert!(!draft.has_changes(&previous.workspace_identity()));
    }

    #[test]
    fn queued_profile_edit_keeps_a_newer_independent_device_document() {
        let previous = device();
        let mut proposed = previous.clone();
        proposed.profiles[0].name = "User edit".into();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed.clone()]);
        let mut latest = previous;
        latest.source_device_settings = Some(serde_json::json!({"version": 2}));
        latest
            .product_name
            .values
            .insert("en".into(), "Current name".into());
        let rebased = draft.rebase(&[latest.clone()]).unwrap();
        assert_eq!(rebased[0].profiles, proposed.profiles);
        assert_eq!(
            rebased[0].source_device_settings,
            latest.source_device_settings
        );
        assert_eq!(rebased[0].product_name.get("en"), "Current name");
        assert!(draft.has_changes(&latest.workspace_identity()));
    }

    #[test]
    fn replaced_profile_identities_and_removed_edited_owners_are_rejected() {
        let previous = device();
        let mut proposed = previous.clone();
        proposed.profiles[0].name = "Captured edit".into();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed]);
        let mut latest = previous;
        latest.profiles[0].guid = "Reused local ID".into();
        let before = latest.profiles.clone();
        assert!(
            draft
                .rebase(&[latest.clone()])
                .unwrap_err()
                .contains("identity was replaced")
        );
        assert_eq!(latest.profiles, before);
        assert!(draft.rebase(&[]).unwrap_err().contains("owner is absent"));
    }

    #[test]
    fn queued_device_set_tracks_current_owners_without_resurrecting_removed_ones() {
        let previous = device();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![previous]);
        let mut discovered = device();
        discovered.device_container_id = "new-device".into();
        let rebased = draft.rebase(&[discovered.clone()]).unwrap();
        assert_eq!(rebased.len(), 1);
        assert_eq!(
            rebased[0].workspace_identity(),
            discovered.workspace_identity()
        );
        assert!(draft.rebase(&[discovered.clone(), discovered]).is_err());
    }

    #[test]
    fn queued_profile_edit_preserves_an_import_appended_during_the_previous_write() {
        let previous = device();
        let mut proposed = previous.clone();
        proposed.profiles[0].name = "Edited while importing".into();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed.clone()]);
        let mut latest = previous.clone();
        let mut imported = previous.profiles[0].clone();
        imported.id = "imported".into();
        imported.guid = "imported".into();
        latest.profiles.push(imported.clone());
        latest.source_device_settings = Some(serde_json::json!({"importedProjection": 42}));
        let rebased = draft.rebase(&[latest.clone()]).unwrap();
        assert_eq!(
            rebased[0].profiles,
            vec![proposed.profiles[0].clone(), imported]
        );
        assert_eq!(rebased[0].active_profile, previous.active_profile);
        assert_eq!(
            rebased[0].source_device_settings,
            latest.source_device_settings
        );
    }

    #[test]
    fn independent_fields_of_the_same_profile_merge_without_reverting_a_saved_setting() {
        let previous = device();
        let mut proposed = previous.clone();
        proposed.profiles[0].name = "Renamed".into();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed.clone()]);
        let mut latest = previous;
        latest.profiles[0].source_settings = Some(serde_json::json!({"independentSetting": 42}));
        let rebased = draft.rebase(&[latest.clone()]).unwrap();
        assert_eq!(rebased[0].profiles[0].name, "Renamed");
        assert_eq!(
            rebased[0].profiles[0].source_settings,
            latest.profiles[0].source_settings
        );
    }

    #[test]
    fn removing_a_profile_follows_queue_order_and_rejects_a_reused_identity() {
        let mut previous = device();
        let mut second = previous.profiles[0].clone();
        second.id = "removed".into();
        second.guid = "removed".into();
        previous.profiles.push(second);
        let mut proposed = previous.clone();
        proposed.profiles.pop();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed]);
        let mut latest = previous;
        let mut imported = latest.profiles[0].clone();
        imported.id = "imported".into();
        imported.guid = "imported".into();
        latest.profiles.push(imported);
        let rebased = draft.rebase(&[latest.clone()]).unwrap();
        assert_eq!(
            rebased[0]
                .profiles
                .iter()
                .map(|profile| profile.id.as_str())
                .collect::<Vec<_>>(),
            vec![latest.profiles[0].id.as_str(), "imported"]
        );
        latest.profiles[1].name = "Edited before deletion receipt".into();
        let rebased = draft.rebase(&[latest.clone()]).unwrap();
        assert!(
            !rebased[0]
                .profiles
                .iter()
                .any(|profile| profile.id == "removed")
        );
        latest.profiles[1].guid = "Reused local ID".into();
        assert!(
            draft
                .rebase(&[latest])
                .unwrap_err()
                .contains("replaced profile identity")
        );
    }

    #[test]
    fn successive_accepted_edits_to_the_same_field_follow_queue_order() {
        let previous = device();
        let mut first = previous.clone();
        first.profiles[0].name = "First edit".into();
        let first_draft = DeviceSaveDraft::new(vec![previous.clone()], vec![first]);
        let mut second = previous.clone();
        second.profiles[0].name = "Second edit".into();
        let second_draft = DeviceSaveDraft::new(vec![previous.clone()], vec![second]);
        let first_saved = first_draft.rebase(&[previous]).unwrap();
        let second_saved = second_draft.rebase(&first_saved).unwrap();
        assert_eq!(first_saved[0].profiles[0].name, "First edit");
        assert_eq!(second_saved[0].profiles[0].name, "Second edit");
    }

    #[test]
    fn source_and_typed_setting_deltas_preserve_unedited_capabilities() {
        let mut previous = device();
        previous.profiles[0].source_settings = Some(serde_json::json!({"x": 0, "y": 0}));
        previous.profiles[0].settings =
            Some(razer_model::settings::ProfileSettings::for_product(756));
        let mut proposed = previous.clone();
        proposed.profiles[0].source_settings.as_mut().unwrap()["y"] = 2.into();
        proposed.profiles[0].settings.as_mut().unwrap().volume = 29;
        let mut latest = previous.clone();
        latest.profiles[0].source_settings.as_mut().unwrap()["x"] = 1.into();
        latest.profiles[0].settings.as_mut().unwrap().idle_minutes = 8;
        let rebased = DeviceSaveDraft::new(vec![previous], vec![proposed])
            .rebase(&[latest])
            .unwrap();
        assert_eq!(
            rebased[0].profiles[0].source_settings,
            Some(serde_json::json!({"x": 1, "y": 2}))
        );
        let settings = rebased[0].profiles[0].settings.as_ref().unwrap();
        assert_eq!(settings.volume, 29);
        assert_eq!(settings.idle_minutes, 8);
    }

    #[test]
    fn selection_during_import_preserves_appended_profiles_and_latest_compiler_input() {
        for product_id in [70, 182, 756] {
            let mut previous = device_for_product(product_id);
            previous.source_device_settings = None;
            let mut second = previous.profiles[0].clone();
            second.id = "selected".into();
            second.guid = "selected".into();
            previous.profiles.push(second);
            let mut proposed = previous.clone();
            proposed.active_profile = "selected".into();
            proposed.source_device_settings =
                Some(serde_json::json!({"activeProfile": "selected"}));
            let mut latest = previous.clone();
            let mut imported = previous.profiles[0].clone();
            imported.id = "imported".into();
            imported.guid = "imported".into();
            latest.profiles.push(imported);
            latest.source_device_settings = Some(serde_json::json!({"extension": 42}));
            if let Some(product) =
                razer_device::mapping_document::MappingProduct::for_product_id(product_id)
            {
                latest.source_device_settings.as_mut().unwrap()[product.storage_field()] =
                    serde_json::json!({"profiles": [{"guid": "imported"}], "version": 3});
            }
            let rebased = DeviceSaveDraft::new(vec![previous], vec![proposed])
                .rebase(&[latest.clone()])
                .unwrap();
            assert_eq!(rebased[0].profiles, latest.profiles);
            assert_eq!(rebased[0].active_profile, "selected");
            let root = rebased[0].source_device_settings.as_ref().unwrap();
            assert_eq!(root["activeProfile"], "selected");
            assert_eq!(root["extension"], 42);
            if let Some(product) =
                razer_device::mapping_document::MappingProduct::for_product_id(product_id)
            {
                assert_eq!(
                    root[product.storage_field()],
                    latest.source_device_settings.as_ref().unwrap()[product.storage_field()]
                );
            }
        }
    }

    #[test]
    fn removed_or_duplicate_profile_selection_is_not_resurrected() {
        let mut previous = device();
        let mut second = previous.profiles[0].clone();
        second.id = "selected".into();
        second.guid = "selected".into();
        previous.profiles.push(second);
        let mut proposed = previous.clone();
        proposed.active_profile = "selected".into();
        let draft = DeviceSaveDraft::new(vec![previous.clone()], vec![proposed]);
        let mut latest = previous.clone();
        latest.profiles.pop();
        assert!(
            draft
                .rebase(&[latest])
                .unwrap_err()
                .contains("selected a removed profile")
        );
        latest = previous;
        latest.profiles.push(latest.profiles[0].clone());
        assert!(
            draft
                .rebase(&[latest])
                .unwrap_err()
                .contains("invalid profile identity")
        );
    }
}
