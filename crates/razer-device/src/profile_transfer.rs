//! Current756 UI55714/31867 and MW50574.ry. A .synapse4 file contains
//! Base64 UTF-8 JSON and per-entry hashes; parsing never applies a profile.
use anyhow::{Context as _, Result, ensure};
use html5ever::{LocalName, QualName, namespace_url, ns, parse_fragment, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde::Deserialize;
use serde_json::{Value, json};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TransferTarget {
    pub product_id: u32,
    pub dongle_id: Option<u32>,
    pub ble_id: Option<u32>,
    pub category: TransferCategory,
    #[serde(default, rename = "is8kAnalogDevice")]
    pub is_eight_k_analog_device: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferCategory {
    Accessory,
    Audio,
    Broadcaster,
    Case,
    Chair,
    Egpu,
    Gamepad,
    Hue,
    Iot,
    Keyboard,
    Keypad,
    Monitor,
    Mouse,
    Mousemat,
    Mouseplusmat,
    System,
}
impl TransferCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accessory => "ACCESSORY",
            Self::Audio => "AUDIO",
            Self::Broadcaster => "BROADCASTER",
            Self::Case => "CASE",
            Self::Chair => "CHAIR",
            Self::Egpu => "EGPU",
            Self::Gamepad => "GAMEPAD",
            Self::Hue => "HUE",
            Self::Iot => "IOT",
            Self::Keyboard => "KEYBOARD",
            Self::Keypad => "KEYPAD",
            Self::Monitor => "MONITOR",
            Self::Mouse => "MOUSE",
            Self::Mousemat => "MOUSEMAT",
            Self::Mouseplusmat => "MOUSEPLUSMAT",
            Self::System => "SYSTEM",
        }
    }
}
impl TransferTarget {
    pub fn matches_category(&self, category: Option<&str>) -> bool {
        category == Some(self.category.as_str())
    }

    pub fn matches_product(&self, product: Option<f64>) -> bool {
        product.is_some_and(|product| {
            [Some(self.product_id), self.dongle_id, self.ble_id]
                .into_iter()
                .flatten()
                .any(|identity| f64::from(identity) == product)
        })
    }
}

#[non_exhaustive]
pub struct ProfileTransferProduct {
    pub mounted_product_id: u32,
    pub target: TransferTarget,
    pub export_target: TransferTarget,
    pub export_rule: ProfileExportRule,
    pub device_name: Option<&'static str>,
}

#[path = "profile_transfer/blackshark.rs"]
mod blackshark;
#[path = "profile_transfer/mouse.rs"]
mod mouse;
#[path = "profile_transfer/mouse_defaults.rs"]
mod mouse_defaults;
#[path = "profile_transfer/targets.rs"]
mod targets;
pub use blackshark::{PreparedGlobalImport1398, prepare_global_import_1398};
pub use mouse::{
    MouseImportContext, PreparedMouseImport, prepare_global_mouse_import, prepare_mouse_import,
};
pub use mouse_defaults::{default_device_182, default_profile_182};

pub fn product_target(product_id: u32) -> Option<&'static ProfileTransferProduct> {
    targets::PRODUCTS
        .binary_search_by_key(&product_id, |product| product.mounted_product_id)
        .ok()
        .map(|index| &targets::PRODUCTS[index])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileExportRule {
    ProfilesOnly,
    ProfilesWithSelectedMacros,
    ProfilesWithRequiredSelectedMacros,
}
impl ProfileExportRule {
    pub fn includes_macros(self) -> bool {
        self != Self::ProfilesOnly
    }
    pub fn requires_selected_macros(self) -> bool {
        self == Self::ProfilesWithRequiredSelectedMacros
    }
}
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct DecodedFile {
    pub profiles: Vec<Value>,
    pub macros: Vec<Value>,
    pub device_name: String,
    pub warning: bool,
}
impl DecodedFile {
    pub fn profiles(&self) -> &[Value] {
        &self.profiles
    }
    pub fn macros(&self) -> &[Value] {
        &self.macros
    }
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
    pub fn has_warning(&self) -> bool {
        self.warning
    }
}

/// UI55714 recursively reads the textContent of a detached div after assigning
/// innerHTML. Parse a fragment with the same HTML context; no script is run.
pub fn decode_xml_entities(value: &Value) -> Value {
    fn collect(node: &Handle, result: &mut String) {
        if let NodeData::Text { contents } = &node.data {
            result.push_str(&contents.borrow());
        }
        for child in node.children.borrow().iter() {
            collect(child, result);
        }
    }
    match value {
        Value::String(value) => {
            let dom = parse_fragment(
                RcDom::default(),
                Default::default(),
                QualName::new(None, ns!(html), LocalName::from("div")),
                Vec::new(),
            )
            .one(value.clone());
            let mut result = String::new();
            collect(&dom.document, &mut result);
            Value::String(result)
        }
        Value::Array(rows) => Value::Array(rows.iter().map(decode_xml_entities).collect()),
        Value::Object(rows) => Value::Object(
            rows.iter()
                .map(|(key, value)| (key.clone(), decode_xml_entities(value)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// MW50574.ay builds a fresh ordinary macro engine from the imported source
/// events. Hashing precedes assigning the selected mouse mode in that body.
pub fn prepare_macro(value: &Value) -> Result<Value> {
    let mut result = value.clone();
    let events = value["macroEvents"]
        .as_array()
        .context("Imported macro events are absent")?;
    let mode = events
        .first()
        .and_then(|event| event["recordProfile"]["mmtSetting"].as_u64())
        .unwrap_or(0);
    let mode = u8::try_from(mode).context("Imported macro mouse mode is invalid")?;
    let movement = match mode {
        0 => "none",
        1 => "absolute",
        2 => "foreground",
        3 => "relative",
        _ => anyhow::bail!("Imported macro mouse mode is unknown"),
    };
    let mut compiled = Vec::new();
    for event in events {
        let geometry = event.get("monitorInforBefore").map(|_| event);
        compiled.extend(crate::macro_engine::compile_event(event, mode, geometry)?);
    }
    let mut engine = json!({"events":compiled,"hash":"","mouseMoveType":"absolute"});
    engine["hash"] = crate::mapping_engine_hash::source_hash(&engine)?.into();
    engine["mouseMoveType"] = movement.into();
    result["appEngine"] = engine;
    Ok(result)
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(input.len() * 3 / 4);
    let (mut bits, mut available) = (0u32, 0);
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            // UI57207 normalizes URL-safe characters and removes all other
            // characters, including padding, before decoding.
            _ => continue,
        };
        bits = (bits << 6) | u32::from(value);
        available += 6;
        if available >= 8 {
            available -= 8;
            bytes.push((bits >> available) as u8);
        }
    }
    ensure!(available != 6, "Truncated Base64 profile payload");
    Ok(bytes)
}
fn encode_base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for bytes in input.chunks(3) {
        let value = (u32::from(bytes[0]) << 16)
            | (u32::from(*bytes.get(1).unwrap_or(&0)) << 8)
            | u32::from(*bytes.get(2).unwrap_or(&0));
        out.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        out.push(if bytes.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if bytes.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reader {
    Product,
    Profiles,
}

fn decode_entry(encoded: &Value, reader: Reader) -> Result<Value> {
    let bytes = decode_base64(
        encoded["payload"]
            .as_str()
            .context("Profile payload is absent")?,
    )?;
    // The product's js-base64 path replaces malformed UTF-8. The global
    // Profiles reader retains its own strict JSON payload boundary.
    let mut value: Value = match reader {
        Reader::Product => serde_json::from_str(&String::from_utf8_lossy(&bytes))?,
        Reader::Profiles => serde_json::from_slice(&bytes)?,
    };
    ensure!(value.is_object(), "Profile payload is not an object");
    let digest = match reader {
        Reader::Profiles => crate::mapping_engine_hash::source_hash(&value)?,
        Reader::Product => {
            let mut hashed = value.clone();
            let object = hashed.as_object_mut().expect("validated profile object");
            object.remove("hash");
            object.remove("gamemode");
            format!(
                "{:x}",
                md5::compute(crate::mapping_engine_hash::canonical_json(&hashed)?)
            )
        }
    };
    ensure!(
        encoded["hash"].as_str() == Some(digest.as_str()),
        "Profile hash does not match"
    );
    value["name"] = json!(encoded["name"].as_str().context("Profile name is absent")?);
    Ok(value)
}

/// The file reader's category/PID compatibility and corrupt-entry semantics.
/// Payload hashes are verified before applying the outer display name.
pub fn decode(bytes: &[u8], target: &Value) -> Result<DecodedFile> {
    let target: TransferTarget =
        serde_json::from_value(target.clone()).context("Invalid profile compatibility target")?;
    decode_file(bytes, &target, Reader::Product)
}

/// Global Profiles 43/Ea and 1867/D reject incompatible files while the
/// product reader can still return independent macro entries from that file.
pub fn decode_global(
    bytes: &[u8],
    target: &TransferTarget,
    default_profile: Option<&Value>,
) -> Result<DecodedFile> {
    let mut file = decode_file(bytes, target, Reader::Profiles)?;
    // Current Profiles useDa reads DEFAULTPROFILE from updateDeviceInfo's
    // first cached profile. It does not use a product's factory defaults.
    if let Some(mode) = default_profile
        .and_then(|profile| profile.pointer("/snapTap/keyList/0/mode"))
        .filter(|mode| truthy(mode))
    {
        for profile in &mut file.profiles {
            if let Some(keys) = profile
                .pointer_mut("/snapTap/keyList")
                .and_then(Value::as_array_mut)
            {
                for key in keys {
                    let key = key
                        .as_object_mut()
                        .context("Imported Snap Tap key is not an object")?;
                    if !key.get("mode").is_some_and(truthy) {
                        key.insert("mode".into(), mode.clone());
                    }
                }
            }
        }
    }
    Ok(file)
}

fn decode_file(bytes: &[u8], target: &TransferTarget, reader: Reader) -> Result<DecodedFile> {
    ensure!(bytes.len() <= MAX_BYTES, "Profile file exceeds 16 MiB");
    if reader == Reader::Profiles && bytes.starts_with(b"PK") {
        anyhow::bail!("Synapse 3 migration is not connected");
    }
    let text = String::from_utf8_lossy(bytes);
    let document: Value = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text))?;
    let rows = document["profiles"]
        .as_array()
        .context("Profile file has no profiles array")?;
    let same_category = target.matches_category(document["category"].as_str());
    let product_id = document["productId"].as_f64();
    let compatible = same_category || target.matches_product(product_id);
    if reader == Reader::Profiles && !compatible {
        anyhow::bail!("No compatible profiles for this product");
    }
    let mut warning = same_category && product_id != Some(f64::from(target.product_id));
    let mut decode_rows = |rows: &[Value]| {
        rows.iter()
            .filter_map(|entry| match decode_entry(entry, reader) {
                Ok(value) => Some(value),
                Err(_) => {
                    warning = true;
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let mut profiles = if compatible { decode_rows(rows) } else { Vec::new() };
    let macros = decode_rows(
        document["macros"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    );
    if !target.is_eight_k_analog_device {
        for profile in &mut profiles {
            let mut rows = profile["mappings"].as_array().cloned().unwrap_or_default();
            rows.retain(|row| {
                !row["mapping"].as_array().is_some_and(|stages| {
                    stages
                        .iter()
                        .any(|stage| truthy(&stage["dynamicKeyStrokeGroup"]))
                })
            });
            profile["mappings"] = json!(rows);
        }
    }
    Ok(DecodedFile {
        profiles,
        macros,
        device_name: document["deviceName"].as_str().unwrap_or("").into(),
        warning,
    })
}

fn encode_entry(value: &Value, name: &str) -> Result<Value> {
    ensure!(value.is_object(), "Export payload is not an object");
    Ok(
        json!({"name":name,"payload":encode_base64(serde_json::to_string(value)?.as_bytes()),
        "hash":crate::mapping_engine_hash::source_hash(value)?}),
    )
}
/// MW50574.ry excludes only profile appEngine and exports only the three macro
/// payload fields. Game links and external Chroma effects live in other stores.
pub fn encode(target: &Value, profiles: &[Value], macros: &[Value]) -> Result<Vec<u8>> {
    encode_with_rule(
        target,
        profiles,
        macros,
        ProfileExportRule::ProfilesWithSelectedMacros,
    )
}

pub fn encode_with_rule(
    target: &Value,
    profiles: &[Value],
    macros: &[Value],
    rule: ProfileExportRule,
) -> Result<Vec<u8>> {
    let profiles = profiles
        .iter()
        .map(|profile| {
            let mut value = profile.clone();
            value
                .as_object_mut()
                .context("Export profile is not an object")?
                .remove("appEngine");
            encode_entry(
                &value,
                profile["name"]
                    .as_str()
                    .context("Export profile name is absent")?,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let macros = if rule.includes_macros() { macros } else { &[] }
        .iter()
        .map(|mac| {
            encode_entry(
                &json!({"guid":mac["guid"],"macroEvents":mac["macroEvents"],
        "macroPlaybackOption":mac["macroPlaybackOption"]}),
                mac["name"]
                    .as_str()
                    .context("Export macro name is absent")?,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let mut document = json!({"productId":target["productId"],"deviceName":target["deviceName"],
        "category":target["category"],"profiles":profiles});
    if rule.includes_macros() {
        document["macros"] = macros.into();
    }
    let bytes = serde_json::to_vec_pretty(&document)?;
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Profile file exceeds 16 MiB export limit"
    );
    Ok(bytes)
}

/// The current middleware helper preserves an existing numeric suffix. The UI
/// duplication helper removes that suffix before dispatching; callers choose
/// the helper belonging to their own stage instead of conflating them.
pub fn imported_name(rows: &[Value], name: &str) -> String {
    let used = |candidate: &str| {
        rows.iter()
            .any(|row| row["name"] == candidate || row["content"] == candidate)
    };
    if !used(name) {
        return if name.trim().is_empty() {
            format!("{name} (1)")
        } else {
            name.into()
        };
    }
    (1..)
        .map(|n| format!("{name} ({n})"))
        .find(|candidate| !used(candidate))
        .expect("finite profile names")
}

/// Current UI21561/3342/35380, followed by MW50574.ay. Each selected row has
/// profile/listItems/hasWarning. The analog root imports the complete macroData
/// array, even when an individual row's macro checkbox is cleared.
pub fn prepare_import_756(
    rows: &[Value],
    macros: &[Value],
    existing_profiles: &[Value],
    existing_macros: &[Value],
    active_guid: &str,
    layout: u32,
    fresh_guid: impl FnMut() -> String,
) -> Result<(Vec<Value>, Vec<Value>)> {
    prepare_import_756_inner(
        rows,
        macros,
        existing_profiles,
        existing_macros,
        ImportStage::ProductUi { active_guid },
        layout,
        fresh_guid,
    )
}

/// Global Profiles Ua dispatches selectedMacroData as an empty array and sends
/// the decoded rows directly to MW50574.ay. Product UI analog conversion is a
/// separate caller stage and must not run for that command.
pub fn prepare_global_import_756(
    rows: &[Value],
    existing_profiles: &[Value],
    existing_macros: &[Value],
    layout: u32,
    fresh_guid: impl FnMut() -> String,
) -> Result<(Vec<Value>, Vec<Value>)> {
    prepare_import_756_inner(
        rows,
        &[],
        existing_profiles,
        existing_macros,
        ImportStage::Middleware,
        layout,
        fresh_guid,
    )
}

enum ImportStage<'a> {
    ProductUi { active_guid: &'a str },
    Middleware,
}

fn prepare_import_756_inner(
    rows: &[Value],
    macros: &[Value],
    existing_profiles: &[Value],
    existing_macros: &[Value],
    stage: ImportStage<'_>,
    layout: u32,
    mut fresh_guid: impl FnMut() -> String,
) -> Result<(Vec<Value>, Vec<Value>)> {
    use std::collections::BTreeMap;
    let source = crate::keyboard_latest_mapping::source();
    let mut all_macros = existing_macros.to_vec();
    let mut remapped = BTreeMap::<String, (String, String)>::new();
    let mut imported_macros = Vec::new();
    for value in macros {
        let old = value["guid"]
            .as_str()
            .context("Imported macro GUID is absent")?;
        let mut value = value.clone();
        let name = imported_name(
            &all_macros,
            value["name"]
                .as_str()
                .context("Imported macro name is absent")?,
        );
        value["name"] = json!(name);
        if all_macros.iter().any(|mac| mac["guid"] == old) {
            let guid = fresh_guid();
            remapped.insert(old.to_owned(), (guid.clone(), name));
            value["guid"] = json!(guid);
        }
        let value = prepare_macro(&value)?;
        all_macros.push(value.clone());
        imported_macros.push(value);
    }
    let target = &source["profile_transfer_target"];
    let product_ui = matches!(stage, ImportStage::ProductUi { .. });
    let active_guid = match stage {
        ImportStage::ProductUi { active_guid } => active_guid,
        ImportStage::Middleware => "",
    };
    let info = if active_guid
        == source["preset_guids"]["HIGH_SENSITIVITY"]
            .as_str()
            .unwrap_or("")
    {
        &target["actuationHighSensitivityInfo"]
    } else {
        &target["actuationInfo"]
    };
    let default = info["defaultValue"]
        .as_u64()
        .context("Import actuation default is absent")?;
    let unit = info["unit"]
        .as_u64()
        .context("Import actuation unit is absent")?;
    let minimum = info["minBreak"]
        .as_u64()
        .context("Import actuation break minimum is absent")?;
    let actuation = json!({"0":default,"1":default.saturating_sub(unit).max(minimum)});
    let disabled = source["button_list"]
        .as_array()
        .context("Import button list is absent")?
        .iter()
        .filter(|button| button["disableHypershiftMapping"] == true)
        .filter_map(|button| button["inputID"].as_str())
        .collect::<Vec<_>>();
    let non_analog = source["import_non_analog_ids"]
        .as_array()
        .context("Import analog exemptions are absent")?;
    let keymap_passthrough = source["import_keymap_passthrough_ids"]
        .as_array()
        .context("Import keymap exemptions are absent")?;
    let restricted_layout = source["import_restricted_layouts"]
        .as_array()
        .context("Import layout rules are absent")?
        .iter()
        .any(|value| value.as_u64() == Some(u64::from(layout)));
    let restricted_ids = source["import_restricted_input_ids"]
        .as_array()
        .context("Import layout input IDs are absent")?;
    let convert_analog = |mapping: &Value| -> Value {
        if mapping["inputType"] == "AnalogInput" || non_analog.contains(&mapping["inputID"]) {
            return mapping.clone();
        }
        let mut primary = json!({"actuationPoint":actuation,"outputType":mapping["outputType"]});
        if let Some(kind) = mapping["outputType"].as_str() {
            if let Some(value) = mapping.get(kind) {
                primary[kind] = value.clone();
            }
        }
        let mut result = json!({"inputType":"AnalogInput","mapping":[primary,
            {"actuationPoint":actuation,"isDefault":false,"outputType":"defaultGroup","isHyperShift":false}]});
        for key in ["inputID", "isHyperShift"] {
            if let Some(value) = mapping.get(key) {
                result[key] = value.clone();
            }
        }
        result
    };
    let rewrite = |mapping: &mut Value| {
        let direct = mapping.get("macroGroup").is_some();
        let group = if direct {
            mapping.get_mut("macroGroup")
        } else {
            mapping
                .get_mut("mapping")
                .and_then(Value::as_array_mut)
                .and_then(|stages| stages.first_mut())
                .and_then(|stage| stage.get_mut("macroGroup"))
        };
        if let Some(group) = group {
            if let Some(guid) = group["guid"].as_str() {
                if let Some((guid, name)) = remapped.get(guid) {
                    group["guid"] = json!(guid);
                    group["name"] = json!(name);
                } else if let Some(mac) = all_macros.iter().find(|mac| mac["guid"] == guid) {
                    group["name"] = mac["name"].clone();
                }
            }
        }
    };
    let mut all_profiles = existing_profiles.to_vec();
    let mut imported_profiles = Vec::new();
    for row in rows {
        let mut profile = row["profile"].clone();
        ensure!(profile.is_object(), "Imported profile is not an object");
        let mappings = profile["mappings"]
            .as_array()
            .context("Imported profile mappings are absent")?;
        // UI35380 filters both spellings before Analog conversion.
        let mut mappings = mappings
            .iter()
            .filter(|mapping| {
                !(product_ui
                    && (mapping["isHyperShift"] == true || mapping["IsHyperShift"] == true)
                    && mapping["inputID"]
                        .as_str()
                        .is_some_and(|id| disabled.contains(&id)))
            })
            .map(|mapping| {
                if product_ui {
                    convert_analog(mapping)
                } else {
                    mapping.clone()
                }
            })
            .collect::<Vec<_>>();
        let mut deselected = row["listItems"]
            .as_array()
            .context("Imported profile macro choices are absent")?
            .clone();
        for mac in &all_macros {
            if !deselected.iter().any(|item| item["guid"] == mac["guid"]) {
                deselected.push(json!({"guid":mac["guid"],"text":mac["name"]}));
            }
        }
        mappings.retain(|mapping| {
            !deselected.iter().any(|item| {
                item["isMacroSelected"] != true
                    && mapping
                        .get("macroGroup")
                        .is_some_and(|group| group["guid"] == item["guid"])
            })
        });
        for mapping in &mut mappings {
            rewrite(mapping);
        }
        if row["hasWarning"] == true {
            mappings.retain(|mapping| {
                !matches!(
                    mapping["inputID"].as_str(),
                    Some("DKM_KBMK_01" | "DKM_KBMK_02" | "DKM_KBMK_03")
                )
            });
        }
        mappings.retain(|mapping| {
            !(restricted_layout && restricted_ids.contains(&mapping["inputID"])
                || mapping["isHyperShift"] == true
                    && mapping["inputID"]
                        .as_str()
                        .is_some_and(|id| disabled.contains(&id)))
        });
        profile["mappings"] = json!(mappings);
        if let Some(keymaps) = profile.get_mut("keymaps").and_then(Value::as_array_mut) {
            for keymap in keymaps {
                if let Some(mappings) = keymap.get_mut("mappings").and_then(Value::as_array_mut) {
                    for mapping in mappings {
                        if product_ui && !keymap_passthrough.contains(&mapping["inputID"]) {
                            *mapping = convert_analog(mapping);
                        }
                        rewrite(mapping);
                    }
                }
            }
        }
        if let Some(keys) = profile
            .get_mut("snapTap")
            .and_then(|tap| tap.get_mut("keyList"))
            .and_then(Value::as_array_mut)
            .filter(|_| product_ui)
        {
            let mode = &source["default_profile"]["snapTap"]["keyList"][0]["mode"];
            for key in keys {
                if !truthy(&key["mode"]) {
                    key["mode"] = mode.clone();
                }
            }
        }
        if !truthy(&profile["commandDial"])
            && source["default_profile"].get("commandDial").is_some()
        {
            profile["commandDial"] = source["default_profile"]["commandDial"].clone();
        }
        profile["name"] = json!(imported_name(
            &all_profiles,
            profile["name"]
                .as_str()
                .context("Imported profile name is absent")?
        ));
        profile["guid"] = json!(fresh_guid());
        profile.as_object_mut().unwrap().remove("appEngine");
        all_profiles.push(profile.clone());
        imported_profiles.push(profile);
    }
    Ok((imported_profiles, imported_macros))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_file_roundtrip_excludes_engines_and_checks_before_outer_name() {
        let target = json!({"productId":756,"deviceName":"Reclusa","category":"KEYBOARD","is8kAnalogDevice":true});
        let profile =
            json!({"guid":"profile","name":"键盘","mappings":[],"appEngine":{"mappings":[1]}});
        let mac = json!({"guid":"macro","name":"宏","macroEvents":[{"Type":1}],"macroPlaybackOption":"standard","appEngine":{"events":[]}});
        let bytes = encode(&target, &[profile], &[mac]).unwrap();
        let mut outer: Value = serde_json::from_slice(&bytes).unwrap();
        outer["profiles"][0]["name"] = json!("Outer name");
        let loaded = decode(&serde_json::to_vec(&outer).unwrap(), &target).unwrap();
        assert_eq!(loaded.profiles()[0]["name"], "Outer name");
        assert!(loaded.profiles()[0].get("appEngine").is_none());
        assert!(loaded.macros()[0].get("appEngine").is_none());
        outer["profiles"][0]["hash"] = json!("invalid");
        let loaded = decode(&serde_json::to_vec(&outer).unwrap(), &target).unwrap();
        assert!(loaded.profiles().is_empty());
        assert!(loaded.has_warning());
        assert_eq!(loaded.macros().len(), 1);
    }
    #[test]
    fn current_reader_and_writer_keep_their_distinct_less_than_hash_rules() {
        let target = json!({"productId":756,"category":"KEYBOARD"});
        let bytes = encode(&target, &[json!({"name":"<tag>","mappings":[]})], &[]).unwrap();
        let loaded = decode(&bytes, &target).unwrap();
        assert!(loaded.profiles().is_empty());
        assert!(loaded.has_warning());
        assert_eq!(decode_base64("e=y!J4I joxfQ==").unwrap(), br#"{"x":1}"#);
    }

    #[test]
    fn profiles_only_writer_omits_macros_and_preserves_external_profile_fields() {
        let target = json!({"productId":1398,"category":"AUDIO","deviceName":"Headset"});
        let profile = json!({"guid":"profile","name":"Profile","extension":{"keep":42},"appEngine":{"stale":true}});
        let bytes = encode_with_rule(
            &target,
            &[profile.clone()],
            &[Value::Null],
            ProfileExportRule::ProfilesOnly,
        )
        .unwrap();
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            document
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["productId", "deviceName", "category", "profiles"]
                .into_iter()
                .collect()
        );
        let payload = decode_base64(document["profiles"][0]["payload"].as_str().unwrap()).unwrap();
        let payload: Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(payload["extension"], profile["extension"]);
        assert!(payload.get("appEngine").is_none());
        assert_eq!(
            document["profiles"][0]["hash"],
            crate::mapping_engine_hash::source_hash(&payload).unwrap()
        );
        assert_eq!(profile["appEngine"], json!({"stale":true}));
    }
}

#[cfg(test)]
mod global_tests {
    use super::{decode_base64 as base64, truthy as js_truthy};
    use crate::mapping_engine_hash::canonical_json;
    use serde_json::json;

    fn target() -> super::TransferTarget {
        super::TransferTarget {
            product_id: 756,
            dongle_id: Some(45098),
            ble_id: None,
            category: super::TransferCategory::Keyboard,
            is_eight_k_analog_device: false,
        }
    }

    #[test]
    fn product_and_global_readers_keep_distinct_hash_and_compatibility_semantics() {
        let target = target();
        let source = serde_json::to_value(&target).unwrap();
        let bytes = super::encode(&source, &[json!({"name":"<tag>","mappings":[],"extension":{"retained":true}})], &[
            json!({"guid":"macro","name":"Macro","macroEvents":[],"macroPlaybackOption":"standard"})
        ]).unwrap();
        let global = super::decode_global(&bytes, &target, None).unwrap();
        assert_eq!(global.profiles()[0]["name"], "<tag>");
        assert_eq!(global.profiles()[0]["extension"], json!({"retained":true}));
        assert!(!global.has_warning());
        let product = super::decode(&bytes, &source).unwrap();
        assert!(product.profiles().is_empty());
        assert!(product.has_warning());
        assert_eq!(product.macros().len(), 1);
        let mut document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        document["category"] = json!("AUDIO");
        document["productId"] = json!(1234);
        let bytes = serde_json::to_vec(&document).unwrap();
        assert!(super::decode_global(&bytes, &target, None).is_err());
        let product = super::decode(&bytes, &source).unwrap();
        assert!(product.profiles().is_empty());
        assert_eq!(product.macros().len(), 1);
    }

    #[test]
    fn entry_validation_precedes_dks_filtering_and_retains_external_fields() {
        let target = target();
        let source = serde_json::to_value(&target).unwrap();
        let profile = json!({"name":"Profile","vendorExtension":{"a":1},"mappings":[
            {"inputID":"KEY_A","mapping":[{"dynamicKeyStrokeGroup":{}}]},
            {"inputID":"KEY_B","mapping":[{"dynamicKeyStrokeGroup":0}]}
        ]});
        let bytes = super::encode(&source, &[profile], &[]).unwrap();
        let decoded = super::decode_global(&bytes, &target, None).unwrap();
        assert_eq!(
            decoded.profiles()[0]["mappings"].as_array().unwrap().len(),
            1
        );
        assert_eq!(decoded.profiles()[0]["mappings"][0]["inputID"], "KEY_B");
        assert_eq!(decoded.profiles()[0]["vendorExtension"], json!({"a":1}));
        let mut document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        document["profiles"][0]["hash"] = json!("invalid");
        let decoded =
            super::decode_global(&serde_json::to_vec(&document).unwrap(), &target, None).unwrap();
        assert!(decoded.profiles().is_empty());
        assert!(decoded.has_warning());
    }

    #[test]
    fn global_reader_matches_receiver_pid_and_limits_file_size() {
        let target = target();
        let source = json!({"productId":45098,"category":"AUDIO"});
        let bytes =
            super::encode(&source, &[json!({"name":"Profile","mappings":[]})], &[]).unwrap();
        let decoded = super::decode_global(&bytes, &target, None).unwrap();
        assert_eq!(decoded.profiles().len(), 1);
        assert!(!decoded.has_warning());
        assert!(super::decode_global(&vec![b' '; super::MAX_BYTES + 1], &target, None).is_err());
    }

    #[test]
    fn source_truthiness_preserves_zero_and_empty_string_mappings() {
        for value in [json!(null), json!(false), json!(0), json!(-0.0), json!("")] {
            assert!(!js_truthy(&value));
        }
        for value in [json!(true), json!(1), json!("false"), json!([]), json!({})] {
            assert!(js_truthy(&value));
        }
        let target = super::TransferTarget {
            product_id: 1318,
            dongle_id: None,
            ble_id: None,
            category: super::TransferCategory::Audio,
            is_eight_k_analog_device: false,
        };
        assert!(target.matches_product(json!(1318.0).as_f64()));
        assert!(target.matches_product(json!(1318).as_f64()));
        assert!(!target.matches_product(json!("1318").as_f64()));
        assert!(!target.matches_product(json!(null).as_f64()));
        assert!(target.matches_category(Some("AUDIO")));
        assert!(!target.matches_category(Some("MOUSE")));
    }

    #[test]
    fn decoder_uses_the_source_normalized_base64_alphabet() {
        assert_eq!(base64("e=y!J4I joxfQ==").unwrap(), br#"{"x":1}"#);
        assert_eq!(base64("-_8=").unwrap(), [251, 255]);
        assert!(base64("A").is_err());
    }

    #[test]
    fn global_snap_tap_defaults_come_from_the_captured_first_profile() {
        let target = target();
        let source = serde_json::to_value(target).unwrap();
        let profile = json!({"name":"Imported","mappings":[],"snapTap":{"keyList":[{"mode":0},{"mode":"FIRST_INPUT"},{}]}});
        let bytes = super::encode(&source, &[profile.clone()], &[]).unwrap();
        let default = json!({"snapTap":{"keyList":[{"mode":"HIGH_SENSITIVITY"}]}});
        let loaded = super::decode_global(&bytes, &target, Some(&default)).unwrap();
        assert_eq!(
            loaded.profiles[0]["snapTap"]["keyList"][0]["mode"],
            "HIGH_SENSITIVITY"
        );
        assert_eq!(
            loaded.profiles[0]["snapTap"]["keyList"][1]["mode"],
            "FIRST_INPUT"
        );
        assert_eq!(
            loaded.profiles[0]["snapTap"]["keyList"][2]["mode"],
            "HIGH_SENSITIVITY"
        );
        assert_eq!(
            super::decode_global(&bytes, &target, None)
                .unwrap()
                .profiles[0],
            profile
        );
    }

    #[test]
    fn global_import_keeps_middleware_mapping_semantics_and_does_not_import_file_macros() {
        let rows = vec![json!({"profile":{
            "guid":"old","name":"Imported","extension":{"keep":42},
            "mappings":[
                {"inputID":"KEY_A","inputType":"KeyboardInput","outputType":"defaultGroup"},
                {"inputID":"KEY_B","outputType":"macroGroup","macroGroup":{"guid":"off","name":"Old"}},
                {"inputID":"KEY_C","inputType":"AnalogInput","mapping":[{"outputType":"macroGroup","macroGroup":{"guid":"off","name":"Old"}}]},
                {"inputID":"KEY_D","outputType":"macroGroup","macroGroup":{"guid":"known","name":"Old"}}
            ],
            "keymaps":[{"mappings":[{"inputID":"KEY_A","macroGroup":{"guid":"known","name":"Old"}}]}]
        },"listItems":[{"guid":"off","isMacroSelected":false},{"guid":"known","isMacroSelected":true}],"hasWarning":false})];
        let existing = [json!({"guid":"existing","name":"Imported"})];
        let macros = [
            json!({"guid":"known","name":"Current"}),
            json!({"guid":"off","name":"Unselected"}),
        ];
        let before = rows.clone();
        let (profiles, imported_macros) =
            super::prepare_global_import_756(&rows, &existing, &macros, 1, || "fresh".into())
                .unwrap();
        assert!(imported_macros.is_empty());
        assert_eq!(profiles[0]["guid"], "fresh");
        assert_eq!(profiles[0]["name"], "Imported (1)");
        assert_eq!(profiles[0]["extension"]["keep"], 42);
        assert_eq!(profiles[0]["mappings"][0]["inputType"], "KeyboardInput");
        assert_eq!(profiles[0]["mappings"].as_array().unwrap().len(), 3);
        assert_eq!(
            profiles[0]["mappings"][1]["mapping"][0]["macroGroup"]["guid"],
            "off"
        );
        assert_eq!(profiles[0]["mappings"][2]["macroGroup"]["name"], "Current");
        assert_eq!(
            profiles[0]["keymaps"][0]["mappings"][0]["macroGroup"]["name"],
            "Current"
        );
        assert_eq!(rows, before);
    }

    #[test]
    fn stable_hash_text_uses_utf16_key_order_and_javascript_number_thresholds() {
        assert_eq!(
            canonical_json(&json!({"\u{e000}":1,"\u{10000}":2})).unwrap(),
            "{\"\u{10000}\":2,\"\u{e000}\":1}"
        );
        for (value, expected) in [
            (json!(-0.0), "0"),
            (json!(1e-6), "0.000001"),
            (json!(1e-7), "1e-7"),
            (json!(1e20), "100000000000000000000"),
            (json!(1e21), "1e+21"),
        ] {
            assert_eq!(canonical_json(&value).unwrap(), expected);
        }
    }

    #[test]
    fn source_profile_hash_excludes_engine_metadata_before_applying_the_outer_name() {
        let encoded = json!({
            "payload":"eyJ4IjoxLCJoYXNoIjoib2xkIiwiZ2FtZW1vZGUiOnRydWV9",
            "hash":"ac3ef48caa08fa3ed5e025da69edc645",
            "name":"Outer name",
        });
        assert_eq!(
            super::decode_entry(&encoded, super::Reader::Profiles).unwrap(),
            json!({"x":1,"hash":"old","gamemode":true,"name":"Outer name"})
        );
        let mut invalid = encoded;
        invalid["hash"] = json!("bad");
        assert!(super::decode_entry(&invalid, super::Reader::Profiles).is_err());
    }
}
