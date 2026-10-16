//! Single receiver LED local profile writes. This app-owned document preserves
//! the source keys and version rules; it is separate from device acknowledgements.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::path::Path;

pub fn save_selection(
    path: &Path,
    product: u32,
    serial: &str,
    profile: &Value,
    status: &Value,
) -> anyhow::Result<u64> {
    ensure!(
        matches!(product, 158 | 192 | 194),
        "Single receiver LED storage source is absent"
    );
    ensure!(
        !serial.is_empty(),
        "Receiver profile needs the queried serial"
    );
    let fields = profile
        .as_object()
        .context("Receiver source profile is not an object")?;
    let guid = profile["guid"]
        .as_str()
        .filter(|guid| !guid.is_empty())
        .context("Receiver local UI profile GUID is absent")?;
    let mut version = 0;
    crate::source_local::update(path, &format!("synapse_{product}"), |previous| {
        let mut document = previous
            .unwrap_or_else(|| json!({"productId":product,"profiles":[],"deviceMetadatas":{}}));
        ensure!(
            document["productId"].as_u64() == Some(u64::from(product))
                && document["profiles"].is_array(),
            "Receiver local document differs or is malformed"
        );
        version = match document.get("version").filter(|value| !value.is_null()) {
            None => 1,
            Some(value) => {
                let value = value
                    .as_u64()
                    .context("Source version is not an unsigned integer")?;
                ensure!(value < (1_u64 << 53), "Source version exceeds 53 bits");
                (value + 1) & ((1_u64 << 53) - 1)
            }
        };
        let profiles = document["profiles"].as_array_mut().unwrap();
        let index = match profiles.iter().position(|entry| entry["guid"] == guid) {
            Some(index) => index,
            None => {
                profiles.push(profile.clone());
                profiles.len() - 1
            }
        };
        let entry = profiles[index]
            .as_object_mut()
            .context("Stored receiver profile is not an object")?;
        // Merge the supplied local profile; unrelated settings and unknown
        // fields in the document and profile remain intact.
        entry.extend(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        entry.insert("indicatorLedStatus".into(), status.clone());
        if document.get("deviceMetadatas").is_none() {
            document["deviceMetadatas"] = json!({});
        }
        ensure!(
            document["deviceMetadatas"].is_object(),
            "Receiver metadata is malformed"
        );
        if document["deviceMetadatas"].get(serial).is_none() {
            document["deviceMetadatas"][serial] = json!({});
        }
        ensure!(
            document["deviceMetadatas"][serial].is_object(),
            "Receiver serial metadata is malformed"
        );
        document["deviceMetadatas"][serial]["activeProfileGuid"] = json!(guid);
        document["activeProfile"] = json!(guid);
        document["version"] = json!(version);
        Ok(Some(document))
    })?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiver_legacy_selection_versions_preserve_unknown_fields_and_reject_corruption() {
        let folder = std::env::temp_dir().join(format!(
            "receiver-legacy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("local.json");
        let profile = json!({"guid":"local-profile","name":"Default","other":{"x":1}});
        assert_eq!(
            save_selection(&path, 158, "queried-serial", &profile, &json!(2)).unwrap(),
            1
        );
        let mut document = crate::source_local::get(&path, "synapse_158")
            .unwrap()
            .unwrap();
        document["version"] = json!((1_u64 << 53) - 1);
        document["unknown"] = json!(true);
        document["profiles"][0]["unknown"] = json!("retained");
        crate::source_local::set(&path, "synapse_158", &document).unwrap();
        assert_eq!(
            save_selection(&path, 158, "queried-serial", &profile, &json!(3)).unwrap(),
            0
        );
        let mut saved = crate::source_local::get(&path, "synapse_158")
            .unwrap()
            .unwrap();
        assert_eq!(saved["unknown"], true);
        assert_eq!(saved["profiles"][0]["unknown"], "retained");
        assert_eq!(saved["profiles"][0]["indicatorLedStatus"], 3);
        saved["version"] = json!(-1);
        crate::source_local::set(&path, "synapse_158", &saved).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(save_selection(&path, 158, "queried-serial", &profile, &json!(1)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
