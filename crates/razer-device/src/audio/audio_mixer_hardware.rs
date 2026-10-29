//! Current1342 AudioMixer.getHardwareInfo and rzHardwareEventsAudioMixerParser.
use crate::backend::FeatureTransport;
use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub keys: [bool; 6],
    pub channel_mutes: [bool; 4],
    pub mic_muted: bool,
    pub mic_long_press: bool,
    pub sliders: [u8; 4],
    pub phantom_power: bool,
    pub mic_plugged: bool,
    pub line_in_plugged: bool,
}
impl Snapshot {
    pub fn decode(bytes: &[u8], initial: bool) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() >= 8 && bytes[0] == 9,
            "Mixer hardware report is incomplete or has wrong ID"
        );
        Ok(Self {
            keys: [1, 2, 4, 8, 128, 32].map(|mask| bytes[1] & mask != 0),
            channel_mutes: [1, 2, 4, 8].map(|mask| bytes[2] & mask != 0),
            mic_muted: bytes[2] & 32 != 0,
            // Startup getter deliberately omits the long-press bit.
            mic_long_press: !initial && bytes[1] & 64 != 0,
            sliders: [bytes[3], bytes[4], bytes[5], bytes[6]],
            phantom_power: bytes[7] & 1 != 0,
            mic_plugged: bytes[7] & 2 != 0,
            line_in_plugged: bytes[7] & 8 != 0,
        })
    }
}

#[derive(Debug)]
pub struct InitialReadFailure {
    pub snapshot: Snapshot,
}

impl std::fmt::Display for InitialReadFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .write_str("Mixer initial snapshot decoded; startup or exchange finalization failed")
    }
}

impl std::error::Error for InitialReadFailure {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HardwareEventReceipt {
    pub snapshot: Snapshot,
    pub receipts: Vec<crate::audio_mixer_local::Receipt>,
    pub command_failures: Vec<crate::audio_mixer_local::CommandBatchFailure>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HardwareEventsFailure {
    /// Includes any decoded reports not processed when the input phase failed.
    pub snapshots: Vec<Snapshot>,
    pub events: Vec<HardwareEventReceipt>,
}

impl std::fmt::Display for HardwareEventsFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "Mixer hardware-event processing failed; actual inputs and command receipts retained",
        )
    }
}
impl std::error::Error for HardwareEventsFailure {}

pub fn read_initial(device: &dyn FeatureTransport) -> anyhow::Result<Snapshot> {
    let mut snapshot = None;
    let result = crate::feature_exchange::with_exchange(
        device,
        crate::backend::ExchangeOptions::plain(),
        |device, _| {
            let mut report = [0; 16];
            report[0] = 9;
            let count = device.get_input(&mut report)?;
            ensure!(
                count <= report.len(),
                "Mixer hardware input count exceeds requested capacity"
            );
            let decoded = Snapshot::decode(&report[..count], true)?;
            snapshot = Some(decoded.clone());
            Ok(decoded)
        },
    );
    result.map_err(|error| match snapshot {
        Some(snapshot) => error.context(InitialReadFailure { snapshot }),
        None => error,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_getter_and_event_long_press() {
        let bytes = [9, 255, 37, 10, 20, 30, 40, 11];
        let getter = Snapshot::decode(&bytes, true).unwrap();
        let event = Snapshot::decode(&bytes, false).unwrap();
        assert!(!getter.mic_long_press);
        assert!(event.mic_long_press);
        assert_eq!(event.channel_mutes, [true, false, true, false]);
        assert!(
            event.mic_muted && event.phantom_power && event.mic_plugged && event.line_in_plugged
        );
        assert_eq!(event.sliders, [10, 20, 30, 40]);
        assert!(Snapshot::decode(&bytes[..7], false).is_err());
    }

    #[test]
    fn parsed_initial_snapshot_is_diagnostic_after_release_failure_only() {
        struct Input(Vec<u8>);
        impl FeatureTransport for Input {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("unexpected SET")
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                panic!("unexpected GET")
            }
            fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
                assert_eq!(report[0], 9);
                report[..self.0.len()].copy_from_slice(&self.0);
                Ok(self.0.len())
            }
            fn metadata(&self) -> serde_json::Value {
                serde_json::Value::Null
            }
        }
        let bytes = vec![9, 255, 37, 10, 20, 30, 40, 11];
        for valid in [false, true] {
            let input = Input(if valid { bytes.clone() } else { bytes[..7].to_vec() });
            let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&input);
            let error = read_initial(&probe).unwrap_err();
            let evidence = error.downcast_ref::<InitialReadFailure>();
            assert_eq!(evidence.is_some(), valid);
            if let Some(evidence) = evidence {
                assert_eq!(evidence.snapshot, Snapshot::decode(&bytes, true).unwrap());
                assert!(!evidence.snapshot.mic_long_press);
                assert_eq!(
                    error.root_cause().to_string(),
                    "simulated explicit release failed"
                );
            }
        }
    }
}
