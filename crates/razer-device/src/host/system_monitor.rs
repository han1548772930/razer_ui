//! Typed CPU/GPU observations from the local hardware monitor callback.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Deserializer, Serialize};

pub const HANBO_PRODUCT_ID: u32 = 3893;
pub const HARDWARE_MASK: i32 = 0;
pub const CALLBACK_INTERVAL_MS: i32 = 1000;

/// `None` is a native JSON null, rather than a measured zero. Clock values are
/// supplied in MHz; temperature values are supplied in degrees Celsius.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemSample {
    #[serde(deserialize_with = "nullable_number")]
    pub cpu_temperature: Option<f64>,
    #[serde(deserialize_with = "nullable_number")]
    pub cpu_clock_mhz: Option<f64>,
    #[serde(deserialize_with = "nullable_number")]
    pub gpu_temperature: Option<f64>,
    #[serde(deserialize_with = "nullable_number")]
    pub gpu_clock_mhz: Option<f64>,
}

impl SystemSample {
    pub fn validate(&self) -> anyhow::Result<()> {
        for value in [
            self.cpu_temperature,
            self.cpu_clock_mhz,
            self.gpu_temperature,
            self.gpu_clock_mhz,
        ] {
            ensure!(
                value.is_none_or(f64::is_finite),
                "Non-finite hardware observation"
            );
        }
        Ok(())
    }
}

/// Every callback remains in arrival order. A poll consumes a bounded batch;
/// additional pending callbacks remain with the same monitor owner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorObservation {
    pub owner_guid: String,
    pub samples: Vec<SystemSample>,
    pub diagnostics: Vec<String>,
}

impl MonitorObservation {
    pub fn validate_for(&self, owner_guid: &str) -> anyhow::Result<()> {
        validate_identity(owner_guid)?;
        ensure!(
            self.owner_guid == owner_guid,
            "System monitor observation owner changed"
        );
        for sample in &self.samples {
            sample.validate()?;
        }
        Ok(())
    }
}

pub fn validate_identity(identity: &str) -> anyhow::Result<()> {
    ensure!(
        !identity.trim().is_empty() && !identity.as_bytes().contains(&0),
        "System monitor identity is empty or contains NUL"
    );
    Ok(())
}

#[derive(Deserialize)]
struct HardwareCallback {
    event: String,
    #[serde(rename = "dllName")]
    dll_name: String,
    #[serde(rename = "dllFunc")]
    dll_func: String,
    #[serde(rename = "dataType")]
    data_type: i32,
    status: String,
    #[serde(rename = "CPUTemperature", deserialize_with = "nullable_number")]
    cpu_temperature: Option<f64>,
    #[serde(rename = "CPUClock", deserialize_with = "nullable_number")]
    cpu_clock_mhz: Option<f64>,
    #[serde(rename = "GPUTemperature", deserialize_with = "nullable_number")]
    gpu_temperature: Option<f64>,
    #[serde(rename = "GPUClock", deserialize_with = "nullable_number")]
    gpu_clock_mhz: Option<f64>,
}

/// Only the four fields consumed by the source Hanbo monitor enter its domain
/// state. The DLL's other mask-zero diagnostics do not become UI observations.
pub fn parse_callback(bytes: &[u8], container_id: &str) -> anyhow::Result<SystemSample> {
    validate_identity(container_id)?;
    let callback: HardwareCallback =
        serde_json::from_slice(bytes).context("Invalid system monitor callback")?;
    ensure!(
        callback.event == container_id,
        "System monitor callback container changed"
    );
    ensure!(
        callback.dll_name == "RzSystemCommon" && callback.dll_func == "HardwareMonitorCallback",
        "Unexpected system monitor callback route"
    );
    ensure!(
        callback.data_type == HARDWARE_MASK,
        "Unexpected system monitor callback mask"
    );
    ensure!(
        callback.status == "SUCCESS",
        "System monitor callback failed: {}",
        callback.status
    );
    Ok(SystemSample {
        cpu_temperature: callback.cpu_temperature,
        cpu_clock_mhz: callback.cpu_clock_mhz,
        gpu_temperature: callback.gpu_temperature,
        gpu_clock_mhz: callback.gpu_clock_mhz,
    })
}

fn nullable_number<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;
    if value.is_some_and(|number| !number.is_finite()) {
        return Err(serde::de::Error::custom("Non-finite hardware observation"));
    }
    Ok(value)
}

#[cfg(test)]
#[path = "system_monitor_tests.rs"]
mod tests;
