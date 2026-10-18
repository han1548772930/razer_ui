//! AMD K17 measurements from the current SDK's 0x1841d0 and 0x190d70.
use crate::hardware_driver::{Driver, Transport};
use anyhow::{Context as _, ensure};
use super::acpi_timer::{AcpiTimer, FREQUENCY};

/// SDK 0x187310 initializes zero calibration except these brand/model branches.
pub fn temperature_offset(family: u32, model: u32, brand: &str) -> f32 {
    if family != 0x17 { return 0.0; }
    if matches!(model, 0 | 1) {
        if brand.contains("EPYC") { return 0.0; }
        if brand.contains("Threadripper") {
            for part in ["1950", "1920", "1900"] {
                if brand.contains(&format!("{part}X")) { return 27.0; }
                if brand.contains(part) { return 10.0; }
            }
        } else if ["1800X", "1700X", "1600X"].iter().any(|part| brand.contains(part)) {
            return 20.0;
        }
    } else if model == 8 {
        if brand.contains("Threadripper") {
            if brand.contains("2990WX") || brand.contains("2990X") { return 27.0; }
        } else if brand.contains("2700X") { return 10.0; }
    }
    0.0
}

/// Source creates this calibration with the first valid SMN response and keeps
/// it with the sensor. Subsequent reads update raw temperature, not calibration.
pub struct TemperatureSensor { offset: f32 }
impl TemperatureSensor {
    pub fn new(first: u32, brand_offset: f32, single_node: bool) -> anyhow::Result<Self> {
        ensure!(first != 0 && brand_offset.is_finite(), "AMD thermal calibration was not observed");
        let extended_range = first & 0x80000 != 0 || (single_node && first & 0x30000 == 0x30000);
        Ok(Self { offset: -brand_offset - if extended_range { 49.0 } else { 0.0 } })
    }
    pub fn decode(&self, value: u32) -> anyhow::Result<f64> {
        let raw = value >> 21;
        ensure!(raw != 0, "AMD package temperature was not observed");
        Ok(f64::from(raw as f32 * 0.125 + self.offset))
    }
}

#[derive(Clone, Copy)]
pub struct ClockCounters {
    pub aperf: u64,
    pub mperf: u64,
    pub acpi: u32,
    pub qpc: i64,
}

/// The complete assembly uses 64-bit counters and integer timer products before
/// single-precision division (0x19106f..0x1910bf). Invalid windows stay failures.
pub fn clock_mhz(
    before: ClockCounters,
    after: ClockCounters,
    tsc_mhz: f32,
    qpc_frequency: u64,
    dynamic_clock: Option<(f32, f32)>,
) -> anyhow::Result<f64> {
    ensure!(tsc_mhz.is_finite() && tsc_mhz > 0.0 && qpc_frequency > 0, "AMD reference frequency was not observed");
    if let Some((before, after)) = dynamic_clock {
        ensure!(before.is_finite() && after.is_finite() && before >= 0.0 && after >= 0.0
            && (after - before).abs() < 0.05, "AMD dynamic clock changed during measurement");
    }
    let aperf = after.aperf.wrapping_sub(before.aperf) as i64;
    let mperf = after.mperf.wrapping_sub(before.mperf) as i64;
    // The source compensates wrapping only inside its wait loop, not the
    // measurement formula. Reject that window instead of publishing a negative clock.
    let acpi = i64::from(after.acpi) - i64::from(before.acpi);
    let qpc = after.qpc.checked_sub(before.qpc).context("AMD QPC interval overflow")?;
    ensure!(aperf > 0 && mperf > 0 && acpi > 0 && qpc > 0, "AMD counters did not advance within a valid measurement window");
    let numerator = qpc.checked_mul(FREQUENCY as i64).context("AMD timer numerator overflow")?;
    let denominator = u64::try_from(acpi)?.checked_mul(qpc_frequency)
        .filter(|value| *value <= i64::MAX as u64).context("AMD timer denominator overflow")?;
    let value = (numerator as f32 / denominator as f32) * ((aperf as f32 * tsc_mhz) / mperf as f32);
    ensure!(value.is_finite() && value > 0.0, "AMD measured frequency is invalid");
    Ok(f64::from(value))
}

/// Caller retains processor affinity and the driver mutex for the entire sample.
/// QPC and optional dynamic estimator reads occur at the source-defined points.
pub fn sample_clock<T: Transport>(
    driver: &mut Driver<T>,
    timer: AcpiTimer,
    qpc_frequency: u64,
    mut qpc: impl FnMut() -> anyhow::Result<i64>,
    mut dynamic_clock: Option<&mut dyn FnMut() -> anyhow::Result<f32>>,
) -> anyhow::Result<f64> {
    let tsc = driver.measured_tsc_mhz()?;
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
    clock_mhz(before, after, tsc, qpc_frequency, dynamic_before.zip(dynamic_after))
}

#[cfg(test)]
#[path = "amd_cpu_tests.rs"]
mod tests;
