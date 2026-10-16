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
pub fn read_initial(device: &dyn FeatureTransport) -> anyhow::Result<Snapshot> {
    let mut report = [0; 16];
    report[0] = 9;
    let count = device.get_input(&mut report)?;
    ensure!(
        count <= report.len(),
        "Mixer hardware input count exceeds requested capacity"
    );
    Snapshot::decode(&report[..count], true)
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
}
