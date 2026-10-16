//! Actual AudioMixer JS templates and query-copy order (not native RMW).
use super::*;
fn output(device: &dyn FeatureTransport, bytes: &[u8]) -> anyhow::Result<()> {
    let mut report = [0; 66];
    report[..bytes.len()].copy_from_slice(bytes);
    device.write_output(&report)
}
fn query(device: &dyn FeatureTransport, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    output(device, bytes)?;
    let mut input = [0; 16];
    input[0] = 18;
    let count = device.get_input(&mut input)?;
    ensure!(
        count > 1 && count <= 16,
        "Mixer source query has no actual data"
    );
    Ok(input[1..count].to_vec())
}
pub(super) fn execute(
    device: &dyn FeatureTransport,
    command: &Command,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let mut observation = Value::Null;
    match command {
        Command::AnalogMute { key, enabled } => {
            let bytes = match key.as_str() {
                "optical" => vec![
                    19,
                    95,
                    252,
                    0,
                    44,
                    0,
                    0,
                    if *enabled { 128 } else { 0 },
                    if *enabled { 128 } else { 0 },
                ],
                "lineIn" => vec![
                    3,
                    24,
                    0,
                    192,
                    154,
                    if *enabled { 192 } else { 64 },
                    if *enabled { 192 } else { 64 },
                ],
                "lineOutReducer" => vec![
                    3,
                    24,
                    0,
                    192,
                    2,
                    if *enabled { 224 } else { 96 },
                    if *enabled { 224 } else { 96 },
                ],
                "playbackMixReducer" => {
                    observation = json!({"query":query(device,&[4,24,0,192,6])?});
                    validate()?;
                    vec![
                        3,
                        24,
                        0,
                        192,
                        6,
                        if *enabled { 0 } else { 128 },
                        if *enabled { 0 } else { 128 },
                    ]
                }
                _ => anyhow::bail!("Unknown source analog mute {key}"),
            };
            output(device, &bytes)?;
        }
        Command::AnalogGain { key, value } => {
            ensure!(value.is_finite(), "Mixer analog gain not finite");
            let bytes = match key.as_str() {
                "optical" => {
                    ensure!(
                        (-45. ..=0.).contains(value),
                        "Optical source gain range invalid"
                    );
                    let data = query(device, &[4, 95, 252, 0, 44])?;
                    ensure!(data.len() > 2, "Optical query missing original copy byte");
                    validate()?;
                    let raw = value.trunc().abs() as u8;
                    let bytes = vec![19, 95, 252, 0, 44, 0, data[2], raw, raw];
                    observation = json!({"query":data});
                    bytes
                }
                "lineIn" => {
                    ensure!(
                        (-10. ..=2.).contains(value),
                        "LineIn source gain range invalid"
                    );
                    let raw = ((value + 10.) / 12. * 16. + 2.).trunc() as u8;
                    vec![3, 24, 0, 192, 14, raw, raw]
                }
                "playbackMixReducer" => {
                    ensure!(
                        (-62.25..=0.).contains(value),
                        "Headphones source gain range invalid"
                    );
                    let raw = ((value + 62.25) / 62.25 * 83.).trunc() as u8;
                    vec![3, 24, 0, 192, 40, raw, raw]
                }
                "lineOutReducer" => {
                    ensure!(
                        (-65.625..=0.).contains(value),
                        "LineOut source gain range invalid"
                    );
                    let raw = ((value + 65.625) / 65.625 * 175.).trunc() as u8;
                    vec![3, 24, 0, 192, 44, raw, raw]
                }
                _ => anyhow::bail!("Unknown source analog gain {key}"),
            };
            output(device, &bytes)?;
        }
        Command::Bleep { enabled } => {
            let mut report = [0; 64];
            report[..19].copy_from_slice(&[
                7,
                0,
                0,
                0,
                0,
                0,
                0,
                2,
                12,
                0,
                112,
                0,
                20,
                5,
                u8::from(!enabled),
                0,
                0,
                0,
                0,
            ]);
            device.send_feature(&report)?;
            validate()?;
            let mut reply = [0; 64];
            reply[0] = 7;
            let count = device.get_feature(&mut reply)?;
            ensure!(count > 0 && count <= 64, "Bleep Feature reply absent");
            observation = json!({"feature_reply":&reply[..count]});
        }
        Command::Fader { region, mode } => {
            ensure!((1..=2).contains(mode), "Fader mode outside mounted source");
            let mut report = [0; 64];
            report[..13].copy_from_slice(&[7, 0, 31, 0, 0, 0, 4, 15, 17, 1, *region, *mode, 0]);
            device.send_feature(&report)?;
        }
        _ => anyhow::bail!("Not a fixed Mixer source report"),
    }
    Ok(observation)
}
