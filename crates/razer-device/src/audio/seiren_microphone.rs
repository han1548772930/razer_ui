//! Current UI7405/9374 microphone endpoint rules and UI9909/596 measurements.
//! OS capture/playback remains in the service; these values describe actual PCM.
use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CapturePurpose {
    #[default]
    Equalizer,
    LivePreview,
    Recording,
    Speech,
    Environment,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct CaptureOptions {
    purpose: CapturePurpose,
    sample_rate: Option<u32>,
}
impl CaptureOptions {
    pub fn new(purpose: CapturePurpose, sample_rate: Option<u32>) -> anyhow::Result<Self> {
        let value = Self {
            purpose,
            sample_rate,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn purpose(&self) -> CapturePurpose {
        self.purpose
    }
    pub fn sample_rate(&self) -> Option<u32> {
        self.sample_rate
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.sample_rate
                .is_none_or(|rate| matches!(rate, 48_000 | 96_000)),
            "Seiren microphone rate differs from current 48/96kHz choices"
        );
        Ok(())
    }
    /// Source enumerateDevices includes the default alias first. WASAPI exposes
    /// that alias separately; callers pass its actual selected endpoint index.
    pub fn select_endpoint(&self, labels: &[&str], default: Option<usize>) -> Option<usize> {
        match self.purpose {
            CapturePurpose::Equalizer => {
                super::seiren_audio_capture::select_endpoint(labels, default)
            }
            CapturePurpose::LivePreview => default
                .filter(|ix| *ix < labels.len())
                .or_else(|| (!labels.is_empty()).then_some(0)),
            CapturePurpose::Recording => labels
                .iter()
                .position(|label| label.contains("Service (Do not use)"))
                .or_else(|| default.filter(|ix| *ix < labels.len())),
            CapturePurpose::Speech | CapturePurpose::Environment => {
                let excluded = |label: &str| {
                    let lower = label.to_lowercase();
                    lower.contains("stream mix")
                        || lower.contains("playback mix")
                        || lower.starts_with("default - ")
                        || lower.contains("service (do not use)")
                };
                let product = |label: &str| {
                    !excluded(label)
                        && label.contains("Razer Seiren V3 Pro")
                        && !label.contains("Mic (Dry)")
                };
                labels
                    .iter()
                    .position(|label| product(label) && label.to_lowercase().contains("microphone"))
                    .or_else(|| labels.iter().position(|label| product(label)))
                    .or_else(|| {
                        labels.iter().position(|label| {
                            !excluded(label) && label.to_lowercase().contains("microphone")
                        })
                    })
                    .or_else(|| labels.iter().position(|label| !excluded(label)))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct PcmClip {
    rate: u32,
    samples: Vec<f32>,
}
impl PcmClip {
    pub fn new(rate: u32, samples: Vec<f32>) -> anyhow::Result<Self> {
        ensure!(
            rate > 0 && !samples.is_empty(),
            "Microphone recording has no actual PCM"
        );
        ensure!(
            samples.iter().all(|sample| sample.is_finite()),
            "Microphone PCM contains non-finite samples"
        );
        Ok(Self { rate, samples })
    }
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

/// Local storage prepares recorded floats in one owned file, avoiding an
/// unbounded IPC JSON frame. The caller releases it after the service joins.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PcmPlaybackFile {
    path: String,
    sample_rate: u32,
    sample_count: u64,
}
impl PcmPlaybackFile {
    pub fn new(path: String, sample_rate: u32, sample_count: u64) -> anyhow::Result<Self> {
        ensure!(
            !path.is_empty() && sample_rate > 0 && sample_count > 0,
            "Local microphone playback is empty"
        );
        Ok(Self {
            path,
            sample_rate,
            sample_count,
        })
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlaybackPoll {
    pub session_id: u64,
    pub product_id: u32,
    pub started: bool,
    pub terminal: bool,
    pub canceled: bool,
    pub error: Option<String>,
}

pub fn percent_to_db(percent: f64) -> f64 {
    // UI8951: piecewise native microphone fader curve, not 36 * percent.
    const POINTS: [(f64, f64); 9] = [
        (0., 0.),
        (14., 2.1),
        (40., 6.9),
        (60., 12.),
        (70., 15.3),
        (80., 19.7),
        (84., 21.8),
        (87., 23.7),
        (100., 36.),
    ];
    interpolate(percent, &POINTS)
}
pub fn db_to_percent(db: f64) -> f64 {
    const POINTS: [(f64, f64); 9] = [
        (0., 0.),
        (2.1, 14.),
        (6.9, 40.),
        (12., 60.),
        (15.3, 70.),
        (19.7, 80.),
        (21.8, 84.),
        (23.7, 87.),
        (36., 100.),
    ];
    interpolate(db, &POINTS)
}
fn interpolate(value: f64, points: &[(f64, f64)]) -> f64 {
    let value = value.clamp(points[0].0, points[points.len() - 1].0);
    for pair in points.windows(2) {
        if value <= pair[1].0 {
            return pair[0].1
                + (value - pair[0].0) / (pair[1].0 - pair[0].0) * (pair[1].1 - pair[0].1);
        }
    }
    points[points.len() - 1].1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeechMeasurement {
    pub metric_db: f64,
    pub recommended_db: f64,
    pub clipped_windows: usize,
}
pub fn speech_measurement(samples: &[f32], rate: u32) -> Option<SpeechMeasurement> {
    if rate == 0 {
        return None;
    }
    let window = ((f64::from(rate) * 0.05).round() as usize).max(1);
    let hop = ((f64::from(rate) * 0.025).round() as usize).max(1);
    let mut peaks = Vec::new();
    for at in (0..samples.len().saturating_sub(window).saturating_add(1)).step_by(hop) {
        let values = samples.get(at..at + window)?;
        let peak = values
            .iter()
            .map(|value| f64::from(value.abs()))
            .fold(0., f64::max);
        peaks.push(20. * peak.max(1e-12).log10());
    }
    if peaks.is_empty() {
        return None;
    }
    let clipped_windows = peaks.iter().filter(|value| **value >= -0.1).count();
    let mut selected = peaks
        .iter()
        .copied()
        .filter(|value| *value >= -50.)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        selected = peaks;
    }
    selected.sort_by(f64::total_cmp);
    let at = 0.95 * (selected.len() - 1) as f64;
    let low = at.floor() as usize;
    let metric_db = selected[low] * (1. - at.fract())
        + selected[(low + 1).min(selected.len() - 1)] * at.fract();
    Some(SpeechMeasurement {
        metric_db,
        recommended_db: (-10. - metric_db).clamp(-36., 36.),
        clipped_windows,
    })
}
pub fn environment_measurement(samples: &[f32], rate: u32) -> Option<f64> {
    if rate == 0 {
        return None;
    }
    let window = ((f64::from(rate) * 0.1).floor() as usize).max(1);
    let mut db = samples
        .chunks_exact(window)
        .map(|values| {
            let rms = (values
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum::<f64>()
                / window as f64)
                .sqrt();
            20. * rms.max(1e-9).log10()
        })
        .collect::<Vec<_>>();
    if db.is_empty() {
        return None;
    }
    db.sort_by(f64::total_cmp);
    let floor = db[((db.len() as f64 * 0.3).floor() as usize).min(db.len() - 1)];
    // UI596 threshold=floor+5 even when gateOpen=false; UI7405 Ma applies
    // Math.round then Math.min(-65). Preserve both independent source quirks.
    Some((floor + 5. + 0.5).floor().min(-65.))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn microphone_owners_select_distinct_current_source_endpoints() {
        let labels = [
            "Microphone (USB)",
            "Service (Do not use)",
            "Microphone (Razer Seiren V3 Pro Mic (Dry))",
            "Microphone (Razer Seiren V3 Pro)",
        ];
        let option = |purpose| CaptureOptions::new(purpose, Some(48_000)).unwrap();
        assert_eq!(
            option(CapturePurpose::Recording).select_endpoint(&labels, Some(0)),
            Some(1)
        );
        assert_eq!(
            option(CapturePurpose::LivePreview).select_endpoint(&labels, Some(0)),
            Some(0)
        );
        assert_eq!(
            option(CapturePurpose::Speech).select_endpoint(&labels, Some(0)),
            Some(3)
        );
    }
    #[test]
    fn microphone_measurements_require_real_windows_and_preserve_noise_clamp() {
        assert!(speech_measurement(&[], 48_000).is_none());
        let speech = vec![0.1; 48_000];
        let result = speech_measurement(&speech, 48_000).unwrap();
        assert!((result.metric_db + 20.).abs() < 0.001);
        assert!((result.recommended_db - 10.).abs() < 0.001);
        assert_eq!(
            environment_measurement(&vec![0.01; 48_000], 48_000),
            Some(-65.)
        );
        assert_eq!(
            environment_measurement(&vec![0.0001; 48_000], 48_000),
            Some(-75.)
        );
    }
}
