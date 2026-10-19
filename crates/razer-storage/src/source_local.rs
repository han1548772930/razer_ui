//! Reconstructed local preferences use original localStorage keys and strings.
//! This file is local to this app; it is neither Chromium storage nor DLL data.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions, TryLockError},
    path::Path,
    time::Duration,
};

type Entries = BTreeMap<String, String>;
fn write_lock(path: &Path) -> anyhow::Result<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("json.lock"))?;
    for _ in 0..100 {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    anyhow::bail!("source local storage write lock timed out")
}
fn entries(path: &Path) -> anyhow::Result<Entries> {
    Ok(
        crate::read_document(path, |text| Ok(serde_json::from_str::<Entries>(text)?))?
            .unwrap_or_default(),
    )
}
pub fn path() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::store_path()?.with_file_name("source-local-storage.json"))
}
/// Original helper get(key) calls JSON.parse(localStorage.getItem(key)).
pub fn get(path: &Path, key: &str) -> anyhow::Result<Option<Value>> {
    entries(path)?
        .get(key)
        .map(|raw| serde_json::from_str(raw).map_err(Into::into))
        .transpose()
}
pub fn get_raw(path: &Path, key: &str) -> anyhow::Result<Option<String>> {
    Ok(entries(path)?.get(key).cloned())
}
/// Original helper set(key,value) serializes the value into a string first.
pub fn set(path: &Path, key: &str, value: &Value) -> anyhow::Result<()> {
    set_raw(path, key, &serde_json::to_string(value)?)
}

/// Preserve the exact string accepted by localStorage.setItem, including a
/// source caller's JSON.stringify(undefined) -> "undefined" coercion.
pub fn set_raw(path: &Path, key: &str, value: &str) -> anyhow::Result<()> {
    let _lock = write_lock(path)?;
    let previous = entries(path)?;
    let mut values = previous.clone();
    values.insert(key.to_owned(), value.to_owned());
    crate::write_document(path, &values, |text| {
        anyhow::ensure!(
            serde_json::from_str::<Entries>(text)? == previous,
            "source local storage changed after it was read; reread before saving"
        );
        Ok(())
    })
}

pub fn update(
    path: &Path,
    key: &str,
    mut transform: impl FnMut(Option<Value>) -> anyhow::Result<Option<Value>>,
) -> anyhow::Result<bool> {
    let _lock = write_lock(path)?;
    for _ in 0..8 {
        let previous = entries(path)?;
        let old = previous
            .get(key)
            .map(|raw| serde_json::from_str(raw))
            .transpose()?;
        let Some(value) = transform(old)? else {
            return Ok(false);
        };
        let mut values = previous.clone();
        values.insert(key.to_owned(), serde_json::to_string(&value)?);
        let result = crate::write_document(path, &values, |text| {
            anyhow::ensure!(
                serde_json::from_str::<Entries>(text)? == previous,
                "source local storage changed after it was read; reread before saving"
            );
            Ok(())
        });
        match result {
            Ok(()) => return Ok(true),
            Err(error) if entries(path)? == previous => return Err(error),
            Err(_) => {}
        }
    }
    anyhow::bail!("source local storage remained contested after eight rereads")
}

/// Compare and remove under the same process/file writer lock. A completed
/// older restore must never remove the journal left by a newer restore.
pub fn remove_raw_if(
    path: &Path,
    key: &str,
    predicate: impl FnOnce(&str) -> anyhow::Result<bool>,
) -> anyhow::Result<bool> {
    let _lock = write_lock(path)?;
    let previous = entries(path)?;
    let Some(raw) = previous.get(key) else {
        return Ok(false);
    };
    if !predicate(raw)? {
        return Ok(false);
    }
    let mut values = previous.clone();
    values.remove(key);
    crate::write_document(path, &values, |text| {
        anyhow::ensure!(
            serde_json::from_str::<Entries>(text)? == previous,
            "source local storage changed after it was read; reread before removing"
        );
        Ok(())
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "razer-source-local-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> std::path::PathBuf {
            self.0.join("storage.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn source_localstorage_raw_coercion_survives_other_json_writes() {
        let temp = Temp::new();
        let path = temp.file();
        set_raw(&path, "notifications", "undefined").unwrap();
        set(&path, "displayNotifications", &Value::Bool(false)).unwrap();
        assert_eq!(
            get_raw(&path, "notifications").unwrap().as_deref(),
            Some("undefined")
        );
        assert!(get(&path, "notifications").is_err());
        assert_eq!(
            get(&path, "displayNotifications").unwrap(),
            Some(Value::Bool(false))
        );
        set_raw(&path, "notifications", "").unwrap();
        assert_eq!(
            get_raw(&path, "notifications").unwrap().as_deref(),
            Some("")
        );
    }

    #[test]
    fn source_identity_update_retries_changed_snapshot_and_preserves_other_keys() {
        let temp = Temp::new();
        let path = temp.file();
        set(&path, "connectedDeviceInfo", &serde_json::json!([])).unwrap();
        let mut calls = 0;
        assert!(
            update(&path, "connectedDeviceInfo", |old| {
                calls += 1;
                if calls == 1 {
                    let mut external = entries(&path)?;
                    external.insert(
                        "another".into(),
                        serde_json::json!({"retained":true}).to_string(),
                    );
                    std::fs::write(&path, serde_json::to_vec(&external)?)?;
                }
                let mut rows = old.unwrap().as_array().unwrap().clone();
                rows.push(serde_json::json!({"productId":691,"serialNumber":"observed"}));
                Ok(Some(Value::Array(rows)))
            })
            .unwrap()
        );
        assert_eq!(calls, 2);
        assert_eq!(
            get(&path, "connectedDeviceInfo")
                .unwrap()
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            get(&path, "another").unwrap(),
            Some(serde_json::json!({"retained":true}))
        );
        let before = std::fs::read(&path).unwrap();
        assert!(!update(&path, "connectedDeviceInfo", |_| Ok(None)).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn source_identity_update_rejects_invalid_source_json_without_replacing_it() {
        let temp = Temp::new();
        let path = temp.file();
        std::fs::write(&path, br#"{"connectedDeviceInfo":"invalid JSON"}"#).unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(
            update(&path, "connectedDeviceInfo", |_| Ok(Some(
                serde_json::json!([])
            )))
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn source_identity_writer_lock_is_exclusive_and_released_by_owner() {
        let temp = Temp::new();
        let path = temp.file();
        let owner = write_lock(&path).unwrap();
        let other = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.with_extension("json.lock"))
            .unwrap();
        assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
        drop(owner);
        other.try_lock().unwrap();
    }
    #[test]
    fn dismissal_round_trip_preserves_other_raw_entries() {
        let temp = Temp::new();
        let path = temp.file();
        assert_eq!(
            get(&path, "1453_OnboardingTutorialDismissed").unwrap(),
            None
        );
        let preference =
            serde_json::json!({"show_sensitivity_clutch_warning":true,"unknown":{"text":"原值"}});
        set(&path, "synapse_691", &preference).unwrap();
        let original = get_raw(&path, "synapse_691").unwrap();
        set(
            &path,
            "1453_OnboardingTutorialDismissed",
            &Value::Bool(true),
        )
        .unwrap();
        assert_eq!(
            get_raw(&path, "1453_OnboardingTutorialDismissed")
                .unwrap()
                .as_deref(),
            Some("true")
        );
        assert_eq!(
            get(&path, "1453_OnboardingTutorialDismissed").unwrap(),
            Some(Value::Bool(true))
        );
        assert_eq!(get_raw(&path, "synapse_691").unwrap(), original);
        assert_eq!(get(&path, "synapse_691").unwrap(), Some(preference));
    }
    #[test]
    fn malformed_outer_file_is_preserved_and_not_observed_as_missing() {
        let temp = Temp::new();
        let path = temp.file();
        std::fs::write(&path, b"corrupt document").unwrap();
        assert!(get(&path, "missing").is_err());
        assert!(set(&path, "new", &Value::Bool(true)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt document");
    }
    #[test]
    fn malformed_inner_value_is_not_a_successful_absent_value() {
        let temp = Temp::new();
        let path = temp.file();
        std::fs::write(&path, br#"{"broken":"not JSON","other":"null"}"#).unwrap();
        assert!(get(&path, "broken").is_err());
        assert_eq!(
            get_raw(&path, "broken").unwrap().as_deref(),
            Some("not JSON")
        );
        assert_eq!(get(&path, "other").unwrap(), Some(Value::Null));
        set(
            &path,
            "1453_OnboardingTutorialDismissed",
            &Value::Bool(true),
        )
        .unwrap();
        assert_eq!(
            get_raw(&path, "broken").unwrap().as_deref(),
            Some("not JSON")
        );
    }
}
