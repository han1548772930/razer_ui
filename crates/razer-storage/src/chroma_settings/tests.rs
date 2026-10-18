use super::*;
use razer_model::chroma_settings::{AppFilter, DashboardGroup};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "razer-chroma-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join(FILE_NAME)
    }
    fn temporary_count(&self) -> usize {
        std::fs::read_dir(&self.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "tmp")
            })
            .count()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_files_are_observed_without_creating_a_saved_configuration() {
    let temp = Temp::new();
    let (document, origin, previous) = load_at(&temp.file()).unwrap();
    assert_eq!(origin, Origin::Missing);
    assert_eq!(document.preferences(), Preferences::default());
    assert!(previous.is_none());
    assert!(!temp.file().exists());
}

#[test]
fn legacy_files_migrate_together_and_preserve_unknown_extensions() {
    let temp = Temp::new();
    let settings = br#"{"auto_start":false,"notifications":false,"future":{"value":7}}"#;
    let dashboard = br#"{"introduction":false,"filter":2,"collapsed":["chroma-device-group","future-group"],"future_dashboard":[1,2]}"#;
    std::fs::write(temp.0.join(LEGACY_SETTINGS), settings).unwrap();
    std::fs::write(temp.0.join(LEGACY_DASHBOARD), dashboard).unwrap();
    let (loaded, origin, _) = load_at(&temp.file()).unwrap();
    assert_eq!(origin, Origin::Legacy);
    assert!(!loaded.preferences().settings().notifications_enabled());
    assert_eq!(loaded.preferences().dashboard().filter(), AppFilter::Games);
    let saved = update_at(&temp.file(), &[Change::Onboard(false)]).unwrap();
    assert!(!saved.settings().auto_start_enabled());
    assert!(!saved.dashboard().introduction_enabled());
    assert!(!saved.dashboard().onboard_enabled());
    assert!(saved.dashboard().is_collapsed(DashboardGroup::Devices));
    let file: Value = serde_json::from_slice(&std::fs::read(temp.file()).unwrap()).unwrap();
    assert_eq!(file["settings"]["future"], serde_json::json!({"value":7}));
    assert_eq!(
        file["dashboard"]["future_dashboard"],
        serde_json::json!([1, 2])
    );
    assert_eq!(file["dashboard"]["filter"], "games");
    assert!(
        file["dashboard"]["collapsed"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("future-group"))
    );
    assert_eq!(
        std::fs::read(temp.0.join(LEGACY_SETTINGS)).unwrap(),
        settings
    );
    assert_eq!(
        std::fs::read(temp.0.join(LEGACY_DASHBOARD)).unwrap(),
        dashboard
    );
    assert_eq!(temp.temporary_count(), 0);
}

#[test]
fn a_patch_uses_the_latest_file_and_retains_other_process_changes() {
    let temp = Temp::new();
    update_at(&temp.file(), &[Change::Notifications(false)]).unwrap();
    let mut file: Value = serde_json::from_slice(&std::fs::read(temp.file()).unwrap()).unwrap();
    file["future"] = serde_json::json!({"retained":true});
    file["settings"]["future_settings"] = serde_json::json!("keep");
    file["settings"]["auto_start"] = serde_json::json!(false);
    std::fs::write(temp.file(), serde_json::to_vec(&file).unwrap()).unwrap();
    let saved = update_at(&temp.file(), &[Change::ShowDisabled(false)]).unwrap();
    assert!(!saved.settings().auto_start_enabled());
    assert!(!saved.settings().notifications_enabled());
    assert!(!saved.dashboard().show_disabled());
    let file: Value = serde_json::from_slice(&std::fs::read(temp.file()).unwrap()).unwrap();
    assert_eq!(file["future"], serde_json::json!({"retained":true}));
    assert_eq!(file["settings"]["future_settings"], "keep");
}

#[test]
fn malformed_known_fields_and_future_versions_are_not_defaulted_or_replaced() {
    let temp = Temp::new();
    for file in [
        r#"{"version":2,"settings":{},"dashboard":{}}"#,
        r#"{"version":1,"settings":{"notifications":"false"},"dashboard":{}}"#,
        r#"{"version":1,"settings":{},"dashboard":{"filter":3}}"#,
        r#"{"version":1,"settings":{}}"#,
        "broken JSON",
    ] {
        std::fs::write(temp.file(), file).unwrap();
        assert!(load_at(&temp.file()).is_err());
        assert!(update_at(&temp.file(), &[Change::Notifications(true)]).is_err());
        assert_eq!(std::fs::read_to_string(temp.file()).unwrap(), file);
        assert_eq!(temp.temporary_count(), 0);
    }
}

#[test]
fn malformed_legacy_data_cannot_be_overwritten_by_a_new_default_file() {
    let temp = Temp::new();
    std::fs::write(temp.0.join(LEGACY_DASHBOARD), br#"{"filter":99}"#).unwrap();
    assert!(load_at(&temp.file()).is_err());
    assert!(update_at(&temp.file(), &[Change::Notifications(true)]).is_err());
    assert!(!temp.file().exists());
}

#[test]
fn conflicting_publication_preserves_the_external_result_and_cleans_its_stage() {
    let temp = Temp::new();
    update_at(&temp.file(), &[Change::Notifications(false)]).unwrap();
    let (mut document, _, previous) = load_at(&temp.file()).unwrap();
    document.apply(&[Change::Notifications(true)]);
    let result = publish(&temp.file(), previous, &document, || {
        std::fs::write(temp.file(), b"external publication")?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(temp.file()).unwrap(), b"external publication");
    assert_eq!(temp.temporary_count(), 0);
}

#[test]
fn preparation_failure_keeps_previous_file_and_another_owners_stage() {
    let temp = Temp::new();
    update_at(&temp.file(), &[Change::AutoStart(false)]).unwrap();
    let (document, _, previous) = load_at(&temp.file()).unwrap();
    let previous = previous.unwrap();
    let other_stage = temp.0.join("other-owner.tmp");
    std::fs::write(&other_stage, b"owned elsewhere").unwrap();
    assert!(
        publish(
            &temp.file(),
            Some(previous.clone()),
            &document,
            || anyhow::bail!("simulated publish failure")
        )
        .is_err()
    );
    assert_eq!(std::fs::read(temp.file()).unwrap(), previous);
    assert_eq!(std::fs::read(other_stage).unwrap(), b"owned elsewhere");
    assert_eq!(temp.temporary_count(), 1);
}

#[test]
fn settings_writer_lock_is_exclusive_and_release_follows_the_owner() {
    let temp = Temp::new();
    let owner = writer_lock(&temp.file()).unwrap();
    let other = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.file().with_extension("json.lock"))
        .unwrap();
    assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
    drop(owner);
    other.try_lock().unwrap();
}

#[test]
fn oversized_settings_are_explicitly_rejected() {
    let temp = Temp::new();
    std::fs::write(temp.file(), vec![b' '; MAX_BYTES + 1]).unwrap();
    assert!(load_at(&temp.file()).is_err());
    assert!(update_at(&temp.file(), &[Change::Onboard(false)]).is_err());
    assert_eq!(
        std::fs::metadata(temp.file()).unwrap().len(),
        MAX_BYTES as u64 + 1
    );
}
