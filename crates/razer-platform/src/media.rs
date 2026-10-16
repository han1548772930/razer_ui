//! Offline audible media. The retained owner controls a worker; the worker owns
//! native playback, consumes real endpoint progress and releases it on stop.
mod clip;
use anyhow::{Context as _, ensure};
pub use clip::Clip;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
#[cfg(windows)]
mod windows;

#[derive(Clone, Debug, Default)]
pub struct Status {
    cursor: u64,
    playing: bool,
    ended: bool,
    error: Option<String>,
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
}
enum Command {
    Pause,
    Play,
    Seek(u64),
}
trait Output {
    fn capacity(&self) -> u32;
    fn padding(&self) -> anyhow::Result<u32>;
    fn write(&mut self, pcm: &[u8], frames: u32) -> anyhow::Result<()>;
    fn start(&mut self) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
    fn reset(&mut self) -> anyhow::Result<()>;
}
struct Playback {
    clip: Arc<Clip>,
    base: u64,
    written: u64,
    started: bool,
    wants_play: bool,
    status: Status,
}
impl Playback {
    fn new(clip: Arc<Clip>) -> Self {
        Self {
            clip,
            base: 0,
            written: 0,
            started: false,
            wants_play: true,
            status: Status::default(),
        }
    }
    fn observe(&mut self, output: &dyn Output) -> anyhow::Result<u32> {
        let padding = output.padding()?;
        ensure!(
            padding <= output.capacity() && u64::from(padding) <= self.written,
            "Playback queue observation differs"
        );
        self.status.cursor = self.base + self.written - u64::from(padding);
        Ok(padding)
    }
    fn command(&mut self, command: Command, output: &mut dyn Output) -> anyhow::Result<()> {
        match command {
            Command::Pause => {
                output.stop()?;
                self.started = false;
                self.wants_play = false;
                self.observe(output)?;
                self.status.playing = false;
            }
            Command::Play => {
                if self.status.ended {
                    self.command(Command::Seek(0), output)?;
                }
                self.wants_play = true;
            }
            Command::Seek(cursor) => {
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
            }
        }
        Ok(())
    }
    fn tick(&mut self, output: &mut dyn Output) -> anyhow::Result<()> {
        let padding = self.observe(output)?;
        if !self.wants_play {
            return Ok(());
        }
        let remaining = self.clip.sample_count() - self.base - self.written;
        let available = output.capacity() - padding;
        if remaining > 0 && available > 0 {
            let count = remaining.min(u64::from(available)) as u32;
            output.write(self.clip.pcm(self.base + self.written, count)?, count)?;
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
    worker: Option<JoinHandle<anyhow::Result<()>>>,
}
impl Session {
    pub fn start(clip: Arc<Clip>) -> anyhow::Result<Self> {
        #[cfg(not(windows))]
        {
            let _ = clip;
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
                        let mut playback = Playback::new(clip);
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
                        let cleanup = output.stop().and_then(|_| output.reset());
                        result.and(cleanup)
                    })();
                    if let Ok(mut state) = result_status.lock() {
                        state.playing = false;
                        if let Err(error) = &result {
                            state.error = Some(format!("{error:#}"));
                        }
                    }
                    result
                })?;
            Ok(Self {
                commands,
                status,
                cancel,
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
    pub fn seek(&self, frame: u64) -> anyhow::Result<()> {
        self.send(Command::Seek(frame))
    }
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.cancel.store(true, Ordering::Release);
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
        if let Err(error) = self.stop() {
            eprintln!("Media cleanup: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests;
