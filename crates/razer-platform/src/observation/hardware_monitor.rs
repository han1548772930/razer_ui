//! Actual local sensor readings. Driver/API failures never become numeric defaults.
use anyhow::ensure;

#[path = "hardware_monitor/amd_cpu.rs"]
pub(crate) mod amd_cpu;
#[path = "hardware_monitor/acpi_timer.rs"]
pub(crate) mod acpi_timer;

#[path = "hardware_monitor/intel_cpu.rs"]
pub(crate) mod intel_cpu;
#[path = "hardware_monitor/nvidia.rs"]
pub(crate) mod nvidia;

#[derive(Debug)]
pub struct Readings {
    pub temperature_celsius: anyhow::Result<f64>,
    pub clock_mhz: anyhow::Result<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PciAddress {
    pub domain: Option<u32>,
    pub bus: u32,
    pub device: u32,
    pub function: Option<u32>,
}

#[derive(Debug)]
pub struct GraphicsReading {
    pub address: PciAddress,
    pub vendor_id: u32,
    pub device_id: u32,
    pub name: String,
    pub enabled: Option<bool>,
    pub readings: Readings,
}

pub(crate) fn measured(value: f64, label: &str) -> anyhow::Result<f64> {
    ensure!(value.is_finite(), "{label} returned a non-finite measurement");
    Ok(value)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub use crate::platform::windows::hardware_monitor::Monitor;

/// SystemCommon 0x6bc20 keeps the first enabled NVIDIA, otherwise the last
/// enabled AMD, otherwise the first enabled Intel; absent enabled candidates it
/// consumes the first source adapter. It never switches GPUs on read failure.
pub fn selected_graphics(graphics: &[GraphicsReading]) -> Option<usize> {
    if graphics.len() == 1 { return Some(0); }
    let mut selected = None;
    for (index, gpu) in graphics.iter().enumerate() {
        if gpu.enabled != Some(true) { continue; }
        if gpu.name.contains("NVIDIA") { return Some(index); }
        if gpu.name.contains("AMD") { selected = Some(index); }
        else if selected.is_none() && gpu.name.contains("Intel") { selected = Some(index); }
    }
    selected.or_else(|| (!graphics.is_empty()).then_some(0))
}
