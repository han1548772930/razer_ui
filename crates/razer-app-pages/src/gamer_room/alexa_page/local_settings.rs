//! Local replacement for Alexa `ta.update("alexaSettings", patch)`.
//! JSON files are this program's storage adapter, not Chromium's storage files.
use serde_json::{Map, Value};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

pub(super) enum StorageResult {
    Loaded(Result<Map<String, Value>, String>),
    Saved(Result<(), String>),
}

pub(super) struct LocalSettings {
    sender: mpsc::Sender<(String, Value)>,
}

fn read(path: &Path) -> Result<Map<String, Value>, String> {
    razer_storage::read_document(path, |text| Ok(serde_json::from_str(text)?))
        .map(|value| value.unwrap_or_default())
        .map_err(|error| format!("{error:#}"))
}

fn merge(mut values: Map<String, Value>, key: String, value: Value) -> Map<String, Value> {
    values.insert(key, value);
    values
}

fn patch(path: &Path, key: String, value: Value) -> Result<(), String> {
    let values = merge(read(path)?, key, value);
    razer_storage::write_document(path, &values, |text| {
        let _: Map<String, Value> = serde_json::from_str(text)?;
        Ok(())
    })
    .map_err(|error| format!("{error:#}"))
}

impl LocalSettings {
    /// One worker serializes patches, so a second click cannot overtake the first.
    /// Dropping the page closes its sender; queued local writes drain before exit.
    pub(super) fn open() -> Result<(Self, async_channel::Receiver<StorageResult>), String> {
        let path: PathBuf = razer_storage::store_path().with_file_name("alexa-settings.json");
        let (sender, receiver) = mpsc::channel();
        let (results, observations) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("razer-alexa-local-settings".into())
            .spawn(move || {
                let _ = results.send_blocking(StorageResult::Loaded(read(&path)));
                while let Ok((key, value)) = receiver.recv() {
                    let _ = results.send_blocking(StorageResult::Saved(patch(&path, key, value)));
                }
            })
            .map_err(|error| error.to_string())?;
        Ok((Self { sender }, observations))
    }

    pub(super) fn update(&self, key: &str, value: Value) -> Result<(), String> {
        self.sender
            .send((key.to_owned(), value))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_patch_preserves_other_settings_and_stores_default_input_as_null() {
        let values = serde_json::from_value(serde_json::json!({
            "synapseSkills": true, "alexaLanguage": "ja-JP", "futureSetting": {"enabled": false}
        }))
        .unwrap();
        let values = merge(values, "synapseSkills".into(), Value::Bool(false));
        let values = merge(values, "alexaAudioInputDevice".into(), Value::Null);
        assert_eq!(values["synapseSkills"], false);
        assert_eq!(values["alexaLanguage"], "ja-JP");
        assert_eq!(
            values["futureSetting"],
            serde_json::json!({"enabled": false})
        );
        assert_eq!(values["alexaAudioInputDevice"], Value::Null);
    }
}
