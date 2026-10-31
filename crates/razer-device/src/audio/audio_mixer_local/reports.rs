//! Actual AudioMixer JS templates and query-copy order (not native RMW).
use super::*;
use std::{thread, time::Duration};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportChannel {
    Feature,
    Output,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransferProgress {
    pub channel: ReportChannel,
    pub report: Vec<u8>,
    pub sends_attempted: u32,
    pub sends_completed: u32,
    /// The source accepts a bounded actual report without a status template.
    pub input: Option<Vec<u8>>,
}

impl TransferProgress {
    /// Reconcile the retained transport result before rejecting stale owners.
    /// Complete sends remain diagnostics when a later identity check fails.
    pub(super) fn observe_send(&mut self, result: &anyhow::Result<usize>) {
        use crate::backend::{NativeIoOutcome, native_io_attempted, native_io_outcome};

        self.sends_attempted += u32::from(native_io_attempted(result));
        let returned = match result {
            Ok(count) => Some(*count),
            Err(error) => match native_io_outcome(error) {
                Some(NativeIoOutcome::Returned { byte_count }) => Some(byte_count),
                _ => None,
            },
        };
        self.sends_completed += u32::from(returned == Some(self.report.len()));
    }
}

#[derive(Debug)]
pub struct TransferFailure {
    pub progress: TransferProgress,
}

impl std::fmt::Display for TransferFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mixer source exchange failed; actual transfer progress retained")
    }
}

impl std::error::Error for TransferFailure {}

/// AudioMixer.sendCommandOut retries native failures swallowed by the host.
/// These mounted callers have no expected-reply template: an actual response
/// completes the exchange without a Protocol25 status interpretation.
fn exchange(
    device: &dyn FeatureTransport,
    channel: ReportChannel,
    report: &[u8],
    reply: Option<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Option<Vec<u8>>> {
    let validate = &|| {
        validate().map_err(|error| {
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            )
        })
    };
    let mut progress = TransferProgress {
        channel,
        report: report.to_vec(),
        sends_attempted: 0,
        sends_completed: 0,
        input: None,
    };
    let result = crate::feature_exchange::with_validated_exchange(
        device,
        crate::backend::ExchangeOptions::plain(),
        validate,
        |device, _| {
            let mut last_error = None;
            for attempt in 0..20 {
                validate()?;
                if attempt > 0 {
                    thread::sleep(Duration::from_millis(10));
                    validate()?;
                }
                let sent = match channel {
                    ReportChannel::Feature => device.send_feature_at_collection_max_counted(report),
                    ReportChannel::Output => device.write_output(report).map(|()| report.len()),
                };
                progress.observe_send(&sent);
                if let Err(validation) = validate() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                if let Err(error) = sent
                    .and_then(|count| crate::backend::require_complete_send(count, report.len()))
                {
                    if crate::backend::exchange_interrupted(&error) {
                        return Err(error);
                    }
                    last_error = Some(error);
                    continue;
                }
                let Some(selector) = reply else {
                    return Ok(None);
                };
                thread::sleep(Duration::from_millis(5));
                validate()?;
                let mut bytes = vec![
                    0;
                    match channel {
                        ReportChannel::Feature => 64,
                        ReportChannel::Output => 16,
                    }
                ];
                bytes[0] = selector;
                let received = match channel {
                    ReportChannel::Feature => device.get_feature_at_collection_max(&mut bytes),
                    ReportChannel::Output => device.get_input(&mut bytes),
                };
                let received = match validate() {
                    Ok(()) => received,
                    Err(validation) => Err(crate::backend::complete_native_read(
                        received, &bytes, validation,
                    )),
                };
                if let Err(error) = &received {
                    if let Some(actual) = crate::backend::native_read_bytes(error) {
                        if !actual.is_empty() && actual.len() <= bytes.len() {
                            progress.input = Some(actual.to_vec());
                        }
                        return received.map(|_| None);
                    }
                    if crate::backend::exchange_interrupted(error) {
                        return received.map(|_| None);
                    }
                }
                match received {
                    Ok(count) if count > 0 && count <= bytes.len() => {
                        progress.input = Some(bytes[..count].to_vec());
                        return Ok(Some(bytes[..count].to_vec()));
                    }
                    Ok(count) => {
                        last_error = Some(anyhow::anyhow!(
                            "Mixer source reply length {count} outside 1..={}",
                            bytes.len()
                        ));
                    }
                    Err(error) => last_error = Some(error),
                }
            }
            Err(last_error.expect("Each unsuccessful source attempt records its error"))
                .context("Mixer source report failed after 20 OUT attempts")
        },
    );
    result.map_err(|error| error.context(TransferFailure { progress }))
}

pub(super) fn feature(
    device: &dyn FeatureTransport,
    report: &[u8; 64],
    reply: Option<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Option<Vec<u8>>> {
    exchange(device, ReportChannel::Feature, report, reply, validate)
}

pub(super) fn output(
    device: &dyn FeatureTransport,
    bytes: &[u8],
    reply: Option<u8>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Option<Vec<u8>>> {
    ensure!(bytes.len() <= 66, "Mixer source output exceeds 66 bytes");
    let mut report = [0; 66];
    report[..bytes.len()].copy_from_slice(bytes);
    exchange(device, ReportChannel::Output, &report, reply, validate)
}
fn query(
    device: &dyn FeatureTransport,
    bytes: &[u8],
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    let input = output(device, bytes, Some(18), validate)?
        .context("Mixer source query did not return an Input report")?;
    ensure!(input.len() > 1, "Mixer source query has no actual data");
    Ok(input[1..].to_vec())
}
pub(super) fn execute(
    device: &dyn FeatureTransport,
    command: &Command,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let mut observation = Value::Null;
    let mut query_data = None;
    let result = (|| {
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
                        let data = query(device, &[4, 24, 0, 192, 6], validate)?;
                        query_data = Some(data.clone());
                        observation = json!({"query":data});
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
                output(device, &bytes, None, validate)?;
            }
            Command::AnalogGain { key, value } => {
                ensure!(value.is_finite(), "Mixer analog gain not finite");
                let bytes = match key.as_str() {
                    "optical" => {
                        ensure!(
                            (-45. ..=0.).contains(value),
                            "Optical source gain range invalid"
                        );
                        let data = query(device, &[4, 95, 252, 0, 44], validate)?;
                        query_data = Some(data.clone());
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
                output(device, &bytes, None, validate)?;
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
                observation = json!({"feature_reply":feature(device, &report, Some(7), validate)?});
            }
            Command::Fader { region, mode } => {
                ensure!((1..=2).contains(mode), "Fader mode outside mounted source");
                let mut report = [0; 64];
                report[..13].copy_from_slice(&[7, 0, 31, 0, 0, 0, 4, 15, 17, 1, *region, *mode, 0]);
                feature(device, &report, None, validate)?;
            }
            _ => anyhow::bail!("Not a fixed Mixer source report"),
        }
        Ok(observation)
    })();
    result.map_err(|error| match query_data {
        Some(data) => CommandBatchFailure::retain(error, Vec::new(), command.clone(), Some(data)),
        None => error,
    })
}
