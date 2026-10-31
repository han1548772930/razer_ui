use super::*;
use std::path::PathBuf;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-ui-preferences-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("source-local-storage.json")
    }
    fn legacy(&self, flag: Flag) -> PathBuf {
        self.0.join(format!("{}.json", flag.key()))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_values_keep_source_defaults_without_inventing_files() {
    let temp = Temp::new();
    for flag in [
        Flag::MagneticCalibrationSkipPrompt,
        Flag::HueTutorial,
        Flag::ArmoryIntroduction,
    ] {
        assert_eq!(
            load_at(&temp.path(), flag).unwrap(),
            Observation {
                value: None,
                origin: Origin::Missing
            }
        );
        assert!(!temp.path().exists());
        assert!(!temp.legacy(flag).exists());
    }
    assert!(!Flag::MagneticCalibrationSkipPrompt.default_value());
    assert!(Flag::HueTutorial.default_value());
    assert!(Flag::ArmoryIntroduction.default_value());
}

#[test]
fn legacy_calibration_mount_initializes_its_source_true_value() {
    let temp = Temp::new();
    assert_eq!(
        load_at(&temp.path(), Flag::KeyboardCalibrationIntro).unwrap(),
        Observation {
            value: Some(true),
            origin: Origin::Initialized
        }
    );
    assert_eq!(
        load_at(&temp.path(), Flag::KeyboardCalibrationIntro)
            .unwrap()
            .origin,
        Origin::Current
    );
    assert!(!temp.legacy(Flag::KeyboardCalibrationIntro).exists());
}

#[test]
fn existing_legacy_flags_migrate_once_and_current_values_win() {
    for flag in Flag::ALL {
        if flag == Flag::DisplayNotifications {
            continue;
        }
        let temp = Temp::new();
        std::fs::write(temp.legacy(flag), "false").unwrap();
        assert_eq!(
            load_at(&temp.path(), flag).unwrap(),
            Observation {
                value: Some(false),
                origin: Origin::MigratedLegacy
            }
        );
        assert_eq!(std::fs::read(temp.legacy(flag)).unwrap(), b"false");
        save_at(&temp.path(), flag, true).unwrap();
        assert_eq!(
            load_at(&temp.path(), flag).unwrap(),
            Observation {
                value: Some(true),
                origin: Origin::Current
            }
        );
        std::fs::write(temp.legacy(flag), "broken old file").unwrap();
        assert_eq!(load_at(&temp.path(), flag).unwrap().value(), Some(true));
    }
}

#[test]
fn malformed_legacy_file_is_preserved_and_is_not_missing() {
    let temp = Temp::new();
    std::fs::write(temp.legacy(Flag::HueTutorial), "{broken").unwrap();
    assert!(load_at(&temp.path(), Flag::HueTutorial).is_err());
    assert_eq!(
        std::fs::read(temp.legacy(Flag::HueTutorial)).unwrap(),
        b"{broken"
    );
    assert!(!temp.path().exists());
}

#[test]
fn malformed_current_values_and_outer_documents_survive_load_and_save() {
    for raw in [
        b"invalid document".as_slice(),
        br#"{"isShowTutorialHue":"{broken"}"#,
        br#"{"isShowTutorialHue":"null"}"#,
    ] {
        let temp = Temp::new();
        std::fs::write(temp.path(), raw).unwrap();
        assert!(load_at(&temp.path(), Flag::HueTutorial).is_err());
        assert!(save_at(&temp.path(), Flag::HueTutorial, false).is_err());
        assert_eq!(std::fs::read(temp.path()).unwrap(), raw);
    }
}

#[test]
fn another_writer_wins_migration_without_being_overwritten() {
    let temp = Temp::new();
    save_at(&temp.path(), Flag::ArmoryIntroduction, false).unwrap();
    assert!(!insert_missing(&temp.path(), Flag::ArmoryIntroduction, true).unwrap());
    assert_eq!(
        read_current(&temp.path(), Flag::ArmoryIntroduction).unwrap(),
        Some(false)
    );
}

#[test]
fn independent_flags_and_raw_extension_values_survive_each_write() {
    let temp = Temp::new();
    crate::source_local::set_raw(&temp.path(), "extension", "undefined").unwrap();
    for flag in Flag::ALL {
        save_at(&temp.path(), flag, !flag.default_value()).unwrap();
    }
    for flag in Flag::ALL {
        assert_eq!(
            read_current(&temp.path(), flag).unwrap(),
            Some(!flag.default_value())
        );
    }
    assert_eq!(
        crate::source_local::get_raw(&temp.path(), "extension")
            .unwrap()
            .as_deref(),
        Some("undefined")
    );
}

#[test]
fn failed_publication_preserves_the_valid_previous_document() {
    let temp = Temp::new();
    save_at(&temp.path(), Flag::HueTutorial, true).unwrap();
    let before = std::fs::read(temp.path()).unwrap();
    std::fs::create_dir(temp.path().with_extension("json.bak")).unwrap();
    assert!(save_at(&temp.path(), Flag::HueTutorial, false).is_err());
    assert_eq!(std::fs::read(temp.path()).unwrap(), before);
    assert_eq!(
        read_current(&temp.path(), Flag::HueTutorial).unwrap(),
        Some(true)
    );
    assert!(std::fs::read_dir(&temp.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}

#[test]
fn initial_tab_accepts_every_present_raw_value_without_overwriting_it() {
    let temp = Temp::new();
    for raw in ["false", "null", "undefined", "", "{broken", "true"] {
        crate::source_local::set_raw(&temp.path(), "isInitialTabKeyHit", raw).unwrap();
        let before = std::fs::read(temp.path()).unwrap();
        assert_eq!(record_initial_tab_at(&temp.path()).unwrap(), InitialTabObservation::Present);
        assert_eq!(std::fs::read(temp.path()).unwrap(), before);
    }
}

#[test]
fn initial_tab_records_only_the_missing_key_and_reports_actual_write_failure() {
    let temp = Temp::new();
    crate::source_local::set_raw(&temp.path(), "extension", "unchanged").unwrap();
    let before = std::fs::read(temp.path()).unwrap();
    std::fs::create_dir(temp.path().with_extension("json.bak")).unwrap();
    assert!(matches!(record_initial_tab_at(&temp.path()).unwrap(),
        InitialTabObservation::First { persistence: Err(_) }));
    assert_eq!(std::fs::read(temp.path()).unwrap(), before);
    std::fs::remove_dir(temp.path().with_extension("json.bak")).unwrap();
    assert_eq!(record_initial_tab_at(&temp.path()).unwrap(),
        InitialTabObservation::First { persistence: Ok(()) });
    assert_eq!(crate::source_local::get_raw(&temp.path(), "isInitialTabKeyHit").unwrap().as_deref(), Some("true"));
    assert_eq!(crate::source_local::get_raw(&temp.path(), "extension").unwrap().as_deref(), Some("unchanged"));
    assert_eq!(record_initial_tab_at(&temp.path()).unwrap(), InitialTabObservation::Present);
}

#[test]
fn initial_tab_failed_read_never_writes_or_becomes_a_first_tab() {
    let temp = Temp::new();
    std::fs::write(temp.path(), b"malformed source storage").unwrap();
    let before = std::fs::read(temp.path()).unwrap();
    assert!(record_initial_tab_at(&temp.path()).is_err());
    assert_eq!(std::fs::read(temp.path()).unwrap(), before);
}
