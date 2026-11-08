//! The source OTFS closure retains binding rows, independently of a saved document.
use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Snapshot {
    mappings: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    product_id: u32,
    active_profile: String,
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    guid: String,
    mappings: Vec<Value>,
}

impl Snapshot {
    pub fn capture(document: &Value) -> Result<Self> {
        let document: Document = serde_json::from_value(document.clone())?;
        let profile = document
            .profiles
            .into_iter()
            .find(|profile| profile.guid == document.active_profile)
            .context("Active OTFS profile is absent")?;
        Ok(Self {
            mappings: profile.mappings,
        })
    }
    pub(crate) fn mappings(&self) -> &[Value] {
        &self.mappings
    }
}

/// The caller installs the snapshot before compilation and keeps it on failure.
/// Repeated enable restores; successful restoration is the only clearing point.
pub fn project(document: &Value, snapshot: &Snapshot, temporary: bool) -> Result<Option<Value>> {
    let parsed: Document = serde_json::from_value(document.clone())?;
    ensure!(
        parsed.product_id == 70
            || crate::mouse_receiver_mapping::supports_ordinary(parsed.product_id),
        "OTFS compiler is unavailable for this product"
    );
    if !parsed
        .profiles
        .iter()
        .any(|profile| profile.guid == parsed.active_profile)
    {
        return Ok(None);
    }
    let result = match parsed.product_id {
        70 => crate::mouse_legacy_mapping::project_interdevice_otfs(
            document,
            &snapshot.mappings,
            temporary,
        ),
        pid if crate::mouse_receiver_mapping::supports_ordinary(pid) => {
            crate::mouse_receiver_mapping::project_interdevice_otfs(
                document,
                &snapshot.mappings,
                temporary,
            )
        }
        _ => unreachable!(),
    }?;
    Ok(Some(result))
}

#[derive(Default)]
pub struct PublicationCache {
    previous: Option<PublicationIdentity>,
}
#[derive(PartialEq)]
struct PublicationIdentity {
    active: String,
    defaults: Option<Value>,
    analog: Option<String>,
    profiles: Vec<ProfileIdentity>,
}
#[derive(PartialEq)]
struct ProfileIdentity {
    guid: String,
    hash: Option<Value>,
    gamemode: Option<Value>,
}

impl PublicationCache {
    /// The current publisher advances its comparison state before awaiting I/O.
    pub fn admit(&mut self, document: &Value) -> Result<bool> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Input {
            active_profile: String,
            profiles: Vec<Entry>,
            default_mappings: Option<Defaults>,
            #[serde(default, deserialize_with = "present_json")]
            analog_data: Option<Value>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Entry {
            guid: String,
            app_engine: Option<Engine>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Defaults {
            app_engine: Option<Engine>,
        }
        #[derive(Deserialize)]
        struct Engine {
            hash: Option<Value>,
            gamemode: Option<Value>,
        }
        let input: Input = serde_json::from_value(document.clone())?;
        let mut next = PublicationIdentity {
            active: input.active_profile,
            defaults: input
                .default_mappings
                .and_then(|defaults| defaults.app_engine)
                .and_then(|engine| engine.hash),
            analog: input
                .analog_data
                .as_ref()
                .map(crate::mapping_engine_hash::source_hash)
                .transpose()?,
            profiles: input
                .profiles
                .into_iter()
                .map(|entry| ProfileIdentity {
                    guid: entry.guid,
                    hash: entry
                        .app_engine
                        .as_ref()
                        .and_then(|engine| engine.hash.clone()),
                    gamemode: entry.app_engine.and_then(|engine| engine.gamemode),
                })
                .collect(),
        };
        let changed = self.previous.as_ref().is_none_or(|previous| {
            previous.active != next.active
                || previous.defaults != next.defaults
                || (next.analog.is_some() && previous.analog != next.analog)
                || previous.profiles.len() != next.profiles.len()
                || next
                    .profiles
                    .iter()
                    .any(|entry| !previous.profiles.contains(entry))
        });
        if changed {
            if next.analog.is_none() {
                next.analog = self
                    .previous
                    .as_ref()
                    .and_then(|previous| previous.analog.clone());
            }
            self.previous = Some(next);
        }
        Ok(changed)
    }
}

fn present_json<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn document(pid: u32) -> Value {
        let profiles = [
            json!({"guid":"a","name":"A","mappings":[
            {"inputType":"MouseInput","inputID":"Button4","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}},
            {"inputType":"MouseInput","inputID":"Button5","outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_OnTheFly"}}]}),
            json!({"guid":"b","name":"B","mappings":[],"extension":42}),
        ];
        let guids = (0..128).map(|i| format!("00000000-0000-4000-8000-{i:012}"));
        match pid {
            70 => {
                crate::mouse_legacy_mapping::prepare_document(&profiles, "a", None, guids).unwrap()
            }
            158 => crate::mouse_receiver_mapping::prepare_document(&profiles, "a", None, guids)
                .unwrap(),
            _ => unreachable!(),
        }
    }
    #[test]
    fn snapshot_restores_into_current_profile_without_overwriting_current_bindings() {
        for pid in [70, 158] {
            let ordinary = document(pid);
            let snapshot = Snapshot::capture(&ordinary).unwrap();
            let mut temporary = project(&ordinary, &snapshot, true).unwrap().unwrap();
            assert_eq!(
                temporary["profiles"][0]["mappings"],
                ordinary["profiles"][0]["mappings"]
            );
            assert_eq!(temporary["defaultMappings"], ordinary["defaultMappings"]);
            let serialized = temporary["profiles"][0]["appEngine"].to_string();
            assert!(serialized.contains("DPI_Value_Up"));
            assert!(serialized.contains("DPI_Value_Down"));
            temporary["activeProfile"] = json!("b");
            let restored = project(&temporary, &snapshot, false).unwrap().unwrap();
            assert_eq!(restored["profiles"][1]["mappings"], json!([]));
            assert_eq!(restored["profiles"][1]["extension"], 42);
            assert_eq!(
                restored["profiles"][1]["appEngine"]["hash"],
                ordinary["profiles"][0]["appEngine"]["hash"]
            );
            assert_eq!(restored["profiles"][0], temporary["profiles"][0]);
            temporary["activeProfile"] = json!("removed");
            assert!(project(&temporary, &snapshot, false).unwrap().is_none());
        }
    }
    #[test]
    fn publisher_cache_advances_before_failure_and_compares_profiles_by_guid() {
        let mut doc = document(158);
        let mut cache = PublicationCache::default();
        assert!(cache.admit(&doc).unwrap());
        assert!(!cache.admit(&doc).unwrap());
        doc["profiles"].as_array_mut().unwrap().reverse();
        assert!(!cache.admit(&doc).unwrap());
        doc["profiles"][0]["appEngine"]["gamemode"] = json!(true);
        assert!(cache.admit(&doc).unwrap());
        assert!(!cache.admit(&doc).unwrap());
    }

    #[test]
    fn publisher_preserves_analog_hash_when_field_is_absent() {
        let mut doc = document(158);
        doc["analogData"] = json!({"value": 42});
        let mut cache = PublicationCache::default();
        assert!(cache.admit(&doc).unwrap());
        doc.as_object_mut().unwrap().remove("analogData");
        assert!(!cache.admit(&doc).unwrap());
        doc["activeProfile"] = json!("b");
        assert!(cache.admit(&doc).unwrap());
        doc["analogData"] = json!({"value": 42});
        assert!(!cache.admit(&doc).unwrap());
        doc["analogData"] = Value::Null;
        assert!(cache.admit(&doc).unwrap());
        doc.as_object_mut().unwrap().remove("analogData");
        assert!(!cache.admit(&doc).unwrap());
        doc["analogData"] = Value::Null;
        assert!(!cache.admit(&doc).unwrap());
    }
}
