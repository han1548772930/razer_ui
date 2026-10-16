//! Pure equivalents of EQ6007 Y/Q and Web Audio AnalyserNode used by Q::U.
//! Response sample rate comes from the actual default output format. Capture
//! data comes from the local microphone; neither is inferred from draft gain.
use super::model::Band;
use std::f64::consts::PI;

pub(super) fn response_db(band: &Band, frequency: f64, sample_rate: u32) -> f64 {
    if sample_rate == 0 || frequency < 0. || frequency > f64::from(sample_rate) / 2. {
        return f64::NAN;
    }
    let centre = band.frequency.clamp(0., f64::from(sample_rate) / 2.);
    let omega = 2. * PI * centre / f64::from(sample_rate);
    let sin = omega.sin();
    let cos = omega.cos();
    let a = 10_f64.powf(band.gain / 40.);
    let alpha_q = sin / (2. * band.q);
    // Web Audio low/high-pass Q is a resonance value in dB. Peaking Q is
    // dimensionless; shelves ignore Q and use the fixed slope S=1.
    let alpha_pass = sin / (2. * 10_f64.powf(band.q / 20.));
    let beta = a.sqrt() * 2. * sin / 2_f64.sqrt();
    let (b0, b1, b2, a0, a1, a2) = match band.kind.as_str() {
        "lowpass" => (
            (1. - cos) / 2.,
            1. - cos,
            (1. - cos) / 2.,
            1. + alpha_pass,
            -2. * cos,
            1. - alpha_pass,
        ),
        "highpass" => (
            (1. + cos) / 2.,
            -(1. + cos),
            (1. + cos) / 2.,
            1. + alpha_pass,
            -2. * cos,
            1. - alpha_pass,
        ),
        "lowshelf" => (
            a * ((a + 1.) - (a - 1.) * cos + beta),
            2. * a * ((a - 1.) - (a + 1.) * cos),
            a * ((a + 1.) - (a - 1.) * cos - beta),
            (a + 1.) + (a - 1.) * cos + beta,
            -2. * ((a - 1.) + (a + 1.) * cos),
            (a + 1.) + (a - 1.) * cos - beta,
        ),
        "highshelf" => (
            a * ((a + 1.) + (a - 1.) * cos + beta),
            -2. * a * ((a - 1.) + (a + 1.) * cos),
            a * ((a + 1.) + (a - 1.) * cos - beta),
            (a + 1.) - (a - 1.) * cos + beta,
            2. * ((a - 1.) - (a + 1.) * cos),
            (a + 1.) - (a - 1.) * cos - beta,
        ),
        _ => (
            1. + alpha_q * a,
            -2. * cos,
            1. - alpha_q * a,
            1. + alpha_q / a,
            -2. * cos,
            1. - alpha_q / a,
        ),
    };
    let w = 2. * PI * frequency / f64::from(sample_rate);
    let (s, c) = w.sin_cos();
    let (s2, c2) = (2. * w).sin_cos();
    let numerator = (b0 + b1 * c + b2 * c2).powi(2) + (b1 * s + b2 * s2).powi(2);
    let denominator = (a0 + a1 * c + a2 * c2).powi(2) + (a1 * s + a2 * s2).powi(2);
    if numerator == 0. {
        f64::NEG_INFINITY
    } else {
        10. * (numerator / denominator).log10()
    }
}

pub(super) fn response_points(bands: &[Band], sample_rate: Option<u32>) -> Vec<(f64, f64)> {
    let mut frequencies = Vec::new();
    let mut frequency = 20.;
    while frequency <= 20000. {
        frequencies.push(frequency);
        frequency *= 1.005;
    }
    if frequencies.last().copied().unwrap_or_default() < 20000. {
        frequencies.push(20000.);
    }
    frequencies
        .into_iter()
        .map(|frequency| {
            // Y explicitly returns zero dB if it cannot create an AudioContext.
            // Until the native default-output query arrives, preserve that fallback.
            let gain = sample_rate.map_or(0., |rate| {
                bands
                    .iter()
                    .map(|band| response_db(band, frequency, rate))
                    .sum()
            });
            (
                super::model::log_position(frequency),
                1. - (gain + 12.) / 24.,
            )
        })
        .collect()
}

const FFT_SIZE: usize = 8192;
/// MediaStreamSource feeds the default-output AudioContext's actual rate.
/// Keep interpolation history across native PCM batches, with no assumed rate.
#[derive(Default)]
pub(super) struct Resampler {
    rates: Option<(u32, u32)>,
    pending: Vec<f32>,
    next: f64,
}
impl Resampler {
    pub fn clear(&mut self) {
        self.rates = None;
        self.pending.clear();
        self.next = 0.;
    }
    pub fn push(&mut self, samples: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
        if input_rate == 0 || output_rate == 0 {
            return Vec::new();
        }
        if self.rates != Some((input_rate, output_rate)) {
            self.clear();
            self.rates = Some((input_rate, output_rate));
        }
        if input_rate == output_rate {
            return samples.to_vec();
        }
        self.pending.extend_from_slice(samples);
        let mut output = Vec::new();
        let step = f64::from(input_rate) / f64::from(output_rate);
        while self.next.floor() + 1. < self.pending.len() as f64 {
            let index = self.next.floor() as usize;
            let fraction = (self.next - index as f64) as f32;
            output.push(self.pending[index] * (1. - fraction) + self.pending[index + 1] * fraction);
            self.next += step;
        }
        let consumed = (self.next.floor() as usize).min(self.pending.len().saturating_sub(1));
        self.pending.drain(..consumed);
        self.next -= consumed as f64;
        output
    }
}
#[derive(Clone, Copy, Default)]
struct Complex {
    real: f64,
    imaginary: f64,
}
impl Complex {
    fn add(self, rhs: Self) -> Self {
        Self {
            real: self.real + rhs.real,
            imaginary: self.imaginary + rhs.imaginary,
        }
    }
    fn subtract(self, rhs: Self) -> Self {
        Self {
            real: self.real - rhs.real,
            imaginary: self.imaginary - rhs.imaginary,
        }
    }
    fn multiply(self, rhs: Self) -> Self {
        Self {
            real: self.real * rhs.real - self.imaginary * rhs.imaginary,
            imaginary: self.real * rhs.imaginary + self.imaginary * rhs.real,
        }
    }
}

pub(super) struct Spectrum {
    samples: Vec<f64>,
    cursor: usize,
    observed: bool,
    magnitude: Vec<f64>,
}
impl Default for Spectrum {
    fn default() -> Self {
        Self {
            samples: vec![0.; FFT_SIZE],
            cursor: 0,
            observed: false,
            magnitude: vec![0.; FFT_SIZE / 2],
        }
    }
}
impl Spectrum {
    pub fn clear(&mut self) {
        self.samples.fill(0.);
        self.magnitude.fill(0.);
        self.cursor = 0;
        self.observed = false;
    }
    pub fn push(&mut self, mono_pcm: &[f32]) {
        for &sample in mono_pcm {
            self.samples[self.cursor] = if sample.is_finite() { f64::from(sample) } else { 0. };
            self.cursor = (self.cursor + 1) % FFT_SIZE;
            self.observed = true;
        }
    }
    pub fn bins(&mut self) -> Option<Vec<f32>> {
        if !self.observed {
            return None;
        }
        let mut buffer: Vec<Complex> = (0..FFT_SIZE)
            .map(|n| {
                let phase = 2. * PI * n as f64 / FFT_SIZE as f64;
                let blackman = 0.42 - 0.5 * phase.cos() + 0.08 * (2. * phase).cos();
                Complex {
                    real: self.samples[(self.cursor + n) % FFT_SIZE] * blackman,
                    imaginary: 0.,
                }
            })
            .collect();
        let mut reversed = 0;
        for index in 1..FFT_SIZE {
            let mut bit = FFT_SIZE >> 1;
            while reversed & bit != 0 {
                reversed ^= bit;
                bit >>= 1;
            }
            reversed ^= bit;
            if index < reversed {
                buffer.swap(index, reversed);
            }
        }
        let mut length = 2;
        while length <= FFT_SIZE {
            let phase = -2. * PI / length as f64;
            let step = Complex {
                real: phase.cos(),
                imaginary: phase.sin(),
            };
            for start in (0..FFT_SIZE).step_by(length) {
                let mut twiddle = Complex {
                    real: 1.,
                    imaginary: 0.,
                };
                for offset in 0..length / 2 {
                    let even = buffer[start + offset];
                    let odd = buffer[start + offset + length / 2].multiply(twiddle);
                    buffer[start + offset] = even.add(odd);
                    buffer[start + offset + length / 2] = even.subtract(odd);
                    twiddle = twiddle.multiply(step);
                }
            }
            length *= 2;
        }
        Some(
            self.magnitude
                .iter_mut()
                .zip(&buffer)
                .map(|(previous, now)| {
                    let magnitude =
                        (now.real.powi(2) + now.imaginary.powi(2)).sqrt() / FFT_SIZE as f64;
                    *previous = 0.85 * *previous + 0.15 * magnitude;
                    (20. * previous.log10()) as f32
                })
                .collect(),
        )
    }
}

/// Q::U's logarithmic RMS buckets; fractional y is relative to the plot.
pub(super) fn spectrum_points(bins: &[f32], sample_rate: u32, width: f64) -> Vec<(f64, f64)> {
    if bins.is_empty() || sample_rate == 0 || width <= 0. {
        return Vec::new();
    }
    let high = 20000_f64.min(f64::from(sample_rate) / 2.);
    if high <= 20. {
        return Vec::new();
    }
    let count = (width / 3.).floor().max(1.) as usize;
    let start_bin = (20. * bins.len() as f64 * 2. / f64::from(sample_rate)).ceil() as usize;
    let end_bin = (high * bins.len() as f64 * 2. / f64::from(sample_rate)).floor() as usize;
    let mut points = Vec::new();
    for bucket in 0..count {
        let low = 20. * (high / 20.).powf(bucket as f64 / count as f64);
        let upper = 20. * (high / 20.).powf((bucket + 1) as f64 / count as f64);
        let from =
            start_bin.max((low * bins.len() as f64 * 2. / f64::from(sample_rate)).ceil() as usize);
        let to = end_bin
            .min((upper * bins.len() as f64 * 2. / f64::from(sample_rate)).floor() as usize)
            .min(bins.len() - 1);
        if from > to {
            continue;
        }
        let mut power = 0.;
        let mut present = 0;
        for &value in &bins[from..=to] {
            if value.is_finite() {
                power += 10_f64.powf(f64::from(value) / 10.);
                present += 1;
            }
        }
        let db = if present == 0 || power == 0. {
            -130.
        } else {
            10. * (power / present as f64).log10()
        };
        let x = if points.is_empty() {
            0.
        } else {
            ((low * upper).sqrt() / 20.).log10() / (high / 20.).log10()
        };
        points.push((x, 1. - ((db + 130.) / 100.).clamp(0., 1.)));
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    #[core::prelude::v1::test]
    fn seiren_eq_biquad_matches_peak_gain_and_webaudio_pass_q_db() {
        let mut band = Band {
            id: 0,
            frequency: 1000.,
            gain: 6.,
            q: 1.,
            kind: "peaking".into(),
            status: None,
        };
        assert!((response_db(&band, 1000., 48000) - 6.).abs() < 1e-9);
        band.kind = "lowpass".into();
        band.gain = 0.;
        assert!((response_db(&band, 1000., 48000) - 1.).abs() < 1e-9);
        band.kind = "highpass".into();
        assert!((response_db(&band, 1000., 48000) - 1.).abs() < 1e-9);
    }
    #[core::prelude::v1::test]
    fn seiren_eq_fft_uses_real_pcm_blackman_and_magnitude_smoothing() {
        let mut spectrum = Spectrum::default();
        assert!(spectrum.bins().is_none());
        let pcm = (0..FFT_SIZE)
            .map(|n| (2. * PI * 64. * n as f64 / FFT_SIZE as f64).sin() as f32)
            .collect::<Vec<_>>();
        spectrum.push(&pcm);
        let first = spectrum.bins().unwrap();
        assert!((f64::from(first[64]) - 20. * (0.15_f64 * 0.21).log10()).abs() < 1e-5);
        let second = spectrum.bins().unwrap();
        assert!((f64::from(second[64]) - 20. * (0.2775_f64 * 0.21).log10()).abs() < 1e-5);
        spectrum.clear();
        assert!(spectrum.bins().is_none());
    }
    #[core::prelude::v1::test]
    fn seiren_eq_missing_samples_never_generate_a_spectrum() {
        assert!(spectrum_points(&[], 48000, 840.).is_empty());
        let bins = vec![f32::NEG_INFINITY; 4096];
        let points = spectrum_points(&bins, 48000, 840.);
        assert!(!points.is_empty());
        assert!(points.iter().all(|(_, y)| *y == 1.));
    }
    #[core::prelude::v1::test]
    fn seiren_eq_resampling_retains_phase_across_capture_batches() {
        let pcm = (0..1025).map(|n| n as f32 / 1024.).collect::<Vec<_>>();
        let mut whole = Resampler::default();
        let expected = whole.push(&pcm, 96000, 48000);
        let mut split = Resampler::default();
        let mut actual = Vec::new();
        for chunk in pcm.chunks(37) {
            actual.extend(split.push(chunk, 96000, 48000));
        }
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 512);
        assert_eq!(actual[255], pcm[510]);
        assert_eq!(split.push(&pcm, 48000, 48000), pcm);
    }
}
