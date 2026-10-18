//! Strict local profile document contracts; no filesystem or UI ownership.
use crate::{
    macro_library::MacroLibraryFile,
    model::{Device, Profile},
    settings::{LinkedGame, ProfileSettings},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

/// Captured local owner identity for rejecting late profile file results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileContext {
    workspace: String,
    serial_number: String,
    real_product_id: u32,
    layout_id: u32,
    profile: String,
    use_ble: bool,
    connection: Option<crate::model::DeviceConnectionObservation>,
}
impl ProfileContext {
    pub fn from_device(device: &Device) -> Self {
        Self {
            workspace: device.workspace_identity(),
            serial_number: device.serial_number.clone(),
            real_product_id: device.real_product_id,
            layout_id: device.layout_id,
            profile: device.active_profile.clone(),
            use_ble: device.use_ble,
            connection: device.dashboard.connection_observation,
        }
    }
    pub fn matches(&self, device: &Device) -> bool {
        *self == Self::from_device(device)
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SourceExportSelection {
    pub profile_id: String,
    pub macro_guids: Vec<String>,
}
impl SourceExportSelection {
    pub fn new(profile_id: impl Into<String>, macro_guids: Vec<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            macro_guids,
        }
    }
}

/// An immutable local collection change retained until its storage receipt.
/// The request identity belongs to this transaction; it cannot acknowledge
/// another import with equal profile contents.
#[derive(Clone)]
pub struct SourceProfileImport {
    original: Device,
    candidate: Device,
    macros: Option<SourceMacroImport>,
    identity: Arc<()>,
}

#[derive(Clone)]
struct SourceMacroImport {
    catalog: Vec<Value>,
    documents: Vec<Value>,
}

/// The shared macro document written in the same local transaction as profiles.
#[derive(Clone)]
pub struct ProfileMacroImport {
    baseline: MacroLibraryFile,
    candidate: MacroLibraryFile,
    documents: Vec<Value>,
}
impl ProfileMacroImport {
    pub fn matches(&self, current: &MacroLibraryFile) -> bool {
        self.baseline.same_document(current)
    }
    pub fn candidate(&self) -> &MacroLibraryFile {
        &self.candidate
    }
    /// Reapply the admitted append to the current editor document. Local
    /// numeric IDs belong to that document; source macro references use GUIDs.
    pub fn merge_receipt(&self, current: &MacroLibraryFile) -> Result<MacroLibraryFile, String> {
        current.validate()?;
        let mut merged = current.clone();
        merged.append_source_documents(&self.documents)?;
        Ok(merged)
    }
}

impl SourceProfileImport {
    pub fn new(original: &Device, candidate: Device) -> Result<Self, String> {
        if !ProfileContext::from_device(original).matches(&candidate)
            || candidate.profiles.len() <= original.profiles.len()
            || !candidate.profiles.starts_with(&original.profiles)
        {
            return Err(
                "A profile import must append to the captured owner without selecting a profile"
                    .into(),
            );
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut guids = std::collections::BTreeSet::new();
        for profile in &candidate.profiles {
            if profile.id.is_empty()
                || profile.guid.is_empty()
                || !ids.insert(&profile.id)
                || !guids.insert(&profile.guid)
            {
                return Err(
                    "The imported profile collection has an absent or repeated identity".into(),
                );
            }
        }
        for profile in &candidate.profiles[original.profiles.len()..] {
            if profile.id != profile.guid
                || profile.name.is_empty()
                || !profile.source_settings.as_ref().is_some_and(|source| {
                    source.get("guid").and_then(Value::as_str) == Some(&profile.guid)
                        && source.get("name").and_then(Value::as_str) == Some(&profile.name)
                })
            {
                return Err("An imported profile must retain its source identity and name".into());
            }
        }
        Ok(Self {
            original: original.clone(),
            candidate,
            macros: None,
            identity: Arc::new(()),
        })
    }
    pub fn with_source_macros(mut self, catalog: &[Value], documents: Vec<Value>) -> Self {
        self.macros = Some(SourceMacroImport {
            catalog: catalog.to_vec(),
            documents,
        });
        self.identity = Arc::new(());
        self
    }
    pub fn prepare_macros(
        &self,
        current: &MacroLibraryFile,
    ) -> Result<Option<ProfileMacroImport>, String> {
        let Some(source) = &self.macros else {
            return Ok(None);
        };
        if current.mapping_catalog() != source.catalog {
            return Err(
                "The shared macro catalog changed before profile import preparation".into(),
            );
        }
        let mut candidate = current.clone();
        candidate.append_source_documents(&source.documents)?;
        Ok(Some(ProfileMacroImport {
            baseline: current.clone(),
            candidate,
            documents: source.documents.clone(),
        }))
    }
    pub fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
    pub fn matches(&self, current: &Device) -> bool {
        ProfileContext::from_device(&self.original).matches(current)
            && current.profiles == self.original.profiles
            && current.source_device_settings == self.original.source_device_settings
    }
    pub fn candidate(&self) -> &Device {
        &self.candidate
    }
    /// A successful local append does not undo edits made while the writer
    /// was busy. The durable candidate remains a separate save baseline.
    pub fn merge_receipt(&self, current: &Device) -> Result<Device, String> {
        let mut owner = ProfileContext::from_device(current);
        owner.profile = self.original.active_profile.clone();
        if owner != ProfileContext::from_device(&self.original) {
            return Err("The profile import device owner changed during persistence".into());
        }
        let imported = &self.candidate.profiles[self.original.profiles.len()..];
        let mut ids = std::collections::BTreeSet::new();
        let mut guids = std::collections::BTreeSet::new();
        for profile in current.profiles.iter().chain(imported) {
            if profile.id.is_empty()
                || profile.guid.is_empty()
                || !ids.insert(&profile.id)
                || !guids.insert(&profile.guid)
            {
                return Err(
                    "An imported profile identity conflicts with the current collection".into(),
                );
            }
        }
        let mut merged = current.clone();
        merged.profiles.extend_from_slice(imported);
        let imported_guids = imported
            .iter()
            .map(|profile| profile.guid.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        merged.source_device_settings = merge_import_field(
            self.original.source_device_settings.as_ref(),
            self.candidate.source_device_settings.as_ref(),
            current.source_device_settings.as_ref(),
            "device settings",
            &imported_guids,
        )?;
        let active = merged
            .profiles
            .iter()
            .find(|profile| profile.id == merged.active_profile)
            .ok_or("The current profile selection is absent after import")?
            .guid
            .clone();
        if let Some(product) =
            razer_device::mapping_document::MappingProduct::for_product_id(merged.product_id)
        {
            if let Some(document) = merged
                .source_device_settings
                .as_mut()
                .and_then(|settings| settings.get_mut(product.storage_field()))
            {
                let fields = document
                    .as_object_mut()
                    .ok_or("The merged mapping projection is not an object")?;
                fields.insert("activeProfile".into(), active.into());
            }
        }
        Ok(merged)
    }
}

/// Merge only the external source-document delta admitted by the importer.
/// Objects retain unrelated extension fields; GUID collections preserve their
/// current order and edits while appending the import's new identities.
fn merge_import_field(
    baseline: Option<&Value>,
    candidate: Option<&Value>,
    current: Option<&Value>,
    field: &str,
    imported_profiles: &std::collections::BTreeSet<&str>,
) -> Result<Option<Value>, String> {
    if candidate == baseline {
        return Ok(current.cloned());
    }
    if current == baseline {
        return Ok(candidate.cloned());
    }
    // Import appends profiles without selecting one. A projection first
    // materialized during the write still belongs to the current selection.
    if field == "activeProfile"
        && candidate.and_then(Value::as_str).is_some()
        && current.and_then(Value::as_str).is_some()
    {
        return Ok(current.cloned());
    }
    if let (Some(Value::Object(candidate)), Some(Value::Object(current))) = (candidate, current) {
        let baseline = match baseline {
            Some(Value::Object(fields)) => Some(fields),
            None => None,
            _ => return Err(format!("Profile import conflicts at {field}")),
        };
        let mut merged = current.clone();
        let keys = candidate
            .keys()
            .chain(baseline.into_iter().flat_map(|fields| fields.keys()))
            .collect::<std::collections::BTreeSet<_>>();
        for key in keys {
            match merge_import_field(
                baseline.and_then(|fields| fields.get(key)),
                candidate.get(key),
                current.get(key),
                key,
                imported_profiles,
            )? {
                Some(value) => {
                    merged.insert(key.clone(), value);
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        return Ok(Some(Value::Object(merged)));
    }
    if matches!(field, "profiles" | "synapseTurbos") {
        if let (Some(Value::Array(candidate)), Some(Value::Array(current))) = (candidate, current) {
            let baseline = match baseline {
                Some(Value::Array(rows)) => rows.as_slice(),
                None => &[],
                _ => return Err(format!("Profile import conflicts at {field}")),
            };
            return merge_import_collection(baseline, candidate, current, field, imported_profiles)
                .map(|rows| Some(Value::Array(rows)));
        }
    }
    if current == candidate {
        return Ok(current.cloned());
    }
    Err(format!("Profile import conflicts at {field}"))
}

fn merge_import_collection(
    baseline: &[Value],
    candidate: &[Value],
    current: &[Value],
    field: &str,
    imported_profiles: &std::collections::BTreeSet<&str>,
) -> Result<Vec<Value>, String> {
    fn index<'a>(
        rows: &'a [Value],
        field: &str,
    ) -> Result<std::collections::BTreeMap<String, &'a Value>, String> {
        let mut indexed = std::collections::BTreeMap::new();
        for row in rows {
            let guid = row
                .get("guid")
                .and_then(Value::as_str)
                .filter(|guid| !guid.is_empty())
                .ok_or_else(|| format!("Profile import {field} identity is absent"))?;
            if indexed.insert(guid.to_owned(), row).is_some() {
                return Err(format!("Profile import {field} identity is repeated"));
            }
        }
        Ok(indexed)
    }
    let baseline = index(baseline, field)?;
    let candidate_index = index(candidate, field)?;
    let current_index = index(current, field)?;
    let mut merged = Vec::new();
    for row in current {
        let guid = row["guid"].as_str().expect("validated source identity");
        match (baseline.get(guid), candidate_index.get(guid)) {
            (None, Some(_)) if field == "profiles" && !imported_profiles.contains(guid) => {
                merged.push(row.clone());
            }
            (None, Some(_)) => {
                return Err(format!(
                    "An imported {field} GUID conflicts with the current collection"
                ));
            }
            (Some(original), Some(imported)) if field != "profiles" => {
                if let Some(row) = merge_import_field(
                    Some(original),
                    Some(imported),
                    Some(row),
                    field,
                    imported_profiles,
                )? {
                    merged.push(row);
                }
            }
            (Some(original), None) if field != "profiles" && row == *original => (),
            (Some(_), None) if field != "profiles" => {
                return Err(format!(
                    "Profile import conflicts with an edited {field} entry"
                ));
            }
            _ => merged.push(row.clone()),
        }
    }
    for row in candidate {
        let guid = row["guid"].as_str().expect("validated source identity");
        if !baseline.contains_key(guid)
            && !current_index.contains_key(guid)
            && (field != "profiles" || imported_profiles.contains(guid))
        {
            merged.push(row.clone());
        }
    }
    Ok(merged)
}

/// Capture the selected vendor payloads at the profile-file boundary. The
/// workspace's stable identity and name override stale mirrored metadata;
/// absent source settings never become an empty export document.
pub fn source_profile_payloads(device: &Device, ids: &[String]) -> Result<Vec<Value>, String> {
    if ids.is_empty() {
        return Err("No profiles were selected for transfer".into());
    }
    let mut selected = std::collections::BTreeSet::new();
    ids.iter()
        .map(|id| {
            if !selected.insert(id) {
                return Err("A profile was selected more than once".into());
            }
            let profile = device
                .profiles
                .iter()
                .find(|profile| &profile.id == id)
                .ok_or("A selected profile was removed")?;
            if profile.guid.is_empty() || profile.name.is_empty() {
                return Err("A selected profile has no source identity or name".into());
            }
            let mut source = profile
                .source_settings
                .clone()
                .filter(Value::is_object)
                .ok_or("The selected profile has no source settings to transfer")?;
            let fields = source.as_object_mut().expect("validated source profile");
            fields.insert("guid".into(), profile.guid.clone().into());
            fields.insert("name".into(), profile.name.clone().into());
            Ok(source)
        })
        .collect()
}

pub fn source_export_payloads(
    device: &Device,
    selection: &[SourceExportSelection],
    macros: &[Value],
    rule: razer_device::profile_transfer::ProfileExportRule,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let ids = selection
        .iter()
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let profiles = source_profile_payloads(device, &ids)?;
    if !rule.includes_macros() {
        return Ok((profiles, Vec::new()));
    }
    let mut selected = std::collections::BTreeSet::new();
    let mut exported = Vec::new();
    for row in selection {
        for guid in &row.macro_guids {
            if selected.insert(guid) {
                if let Some(mac) = macros
                    .iter()
                    .find(|mac| mac.get("guid").and_then(Value::as_str) == Some(guid.as_str()))
                {
                    exported.push(mac.clone());
                } else if rule.requires_selected_macros() {
                    return Err(format!(
                        "The selected macro {guid} is absent from the shared library"
                    ));
                }
            }
        }
    }
    Ok((profiles, exported))
}

pub fn executable_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

pub fn valid_game(game: &LinkedGame) -> bool {
    let path = game.executable.as_str();
    let bytes = path.as_bytes();
    let absolute = Path::new(path).is_absolute()
        || bytes.len() > 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/')
        || path.starts_with("\\\\");
    let file = path.rsplit(['/', '\\']).next().unwrap_or_default();
    !game.name.trim().is_empty()
        && game.name.encode_utf16().count() <= 128
        && !game.name.chars().any(char::is_control)
        && absolute
        && path.len() <= 4096
        && !path.chars().any(char::is_control)
        && file.len() > 4
        && file.to_ascii_lowercase().ends_with(".exe")
}

const FORMAT: &str = "razer-ui-profile";
const VERSION: u32 = 1;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDocument {
    format: String,
    version: u32,
    product_id: u32,
    layout_id: u32,
    profile: ProfileContent,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileContent {
    name: String,
    settings: Value,
}

pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.trim() == name
        && name.encode_utf16().count() <= 32
        && !name.chars().any(char::is_control)
}

pub fn import_name(profiles: &[Profile], original: &str) -> String {
    if !profiles
        .iter()
        .any(|profile| profile.name.eq_ignore_ascii_case(original))
    {
        return original.into();
    }
    (1..)
        .map(|index| {
            let suffix = format!(" ({index})");
            let mut base = original.to_string();
            while base.encode_utf16().count() + suffix.len() > 32 {
                base.pop();
            }
            format!("{}{suffix}", base.trim_end())
        })
        .find(|name| {
            !profiles
                .iter()
                .any(|profile| profile.name.eq_ignore_ascii_case(name))
        })
        .expect("an unused import name")
}

fn validate_settings(settings: &ProfileSettings, pid: u32) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.normalize(pid);
    if normalized != *settings {
        return Err("配置包含超出范围或不完整的设置。".into());
    }
    if ![125, 250, 500, 1000, 2000, 4000, 8000].contains(&settings.polling) {
        return Err("配置包含无效的轮询率。".into());
    }
    for bindings in [&settings.bindings, &settings.hypershift_bindings] {
        if bindings.len() > 512
            || bindings.iter().any(|(key, value)| {
                key.is_empty()
                    || key.len() > 128
                    || key.chars().any(char::is_control)
                    || value.len() > 65536
            })
        {
            return Err("配置中的按键映射超出限制。".into());
        }
    }
    let mut paths = std::collections::BTreeSet::new();
    if settings.linked_games.len() > 128
        || settings
            .linked_games
            .iter()
            .any(|game| !valid_game(game) || !paths.insert(executable_key(&game.executable)))
    {
        return Err("配置包含无效或重复的关联程序。".into());
    }
    if [&settings.audio, &settings.mic]
        .iter()
        .any(|eq| eq.bands.len() > 64 || eq.custom.len() > 64)
    {
        return Err("配置中的均衡器频段数量超出限制。".into());
    }
    let keyboard = &settings.keyboard;
    let pairs = keyboard.snap_key_pairs();
    let mut keys = std::collections::BTreeSet::new();
    if pairs.is_empty()
        || pairs.len() > 4
        || keyboard.snap_keys != pairs[0]
        || pairs.iter().flatten().any(|key| {
            crate::settings::canonical_snap_key(key).as_ref() != Some(key) || !keys.insert(key)
        })
    {
        return Err("配置包含无效或重复的 Snap Tap 按键。".into());
    }
    let defaults = crate::settings::Keyboard::default();
    let mut modes = std::collections::BTreeSet::new();
    if keyboard.dial_modes.len() > defaults.dial_modes.len() + 100
        || !keyboard.dial_selection_valid()
        || defaults.dial_modes.iter().any(|default| {
            !keyboard
                .dial_modes
                .iter()
                .any(|mode| !mode.is_custom && mode.uid == default.uid)
        })
        || keyboard.dial_modes.iter().any(|mode| {
            mode.uid.is_empty()
                || mode.uid.len() > 128
                || !modes.insert(&mode.uid)
                || mode.uid.chars().any(char::is_control)
                || !mode.is_custom
                    && !defaults
                        .dial_modes
                        .iter()
                        .any(|default| default.uid == mode.uid)
                || mode.is_custom
                    && (mode.name.trim().is_empty()
                        || mode.name.encode_utf16().count() > 40
                        || mode.name.chars().any(char::is_control))
                || mode.mappings.iter().any(|(input, value)| {
                    !["ScrollLeft", "ScrollRight"].contains(&input.as_str()) || value.len() > 65536
                })
        })
    {
        return Err("配置包含无效的拨轮模式或映射。".into());
    }
    Ok(())
}

// Reject duplicate JSON keys at every depth instead of silently accepting the
// last occurrence. The ordinary serde_json recursion limit also remains active.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Value;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
                serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Value, E> {
                Ok(value.into())
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
                Ok(Value::Null)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = seq.next_element()? {
                    values.push(value);
                }
                Ok(Value::Array(values))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, UniqueValue(value))) =
                    map.next_entry::<String, UniqueValue>()?
                {
                    if values.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                }
                Ok(Value::Object(values))
            }
        }
        deserializer.deserialize_any(Visitor).map(Self)
    }
}

pub fn decode_profile(bytes: &[u8], pid: u32, layout: u32) -> Result<Profile, String> {
    if bytes.len() > MAX_BYTES {
        return Err("配置文件不得大于 2 MiB。".into());
    }
    let UniqueValue(value) =
        serde_json::from_slice(bytes).map_err(|error| format!("无法读取配置 JSON：{error}"))?;
    let document: ProfileDocument = serde_json::from_value(value)
        .map_err(|_| "请选择本项目导出的 .razer-ui-profile.json 文件。".to_string())?;
    if document.format != FORMAT {
        return Err("此文件不是本项目的配置格式。".into());
    }
    if document.version != VERSION {
        return Err(format!("不支持配置格式版本 {}。", document.version));
    }
    if document.product_id != pid || document.layout_id != layout {
        return Err("此配置的设备型号或键盘布局与当前设备不符。".into());
    }
    if !valid_name(&document.profile.name) {
        return Err("配置名称须为 1–32 个字符，且不能包含控制字符。".into());
    }
    let settings: ProfileSettings = serde_json::from_value(document.profile.settings.clone())
        .map_err(|error| format!("配置设置无效：{error}"))?;
    // The store accepts older local settings with defaults. The transfer format
    // is stricter: compare to the current serializer to reject unknown/missing
    // fields recursively without maintaining a second schema or dropping data.
    if serde_json::to_value(&settings).map_err(|error| error.to_string())?
        != document.profile.settings
    {
        return Err("配置设置包含未知、缺失或不兼容的字段。".into());
    }
    validate_settings(&settings, pid)?;
    Ok(Profile {
        source_settings: None,
        id: String::new(),
        guid: String::new(),
        name: document.profile.name,
        dpi_stages: None,
        settings: Some(settings),
    })
}

pub fn encode_profile(device: &Device, profile: &Profile) -> Result<Vec<u8>, String> {
    if !valid_name(&profile.name) {
        return Err("请先将配置文件重命名为有效名称。".into());
    }
    let settings = profile.settings.as_ref().ok_or("配置设置尚未加载。")?;
    validate_settings(settings, device.product_id)?;
    let document = ProfileDocument {
        format: FORMAT.into(),
        version: VERSION,
        product_id: device.product_id,
        layout_id: device.layout_id,
        profile: ProfileContent {
            name: profile.name.clone(),
            settings: serde_json::to_value(settings).map_err(|error| error.to_string())?,
        },
    };
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("配置内容超出 2 MiB 导出限制。".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
