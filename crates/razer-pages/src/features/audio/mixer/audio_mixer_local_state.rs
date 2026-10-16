//! Current 1342 MW 5736.fdd55044a41484df28aa: taskMakerSetMicSetup,
//! Li/bi and their explicit QY reducer dispatches. Stored routing and visible
//! preview routing have distinct owners in the original source.
use serde_json::{Value, json};

pub(super) struct ProfileRouting {
    fields: Value,
}

const FIELDS: [&str; 6] = [
    "outputMixerReducer",
    "playbackMixReducer",
    "streamMixReducer",
    "lineOutReducer",
    "voiceChatReducer",
    "micMonitor",
];

impl ProfileRouting {
    pub fn from_draft(draft: &Value) -> Self {
        let mut fields = json!({});
        for field in FIELDS {
            if let Some(value) = draft["device"].get(field) {
                fields[field] = value.clone();
            }
        }
        Self { fields }
    }

    pub fn apply(&self, profile: &mut Value) {
        for field in FIELDS {
            if let Some(value) = self.fields.get(field) {
                profile["device"][field] = value.clone();
            }
        }
    }

    pub fn retain_edit(&mut self, path: &str, draft: &Value) {
        if path.starts_with("/device/micSetup/") {
            // taskMaker persists both complete channel objects, even when the
            // sole UI action changed useMode or phantom power.
            for channel in ["xlr", "mic"] {
                self.fields["outputMixerReducer"][channel] =
                    draft["device"]["micSetup"][channel].clone();
            }
        }
        for field in FIELDS {
            let prefix = format!("/device/{field}");
            if let Some(suffix) = path.strip_prefix(&prefix) {
                if !suffix.is_empty() && !suffix.starts_with('/') {
                    continue;
                }
                if let Some(value) = draft.pointer(path) {
                    if suffix.is_empty() {
                        self.fields[field] = value.clone();
                    } else if let Some(target) = self.fields[field].pointer_mut(suffix) {
                        *target = value.clone();
                    }
                }
            }
        }
    }

    pub fn retain_service_updates(&mut self, updates: &[(String, Value)]) -> bool {
        let mut changed = false;
        for (path, value) in updates {
            let Some(relative) = path.strip_prefix("/device/") else {
                continue;
            };
            let Some((field, suffix)) = relative.split_once('/') else {
                continue;
            };
            if !FIELDS.contains(&field) {
                continue;
            }
            if let Some(target) = self.fields[field].pointer_mut(&format!("/{suffix}")) {
                changed |= target != value;
                *target = value.clone();
            }
        }
        changed
    }
}

pub(super) fn apply_source_overlay(draft: &mut Value, profile: &Value, path: &str) {
    let saved = &profile["device"];
    let preview = saved["micPreview"].as_bool() == Some(true);
    let channel = if saved["micSetup"]["useMode"].as_u64() == Some(0) {
        "xlr"
    } else {
        "mic"
    };
    if path == "/device/micPreview" {
        draft["device"]["playbackMixReducer"]["mic"] = if preview {
            json!({"isEnabled":true,"isMuted":false})
        } else {
            saved["playbackMixReducer"]["mic"].clone()
        };
        draft["device"]["outputMixerReducer"][channel] =
            saved["outputMixerReducer"][channel].clone();
        if preview {
            draft["device"]["outputMixerReducer"][channel]["value"] =
                saved["micSetup"][channel]["value"].clone();
        }
        draft["device"]["micMonitor"] = if preview {
            json!({"isEnabled":true,"value":80})
        } else {
            saved["micMonitor"].clone()
        };
    }
}

/// bi publishes the setup preview only after its first native gain write has
/// completed. A failed MicSetup/phantom/gain prefix must leave the reducer alone.
pub(super) fn apply_setup_completion(
    draft: &mut Value,
    profile: &Value,
    path: &str,
    receipts: &[razer_device::audio_mixer_local::Receipt],
) -> bool {
    if !(path == "/device/micSetup" || path.starts_with("/device/micSetup/"))
        || profile["device"]["micPreview"] != true
    {
        return false;
    }
    let first_gain = receipts.iter().find(|receipt| {
        matches!(
            receipt.command,
            razer_device::audio_mixer_local::Command::InputMicGain { .. }
        )
    });
    if first_gain.is_some_and(|receipt| receipt.transport_completed) {
        let saved = &profile["device"];
        let channel = if saved["micSetup"]["useMode"] == 0 {
            "xlr"
        } else {
            "mic"
        };
        draft["device"]["outputMixerReducer"][channel] =
            saved["outputMixerReducer"][channel].clone();
        draft["device"]["outputMixerReducer"][channel]["value"] =
            saved["micSetup"][channel]["value"].clone();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{ProfileRouting, apply_setup_completion, apply_source_overlay};
    use serde_json::json;
    #[test]
    fn preview_dispatches_do_not_replace_persisted_routing() {
        let mut draft = json!({"device":{
            "micPreview":true,"micSetup":{"useMode":0,"xlr":{"value":38}},
            "outputMixerReducer":{"xlr":{"value":12}},
            "playbackMixReducer":{"mic":{"isEnabled":false,"isMuted":true}},
            "micMonitor":{"isEnabled":false,"value":31}
        }});
        let routing = ProfileRouting::from_draft(&draft);
        let saved = draft.clone();
        apply_source_overlay(&mut draft, &saved, "/device/micPreview");
        assert_eq!(draft["device"]["micMonitor"]["value"], 80);
        assert_eq!(draft["device"]["outputMixerReducer"]["xlr"]["value"], 38);
        let mut persisted = draft.clone();
        routing.apply(&mut persisted);
        assert_eq!(persisted, saved);
        persisted["device"]["micPreview"] = json!(false);
        apply_source_overlay(&mut draft, &persisted, "/device/micPreview");
        assert_eq!(draft["device"]["micMonitor"]["value"], 31);
        assert_eq!(draft["device"]["outputMixerReducer"]["xlr"]["value"], 12);
        assert_eq!(
            draft["device"]["playbackMixReducer"]["mic"]["isMuted"],
            true
        );
    }

    #[test]
    fn setup_persists_both_channels_without_an_unrequested_ui_dispatch() {
        let mut draft = json!({"device":{
            "micPreview":false,
            "micSetup":{"useMode":1,"xlr":{"value":44},"mic":{"value":13}},
            "outputMixerReducer":{"xlr":{"value":12},"mic":{"value":7}},
            "playbackMixReducer":{},"micMonitor":{"value":31}
        }});
        let mut routing = ProfileRouting::from_draft(&draft);
        routing.retain_edit("/device/micSetup/useMode", &draft);
        let mut profile = draft.clone();
        routing.apply(&mut profile);
        apply_source_overlay(&mut draft, &profile, "/device/micSetup/useMode");
        assert_eq!(profile["device"]["outputMixerReducer"]["xlr"]["value"], 44);
        assert_eq!(profile["device"]["outputMixerReducer"]["mic"]["value"], 13);
        assert_eq!(draft["device"]["outputMixerReducer"]["mic"]["value"], 7);
    }

    #[test]
    fn setup_preview_respects_native_gain_failure_boundary() {
        use razer_device::audio_mixer_local::{Command, Receipt};
        let profile = json!({"device":{"micPreview":true,
            "micSetup":{"useMode":1,"mic":{"value":13}},
            "outputMixerReducer":{"mic":{"value":7,"min":0,"max":40}}
        }});
        let mut draft = profile.clone();
        let path = "/device/micSetup/useMode";
        apply_source_overlay(&mut draft, &profile, path);
        assert_eq!(draft["device"]["outputMixerReducer"]["mic"]["value"], 7);
        let mut receipt = Receipt {
            command: Command::InputMicGain { value: 3. },
            transport_completed: false,
            response: json!({"error":"disconnected"}),
            profile_updates: vec![],
        };
        assert!(!apply_setup_completion(
            &mut draft,
            &profile,
            path,
            &[receipt.clone()]
        ));
        assert_eq!(draft, profile);
        receipt.transport_completed = true;
        assert!(apply_setup_completion(
            &mut draft,
            &profile,
            path,
            &[receipt]
        ));
        assert_eq!(draft["device"]["outputMixerReducer"]["mic"]["value"], 13);
        assert_eq!(draft["device"]["outputMixerReducer"]["mic"]["max"], 40);
    }
}
