//! Bounded offline video resource: timestamped JPEG frames and PCM16 samples.
//! Original acquisition hashes remain analysis evidence, never runtime gates.
use anyhow::{Context as _, ensure};
use std::sync::Arc;

#[derive(Clone)]
struct Frame {
    timestamp: u64,
    start: usize,
    end: usize,
}
pub struct Clip {
    bytes: Arc<[u8]>,
    frames: Vec<Frame>,
    width: u32,
    height: u32,
    rate: u32,
    channels: u16,
    channel_mask: u32,
    samples: u64,
    pcm_offset: usize,
}
impl Clip {
    pub fn parse(bytes: Arc<[u8]>) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() >= 64 && bytes.len() <= 512 * 1024 * 1024,
            "Local media resource size differs"
        );
        ensure!(&bytes[..8] == b"RZAV0001", "Local media format differs");
        let u32_at = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let u64_at = |at| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        let width = u32_at(12);
        let height = u32_at(16);
        let rate = u32_at(28);
        let channels = u16::from_le_bytes(bytes[32..34].try_into().unwrap());
        let bits = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
        let channel_mask = u32_at(36);
        let samples = u64_at(40);
        let count = u32_at(48) as usize;
        // Header fields are intentionally decoded at their wire offsets. The
        // index follows immediately; reserved bytes must stay zero.
        ensure!(
            u32_at(8) == 64 && u32_at(52) == 0,
            "Local media header differs"
        );
        ensure!(
            (1..=8192).contains(&width)
                && (1..=8192).contains(&height)
                && u32_at(20) > 0
                && u32_at(24) > 0,
            "Local video geometry differs"
        );
        ensure!(
            (8000..=192000).contains(&rate) && (1..=32).contains(&channels) && bits == 16,
            "Local PCM format differs"
        );
        ensure!(
            samples > 0 && samples <= u64::from(rate) * 600 && count > 0 && count <= 100000,
            "Local media duration exceeds limit"
        );
        ensure!(
            channel_mask == 0 || channel_mask.count_ones() == u32::from(channels),
            "Local speaker mask differs"
        );
        let pcm_offset = usize::try_from(u64_at(56))?;
        let index_end = 64 + count * 24;
        let pcm_bytes = usize::try_from(samples)?
            .checked_mul(usize::from(channels) * 2)
            .context("PCM length overflow")?;
        ensure!(
            index_end <= pcm_offset && pcm_offset.checked_add(pcm_bytes) == Some(bytes.len()),
            "Local media track bounds differ"
        );
        let mut frames = Vec::with_capacity(count);
        let mut end = index_end;
        for ix in 0..count {
            let at = 64 + ix * 24;
            let timestamp = u64_at(at);
            let start = usize::try_from(u64_at(at + 8))?;
            let length = u32_at(at + 16) as usize;
            ensure!(
                u32_at(at + 20) == 0 && start == end && length >= 4,
                "Local video index differs"
            );
            end = start
                .checked_add(length)
                .context("Local JPEG length overflow")?;
            ensure!(
                end <= pcm_offset && timestamp < samples,
                "Local video frame exceeds its track"
            );
            ensure!(
                if ix == 0 {
                    timestamp == 0
                } else {
                    timestamp > frames.last().map(|frame: &Frame| frame.timestamp).unwrap()
                },
                "Local video timestamps differ"
            );
            ensure!(
                &bytes[start..start + 2] == b"\xff\xd8" && &bytes[end - 2..end] == b"\xff\xd9",
                "Local JPEG framing differs"
            );
            frames.push(Frame {
                timestamp,
                start,
                end,
            });
        }
        ensure!(
            end == pcm_offset,
            "Local media has an unindexed video region"
        );
        Ok(Self {
            bytes,
            frames,
            width,
            height,
            rate,
            channels,
            channel_mask,
            samples,
            pcm_offset,
        })
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn channel_mask(&self) -> u32 {
        self.channel_mask
    }
    pub fn sample_count(&self) -> u64 {
        self.samples
    }
    pub fn duration(&self) -> f64 {
        self.samples as f64 / f64::from(self.rate)
    }
    pub fn frame_index(&self, cursor: u64) -> usize {
        self.frames
            .partition_point(|frame| frame.timestamp <= cursor)
            .saturating_sub(1)
    }
    pub fn jpeg(&self, ix: usize) -> Option<&[u8]> {
        self.frames
            .get(ix)
            .map(|frame| &self.bytes[frame.start..frame.end])
    }
    pub(super) fn pcm(&self, start: u64, count: u32) -> anyhow::Result<&[u8]> {
        ensure!(
            start <= self.samples && u64::from(count) <= self.samples - start,
            "PCM request exceeds track"
        );
        let align = usize::from(self.channels) * 2;
        let at = self.pcm_offset + usize::try_from(start)? * align;
        Ok(&self.bytes[at..at + count as usize * align])
    }
    pub(super) fn sample(&self, frame: u64, channel: usize) -> Option<i16> {
        if frame >= self.samples || channel >= usize::from(self.channels) {
            return None;
        }
        let at = self.pcm_offset + (frame as usize * usize::from(self.channels) + channel) * 2;
        Some(i16::from_le_bytes([self.bytes[at], self.bytes[at + 1]]))
    }
}
