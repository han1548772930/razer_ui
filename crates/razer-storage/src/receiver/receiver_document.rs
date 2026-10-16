//! Typed receiver document boundaries. Unknown extension fields remain lossless;
//! an absent field and an explicitly null field keep their distinct meanings.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Field<T> {
    Missing,
    Present(Option<T>),
}

impl<T> Default for Field<T> {
    fn default() -> Self {
        Self::Missing
    }
}

impl<T> Field<T> {
    pub fn set(value: T) -> Self {
        Self::Present(Some(value))
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }

    pub fn get(&self) -> Option<&T> {
        match self {
            Self::Present(value) => value.as_ref(),
            Self::Missing => None,
        }
    }

    pub fn require(&self, owner: &str) -> anyhow::Result<&T> {
        self.get()
            .with_context(|| format!("Missing or null receiver field: {owner}"))
    }

    pub fn require_mut(&mut self, owner: &str) -> anyhow::Result<&mut T> {
        match self {
            Self::Present(Some(value)) => Ok(value),
            _ => anyhow::bail!("Missing or null receiver field: {owner}"),
        }
    }

    pub fn insert_if_missing(&mut self, value: T) {
        if self.is_missing() {
            *self = Self::set(value);
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::Present)
    }
}

impl<T: Serialize> Serialize for Field<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Missing => serializer.serialize_none(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}

pub(crate) type Extensions = BTreeMap<String, Value>;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PairingCategory {
    Mouse,
    Keyboard,
}

impl PairingCategory {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "MOUSE" => Ok(Self::Mouse),
            "KEYBOARD" => Ok(Self::Keyboard),
            _ => anyhow::bail!("配对缓存缺少真实产品类别"),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingMasterData {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub product_id: Field<u16>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub dongle_id: Field<u16>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub ble_id: Field<u16>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub claim_interface: Field<u8>,
    #[serde(
        default,
        rename = "supports8KHzPollingRate",
        skip_serializing_if = "Field::is_missing"
    )]
    pub supports_eight_khz: Field<bool>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl From<&crate::receiver_defaults::PairingMaster> for PairingMasterData {
    fn from(value: &crate::receiver_defaults::PairingMaster) -> Self {
        Self {
            product_id: Field::set(value.product_id),
            dongle_id: Field::set(value.dongle_id),
            ble_id: Field::set(value.ble_id),
            claim_interface: Field::set(value.claim_interface),
            supports_eight_khz: match value.supports_eight_khz {
                Some(value) => Field::set(value),
                None => Field::Missing,
            },
            extensions: Extensions::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PairingRecord {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub vendor_id: Field<u16>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub product_id: Field<u32>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub dongle_id: Field<u16>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub device_container_id: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub category: Field<PairingCategory>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub master: Field<PairingMasterData>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrightnessData {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub value: Field<u8>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LightingData {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_display_on: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_idle_enabled: Field<bool>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub idle_minutes: Field<u16>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectsData {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub selected_effect_id: Field<u16>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileData {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub guid: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub name: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub brightness: Field<BrightnessData>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub switch_off_lighting: Field<LightingData>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub quick_effects: Field<EffectsData>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub mappings: Field<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub can_pair_two_devices: Field<bool>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceMetadata {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub active_profile_guid: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub name: Field<String>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlobalConfig {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_linked: Field<bool>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceDocument {
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub product_id: Field<u32>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub vendor_id: Field<u16>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub category: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub schema_version: Field<u32>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub version: Field<u64>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub profiles: Field<Vec<ProfileData>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub active_profile: Field<String>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub device_metadatas: Field<BTreeMap<String, DeviceMetadata>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub global_config: Field<GlobalConfig>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub is_default: Field<bool>,
    #[serde(
        default,
        rename = "reportIDs",
        skip_serializing_if = "Field::is_missing"
    )]
    pub report_ids: Field<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Field::is_missing")]
    pub can_pair_two_devices: Field<bool>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl DeviceDocument {
    pub fn parse(value: &Value) -> anyhow::Result<Self> {
        let document: Self = serde_json::from_value(value.clone())
            .context("Receiver document fields have invalid types")?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(version) = self.version.get() {
            ensure!(
                *version <= super::receiver_reset::MAX_SAFE_INTEGER,
                "Source document version exceeds 53 bits"
            );
        }
        if let Some(profiles) = self.profiles.get() {
            for profile in profiles {
                if let Some(value) = profile
                    .brightness
                    .get()
                    .and_then(|setting| setting.value.get())
                {
                    ensure!(*value <= 100, "Source brightness is not a percent");
                }
            }
        }
        Ok(())
    }

    pub fn require_scope(&self, product: u32, schema: u32) -> anyhow::Result<()> {
        ensure!(
            self.product_id.get() == Some(&product),
            "Receiver document product differs from its owner"
        );
        ensure!(
            self.schema_version.get() == Some(&schema),
            "Stored source document needs its original schema migration"
        );
        ensure!(
            !self
                .global_config
                .get()
                .is_some_and(|value| value.is_linked.get() == Some(&true)),
            "Linked sub-device metadata requires the actual runtime sub-device scope"
        );
        Ok(())
    }

    pub fn metadata_mut(&mut self, serial: &str) -> anyhow::Result<&mut DeviceMetadata> {
        self.device_metadatas.insert_if_missing(BTreeMap::new());
        Ok(self
            .device_metadatas
            .require_mut("deviceMetadatas")?
            .entry(serial.to_owned())
            .or_default())
    }
}
