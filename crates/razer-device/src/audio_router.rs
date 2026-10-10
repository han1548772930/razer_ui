//! Current RzAudioUtil TLWinCoreAudioRouter byte FIFO and clock-drift correction.
//! IDA RVAs 0x47000, 0x47640, 0x48020, 0x48570, 0x48910, 0x48d50,
//! 0x48f00 and 0x490d0. This module performs no OS operations.
use anyhow::{Result, ensure};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub block_align: u16,
}

pub struct AudioFifo {
    bytes: VecDeque<u8>,
    capacity: usize,
    format: StreamFormat,
    period_100ns: u64,
    silence_frames: usize,
    history: VecDeque<u32>,
    history_capacity: usize,
    history_sum: u64,
    correction: i32,
    audible: bool,
}

impl AudioFifo {
    pub fn new(format: StreamFormat, capture_period: u64, render_period: u64) -> Result<Self> {
        ensure!(
            format.sample_rate != 0 && format.block_align != 0,
            "AudioRouter mix format has zero rate/alignment"
        );
        ensure!(
            capture_period != 0 && render_period != 0,
            "AudioRouter default device period is zero"
        );
        let period = capture_period.max(render_period);
        // Source constructor percentages: FIFO 600%, silence 300%, drift
        // band 240..360%, 50 ms warm-up, 5/1000 correction rate, 2000 ms history.
        let frames = (u64::from(format.sample_rate) * (period * 600 / 100) / 10_000_000) as usize;
        let silence_frames =
            (u64::from(format.sample_rate) * (period * 300 / 100) / 10_000_000) as usize;
        let history_capacity = (10_000u64 * 2000 / render_period) as usize;
        ensure!(
            frames != 0 && history_capacity != 0,
            "AudioRouter FIFO/history would be empty"
        );
        let capacity = frames
            .checked_mul(usize::from(format.block_align))
            .ok_or_else(|| anyhow::anyhow!("AudioRouter FIFO size overflow"))?;
        let mut fifo = Self {
            bytes: VecDeque::with_capacity(capacity),
            capacity,
            format,
            period_100ns: period,
            silence_frames,
            history: VecDeque::with_capacity(history_capacity),
            history_capacity,
            history_sum: 0,
            correction: 0,
            audible: false,
        };
        fifo.pad_silence(silence_frames * usize::from(format.block_align));
        Ok(fifo)
    }

    fn pad_silence(&mut self, requested: usize) {
        let count = requested.min(self.capacity - self.bytes.len());
        self.bytes.extend(std::iter::repeat_n(0, count));
    }

    fn discard(&mut self, count: usize) {
        self.bytes.drain(..count.min(self.bytes.len()));
    }

    pub fn capture(&mut self, data: &[u8]) -> Result<()> {
        ensure!(
            data.len() % usize::from(self.format.block_align) == 0,
            "AudioRouter capture packet is not frame-aligned"
        );
        if self.capacity - self.bytes.len() < data.len() {
            // Original overflow branch removes exactly half the FIFO capacity.
            self.discard(
                self.capacity / usize::from(self.format.block_align) / 2
                    * usize::from(self.format.block_align),
            );
        }
        let count = data.len().min(self.capacity - self.bytes.len());
        self.bytes.extend(data[..count].iter().copied());
        if !self.audible {
            // Capture is consumed during the initial 50 ms render warm-up.
            self.discard(data.len());
        }
        Ok(())
    }

    pub fn render(&mut self, output_frames: u32, elapsed_ms: u64) -> Result<Vec<u8>> {
        let align = usize::from(self.format.block_align);
        let output_frames = output_frames as usize;
        let bytes = output_frames
            .checked_mul(align)
            .ok_or_else(|| anyhow::anyhow!("AudioRouter render size overflow"))?;
        if !self.audible && elapsed_ms >= 50 {
            self.audible = true;
        }
        if !self.audible {
            return Ok(vec![0; bytes]);
        }
        let observed = (self.correction as u32).wrapping_add((self.bytes.len() / align) as u32);
        if self.history.len() == self.history_capacity {
            self.history_sum -= u64::from(self.history.pop_front().unwrap());
        }
        self.history.push_back(observed);
        self.history_sum += u64::from(observed);
        if self.history.len() == self.history_capacity && self.correction == 0 {
            let average =
                (self.history_sum + (self.history.len() as u64 >> 1)) / self.history.len() as u64;
            let rate = u64::from(self.format.sample_rate);
            let low = rate * (self.period_100ns * 240 / 100) / 10_000_000;
            let high = rate * (self.period_100ns * 360 / 100) / 10_000_000;
            let adjustment = (rate * (self.period_100ns / 10_000) / 1000) as i32;
            if average > high {
                self.correction = -adjustment;
                self.clear_history();
            }
            if average < low {
                self.correction = self.correction.wrapping_add(adjustment);
                self.clear_history();
            }
        }
        let per_packet =
            (5 * (u64::from(self.format.sample_rate) * self.period_100ns / 10_000_000) / 1000)
                .max(1);
        let amount = (u64::from(self.correction.unsigned_abs())).min(per_packet) as usize;
        let input_frames = if self.correction > 0 {
            self.correction -= amount as i32;
            output_frames
                .checked_sub(amount)
                .ok_or_else(|| anyhow::anyhow!("AudioRouter drift would underflow output frames"))?
        } else if self.correction < 0 {
            self.correction += amount as i32;
            output_frames
                .checked_add(amount)
                .ok_or_else(|| anyhow::anyhow!("AudioRouter drift size overflow"))?
        } else {
            output_frames
        };
        if self.bytes.len() < input_frames * align {
            self.pad_silence(
                self.silence_frames
                    .saturating_mul(align)
                    .saturating_sub(self.bytes.len()),
            );
        }
        // Source copies min(in,out) frames, then skips excess input or repeats
        // the final output frame. It does not resample numerical PCM samples.
        let copy_bytes = input_frames.min(output_frames) * align;
        ensure!(
            self.bytes.len() >= copy_bytes,
            "AudioRouter source FIFO underrun exceeds silence reserve"
        );
        let mut output: Vec<u8> = self.bytes.drain(..copy_bytes).collect();
        if input_frames > output_frames {
            self.discard((input_frames - output_frames) * align);
        } else if input_frames < output_frames {
            ensure!(
                input_frames != 0,
                "AudioRouter cannot repeat a nonexistent last frame"
            );
            let last_frame = output[output.len() - align..].to_vec();
            for _ in input_frames..output_frames {
                output.extend_from_slice(&last_frame);
            }
        }
        Ok(output)
    }

    fn clear_history(&mut self) {
        self.history.clear();
        self.history_sum = 0;
    }
}
