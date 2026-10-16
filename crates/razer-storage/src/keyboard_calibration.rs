//! Source magnetic calibration records are device-wide and keyed by a real
//! serial number. Flat legacy records migrate only into that observed serial.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::path::Path;
pub fn read(path: &Path, pid: u32, serial: &str) -> anyhow::Result<Vec<Value>> {
    let root =
        crate::source_local::get(path, &format!("synapse_{pid}"))?.unwrap_or_else(|| json!({}));
    records(&root, serial)
}
fn records(root: &Value, serial: &str) -> anyhow::Result<Vec<Value>> {
    root.as_object()
        .context("Keyboard local cache is not an object")?;
    let Some(owner) = root.get(crate::linked_profiles::LOCAL_OWNER) else {
        return Ok(vec![]);
    };
    owner
        .as_object()
        .context("Keyboard local owner is not an object")?;
    let Some(keys) = owner.get("calibratedKeys") else {
        return Ok(vec![]);
    };
    let keys = keys
        .as_array()
        .context("Keyboard calibratedKeys is not an array")?;
    if keys.first().is_some_and(|row| row.get("inputID").is_some()) {
        return Ok(keys.clone());
    }
    if serial.is_empty() {
        return Ok(vec![]);
    }
    match keys
        .iter()
        .find(|row| row["serialNumber"] == serial)
        .and_then(|row| row.get("calibratedKeys"))
    {
        Some(keys) => keys
            .as_array()
            .cloned()
            .context("Serial calibration keys is not an array"),
        None => Ok(vec![]),
    }
}
pub fn save_key(
    path: &Path,
    pid: u32,
    serial: &str,
    input: &str,
    input_type: &str,
) -> anyhow::Result<Vec<Value>> {
    ensure!(
        !serial.is_empty(),
        "Calibration serial number was not observed"
    );
    let input = if input == "KEY_LEFT_GUI" {
        "KEY_WINDOWS"
    } else {
        input
    };
    let mut result = vec![];
    crate::source_local::update(path, &format!("synapse_{pid}"), |previous| {
        let mut root = previous.unwrap_or_else(|| json!({}));
        let object = root
            .as_object_mut()
            .context("Keyboard local cache is not an object")?;
        let owner = object
            .entry(crate::linked_profiles::LOCAL_OWNER)
            .or_insert_with(|| json!({}));
        let owner = owner
            .as_object_mut()
            .context("Keyboard local owner is not an object")?;
        let value = owner.entry("calibratedKeys").or_insert_with(|| json!([]));
        let keys = value
            .as_array_mut()
            .context("Keyboard calibratedKeys is not an array")?;
        if keys.first().is_some_and(|row| row.get("inputID").is_some()) {
            *keys = vec![json!({"serialNumber":serial,"calibratedKeys":keys.clone()})];
        }
        let index = keys
            .iter()
            .position(|row| row["serialNumber"] == serial)
            .unwrap_or_else(|| {
                keys.push(json!({"serialNumber":serial,"calibratedKeys":[]}));
                keys.len() - 1
            });
        let list = keys[index]["calibratedKeys"]
            .as_array_mut()
            .context("Serial calibration keys is not an array")?;
        if !list.iter().any(|key| key["inputID"] == input) {
            list.push(json!({"inputID":input,"inputType":if input_type.is_empty(){"KeyInput"}else{input_type}}));
        }
        result = list.clone();
        Ok(Some(root))
    })?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serial_records_migrate_and_preserve_unrelated_local_data() {
        let path = std::env::temp_dir().join(format!(
            "razer-magnetic-calibration-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let initial = json!({"other-owner":{"retain":true},"local":{"profile":{"guid":"retained"},"calibratedKeys":[{"inputID":"KEY_A","inputType":"Alphanumeric"}]}});
        crate::source_local::set(&path, "synapse_740", &initial).unwrap();
        assert_eq!(read(&path, 740, "").unwrap().len(), 1);
        let list = save_key(&path, 740, "SERIAL-A", "KEY_LEFT_GUI", "Modifier").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1]["inputID"], "KEY_WINDOWS");
        assert_eq!(
            save_key(&path, 740, "SERIAL-A", "KEY_WINDOWS", "Modifier")
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            save_key(&path, 740, "SERIAL-B", "KEY_FN", "").unwrap(),
            [json!({"inputID":"KEY_FN","inputType":"KeyInput"})]
        );
        assert_eq!(read(&path, 740, "SERIAL-A").unwrap(), list);
        assert!(read(&path, 740, "MISSING").unwrap().is_empty());
        let saved = crate::source_local::get(&path, "synapse_740")
            .unwrap()
            .unwrap();
        assert_eq!(saved["other-owner"], initial["other-owner"]);
        assert_eq!(saved["local"]["profile"], initial["local"]["profile"]);
        let bytes = std::fs::read(&path).unwrap();
        assert!(save_key(&path, 740, "", "KEY_B", "Alphanumeric").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        crate::source_local::set(
            &path,
            "synapse_740",
            &json!({"local":{"calibratedKeys":{}}}),
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(save_key(&path, 740, "SERIAL-A", "KEY_B", "Alphanumeric").is_err());
        assert!(read(&path, 740, "SERIAL-A").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.lock")).unwrap();
    }
}
