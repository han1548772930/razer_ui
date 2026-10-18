//! Full-width CPU counters with source-defined ACPI/QPC compensation.
use super::acpi_timer::FREQUENCY;
use anyhow::{Context as _, ensure};

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
    ensure!(
        tsc_mhz.is_finite() && tsc_mhz > 0.0 && qpc_frequency > 0,
        "AMD reference frequency was not observed"
    );
    if let Some((before, after)) = dynamic_clock {
        ensure!(
            before.is_finite()
                && after.is_finite()
                && before >= 0.0
                && after >= 0.0
                && (after - before).abs() < 0.05,
            "AMD dynamic clock changed during measurement"
        );
    }
    let aperf = after.aperf.wrapping_sub(before.aperf) as i64;
    let mperf = after.mperf.wrapping_sub(before.mperf) as i64;
    // The source compensates wrapping only inside its wait loop, not the
    // measurement formula. Reject that window instead of publishing a negative clock.
    let acpi = i64::from(after.acpi) - i64::from(before.acpi);
    let qpc = after
        .qpc
        .checked_sub(before.qpc)
        .context("AMD QPC interval overflow")?;
    ensure!(
        aperf > 0 && mperf > 0 && acpi > 0 && qpc > 0,
        "AMD counters did not advance within a valid measurement window"
    );
    let numerator = qpc
        .checked_mul(FREQUENCY as i64)
        .context("AMD timer numerator overflow")?;
    let denominator = u64::try_from(acpi)?
        .checked_mul(qpc_frequency)
        .filter(|value| *value <= i64::MAX as u64)
        .context("AMD timer denominator overflow")?;
    let value = (numerator as f32 / denominator as f32) * ((aperf as f32 * tsc_mhz) / mperf as f32);
    ensure!(
        value.is_finite() && value > 0.0,
        "AMD measured frequency is invalid"
    );
    Ok(f64::from(value))
}
