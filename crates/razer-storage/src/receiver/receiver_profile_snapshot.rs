//! Validated receiver document projection at the local workspace boundary.
use super::*;
use razer_model::model::{Device, Profile};
use std::collections::BTreeSet;

const DOCUMENT_FIELD: &str = "_receiver_source_document";

/// The fields owned by receiver profile operations. Unrelated observations
/// and device settings do not create spurious persistence conflicts.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverProfileRevision {
    profiles: Vec<Profile>,
    active_profile: String,
    document: Option<DeviceDocument>,
}

impl ReceiverProfileRevision {
    pub fn capture(device: &Device) -> anyhow::Result<Self> {
        let source = defaults(device.product_id, device.edition_id)?;
        let document = device
            .source_device_settings
            .as_ref()
            .map(|settings| {
                settings
                    .as_object()
                    .context("Receiver device settings are not an object")
            })
            .transpose()?
            .and_then(|settings| settings.get(DOCUMENT_FIELD))
            .map(DeviceDocument::parse)
            .transpose()?;
        if let Some(document) = &document {
            document.require_scope(device.product_id, source.schema_version)?;
        }
        Ok(Self {
            profiles: device.profiles.clone(),
            active_profile: device.active_profile.clone(),
            document,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiverBrightness {
    pub enabled: bool,
    pub value: u8,
}

impl ReceiverBrightness {
    pub fn percent(self) -> u8 {
        if self.enabled { self.value } else { 0 }
    }
}

/// A real serial's complete source document. Page drafts and transport
/// acknowledgements are deliberately absent from this persistence object.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverProfileSnapshot {
    product_id: u32,
    edition_id: u32,
    serial_number: String,
    document: DeviceDocument,
}

impl ReceiverProfileSnapshot {
    pub fn has_document(device: &Device) -> anyhow::Result<bool> {
        device
            .source_device_settings
            .as_ref()
            .map(|settings| {
                settings
                    .as_object()
                    .map(|settings| settings.contains_key(DOCUMENT_FIELD))
                    .context("Receiver device settings are not an object")
            })
            .transpose()
            .map(|present| present.unwrap_or(false))
    }

    pub fn parse(
        product_id: u32,
        edition_id: u32,
        serial_number: &str,
        document: &Value,
    ) -> anyhow::Result<Self> {
        let source = defaults(product_id, edition_id)?;
        ensure!(
            !serial_number.is_empty(),
            "Receiver source serial is absent"
        );
        let document = DeviceDocument::parse(document)?;
        document.require_scope(product_id, source.schema_version)?;
        let profiles = document.profiles.require("profiles")?;
        let active = document.active_profile.require("activeProfile")?;
        let mut identities = BTreeSet::new();
        for profile in profiles {
            let guid = profile.guid.require("profile.guid")?;
            ensure!(valid_guid(guid), "Receiver source profile GUID is invalid");
            ensure!(
                identities.insert(guid.clone()),
                "Receiver source profile GUID is duplicated"
            );
            profile.name.require("profile.name")?;
        }
        ensure!(
            identities.contains(active),
            "Receiver active profile is absent"
        );
        let metadata = document
            .device_metadatas
            .require("deviceMetadatas")?
            .get(serial_number)
            .context("Receiver source has no queried serial metadata")?;
        ensure!(
            metadata.active_profile_guid.require("activeProfileGuid")? == active,
            "Receiver active profile differs from the queried serial"
        );
        Ok(Self {
            product_id,
            edition_id,
            serial_number: serial_number.into(),
            document,
        })
    }

    pub fn in_device(device: &Device, serial_number: &str) -> anyhow::Result<Option<Self>> {
        let Some(settings) = &device.source_device_settings else {
            return Ok(None);
        };
        let settings = settings
            .as_object()
            .context("Receiver device settings are not an object")?;
        settings
            .get(DOCUMENT_FIELD)
            .map(|document| {
                Self::parse(
                    device.product_id,
                    device.edition_id,
                    serial_number,
                    document,
                )
            })
            .transpose()
    }

    pub fn serial_number(&self) -> &str {
        &self.serial_number
    }

    pub fn active_profile(&self) -> &str {
        // Construction validates the complete profile/serial relationship.
        self.document
            .active_profile
            .get()
            .expect("validated receiver active profile")
    }

    pub fn document(&self) -> anyhow::Result<Value> {
        Ok(serde_json::to_value(&self.document)?)
    }

    fn active_profile_data(&self) -> anyhow::Result<&ProfileData> {
        self.document
            .profiles
            .require("profiles")?
            .iter()
            .find(|profile| profile.guid.get().map(String::as_str) == Some(self.active_profile()))
            .context("Receiver source active profile disappeared")
    }

    /// Current taskMakerSwitchProfile changes the selected GUID and metadata
    /// for the queried serial, retaining other serials and profile contents.
    pub fn select_profile(&self, guid: &str) -> anyhow::Result<Self> {
        ensure!(
            valid_guid(guid),
            "Receiver selected profile GUID is invalid"
        );
        ensure!(
            self.document
                .profiles
                .require("profiles")?
                .iter()
                .any(|profile| { profile.guid.get().map(String::as_str) == Some(guid) }),
            "Receiver selected profile is absent from its source document"
        );
        let mut selected = self.clone();
        selected.document.version = Field::set(match &self.document.version {
            Field::Missing => 1,
            Field::Present(Some(version)) => next_version(*version)?,
            Field::Present(None) => anyhow::bail!("Receiver source document version is null"),
        });
        selected.document.active_profile = Field::set(guid.to_owned());
        selected
            .document
            .metadata_mut(&self.serial_number)?
            .active_profile_guid = Field::set(guid.to_owned());
        Ok(selected)
    }

    pub fn active_quick_effect(&self) -> anyhow::Result<Value> {
        let effects = self
            .active_profile_data()?
            .quick_effects
            .require("quickEffects")?;
        effects
            .selected_effect_id
            .require("quickEffects.selectedEffectId")?;
        Ok(serde_json::to_value(effects)?)
    }

    pub fn active_mappings(&self) -> anyhow::Result<Vec<Value>> {
        Ok(self
            .active_profile_data()?
            .mappings
            .require("mappings")?
            .clone())
    }

    pub fn selected_docking_mouse(&self) -> anyhow::Result<Option<Value>> {
        Ok(self
            .active_profile_data()?
            .extensions
            .get("selectedDockingMouse")
            .cloned())
    }

    pub fn brightness(&self) -> anyhow::Result<ReceiverBrightness> {
        let profile = self.active_profile_data()?;
        let brightness = profile.brightness.require("brightness")?;
        Ok(ReceiverBrightness {
            enabled: *brightness.is_enabled.require("brightness.isEnabled")?,
            value: *brightness.value.require("brightness.value")?,
        })
    }

    /// Fk/lY publishes the source profile list and active GUID. Retain local
    /// fields belonging to matching GUIDs, and keep source extensions lossless.
    pub fn project(&self, device: &Device) -> anyhow::Result<Device> {
        ensure!(
            device.product_id == self.product_id
                && device.real_product_id == self.product_id
                && device.edition_id == self.edition_id,
            "Receiver source snapshot belongs to a different product"
        );
        let mut candidate = device.clone();
        let settings = candidate
            .source_device_settings
            .get_or_insert_with(|| Value::Object(Default::default()))
            .as_object_mut()
            .context("Receiver device settings are not an object")?;
        settings.insert(DOCUMENT_FIELD.into(), self.document()?);
        candidate.profiles = self
            .document
            .profiles
            .require("profiles")?
            .iter()
            .map(|source| {
                let guid = source.guid.require("profile.guid")?;
                let mut profile = device
                    .profiles
                    .iter()
                    .find(|profile| &profile.guid == guid)
                    .cloned()
                    .unwrap_or_else(|| Profile {
                        id: guid.clone(),
                        guid: guid.clone(),
                        name: String::new(),
                        source_settings: None,
                        settings: None,
                        dpi_stages: None,
                    });
                profile.id = guid.clone();
                profile.guid = guid.clone();
                profile.name = source.name.require("profile.name")?.clone();
                profile.source_settings = Some(serde_json::to_value(source)?);
                Ok(profile)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        candidate.active_profile = self.active_profile().into();
        candidate.serial_number = self.serial_number.clone();
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SERIAL: &str = "receiver-observed-serial";
    const FIRST: &str = "11111111-2222-3333-4444-555555555555";
    const NEXT: &str = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

    fn device(product: u32) -> Device {
        let mut device = razer_model::product_preview::registered_preview(product).unwrap();
        device.source_device_settings = Some(json!({"otherOwner":{"value":9}}));
        device
    }

    #[test]
    fn receiver_snapshot_projects_complete_serial_document_and_preserves_other_owners() {
        for product in [164, 241] {
            let original = device(product);
            let mut document = initial_document(product, 0, SERIAL, "PC", FIRST).unwrap();
            document["profiles"][0]["unknownSource"] = json!({"flag":null});
            let plan = ResetPlan::prepare(
                product,
                0,
                &document,
                SERIAL,
                "observed-container",
                "PC",
                NEXT,
            )
            .unwrap();
            let snapshot =
                ReceiverProfileSnapshot::parse(product, 0, SERIAL, plan.document()).unwrap();
            let candidate = snapshot.project(&original).unwrap();
            assert_eq!(candidate.profiles.len(), 2);
            assert_eq!(candidate.active_profile, NEXT);
            assert_eq!(candidate.profiles[1].name, "PC-Default 1");
            assert_eq!(
                candidate.profiles[0].source_settings.as_ref().unwrap()["unknownSource"],
                json!({"flag":null})
            );
            assert_eq!(candidate.serial_number, SERIAL);
            assert_eq!(
                candidate.source_device_settings.as_ref().unwrap()["otherOwner"],
                json!({"value":9})
            );
            assert_eq!(
                ReceiverProfileSnapshot::in_device(&candidate, SERIAL).unwrap(),
                Some(snapshot.clone())
            );
            assert_eq!(
                snapshot.brightness().unwrap(),
                ReceiverBrightness {
                    enabled: true,
                    value: 100
                }
            );
            assert_eq!(
                candidate.dashboard.connection_observation,
                original.dashboard.connection_observation
            );
            assert_eq!(
                serde_json::to_value(&candidate.firmware_info).unwrap(),
                serde_json::to_value(&original.firmware_info).unwrap()
            );
            assert_ne!(original.active_profile, candidate.active_profile);
        }
    }

    #[test]
    fn receiver_snapshot_rejects_partial_wrong_serial_and_duplicate_profile_documents() {
        let valid = initial_document(164, 0, SERIAL, "PC", FIRST).unwrap();
        for case in 0..8 {
            let mut document = valid.clone();
            match case {
                0 => {
                    document["profiles"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("guid");
                }
                1 => document["profiles"][0]["guid"] = json!("invalid-guid"),
                2 => {
                    document["profiles"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("name");
                }
                3 => document["activeProfile"] = json!(NEXT),
                4 => document["deviceMetadatas"][SERIAL]["activeProfileGuid"] = json!(NEXT),
                5 => {
                    let duplicate = document["profiles"][0].clone();
                    document["profiles"].as_array_mut().unwrap().push(duplicate);
                }
                6 => document["schemaVersion"] = json!(12),
                7 => document["profiles"][0]["brightness"]["value"] = json!(101),
                _ => unreachable!(),
            }
            assert!(
                ReceiverProfileSnapshot::parse(164, 0, SERIAL, &document).is_err(),
                "case {case}"
            );
        }
        assert!(ReceiverProfileSnapshot::parse(164, 0, "other-serial", &valid).is_err());
        assert!(ReceiverProfileSnapshot::parse(241, 0, SERIAL, &valid).is_err());
        assert!(ReceiverProfileSnapshot::parse(164, 1, SERIAL, &valid).is_err());
        let snapshot = ReceiverProfileSnapshot::parse(164, 0, SERIAL, &valid).unwrap();
        assert!(snapshot.project(&device(241)).is_err());
    }

    #[test]
    fn receiver_revision_rejects_competing_profile_edits_but_accepts_unrelated_observations() {
        let document = initial_document(164, 0, SERIAL, "PC", FIRST).unwrap();
        let snapshot = ReceiverProfileSnapshot::parse(164, 0, SERIAL, &document).unwrap();
        let original = snapshot.project(&device(164)).unwrap();
        let revision = ReceiverProfileRevision::capture(&original).unwrap();
        let mut current = original.clone();
        current.dashboard.connection_observation =
            Some(razer_model::model::DeviceConnectionObservation::HidPresent);
        current.source_device_settings.as_mut().unwrap()["otherOwner"] = json!({"value":10});
        assert_eq!(
            ReceiverProfileRevision::capture(&current).unwrap(),
            revision
        );
        for case in 0..3 {
            let mut current = original.clone();
            match case {
                0 => current.profiles[0].name.push_str(" edited"),
                1 => current.active_profile = NEXT.into(),
                2 => {
                    current.source_device_settings.as_mut().unwrap()[DOCUMENT_FIELD]["version"] =
                        json!(8)
                }
                _ => unreachable!(),
            }
            assert_ne!(
                ReceiverProfileRevision::capture(&current).unwrap(),
                revision,
                "case {case}"
            );
        }
    }

    #[test]
    fn receiver_selection_changes_only_active_guid_serial_metadata_and_version() {
        for product in [164, 241] {
            let document = initial_document(product, 0, SERIAL, "PC", FIRST).unwrap();
            let plan =
                ResetPlan::prepare(product, 0, &document, SERIAL, "container", "PC", NEXT).unwrap();
            let mut document = plan.document().clone();
            document["version"] = json!(MAX_SAFE_INTEGER);
            document["extension"] = json!({"keep":null});
            document["profiles"][0]["extension"] = json!("first-profile");
            document["deviceMetadatas"]["other-serial"] =
                json!({"activeProfileGuid":NEXT,"extension":7});
            let snapshot = ReceiverProfileSnapshot::parse(product, 0, SERIAL, &document).unwrap();
            let selected = snapshot.select_profile(FIRST).unwrap();
            let mut expected = document.clone();
            expected["version"] = json!(0);
            expected["activeProfile"] = json!(FIRST);
            expected["deviceMetadatas"][SERIAL]["activeProfileGuid"] = json!(FIRST);
            assert_eq!(selected.document().unwrap(), expected);
            assert_eq!(snapshot.document().unwrap(), document);
            assert_eq!(selected.active_profile(), FIRST);
            assert!(snapshot.select_profile("preview-profile-164").is_err());
            assert!(
                snapshot
                    .select_profile("99999999-2222-3333-4444-555555555555")
                    .is_err()
            );
        }
    }

    #[test]
    fn receiver_selection_preserves_missing_version_and_rejects_explicit_null() {
        let mut document = initial_document(164, 0, SERIAL, "PC", FIRST).unwrap();
        document.as_object_mut().unwrap().remove("version");
        let snapshot = ReceiverProfileSnapshot::parse(164, 0, SERIAL, &document).unwrap();
        assert_eq!(
            snapshot.select_profile(FIRST).unwrap().document().unwrap()["version"],
            1
        );
        document["version"] = Value::Null;
        let snapshot = ReceiverProfileSnapshot::parse(164, 0, SERIAL, &document).unwrap();
        assert!(snapshot.select_profile(FIRST).is_err());
    }

    #[test]
    fn receiver_refresh_accessors_use_the_selected_source_and_preserve_absence() {
        let mut document = initial_document(241, 0, SERIAL, "PC", FIRST).unwrap();
        document["profiles"][0]["quickEffects"] = json!({"selectedEffectId":1,"selectedEffectSetting":{"color1":"#112233"},"extension":true});
        document["profiles"][0]["mappings"] = json!([{"input":7,"extension":null}]);
        let snapshot = ReceiverProfileSnapshot::parse(241, 0, SERIAL, &document).unwrap();
        assert_eq!(
            snapshot.active_quick_effect().unwrap(),
            document["profiles"][0]["quickEffects"]
        );
        assert_eq!(
            snapshot.active_mappings().unwrap(),
            vec![json!({"input":7,"extension":null})]
        );
        assert_eq!(snapshot.selected_docking_mouse().unwrap(), None);
        document["profiles"][0]["selectedDockingMouse"] = Value::Null;
        let snapshot = ReceiverProfileSnapshot::parse(241, 0, SERIAL, &document).unwrap();
        assert_eq!(
            snapshot.selected_docking_mouse().unwrap(),
            Some(Value::Null)
        );
        document["profiles"][0]["quickEffects"]
            .as_object_mut()
            .unwrap()
            .remove("selectedEffectId");
        let snapshot = ReceiverProfileSnapshot::parse(241, 0, SERIAL, &document).unwrap();
        assert!(snapshot.active_quick_effect().is_err());
        assert!(!ReceiverProfileSnapshot::has_document(&device(241)).unwrap());
    }
}
