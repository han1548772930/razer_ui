//! Current 1342 il() restores only the selected profile's own fields. The
//! mounted reducer's initial values are not inputs to that restore chain.
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Action {
    Write(String),
    MicEqualizer,
}

pub(super) fn profile_actions(source: Option<&Value>) -> Vec<Action> {
    let Some(source) = source else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    if let Some(profile) = source.get("profile").and_then(Value::as_object) {
        for field in ["brightness", "quickEffects"] {
            if profile.contains_key(field) {
                actions.push(Action::Write(format!("/profile/{field}")));
            }
        }
    }
    if let Some(device) = source.get("device").and_then(Value::as_object) {
        for field in [
            "fader",
            "mixerBind",
            "outputMixerReducer",
            "playbackMixReducer",
            "streamMixReducer",
            "lineOutReducer",
            "voiceChatReducer",
            "micSetup",
            "micMonitor",
        ] {
            let Some(value) = device.get(field) else {
                continue;
            };
            match field {
                "fader" => actions.push(Action::Write("/device/fader/useMode".into())),
                "mixerBind" => actions.push(Action::Write("/device/mixerBind/bleepButton".into())),
                "micSetup" => actions.push(Action::Write("/device/micSetup".into())),
                "micMonitor" => actions.push(Action::Write("/device/micMonitor/value".into())),
                "outputMixerReducer" => {
                    if let Some(channels) = value.as_object() {
                        for channel in [
                            "xlr", "mic", "chat", "music", "game", "optical", "lineIn", "system",
                        ] {
                            if !channels.contains_key(channel) {
                                continue;
                            }
                            if matches!(channel, "mic" | "xlr") {
                                let mode = source.pointer("/device/micSetup/useMode");
                                if channel == "mic" && mode.and_then(Value::as_u64) != Some(1)
                                    || channel == "xlr" && mode.and_then(Value::as_u64) != Some(0)
                                {
                                    continue;
                                }
                            }
                            actions.push(Action::Write(format!(
                                "/device/outputMixerReducer/{channel}/value"
                            )));
                        }
                    }
                }
                "playbackMixReducer" | "streamMixReducer" | "lineOutReducer"
                | "voiceChatReducer" => {
                    actions.push(Action::Write(format!("/device/{field}/volume/value")));
                    // ol() restores volume followed by these independent routes;
                    // voiceChat deliberately excludes its own chat input.
                    for channel in [
                        "mic", "chat", "music", "game", "optical", "lineIn", "system",
                    ] {
                        if field == "voiceChatReducer" && channel == "chat" {
                            continue;
                        }
                        actions.push(Action::Write(format!(
                            "/device/{field}/{channel}/isEnabled"
                        )));
                    }
                }
                _ => {}
            }
        }
    }
    if source.pointer("/equalizers/mic").is_some()
        || source.pointer("/equalizers/mic_basic").is_some()
    {
        actions.push(Action::MicEqualizer);
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absent_profile_fields_cannot_restore_ui_defaults() {
        assert!(profile_actions(None).is_empty());
        assert!(profile_actions(Some(&json!({}))).is_empty());
        assert!(profile_actions(Some(&json!({"device":{"micPreview":false}}))).is_empty());
        assert_eq!(
            profile_actions(Some(&json!({"device":{"micMonitor":{"value":31}}}))),
            [Action::Write("/device/micMonitor/value".into())]
        );
    }

    #[test]
    fn input_routes_require_the_saved_mode_and_keep_source_bus_order() {
        let mut source = json!({"device":{"outputMixerReducer":{
            "xlr":{"value":17},"mic":{"value":11},"music":{"value":-19}
        }}});
        assert_eq!(
            profile_actions(Some(&source)),
            [Action::Write(
                "/device/outputMixerReducer/music/value".into()
            )]
        );
        source["device"]["micSetup"] = json!({"useMode":1});
        let actions = profile_actions(Some(&source));
        assert!(actions.contains(&Action::Write(
            "/device/outputMixerReducer/mic/value".into()
        )));
        assert!(!actions.contains(&Action::Write(
            "/device/outputMixerReducer/xlr/value".into()
        )));
        let actions = profile_actions(Some(&json!({"device":{"voiceChatReducer":{}}})));
        assert_eq!(
            actions[0],
            Action::Write("/device/voiceChatReducer/volume/value".into())
        );
        assert_eq!(
            actions[1],
            Action::Write("/device/voiceChatReducer/mic/isEnabled".into())
        );
        assert!(!actions.contains(&Action::Write(
            "/device/voiceChatReducer/chat/isEnabled".into()
        )));
    }
}
