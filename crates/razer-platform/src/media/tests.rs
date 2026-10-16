use super::*;

fn resource(samples: u64, channels: u16) -> Vec<u8> {
    let pcm_offset = 64 + 2 * 24 + 8;
    let mut bytes = vec![0; pcm_offset + samples as usize * channels as usize * 2];
    bytes[..8].copy_from_slice(b"RZAV0001");
    for (offset, value) in [(8,64u32),(12,2),(16,2),(20,25),(24,1),(28,8000),(48,2)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[32..34].copy_from_slice(&channels.to_le_bytes());
    bytes[34..36].copy_from_slice(&16u16.to_le_bytes());
    bytes[40..48].copy_from_slice(&samples.to_le_bytes());
    bytes[56..64].copy_from_slice(&(pcm_offset as u64).to_le_bytes());
    for (ix, timestamp) in [0, samples / 2].into_iter().enumerate() {
        let at = 64 + ix * 24;
        let start = 112 + ix * 4;
        bytes[at..at + 8].copy_from_slice(&timestamp.to_le_bytes());
        bytes[at + 8..at + 16].copy_from_slice(&(start as u64).to_le_bytes());
        bytes[at + 16..at + 20].copy_from_slice(&4u32.to_le_bytes());
        bytes[start..start + 4].copy_from_slice(&[255, 216, 255, 217]);
    }
    for (ix, value) in bytes[pcm_offset..].iter_mut().enumerate() { *value = ix as u8; }
    bytes
}

#[derive(Default)]
struct SimulatedOutput {
    queued: u32,
    started: bool,
    starts: u32,
    resets: u32,
    packets: Vec<Vec<u8>>,
}
impl Output for SimulatedOutput {
    fn capacity(&self) -> u32 { 4 }
    fn padding(&self) -> anyhow::Result<u32> { Ok(self.queued) }
    fn write(&mut self, pcm: &[u8], frames: u32) -> anyhow::Result<()> {
        ensure!(frames <= 4 - self.queued, "Simulated queue overflow");
        self.queued += frames;
        self.packets.push(pcm.to_vec());
        Ok(())
    }
    fn start(&mut self) -> anyhow::Result<()> { self.started = true; self.starts += 1; Ok(()) }
    fn stop(&mut self) -> anyhow::Result<()> { self.started = false; Ok(()) }
    fn reset(&mut self) -> anyhow::Result<()> { self.queued = 0; self.resets += 1; Ok(()) }
}

#[test]
fn media_clock_tracks_consumed_frames_and_preserves_pause_buffer() {
    let clip = Arc::new(Clip::parse(resource(10, 2).into()).unwrap());
    let mut playback = Playback::new(clip);
    let mut output = SimulatedOutput::default();
    playback.tick(&mut output).unwrap();
    assert_eq!(playback.status.cursor(), 0);
    assert!(playback.status.is_playing());
    output.queued = 2;
    playback.tick(&mut output).unwrap();
    assert_eq!(playback.status.cursor(), 2);
    playback.command(Command::Pause, &mut output).unwrap();
    playback.tick(&mut output).unwrap();
    assert!(!playback.status.is_playing());
    assert_eq!(output.queued, 4);
    assert_eq!(output.resets, 0);
    assert_eq!(output.packets.len(), 2);
    playback.command(Command::Play, &mut output).unwrap();
    playback.tick(&mut output).unwrap();
    assert!(output.started);
    assert_eq!(output.starts, 2);
    assert_eq!(playback.status.cursor(), 2);
}

#[test]
fn media_seek_flushes_old_audio_and_end_waits_for_real_queue_drain() {
    let clip = Arc::new(Clip::parse(resource(10, 2).into()).unwrap());
    let expected = clip.pcm(8, 2).unwrap().to_vec();
    let mut playback = Playback::new(clip);
    let mut output = SimulatedOutput::default();
    playback.tick(&mut output).unwrap();
    playback.command(Command::Seek(8), &mut output).unwrap();
    assert_eq!(output.queued, 0);
    assert_eq!(output.resets, 1);
    playback.tick(&mut output).unwrap();
    assert_eq!(output.packets.last().unwrap(), &expected);
    assert!(!playback.status.is_ended());
    playback.tick(&mut output).unwrap();
    assert_eq!(playback.status.cursor(), 8);
    output.queued = 0;
    playback.tick(&mut output).unwrap();
    assert_eq!(playback.status.cursor(), 10);
    assert!(playback.status.is_ended());
    assert!(!output.started);
    playback.command(Command::Play, &mut output).unwrap();
    playback.tick(&mut output).unwrap();
    assert_eq!(playback.status.cursor(), 0);
    assert!(playback.status.is_playing());
    assert_eq!(output.resets, 2);
}

#[test]
fn media_rejects_invalid_endpoint_observation_and_out_of_range_seek() {
    let clip = Arc::new(Clip::parse(resource(10, 2).into()).unwrap());
    let mut playback = Playback::new(clip);
    let mut output = SimulatedOutput::default();
    output.queued = 1;
    assert!(playback.tick(&mut output).is_err());
    assert!(playback.command(Command::Seek(11), &mut output).is_err());
    assert_eq!(output.resets, 0);
}

#[test]
fn media_clip_preserves_discrete_channels_and_timestamp_boundaries() {
    let bytes = resource(10, 8);
    let clip = Clip::parse(bytes.clone().into()).unwrap();
    assert_eq!((clip.channels(),clip.channel_mask(),clip.sample_rate()), (8,0,8000));
    assert_eq!((clip.width(),clip.height(),clip.sample_count()), (2,2,10));
    assert_eq!(clip.pcm(0,10).unwrap(), &bytes[120..]);
    assert_eq!((clip.frame_index(0),clip.frame_index(4),clip.frame_index(5),clip.frame_index(10)), (0,0,1,1));
    assert!(clip.pcm(10,1).is_err());
    for offset in [8, 34, 52, 56, 80, 84, 104, 112, 119] {
        let mut damaged = bytes.clone(); damaged[offset] ^= 1;
        assert!(Clip::parse(damaged.into()).is_err(), "accepted damaged field {offset}");
    }
    let mut repeated_timestamp = bytes.clone();
    repeated_timestamp[88..96].fill(0);
    assert!(Clip::parse(repeated_timestamp.into()).is_err());
    assert!(Clip::parse(bytes[..bytes.len()-1].to_vec().into()).is_err());
}
