//! Current 190/229/239 active-profile receiver LED storage. This application's
//! local document uses source keys/fields; it is separate from vendor storage
//! and never confirms device delivery. Unknown profile/document fields survive.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

const MAX_VERSION: u64 = (1_u64 << 53) - 1;
const FIELD: &str = "indicatorLedConfiguration";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ActiveProfile {
    pub document: Value,
    pub profile: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SaveReceipt {
    pub profile_guid: String,
    pub version: u64,
    pub document: Value,
}
fn key(product: u32) -> anyhow::Result<String> {
    ensure!(
        matches!(product, 190 | 229 | 239),
        "Current receiver indicator source is absent"
    );
    Ok(format!("synapse_{product}"))
}
fn next_version(value: Option<&Value>, initialization: bool) -> anyhow::Result<u64> {
    let version = match value.filter(|value| !value.is_null()) {
        None => return Ok(if initialization { 2 } else { 1 }),
        Some(value) => value
            .as_u64()
            .context("Source profile version is not an unsigned integer")?,
    };
    ensure!(
        version <= MAX_VERSION,
        "Source profile version exceeds 53 bits"
    );
    Ok(if version == MAX_VERSION { 0 } else { version + 1 })
}
fn uuid() -> anyhow::Result<String> {
    use ring::rand::SecureRandom;
    let mut bytes = [0_u8; 16];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Local profile UUID entropy is unavailable"))?;
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    ))
}
fn check_document(document: &Value, product: u32) -> anyhow::Result<()> {
    ensure!(
        document.is_object()
            && document["productId"].as_u64() == Some(u64::from(product))
            && document["profiles"].is_array(),
        "Receiver local document belongs to another product or is malformed"
    );
    Ok(())
}
fn selected(document: &Value, serial: &str) -> anyhow::Result<String> {
    // Source getDeviceActiveProfileGuid falls back to the first profile only
    // when this serial has no metadata record. An invalid existing record does
    // not silently select some other profile.
    let guid = if let Some(metadata) = document.get("deviceMetadatas").and_then(|v| v.get(serial)) {
        metadata.get("activeProfileGuid").and_then(Value::as_str)
    } else {
        document["profiles"]
            .as_array()
            .and_then(|v| v.first())
            .and_then(|v| v["guid"].as_str())
    };
    Ok(guid
        .filter(|guid| !guid.is_empty())
        .context("Current serial has no source active profile")?
        .to_owned())
}
fn projection(mut document: Value, product: u32, serial: &str) -> anyhow::Result<ActiveProfile> {
    check_document(&document, product)?;
    let guid = selected(&document, serial)?;
    let profile = document["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|profile| profile["guid"].as_str() == Some(&guid))
        .cloned()
        .context("Current source active profile is absent")?;
    document["activeProfile"] = json!(guid);
    Ok(ActiveProfile { document, profile })
}
pub fn active_profile(
    path: &Path,
    product: u32,
    serial: &str,
) -> anyhow::Result<Option<ActiveProfile>> {
    let key = key(product)?;
    if serial.is_empty() {
        return Ok(None);
    }
    crate::source_local::get(path, &key)?
        .map(|document| projection(document, product, serial))
        .transpose()
}

/// Establish this application's local cache from the caller's source-derived
/// default profile. This is local initialization, not a hardware read or an
/// import of the vendor's Chromium storage. The existing local UI profile GUID
/// is retained when supplied; an absent GUID receives a fresh UUID.
pub fn ensure_cache(
    path: &Path,
    product: u32,
    serial: &str,
    default_profile: &Value,
) -> anyhow::Result<Option<ActiveProfile>> {
    let key = key(product)?;
    if serial.is_empty() {
        return Ok(None);
    }
    ensure!(
        default_profile.is_object(),
        "Local source default profile must be an object"
    );
    let mut result = None;
    crate::source_local::update(path, &key, |previous| {
        let absent = previous.is_none();
        let mut document=previous.unwrap_or_else(|| {
            let mut document=json!({"version":1,"vendorId":5426,"productId":product,"category":"MOUSE",
                "reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"},
                "calibration":{"smartTracking":{"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":13,"landingDistance":12},
                    "profiles":[{"type":"default"}],"selectedProfile":{}},
                "lowPowerMode":30,"sensitivityMatcher":{"profiles":[]},"selectedSensitivityMatcher":"",
                "profiles":[],"deviceMetadatas":{}});
            if product==229 {document["powerSavingValue"]=json!(5);}
            document
        });
        check_document(&document, product)?;
        let metadata = document.get("deviceMetadatas").and_then(|v| v.get(serial));
        if metadata
            .and_then(|v| v["activeProfileGuid"].as_str())
            .is_some_and(|v| !v.is_empty())
        {
            result = Some(projection(document, product, serial)?);
            return Ok(None);
        }
        let mut profile = default_profile.clone();
        let guid = match profile["guid"].as_str().filter(|guid| !guid.is_empty()) {
            Some(guid) => guid.to_owned(),
            None => uuid()?,
        };
        profile["guid"] = json!(guid);
        let base = profile["name"].as_str().unwrap_or("Default").to_owned();
        let profiles = document["profiles"].as_array_mut().unwrap();
        if !profiles
            .iter()
            .any(|profile| profile["guid"].as_str() == Some(&guid))
        {
            let name = razer_device::source_string::unique_profile_name(&base, |candidate| {
                profiles
                    .iter()
                    .any(|profile| profile["name"].as_str() == Some(candidate))
            });
            profile["name"] = json!(name);
            profiles.push(profile);
        }
        if !document
            .get("deviceMetadatas")
            .is_some_and(Value::is_object)
        {
            document["deviceMetadatas"] = json!({});
        }
        if !document["deviceMetadatas"]
            .get(serial)
            .is_some_and(Value::is_object)
        {
            document["deviceMetadatas"][serial] = json!({});
        }
        document["deviceMetadatas"][serial]["activeProfileGuid"] = json!(guid);
        if document["deviceMetadatas"][serial].get("name").is_none() {
            document["deviceMetadatas"][serial]["name"] = json!(match product {
                190 => "Razer DeathAdder V4 Pro",
                229 => "Razer Viper V4 Pro",
                239 => "Razer DeathAdder V4 Pro Carbon Fiber Edition",
                _ => unreachable!(),
            });
        }
        if absent {
            document["activeProfile"] = json!(guid);
        }
        let projected = projection(document, product, serial)?;
        result = Some(projected.clone());
        Ok(Some(projected.document))
    })?;
    Ok(result)
}

/// Bind the actual local UI profile to the source-shaped serial metadata. This
/// stores a local selection/draft; it neither imports vendor data nor reports a
/// hardware profile switch. Missing fields in the supplied profile preserve
/// existing source/unknown fields, including other settings and profiles.
pub fn bind_local_profile(
    path: &Path,
    product: u32,
    serial: &str,
    profile: &Value,
) -> anyhow::Result<ActiveProfile> {
    let key = key(product)?;
    ensure!(
        !serial.is_empty(),
        "Local profile binding requires the queried protocol serial"
    );
    let guid = profile["guid"]
        .as_str()
        .filter(|guid| !guid.is_empty())
        .context("Local profile binding requires the existing UI profile GUID")?
        .to_owned();
    let fields = profile
        .as_object()
        .context("Local UI profile is not an object")?;
    ensure_cache(path, product, serial, profile)?;
    let mut result = None;
    crate::source_local::update(path, &key, |previous| {
        let mut document =
            previous.context("Local receiver cache disappeared before profile binding")?;
        check_document(&document, product)?;
        let before = document.clone();
        let profiles = document["profiles"].as_array_mut().unwrap();
        if let Some(current) = profiles
            .iter_mut()
            .find(|profile| profile["guid"].as_str() == Some(&guid))
        {
            current
                .as_object_mut()
                .context("Existing receiver profile is malformed")?
                .extend(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
        } else {
            profiles.push(profile.clone());
        }
        if !document
            .get("deviceMetadatas")
            .is_some_and(Value::is_object)
        {
            document["deviceMetadatas"] = json!({});
        }
        if !document["deviceMetadatas"]
            .get(serial)
            .is_some_and(Value::is_object)
        {
            document["deviceMetadatas"][serial] = json!({});
        }
        document["deviceMetadatas"][serial]["activeProfileGuid"] = json!(guid);
        document["activeProfile"] = json!(guid);
        let changed = document != before;
        if changed {
            document["version"] = json!(next_version(document.get("version"), false)?);
        }
        let projected = projection(document, product, serial)?;
        result = Some(projected.clone());
        Ok(changed.then_some(projected.document))
    })?;
    result.context("Local profile binding did not read a cache")
}

fn save(
    path: &Path,
    product: u32,
    serial: &str,
    configuration: &Value,
    initialization: bool,
) -> anyhow::Result<Option<SaveReceipt>> {
    let key = key(product)?;
    ensure!(
        configuration.is_array(),
        "Source indicator setter requires an array"
    );
    if serial.is_empty() {
        return Ok(None);
    }
    let mut receipt = None;
    crate::source_local::update(path, &key, |previous| {
        let Some(document) = previous else {
            return Ok(None);
        };
        let mut projected = projection(document, product, serial)?;
        let guid = projected.profile["guid"]
            .as_str()
            .context("Source active profile GUID is absent")?
            .to_owned();
        if initialization && projected.profile.get(FIELD).is_none() {
            return Ok(None);
        }
        let version = next_version(projected.document.get("version"), initialization)?;
        let profile = projected.document["profiles"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|profile| profile["guid"].as_str() == Some(&guid))
            .unwrap();
        profile[FIELD] = configuration.clone();
        projected.document["version"] = json!(version);
        if projected.document.get("isDefault").is_some() {
            projected
                .document
                .as_object_mut()
                .unwrap()
                .remove("isDefault");
        }
        receipt = Some(SaveReceipt {
            profile_guid: guid,
            version,
            document: projected.document.clone(),
        });
        Ok(Some(projected.document))
    })?;
    Ok(receipt)
}
/// Await this write before enqueueing the NORMAL_SKIPPABLE device setter.
/// Its local success says nothing about a later device acknowledgement.
pub fn save_user_selection(
    path: &Path,
    product: u32,
    serial: &str,
    configuration: &Value,
) -> anyhow::Result<Option<SaveReceipt>> {
    save(path, product, serial, configuration, false)
}
/// The init owner starts this task after the SET/UI post and does not await it
/// before the quality GET. The task rereads the current active profile.
pub fn save_initialization(
    path: &Path,
    product: u32,
    serial: &str,
    configuration: &Value,
) -> anyhow::Result<Option<SaveReceipt>> {
    save(path, product, serial, configuration, true)
}

#[cfg(test)]
#[path = "receiver_indicator_tests.rs"]
mod tests;
