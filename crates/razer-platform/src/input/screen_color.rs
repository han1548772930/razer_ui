//! Local screen sampling from the current host and Studio native picker chains.
//! The owner only exchanges typed memory events; native I/O and cleanup stay on
//! its worker. A completed sample is distinct from an explicitly canceled one.
use anyhow::{Context as _, ensure};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScreenColorEvent {
    /// The source event's 0xRRGGBB value, not Windows' COLORREF byte order.
    Color(u32),
    /// The native window and sampling resources have actually closed.
    Done,
    Canceled,
    Failed(String),
}

#[derive(Default)]
struct Events {
    queue: VecDeque<ScreenColorEvent>,
    finished: bool,
}

impl Events {
    fn push(&mut self, event: ScreenColorEvent) {
        if self.finished {
            return;
        }
        if matches!(event, ScreenColorEvent::Color(_)) {
            if matches!(self.queue.back(), Some(ScreenColorEvent::Color(_))) {
                self.queue.pop_back();
            }
        } else {
            self.finished = true;
        }
        self.queue.push_back(event);
    }
}

pub(crate) struct Worker {
    canceled: Arc<AtomicBool>,
    events: Arc<Mutex<Events>>,
}

impl Worker {
    pub(crate) fn canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }

    pub(crate) fn color(&self, value: u32) -> anyhow::Result<()> {
        ensure!(value <= 0x00ff_ffff, "Screen sample exceeds RGB24");
        if !self.canceled() {
            self.events
                .lock()
                .map_err(|_| anyhow::anyhow!("Screen color event buffer was poisoned"))?
                .push(ScreenColorEvent::Color(value));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Completion {
    Done,
    Canceled,
}

static CURRENT: Mutex<Option<Weak<AtomicBool>>> = Mutex::new(None);
static NATIVE_OWNER: Mutex<()> = Mutex::new(());

pub struct ScreenColorPicker {
    canceled: Arc<AtomicBool>,
    events: Arc<Mutex<Events>>,
}

impl ScreenColorPicker {
    /// Spawns the worker without waiting for a window, disk, cursor decoding,
    /// or screen I/O. `cursor` is the requested local resource cache payload.
    pub fn start(cursor: Vec<u8>) -> anyhow::Result<Self> {
        #[cfg(windows)]
        return Self::spawn(move |worker| {
            crate::platform::windows::screen_color::run(&cursor, worker)
        });
        #[cfg(not(windows))]
        {
            let _ = cursor;
            anyhow::bail!("Screen color sampling requires the Windows native adapter")
        }
    }

    fn spawn(
        run: impl FnOnce(&Worker) -> anyhow::Result<Completion> + Send + 'static,
    ) -> anyhow::Result<Self> {
        let canceled = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Events::default()));
        let mut current = CURRENT
            .lock()
            .map_err(|_| anyhow::anyhow!("Screen color ownership state was poisoned"))?;
        if let Some(previous) = current.as_ref().and_then(Weak::upgrade) {
            previous.store(true, Ordering::Release);
        }
        let worker = Worker {
            canceled: canceled.clone(),
            events: events.clone(),
        };
        thread::Builder::new()
            .name("screen-color-picker".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Replacement waits here, after the old owner has canceled.
                    // The old window/DC/cursor cleanup precedes releasing this lock.
                    let _owner = NATIVE_OWNER.lock().map_err(|_| {
                        anyhow::anyhow!("Screen color native ownership was poisoned")
                    })?;
                    if worker.canceled() {
                        return Ok(Completion::Canceled);
                    }
                    run(&worker)
                }));
                let event = match outcome {
                    Ok(Ok(_)) if worker.canceled() => ScreenColorEvent::Canceled,
                    Ok(Ok(Completion::Done)) => ScreenColorEvent::Done,
                    Ok(Ok(Completion::Canceled)) => ScreenColorEvent::Canceled,
                    Ok(Err(error)) => ScreenColorEvent::Failed(format!("{error:#}")),
                    Err(_) => ScreenColorEvent::Failed("Screen color worker panicked".into()),
                };
                if let Ok(mut events) = worker.events.lock() {
                    events.push(event);
                }
            })
            .context("Create screen color worker")?;
        *current = Some(Arc::downgrade(&canceled));
        Ok(Self { canceled, events })
    }

    /// Reads only the worker's memory queue. Consecutive undelivered colors are
    /// coalesced so a paused UI cannot accumulate unbounded mouse-move events.
    pub fn drain(&self) -> anyhow::Result<Vec<ScreenColorEvent>> {
        Ok(self
            .events
            .lock()
            .map_err(|_| anyhow::anyhow!("Screen color event buffer was poisoned"))?
            .queue
            .drain(..)
            .collect())
    }

    pub fn cancel(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}

impl Drop for ScreenColorPicker {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub(crate) fn colorref_rgb(value: u32) -> anyhow::Result<u32> {
    ensure!(
        value != u32::MAX,
        "GetPixel could not read the screen snapshot"
    );
    Ok(((value & 0xff) << 16) | (value & 0xff00) | ((value >> 16) & 0xff))
}

pub(crate) struct CursorImage<'a> {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) hotspot: [u16; 2],
    pub(crate) dib: &'a [u8],
}

fn word(bytes: &[u8], offset: usize) -> anyhow::Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .context("Cursor word is truncated")?
            .try_into()?,
    ))
}

fn dword(bytes: &[u8], offset: usize) -> anyhow::Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("Cursor dword is truncated")?
            .try_into()?,
    ))
}

pub(crate) fn cursor_images(bytes: &[u8]) -> anyhow::Result<Vec<CursorImage<'_>>> {
    ensure!(
        word(bytes, 0)? == 0 && word(bytes, 2)? == 2,
        "Resource is not a CUR file"
    );
    let count = usize::from(word(bytes, 4)?);
    ensure!((1..=64).contains(&count), "Cursor image count is invalid");
    let directory_end = 6 + count * 16;
    ensure!(
        directory_end <= bytes.len(),
        "Cursor directory is truncated"
    );
    let mut images = Vec::with_capacity(count);
    let mut regions = Vec::with_capacity(count);
    for index in 0..count {
        let entry = 6 + index * 16;
        let size = |byte: u8| if byte == 0 { 256 } else { u16::from(byte) };
        let width = size(bytes[entry]);
        let height = size(bytes[entry + 1]);
        ensure!(
            bytes[entry + 2] == 0 && bytes[entry + 3] == 0,
            "Cursor directory format is invalid"
        );
        let hotspot = [word(bytes, entry + 4)?, word(bytes, entry + 6)?];
        ensure!(
            hotspot[0] < width && hotspot[1] < height,
            "Cursor hotspot is outside its image"
        );
        let length = usize::try_from(dword(bytes, entry + 8)?)?;
        let offset = usize::try_from(dword(bytes, entry + 12)?)?;
        let end = offset
            .checked_add(length)
            .context("Cursor image range overflow")?;
        ensure!(
            offset >= directory_end && end <= bytes.len(),
            "Cursor image range is invalid"
        );
        ensure!(
            regions
                .iter()
                .all(|&(start, end)| offset >= end || start >= offset + length),
            "Cursor images overlap"
        );
        regions.push((offset, end));
        let dib = &bytes[offset..end];
        ensure!(
            dword(dib, 0)? == 40
                && dword(dib, 4)? == u32::from(width)
                && dword(dib, 8)? == u32::from(height) * 2
                && word(dib, 12)? == 1
                && word(dib, 14)? == 32
                && dword(dib, 16)? == 0,
            "Cursor image differs from the source 32-bit uncompressed DIB format"
        );
        let mask_stride = usize::from(width).div_ceil(32) * 4;
        let expected = 40 + usize::from(height) * (usize::from(width) * 4 + mask_stride);
        ensure!(
            dib.len() == expected,
            "Cursor pixels or AND mask are incomplete"
        );
        images.push(CursorImage {
            width,
            height,
            hotspot,
            dib,
        });
    }
    Ok(images)
}

#[cfg(test)]
#[path = "screen_color_tests.rs"]
mod tests;
