use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-earbuds-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("local.json")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn profile(guid: &str, auto_pause: Option<bool>) -> EarbudsProfile {
    EarbudsProfile {
        guid: guid.into(),
        name: guid.into(),
        audio_auto_pause_control: auto_pause.map(|is_enabled| AutoPauseControl { is_enabled }),
        noise_cancellation: None,
        extra: Map::new(),
    }
}

#[test]
fn default_store_matches_source_default_profile() {
    let store = EarbudsProfileStore::source_default(2657);
    let profile = &store.profiles[0];
    assert_eq!(profile.name, "Default");
    assert_eq!(
        profile.audio_auto_pause_control,
        Some(AutoPauseControl { is_enabled: true })
    );
    assert_eq!(
        profile.noise_cancellation,
        Some(NoiseCancellation {
            is_enabled: true,
            status: 10,
            level: 10
        })
    );
    let other = EarbudsProfileStore::source_default(2660);
    assert!(other.profiles[0].noise_cancellation.is_none());
    assert!(other.profiles[0].audio_auto_pause_control.is_some());
}

#[test]
fn selection_active_guid_first_profile_and_unmatched() {
    let mut store = EarbudsProfileStore {
        schema_version: 1,
        active_guid: None,
        profiles: vec![profile("a", Some(true)), profile("b", Some(false))],
    };
    assert_eq!(store.select(), ProfileSelection::Profile(0));
    store.active_guid = Some("b".into());
    assert_eq!(store.select(), ProfileSelection::Profile(1));
    store.active_guid = Some("zzz".into());
    assert_eq!(store.select(), ProfileSelection::Unmatched("zzz".into()));
    assert!(store.selected().is_err());
    store.active_guid = None;
    store.profiles.clear();
    assert_eq!(store.select(), ProfileSelection::Empty);
}

#[test]
fn anc_update_storage_level_defaults_to_one_and_merges() {
    let mut p = profile("a", None);
    let stored = p
        .set_noise_cancellation(AncChange {
            enabled: Some(false),
            ..AncChange::default()
        })
        .unwrap();
    assert_eq!(
        stored,
        NoiseCancellation {
            is_enabled: false,
            status: 10,
            level: 1
        }
    );
    let stored = p
        .set_noise_cancellation(AncChange {
            enabled: Some(true),
            mode: Some(AncMode::Ambient),
            level: Some(AncLevel::Full),
        })
        .unwrap();
    assert_eq!(
        stored,
        NoiseCancellation {
            is_enabled: true,
            status: 255,
            level: 10
        }
    );
}

#[test]
fn unknown_persisted_anc_values_are_rejected() {
    assert!(
        NoiseCancellation {
            is_enabled: true,
            status: 80,
            level: 10
        }
        .to_setting()
        .is_err()
    );
    assert!(
        NoiseCancellation {
            is_enabled: true,
            status: 10,
            level: 5
        }
        .to_setting()
        .is_err()
    );
}

#[test]
fn load_or_create_only_creates_when_missing_and_preserves_extra() {
    let temp = Temp::new();
    let path = temp.file();
    assert!(load(&path, 2657, "SN1").unwrap().is_none());
    let mut store = load_or_create(&path, 2657, "SN1").unwrap();
    store.profiles[0].set_auto_pause(false);
    store.profiles[0]
        .extra
        .insert("notDisturb".into(), serde_json::json!({"isEnabled":true}));
    save(&path, 2657, "SN1", &store).unwrap();
    let again = load_or_create(&path, 2657, "SN1").unwrap();
    assert_eq!(again, store);
    assert_eq!(again.profiles[0].auto_pause_field().is_enabled, Some(false));
    assert!(load(&path, 2657, "").is_err());
}
