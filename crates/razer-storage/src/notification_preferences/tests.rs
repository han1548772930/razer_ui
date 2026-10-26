use super::*;
use crate::{
    source_local,
    ui_preferences::{self, Flag},
};

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-notification-preferences-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn workspace(&self) -> std::path::PathBuf {
        self.0.join("profiles.json")
    }
    fn path(&self) -> std::path::PathBuf {
        source_local::beside_workspace(&self.workspace())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn notification_mount_observes_missing_without_claiming_publication() {
    let temp = Temp::new();
    assert_eq!(
        ui_preferences::load_at(&temp.path(), Flag::DisplayNotifications)
            .unwrap()
            .value(),
        None
    );
    assert!(!temp.path().exists());
    save_at_with(&temp.path(), true, 17, |path, value| {
        source_local::set(path, DISPLAY_KEY, &value.into())
    })
    .unwrap();
    assert_eq!(
        source_local::get(&temp.path(), DISPLAY_KEY).unwrap(),
        Some(true.into())
    );
    assert_eq!(source_local::get_raw(&temp.path(), APPS_KEY).unwrap(), None);
}

#[test]
fn notification_writer_updates_only_matching_app_and_preserves_extensions() {
    let temp = Temp::new();
    let apps = serde_json::json!([
        {"name":"synapse", "displayNotifications":true, "lastUpdated":1, "install":{"version":"4"}, "extra":[2,3]},
        {"name":"chroma", "displayNotifications":true, "lastUpdated":7}
    ]);
    source_local::set(&temp.path(), APPS_KEY, &apps).unwrap();
    source_local::set_raw(&temp.path(), "notifications", "undefined").unwrap();
    save_at_with(&temp.path(), false, 42, |path, value| {
        let current = source_local::get(path, APPS_KEY)?.unwrap();
        assert_eq!(current[0][DISPLAY_KEY], false);
        assert_eq!(
            source_local::get_raw(path, DISPLAY_KEY)?,
            None,
            "app commit precedes key"
        );
        source_local::set(path, DISPLAY_KEY, &value.into())
    })
    .unwrap();
    let result = source_local::get(&temp.path(), APPS_KEY).unwrap().unwrap();
    assert_eq!(result[0]["lastUpdated"], 42);
    assert_eq!(result[0]["install"], apps[0]["install"]);
    assert_eq!(result[0]["extra"], apps[0]["extra"]);
    assert_eq!(result[1], apps[1]);
    assert_eq!(
        source_local::get_raw(&temp.path(), "notifications")
            .unwrap()
            .as_deref(),
        Some("undefined")
    );
}

#[test]
fn notification_key_failure_keeps_the_successful_metadata_commit() {
    let temp = Temp::new();
    source_local::set(
        &temp.path(),
        APPS_KEY,
        &serde_json::json!([{"name":"synapse"}]),
    )
    .unwrap();
    source_local::set(&temp.path(), DISPLAY_KEY, &true.into()).unwrap();
    let error = save_at_with(&temp.path(), false, 55, |_, _| {
        anyhow::bail!("injected key publication failure")
    })
    .unwrap_err();
    assert!(format!("{error:#}").contains("injected key"));
    assert_eq!(
        source_local::get(&temp.path(), APPS_KEY).unwrap().unwrap()[0][DISPLAY_KEY],
        false
    );
    assert_eq!(
        source_local::get(&temp.path(), DISPLAY_KEY).unwrap(),
        Some(true.into())
    );
}

#[test]
fn notification_metadata_failure_prevents_key_write_and_preserves_bytes() {
    let temp = Temp::new();
    source_local::set_raw(&temp.path(), APPS_KEY, "{broken").unwrap();
    source_local::set(&temp.path(), DISPLAY_KEY, &false.into()).unwrap();
    let before = std::fs::read(temp.path()).unwrap();
    assert!(
        save_at_with(&temp.path(), true, 1, |_, _| panic!(
            "key must not be reached"
        ))
        .is_err()
    );
    assert_eq!(std::fs::read(temp.path()).unwrap(), before);
}

#[test]
fn notification_non_boolean_values_never_become_confirmed_visibility() {
    assert_eq!(visibility_decision(None), (Some(true), true));
    assert_eq!(visibility_decision(Some("null")), (Some(true), true));
    assert_eq!(visibility_decision(Some("true")), (Some(true), false));
    assert_eq!(visibility_decision(Some("false")), (Some(false), false));
    for raw in ["0", "1", "\"\"", "\"true\"", "[]", "{}", "undefined", ""] {
        assert_eq!(visibility_decision(Some(raw)), (None, false), "{raw}");
    }
}

#[test]
fn notification_visibility_initializes_only_missing_or_null_and_preserves_invalid_values() {
    for raw in [
        None,
        Some("null"),
        Some("false"),
        Some("0"),
        Some("undefined"),
    ] {
        let temp = Temp::new();
        if let Some(raw) = raw {
            source_local::set_raw(&temp.path(), DISPLAY_KEY, raw).unwrap();
        }
        let expected = visibility_decision(raw);
        assert_eq!(visibility_at(&temp.path()).unwrap(), expected.0);
        assert_eq!(
            source_local::get_raw(&temp.path(), DISPLAY_KEY)
                .unwrap()
                .as_deref(),
            if expected.1 { Some("true") } else { raw }
        );
        if matches!(raw, Some("0" | "undefined")) {
            assert!(ui_preferences::load_at(&temp.path(), Flag::DisplayNotifications).is_err());
        }
    }
}

#[test]
fn notification_legacy_workspace_migrates_once_and_current_value_wins() {
    let temp = Temp::new();
    let mut original = crate::workspace::WorkspaceFile::new(vec![], false);
    original.preferences.legacy_notifications = Some(false);
    std::fs::write(temp.workspace(), serde_json::to_vec(&original).unwrap()).unwrap();
    let mut loaded = crate::workspace::read_workspace(&temp.workspace())
        .unwrap()
        .unwrap();
    assert_eq!(loaded.preferences.legacy_notifications, None);
    assert_eq!(
        source_local::get(&temp.path(), DISPLAY_KEY).unwrap(),
        Some(false.into())
    );
    source_local::set(&temp.path(), DISPLAY_KEY, &true.into()).unwrap();
    loaded = crate::workspace::read_workspace(&temp.workspace())
        .unwrap()
        .unwrap();
    assert_eq!(
        source_local::get(&temp.path(), DISPLAY_KEY).unwrap(),
        Some(true.into())
    );
    crate::workspace::write_workspace(&temp.workspace(), &loaded).unwrap();
    let persisted: serde_json::Value =
        serde_json::from_slice(&std::fs::read(temp.workspace()).unwrap()).unwrap();
    assert!(persisted["preferences"].get("notifications").is_none());
}

#[test]
fn notification_legacy_migration_cannot_overwrite_corrupt_shared_data() {
    let temp = Temp::new();
    source_local::set_raw(&temp.path(), DISPLAY_KEY, "undefined").unwrap();
    let mut original = crate::workspace::WorkspaceFile::new(vec![], false);
    original.preferences.legacy_notifications = Some(false);
    let before = serde_json::to_vec(&original).unwrap();
    std::fs::write(temp.workspace(), &before).unwrap();
    assert!(crate::workspace::read_workspace(&temp.workspace()).is_err());
    assert_eq!(std::fs::read(temp.workspace()).unwrap(), before);
    assert_eq!(
        source_local::get_raw(&temp.path(), DISPLAY_KEY)
            .unwrap()
            .as_deref(),
        Some("undefined")
    );
}
