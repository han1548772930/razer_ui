//! Source verified 1342 Audio Mixer DSP page bindings.
//!
//! The current middleware names these controls through `MixerSDKLib_PropertyControl`.
//! This module only maps controls whose path and native property are present in
//! the current 1342 source; virtual Windows mix devices are intentionally left
//! to the separate AudioCamy adapter.

use razer_device::audio_mixer::{MixerChannel, MixerControl, MixerTarget, MixerValue};
use std::sync::{Arc, atomic::AtomicBool};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioMixerOperation {
    Read,
    Write,
    MicEqWrite,
}

#[derive(Clone, Debug)]
pub struct AudioMixerRequest {
    pub generation: u64,
    pub operation: AudioMixerOperation,
    pub path: String,
    pub target: MixerTarget,
    pub value: Option<MixerValue>,
    pub eq_bands: Option<[i32; 10]>,
    pub epoch: u64,
    pub edit_revision: u64,
}

pub enum AudioMixerReply {
    Read(MixerValue),
    Write(razer_device::audio_mixer::MixerWriteResult),
    MicEqWrite(Vec<razer_device::audio_mixer::MixerWriteResult>),
}

pub struct AudioMixerCompletion {
    pub reply: AudioMixerReply,
    pub warning: Option<String>,
}

#[derive(Default)]
pub struct AudioMixerState {
    pub active: bool,
    pub epoch: u64,
    pub generation: u64,
    pub edit_revision: u64,
    pub pending: Option<AudioMixerRequest>,
    pub cancellation: Option<Arc<AtomicBool>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioMixerPath {
    NoiseGateEnabled,
    NoiseGateThreshold,
    NoiseGateTargetGain,
    NoiseGateAttack,
    NoiseGateRelease,
    CompressorEnabled,
    CompressorThreshold,
    CompressorKnee,
    CompressorRatio,
    CompressorMakeupGain,
    CompressorAttack,
    CompressorRelease,
    VocalFadingEnabled,
    VocalFadingLevel,
    VoiceChangerEnabled,
    VoiceChangerMode,
    EchoReverbEnabled,
    ReverbRoom,
    ReverbDecay,
    EchoGain,
    EchoDelay,
    KeyShift,
    MicEqBand(u8),
    MicEqEnabled,
}

impl AudioMixerPath {
    pub fn target(self) -> MixerTarget {
        let control = match self {
            Self::NoiseGateEnabled => MixerControl::NoiseGateEnabled,
            Self::NoiseGateThreshold => MixerControl::NoiseGateThreshold,
            Self::NoiseGateTargetGain => MixerControl::NoiseGateTargetGain,
            Self::NoiseGateAttack => MixerControl::NoiseGateAttack,
            Self::NoiseGateRelease => MixerControl::NoiseGateRelease,
            Self::CompressorEnabled => MixerControl::CompressorEnabled,
            Self::CompressorThreshold => MixerControl::CompressorThreshold,
            Self::CompressorKnee => MixerControl::CompressorKnee,
            Self::CompressorRatio => MixerControl::CompressorRatio,
            Self::CompressorMakeupGain => MixerControl::CompressorMakeupGain,
            Self::CompressorAttack => MixerControl::CompressorAttack,
            Self::CompressorRelease => MixerControl::CompressorRelease,
            Self::VocalFadingEnabled => MixerControl::PageVocalFadingEnabled,
            Self::VocalFadingLevel => MixerControl::PageVocalFadingLevel,
            Self::VoiceChangerEnabled => MixerControl::PageMagicVoiceEnabled,
            Self::VoiceChangerMode => MixerControl::PageMagicVoice,
            Self::EchoReverbEnabled => MixerControl::PageEchoReverbEnabled,
            Self::ReverbRoom => MixerControl::PageReverbRoom,
            Self::ReverbDecay => MixerControl::PageReverbDecay,
            Self::EchoGain => MixerControl::PageEchoGain,
            Self::EchoDelay => MixerControl::PageEchoDelay,
            Self::KeyShift => MixerControl::PageKeyShift,
            Self::MicEqBand(_) => MixerControl::EqBand,
            Self::MicEqEnabled => MixerControl::PageEqEnabled,
        };
        MixerTarget {
            control,
            band: match self {
                Self::MicEqBand(index) => Some(index),
                _ => None,
            },
            channel: MixerChannel::Both,
        }
    }
}

/// Return a native target only when the current source names the property.
/// `outputMixer`, `playbackMix`, `streamMix`, `lineOut` and `voiceChat` are
/// AudioCamy virtual endpoints and must not be routed to this HID protocol.
pub fn path(path: &str) -> Option<AudioMixerPath> {
    Some(match path {
        "/device/noiseGate/isEnabled" => AudioMixerPath::NoiseGateEnabled,
        "/device/noiseGate/basic/value" | "/device/noiseGate/threshold/value" => {
            AudioMixerPath::NoiseGateThreshold
        }
        "/device/noiseGate/targetGain/value" => AudioMixerPath::NoiseGateTargetGain,
        "/device/noiseGate/attackTime/value" => AudioMixerPath::NoiseGateAttack,
        "/device/noiseGate/releaseTime/value" => AudioMixerPath::NoiseGateRelease,
        "/device/compressor/isEnabled" => AudioMixerPath::CompressorEnabled,
        "/device/compressor/basic/value" | "/device/compressor/threshold/value" => {
            AudioMixerPath::CompressorThreshold
        }
        "/device/compressor/softKneeWidth/value" => AudioMixerPath::CompressorKnee,
        "/device/compressor/ratio/value" => AudioMixerPath::CompressorRatio,
        "/device/compressor/makeUpGain/value" => AudioMixerPath::CompressorMakeupGain,
        "/device/compressor/attackTime/value" => AudioMixerPath::CompressorAttack,
        "/device/compressor/releaseTime/value" => AudioMixerPath::CompressorRelease,
        "/device/vocalFading/isEnabled" => AudioMixerPath::VocalFadingEnabled,
        "/device/vocalFading/value" => AudioMixerPath::VocalFadingLevel,
        "/device/voiceChanger/isEnabled" => AudioMixerPath::VoiceChangerEnabled,
        "/device/voiceChanger/value" => AudioMixerPath::VoiceChangerMode,
        "/device/echoReverb/isEnabled" => AudioMixerPath::EchoReverbEnabled,
        "/device/echoReverb/modeValues/0" => AudioMixerPath::ReverbRoom,
        "/device/echoReverb/modeValues/1" => AudioMixerPath::ReverbDecay,
        "/device/echoReverb/modeValues/2" => AudioMixerPath::EchoGain,
        "/device/echoReverb/modeValues/3" => AudioMixerPath::EchoDelay,
        "/device/keyShifter/value" => AudioMixerPath::KeyShift,
        "/device/keyShifter/isEnabled" => AudioMixerPath::KeyShift,
        "/equalizers/mic/isEnabled" => AudioMixerPath::MicEqEnabled,
        _ => {
            let suffix = path.strip_prefix("/equalizers/mic/bands/")?;
            let band: u8 = suffix.parse().ok()?;
            if band >= 10 {
                return None;
            }
            AudioMixerPath::MicEqBand(band)
        }
    })
}

pub fn value_for(path: AudioMixerPath, value: &serde_json::Value) -> anyhow::Result<MixerValue> {
    match path {
        AudioMixerPath::NoiseGateEnabled
        | AudioMixerPath::CompressorEnabled
        | AudioMixerPath::VocalFadingEnabled
        | AudioMixerPath::VoiceChangerEnabled
        | AudioMixerPath::EchoReverbEnabled
        | AudioMixerPath::MicEqEnabled => Ok(MixerValue::Boolean {
            enabled: value
                .as_bool()
                .ok_or_else(|| anyhow::anyhow!("DSP toggle must be boolean"))?,
        }),
        AudioMixerPath::MicEqBand(_) => Ok(MixerValue::Scalar {
            value: value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("EQ band must be numeric"))?
                as f32,
        }),
        _ => Ok(MixerValue::Scalar {
            value: value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("DSP value must be numeric"))?
                as f32,
        }),
    }
}

/// Current `ki` / `wi` dispatch exactly one changed field. Mode changes are
/// local; Basic writes only threshold in mode 0, advanced fields only mode 1.
pub fn accepts_mode(path: &str, draft: &serde_json::Value) -> bool {
    for base in ["/device/noiseGate", "/device/compressor"] {
        if let Some(field) = path
            .strip_prefix(base)
            .and_then(|rest| rest.strip_prefix('/'))
        {
            if field == "isEnabled" {
                return true;
            }
            let mode = draft
                .pointer(&format!("{base}/useMode"))
                .and_then(serde_json::Value::as_u64);
            return if field == "basic/value" {
                mode == Some(0)
            } else {
                mode == Some(1) && field != "useMode"
            };
        }
    }
    true
}

/// Caller transformation, separate from native register encoding. Keys use
/// `n ? Number(i) : 0`; enabling vocal fading submits the retained level after
/// the switch. EQ editing enables EQ before submitting all ten bands.
pub fn write_plan(
    path: &str,
    draft: &serde_json::Value,
) -> anyhow::Result<Vec<(String, MixerValue)>> {
    if path.starts_with("/equalizers/mic/bands/") {
        anyhow::ensure!(
            self::path(path).is_some(),
            "Mic EQ band is outside the source table"
        );
        let bands = mic_eq_bands(draft)?;
        let values = razer_device::audio_mixer::mic_eq_values(&bands)?;
        let mut actions = vec![(
            "/equalizers/mic/isEnabled".into(),
            MixerValue::Boolean { enabled: true },
        )];
        actions.extend(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| (format!("/equalizers/mic/bands/{index}"), value)),
        );
        return Ok(actions);
    }
    if !accepts_mode(path, draft)
        || (self::path(path).is_none() && path != "/device/echoReverb/activeMode")
    {
        return Ok(Vec::new());
    }
    if path.starts_with("/device/keyShifter/") {
        let enabled = draft
            .pointer("/device/keyShifter/isEnabled")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("keyShifter enabled missing"))?;
        let value = if enabled {
            draft
                .pointer("/device/keyShifter/value")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("keyShifter value missing"))?
        } else {
            serde_json::json!(0)
        };
        return Ok(vec![(
            "/device/keyShifter/value".into(),
            value_for(AudioMixerPath::KeyShift, &value)?,
        )]);
    }
    if path.starts_with("/device/voiceChanger/") {
        let enabled = draft
            .pointer("/device/voiceChanger/isEnabled")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("Voice Changer enabled missing"))?;
        let mut actions = vec![(
            "/device/voiceChanger/isEnabled".into(),
            MixerValue::Boolean { enabled },
        )];
        if enabled {
            let mode = draft
                .pointer("/device/voiceChanger/value")
                .and_then(serde_json::Value::as_f64)
                .ok_or_else(|| anyhow::anyhow!("Voice Changer mode missing"))?;
            anyhow::ensure!(
                mode.is_finite() && mode.fract() == 0. && (0. ..=3.).contains(&mode),
                "Voice Changer mode is outside the current source modes"
            );
            actions.push((
                "/device/voiceChanger/value".into(),
                MixerValue::Scalar { value: mode as f32 },
            ));
        }
        return Ok(actions);
    }
    if path.starts_with("/device/echoReverb/") {
        let enabled = draft
            .pointer("/device/echoReverb/isEnabled")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("Echo/Reverb enabled missing"))?;
        let mut actions = vec![(
            "/device/echoReverb/isEnabled".into(),
            MixerValue::Boolean { enabled },
        )];
        if !enabled {
            return Ok(actions);
        }
        let array = draft
            .pointer("/device/echoReverb/modeValues")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("Echo/Reverb full array missing"))?;
        anyhow::ensure!(array.len() == 4, "Echo/Reverb needs four source fields");
        for (index, (min, max)) in [(0., 100.), (0.6, 2.7), (0., 1.), (110., 200.)]
            .into_iter()
            .enumerate()
        {
            let original = array[index]
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("Echo field must be numeric"))?;
            anyhow::ensure!(
                original.is_finite() && (min..=max).contains(&original),
                "Echo field exceeds mounted source range"
            );
            let value = if index == 0 {
                (original * 24. / 100. - 43.).floor()
            } else {
                original
            };
            // Current Ci guards each field with truthiness. Gain zero is
            // deliberately omitted rather than inventing a zero command.
            if value != 0. {
                actions.push((
                    format!("/device/echoReverb/modeValues/{index}"),
                    MixerValue::Scalar {
                        value: value as f32,
                    },
                ));
            }
        }
        Ok(actions)
    } else {
        write_plan_non_effect(path, draft)
    }
}

fn write_plan_non_effect(
    path: &str,
    draft: &serde_json::Value,
) -> anyhow::Result<Vec<(String, MixerValue)>> {
    if path.starts_with("/device/vocalFading/") {
        let enabled = draft
            .pointer("/device/vocalFading/isEnabled")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("vocalFading enabled missing"))?;
        if !enabled && path.ends_with("/value") {
            return Ok(Vec::new());
        }
        let mut actions = vec![(
            "/device/vocalFading/isEnabled".into(),
            MixerValue::Boolean { enabled },
        )];
        if enabled {
            let level = draft
                .pointer("/device/vocalFading/value")
                .ok_or_else(|| anyhow::anyhow!("vocalFading level missing"))?;
            actions.push((
                "/device/vocalFading/value".into(),
                value_for(AudioMixerPath::VocalFadingLevel, level)?,
            ));
        }
        return Ok(actions);
    }
    let mapped = self::path(path).expect("checked binding");
    let value = draft
        .pointer(path)
        .ok_or_else(|| anyhow::anyhow!("DSP draft value missing"))?;
    let mut actions = Vec::new();
    if matches!(mapped, AudioMixerPath::MicEqBand(_)) {
        actions.push((
            "/equalizers/mic/isEnabled".into(),
            MixerValue::Boolean { enabled: true },
        ));
    }
    actions.push((path.into(), value_for(mapped, value)?));
    Ok(actions)
}

/// Current preset, Reset and slider callers all submit the complete ten-band
/// array. Scalar UI values are integers; never derive raw data from a getter.
pub fn mic_eq_bands(draft: &serde_json::Value) -> anyhow::Result<[i32; 10]> {
    let basic = draft
        .pointer("/equalizers/mic/useMode")
        .and_then(serde_json::Value::as_u64)
        == Some(1);
    let values = draft
        .pointer(if basic {
            "/equalizers/mic_basic/bands"
        } else {
            "/equalizers/mic/bands"
        })
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Mic EQ full band array missing"))?;
    anyhow::ensure!(
        values.len() == if basic { 3 } else { 10 },
        "Mic EQ array does not match its mounted mode"
    );
    let mut bands = [0; 10];
    let source_indexes = if basic {
        [0, 0, 0, 0, 1, 1, 1, 1, 2, 2]
    } else {
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    };
    for (index, source) in source_indexes.into_iter().enumerate() {
        let value = &values[source];
        let value = value
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Mic EQ band must be numeric"))?;
        anyhow::ensure!(
            value.is_finite() && value.fract() == 0. && (-12. ..=12.).contains(&value),
            "Mic EQ band exceeds current slider range"
        );
        bands[index] = value as i32;
    }
    Ok(bands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn draft() -> serde_json::Value {
        json!({"device": {
            "noiseGate": {"useMode":0,"basic":{"value":-15},"threshold":{"value":-30}},
            "compressor": {"useMode":0,"basic":{"value":-12},"threshold":{"value":-24}},
            "keyShifter": {"isEnabled":true,"value":7},
            "vocalFading": {"isEnabled":true,"value":43}
        }, "equalizers":{"mic":{"bands":[0,1,2,3,4,5,6,7,8,9]}}})
    }
    #[test]
    fn basic_and_advanced_follow_current_caller_modes() {
        let mut state = draft();
        for base in ["noiseGate", "compressor"] {
            let basic = format!("/device/{base}/basic/value");
            let advanced = format!("/device/{base}/threshold/value");
            let plan = write_plan(&basic, &state).unwrap();
            assert_eq!(plan.len(), 1);
            let expected = state.pointer(&basic).unwrap().as_f64().unwrap() as f32;
            assert!(matches!(plan[0].1, MixerValue::Scalar { value } if value == expected));
            assert!(write_plan(&advanced, &state).unwrap().is_empty());
            state["device"][base]["useMode"] = json!(1);
            assert!(write_plan(&basic, &state).unwrap().is_empty());
            assert_eq!(write_plan(&advanced, &state).unwrap().len(), 1);
            assert!(
                write_plan(&format!("/device/{base}/useMode"), &state)
                    .unwrap()
                    .is_empty()
            );
        }
    }
    #[test]
    fn disabling_key_shift_submits_zero_and_preserves_draft() {
        let mut state = draft();
        state["device"]["keyShifter"]["isEnabled"] = json!(false);
        let plan = write_plan("/device/keyShifter/isEnabled", &state).unwrap();
        assert_eq!(plan[0].0, "/device/keyShifter/value");
        assert!(matches!(plan[0].1, MixerValue::Scalar { value: 0. }));
        assert_eq!(state["device"]["keyShifter"]["value"], json!(7));
    }
    #[test]
    fn vocal_enable_preserves_source_submission_order() {
        let state = draft();
        let plan = write_plan("/device/vocalFading/isEnabled", &state).unwrap();
        assert_eq!(plan.len(), 2);
        assert!(matches!(plan[0].1, MixerValue::Boolean { enabled: true }));
        assert!(matches!(plan[1].1, MixerValue::Scalar { value: 43. }));
    }
    #[test]
    fn echo_full_object_preserves_source_floor_zero_skip_and_disable() {
        let mut state = json!({"device":{"echoReverb":{
            "isEnabled":true,"activeMode":"custom","modeValues":[40,1.7,0,150]
        }}});
        let actions = write_plan("/device/echoReverb/activeMode", &state).unwrap();
        assert_eq!(actions.len(), 4);
        assert_eq!(actions[0].0, "/device/echoReverb/isEnabled");
        assert!(matches!(actions[1].1, MixerValue::Scalar { value: -34. }));
        assert_eq!(actions[2].0, "/device/echoReverb/modeValues/1");
        assert_eq!(actions[3].0, "/device/echoReverb/modeValues/3");
        assert!(!actions.iter().any(|(path, _)| path.ends_with("/2")));
        state["device"]["echoReverb"]["isEnabled"] = json!(false);
        let actions = write_plan("/device/echoReverb/isEnabled", &state).unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0].1,
            MixerValue::Boolean { enabled: false }
        ));
        state["device"]["echoReverb"]["isEnabled"] = json!(true);
        state["device"]["echoReverb"]["modeValues"][0] = json!(101);
        assert!(write_plan("/device/echoReverb/activeMode", &state).is_err());
    }
    #[test]
    fn eq_enables_before_band_and_rejects_out_of_table_indices() {
        let plan = write_plan("/equalizers/mic/bands/9", &draft()).unwrap();
        assert_eq!(plan[0].0, "/equalizers/mic/isEnabled");
        assert!(matches!(plan[0].1, MixerValue::Boolean { enabled: true }));
        assert_eq!(plan.len(), 11);
        assert_eq!(plan[1].0, "/equalizers/mic/bands/0");
        assert_eq!(plan[10].0, "/equalizers/mic/bands/9");
        assert!(matches!(
            plan[1].1,
            MixerValue::EqBand { data: 30, gain: 0 }
        ));
        assert!(matches!(
            plan[10].1,
            MixerValue::EqBand {
                data: 16000,
                gain: 9
            }
        ));
        assert!(path("/equalizers/mic/bands/10").is_none());
    }
    #[test]
    fn vocal_disable_never_submits_retained_level() {
        let mut state = draft();
        state["device"]["vocalFading"]["isEnabled"] = json!(false);
        let plan = write_plan("/device/vocalFading/isEnabled", &state).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(matches!(plan[0].1, MixerValue::Boolean { enabled: false }));
        assert!(
            write_plan("/device/vocalFading/value", &state)
                .unwrap()
                .is_empty()
        );
        assert_eq!(state["device"]["vocalFading"]["value"], json!(43));
    }
    #[test]
    fn virtual_bus_paths_never_become_hid_properties() {
        for name in [
            "outputMixerReducer",
            "playbackMixReducer",
            "streamMixReducer",
            "lineOutReducer",
            "voiceChatReducer",
        ] {
            assert!(path(&format!("/device/{name}/volume/value")).is_none());
        }
    }
}
