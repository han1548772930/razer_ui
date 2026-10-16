//! Current simple_service speaker/microphone endpoint volume semantics.
//! Both source entry points reach the same ID-keyed native implementation.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioVolumeReadResult {
    pub result: bool,
    pub reason: String,
    #[serde(rename = "deviceId")]
    pub device_id: String,
    pub muted: bool,
    pub volume: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioVolumeWriteResponse {
    pub result: bool,
    pub reason: String,
}

/// The source callback result and the subsequent real observation are distinct.
/// A failed mute after a successful volume call does not roll back volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioVolumeWriteResult {
    pub source_response: AudioVolumeWriteResponse,
    pub previous: AudioVolumeReadResult,
    pub observation: Option<AudioVolumeReadResult>,
    pub observation_error: Option<String>,
    pub volume_submitted: bool,
    pub mute_submitted: bool,
    pub hresult: i32,
}

/// RVA 0x37A97 performs an f32 multiply, then 0x37A9F/0x37AA3 converts to
/// f64 and adds 0.5; CVTTSD2SI truncates before storing the low byte.
/// This function accepts a validated Core Audio scalar in the range 0..=1.
pub fn scalar_to_volume(scalar: f32) -> u8 {
    (f64::from(scalar * 100.0_f32) + 0.5_f64) as u8
}

/// RVA 0x37B6D zero-extends the FFI uint8 and divides f32 by 100.0.
/// Values above 100 reach Core Audio and may fail; the source does not clamp.
pub fn volume_to_scalar(volume: u8) -> f32 {
    f32::from(volume) / 100.0_f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_quantization_round_trips_valid_core_audio_values() {
        for (scalar, expected) in [(0.0, 0), (0.005, 1), (0.5, 50), (0.995, 100), (1.0, 100)] {
            assert_eq!(scalar_to_volume(scalar), expected);
            assert!((volume_to_scalar(expected) - expected as f32 / 100.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn source_setter_does_not_change_the_wire_quantization_rule() {
        assert_eq!(volume_to_scalar(37), 0.37);
        assert_eq!(scalar_to_volume(volume_to_scalar(37)), 37);
    }
}
