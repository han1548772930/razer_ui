//! Current MW profile templates. Edition 0 and the fallback share the 2660 template.
use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    guid: &'a str,
    name: &'a str,
    audio_equalizer: Equalizer,
    #[serde(skip_serializing_if = "Option::is_none")]
    noise_cancellation: Option<NoiseCancellationDefault>,
    not_disturb: Enabled,
    audio_gesture_control: Enabled,
    audio_voice_prompt_control: Enabled,
    audio_auto_pause_control: Enabled,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Enabled {
    is_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NoiseCancellationDefault {
    is_enabled: bool,
    status: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Standard,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum PresetName {
    Game,
    Movie,
    Music,
    Custom,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Equalizer {
    selected_mode: Mode,
    presets: Presets,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Presets {
    selected_preset: SelectedPreset,
    standard: [Preset; 4],
}

#[derive(Serialize)]
struct SelectedPreset {
    standard: PresetName,
}

#[derive(Serialize)]
struct Preset {
    name: PresetName,
    default: [i8; 10],
    current: [i8; 10],
}

impl Preset {
    fn new(name: PresetName, bands: [i8; 10]) -> Self {
        Self {
            name,
            default: bands,
            current: bands,
        }
    }
}

pub(super) fn profile(product_id: u32, guid: &str, name: &str) -> Result<Value> {
    let noise_cancellation = match product_id {
        2657 => Some(NoiseCancellationDefault {
            is_enabled: true,
            status: 10,
            level: Some(10),
        }),
        2664 | 2668 => Some(NoiseCancellationDefault {
            is_enabled: true,
            status: 10,
            level: None,
        }),
        2660 | 2689 => None,
        _ => bail!("product {product_id} has no audited earbuds profile template"),
    };
    serde_json::to_value(Document {
        guid,
        name,
        audio_equalizer: Equalizer {
            selected_mode: Mode::Standard,
            presets: Presets {
                selected_preset: SelectedPreset {
                    standard: PresetName::Game,
                },
                standard: [
                    Preset::new(PresetName::Game, [2, 2, 3, 4, 1, -1, 2, 3, 3, 3]),
                    Preset::new(PresetName::Movie, [3, 3, 3, -4, -4, -2, 2, 3, 3, 3]),
                    Preset::new(PresetName::Music, [2, 2, -1, -1, -1, -1, 1, 3, 3, 3]),
                    Preset::new(PresetName::Custom, [0; 10]),
                ],
            },
        },
        noise_cancellation,
        not_disturb: Enabled { is_enabled: false },
        audio_gesture_control: Enabled { is_enabled: true },
        audio_voice_prompt_control: Enabled { is_enabled: true },
        audio_auto_pause_control: Enabled { is_enabled: true },
    })
    .map_err(Into::into)
}
