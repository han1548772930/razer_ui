//! Current 164/241 Help taskMakerResetOBM document semantics. Local storage is
//! separate from Chromium storage; queued refresh events are not device acks.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::Path, sync::OnceLock};

const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

fn unique_name(profiles: &[Value], base: String) -> String {
    let mut name = base.clone();
    // Original W$: at most 100 probes; the final suffix may already exist.
    for suffix in 1..=100 {
        if !profiles
            .iter()
            .any(|p| p.get("name").and_then(Value::as_str) == Some(&name))
        {
            break;
        }
        name = format!("{base} {suffix}");
    }
    name
}

fn valid_guid(guid: &str) -> bool {
    guid.len() == 36
        && guid.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

#[derive(Deserialize)]
struct Defaults {
    product_id: u32,
    edition_id: u32,
    device_name: String,
    schema_version: u32,
    default_global: Value,
    default_profile: Value,
}
fn defaults(product: u32, edition: u32) -> anyhow::Result<&'static Defaults> {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        products: Vec<Defaults>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        let value: Catalog = serde_json::from_str(include_str!(
            "../../../assets/data/receiver-reset-defaults.json"
        ))
        .expect("statically audited reset defaults");
        assert_eq!(value.schema_version, 1);
        value
    });
    catalog
        .products
        .iter()
        .find(|p| p.product_id == product && p.edition_id == edition)
        .context("Current source has no audited reset defaults for this product/edition")
}

/// A source-backed local document mutation and the actual follow-up events.
/// This receipt deliberately has no successful hardware-reset flag.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResetPlan {
    document: Value,
    new_profile: Value,
    runtime_projection: Value,
    refresh_events: Vec<Value>,
}
impl ResetPlan {
    pub fn prepare(
        product: u32,
        edition: u32,
        current: &Value,
        serial: &str,
        container: &str,
        computer_name: &str,
        guid: &str,
    ) -> anyhow::Result<Self> {
        let defaults = defaults(product, edition)?;
        ensure!(
            !serial.is_empty(),
            "Device serial is required for the source local storage task"
        );
        ensure!(
            !container.is_empty(),
            "Device container is required for reset ownership"
        );
        ensure!(valid_guid(guid), "New profile requires a UUID identity");
        let mut document = current.clone();
        let object = document
            .as_object_mut()
            .context("Reset requires a source device document object")?;
        ensure!(
            object.get("productId").and_then(Value::as_u64) == Some(u64::from(product)),
            "Reset document product does not match its owner"
        );
        ensure!(
            object.get("schemaVersion").and_then(Value::as_u64)
                == Some(u64::from(defaults.schema_version)),
            "Stored source document needs its original schema migration before reset"
        );
        ensure!(
            object.get("globalConfig").and_then(|v| v.get("isLinked")) != Some(&Value::Bool(true)),
            "Linked sub-device metadata reset requires the actual runtime sub-device scope"
        );
        let version = match object.get("version") {
            None => 1,
            Some(value) => {
                let version = value
                    .as_u64()
                    .context("Source document version is not an integer")?;
                ensure!(
                    version <= MAX_SAFE_INTEGER,
                    "Source document version exceeds 53 bits"
                );
                if version == MAX_SAFE_INTEGER {
                    0
                } else {
                    version + 1
                }
            }
        };
        let profiles = object
            .get_mut("profiles")
            .and_then(Value::as_array_mut)
            .context(
                "Reset requires the real profile array; it does not invent missing profiles",
            )?;
        ensure!(
            profiles.iter().all(|p| p.is_object()),
            "Source profile is not an object"
        );
        ensure!(
            !profiles
                .iter()
                .any(|p| p.get("guid").and_then(Value::as_str) == Some(guid)),
            "Profile UUID already exists"
        );
        let base = if computer_name.is_empty() {
            "Default".to_owned()
        } else {
            format!("{computer_name}-Default")
        };
        let name = unique_name(profiles, base);
        let mut profile = defaults.default_profile.clone();
        profile["guid"] = json!(guid);
        profile["name"] = json!(name);
        profile["mappings"] = json!([]);
        profiles.push(profile.clone());
        object.insert("activeProfile".into(), json!(guid));
        object.insert("version".into(), json!(version));
        let metadata = object
            .entry("deviceMetadatas")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .context("Source deviceMetadatas is not an object")?;
        let entry = metadata
            .entry(serial)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .context("Serial metadata is not an object")?;
        entry.insert("activeProfileGuid".into(), json!(guid));
        entry
            .entry("name")
            .or_insert_with(|| json!(defaults.device_name));
        object.remove("isDefault");
        if product == 241 {
            object.insert(
                "reportIDs".into(),
                defaults.default_global["reportIDs"].clone(),
            );
        }
        let runtime_projection = json!({"productId":product,"activeProfile":guid,
            "profiles":document["profiles"].as_array().unwrap().iter().map(|p|json!({"name":p["name"],"guid":p["guid"]})).collect::<Vec<_>>()});
        let refresh_events = vec![
            json!({"type":"ON_INIT_BRIGHTNESS","from":"MW_ACTION_FROM_LOCALSTORAGE",
                "payload":{"brightness":profile["brightness"],"switchOffLighting":profile["switchOffLighting"],"deviceContainerId":container,"version":version}}),
            json!({"type":"ON_SET_EFFECTS","from":"MW_ACTION_FROM_LOCALSTORAGE",
                "payload":{"effectSetting":profile["quickEffects"],"deviceContainerId":container,"version":version}}),
            json!({"type":"ON_SET_KEYMAPPING","from":"MW_ACTION_FROM_LOCALSTORAGE",
                "payload":{"mappingList":profile["mappings"],"viewIndex":0,"version":version}}),
        ];
        Ok(Self {
            document,
            new_profile: profile,
            runtime_projection,
            refresh_events,
        })
    }
    pub fn document(&self) -> &Value {
        &self.document
    }
    pub fn new_profile(&self) -> &Value {
        &self.new_profile
    }
    pub fn runtime_projection(&self) -> &Value {
        &self.runtime_projection
    }
    pub fn refresh_events(&self) -> &[Value] {
        &self.refresh_events
    }
}

/// Original b4 creates an additional uniquely named default profile if this
/// serial lacks a truthy activeProfileGuid, before the reset task runs. This is
/// an in-memory cache read mutation; persist still compares the raw disk value.
pub fn prepare_serial_cache(
    product: u32,
    edition: u32,
    current: &Value,
    serial: &str,
    computer_name: &str,
    guid: &str,
) -> anyhow::Result<Value> {
    let source = defaults(product, edition)?;
    ensure!(!serial.is_empty(), "Source cache requires a real serial");
    let mut document = current.clone();
    ensure!(document.is_object(), "Source cache is not a document");
    ensure!(
        document["productId"] == product,
        "Source cache product differs from serial owner"
    );
    ensure!(
        document["schemaVersion"] == source.schema_version,
        "Stored source cache requires its original schema migration"
    );
    ensure!(
        document["globalConfig"]["isLinked"] != true,
        "Linked cache requires the actual runtime sub-device scope"
    );
    let active = document["deviceMetadatas"][serial]["activeProfileGuid"]
        .as_str()
        .filter(|guid| !guid.is_empty())
        .map(str::to_owned);
    let active = match active {
        Some(active) => active,
        None => {
            ensure!(
                valid_guid(guid),
                "Source cache profile requires a UUID identity"
            );
            let profiles = document
                .get_mut("profiles")
                .and_then(Value::as_array_mut)
                .context("Source cache requires the real profiles array")?;
            ensure!(
                profiles.iter().all(Value::is_object),
                "Source cache profile is not an object"
            );
            ensure!(
                !profiles.iter().any(|profile| profile["guid"] == guid),
                "Cache profile UUID already exists"
            );
            let mut profile = source.default_profile.clone();
            profile["name"] = json!(unique_name(profiles, format!("{computer_name}-Default")));
            profile["guid"] = json!(guid);
            profile["mappings"] = json!([]);
            profiles.push(profile);
            let metadata = document
                .as_object_mut()
                .unwrap()
                .entry("deviceMetadatas")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .context("Source deviceMetadatas is not an object")?;
            let metadata = metadata
                .entry(serial)
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .context("Serial metadata is not an object")?;
            metadata.insert("activeProfileGuid".into(), json!(guid));
            metadata
                .entry("name")
                .or_insert_with(|| json!(source.device_name));
            guid.to_owned()
        }
    };
    document["activeProfile"] = json!(active);
    Ok(document)
}

/// Source startup explicitly creates getDeviceDefaultLocalData when b4 has no
/// stored document. Serial here must come from the real device command.
pub fn initial_document(
    product: u32,
    edition: u32,
    serial: &str,
    computer_name: &str,
    guid: &str,
) -> anyhow::Result<Value> {
    let source = defaults(product, edition)?;
    ensure!(!serial.is_empty(), "Source startup requires a real serial");
    let mut profile = source.default_profile.clone();
    profile["guid"] = json!(guid);
    // getDefaultProfile has the literal `computerName-Default`, even if empty.
    profile["name"] = json!(format!("{computer_name}-Default"));
    profile["mappings"] = json!([]);
    let mut document = source.default_global.clone();
    document["schemaVersion"] = json!(source.schema_version);
    document["isDefault"] = json!(true);
    document["profiles"] = json!([profile]);
    document["activeProfile"] = json!(guid);
    document["deviceMetadatas"] =
        json!({serial:{"activeProfileGuid":guid,"name":source.device_name}});
    Ok(document)
}

pub fn load(
    path: &Path,
    product: u32,
    container: &str,
    serial: &str,
) -> anyhow::Result<Option<Value>> {
    crate::read_document(path, |text| {
        let file: Value = serde_json::from_str(text)?;
        ensure!(
            file["schema_version"] == 1
                && file["product_id"] == product
                && file["container"] == container
                && file["serial"] == serial,
            "Source document adaptation owner/schema mismatch"
        );
        ensure!(
            file["document"].is_object(),
            "Stored source document is not an object"
        );
        Ok(file["document"].clone())
    })
}

/// Record an actual follow-up response separately from the saved profile.
/// Missing executors never become successful device refreshes.
pub fn record_refresh(
    path: &Path,
    product: u32,
    container: &str,
    serial: &str,
    document: &Value,
    receipt: Value,
) -> anyhow::Result<()> {
    let mut file = crate::read_document(path, |text| Ok(serde_json::from_str::<Value>(text)?))?
        .context("Local reset document disappeared before refresh receipt")?;
    ensure!(
        file["product_id"] == product
            && file["container"] == container
            && file["serial"] == serial
            && file["document"] == *document,
        "Local reset document changed before device refresh receipt"
    );
    let previous = file.clone();
    file["executed_refresh_events"] = receipt;
    file["device_refresh_complete"] = json!(false);
    crate::write_document(path, &file, |text| {
        let current: Value = serde_json::from_str(text)?;
        ensure!(
            current == previous,
            "Local reset file changed before refresh receipt"
        );
        Ok(())
    })
}

/// Persist exactly the captured local source document and its required refresh
/// intents. Reject external changes; do not erase another owner's document.
pub fn persist(
    path: &Path,
    product: u32,
    container: &str,
    serial: &str,
    previous: &Value,
    plan: &ResetPlan,
) -> anyhow::Result<()> {
    let disk = crate::read_document(path, |text| Ok(serde_json::from_str::<Value>(text)?))?;
    if let Some(disk) = &disk {
        ensure!(
            disk["schema_version"] == 1
                && disk["product_id"] == product
                && disk["container"] == container
                && disk["serial"] == serial,
            "Local reset file owner/schema differs; refusing replacement"
        );
        ensure!(
            disk["document"] == *previous,
            "Local reset document changed; reload it before resetting"
        );
    }
    let file = json!({"schema_version":1,"product_id":product,"container":container,"serial":serial,
        "storage_kind":"local_source_document_adaptation","document":plan.document(),
        "runtime_projection":plan.runtime_projection(),"required_refresh_events":plan.refresh_events(),
        "device_refresh_complete":false});
    crate::write_document(path, &file, |text| {
        let current: Value = serde_json::from_str(text)?;
        ensure!(
            disk.as_ref() == Some(&current),
            "Local reset file changed during preparation"
        );
        Ok(())
    })
}

pub fn local_path(
    product: u32,
    container: &str,
    serial: &str,
) -> anyhow::Result<std::path::PathBuf> {
    ensure!(
        matches!(product, 164 | 241),
        "No source-audited reset storage for this product"
    );
    // Encoding avoids path separators and collisions in arbitrary serials.
    let encode = |value: &str| {
        value
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let mut path = crate::store_path();
    path.pop();
    Ok(path.join("receiver-documents").join(format!(
        "{product}-{}-{}.json",
        encode(container),
        encode(serial)
    )))
}

/// Source taskMakerSetBrightness updates its active profile then mZ persists
/// it before the skippable physical setting task. Other profile fields survive.
pub fn brightness_document(
    current: &Value,
    profile_guid: &str,
    enabled: bool,
    value: u8,
) -> anyhow::Result<Value> {
    ensure!(value <= 100, "Brightness is not a source percent");
    let mut document = current.clone();
    ensure!(
        document["activeProfile"] == profile_guid,
        "Brightness profile is not the current source active profile"
    );
    let version = document["version"].as_u64().unwrap_or(0);
    ensure!(
        version <= MAX_SAFE_INTEGER,
        "Brightness document version exceeds 53 bits"
    );
    document["version"] = json!(if version == MAX_SAFE_INTEGER {
        0
    } else {
        version + 1
    });
    let profiles = document["profiles"]
        .as_array_mut()
        .context("Source brightness profiles are absent")?;
    let profile = profiles
        .iter_mut()
        .find(|profile| profile["guid"] == profile_guid)
        .context("Source brightness active profile is absent")?;
    let brightness = profile["brightness"]
        .as_object_mut()
        .context("Source active profile has no brightness setting")?;
    brightness.insert("isEnabled".into(), json!(enabled));
    brightness.insert("value".into(), json!(value));
    document
        .as_object_mut()
        .context("Source brightness document is not an object")?
        .remove("isDefault");
    Ok(document)
}

/// Source first producer stores its default document independently of later
/// device submission. This is the application's local storage adaptation.
pub fn persist_document(
    path: &Path,
    product: u32,
    container: &str,
    serial: &str,
    previous: Option<&Value>,
    document: &Value,
) -> anyhow::Result<()> {
    let disk = crate::read_document(path, |text| Ok(serde_json::from_str::<Value>(text)?))?;
    match (&disk, previous) {
        (None, None) => {}
        (Some(disk), Some(previous)) => ensure!(
            disk["schema_version"] == 1
                && disk["product_id"] == product
                && disk["container"] == container
                && disk["serial"] == serial
                && disk["document"] == *previous,
            "Source document owner or state changed"
        ),
        _ => anyhow::bail!("Source document changed before local submission"),
    }
    let file = json!({"schema_version":1,"product_id":product,"container":container,"serial":serial,"storage_kind":"local_source_document_adaptation","document":document,"device_refresh_complete":false});
    crate::write_document(path, &file, |text| {
        let current: Value = serde_json::from_str(text)?;
        ensure!(
            disk.as_ref() == Some(&current),
            "Source document changed during local submission"
        );
        Ok(())
    })
}
