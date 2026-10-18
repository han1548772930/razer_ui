//! AMD K17 measurements from the current SDK's 0x1841d0 and 0x190d70.
use super::acpi_timer::AcpiTimer;
use crate::hardware_driver::{Driver, Transport};
use anyhow::ensure;

/// SDK 0x187310 initializes zero calibration except these brand/model branches.
pub fn temperature_offset(family: u32, model: u32, brand: &str) -> f32 {
    if family != 0x17 {
        return 0.0;
    }
    if matches!(model, 0 | 1) {
        if brand.contains("EPYC") {
            return 0.0;
        }
        if brand.contains("Threadripper") {
            for part in ["1950", "1920", "1900"] {
                if brand.contains(&format!("{part}X")) {
                    return 27.0;
                }
                if brand.contains(part) {
                    return 10.0;
                }
            }
        } else if ["1800X", "1700X", "1600X"]
            .iter()
            .any(|part| brand.contains(part))
        {
            return 20.0;
        }
    } else if model == 8 {
        if brand.contains("Threadripper") {
            if brand.contains("2990WX") || brand.contains("2990X") {
                return 27.0;
            }
        } else if brand.contains("2700X") {
            return 10.0;
        }
    }
    0.0
}

/// Source creates this calibration with the first valid SMN response and keeps
/// it with the sensor. Subsequent reads update raw temperature, not calibration.
pub struct TemperatureSensor {
    offset: f32,
}
impl TemperatureSensor {
    pub fn new(first: u32, brand_offset: f32, single_node: bool) -> anyhow::Result<Self> {
        ensure!(
            first != 0 && brand_offset.is_finite(),
            "AMD thermal calibration was not observed"
        );
        let extended_range = first & 0x80000 != 0 || (single_node && first & 0x30000 == 0x30000);
        Ok(Self {
            offset: -brand_offset - if extended_range { 49.0 } else { 0.0 },
        })
    }
    pub fn decode(&self, value: u32) -> anyhow::Result<f64> {
        let raw = value >> 21;
        ensure!(raw != 0, "AMD package temperature was not observed");
        Ok(f64::from(raw as f32 * 0.125 + self.offset))
    }
}

pub use super::counter_clock::{ClockCounters, clock_mhz};

/// Caller retains processor affinity and the driver mutex for the entire sample.
/// QPC and optional dynamic estimator reads occur at the source-defined points.
pub fn sample_clock<T: Transport>(
    driver: &mut Driver<T>,
    timer: AcpiTimer,
    qpc_frequency: u64,
    mut qpc: impl FnMut() -> anyhow::Result<i64>,
    mut dynamic_clock: Option<&mut dyn FnMut() -> anyhow::Result<f32>>,
) -> anyhow::Result<f64> {
    let tsc = timer.measured_tsc(driver)?;
    let before = ClockCounters {
        aperf: driver.read_msr(0xc000_00e8)?,
        mperf: driver.read_msr(0xc000_00e7)?,
        acpi: timer.read(driver)?,
        qpc: qpc()?,
    };
    let dynamic_before = dynamic_clock.as_mut().map(|read| read()).transpose()?;
    timer.wait(driver, 2500)?;
    let after = ClockCounters {
        aperf: driver.read_msr(0xc000_00e8)?,
        mperf: driver.read_msr(0xc000_00e7)?,
        acpi: timer.read(driver)?,
        qpc: qpc()?,
    };
    let dynamic_after = dynamic_clock.as_mut().map(|read| read()).transpose()?;
    clock_mhz(
        before,
        after,
        tsc,
        qpc_frequency,
        dynamic_before.zip(dynamic_after),
    )
}

#[cfg(test)]
#[path = "amd_cpu_tests.rs"]
mod tests;
