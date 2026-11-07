use super::*;
use razer_device::earbuds_equalizer::{Mode, Preset, Setting};
use std::collections::BTreeMap;
const FIELD: &str = "audioEqualizer";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Equalizer { selected_mode: Mode, presets: Presets }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Presets { selected_preset: BTreeMap<String, Preset>, #[serde(flatten)] modes: BTreeMap<String, Vec<Entry>> }
#[derive(Deserialize)]
struct Entry { name: Preset, current: [i8;10] }

pub fn read_equalizer(device: &razer_model::model::Device, profile_id: &str) -> Result<Option<Setting>> {
    let index = workspace_profile_index(device, profile_id)?;
    let Some(control) = workspace_control(device.profiles[index].source_settings.as_ref(), FIELD)? else { return Ok(None); };
    let document: Equalizer = serde_json::from_value(Value::Object(control.clone())).context("Invalid local earbud EQ")?;
    let mode = document.selected_mode;
    let preset = *document.presets.selected_preset.get(mode.key()).context("Selected earbud EQ preset is absent")?;
    let entries = document.presets.modes.get(mode.key()).context("Selected earbud EQ mode is absent")?;
    let matching = entries.iter().filter(|entry| entry.name == preset).collect::<Vec<_>>();
    anyhow::ensure!(matching.len() == 1, "Selected earbud EQ preset is absent or duplicated");
    let setting = Setting { mode, preset, bands: matching[0].current };
    setting.validate()?;
    Ok(Some(setting))
}

pub fn merge_equalizer(device: &mut razer_model::model::Device, profile_id: &str, setting: Setting) -> Result<()> {
    setting.validate()?;
    read_equalizer(device, profile_id)?.context("Earbud EQ profile is absent")?;
    let index = workspace_profile_index(device, profile_id)?;
    let field = workspace_control_mut(&mut device.profiles[index].source_settings, FIELD);
    let mut candidate = Value::Object(field.clone());
    let presets = candidate.get_mut("presets").and_then(Value::as_object_mut).context("Earbud EQ presets are not an object")?;
    let entries = presets.get_mut(setting.mode.key()).and_then(Value::as_array_mut).context("Earbud EQ mode is absent")?;
    let indices = entries.iter().enumerate().filter(|(_, entry)| entry.get("name").and_then(Value::as_str) == Some(setting.preset.key())).map(|(index, _)| index).collect::<Vec<_>>();
    anyhow::ensure!(indices.len() == 1, "Earbud EQ target preset is absent or duplicated");
    entries[indices[0]].as_object_mut().context("Earbud EQ preset is not an object")?.insert("current".into(), serde_json::to_value(setting.bands)?);
    presets.get_mut("selectedPreset").and_then(Value::as_object_mut).context("Earbud EQ selection is not an object")?.insert(setting.mode.key().into(), serde_json::to_value(setting.preset)?);
    candidate.as_object_mut().unwrap().insert("selectedMode".into(), serde_json::to_value(setting.mode)?);
    *field = candidate.as_object().unwrap().clone();
    Ok(())
}
