//! Current756 MW modules99048/14339/75938/32637. The queue invalidates pending
//! requests without aborting an in-flight command. A generation guard controls
//! each subsequent side effect; BLE restores only an observed onboard slot.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub const PRODUCT_ID: u32 = 756;
pub const RECEIVER_ID: u32 = 757;
pub const BLE_ID: u32 = 758;
pub const STORAGE_FIELD: &str = "_keyboard756_mapping_document";

/// A validated local profile collection. Import publication can append missing
/// profiles without replacing an observed active profile or device settings.
#[derive(Clone, Debug)]
pub struct ProfileCollection {
    profiles: Vec<CollectionProfile>,
}
#[derive(Clone, Debug)]
struct CollectionProfile {
    guid: String,
    settings: Value,
}
impl ProfileCollection {
    pub fn from_document(document: &Value) -> anyhow::Result<Self> {
        let profiles = document
            .get("profiles")
            .and_then(Value::as_array)
            .context("756 profile collection is absent")?;
        let mut guids = BTreeSet::new();
        let profiles = profiles
            .iter()
            .map(|profile| -> anyhow::Result<_> {
                let guid = collection_guid(profile)?;
                ensure!(guids.insert(guid), "756 profile collection repeats a GUID");
                Ok(CollectionProfile {
                    guid: guid.to_owned(),
                    settings: profile.clone(),
                })
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(Self { profiles })
    }

    /// Existing profiles and all root fields remain owned by the target. The
    /// collection changes neither source version nor device/serial metadata.
    pub fn append_to(&self, document: &mut Value) -> anyhow::Result<bool> {
        let profiles = document
            .get("profiles")
            .and_then(Value::as_array)
            .context("756 retained profile collection is absent")?;
        let mut guids = BTreeSet::new();
        for profile in profiles {
            ensure!(
                guids.insert(collection_guid(profile)?),
                "756 retained profile collection repeats a GUID"
            );
        }
        let appended = self
            .profiles
            .iter()
            .filter(|profile| !guids.contains(profile.guid.as_str()))
            .map(|profile| profile.settings.clone())
            .collect::<Vec<_>>();
        if appended.is_empty() {
            return Ok(false);
        }
        document
            .get_mut("profiles")
            .and_then(Value::as_array_mut)
            .expect("validated retained profile collection")
            .extend(appended);
        Ok(true)
    }
}

fn collection_guid(profile: &Value) -> anyhow::Result<&str> {
    profile
        .get("guid")
        .and_then(Value::as_str)
        .filter(|guid| !guid.is_empty())
        .context("756 profile collection GUID is absent or invalid")
}

/// Explicit source task-maker operations survive the local save queue. They
/// carry the old mapping owner so clearing a profile cannot lose the keys
/// which must be restored on the device.
#[derive(Clone, Debug)]
pub enum ProfileMutation {
    Reset {
        previous_profile: Value,
        bindings_only: bool,
    },
    Delete {
        previous_active: Value,
        deleted_guid: String,
    },
}

/// Current50574 Xf/If retain name/GUID and restore only mappings for the
/// keybind action. Registered presets inject their source mappings; an
/// ordinary user profile has no injected mappings.
pub fn reset_profile_candidate(profile: &Value, bindings_only: bool) -> anyhow::Result<Value> {
    let guid = profile["guid"]
        .as_str()
        .context("756 reset profile GUID is absent")?;
    let name = profile["name"]
        .as_str()
        .context("756 reset profile name is absent")?;
    let source = crate::keyboard_latest_mapping::source();
    let mut candidate = if bindings_only {
        profile.clone()
    } else {
        crate::keyboard_latest_mapping::default_profile()
    };
    candidate["guid"] = json!(guid);
    candidate["name"] = json!(name);
    candidate["mappings"] = json!([]);
    let preset = crate::keyboard_latest_mapping::is_onboard_preset(guid);
    if preset {
        let mappings = source["preset_mappings"]
            .as_array()
            .context("756 preset mapping table is absent")?
            .iter()
            .find(|entry| entry[0] == guid)
            .context("756 registered preset mappings are absent")?;
        ensure!(
            mappings[1].is_array(),
            "756 preset mappings are not an array"
        );
        candidate["mappings"] = mappings[1].clone();
    }
    Ok(candidate)
}

pub fn latest_restore(physical_product_id: u32, ble: bool) -> bool {
    !ble && matches!(physical_product_id, PRODUCT_ID | RECEIVER_ID)
}

#[derive(Clone, Debug)]
pub struct Generation {
    current: Arc<AtomicU64>,
    value: u64,
}
impl Generation {
    pub fn is_current(&self) -> bool {
        self.current.load(Ordering::Acquire) == self.value
    }
    pub fn check(&self) -> anyhow::Result<()> {
        ensure!(
            self.is_current(),
            "Keyboard restore generation was superseded"
        );
        Ok(())
    }
}

pub struct Work<T> {
    pub id: u64,
    pub value: T,
    pub generation: Generation,
}
struct Pending<T> {
    work: Work<T>,
    ready_at: Instant,
}
pub struct Coordinator<T> {
    generation: Arc<AtomicU64>,
    pending: Option<Pending<T>>,
    updates: VecDeque<Work<T>>,
    running: bool,
}
impl<T> Default for Coordinator<T> {
    fn default() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
            pending: None,
            updates: VecDeque::new(),
            running: false,
        }
    }
}
impl<T> Coordinator<T> {
    pub fn is_idle(&self) -> bool {
        !self.running && self.pending.is_none() && self.updates.is_empty()
    }

    /// IDs returned here resolve as superseded; already-running work is guarded
    /// but remains owned until `finish`, matching source Promise completion.
    pub fn invalidate(&mut self) -> Vec<u64> {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let mut ids = self
            .pending
            .take()
            .map(|pending| vec![pending.work.id])
            .unwrap_or_default();
        ids.extend(self.updates.drain(..).map(|work| work.id));
        ids
    }
    fn work(&self, id: u64, value: T) -> Work<T> {
        Work {
            id,
            value,
            generation: Generation {
                current: self.generation.clone(),
                value: self.generation.load(Ordering::Acquire),
            },
        }
    }
    pub fn request(&mut self, id: u64, value: T, now: Instant, delay: Duration) -> Vec<u64> {
        let superseded = self.invalidate();
        self.pending = Some(Pending {
            work: self.work(id, value),
            ready_at: now + delay,
        });
        superseded
    }
    pub fn enqueue_update(&mut self, id: u64, value: T) {
        self.updates.push_back(self.work(id, value));
    }
    pub fn take_ready(&mut self, now: Instant) -> Option<Work<T>> {
        if self.running
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.ready_at > now)
        {
            return None;
        }
        let work = self
            .pending
            .take()
            .map(|pending| pending.work)
            .or_else(|| self.updates.pop_front());
        self.running = work.is_some();
        work
    }
    pub fn finish(&mut self) {
        self.running = false;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub slot_id: u8,
    pub guid: String,
}

/// Current51765.Ye/Ve display metadata. The inventory reader supplies every
/// value; guest ownership is the retained source constant, never a lock getter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardProfile {
    slot_id: u8,
    guid: String,
    name: String,
    owner: String,
    locked: bool,
}
impl OnboardProfile {
    pub fn observe(
        metadata: &crate::keyboard_latest_protocol::ProfileMetadata,
    ) -> anyhow::Result<Self> {
        let guest = crate::keyboard_latest_mapping::guest_profile_owner();
        let preset = crate::keyboard_latest_mapping::is_onboard_preset(&metadata.guid);
        let factory = metadata.name == "Factory Default";
        let factory_slot = preset && factory && metadata.slot_id == 2;
        let owner = metadata.owner.replace('\0', "");
        Ok(Self {
            slot_id: metadata.slot_id,
            guid: metadata.guid.clone(),
            name: metadata.name.clone(),
            locked: !((preset && !factory) || (!factory_slot && owner == guest)),
            owner,
        })
    }
    pub fn slot_id(&self) -> u8 {
        self.slot_id
    }
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn owner(&self) -> &str {
        &self.owner
    }
    pub fn is_locked(&self) -> bool {
        self.locked
    }
}
pub fn slots(observed: &[Slot]) -> Vec<Slot> {
    let mut result = [
        (2, "af06d371-861f-4b98-8d78-bfaef5cfdebf"),
        (3, "d363ba48-79a8-422c-b405-29a07eff7e8e"),
        (4, "2c481cef-62e0-4fe6-85a1-4d134c279c23"),
        (5, "474c90e3-7eab-4aa0-8030-34c43cbc707a"),
        (6, "177a20d5-f30a-4a52-923e-4fadb7db8392"),
    ]
    .into_iter()
    .map(|(slot_id, guid)| Slot {
        slot_id,
        guid: guid.into(),
    })
    .collect::<Vec<_>>();
    for slot in observed.iter().filter(|slot| slot.slot_id > 0) {
        if let Some(old) = result.iter_mut().find(|old| old.slot_id == slot.slot_id) {
            *old = slot.clone();
        } else {
            result.push(slot.clone());
        }
    }
    result.retain(|slot| !slot.guid.is_empty());
    result
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BleSwitch {
    Guid(String),
    Slot(u8),
    Navigation(String),
}
#[derive(Clone, Debug, Default)]
pub struct BleProfiles {
    deferred: Option<Value>,
    hypershift: bool,
}
impl BleProfiles {
    pub fn append_collection(&mut self, profiles: &ProfileCollection) -> anyhow::Result<()> {
        if let Some(document) = &self.deferred {
            let mut document = document.clone();
            profiles.append_to(&mut document)?;
            self.deferred = Some(document);
        }
        Ok(())
    }

    pub fn document<'a>(&'a self, stored: &'a Value) -> &'a Value {
        self.deferred.as_ref().unwrap_or(stored)
    }
    pub fn target<'a>(
        &self,
        stored: &Value,
        slots: &'a [Slot],
        target: &BleSwitch,
    ) -> anyhow::Result<&'a Slot> {
        let doc = self.document(stored);
        let navigation = match target {
            BleSwitch::Navigation(name) => Some(navigate_ble(doc, slots, name, "")?),
            _ => None,
        };
        let slot = slots.iter().find(|slot| match target { BleSwitch::Guid(guid) => slot.guid == *guid, BleSwitch::Slot(id) => slot.slot_id == *id, BleSwitch::Navigation(_)=>navigation.as_ref()==Some(&slot.guid) }).context("This profile has no known onboard slot. Save or refresh its onboard profile through USB before switching over BLE.")?;
        ensure!(
            doc["profiles"].as_array().is_some_and(|profiles| profiles
                .iter()
                .any(|profile| profile["guid"] == slot.guid)),
            "Stored BLE slot has no local profile"
        );
        Ok(slot)
    }
    /// Called only after the real slot setter ACK (or real slot notification).
    /// The caller publishes the returned profile before attempting persistence.
    pub fn switched(
        &mut self,
        stored: &Value,
        slot: &Slot,
        from_notification: bool,
        defer: bool,
    ) -> anyhow::Result<Option<Value>> {
        let previous = self.document(stored);
        if from_notification && previous["activeProfile"] == slot.guid {
            return Ok(None);
        }
        let mut doc = previous.clone();
        doc["activeProfile"] = json!(slot.guid);
        doc["version"] = json!(crate::mapping_submission::next_source_version(
            previous
                .get("version")
                .map(|version| version.as_u64().context("BLE profile version is invalid"))
                .transpose()?
        )?);
        if defer && self.hypershift {
            self.deferred = Some(doc.clone());
        } else {
            self.deferred = None;
        }
        Ok(Some(doc))
    }
    pub fn persistence_deferred(&self) -> bool {
        self.deferred.is_some()
    }
    pub fn switched_for_serial(
        &mut self,
        stored: &Value,
        slot: &Slot,
        from_notification: bool,
        defer: bool,
        serial: &str,
        device_name: &str,
    ) -> anyhow::Result<Option<Value>> {
        let Some(mut document) = self.switched(stored, slot, from_notification, defer)? else {
            return Ok(None);
        };
        set_active_profile_for_serial(&mut document, serial, device_name)?;
        if self.deferred.is_some() {
            self.deferred = Some(document.clone());
        }
        Ok(Some(document))
    }
    pub fn hypershift(&mut self, enabled: bool) {
        self.hypershift = enabled;
    }
    pub fn deferred(&self) -> Option<&Value> {
        self.deferred.as_ref()
    }
    /// Clear only after a successful save so failure retains the deferred doc.
    pub fn persisted(&mut self) {
        self.deferred = None;
    }
    pub fn reset(&mut self) {
        self.deferred = None;
        self.hypershift = false;
    }
}

/// Current89423 setActiveProfile -> setDeviceSerialNumberMetadata. Preserve
/// metadata belonging to other serials and the existing device-local name.
pub fn set_active_profile_for_serial(
    document: &mut Value,
    serial: &str,
    device_name: &str,
) -> anyhow::Result<()> {
    ensure!(
        !serial.is_empty() && !device_name.is_empty(),
        "756 serial metadata identity is absent"
    );
    let active = document["activeProfile"]
        .as_str()
        .context("756 serial metadata active profile is absent")?
        .to_owned();
    let root = document
        .as_object_mut()
        .context("756 serial metadata document is invalid")?;
    let metadatas = root
        .entry("deviceMetadatas")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("756 deviceMetadatas is invalid")?;
    let metadata = metadatas
        .entry(serial)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .context("756 serial metadata is invalid")?;
    metadata.entry("name").or_insert_with(|| json!(device_name));
    metadata.insert("activeProfileGuid".into(), json!(active));
    Ok(())
}

pub fn navigate_ble(
    document: &Value,
    slots: &[Slot],
    navigation: &str,
    guid: &str,
) -> anyhow::Result<String> {
    if navigation == "switchProfile" {
        return Ok(guid.to_owned());
    }
    let available = slots
        .iter()
        .filter(|slot| {
            document["profiles"]
                .as_array()
                .is_some_and(|profiles| profiles.iter().any(|profile| profile["guid"] == slot.guid))
        })
        .collect::<Vec<_>>();
    let index = available.iter().position(|slot| document["activeProfile"] == slot.guid).context("The active BLE profile has no known onboard slot. Reconnect through USB to refresh onboard profiles.")?;
    let next = match navigation {
        "NextProfile" => (index + 1).min(available.len() - 1),
        "PreviousProfile" => index.saturating_sub(1),
        "CycleUp" => (index + 1) % available.len(),
        "CycleDown" => (index + available.len() - 1) % available.len(),
        other => anyhow::bail!("Unsupported BLE profile navigation: {other}"),
    };
    Ok(available[next].guid.clone())
}

/// Current34938 sy analogV3 order: actual OBM GUIDs first, then local-only
/// names sorted by upper-case UTF-16. A device CycleUp skips factory index0.
pub fn navigate_local(
    document: &Value,
    slots: &[Slot],
    target: &BleSwitch,
    device_event: bool,
) -> anyhow::Result<String> {
    let profiles = document["profiles"]
        .as_array()
        .context("756 profile navigation list is absent")?;
    let mut ordered = slots
        .iter()
        .filter_map(|slot| profiles.iter().find(|profile| profile["guid"] == slot.guid))
        .collect::<Vec<_>>();
    let mut local = profiles
        .iter()
        .filter(|profile| !slots.iter().any(|slot| profile["guid"] == slot.guid))
        .collect::<Vec<_>>();
    local.sort_by(|left, right| {
        let name = |profile: &Value| {
            profile["name"]
                .as_str()
                .unwrap_or_default()
                .to_uppercase()
                .encode_utf16()
                .collect::<Vec<_>>()
        };
        name(left).cmp(&name(right))
    });
    ordered.extend(local);
    ensure!(!ordered.is_empty(), "756 profile navigation list is empty");
    if let BleSwitch::Guid(guid) = target {
        ensure!(
            ordered.iter().any(|profile| profile["guid"] == *guid),
            "756 target profile is absent"
        );
        return Ok(guid.clone());
    }
    if let BleSwitch::Slot(id) = target {
        return slots
            .iter()
            .find(|slot| slot.slot_id == *id)
            .map(|slot| slot.guid.clone())
            .context("756 target onboard slot is absent");
    }
    let BleSwitch::Navigation(name) = target else {
        unreachable!()
    };
    let index = ordered
        .iter()
        .position(|profile| profile["guid"] == document["activeProfile"])
        .context("756 active navigation profile is absent")?;
    let next = match name.as_str() {
        "NextProfile" => (index + 1).min(ordered.len() - 1),
        "PreviousProfile" => index.saturating_sub(1),
        "CycleUp" => {
            let mut next = (index + 1) % ordered.len();
            if device_event && next == 0 {
                next = 1;
            }
            next
        }
        "CycleDown" => (index + ordered.len() - 1) % ordered.len(),
        other => anyhow::bail!("Unsupported 756 profile navigation: {other}"),
    };
    ordered
        .get(next)
        .and_then(|profile| profile["guid"].as_str())
        .map(str::to_owned)
        .context("756 source navigation target is absent")
}

/// 756 rzDevice30::{get,set}ActiveProfileID. GATT response status is checked by
/// the transport; an empty getter response is never a successful slot read.
pub fn ble_profile_frame(transaction: u8, slot: Option<u8>) -> anyhow::Result<Vec<u8>> {
    ensure!(
        transaction < 31 && slot != Some(0),
        "Invalid BLE active profile command"
    );
    let mut frame = vec![
        transaction,
        u8::from(slot.is_some()),
        0,
        0,
        3,
        if slot.is_some() { 2 } else { 130 },
        0,
        0,
    ];
    if let Some(slot) = slot {
        frame.push(slot);
    }
    Ok(frame)
}
pub fn ble_profile_response(data: &[u8]) -> anyhow::Result<u8> {
    data.first()
        .copied()
        .filter(|slot| *slot > 0)
        .context("Could not read the active BLE profile.")
}

#[cfg(test)]
#[path = "keyboard_latest_restore_tests.rs"]
mod tests;
