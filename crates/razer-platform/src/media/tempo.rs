//! Bounded multichannel time stretching for the source player's pitch-preserving
//! playback rates. A single waveform alignment is shared by every channel.
use super::{Clip, PlaybackRate};
use std::sync::Arc;

pub(super) struct Tempo {
    clip: Arc<Clip>,
    base: u64,
    rate: PlaybackRate,
    hop: usize,
    tail: Vec<i16>,
    pending: Vec<i16>,
    pending_frame: usize,
    generated: u64,
    emitted: u64,
    total: u64,
}

impl Tempo {
    pub(super) fn new(clip: Arc<Clip>, base: u64, rate: PlaybackRate) -> Self {
        let total = rate.output_frames(clip.sample_count() - base);
        Self {
            hop: (clip.sample_rate() as usize / 100).max(1),
            clip,
            base,
            rate,
            tail: Vec::new(),
            pending: Vec::new(),
            pending_frame: 0,
            generated: 0,
            emitted: 0,
            total,
        }
    }

    pub(super) fn remaining(&self) -> u64 {
        self.total - self.emitted
    }

    fn sample(&self, frame: u64, channel: usize) -> i16 {
        self.clip.sample(frame, channel).unwrap_or(0)
    }

    fn alignment(&self, nominal: u64) -> u64 {
        let end = self
            .clip
            .sample_count()
            .saturating_sub((2 * self.hop) as u64)
            .max(self.base);
        let nominal = nominal.min(end).max(self.base);
        let radius = (self.hop / 2) as u64;
        let left = nominal.saturating_sub(radius).max(self.base);
        let right = nominal.saturating_add(radius).min(end);
        let channels = usize::from(self.clip.channels());
        let energy = self
            .tail
            .iter()
            .step_by(8)
            .map(|v| f64::from(*v).powi(2))
            .sum::<f64>();
        if energy < 1. {
            return nominal;
        }
        let score = |start: u64| {
            let mut dot = 0.;
            let mut reference_energy = 0.;
            let mut candidate_energy = 0.;
            for frame in (0..self.hop).step_by(8) {
                for channel in 0..channels {
                    let a = f64::from(self.tail[frame * channels + channel]);
                    let b = f64::from(self.sample(start + frame as u64, channel));
                    dot += a * b;
                    reference_energy += a * a;
                    candidate_energy += b * b;
                }
            }
            let denominator = (reference_energy * candidate_energy).sqrt();
            if denominator < 1. {
                return -1.;
            }
            dot / denominator - 0.00001 * start.abs_diff(nominal) as f64 / self.hop as f64
        };
        let mut best = nominal;
        let mut best_score = score(nominal);
        for candidate in (left..=right).step_by(4) {
            let candidate_score = score(candidate);
            if candidate_score > best_score {
                best = candidate;
                best_score = candidate_score;
            }
        }
        for candidate in best.saturating_sub(4).max(left)..=best.saturating_add(4).min(right) {
            let candidate_score = score(candidate);
            if candidate_score > best_score {
                best = candidate;
                best_score = candidate_score;
            }
        }
        best
    }

    fn generate(&mut self) {
        let channels = usize::from(self.clip.channels());
        let nominal = self.base + self.rate.source_frames(self.generated);
        let start = if self.tail.is_empty() {
            self.base
        } else {
            self.alignment(nominal)
        };
        let count = self.hop.min((self.total - self.generated) as usize);
        self.pending.clear();
        self.pending.reserve(count * channels);
        for frame in 0..count {
            for channel in 0..channels {
                let next = self.sample(start + frame as u64, channel);
                let value = if self.tail.is_empty() {
                    next
                } else {
                    let previous = i64::from(self.tail[frame * channels + channel]);
                    let numerator =
                        previous * (self.hop - frame) as i64 + i64::from(next) * frame as i64;
                    (numerator / self.hop as i64) as i16
                };
                self.pending.push(value);
            }
        }
        self.tail = (0..self.hop)
            .flat_map(|frame| (0..channels).map(move |channel| (frame, channel)))
            .map(|(frame, channel)| self.sample(start + (self.hop + frame) as u64, channel))
            .collect();
        self.pending_frame = 0;
        self.generated += count as u64;
    }

    pub(super) fn read(&mut self, count: u32) -> Vec<u8> {
        let count = u64::from(count).min(self.remaining()) as usize;
        let channels = usize::from(self.clip.channels());
        let mut bytes = Vec::with_capacity(count * channels * 2);
        let mut remaining = count;
        while remaining > 0 {
            if self.pending_frame == self.pending.len() / channels {
                self.generate();
            }
            let frames = remaining.min(self.pending.len() / channels - self.pending_frame);
            for sample in &self.pending
                [self.pending_frame * channels..(self.pending_frame + frames) * channels]
            {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            self.pending_frame += frames;
            remaining -= frames;
        }
        self.emitted += count as u64;
        bytes
    }
}
