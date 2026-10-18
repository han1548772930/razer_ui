//! Current51765/66487 ordinary magnetic-keyboard OBM conversion.
use super::{controls::is_interruptible, source};
pub use crate::onboard_assignment::MacroPlayback;
use crate::{
    mapping_engine::MappingEngine,
    onboard_assignment::{self, ButtonAssignment},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreparedAssignment {
    Button(ButtonAssignment),
    Macro {
        key_id: u8,
        mode: u8,
        guid: String,
        playback: MacroPlayback,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mapping {
    #[serde(rename = "inputID")]
    input: String,
    input_type: String,
    #[serde(default)]
    is_hyper_shift: bool,
    #[serde(default)]
    mapping: Vec<Value>,
    #[serde(flatten)]
    fields: serde_json::Map<String, Value>,
}
#[derive(Deserialize)]
#[serde(tag = "outputType")]
enum Output {
    #[serde(rename = "keyboardGroup")]
    Keyboard {
        #[serde(rename = "keyboardGroup")]
        group: Keyboard,
    },
    #[serde(rename = "mouseGroup")]
    Mouse {
        #[serde(rename = "mouseGroup")]
        group: Mouse,
    },
    #[serde(rename = "multimediaGroup")]
    Media {
        #[serde(rename = "multimediaGroup")]
        group: Media,
    },
    #[serde(rename = "macroGroup")]
    Macro {
        #[serde(rename = "macroGroup")]
        group: Macro,
    },
    #[serde(rename = "disableGroup")]
    Disabled,
    #[serde(rename = "hyperShiftGroup")]
    Hyper,
    #[serde(other)]
    Software,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Keyboard {
    key: Option<String>,
    #[serde(default)]
    modifiers: Vec<String>,
    turbo_mode: Option<Turbo>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Turbo {
    is_turbo: bool,
    keys_per_second: Option<u16>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mouse {
    mouse_assignment: String,
    turbo_mode: Option<Turbo>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Media {
    multimedia_assignment: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Macro {
    guid: String,
    macro_playback_option: String,
    repeat_count: Option<u8>,
}

pub(super) fn prepare(
    product: u32,
    layout: u32,
    profile: &Value,
    old: &Value,
) -> anyhow::Result<Vec<PreparedAssignment>> {
    let factory = MappingEngine::new(product)?;
    let defaults = source::default_buttons(product, layout)?;
    let mut result = Vec::new();
    for row in super::removed_mappings(profile, old)? {
        let row: Mapping = serde_json::from_value(row)?;
        if !onboard_row(&row)? {
            continue;
        }
        if let Some(default) = defaults
            .iter()
            .find(|default| default.input == row.input && default.hyper == row.is_hyper_shift)
            .or_else(|| defaults.iter().find(|default| default.input == row.input))
        {
            result.push(PreparedAssignment::Button(ButtonAssignment {
                key_id: source::firmware_key(product, &row.input)?,
                mode: u8::from(row.is_hyper_shift),
                function_id: default.function_id,
                data_size: default.size,
                data: default.data,
            }));
        }
    }
    let rows = profile["mappings"]
        .as_array()
        .context("Magnetic OBM mappings are absent")?;
    for value in rows {
        let row: Mapping = serde_json::from_value(value.clone())?;
        if !onboard_row(&row)? {
            continue;
        }
        let Ok(key_id) = source::firmware_key(product, &row.input) else {
            continue;
        };
        if !defaults.iter().any(|default| default.input == row.input) {
            continue;
        }
        let stage = if row.input_type == "AnalogInput" {
            row.mapping
                .first()
                .context("Analog OBM primary mapping is absent")?
                .clone()
        } else {
            Value::Object(row.fields.clone())
        };
        let output: Output = serde_json::from_value(stage.clone())?;
        let mode = u8::from(row.is_hyper_shift);
        let (function_id, data_size, data) = match output {
            Output::Keyboard { group } => {
                if group.key.is_none() {
                    if let Some(default) = defaults.iter().find(|default| {
                        default.input == row.input && default.hyper == row.is_hyper_shift
                    }) {
                        if default.function_id == 2 || default.function_id == 13 {
                            (default.function_id, default.size, default.data)
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    let key = group.key.as_deref().unwrap();
                    let metadata = factory
                        .source_keys()
                        .iter()
                        .find(|row| row.input_id == Some(key))
                        .context("OBM keyboard output key is unknown")?;
                    let hid = metadata
                        .hid
                        .context("OBM output HID is absent")?
                        .parse::<u8>()?;
                    let mut modifiers = 0;
                    for modifier in &group.modifiers {
                        modifiers |= modifier_bit(modifier)?;
                    }
                    onboard_assignment::keyboard_function(
                        hid,
                        modifiers,
                        group
                            .turbo_mode
                            .as_ref()
                            .is_some_and(|turbo| turbo.is_turbo),
                        group.turbo_mode.and_then(|turbo| turbo.keys_per_second),
                        true,
                    )?
                }
            }
            Output::Mouse { group } => mouse(group)?,
            Output::Media { group } => {
                let value = match group.multimedia_assignment.as_str() {
                    "MediaEject" => 184,
                    "MuteVolume" => 226,
                    "VolumeDown" => 234,
                    "VolumeUp" => 233,
                    "PrevTrack" => 182,
                    "MediaStop" => 183,
                    "Play" => 205,
                    "NextTrack" => 181,
                    "CycleApps" => 671,
                    "DisplayBrightnessUp" => 111,
                    "DisplayBrightnessDown" => 112,
                    "SpotlightSearch" => 545,
                    "MicVolumeUp" | "MicVolumeDown" | "MuteMic" => continue,
                    other => anyhow::bail!("Unknown OBM multimedia assignment {other}"),
                } as u16;
                let [high, low] = value.to_be_bytes();
                (10, 2, [high, low, 0, 0, 0])
            }
            Output::Macro { group } => {
                let playback = match group.macro_playback_option.as_str() {
                    "Once" => MacroPlayback::Once,
                    "NTimes" => MacroPlayback::NTimes(
                        group.repeat_count.context("Macro repeat count is absent")?,
                    ),
                    "ContinuousHeld" => MacroPlayback::Held,
                    "ContinuousToggle" => MacroPlayback::Toggle,
                    "Sequence" => MacroPlayback::Sequence,
                    other => anyhow::bail!("Unknown macro playback {other}"),
                };
                ensure!(!group.guid.is_empty(), "OBM macro GUID is absent");
                result.push(PreparedAssignment::Macro {
                    key_id,
                    mode,
                    guid: group.guid,
                    playback,
                });
                continue;
            }
            Output::Disabled => (0, 0, [0; 5]),
            Output::Hyper => (12, 1, [1, 0, 0, 0, 0]),
            Output::Software => continue,
        };
        result.push(PreparedAssignment::Button(ButtonAssignment {
            key_id,
            mode,
            function_id,
            data_size,
            data,
        }));
    }
    Ok(result)
}

fn onboard_row(row: &Mapping) -> anyhow::Result<bool> {
    let stage = if row.input_type == "AnalogInput" {
        row.mapping
            .first()
            .context("Analog OBM primary mapping is absent")?
            .clone()
    } else {
        Value::Object(row.fields.clone())
    };
    if row.input_type == "AnalogInput"
        && stage["outputType"] == "keyboardGroup"
        && stage["keyboardGroup"]
            .as_object()
            .is_some_and(|group| group.is_empty())
    {
        return Ok(false);
    }
    let output: Output = serde_json::from_value(stage)?;
    Ok(match output {
        Output::Software => false,
        Output::Mouse { group } => {
            ![
                "ScrollLeft",
                "ScrollRight",
                "RepeatScrollUp",
                "RepeatScrollDown",
            ]
            .contains(&group.mouse_assignment.as_str())
                && !(group.mouse_assignment == "DoubleClick"
                    && group.turbo_mode.is_some_and(|turbo| turbo.is_turbo))
        }
        Output::Media { group } => !["MicVolumeUp", "MicVolumeDown", "MuteMic"]
            .contains(&group.multimedia_assignment.as_str()),
        _ => true,
    })
}

fn modifier_bit(modifier: &str) -> anyhow::Result<u8> {
    Ok(match modifier {
        "KEY_LEFT_CTRL" => 1,
        "KEY_LEFT_SHIFT" => 2,
        "KEY_LEFT_ALT" => 4,
        "KEY_LEFT_GUI" => 8,
        "KEY_RIGHT_CTRL" => 16,
        "KEY_RIGHT_SHIFT" => 32,
        "KEY_RIGHT_ALT" => 64,
        "KEY_RIGHT_GUI" => 128,
        "Extended_Key" => 0,
        other => anyhow::bail!("Unknown OBM modifier {other}"),
    })
}

fn mouse(group: Mouse) -> anyhow::Result<(u8, u8, [u8; 5])> {
    let button = match group.mouse_assignment.as_str() {
        "Click" => 1,
        "Menu" => 2,
        "ScrollButton" => 3,
        "Previous" => 4,
        "Next" => 5,
        "ScrollUp" => 9,
        "ScrollDown" => 10,
        // These are source software-only mouse actions for740/746.
        "ScrollLeft" | "ScrollRight" | "RepeatScrollUp" | "RepeatScrollDown" => {
            return Ok((0, 0, [0; 5]));
        }
        "DoubleClick" => {
            if group
                .turbo_mode
                .as_ref()
                .is_some_and(|turbo| turbo.is_turbo)
            {
                return Ok((0, 0, [0; 5]));
            }
            return Ok((11, 1, [1, 0, 0, 0, 0]));
        }
        "ScrollModeSwitch" => return Ok((18, 1, [1, 0, 0, 0, 0])),
        other => anyhow::bail!("Unknown OBM mouse action {other}"),
    };
    if let Some(turbo) = group.turbo_mode.filter(|turbo| turbo.is_turbo) {
        let rate = turbo.keys_per_second.unwrap_or(7);
        ensure!(rate > 0, "Mouse turbo rate is zero");
        let [high, low] = (1000 / rate).to_be_bytes();
        Ok((14, 3, [button, high, low, 0, 0]))
    } else {
        Ok((1, 1, [button, 0, 0, 0, 0]))
    }
}

/// Qc normalizes release thresholds before persistence and converts flat OTFM
/// assignments using the prior analog threshold or source preset threshold.
pub(super) fn normalize(
    product: u32,
    layout: u32,
    profile: &Value,
    old: &Value,
    rows: &[Value],
) -> anyhow::Result<Vec<Value>> {
    ensure!(
        super::supports(product),
        "Magnetic mapping normalizer product differs"
    );
    let guid = profile["guid"]
        .as_str()
        .context("Magnetic normalizer profile GUID is absent")?;
    let old = old["mappings"]
        .as_array()
        .context("Previous magnetic mappings are absent")?;
    let mut result = Vec::new();
    for value in rows {
        let input = value["inputID"]
            .as_str()
            .context("Magnetic mapping input is absent")?;
        if [3, 4, 6, 7, 10, 15, 16, 17, 18].contains(&layout) && input == "KEY_BACKSLASH" {
            continue;
        }
        let mut value = value.clone();
        if value["inputType"] == "AnalogInput" {
            for stage in value["mapping"]
                .as_array_mut()
                .context("Analog stages are absent")?
            {
                let low = stage["actuationPoint"]["0"]
                    .as_f64()
                    .or_else(|| stage["actuationPoint"][0].as_f64())
                    .context("Analog make threshold is absent")?;
                ensure!(
                    low.is_finite() && low.fract() == 0. && (0. ..=65535.).contains(&low),
                    "Analog make threshold is not an integral u16"
                );
                let low = low as u64;
                let release = low.saturating_sub(1638).max(256);
                if stage["actuationPoint"].is_array() {
                    stage["actuationPoint"][1] = json!(release);
                } else {
                    stage["actuationPoint"]["1"] = json!(release);
                }
            }
        }
        if value["outputType"] == "macroGroup"
            && (value.get("mapping").is_none() || value["inputType"] == "KeyInput")
        {
            let defaults = source::actuation(guid, Some(input));
            let point = old
                .iter()
                .find(|row| {
                    row["inputID"] == input
                        && row["isHyperShift"] != true
                        && row["inputType"] == "AnalogInput"
                })
                .and_then(|row| row.pointer("/mapping/0/actuationPoint"))
                .cloned()
                .unwrap_or_else(|| json!({"0":defaults.low,"1":defaults.high}));
            value = json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":value["isHyperShift"]==true,
                "mapping":[{"outputType":"macroGroup","macroGroup":value["macroGroup"],"actuationPoint":point},
                    {"outputType":"defaultGroup","actuationPoint":point,"isDefault":false,"isHyperShift":false}]});
        }
        if is_interruptible(&value) {
            value.as_object_mut().unwrap().remove("interrupts");
        }
        result.push(value);
    }
    Ok(result)
}
