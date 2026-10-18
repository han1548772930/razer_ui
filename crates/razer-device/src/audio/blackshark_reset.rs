//! Current 1398 DeviceResetFeature and ProductInfo local reset defaults.
use crate::{
    mapping_submission::next_source_version,
    thx_blackshark_firmware::{BlackSharkLocalEqPreset, default_blackshark_local_eq_presets},
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PRODUCT_ID: u32 = 1398;
pub const DEFAULT_SPATIAL_ENABLED: bool = false;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DefaultProfile {
    pub guid: String,
    pub name: String,
    game_chat_balance: GameChatBalance,
    bass_boost: Level,
    sound_normalization: Level,
    voice_clarity: Level,
    mic_volume_normalization: Level,
    mic_voice_clarity: Level,
    mic_sensitivity: Level,
    mic_preview: Enabled,
    mic_ambient_noise_reduction: Level,
    thx_spatial_audio: Enabled,
    thx_competitive_mode: Enabled,
    application_presets: Vec<()>,
    speaker_calibration: Calibration,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameChatBalance {
    balance_mode: u8,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Level {
    is_enabled: bool,
    value: u8,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Enabled {
    is_enabled: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Calibration {
    speakers: Vec<Speaker>,
    spatial_distance: SpatialDistance,
}
#[derive(Clone, Debug, Serialize)]
struct Speaker {
    id: &'static str,
    angle: i16,
}
#[derive(Clone, Debug, Serialize)]
struct SpatialDistance {
    distance: u8,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDefaults {
    version: u64,
    vendor_id: u16,
    product_id: u32,
    ble_id: u16,
    dongle_id: u16,
    edition_id: u8,
    name: &'static str,
    #[serde(rename = "reportIDs")]
    report_ids: ReportIds,
    custom_spatial_output: Vec<()>,
    power_saving: Level,
    cloud_audio_equalizer: Vec<BlackSharkLocalEqPreset>,
    hyper_speed_mode: u8,
    indicator_led_status: u8,
    configurable_voice_prompt: VoicePrompt,
    mic_equalizer: MicEqualizer,
    mic_side_tone: Level,
    simultaneous_audio_output: AudioOutput,
    mappings: Vec<()>,
    noise_cancellation: NoiseCancellation,
}
#[derive(Clone, Debug, Serialize)]
struct ReportIds {
    #[serde(rename = "4")]
    key: &'static str,
    #[serde(rename = "5")]
    hardware_event: &'static str,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VoicePrompt {
    mic_mute: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MicEqualizer {
    mode: &'static str,
    custom_bands: Vec<MicBand>,
}
#[derive(Clone, Debug, Serialize)]
struct MicBand {
    frequency: u32,
    decibel: i8,
}
#[derive(Clone, Debug, Serialize)]
struct AudioOutput {
    mode: u8,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NoiseCancellation {
    is_enabled: bool,
    status: u8,
    level: u8,
}

#[non_exhaustive]
pub struct LocalReset {
    pub profile: DefaultProfile,
    pub device: DeviceDefaults,
}

/// Dynamic settings are decoded only at this persistence boundary. Profiles
/// stay with their existing owner; the reset appends one new source profile.
pub fn prepare(
    product_id: u32,
    settings: Option<&Value>,
    names: &[&str],
    computer_name: &str,
    guid: &str,
) -> Result<LocalReset> {
    ensure!(
        product_id == PRODUCT_ID,
        "No current reset defaults for this product"
    );
    #[derive(Deserialize)]
    struct Version {
        #[serde(default)]
        #[serde(deserialize_with = "present_version")]
        version: Option<u64>,
    }
    fn present_version<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Option<u64>, D::Error> {
        u64::deserialize(deserializer).map(Some)
    }
    let version = match settings {
        Some(value) => {
            ensure!(
                value.is_object(),
                "Current reset settings are not an object"
            );
            serde_json::from_value::<Version>(value.clone())
                .context("Current reset settings are not a versioned object")?
                .version
        }
        None => None,
    };
    // ProductInfo.g always interpolates computerName, including an empty value.
    let base = format!("{computer_name}-Default");
    let mut name = base.clone();
    for suffix in 0..100 {
        if !names.contains(&name.as_str()) {
            break;
        }
        name = format!("{base} {}", suffix + 1);
    }
    let profile = profile_with_identity(guid, &name)?;
    let device = DeviceDefaults {
        version: next_source_version(version)?,
        vendor_id: 5426,
        product_id: PRODUCT_ID,
        ble_id: 1397,
        dongle_id: 1399,
        edition_id: 0,
        name: "BlackShark V3 Pro",
        report_ids: ReportIds {
            key: "razerKeyReportID",
            hardware_event: "hardwareEventReportID",
        },
        custom_spatial_output: vec![],
        power_saving: Level {
            is_enabled: true,
            value: 15,
        },
        cloud_audio_equalizer: default_blackshark_local_eq_presets(PRODUCT_ID)?,
        hyper_speed_mode: 1,
        indicator_led_status: 0,
        configurable_voice_prompt: VoicePrompt { mic_mute: false },
        mic_equalizer: MicEqualizer {
            mode: "broadcast",
            custom_bands: [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
                .into_iter()
                .zip([5, 4, 3, 1, -1, 0, 2, 3, 4, 4])
                .map(|(frequency, decibel)| MicBand { frequency, decibel })
                .collect(),
        },
        mic_side_tone: Level {
            is_enabled: false,
            value: 0,
        },
        simultaneous_audio_output: AudioOutput { mode: 0 },
        mappings: vec![],
        noise_cancellation: NoiseCancellation {
            is_enabled: false,
            status: 10,
            level: 1,
        },
    };
    Ok(LocalReset { profile, device })
}

/// ProductInfo.getDefaultProfile supplies the same profile fields to reset and
/// import. The caller owns the source name rule and fresh profile identity.
pub(crate) fn profile_with_identity(guid: &str, name: &str) -> Result<DefaultProfile> {
    let bytes = guid.as_bytes();
    ensure!(
        bytes.len() == 36
            && bytes[14] == b'4'
            && matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B')
            && bytes
                .iter()
                .enumerate()
                .all(|(index, byte)| if matches!(index, 8 | 13 | 18 | 23) {
                    *byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }),
        "BlackShark profile GUID is not UUID v4"
    );
    let level = |value| Level {
        is_enabled: false,
        value,
    };
    let disabled = || Enabled { is_enabled: false };
    Ok(DefaultProfile {
        guid: guid.into(),
        name: name.into(),
        game_chat_balance: GameChatBalance { balance_mode: 10 },
        bass_boost: level(55),
        sound_normalization: level(55),
        voice_clarity: level(55),
        mic_volume_normalization: level(55),
        mic_voice_clarity: level(55),
        mic_sensitivity: level(2),
        mic_preview: disabled(),
        mic_ambient_noise_reduction: level(60),
        thx_spatial_audio: Enabled {
            is_enabled: DEFAULT_SPATIAL_ENABLED,
        },
        thx_competitive_mode: disabled(),
        application_presets: vec![],
        speaker_calibration: Calibration {
            speakers: [
                ("A6", -90),
                ("A1", -30),
                ("A2", 0),
                ("A0", 30),
                ("A5", 90),
                ("A4", -145),
                ("A3", 145),
                ("A8", 45),
                ("A9", -45),
                ("A10", 135),
                ("A11", -135),
            ]
            .into_iter()
            .map(|(id, angle)| Speaker { id, angle })
            .collect(),
            spatial_distance: SpatialDistance { distance: 50 },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const GUID: &str = "2f7a6459-2f8c-41c0-8757-e5e92f45e843";

    #[test]
    fn complete_reset_defaults_equal_the_independently_decoded_current_source() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../testdata/re/blackshark-reset-current-source.json"
        ))
        .unwrap();
        let reset = prepare(PRODUCT_ID, None, &[], "PC", GUID).unwrap();
        let mut profile = serde_json::to_value(reset.profile).unwrap();
        profile.as_object_mut().unwrap().remove("guid");
        profile.as_object_mut().unwrap().remove("name");
        let mut device = serde_json::to_value(reset.device).unwrap();
        device.as_object_mut().unwrap().remove("version");
        assert_eq!(profile, fixture["profile_defaults"]);
        assert_eq!(device, fixture["device_defaults"]);
    }

    #[test]
    fn source_reset_name_version_and_complete_defaults_are_preserved() {
        let reset = prepare(
            PRODUCT_ID,
            Some(&json!({"version": (1u64 << 53)-1})),
            &["PC-Default", "PC-Default 1"],
            "PC",
            GUID,
        )
        .unwrap();
        assert_eq!(reset.profile.name, "PC-Default 2");
        assert_eq!(reset.device.version, 0);
        let profile = serde_json::to_value(&reset.profile).unwrap();
        assert_eq!(
            profile["speakerCalibration"]["speakers"]
                .as_array()
                .unwrap()
                .len(),
            11
        );
        assert_eq!(
            profile["micSensitivity"],
            json!({"isEnabled":false,"value":2})
        );
        let device = serde_json::to_value(&reset.device).unwrap();
        assert_eq!(device["cloudAudioEqualizer"][4]["cloudEqualizerId"], 10);
        assert_eq!(
            device["cloudAudioEqualizer"][8]["bandValues"],
            json!([2, 2, 3, 3, -1, -2, 1, 4, 2, 2])
        );
        assert_eq!(
            device["micEqualizer"]["customBands"][4],
            json!({"frequency":500,"decibel":-1})
        );
        assert!(device.get("profiles").is_none());
        assert_eq!(
            prepare(PRODUCT_ID, None, &[], "", GUID)
                .unwrap()
                .profile
                .name,
            "-Default"
        );
    }

    #[test]
    fn malformed_version_and_foreign_products_do_not_become_defaults() {
        for settings in [
            json!([]),
            json!({"version":null}),
            json!({"version":"1"}),
            json!({"version":1u64 << 53}),
        ] {
            assert!(prepare(PRODUCT_ID, Some(&settings), &[], "PC", GUID).is_err());
        }
        assert!(prepare(1401, None, &[], "PC", GUID).is_err());
    }
}
