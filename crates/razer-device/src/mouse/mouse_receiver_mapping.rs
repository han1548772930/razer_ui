//! Current PID158 profile compiler and single-peer receiver publication.
//! Local documents retain logical identity; receiver projections are ephemeral.
use crate::{
    mapping_engine::{EngineContext, MappingEngine},
    mapping_engine_hash::source_hash,
    mapping_submission::next_source_version,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const STORAGE_FIELD: &str = "_mouse158_mapping_document";

/// These branches come from current runtime feature observations, not product
/// labels or the number of cached pairing entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverRoute {
    Normal,
    Linker,
    Mika,
}

/// The retained owner supplies the selected slave's saved, compiled document
/// after applying its observed side panel and keyboard transforms.
pub struct SlaveSnapshot<'a> {
    pub document: &'a Value,
    pub serial: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationRole {
    None,
    Master,
    Slave,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingEntry {
    dongle_id: u32,
    device_container_id: String,
    master: PairingOwner,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingOwner {
    dongle_id: u32,
}

/// Current66825 q uses the first matching entry in source storage order.
/// `None` means no topology observation, distinct from an observed empty list.
pub fn publication_role(
    ordered_records: Option<&[Value]>,
    physical_product: u32,
    container: &str,
) -> Result<Option<PublicationRole>> {
    ensure!(
        !container.is_empty(),
        "Receiver container has not been observed"
    );
    let Some(records) = ordered_records else {
        return Ok(None);
    };
    for record in records {
        let record: PairingEntry = serde_json::from_value(record.clone())
            .context("Receiver pairing record is incomplete")?;
        if record.device_container_id == container
            && (record.dongle_id == physical_product || record.master.dongle_id == physical_product)
        {
            return Ok(Some(if record.dongle_id == physical_product {
                PublicationRole::Slave
            } else {
                PublicationRole::Master
            }));
        }
    }
    Ok(Some(PublicationRole::None))
}

/// Root device settings own serial/profile selection. The compiled mapping
/// document copies that external boundary; it does not infer observed serials.
pub fn prepare_document_with_device_settings(
    profiles: &[Value],
    active_guid: &str,
    previous: Option<&Value>,
    settings: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Settings {
        #[serde(default)]
        device_metadatas: Option<BTreeMap<String, Metadata>>,
    }
    let settings: Option<Settings> = settings
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .context("PID158 device settings are invalid")?;
    let result = prepare_document(profiles, active_guid, previous, fresh_guids)?;
    let mut result = Document::parse(&result)?;
    if let Some(metadata) = settings.and_then(|settings| settings.device_metadatas) {
        result.device_metadatas = Some(metadata);
    }
    Ok(serde_json::to_value(result)?)
}

/// Current30525 It/_t installs an ephemeral inter-device OTFS engine and
/// restores the ordinary document on release. Saved bindings and version stay
/// intact; receiver routing is applied separately from this logical document.
pub fn interdevice_otfs_document(document: &Value) -> Result<Value> {
    let mut document = Document::parse(document)?;
    ensure!(
        document.product_id == 158 && document.dual_linked.is_none(),
        "OTFS requires the logical PID158 document"
    );
    let profile = document
        .profiles
        .iter_mut()
        .find(|profile| profile.guid == document.active_profile)
        .context("Active PID158 OTFS profile is absent")?;
    let mut context = EngineContext {
        default_turbos: Some(document.default_turbos.clone()),
        synapse_turbos: Some(document.synapse_turbos.clone()),
        ..Default::default()
    };
    for key in &DKM_KEYS {
        context
            .dkm_keys
            .insert(key.input_id.into(), serde_json::to_value(key)?);
    }
    let projection = MappingEngine::new(158)?.generate_mouse_otfs(
        &profile.mappings,
        &default_bindings()?,
        &mut context,
    )?;
    profile.app_engine = Some(serde_json::from_value(projection.profile)?);
    profile
        .extensions
        .insert("defaultMappings".into(), projection.defaults.clone());
    document.default_mappings.app_engine = serde_json::from_value(projection.defaults)?;
    Ok(serde_json::to_value(document)?)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    guid: String,
    name: String,
    mappings: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    app_engine: Option<Engine>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Engine {
    mappings: Vec<Mapping>,
    hash: String,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

impl Engine {
    fn rehash(&mut self) -> Result<()> {
        self.hash = source_hash(&serde_json::to_value(&*self)?)?;
        Ok(())
    }

    fn route(&mut self, unit: u16, report: u8, cascades: bool) {
        for mapping in &mut self.mappings {
            mapping.input.route(unit, report);
            if cascades && mapping.output.kind == "cascade" {
                if let Some(children) = &mut mapping.output.cascade {
                    for child in children {
                        child.input.route(unit, report);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Mapping {
    input: Input,
    output: Output,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Input {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    report_id: Option<u8>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

impl Input {
    fn route(&mut self, unit: u16, report: u8) {
        match self.kind.as_str() {
            "mouse" | "keyboard" => self.unit_id = Some(unit),
            "razerKey" => self.report_id = Some(report),
            _ => {}
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Output {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cascade: Option<Vec<Mapping>>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Defaults {
    app_engine: Engine,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dual_linked: Option<bool>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

impl Document {
    fn parse(value: &Value) -> Result<Self> {
        let document: Self = serde_json::from_value(value.clone())
            .context("Receiver mapping document is incomplete")?;
        ensure!(document.vendor_id == 5426, "Mapping vendor differs");
        ensure!(!document.profiles.is_empty(), "Mapping profiles are absent");
        let mut identities = std::collections::BTreeSet::new();
        for profile in &document.profiles {
            ensure!(
                !profile.guid.is_empty() && identities.insert(&profile.guid),
                "Mapping profile identity is empty or duplicated"
            );
        }
        Ok(document)
    }

    fn profile_for_serial(&self, serial: &str) -> Option<usize> {
        let selected = self
            .device_metadatas
            .as_ref()
            .and_then(|values| values.get(serial));
        match selected {
            Some(metadata) => self
                .profiles
                .iter()
                .position(|profile| profile.guid == metadata.active_profile_guid),
            None => (!self.profiles.is_empty()).then_some(0),
        }
    }
}

/// Current66825 single-peer receiver projection. Callers retain logical saved
/// documents and discard this projection on connection or pairing changes.
pub fn compose_receiver(
    master: &Value,
    physical_product: u32,
    master_serial: &str,
    route: ReceiverRoute,
    slave: Option<SlaveSnapshot<'_>>,
) -> Result<Value> {
    ensure!(
        matches!(physical_product, 158 | 159),
        "PID158 receiver physical identity differs"
    );
    ensure!(
        !master_serial.is_empty(),
        "Master serial has not been observed"
    );
    let mut result = Document::parse(master)?;
    ensure!(
        result.product_id == 158 && result.dual_linked.is_none(),
        "Receiver projection requires the logical saved master document"
    );
    let selected = result.profile_for_serial(master_serial).unwrap_or(0);
    result.product_id = physical_product;
    result.dual_linked = Some(true);
    result.default_mappings.app_engine.route(0, 4, false);
    let profile = &mut result.profiles[selected];
    let engine = profile.app_engine.get_or_insert_with(|| Engine {
        mappings: Vec::new(),
        hash: String::new(),
        extensions: Map::new(),
    });
    engine.route(0, 4, true);
    let (unit, report) = match route {
        ReceiverRoute::Normal => (43981, 8),
        ReceiverRoute::Linker | ReceiverRoute::Mika => (0, 4),
    };
    if let Some(slave) = slave {
        ensure!(
            !slave.serial.is_empty(),
            "Slave serial has not been observed"
        );
        let mut slave_document = Document::parse(slave.document)?;
        ensure!(
            slave_document.dual_linked.is_none(),
            "A slave projection cannot be merged twice"
        );
        let selected_slave = slave_document.profile_for_serial(slave.serial);
        result.report_ids.extend(slave_document.report_ids);
        slave_document
            .default_mappings
            .app_engine
            .route(unit, report, false);
        result
            .default_mappings
            .app_engine
            .mappings
            .extend(slave_document.default_mappings.app_engine.mappings);
        if let Some(index) = selected_slave {
            let slave_profile = &mut slave_document.profiles[index];
            profile.mappings.append(&mut slave_profile.mappings);
            if let Some(slave_engine) = &mut slave_profile.app_engine {
                slave_engine.route(unit, report, true);
                engine.mappings.append(&mut slave_engine.mappings);
                if let Some(gamemode) = slave_engine.extensions.get("gamemode") {
                    if match gamemode {
                        Value::Null => false,
                        Value::Bool(value) => *value,
                        Value::Number(value) => value.as_f64() != Some(0.0),
                        Value::String(value) => !value.is_empty(),
                        Value::Array(_) | Value::Object(_) => true,
                    } {
                        engine
                            .extensions
                            .insert("gamemode".into(), gamemode.clone());
                    }
                }
            }
        }
    }
    result.report_ids.insert(report, "razerKeyReportID".into());
    result.default_mappings.app_engine.rehash()?;
    engine.rehash()?;
    Ok(serde_json::to_value(result)?)
}

/// Update only the metadata owned by this observed serial after the profile
/// selection is saved. A cache label or an empty serial cannot select hardware.
pub fn select_profile_for_serial(document: &Value, serial: &str) -> Result<Value> {
    ensure!(!serial.is_empty(), "Mapping serial has not been observed");
    let mut document = Document::parse(document)?;
    ensure!(
        document.product_id == 158 && document.dual_linked.is_none(),
        "Profile metadata requires a local PID158 document"
    );
    ensure!(
        document
            .profiles
            .iter()
            .any(|profile| profile.guid == document.active_profile),
        "Active mapping profile is absent"
    );
    let metadata = document
        .device_metadatas
        .get_or_insert_with(BTreeMap::new)
        .entry(serial.into())
        .or_insert_with(|| Metadata {
            active_profile_guid: document.active_profile.clone(),
            extensions: Map::new(),
        });
    metadata.active_profile_guid = document.active_profile.clone();
    Ok(serde_json::to_value(document)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DefaultBinding {
    output_type: &'static str,
    input_type: &'static str,
    #[serde(rename = "inputID")]
    input_id: &'static str,
    is_hyper_shift: bool,
    sensitivity_group: Sensitivity,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sensitivity {
    sensitivity_assignment: &'static str,
    independent: bool,
    x: u16,
    y: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DkmKey {
    #[serde(rename = "inputID")]
    input_id: &'static str,
    button_key: &'static str,
    key: u8,
}

const DKM_KEYS: [DkmKey; 3] = [
    DkmKey {
        input_id: "DKM_M_01",
        button_key: "SensitivityStageUp",
        key: 32,
    },
    DkmKey {
        input_id: "DKM_M_02",
        button_key: "SensitivityStageDown",
        key: 33,
    },
    DkmKey {
        input_id: "DKM_SB_03",
        button_key: "CycleUpSensitivityStages",
        key: 82,
    },
];

fn default_bindings() -> Result<Vec<Value>> {
    // DeviceInfo3298 includes DKM_SB_01 profile navigation in each layer,
    // but its DKMKEYS has no such key. Factory77898 Dt/Tt emits no input
    // for those two rows; it does not synthesize a firmware identifier.
    [
        ("DKM_M_01", "DPI_Up", false),
        ("DKM_M_02", "DPI_Down", false),
        ("DKM_M_01", "DPI_Up", true),
        ("DKM_M_02", "DPI_Down", true),
        ("DKM_SB_03", "DPI_CycleUp", false),
        ("DKM_SB_03", "DPI_CycleUp", true),
    ]
    .into_iter()
    .map(|(input_id, sensitivity_assignment, is_hyper_shift)| {
        Ok(serde_json::to_value(DefaultBinding {
            output_type: "sensitivityGroup",
            input_type: "DKMInput",
            input_id,
            is_hyper_shift,
            sensitivity_group: Sensitivity {
                sensitivity_assignment,
                independent: false,
                x: 800,
                y: 800,
            },
        })?)
    })
    .collect()
}

fn compile(
    records: &[Value],
    engine: &MappingEngine,
    context: &mut EngineContext,
) -> Result<Engine> {
    let mut compiled: Engine =
        serde_json::from_value(engine.generate(records, context)?.app_engine)?;
    // Current30525 mt removes old OTFS interception before appending the
    // profile's current scroll interception; it never changes saved mappings.
    compiled
        .mappings
        .retain(|mapping| mapping.input.extensions.get("otfs") != Some(&Value::Bool(true)));
    if records.iter().any(|record| {
        record
            .get("sensitivityGroup")
            .and_then(|group| group.get("sensitivityAssignment"))
            .and_then(Value::as_str)
            == Some("DPI_OnTheFly")
    }) {
        let scroll = ["ScrollUp", "ScrollDown"].map(|input| {
            serde_json::json!({
                "outputType":"sensitivityGroup", "inputType":"MouseInput", "inputID":input,
                "sensitivityGroup":{"sensitivityAssignment":"OTFS_Scroll"}
            })
        });
        let scroll: Engine = serde_json::from_value(engine.generate(&scroll, context)?.app_engine)?;
        compiled.mappings.extend(scroll.mappings);
    }
    compiled.rehash()?;
    Ok(compiled)
}

/// Compile PID158 local profiles without reading or changing device settings.
/// The caller owns persistence and publishes only after the save completes.
pub fn prepare_document(
    profiles: &[Value],
    active_guid: &str,
    previous: Option<&Value>,
    fresh_guids: impl IntoIterator<Item = String>,
) -> Result<Value> {
    let previous = previous.map(Document::parse).transpose()?;
    if let Some(previous) = &previous {
        ensure!(
            previous.product_id == 158 && previous.dual_linked.is_none(),
            "A receiver projection cannot replace the local PID158 document"
        );
    }
    let engine = MappingEngine::new(158)?;
    let mut context = EngineContext {
        default_turbos: previous.as_ref().map(|value| value.default_turbos.clone()),
        synapse_turbos: Some(
            previous
                .as_ref()
                .map(|value| value.synapse_turbos.clone())
                .unwrap_or_default(),
        ),
        fresh_guids: fresh_guids.into_iter().collect(),
        ..Default::default()
    };
    for key in &DKM_KEYS {
        context
            .dkm_keys
            .insert(key.input_id.into(), serde_json::to_value(key)?);
    }
    if context.default_turbos.as_ref().is_none_or(Vec::is_empty) {
        engine.generate_default_turbos(&mut context)?;
    }
    let defaults = serde_json::from_value(
        engine
            .generate(&default_bindings()?, &mut context)?
            .app_engine,
    )?;
    let mut compiled = Vec::with_capacity(profiles.len());
    let mut ids = std::collections::BTreeSet::new();
    for source in profiles {
        let mut profile: Profile = serde_json::from_value(source.clone())
            .context("PID158 profile fields are incomplete")?;
        ensure!(
            !profile.guid.is_empty() && ids.insert(profile.guid.clone()),
            "PID158 profile identity is empty or duplicated"
        );
        let records = profile
            .mappings
            .iter()
            .filter(|record| {
                record.get("inputType").and_then(Value::as_str) != Some("DKMInput")
                    || record
                        .get("inputID")
                        .and_then(Value::as_str)
                        .is_some_and(|id| context.dkm_keys.contains_key(id))
            })
            .cloned()
            .collect::<Vec<_>>();
        profile.app_engine = Some(compile(&records, &engine, &mut context)?);
        compiled.push(profile);
    }
    ensure!(ids.contains(active_guid), "PID158 active profile is absent");
    if let Some(previous) = &previous {
        // Current30525 Ve removes only formerly referenced turbo identities
        // that no compiled profile still uses. Unrelated stored turbos survive.
        let references = |profiles: &[Profile]| -> std::collections::BTreeSet<String> {
            profiles
                .iter()
                .filter_map(|profile| profile.app_engine.as_ref())
                .flat_map(|engine| engine.mappings.iter())
                .filter(|mapping| mapping.output.kind == "turbo")
                .filter_map(|mapping| {
                    mapping
                        .output
                        .extensions
                        .get("guid")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect()
        };
        let old = references(&previous.profiles);
        let current = references(&compiled);
        if let Some(turbos) = &mut context.synapse_turbos {
            turbos.retain(|turbo| {
                turbo
                    .get("guid")
                    .and_then(Value::as_str)
                    .is_none_or(|guid| !old.contains(guid) || current.contains(guid))
            });
        }
    }
    let document = Document {
        version: next_source_version(previous.as_ref().map(|value| value.version))?,
        vendor_id: 5426,
        product_id: 158,
        category: "MOUSE".into(),
        report_ids: [
            (4, "razerKeyReportID".into()),
            (5, "hardwareEventReportID".into()),
        ]
        .into(),
        profiles: compiled,
        active_profile: active_guid.into(),
        default_mappings: Defaults {
            app_engine: defaults,
        },
        default_turbos: context
            .default_turbos
            .context("PID158 default turbos are absent")?,
        synapse_turbos: context
            .synapse_turbos
            .context("PID158 profile turbos are absent")?,
        device_metadatas: previous
            .as_ref()
            .and_then(|value| value.device_metadatas.clone()),
        dual_linked: None,
        extensions: previous.map(|value| value.extensions).unwrap_or_default(),
    };
    Ok(serde_json::to_value(document)?)
}

#[cfg(test)]
#[path = "mouse_receiver_mapping_tests.rs"]
mod tests;
