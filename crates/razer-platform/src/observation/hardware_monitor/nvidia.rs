//! Versioned NVAPI replies used by SDK 0x11d3e0, 0x11d2d0 and 0x11f8e0.
use anyhow::{Context as _, ensure};

pub fn thermal_sensor(capabilities: &[u32; 684]) -> anyhow::Result<(usize, u32)> {
    let mask = capabilities[1];
    let index = (0..32).find(|&i| mask & (1 << i) != 0 && capabilities[21 * i + 10] != 0 && capabilities[21 * i + 11] == 0)
        .context("NVAPI has no GPU thermal sensor")?;
    Ok((index, mask))
}

pub fn thermal_value(values: &[u32; 42], index: usize) -> anyhow::Result<f64> {
    ensure!(index < 32, "NVAPI thermal sensor index is out of bounds");
    let value = values[index + 10];
    ensure!(value & 0xff00 != 0xff00, "NVAPI thermal sensor returned its unavailable marker");
    Ok(f64::from(value as i32 as f32 * 0.00390625))
}

pub fn legacy_temperature(reply: &[u32; 17]) -> anyhow::Result<f64> {
    let count = usize::try_from(reply[1])?;
    ensure!(count <= 3, "NVAPI thermal count exceeds the versioned buffer");
    (0..count).find_map(|i| {
        let current = reply[5 + 5 * i] as i32;
        (reply[6 + 5 * i] == 1 && current > 0).then_some(f64::from(current))
    }).context("NVAPI did not observe GPU temperature")
}

pub fn clock(reply: &[u32; 66]) -> anyhow::Result<f64> {
    ensure!(reply[2] & 1 != 0, "NVAPI GPU graphics clock is not present");
    Ok(f64::from(reply[3] as f32 / 1000.0))
}

pub fn pstate_clock(reply: &[u32; 2717]) -> anyhow::Result<f64> {
    let states = reply[1] as usize;
    let domains = reply[3] as usize;
    ensure!(states <= 12 && domains <= 32, "NVAPI performance-state count exceeds the versioned buffer");
    let current = reply[2];
    let mut observed = None;
    for state in 0..states {
        let offset = 4 + state * 226;
        if reply[offset] != current { continue; }
        for domain in 0..domains {
            if reply[offset + 2 + domain * 7] == 0 {
                let frequency = reply[offset + 4 + domain * 7] as i32;
                ensure!(frequency >= 0, "NVAPI performance-state clock is unavailable");
                observed = Some(f64::from(frequency as f32 / 1000.0));
            }
        }
    }
    observed.context("NVAPI has no graphics frequency for the current performance state")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_values_keep_zero_and_fractional_units_without_defaults() {
        let mut thermal=[0;42]; thermal[12]=65*256+128; assert_eq!(thermal_value(&thermal,2).unwrap(),65.5);
        thermal[12]=0; assert_eq!(thermal_value(&thermal,2).unwrap(),0.0);
        thermal[12]=0xff00; assert!(thermal_value(&thermal,2).is_err());
        let mut clock_reply=[0;66]; assert!(clock(&clock_reply).is_err()); clock_reply[2]=1; clock_reply[3]=1_234_500; assert_eq!(clock(&clock_reply).unwrap(),1234.5);
    }
    #[test]
    fn sensor_mask_target_and_capability_must_all_match() {
        let mut caps=[0;684]; caps[1]=4; caps[52]=1; caps[53]=0;
        assert_eq!(thermal_sensor(&caps).unwrap(),(2,4)); caps[53]=1; assert!(thermal_sensor(&caps).is_err());
    }
    #[test]
    fn legacy_and_performance_state_counts_are_bounded() {
        let mut legacy=[0;17]; legacy[1]=4; assert!(legacy_temperature(&legacy).is_err());
        let mut state=[0;2717]; state[1]=13; assert!(pstate_clock(&state).is_err()); state[1]=1; state[3]=33; assert!(pstate_clock(&state).is_err());
        state[3]=1; state[2]=7; state[4]=7; state[8]=987_500; assert_eq!(pstate_clock(&state).unwrap(),987.5); state[2]=8; assert!(pstate_clock(&state).is_err());
    }
}
