//! Offline audible media. The retained owner controls a worker; the worker owns
//! native playback, consumes real endpoint progress and releases it on stop.
mod clip;
mod tempo;
use anyhow::{Context as _, ensure};
pub use clip::Clip;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[cfg(windows)]
mod windows;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaybackRate {
    Quarter,
    Half,
    #[default]
    Normal,
    FiveQuarters,
    ThreeHalves,
    Double,
}
impl PlaybackRate {
    fn ratio(self) -> (u64, u64) {
        match self {
            Self::Quarter => (1, 4),
            Self::Half => (1, 2),
            Self::Normal => (1, 1),
            Self::FiveQuarters => (5, 4),
            Self::ThreeHalves => (3, 2),
            Self::Double => (2, 1),
        }
    }
    fn source_frames(self, frames: u64) -> u64 {
        let (numerator, denominator) = self.ratio();
        frames * numerator / denominator
    }
    fn output_frames(self, frames: u64) -> u64 {
        let (numerator, denominator) = self.ratio();
        (frames * denominator).div_ceil(numerator)
    }
    pub fn faster(self) -> Self {
        match self {
            Self::Quarter => Self::Half,
            Self::Half => Self::Normal,
            Self::Normal => Self::FiveQuarters,
            Self::FiveQuarters => Self::ThreeHalves,
            Self::ThreeHalves | Self::Double => Self::Double,
        }
    }
    pub fn slower(self) -> Self {
        match self {
            Self::Quarter | Self::Half => Self::Quarter,
            Self::Normal => Self::Half,
            Self::FiveQuarters => Self::Normal,
            Self::ThreeHalves => Self::FiveQuarters,
            Self::Double => Self::ThreeHalves,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Status {
    cursor: u64,
    playing: bool,
    ended: bool,
    error: Option<String>,
    seek_revision: u64,
    volume: f32,
    rate: PlaybackRate,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            cursor: 0,
            playing: false,
            ended: false,
            error: None,
            seek_revision: 0,
            volume: 1.,
            rate: PlaybackRate::Normal,
        }
    }
}
impl Status {
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn is_ended(&self) -> bool {
        self.ended
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn seek_revision(&self) -> u64 {
        self.seek_revision
    }
    pub fn volume(&self) -> f32 {
        self.volume
    }
    pub fn playback_rate(&self) -> PlaybackRate {
        self.rate
    }
}
enum Command {
    Pause,
    Play,
    Toggle,
    Seek { cursor: u64, revision: u64 },
    Volume(f32),
    Rate(PlaybackRate),
}
trait Output {
    fn capacity(&self) -> u32;
    fn padding(&self) -> anyhow::Result<u32>;
    fn write(&mut self, pcm: &[u8], frames: u32) -> anyhow::Result<()>;
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
    fn reset(&mut self) -> anyhow::Result<()>;
    fn set_volume(&mut self, volume: f32) -> anyhow::Result<()>;
}
struct Playback {
    clip: Arc<Clip>,
    base: u64,
    written: u64,
    started: bool,
    wants_play: bool,
    status: Status,
    tempo: Option<tempo::Tempo>,
}
impl Playback {
    fn new(clip: Arc<Clip>, autoplay: bool) -> Self {
        Self {
            clip,
            base: 0,
            written: 0,
            started: false,
            wants_play: autoplay,
            status: Status::default(),
            tempo: None,
        }
    }
    fn observe(&mut self, output: &dyn Output) -> anyhow::Result<u32> {
        let padding = output.padding()?;
        ensure!(
            padding <= output.capacity() && u64::from(padding) <= self.written,
            "Playback queue observation differs"
        );
        self.status.cursor = (self.base
            + self
                .status
                .rate
                .source_frames(self.written - u64::from(padding)))
        .min(self.clip.sample_count());
        Ok(padding)
    }
    fn command(&mut self, command: Command, output: &mut dyn Output) -> anyhow::Result<()> {
        match command {
            Command::Toggle => {
                self.command(
                    if self.wants_play {
                        Command::Pause
                    } else {
                        Command::Play
                    },
                    output,
                )?;
            }
            Command::Pause => {
                output.stop()?;
                self.started = false;
                self.wants_play = false;
                self.observe(output)?;
                self.status.playing = false;
            }
            Command::Play => {
                if self.status.ended {
                    self.command(
                        Command::Seek {
                            cursor: 0,
                            revision: self.status.seek_revision,
                        },
                        output,
                    )?;
                }
                self.wants_play = true;
            }
            Command::Seek { cursor, revision } => {
                ensure!(
                    cursor <= self.clip.sample_count(),
                    "Playback seek exceeds duration"
                );
                output.stop()?;
                self.started = false;
                output.reset()?;
                self.base = cursor;
                self.written = 0;
                self.status.cursor = cursor;
                self.status.playing = false;
                self.status.ended = cursor == self.clip.sample_count();
                self.status.seek_revision = revision;
                self.tempo = (self.status.rate != PlaybackRate::Normal)
                    .then(|| tempo::Tempo::new(self.clip.clone(), cursor, self.status.rate));
            }
            Command::Volume(volume) => {
                ensure!(
                    volume.is_finite() && (0.0..=1.0).contains(&volume),
                    "Media volume exceeds range"
                );
                output.set_volume(volume)?;
                self.status.volume = volume;
            }
            Command::Rate(rate) => {
                if self.status.rate != rate {
                    self.observe(output)?;
                    let cursor = self.status.cursor;
                    output.stop()?;
                    self.started = false;
                    output.reset()?;
                    self.base = cursor;
                    self.written = 0;
                    self.status.rate = rate;
                    self.status.playing = false;
                    self.tempo = (rate != PlaybackRate::Normal)
                        .then(|| tempo::Tempo::new(self.clip.clone(), cursor, rate));
                }
            }
        }
        Ok(())
    }
    fn tick(&mut self, output: &mut dyn Output) -> anyhow::Result<()> {
        let padding = self.observe(output)?;
        if !self.wants_play {
            return Ok(());
        }
        let remaining = self
            .status
            .rate
            .output_frames(self.clip.sample_count() - self.base)
            - self.written;
        let available = output.capacity() - padding;
        if remaining > 0 && available > 0 {
            let count = remaining.min(u64::from(available)) as u32;
            if let Some(tempo) = &mut self.tempo {
                output.write(&tempo.read(count), count)?;
            } else {
                output.write(self.clip.pcm(self.base + self.written, count)?, count)?;
            }
            self.written += u64::from(count);
        }
        if remaining == 0 && padding == 0 {
            output.stop()?;
            self.started = false;
            self.wants_play = false;
            self.status.playing = false;
            self.status.ended = true;
        } else if !self.started {
            output.start()?;
            self.started = true;
            self.status.playing = true;
            self.status.ended = false;
        }
        Ok(())
    }
}

pub struct Session {
    commands: mpsc::Sender<Command>,
    status: Arc<Mutex<Status>>,
    cancel: Arc<AtomicBool>,
    seek_revision: AtomicU64,
    worker: Option<JoinHandle<anyhow::Result<()>>>,
}
impl Session {
    pub fn start(clip: Arc<Clip>) -> anyhow::Result<Self> {
        Self::open(clip, true)
    }
    pub fn prepare(clip: Arc<Clip>) -> anyhow::Result<Self> {
        Self::open(clip, false)
    }
    fn open(clip: Arc<Clip>, autoplay: bool) -> anyhow::Result<Self> {
        #[cfg(not(windows))]
        {
            let _ = (clip, autoplay);
            anyhow::bail!("Native audible media output is available on Windows");
        }
        #[cfg(windows)]
        {
            let (commands, incoming) = mpsc::channel();
            let status = Arc::new(Mutex::new(Status::default()));
            let cancel = Arc::new(AtomicBool::new(false));
            let result_status = status.clone();
            let signal = cancel.clone();
            let worker = thread::Builder::new()
                .name("local-audible-media".into())
                .spawn(move || {
                    let result = (|| {
                        if signal.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        let mut output = windows::Endpoint::open(&clip)?;
                        let mut playback = Playback::new(clip, autoplay);
                        let result = (|| {
                            while !signal.load(Ordering::Acquire) {
                                for command in incoming.try_iter() {
                                    playback.command(command, &mut output)?;
                                }
                                if signal.load(Ordering::Acquire) {
                                    break;
                                }
                                playback.tick(&mut output)?;
                                *result_status
                                    .lock()
                                    .map_err(|_| anyhow::anyhow!("Media state poisoned"))? =
                                    playback.status.clone();
                                thread::park_timeout(Duration::from_millis(5));
                            }
                            Ok(())
                        })();
                        let stop = output.stop();
                        let reset = output.reset();
                        result.and(stop).and(reset)
                    })();
                    if let Ok(mut state) = result_status.lock() {
                        state.playing = false;
                        if let Err(error) = &result {
                            state.error = Some(format!("{error:#}"));
                        }
                    }
                    if let Err(error) = &result {
                        eprintln!("Media output: {error:#}");
                    }
                    result
                })?;
            Ok(Self {
                commands,
                status,
                cancel,
                seek_revision: AtomicU64::new(0),
                worker: Some(worker),
            })
        }
    }
    pub fn status(&self) -> anyhow::Result<Status> {
        Ok(self
            .status
            .lock()
            .map_err(|_| anyhow::anyhow!("Media state poisoned"))?
            .clone())
    }
    fn send(&self, command: Command) -> anyhow::Result<()> {
        ensure!(!self.cancel.load(Ordering::Acquire), "Media owner stopped");
        self.commands
            .send(command)
            .context("Media output worker stopped")?;
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
        Ok(())
    }
    pub fn pause(&self) -> anyhow::Result<()> {
        self.send(Command::Pause)
    }
    pub fn play(&self) -> anyhow::Result<()> {
        self.send(Command::Play)
    }
    pub fn toggle(&self) -> anyhow::Result<()> {
        self.send(Command::Toggle)
    }
    pub fn seek(&self, frame: u64) -> anyhow::Result<u64> {
        let revision = self.seek_revision.fetch_add(1, Ordering::Relaxed) + 1;
        self.send(Command::Seek {
            cursor: frame,
            revision,
        })?;
        Ok(revision)
    }
    pub fn set_volume(&self, volume: f32) -> anyhow::Result<()> {
        ensure!(
            volume.is_finite() && (0.0..=1.0).contains(&volume),
            "Media volume exceeds range"
        );
        self.send(Command::Volume(volume))
    }
    pub fn set_playback_rate(&self, rate: PlaybackRate) -> anyhow::Result<()> {
        self.send(Command::Rate(rate))
    }
    /// Signal the output owner without waiting for COM or endpoint cleanup.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Media output worker panicked"))??;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // The worker owns the endpoint and performs stop/reset even when the
        // view disappears. Joining is reserved for explicit background cleanup.
        self.cancel();
    }
}

#[cfg(test)]
mod tests;
