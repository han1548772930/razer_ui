//! Local Blade system details and the native full-report file boundary.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

pub const MAXIMUM_REPORT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    Display,
    Full,
}
impl Query {
    pub fn export(self) -> &'static str {
        match self {
            Self::Display => "GetBladeDisplayData",
            Self::Full => "GetAllBladeInfo",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    Basic,
    Image,
}
pub fn layout(product_id: u32) -> Option<Layout> {
    match product_id {
        694 | 695 | 696 => Some(Layout::Basic),
        709 | 710 | 711 | 736 | 737 => Some(Layout::Image),
        _ => None,
    }
}

/// Native nlohmann JSON retains null when an enumeration has no entries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NativeValue<T> {
    Value(T),
    Null(()),
}
impl<T> NativeValue<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Null(()) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Monitor {
    #[serde(rename = "Name")]
    pub name: String,
    // The external native producer spells this field Manufactuer.
    #[serde(rename = "Manufactuer")]
    pub manufacturer: String,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemDetails {
    #[serde(
        rename = "DeviceName",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub device_name: Option<String>,
    #[serde(
        rename = "ModelName",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub model_name: Option<String>,
    #[serde(
        rename = "CPU",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub cpu: Option<String>,
    #[serde(
        rename = "CPUManufacturer",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub cpu_manufacturer: Option<String>,
    #[serde(
        rename = "GPU",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub gpu: Option<String>,
    #[serde(
        rename = "GPUManufacturer",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub gpu_manufacturer: Option<String>,
    #[serde(
        rename = "RAM",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub ram: Option<String>,
    #[serde(
        rename = "RAMManufacturer",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub ram_manufacturer: Option<String>,
    #[serde(
        rename = "OSVersion",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub os_version: Option<String>,
    #[serde(
        rename = "SerialNumber",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub serial_number: Option<String>,
    #[serde(rename = "Display")]
    pub display: NativeValue<Vec<Monitor>>,
    #[serde(
        rename = "RazerAppEngine",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub razer_app_engine: Option<String>,
    #[serde(
        rename = "RazerSynapse3",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub razer_synapse3: Option<String>,
    #[serde(
        rename = "NVIDIAGeForceExperience",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub nvidia_geforce_experience: Option<String>,
    #[serde(
        rename = "NVIDIANvApp",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub nvidia_app: Option<String>,
    #[serde(
        rename = "NVIDIAControlPanel",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub nvidia_control_panel: Option<String>,
    #[serde(
        rename = "NVIDIAControlPanelPlatform",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub nvidia_control_panel_platform: Option<String>,
    #[serde(
        rename = "IntelGraphicCommandCenter",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub intel_graphic_command_center: Option<String>,
    #[serde(
        rename = "IntelGraphicCommandCenterPlatform",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub intel_graphic_command_center_platform: Option<String>,
    #[serde(
        rename = "AMDAdrenalinEdition",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub amd_adrenalin_edition: Option<String>,
    #[serde(
        rename = "RazerCortex",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub razer_cortex: Option<String>,
    #[serde(
        rename = "RazerAxon",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub razer_axon: Option<String>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}
impl SystemDetails {
    pub fn monitors(&self) -> &[Monitor] {
        self.display.value().map(Vec::as_slice).unwrap_or(&[])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirmwareDetails {
    #[serde(
        rename = "BiosFW",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub bios: Option<String>,
    #[serde(
        rename = "ECFW",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub ec: Option<String>,
    #[serde(
        rename = "CameraFW",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub camera: Option<String>,
    #[serde(
        rename = "VBiosFW",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub vbios: Option<String>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FullReport {
    #[serde(
        rename = "Display",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub display: Option<SystemDetails>,
    #[serde(
        rename = "Firmware",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub firmware: Option<NativeValue<FirmwareDetails>>,
    #[serde(
        rename = "Driver",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub drivers: Option<NativeValue<BTreeMap<String, String>>>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}
impl FullReport {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.display.is_some() || self.firmware.is_some() || self.drivers.is_some(),
            "Native full system report contains none of its report sections"
        );
        Ok(())
    }
    pub fn file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        ensure!(
            bytes.len() <= MAXIMUM_REPORT_BYTES,
            "System report exceeds its file limit"
        );
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "query", content = "details", rename_all = "snake_case")]
pub enum Data {
    Display(SystemDetails),
    Full(FullReport),
}
impl Data {
    pub fn decode(query: Query, bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAXIMUM_REPORT_BYTES,
            "Invalid native report length"
        );
        match query {
            Query::Display => serde_json::from_slice(bytes).map(Self::Display),
            Query::Full => serde_json::from_slice(bytes).map(Self::Full),
        }
        .context("Native system report does not match the current source schema")
        .and_then(|data| {
            if let Self::Full(report) = &data {
                report.validate()?;
            }
            Ok(data)
        })
    }
    pub fn query(&self) -> Query {
        match self {
            Self::Display(_) => Query::Display,
            Self::Full(_) => Query::Full,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub product_id: u32,
    pub data: Data,
}
impl Response {
    pub fn validate(&self, product_id: u32, query: Query) -> anyhow::Result<()> {
        ensure!(
            layout(product_id).is_some(),
            "Product has no current Blade system-info source"
        );
        ensure!(
            self.product_id == product_id && self.data.query() == query,
            "System-info response identity differs"
        );
        if let Data::Full(report) = &self.data {
            report.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "system_info_tests.rs"]
mod tests;
