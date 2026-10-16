//! Marie_FW43828's compared local microphone DSP mutations, UI7405 wizard.
use super::*;
use serde_json::Value;

pub(super) fn validate(module: u8, payload: &Value) -> anyhow::Result<()> {
    ensure!(
        (2..=6).contains(&module) && payload["isEnabled"].is_boolean(),
        "Microphone DSP module/enabled payload differs"
    );
    if payload["isEnabled"] != true {
        return Ok(());
    }
    let number = |name: &str, minimum: f64, maximum: f64| -> anyhow::Result<()> {
        let number = payload[name]
            .as_f64()
            .with_context(|| format!("Microphone DSP {name} is absent"))?;
        ensure!(
            number.is_finite() && (minimum..=maximum).contains(&number),
            "Microphone DSP {name} exceeds wire range"
        );
        Ok(())
    };
    match module {
        2 => {
            number("thresholdDB", -256., 0.)?;
            number("atTime", 0., 255.)?;
            number("rlTime", 0., 65535.)?;
            number("holdTime", 0., 255.)?;
            number("decayDB", -6553.6, 0.)?;
        }
        3 => number("value", 0., 100.)?,
        4 => {
            number("db", -256., 0.)?;
            number("ratio", 0., 6553.5)?;
            number("atTime", 0., 65535.)?;
            number("rlTime", 0., 65535.)?;
        }
        5 => {}
        6 => {
            number("roomMode", 0., 3.)?;
            ensure!(
                payload["roomMode"]
                    .as_f64()
                    .is_some_and(|mode| mode.fract() == 0.),
                "Microphone reverb room mode is not an integer"
            );
            number("intensity", 0., 100.)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}
pub(super) fn execute<T: FnMut() -> anyhow::Result<u8>, V: Fn() -> anyhow::Result<()>>(
    executor: &mut Executor<'_, T, V>,
    module: u8,
    payload: &Value,
) -> anyhow::Result<bool> {
    let enabled = payload["isEnabled"] == true;
    let state = executor
        .stage([3, 8, 150], &[1, module, 0], 3, false)?
        .is_some_and(|data| data[2] == 1);
    let changed = state != enabled;
    let mut state_success = true;
    if changed {
        state_success = executor
            .stage([3, 8, 22], &[1, module, u8::from(enabled)], 3, true)?
            .is_some();
    }
    if module == 5 {
        return Ok(changed && state_success);
    }
    if !enabled {
        return Ok(state_success);
    }
    let value = |key: &str| payload[key].as_f64().expect("validated source DSP number");
    let mut expected = Vec::new();
    let command = match module {
        2 => {
            let release = (value("rlTime") as u16).to_be_bytes();
            let decay = if value("decayDB") == 0. {
                0
            } else {
                (value("decayDB") * 10. + 65536.) as u16
            };
            expected.extend([
                1,
                1,
                negative_byte(value("thresholdDB")),
                value("atTime") as u8,
                release[0],
                release[1],
                value("holdTime") as u8,
            ]);
            expected.extend(decay.to_be_bytes());
            [9, 8, 27]
        }
        3 => {
            expected.extend([1, value("value") as u8]);
            [2, 8, 28]
        }
        4 => {
            expected.extend([1, 1, negative_byte(value("db"))]);
            for n in [value("ratio") * 10., value("atTime"), value("rlTime")] {
                expected.extend((n as u16).to_be_bytes());
            }
            [9, 8, 29]
        }
        6 => {
            expected.extend([1, value("roomMode") as u8, value("intensity") as u8]);
            [3, 8, 31]
        }
        _ => unreachable!(),
    };
    // Noise/compressor get helpers observe module enable again before their
    // parameter getter. That observation is not a source-to-UI state update.
    if matches!(module, 2 | 4) {
        executor.stage([3, 8, 150], &[1, module, 0], 3, false)?;
    }
    let mut request = vec![0; command[0] as usize];
    request[0] = 1;
    let observed = executor.stage(
        [command[0], 8, command[2] + 128],
        &request,
        expected.len(),
        false,
    )?;
    // Current helper exposes reverb `mode`; Marie_FW compares `roomMode`,
    // which remains -1, so enabled source reverb reasserts its parameters.
    if observed
        .as_deref()
        .is_some_and(|data| same_parameters(module, data, payload))
    {
        return Ok(state_success);
    }
    let wrote = executor
        .stage(command, &expected, expected.len(), true)?
        .is_some();
    Ok(state_success && wrote)
}

fn negative_byte(value: f64) -> u8 {
    if value == 0. { 0 } else { (value + 256.) as u8 }
}
fn decoded_byte(value: u8) -> f64 {
    if value == 0 { 0. } else { f64::from(value) - 256. }
}
fn same_parameters(module: u8, data: &[u8], payload: &Value) -> bool {
    let value = |key: &str| payload[key].as_f64().expect("validated source DSP number");
    let word = |at: usize| f64::from(u16::from_be_bytes([data[at], data[at + 1]]));
    match module {
        2 => {
            data[1] != 0
                && decoded_byte(data[2]) == value("thresholdDB")
                && f64::from(data[3]) == value("atTime")
                && word(4) == value("rlTime")
                && f64::from(data[6]) == value("holdTime")
                && (if word(7) == 0. {
                    0.
                } else {
                    0.1 * (word(7) - 65536.)
                }) == value("decayDB")
        }
        3 => f64::from(data[1]) == value("value"),
        4 => {
            data[1] != 0
                && decoded_byte(data[2]) == value("db")
                && word(3) / 10. == value("ratio")
                && word(5) == value("atTime")
                && word(7) == value("rlTime")
        }
        // Protocol helper returns `mode`, whereas the current feature compares
        // `roomMode` (undefined -> -1). Even matching enabled reverb reasserts.
        6 => false,
        _ => unreachable!(),
    }
}
