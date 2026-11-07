use super::*;
use razer_device::earbuds_equalizer::{Mode, Preset, Setting};
use std::collections::BTreeMap;
const FIELD: &str = "audioEqualizer";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Equalizer {
    selected_mode: Mode,
    presets: Presets,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Presets {
    selected_preset: BTreeMap<String, Preset>,
    #[serde(flatten)]
    modes: BTreeMap<String, Value>,
}
#[derive(Deserialize)]
struct Entry {
    name: Preset,
    current: [i8; 10],
    default: [i8; 10],
}

pub fn read_equalizer(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Option<Setting>> {
    let index = workspace_profile_index(device, profile_id)?;
    decode_equalizer(device.profiles[index].source_settings.as_ref())
}

pub fn initial_equalizer(product_id: u32) -> Result<Setting> {
    let source = defaults::profile(product_id, "", "")?;
    decode_equalizer(Some(&source))?.context("Initial earbud EQ is absent")
}

pub fn initial_equalizer_source(product_id: u32) -> Result<Value> {
    defaults::profile(product_id, "", "")?
        .get(FIELD)
        .cloned()
        .context("Initial EQ absent")
}

fn decode_equalizer(source: Option<&Value>) -> Result<Option<Setting>> {
    let Some(control) = workspace_control(source, FIELD)? else {
        return Ok(None);
    };
    let document: Equalizer = serde_json::from_value(Value::Object(control.clone()))
        .context("Invalid local earbud EQ")?;
    let mode = document.selected_mode;
    let preset = *document
        .presets
        .selected_preset
        .get(mode.key())
        .context("Selected earbud EQ preset is absent")?;
    let entries = document
        .presets
        .modes
        .get(mode.key())
        .context("Selected earbud EQ mode is absent")?;
    let entries: Vec<Entry> =
        serde_json::from_value(entries.clone()).context("Invalid earbud EQ mode entries")?;
    let matching = entries
        .iter()
        .filter(|entry| entry.name == preset)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        matching.len() == 1,
        "Selected earbud EQ preset is absent or duplicated"
    );
    let setting = Setting {
        mode,
        preset,
        bands: matching[0].current,
    };
    setting.validate()?;
    Ok(Some(setting))
}

pub fn read_equalizer_presets(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Vec<Setting>> {
    let index = workspace_profile_index(device, profile_id)?;
    let Some(control) = workspace_control(device.profiles[index].source_settings.as_ref(), FIELD)?
    else {
        return Ok(Vec::new());
    };
    let document: Equalizer = serde_json::from_value(Value::Object(control.clone()))?;
    let entries: Vec<Entry> = serde_json::from_value(
        document
            .presets
            .modes
            .get(document.selected_mode.key())
            .context("Earbud EQ mode is absent")?
            .clone(),
    )?;
    let mut result = Vec::new();
    for entry in entries {
        anyhow::ensure!(
            !result
                .iter()
                .any(|existing: &Setting| existing.preset == entry.name),
            "Earbud EQ preset is duplicated"
        );
        let setting = Setting {
            mode: document.selected_mode,
            preset: entry.name,
            bands: entry.current,
        };
        setting.validate()?;
        result.push(setting);
    }
    Ok(result)
}

pub fn initial_equalizer_presets(product_id: u32) -> Result<Vec<Setting>> {
    let source = defaults::profile(product_id, "", "")?;
    let document: Equalizer =
        serde_json::from_value(source.get(FIELD).context("Initial EQ absent")?.clone())?;
    let entries: Vec<Entry> = serde_json::from_value(
        document
            .presets
            .modes
            .get("standard")
            .context("Initial EQ mode absent")?
            .clone(),
    )?;
    Ok(entries
        .into_iter()
        .map(|entry| Setting {
            mode: Mode::Standard,
            preset: entry.name,
            bands: entry.current,
        })
        .collect())
}

pub fn equalizer_defaults(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Vec<Setting>> {
    let index = workspace_profile_index(device, profile_id)?;
    let Some(control) = workspace_control(device.profiles[index].source_settings.as_ref(), FIELD)?
    else {
        return Ok(Vec::new());
    };
    let document: Equalizer = serde_json::from_value(Value::Object(control.clone()))?;
    let entries: Vec<Entry> = serde_json::from_value(
        document
            .presets
            .modes
            .get(document.selected_mode.key())
            .context("Earbud EQ mode absent")?
            .clone(),
    )?;
    entries
        .into_iter()
        .map(|entry| {
            let setting = Setting {
                mode: document.selected_mode,
                preset: entry.name,
                bands: entry.default,
            };
            setting.validate()?;
            Ok(setting)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equalizer_merge_retains_defaults_other_presets_extensions_and_failure_atomicity() {
        for pid in [2657, 2660, 2664, 2668, 2689] {
            let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
            device.profiles.clear();
            device.active_profile.clear();
            initialize_auto_pause_profile(&mut device, "eq-profile", "TEST-Default").unwrap();
            let original = read_equalizer(&device, "eq-profile").unwrap().unwrap();
            assert_eq!(original.bands, [2, 2, 3, 4, 1, -1, 2, 3, 3, 3]);
            device.profiles[0].source_settings.as_mut().unwrap()["profile"][FIELD]["future"] =
                serde_json::json!({"keep":7});
            let before = device.profiles[0].source_settings.clone().unwrap();
            let setting = Setting {
                mode: Mode::Standard,
                preset: Preset::Custom,
                bands: [-6, -5, -4, -3, -2, 0, 2, 4, 5, 6],
            };
            merge_equalizer(&mut device, "eq-profile", setting).unwrap();
            assert_eq!(
                read_equalizer(&device, "eq-profile").unwrap(),
                Some(setting)
            );
            let after = device.profiles[0].source_settings.clone().unwrap();
            assert_eq!(
                after["profile"][FIELD]["future"],
                before["profile"][FIELD]["future"]
            );
            assert_eq!(
                after["profile"][FIELD]["presets"]["standard"][0],
                before["profile"][FIELD]["presets"]["standard"][0]
            );
            assert_eq!(
                after["profile"][FIELD]["presets"]["standard"][3]["default"],
                serde_json::to_value([0; 10]).unwrap()
            );
            let invalid = Setting {
                preset: Preset::Voice,
                ..setting
            };
            assert!(merge_equalizer(&mut device, "eq-profile", invalid).is_err());
            assert_eq!(device.profiles[0].source_settings.as_ref(), Some(&after));
            assert_eq!(
                read_equalizer_presets(&device, "eq-profile").unwrap().len(),
                4
            );
        }
    }
}

pub fn merge_equalizer(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    setting: Setting,
) -> Result<()> {
    setting.validate()?;
    read_equalizer(device, profile_id)?.context("Earbud EQ profile is absent")?;
    let index = workspace_profile_index(device, profile_id)?;
    merge_equalizer_source(&mut device.profiles[index].source_settings, setting)
}

pub fn merge_equalizer_source(source: &mut Option<Value>, setting: Setting) -> Result<()> {
    setting.validate()?;
    decode_equalizer(source.as_ref())?.context("Earbud EQ profile is absent")?;
    let field = workspace_control_mut(source, FIELD);
    let mut candidate = Value::Object(field.clone());
    let presets = candidate
        .get_mut("presets")
        .and_then(Value::as_object_mut)
        .context("Earbud EQ presets are not an object")?;
    let entries = presets
        .get_mut(setting.mode.key())
        .and_then(Value::as_array_mut)
        .context("Earbud EQ mode is absent")?;
    let indices = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.get("name").and_then(Value::as_str) == Some(setting.preset.key())
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        indices.len() == 1,
        "Earbud EQ target preset is absent or duplicated"
    );
    entries[indices[0]]
        .as_object_mut()
        .context("Earbud EQ preset is not an object")?
        .insert("current".into(), serde_json::to_value(setting.bands)?);
    presets
        .get_mut("selectedPreset")
        .and_then(Value::as_object_mut)
        .context("Earbud EQ selection is not an object")?
        .insert(
            setting.mode.key().into(),
            serde_json::to_value(setting.preset)?,
        );
    candidate
        .as_object_mut()
        .unwrap()
        .insert("selectedMode".into(), serde_json::to_value(setting.mode)?);
    *field = candidate.as_object().unwrap().clone();
    Ok(())
}
