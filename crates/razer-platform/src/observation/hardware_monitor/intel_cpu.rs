//! CPU sensor transformations traced through the current SDK's Intel factory.
use crate::hardware_driver::{Driver, Transport};
use anyhow::{Context as _, ensure};

/// These model branches use the target-temperature MSR in the current SDK.
/// Other Intel models have distinct calibration and cannot share this decoder.
pub fn has_temperature_target(family: u32, model: u32) -> bool {
    (family == 6 && matches!(model,
        0x1a | 0x1e | 0x1f | 0x25 | 0x2c | 0x2e | 0x2f |
        0x2a | 0x2d | 0x3a | 0x3e | 0x3c | 0x3d | 0x3f | 0x45 | 0x46 |
        0x47 | 0x4f | 0x56 | 0x37 | 0x4a | 0x4d | 0x4c | 0x5c | 0x5f |
        0x7a | 0x96 | 0x9c | 0x57 | 0x4e | 0x55 | 0x5e | 0x66 | 0x67 |
        0x6a | 0x7d | 0x7e | 0x8c | 0x8d | 0x8e | 0x9e | 0x8f | 0x97 |
        0x9a | 0xaa | 0xac | 0xad | 0xaf | 0xb5 | 0xb6 | 0xb7 | 0xba |
        0xbd | 0xbe | 0xbf | 0xc5 | 0xc6 | 0xcc | 0xcf | 0xd5 | 0xd7 |
        0xe5 | 0xa5 | 0xa6 | 0xa7 | 0xa8)) || family == 0x12
}

/// SDK 0x14b6e0/0x167fa0/0x156170/0x15f850/0x16b410/0x16d5d0.
/// Failed target reads cannot use the SDK's fallback 100 degrees: they are not
/// measurements. A fresh valid status is required on every observation.
pub fn temperature<T: Transport>(driver: &mut Driver<T>) -> anyhow::Result<f64> {
    let target = driver.read_msr(0x1a2)?;
    let target_celsius = ((target >> 16) & 0xff) as i32;
    ensure!(target_celsius > 0, "CPU thermal calibration was not observed");
    let status = driver.read_msr(0x1b1)?;
    ensure!(status & (1 << 31) != 0, "CPU thermal status is invalid");
    Ok(f64::from(target_celsius - ((status >> 16) & 0x7f) as i32))
}

/// SDK 0x14a5f0 measures fixed core/reference counters around the model-specific timer window.
/// Both control registers are saved before any mutation and restored, including
/// failed writes. The caller owns affinity and the shared driver lock throughout.
pub fn fixed_counter_clock<T: Transport>(
    driver: &mut Driver<T>,
    programmable_counter_count: u8,
    wait_measurement_window: impl FnOnce(&mut Driver<T>) -> anyhow::Result<()>,
) -> anyhow::Result<f64> {
    let global = driver.read_msr(0x38f)?;
    let fixed = driver.read_msr(0x38d)?;
    let global_low = match programmable_counter_count {
        2 => 3,
        4 => 15,
        6 => 63,
        8 => 255,
        _ => global as u32,
    };
    let enabled = (global & 0xffff_ffff_0000_0000) | (7_u64 << 32) | u64::from(global_low);
    let mut fixed_attempted = false;
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        write_confirmed(driver, 0x38f, enabled)?;
        fixed_attempted = true;
        write_confirmed(driver, 0x38d, (fixed & 0xffff_ffff_0000_0000) | 819)?;
        let tsc = driver.measured_tsc_mhz()?;
        let core_before = driver.read_msr(0x30a)?;
        let reference_before = driver.read_msr(0x30b)?;
        wait_measurement_window(driver)?;
        let core_after = driver.read_msr(0x30a)?;
        let reference_after = driver.read_msr(0x30b)?;
        let core = core_after.wrapping_sub(core_before) as i64;
        let reference = reference_after.wrapping_sub(reference_before) as i64;
        ensure!(core > 0 && reference > 0, "CPU measurement counters did not advance");
        let value = (core as f32 * tsc) / reference as f32;
        ensure!(value.is_finite() && value > 0.0, "CPU counter frequency is invalid");
        Ok(f64::from(value))
    }));
    let mut result = observed.unwrap_or_else(|_| Err(anyhow::anyhow!("CPU measurement panicked; original controls are being restored")));
    if fixed_attempted {
        result = finish(result, write_confirmed(driver, 0x38d, fixed), "restore fixed CPU counter control");
    }
    finish(result, write_confirmed(driver, 0x38f, global), "restore global CPU counter control")
}

fn write_confirmed<T: Transport>(driver: &mut Driver<T>, register: u32, value: u64) -> anyhow::Result<()> {
    driver.write_msr(register, value)?;
    ensure!(driver.read_msr(register)? == value, "MSR {register:#x} write-back was not confirmed");
    Ok(())
}

fn finish<T>(result: anyhow::Result<T>, cleanup: anyhow::Result<()>, action: &str) -> anyhow::Result<T> {
    match (result, cleanup) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error).with_context(|| action.to_owned()),
        (Err(error), Err(cleanup)) => Err(error.context(format!("{action} also failed: {cleanup:#}"))),
    }
}

#[cfg(test)]
#[path = "intel_cpu_tests.rs"]
mod tests;
