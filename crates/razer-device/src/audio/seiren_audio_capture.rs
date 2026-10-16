//! Current Seiren EQ analyser input: the first Razer Seiren V3 Pro capture
//! label excluding Mic (Dry), then the default input. PCM and rates are separate
//! from the Marie firmware's microphone/headphone peak-meter getters.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureFrame {
    pub output_sample_rate: Option<u32>,
    pub capture_sample_rate: u32,
    pub mono_pcm: Vec<f32>,
    pub microphone_levels: Option<[f32; 2]>,
    pub headphone_levels: Option<[f32; 2]>,
    pub endpoint_id: String,
    pub label: String,
    pub default_fallback: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CapturePoll {
    pub session_id: u64,
    pub product_id: u32,
    pub started: bool,
    pub terminal: bool,
    pub canceled: bool,
    pub error: Option<String>,
    /// Pending initialization has no invented sample rate or sample values.
    pub frame: Option<CaptureFrame>,
}

pub fn validate_owner(session_id: u64, product_id: u32) -> anyhow::Result<()> {
    ensure!(session_id != 0, "Seiren audio capture session ID is zero");
    ensure!(
        super::seiren_eq::capability(product_id).is_some(),
        "Current Seiren EQ analyser has no source caller for this product"
    );
    Ok(())
}

/// Preserve the source enumeration's order and case-sensitive label filters.
pub fn select_endpoint(labels: &[&str], default_index: Option<usize>) -> Option<usize> {
    labels
        .iter()
        // Both current 1422 and 1446 UI8193 exports use this exact value.
        .position(|label| label.contains("Razer Seiren V3 Pro") && !label.contains("Mic (Dry)"))
        .or_else(|| default_index.filter(|index| *index < labels.len()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Integer,
    Float,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub valid_bits_per_sample: u16,
    encoding: Encoding,
}

impl PcmFormat {
    /// WAVEFORMATEX and WAVEFORMATEXTENSIBLE are decoded from OS-owned bytes;
    /// protocol logic does not depend on a Windows SDK or workstation files.
    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(bytes.len() >= 18, "Capture WAVEFORMATEX is truncated");
        let word = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let sample_rate = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let mut tag = word(0);
        let channels = word(2);
        let block_align = word(12);
        let bits_per_sample = word(14);
        let extra = usize::from(word(16));
        ensure!(
            bytes.len() >= 18 + extra,
            "Capture extended format is truncated"
        );
        let mut valid_bits_per_sample = bits_per_sample;
        if tag == 0xfffe {
            ensure!(extra >= 22, "Capture WAVEFORMATEXTENSIBLE is truncated");
            valid_bits_per_sample = word(18);
            ensure!(
                bytes[26..40] == [0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71],
                "Capture audio subtype is not PCM or IEEE float"
            );
            tag = word(24);
        }
        let encoding = match tag {
            1 => Encoding::Integer,
            3 => Encoding::Float,
            _ => anyhow::bail!("Capture audio format tag {tag} has no PCM decoder"),
        };
        ensure!(
            sample_rate > 0 && channels > 0,
            "Capture sample rate/channels are zero"
        );
        ensure!(
            match encoding {
                Encoding::Integer => [8, 16, 24, 32].contains(&bits_per_sample),
                Encoding::Float => [32, 64].contains(&bits_per_sample),
            },
            "Capture PCM container width is unsupported"
        );
        ensure!(
            valid_bits_per_sample > 0 && valid_bits_per_sample <= bits_per_sample,
            "Capture valid sample width differs from its container"
        );
        ensure!(
            usize::from(block_align) >= usize::from(channels) * usize::from(bits_per_sample / 8),
            "Capture interleaved frame size is truncated"
        );
        Ok(Self {
            sample_rate,
            channels,
            block_align,
            bits_per_sample,
            valid_bits_per_sample,
            encoding,
        })
    }

    pub fn bytes_for_frames(&self, frames: usize) -> anyhow::Result<usize> {
        frames
            .checked_mul(usize::from(self.block_align))
            .context("Capture packet byte count overflow")
    }

    fn sample(&self, bytes: &[u8]) -> anyhow::Result<f32> {
        let value = match (self.encoding, self.bits_per_sample) {
            (Encoding::Integer, 8) => (f32::from(bytes[0]) - 128.0) / 128.0,
            (Encoding::Integer, 16) => {
                f32::from(i16::from_le_bytes(bytes[..2].try_into().unwrap())) / 32768.0
            }
            (Encoding::Integer, 24) => {
                let sign = if bytes[2] & 128 == 0 { 0 } else { 255 };
                i32::from_le_bytes([bytes[0], bytes[1], bytes[2], sign]) as f32 / 8_388_608.0
            }
            // Valid PCM bits in extensible formats are left aligned.
            (Encoding::Integer, 32) => {
                i32::from_le_bytes(bytes[..4].try_into().unwrap()) as f32 / 2_147_483_648.0
            }
            (Encoding::Float, 32) => f32::from_le_bytes(bytes[..4].try_into().unwrap()),
            (Encoding::Float, 64) => f64::from_le_bytes(bytes[..8].try_into().unwrap()) as f32,
            _ => unreachable!("validated PCM container"),
        };
        ensure!(
            value.is_finite(),
            "Capture PCM contains a non-finite sample"
        );
        Ok(value)
    }

    /// Web Audio's analyser downmix uses its speaker matrices for mono,
    /// stereo, quad and 5.1; other layouts use the discrete first channel.
    pub fn mono(&self, bytes: &[u8], frames: usize, silent: bool) -> anyhow::Result<Vec<f32>> {
        let expected = self.bytes_for_frames(frames)?;
        if silent {
            // AUDCLNT_BUFFERFLAGS_SILENT describes actual silent frames; its
            // data pointer is allowed to be null.
            return Ok(vec![0.0; frames]);
        }
        ensure!(bytes.len() >= expected, "Capture PCM packet is truncated");
        let width = usize::from(self.bits_per_sample / 8);
        let mut mono = Vec::with_capacity(frames);
        for frame in bytes[..expected].chunks_exact(usize::from(self.block_align)) {
            let sample =
                |channel: usize| self.sample(&frame[channel * width..(channel + 1) * width]);
            let value = match self.channels {
                2 => (sample(0)? + sample(1)?) * 0.5,
                4 => (sample(0)? + sample(1)? + sample(2)? + sample(3)?) * 0.25,
                6 => {
                    (sample(0)? + sample(1)?) * std::f32::consts::FRAC_1_SQRT_2
                        + sample(2)?
                        + (sample(4)? + sample(5)?) * 0.5
                }
                _ => sample(0)?,
            };
            ensure!(value.is_finite(), "Capture downmixed PCM is non-finite");
            mono.push(value);
        }
        Ok(mono)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(tag: u16, channels: u16, bits: u16) -> Vec<u8> {
        let mut bytes = vec![0; 18];
        bytes[..2].copy_from_slice(&tag.to_le_bytes());
        bytes[2..4].copy_from_slice(&channels.to_le_bytes());
        bytes[4..8].copy_from_slice(&48_000u32.to_le_bytes());
        bytes[12..14].copy_from_slice(&(channels * bits / 8).to_le_bytes());
        bytes[14..16].copy_from_slice(&bits.to_le_bytes());
        bytes
    }

    #[test]
    fn endpoint_order_filter_and_default_match_the_current_caller() {
        let labels = [
            "Microphone (Razer Seiren V3 Pro Mic (Dry))",
            "Razer Seiren V3 Chroma",
            "Microphone (Razer Seiren V3 Pro)",
            "Another Razer Seiren V3 Pro",
        ];
        assert_eq!(select_endpoint(&labels, Some(0)), Some(2));
        assert_eq!(
            select_endpoint(&["Razer Seiren V3 Chroma", "USB microphone"], Some(1)),
            Some(1)
        );
        assert_eq!(
            select_endpoint(&["seiren", "USB microphone"], Some(1)),
            Some(1)
        );
        assert_eq!(select_endpoint(&[], None), None);
    }

    #[test]
    fn signed_pcm_and_float_packets_retain_the_actual_scale_and_downmix() {
        let pcm = PcmFormat::parse(&format(1, 1, 24)).unwrap();
        assert_eq!(
            pcm.mono(&[0, 0, 128, 0, 0, 64], 2, false).unwrap(),
            [-1.0, 0.5]
        );
        let pcm = PcmFormat::parse(&format(3, 2, 32)).unwrap();
        let bytes = [0.75f32, -0.25, -1.0, 0.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(pcm.mono(&bytes, 2, false).unwrap(), [0.25, -0.5]);
        assert_eq!(pcm.mono(&[], 3, true).unwrap(), [0.0; 3]);
        assert!(pcm.mono(&bytes[..3], 1, false).is_err());
    }

    #[test]
    fn extensible_pcm_and_invalid_subtypes_are_decoded_from_real_format_bytes() {
        let mut bytes = format(0xfffe, 1, 32);
        bytes[16..18].copy_from_slice(&22u16.to_le_bytes());
        bytes.extend_from_slice(&24u16.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&[
            1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
        ]);
        let pcm = PcmFormat::parse(&bytes).unwrap();
        assert_eq!(pcm.valid_bits_per_sample, 24);
        assert_eq!(pcm.mono(&i32::MIN.to_le_bytes(), 1, false).unwrap(), [-1.0]);
        bytes[24] = 4;
        assert!(PcmFormat::parse(&bytes).is_err());
    }
}
