//! Saved ordinary keyboard engines and their temporary receiver publication.
use crate::{
    keyboard_current_profile::{GamingMode, Mapping, SnapTap},
    keyboard_gaming_mapping::{
        GamingMappingContext, GamingModeSettings, KeyboardGamingMappingPlan,
    },
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[path = "keyboard_receiver_defaults.rs"]
mod defaults;
#[cfg(test)]
#[path = "keyboard_receiver_mapping_tests.rs"]
mod tests;

pub const STORAGE_FIELD: &str = "_keyboard_receiver_mapping_document";
pub const PRODUCTS: [u32; 9] = [600, 602, 658, 664, 697, 716, 717, 724, 727];

pub const fn supports(product: u32) -> bool {
    matches!(product, 600 | 602 | 658 | 664 | 697 | 716 | 717 | 724 | 727)
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
enum FunctionPrimary {
    #[serde(rename = "func")]
    Function,
    #[serde(rename = "multi")]
    Multimedia,
}

#[derive(Clone, Deserialize, Serialize)]
struct Engine {
    mappings: Vec<Value>,
    hash: String,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

impl Engine {
    fn rehash(&mut self) -> anyhow::Result<()> {
        self.hash = source_hash(&serde_json::to_value(&*self)?)?;
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Defaults {
    #[serde(rename = "appEngine")]
    engine: Engine,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    guid: String,
    name: String,
    mappings: Vec<Mapping>,
    gaming_mode: GamingMode,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    function_key_primary: Option<FunctionPrimary>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    snap_tap: Option<SnapTap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app_engine: Option<Engine>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

impl Profile {
    fn validate(&self) -> anyhow::Result<()> {
        self.gaming_mode.validate()?;
        if let Some(snap) = &self.snap_tap {
            snap.validate()?;
        }
        ensure!(
            self.extensions.get("modTap").is_none_or(Value::is_boolean),
            "Receiver keyboard modTap has an invalid type"
        );
        ensure!(
            self.gaming_mode
                .extensions
                .get("isFnSleepEnabled")
                .is_none_or(Value::is_boolean),
            "Receiver keyboard Fn sleep has an invalid type"
        );
        ensure!(
            self.gaming_mode
                .extensions
                .get("scancode")
                .is_none_or(|value| value.as_u64().is_some_and(|value| value <= u16::MAX.into())),
            "Receiver keyboard Fn sleep scancode is invalid"
        );
        for mapping in &self.mappings {
            mapping.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    active_profile_guid: String,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    version: u64,
    vendor_id: u16,
    product_id: u32,
    category: String,
    #[serde(rename = "reportIDs")]
    report_ids: BTreeMap<u8, String>,
    profiles: Vec<Profile>,
    active_profile: String,
    default_mappings: Defaults,
    default_turbos: Vec<Value>,
    synapse_turbos: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    device_metadatas: Option<BTreeMap<String, Metadata>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    game_mode_enabled: Option<bool>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

impl Document {
    fn parse(value: &Value) -> anyhow::Result<Self> {
        let document: Self = serde_json::from_value(value.clone())?;
        ensure!(
            supports(document.product_id)
                && document.vendor_id == 5426
                && document.category == "KEYBOARD",
            "Receiver keyboard document identity differs"
        );
        let mut ids = BTreeSet::new();
        ensure!(
            !document.profiles.is_empty()
                && document
                    .profiles
                    .iter()
                    .all(|profile| !profile.guid.is_empty() && ids.insert(&profile.guid)),
            "Receiver keyboard profiles are absent or duplicated"
        );
        ensure!(
            document
                .profiles
                .iter()
                .any(|profile| profile.guid == document.active_profile),
            "Receiver keyboard active profile is absent"
        );
        for profile in &document.profiles {
            profile.validate()?;
        }
        ensure!(
            document
                .extensions
                .get("analogData")
                .is_none_or(Value::is_object),
            "Receiver keyboard analog defaults are malformed"
        );
        Ok(document)
    }

    fn selected(&self, serial: &str) -> Option<usize> {
        match self
            .device_metadatas
            .as_ref()
            .and_then(|metadata| metadata.get(serial))
        {
            Some(metadata) => self
                .profiles
                .iter()
                .position(|profile| profile.guid == metadata.active_profile_guid),
            None => (!self.profiles.is_empty()).then_some(0),
        }
    }
}

fn factory() -> anyhow::Result<MappingEngine> {
    // All three admitted factories use the same verified key/turbo tables.
    MappingEngine::new(515)
}

fn is_function(factory: &MappingEngine, id: &str) -> bool {
    factory
        .source_keys()
        .iter()
        .any(|key| key.input_id == Some(id) && key.kind == "Function")
}

fn compiled_profile(
    factory: &MappingEngine,
    profile: &Profile,
    layout: u32,
    context: &mut EngineContext,
) -> anyhow::Result<Engine> {
    let mut records = Vec::new();
    let use_backslash = layout == 12 || [1, 20, 5, 8, 11, 2, 9, 19, 21].contains(&layout);
    for source in &profile.mappings {
        source.validate()?;
        if source.output_type.as_deref() == Some("modTapGroup")
            || source.input_id
                == if use_backslash {
                    "KEY_NON_US_POUND"
                } else {
                    "KEY_BACKSLASH"
                }
        {
            continue;
        }
        if let Some(primary) = profile.function_key_primary
            && is_function(factory, &source.input_id)
            && source.is_hyper_shift.unwrap_or(false) == (primary == FunctionPrimary::Function)
        {
            continue;
        }
        let mut record = serde_json::to_value(source)?;
        if source.output_type.as_deref() == Some("keyboardGroup") {
            let empty = record
                .get("keyboardGroup")
                .is_none_or(|group| group.as_object().is_some_and(Map::is_empty));
            if empty {
                record["keyboardGroup"] =
                    json!({"key":source.input_id,"keyGroup":0,"modifiers":[]});
            }
        }
        records.push(record);
    }
    let (analog, ordinary): (Vec<_>, Vec<_>) = records
        .into_iter()
        .partition(|record| record["inputType"] == "AnalogInput");
    let mut engine: Engine =
        serde_json::from_value(factory.generate(&ordinary, context)?.app_engine)?;
    if !analog.is_empty() {
        let generated = factory.generate_analog(&analog, context)?.app_engine;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Analog {
            mappings: Vec<Vec<Value>>,
            actuations: Vec<Value>,
            rapid_triggers: Map<String, Value>,
        }
        let generated: Analog = serde_json::from_value(generated)?;
        for mapping in generated.mappings.into_iter().flatten() {
            let input = &mapping["input"];
            let output = &mapping["output"];
            if input["type"] == "analogKey"
                && output["type"] == "keyboard"
                && input["scancode"] == output["scancode"]
                && input["hypershift"] != true
            {
                continue;
            }
            engine.mappings.push(mapping);
        }
        if !generated.actuations.is_empty() {
            engine
                .extensions
                .insert("actuations".into(), generated.actuations.into());
        }
        if !generated.rapid_triggers.is_empty() {
            engine
                .extensions
                .insert("rapidTriggers".into(), generated.rapid_triggers.into());
        }
    }
    engine.rehash()?;
    Ok(engine)
}

/// Compile committed profiles without storing temporary Gaming/Snap Tap rows.
/// The caller persists this document before the physical receiver publishes it.
pub fn prepare_document(
    product: u32,
    profiles: &[Value],
    active: &str,
    layout: u32,
    previous: Option<&Value>,
    settings: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> anyhow::Result<Value> {
    ensure!(
        supports(product),
        "Receiver keyboard compiler has no current product source"
    );
    let previous = previous.map(Document::parse).transpose()?;
    ensure!(
        previous
            .as_ref()
            .is_none_or(|old| old.product_id == product),
        "Previous receiver keyboard document belongs to another product"
    );
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Settings {
        #[serde(default)]
        device_metadatas: Option<BTreeMap<String, Metadata>>,
        #[serde(default, deserialize_with = "present")]
        game_mode_enabled: Option<bool>,
        #[serde(flatten)]
        extensions: Map<String, Value>,
    }
    let settings: Settings = serde_json::from_value(
        settings
            .context("Receiver keyboard settings have not been loaded")?
            .clone(),
    )?;
    let factory = factory()?;
    let mut context = EngineContext {
        default_turbos: previous.as_ref().map(|old| old.default_turbos.clone()),
        synapse_turbos: Some(
            previous
                .as_ref()
                .map(|old| old.synapse_turbos.clone())
                .unwrap_or_default(),
        ),
        fresh_guids: fresh_guids.into_iter().collect(),
        ..Default::default()
    };
    for key in defaults::dkm_keys(product) {
        context
            .dkm_keys
            .insert(key.id.into(), serde_json::to_value(key)?);
    }
    if context.default_turbos.as_ref().is_none_or(Vec::is_empty) {
        factory.generate_default_turbos(&mut context)?;
    }
    let mut profiles: Vec<Profile> = profiles
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()?;
    let mut ids = BTreeSet::new();
    ensure!(
        profiles
            .iter()
            .all(|profile| !profile.guid.is_empty() && ids.insert(profile.guid.clone()))
            && ids.contains(active),
        "Receiver keyboard active profile is absent or duplicate"
    );
    let active_profile = profiles
        .iter()
        .find(|profile| profile.guid == active)
        .unwrap();
    let mut default_rows = defaults::mappings(product)?;
    if let Some(primary) = active_profile.function_key_primary {
        for record in &mut default_rows {
            if record["inputID"]
                .as_str()
                .is_some_and(|id| is_function(&factory, id))
            {
                record["isHyperShift"] = (primary == FunctionPrimary::Function).into();
            }
        }
    }
    let mut default_engine: Engine =
        serde_json::from_value(factory.generate(&default_rows, &mut context)?.app_engine)?;
    for hypershift in [false, true] {
        for flag in [0, 1] {
            default_engine.mappings.push(json!({"input":{"type":"razerKey","key":1,"hypershift":hypershift,"flag":flag},"output":{"type":"hypershift","flag":flag}}));
        }
    }
    default_engine.rehash()?;
    for profile in &mut profiles {
        profile.validate()?;
        let replaced = previous
            .as_ref()
            .and_then(|old| old.profiles.iter().find(|old| old.guid == profile.guid))
            .and_then(|old| old.app_engine.as_ref())
            .into_iter()
            .flat_map(|engine| &engine.mappings)
            .filter(|row| row["output"]["type"] == "turbo")
            .filter_map(|row| row["output"]["guid"].as_str().map(str::to_owned))
            .collect::<BTreeSet<_>>();
        context.synapse_turbos.as_mut().unwrap().retain(|turbo| {
            !turbo["guid"]
                .as_str()
                .is_some_and(|guid| replaced.contains(guid))
        });
        profile.app_engine = Some(compiled_profile(&factory, profile, layout, &mut context)?);
    }
    let mut extensions = settings.extensions;
    extensions.retain(|name, _| {
        !crate::mapping_document::MappingProduct::owns_storage_field(name)
            && name != STORAGE_FIELD
            && !matches!(
                name.as_str(),
                "version"
                    | "vendorId"
                    | "productId"
                    | "category"
                    | "reportIDs"
                    | "profiles"
                    | "activeProfile"
                    | "defaultMappings"
                    | "defaultTurbos"
                    | "synapseTurbos"
            )
    });
    if let Some(old) = &previous {
        for (name, value) in &old.extensions {
            extensions
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
    }
    let document = Document {
        version: crate::mapping_submission::next_source_version(
            previous.as_ref().map(|old| old.version),
        )?,
        vendor_id: 5426,
        product_id: product,
        category: "KEYBOARD".into(),
        report_ids: BTreeMap::from([
            (4, "razerKeyReportID".into()),
            (5, "hardwareEventReportID".into()),
        ]),
        profiles,
        active_profile: active.into(),
        default_mappings: Defaults {
            engine: default_engine,
        },
        default_turbos: context
            .default_turbos
            .context("Default keyboard turbos are absent")?,
        synapse_turbos: context
            .synapse_turbos
            .context("Keyboard turbo catalog is absent")?,
        device_metadatas: settings.device_metadatas,
        game_mode_enabled: settings.game_mode_enabled,
        extensions,
    };
    Ok(serde_json::to_value(document)?)
}

/// Save the selected profile only for the currently observed serial.
pub fn select_profile_for_serial(
    settings: &Value,
    serial: &str,
    guid: &str,
) -> anyhow::Result<Value> {
    ensure!(
        !serial.is_empty() && !guid.is_empty(),
        "Keyboard profile selection lacks an observed serial or GUID"
    );
    #[derive(Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Selection {
        #[serde(default)]
        device_metadatas: BTreeMap<String, Metadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_profile: Option<String>,
        #[serde(flatten)]
        extensions: Map<String, Value>,
    }
    let mut selection: Selection = serde_json::from_value(settings.clone())?;
    let metadata = selection
        .device_metadatas
        .entry(serial.into())
        .or_insert_with(|| Metadata {
            active_profile_guid: guid.into(),
            extensions: Map::new(),
        });
    metadata.active_profile_guid = guid.into();
    selection.active_profile = Some(guid.into());
    Ok(serde_json::to_value(selection)?)
}

/// The selected GUID is captured before Snap Tap moves the active row.
pub struct PeerDocument {
    document: Value,
    selected_profile: Option<String>,
}
impl PeerDocument {
    pub fn document(&self) -> &Value {
        &self.document
    }
    pub fn selected_profile(&self) -> Option<&str> {
        self.selected_profile.as_deref()
    }
}

pub fn prepare_peer_document(
    master: u32,
    document: &Value,
    serial: &str,
    running_games: Option<usize>,
) -> anyhow::Result<PeerDocument> {
    ensure!(
        crate::mouse_receiver_mapping::supports_ordinary(master),
        "Receiver keyboard transform has no admitted master compiler"
    );
    ensure!(
        !serial.is_empty(),
        "Receiver keyboard serial was not observed"
    );
    let mut document = Document::parse(document)?;
    let active = document
        .profiles
        .iter()
        .rposition(|profile| profile.guid == document.active_profile)
        .context("Active keyboard profile is missing")?;
    let profile = &document.profiles[active];
    let engine = profile
        .app_engine
        .as_ref()
        .context("Saved keyboard engine is missing")?;
    let compose_f4 = document.default_mappings.engine.mappings.iter().any(|row| {
        row["input"]["scancode"] == 5
            && row["input"]["hypershift"] == true
            && row["output"]["type"] == "keyboard"
            && row["output"]["scancode"] == 62
    });
    let mode = &profile.gaming_mode;
    let plan = KeyboardGamingMappingPlan::prepare_receiver(
        &serde_json::to_value(engine)?,
        &serde_json::to_value(&document.default_mappings.engine)?,
        GamingModeSettings {
            state: mode.state,
            is_windows_key_disabled: mode.is_windows_key_disabled,
            is_alt_tab_disabled: mode.is_alt_tab_disabled,
            is_alt_f4_disabled: mode.is_alt_f4_disabled,
            is_alt_space_disabled: mode.is_alt_space_disabled.unwrap_or(false),
            is_alt_enter_disabled: mode
                .extensions
                .get("isAltEnterDisabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            is_fn_sleep_enabled: mode
                .extensions
                .get("isFnSleepEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            scancode: mode
                .extensions
                .get("scancode")
                .and_then(Value::as_u64)
                .map(|value| value as u16),
        },
        GamingMappingContext {
            game_mode_enabled: document.game_mode_enabled,
            running_games,
            compose_f4,
            mod_tap: profile.extensions.get("modTap") == Some(&Value::Bool(true)),
            ..Default::default()
        },
        || Ok(()),
    )?;
    if plan.is_temporary() {
        let mut profile = document.profiles.remove(active);
        profile.app_engine = Some(serde_json::from_value(plan.active_engine().clone())?);
        document.default_mappings.engine = serde_json::from_value(plan.default_engine().clone())?;
        document.profiles.push(profile);
    }
    let selected_profile = document
        .selected(serial)
        .map(|index| document.profiles[index].guid.clone());
    let snap_enabled = document
        .selected(serial)
        .and_then(|index| document.profiles[index].snap_tap.as_ref())
        .is_some_and(|snap| snap.is_enabled);
    if snap_enabled {
        apply_snap_tap(master, &mut document)?;
    }
    Ok(PeerDocument {
        document: serde_json::to_value(document)?,
        selected_profile,
    })
}

fn apply_snap_tap(master: u32, document: &mut Document) -> anyhow::Result<()> {
    let active = document
        .profiles
        .iter()
        .rposition(|profile| profile.guid == document.active_profile)
        .context("Snap Tap active profile is missing")?;
    let profile = &document.profiles[active];
    let Some(snap) = profile.snap_tap.as_ref().filter(|snap| snap.is_enabled) else {
        return Ok(());
    };
    let pairs = snap.key_list.clone();
    let multi = profile.function_key_primary == Some(FunctionPrimary::Multimedia);
    let mut profile = document.profiles.remove(active);
    let engine = profile
        .app_engine
        .as_mut()
        .context("Snap Tap keyboard engine is absent")?;
    let factory = factory()?;
    let analog = document.extensions.contains_key("analogData");
    for pair in pairs {
        for name in [&pair.key1, &pair.key2] {
            let dkm = crate::mouse_receiver_mapping::receiver_dkm_key(master, name);
            let key = factory
                .source_keys()
                .iter()
                .find(|key| key.input_id == Some(name));
            if dkm.is_none() && key.is_none() {
                break;
            }
            let scancode = if dkm.is_some() {
                None
            } else {
                key.map(|key| key.scancode)
            };
            let flag = if dkm.is_some() { 0 } else { key.unwrap().flag };
            let matches = |row: &Value| match dkm {
                Some(key) => row["input"]["key"] == key,
                None => scancode.is_some_and(|scancode| {
                    scancode != 0
                        && row["input"]["hypershift"] != true
                        && row["input"]["scancode"] == scancode
                        && row["input"]["flag"].as_u64().is_some_and(|value| {
                            value == u64::from(flag) || value == u64::from(flag + 1)
                        })
                }),
            };
            let found = engine.mappings.iter().any(&matches);
            let mut copied = false;
            if multi {
                for row in &document.default_mappings.engine.mappings {
                    let same_scancode = match scancode {
                        Some(scancode) => row["input"]["scancode"] == scancode,
                        None => row["input"].get("scancode").is_none(),
                    };
                    if !same_scancode
                        || !row["input"]["flag"].as_u64().is_some_and(|value| {
                            value == u64::from(flag) || value == u64::from(flag + 1)
                        })
                    {
                        continue;
                    }
                    copied = true;
                    if !engine.mappings.iter().any(|existing| {
                        ["type", "scancode", "flag", "hypershift"]
                            .iter()
                            .all(|field| existing["input"][field] == row["input"][field])
                    }) {
                        engine.mappings.push(row.clone());
                    }
                }
            }
            for row in &mut engine.mappings {
                if matches(row) {
                    row["input"]["snaptapId"] = pair.id.into();
                }
            }
            if !found && (!multi || !copied) {
                for flag in [flag, flag + 1] {
                    let field = if dkm.is_some() { "key" } else { "scancode" };
                    let value = dkm.map(u32::from).or(scancode).unwrap();
                    engine.mappings.push(json!({"input":{"type":if dkm.is_some(){"razerKey"}else if analog{"analogKey"}else{"keyboard"},(field):value,"flag":flag,"snaptapId":pair.id},"output":{"type":"keyboard",(field):value,"flag":flag}}));
                }
            }
        }
    }
    engine.rehash()?;
    document.profiles.push(profile);
    Ok(())
}
