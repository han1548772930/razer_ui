//! Current 164/241 Help taskMakerResetOBM document semantics. Local storage is
//! separate from Chromium storage; queued refresh events are not device acks.
use crate::{
    receiver_defaults::ResetDefaults,
    receiver_document::{DeviceDocument, DeviceMetadata, Extensions, Field, ProfileData},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[path = "receiver_profile_snapshot.rs"]
mod snapshot;
pub use snapshot::{ReceiverBrightness, ReceiverProfileRevision, ReceiverProfileSnapshot};

pub(crate) const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

fn unique_name(profiles: &[ProfileData], base: String) -> String {
    let mut name = base.clone();
    // Original W$: at most 100 probes; the final suffix may already exist.
    for suffix in 1..=100 {
        if !profiles.iter().any(|p| p.name.get() == Some(&name)) {
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

fn defaults(product: u32, edition: u32) -> anyhow::Result<&'static ResetDefaults> {
    crate::receiver_reset_defaults::PRODUCTS
        .iter()
        .find(|p| p.product_id == product && p.edition_id == edition)
        .context("Current source has no audited reset defaults for this product/edition")
}

fn next_version(version: u64) -> anyhow::Result<u64> {
    ensure!(
        version <= MAX_SAFE_INTEGER,
        "Source document version exceeds 53 bits"
    );
    Ok(if version == MAX_SAFE_INTEGER {
        0
    } else {
        version + 1
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileIdentity<'a> {
    name: Option<&'a str>,
    guid: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProjection<'a> {
    product_id: u32,
    active_profile: &'a str,
    profiles: Vec<ProfileIdentity<'a>>,
}

#[derive(Serialize)]
enum RefreshKind {
    #[serde(rename = "ON_INIT_BRIGHTNESS")]
    InitBrightness,
    #[serde(rename = "ON_SET_EFFECTS")]
    SetEffects,
    #[serde(rename = "ON_SET_KEYMAPPING")]
    SetKeyMapping,
}

#[derive(Serialize)]
enum RefreshOrigin {
    #[serde(rename = "MW_ACTION_FROM_LOCALSTORAGE")]
    LocalStorage,
}

#[derive(Serialize)]
struct RefreshEvent<T> {
    #[serde(rename = "type")]
    kind: RefreshKind,
    from: RefreshOrigin,
    payload: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrightnessRefresh<'a> {
    brightness: crate::receiver_defaults::Brightness,
    switch_off_lighting: crate::receiver_defaults::SwitchOffLighting,
    device_container_id: &'a str,
    version: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectsRefresh<'a> {
    effect_setting: crate::receiver_defaults::QuickEffects,
    device_container_id: &'a str,
    version: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MappingRefresh {
    mapping_list: &'static [()],
    view_index: u8,
    version: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct StoredReceiverDocument {
    schema_version: u8,
    product_id: u32,
    container: String,
    serial: String,
    document: DeviceDocument,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    storage_kind: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    runtime_projection: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    required_refresh_events: Field<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    executed_refresh_events: Field<Value>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    device_refresh_complete: Field<bool>,
    #[serde(flatten)]
    extensions: Extensions,
}

impl StoredReceiverDocument {
    fn read(text: &str) -> anyhow::Result<Self> {
        let file: Self = serde_json::from_str(text)?;
        file.document.validate()?;
        Ok(file)
    }

    fn require_owner(&self, product: u32, container: &str, serial: &str) -> anyhow::Result<()> {
        ensure!(
            self.schema_version == 1
                && self.product_id == product
                && self.container == container
                && self.serial == serial,
            "Local receiver document owner/schema mismatch"
        );
        Ok(())
    }
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
        let mut document = DeviceDocument::parse(current)?;
        document.require_scope(product, defaults.schema_version)?;
        let version = match &document.version {
            Field::Missing => 1,
            Field::Present(Some(version)) => next_version(*version)?,
            Field::Present(None) => anyhow::bail!("Source document version is null"),
        };
        let profiles = document.profiles.require_mut("profiles")?;
        ensure!(
            !profiles
                .iter()
                .any(|p| p.guid.get().is_some_and(|current| current == guid)),
            "Profile UUID already exists"
        );
        let base = if computer_name.is_empty() {
            "Default".to_owned()
        } else {
            format!("{computer_name}-Default")
        };
        let name = unique_name(profiles, base);
        let profile = defaults.profile(guid, &name);
        profiles.push(profile.clone());
        document.active_profile = Field::set(guid.to_owned());
        document.version = Field::set(version);
        let entry = document.metadata_mut(serial)?;
        entry.active_profile_guid = Field::set(guid.to_owned());
        entry
            .name
            .insert_if_missing(defaults.device_name.to_owned());
        document.is_default = Field::Missing;
        if product == 241 {
            document.report_ids = Field::set(defaults.report_ids());
        }
        let runtime_projection = serde_json::to_value(RuntimeProjection {
            product_id: product,
            active_profile: guid,
            profiles: document
                .profiles
                .require("profiles")?
                .iter()
                .map(|profile| ProfileIdentity {
                    name: profile.name.get().map(String::as_str),
                    guid: profile.guid.get().map(String::as_str),
                })
                .collect(),
        })?;
        let source = defaults.default_profile;
        let refresh_events = vec![
            serde_json::to_value(RefreshEvent {
                kind: RefreshKind::InitBrightness,
                from: RefreshOrigin::LocalStorage,
                payload: BrightnessRefresh {
                    brightness: source.brightness,
                    switch_off_lighting: source.switch_off_lighting,
                    device_container_id: container,
                    version,
                },
            })?,
            serde_json::to_value(RefreshEvent {
                kind: RefreshKind::SetEffects,
                from: RefreshOrigin::LocalStorage,
                payload: EffectsRefresh {
                    effect_setting: source.quick_effects,
                    device_container_id: container,
                    version,
                },
            })?,
            serde_json::to_value(RefreshEvent {
                kind: RefreshKind::SetKeyMapping,
                from: RefreshOrigin::LocalStorage,
                payload: MappingRefresh {
                    mapping_list: source.mappings,
                    view_index: 0,
                    version,
                },
            })?,
        ];
        Ok(Self {
            document: serde_json::to_value(document)?,
            new_profile: serde_json::to_value(profile)?,
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
    let mut document = DeviceDocument::parse(current)?;
    document.require_scope(product, source.schema_version)?;
    let active = document
        .device_metadatas
        .get()
        .and_then(|metadata| metadata.get(serial))
        .and_then(|metadata| metadata.active_profile_guid.get())
        .filter(|guid| !guid.is_empty())
        .cloned();
    let active = match active {
        Some(active) => active,
        None => {
            ensure!(
                valid_guid(guid),
                "Source cache profile requires a UUID identity"
            );
            let profiles = document.profiles.require_mut("profiles")?;
            ensure!(
                !profiles
                    .iter()
                    .any(|profile| profile.guid.get().is_some_and(|current| current == guid)),
                "Cache profile UUID already exists"
            );
            let profile = source.profile(
                guid,
                &unique_name(profiles, format!("{computer_name}-Default")),
            );
            profiles.push(profile);
            let metadata = document.metadata_mut(serial)?;
            metadata.active_profile_guid = Field::set(guid.to_owned());
            metadata
                .name
                .insert_if_missing(source.device_name.to_owned());
            guid.to_owned()
        }
    };
    document.active_profile = Field::set(active);
    Ok(serde_json::to_value(document)?)
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
    // getDefaultProfile has the literal `computerName-Default`, even if empty.
    let profile = source.profile(guid, &format!("{computer_name}-Default"));
    let mut document = source.global_document();
    document.schema_version = Field::set(source.schema_version);
    document.is_default = Field::set(true);
    document.profiles = Field::set(vec![profile]);
    document.active_profile = Field::set(guid.to_owned());
    document.device_metadatas = Field::set(
        [(
            serial.to_owned(),
            DeviceMetadata {
                active_profile_guid: Field::set(guid.to_owned()),
                name: Field::set(source.device_name.to_owned()),
                ..DeviceMetadata::default()
            },
        )]
        .into_iter()
        .collect(),
    );
    Ok(serde_json::to_value(document)?)
}

pub fn load(
    path: &Path,
    product: u32,
    container: &str,
    serial: &str,
) -> anyhow::Result<Option<Value>> {
    crate::read_document(path, |text| {
        let file = StoredReceiverDocument::read(text)?;
        file.require_owner(product, container, serial)?;
        Ok(serde_json::to_value(file.document)?)
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
    let mut path = crate::store_path()?;
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
    let mut document = DeviceDocument::parse(current)?;
    ensure!(
        document
            .active_profile
            .get()
            .is_some_and(|guid| guid == profile_guid),
        "Brightness profile is not the current source active profile"
    );
    let version = match document.version {
        Field::Missing => 0,
        Field::Present(Some(version)) => version,
        Field::Present(None) => anyhow::bail!("Brightness document version is null"),
    };
    document.version = Field::set(next_version(version)?);
    let profiles = document.profiles.require_mut("profiles")?;
    let profile = profiles
        .iter_mut()
        .find(|profile| profile.guid.get().is_some_and(|guid| guid == profile_guid))
        .context("Source brightness active profile is absent")?;
    let brightness = profile.brightness.require_mut("brightness")?;
    brightness.is_enabled = Field::set(enabled);
    brightness.value = Field::set(value);
    document.is_default = Field::Missing;
    Ok(serde_json::to_value(document)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const OLD_GUID: &str = "11111111-2222-3333-4444-555555555555";
    const NEW_GUID: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    const SERIAL: &str = "actual-observed-serial";
    const CONTAINER: &str = "{11111111-2222-3333-4444-555555555555}";

    struct TestFile(std::path::PathBuf);
    impl TestFile {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                    "razer-receiver-document-{}-{}.json",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )))
        }
    }
    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(self.0.with_extension("json.bak"));
        }
    }

    #[test]
    fn compiled_defaults_preserve_original_optional_fields_and_product_branches() {
        for product in [164, 241] {
            let fixed = defaults(product, 0).unwrap();
            let mut profile = json!({"guid":"","name":" ",
                "brightness":{"isEnabled":true,"value":100},
                "switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":1},
                "quickEffects":{"selectedEffectId":3},"mappings":[]});
            let mut global = json!({"version":1,"vendorId":5426,"productId":product,
                "category":"ACCESSORY","reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"}});
            if product == 241 {
                profile["canPairTwoDevices"] = json!(true);
                global["canPairTwoDevices"] = json!(true);
            }
            assert_eq!(
                serde_json::to_value(fixed.default_profile).unwrap(),
                profile
            );
            assert_eq!(serde_json::to_value(fixed.default_global).unwrap(), global);
            assert_eq!(
                serde_json::to_value(
                    fixed.profile(fixed.default_profile.guid, fixed.default_profile.name)
                )
                .unwrap(),
                profile
            );
            assert_eq!(
                serde_json::to_value(fixed.global_document()).unwrap(),
                global
            );
            assert_eq!(fixed.schema_version, 13);
        }
        assert!(defaults(164, 1).is_err());
        assert!(defaults(756, 0).is_err());
    }

    #[test]
    fn reset_preserves_profiles_and_extensions_with_source_name_and_version_semantics() {
        let mut current = initial_document(164, 0, SERIAL, "", OLD_GUID).unwrap();
        assert_eq!(current["profiles"][0]["name"], "-Default");
        current["version"] = json!(MAX_SAFE_INTEGER);
        current["profiles"][0]["name"] = json!("Default");
        current["profiles"][0]["futureSetting"] = json!({"mode":2});
        current["deviceMetadatas"][SERIAL]["name"] = json!("User name");
        current["deviceMetadatas"][SERIAL]["futureMetadata"] = json!(null);
        current["futureDocument"] = json!([1,{"flag":true}]);
        current.as_object_mut().unwrap().remove("reportIDs");
        let previous = current.clone();
        let plan = ResetPlan::prepare(164, 0, &current, SERIAL, CONTAINER, "", NEW_GUID).unwrap();
        assert_eq!(current, previous);
        assert_eq!(plan.document()["profiles"][0], previous["profiles"][0]);
        assert_eq!(plan.new_profile()["name"], "Default 1");
        assert_eq!(plan.document()["version"], 0);
        assert_eq!(plan.document()["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(
            plan.document()["futureDocument"],
            previous["futureDocument"]
        );
        assert_eq!(
            plan.document()["deviceMetadatas"][SERIAL]["name"],
            "User name"
        );
        assert!(
            plan.document()["deviceMetadatas"][SERIAL]
                .get("futureMetadata")
                .unwrap()
                .is_null()
        );
        assert!(plan.document().get("isDefault").is_none());
        assert!(plan.document().get("reportIDs").is_none());
        assert!(plan.new_profile().get("canPairTwoDevices").is_none());
        assert_eq!(
            plan.refresh_events()
                .iter()
                .map(|event| event["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["ON_INIT_BRIGHTNESS", "ON_SET_EFFECTS", "ON_SET_KEYMAPPING"]
        );
        assert_eq!(
            plan.refresh_events()[0]["payload"]["brightness"],
            json!({"isEnabled":true,"value":100})
        );
        assert_eq!(
            plan.refresh_events()[2]["payload"]["mappingList"],
            json!([])
        );

        let mut current = initial_document(241, 0, SERIAL, "PC", OLD_GUID).unwrap();
        current["reportIDs"] = json!({"99":"extension"});
        let plan = ResetPlan::prepare(241, 0, &current, SERIAL, CONTAINER, "PC", NEW_GUID).unwrap();
        assert_eq!(plan.new_profile()["canPairTwoDevices"], true);
        assert_eq!(
            plan.document()["reportIDs"],
            json!({"4":"razerKeyReportID","5":"hardwareEventReportID"})
        );
    }

    #[test]
    fn serial_cache_distinguishes_an_absent_profile_from_a_real_observation() {
        let mut current = initial_document(164, 0, SERIAL, "", OLD_GUID).unwrap();
        let observed = prepare_serial_cache(164, 0, &current, SERIAL, "", NEW_GUID).unwrap();
        assert_eq!(observed, current);
        current["deviceMetadatas"][SERIAL]["activeProfileGuid"] = json!("");
        current["deviceMetadatas"][SERIAL]["name"] = json!(null);
        let prepared = prepare_serial_cache(164, 0, &current, SERIAL, "", NEW_GUID).unwrap();
        assert_eq!(prepared["profiles"].as_array().unwrap().len(), 2);
        assert_eq!(prepared["profiles"][1]["name"], "-Default 1");
        assert_eq!(prepared["activeProfile"], NEW_GUID);
        assert_eq!(prepared["version"], current["version"]);
        assert!(
            prepared["deviceMetadatas"][SERIAL]
                .get("name")
                .unwrap()
                .is_null()
        );
    }

    #[test]
    fn corrupted_known_fields_and_missing_profile_arrays_fail_explicitly() {
        let current = initial_document(164, 0, SERIAL, "PC", OLD_GUID).unwrap();
        for (field, value) in [
            ("productId", json!("164")),
            ("profiles", json!(false)),
            ("schemaVersion", json!(12)),
            ("version", json!(null)),
            ("version", json!(MAX_SAFE_INTEGER + 1)),
        ] {
            let mut invalid = current.clone();
            invalid[field] = value;
            assert!(
                ResetPlan::prepare(164, 0, &invalid, SERIAL, CONTAINER, "PC", NEW_GUID).is_err(),
                "{field}"
            );
        }
        let mut missing = current.clone();
        missing.as_object_mut().unwrap().remove("profiles");
        assert!(ResetPlan::prepare(164, 0, &missing, SERIAL, CONTAINER, "PC", NEW_GUID).is_err());
        let mut invalid = current.clone();
        invalid["profiles"][0]["brightness"]["value"] = json!(101);
        assert!(brightness_document(&invalid, OLD_GUID, true, 75).is_err());
        assert!(brightness_document(&current, NEW_GUID, true, 75).is_err());
        assert!(brightness_document(&current, OLD_GUID, true, 101).is_err());
    }

    #[test]
    fn previous_receiver_document_migration_checks_schema_owner_and_complete_source_bytes() {
        let path = TestFile::new();
        let mut document = initial_document(164, 0, SERIAL, "PC", OLD_GUID).unwrap();
        document["futureDocumentExtension"] = json!({"value":null});
        let stored = json!({"schema_version":1,"product_id":164,"container":CONTAINER,
            "serial":SERIAL,"document":document,"futureFileExtension":{"flag":true}});
        std::fs::write(&path.0, serde_json::to_vec(&stored).unwrap()).unwrap();
        assert_eq!(
            load(&path.0, 164, CONTAINER, SERIAL).unwrap(),
            Some(document)
        );
        assert!(load(&path.0, 241, CONTAINER, SERIAL).is_err());
        assert!(load(&path.0, 164, "other-container", SERIAL).is_err());
        assert!(load(&path.0, 164, CONTAINER, "other-serial").is_err());
        let mut invalid = stored.clone();
        invalid["schema_version"] = json!(2);
        std::fs::write(&path.0, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(load(&path.0, 164, CONTAINER, SERIAL).is_err());
        invalid = stored;
        invalid["document"]["profiles"][0]["brightness"]["value"] = json!(101);
        std::fs::write(&path.0, serde_json::to_vec(&invalid).unwrap()).unwrap();
        assert!(load(&path.0, 164, CONTAINER, SERIAL).is_err());
    }
}
