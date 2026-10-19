//! Full-resolution animation with one displayed frame and one frame of lookahead.
//! The codec owns its disposal canvas; decoded frame count never grows with time.
use gpui_kit::{
    App, AppContext as _, AssetSource, Context, Entity, ImageCacheError, RenderImage, Resource,
    SharedString, Task, WeakEntity, Window,
};
use image::{AnimationDecoder as _, ImageDecoder as _};
use razer_storage::file_transfer::{Operation, Transfer};
use std::{io::Cursor, sync::Arc, thread::JoinHandle, time::{Duration, Instant}};

pub(super) fn candidate(source: &Resource) -> bool {
    let extension = match source {
        Resource::Embedded(path) => path.rsplit('.').next(),
        Resource::Path(path) => path.extension().and_then(|extension| extension.to_str()),
        Resource::Uri(_) => None,
    };
    extension.is_some_and(|extension| extension.eq_ignore_ascii_case("gif") || extension.eq_ignore_ascii_case("webp"))
}

#[derive(Default)]
pub(super) struct Players(Vec<WeakEntity<Player>>);
impl Players {
    pub(super) fn load(&mut self, source: &Resource, window: &mut Window, cx: &mut App)
        -> Option<Result<Arc<RenderImage>, ImageCacheError>>
    {
        let key = format!("resource-animation:{source:?}");
        let player = window.use_keyed_state(SharedString::from(key), cx, |_, cx| Player::new(source.clone(), cx));
        let weak = player.downgrade();
        self.0.retain(|player| player.upgrade().is_some());
        if !self.0.contains(&weak) { self.0.push(weak); }
        player.update(cx, |player, cx| player.image(window, cx))
    }
    pub(super) fn clear(&mut self, cx: &mut App) {
        for player in self.0.drain(..).filter_map(|player| player.upgrade()) {
            player.update(cx, |player, cx| player.stop(cx));
        }
    }
    pub(super) fn retry_failed(&mut self, cx: &mut App) {
        for player in self.0.iter().filter_map(|player| player.upgrade()) {
            player.update(cx, |player, cx| {
                if player.error.is_some() { player.restart(cx); }
            });
        }
    }
    pub(super) fn errors(&self, cx: &App) -> Vec<String> {
        self.0.iter().filter_map(|player| player.upgrade()).filter_map(|player| {
            let player = player.read(cx);
            player.error.as_ref().map(|error| format!("{:?}: {error}", player.source))
        }).collect()
    }
}

struct TimedFrame {
    image: Arc<RenderImage>,
    delay: Duration,
}

struct Playback<T> {
    current: Option<T>,
    next: Option<T>,
    decoder_ended: bool,
    completed: bool,
    last_frame: Option<Duration>,
}
impl<T> Default for Playback<T> {
    fn default() -> Self {
        Self { current: None, next: None, decoder_ended: false, completed: false, last_frame: None }
    }
}
impl<T> Playback<T> {
    /// Returns whether the first frame needs its one-frame lookahead requested.
    fn receive(&mut self, frame: Option<T>) -> bool {
        if let Some(frame) = frame {
            if self.current.is_none() {
                self.current = Some(frame);
                return true;
            }
            assert!(self.next.is_none(), "animation lookahead is bounded to one frame");
            self.next = Some(frame);
        } else {
            self.decoder_ended = true;
        }
        false
    }
    fn advance(&mut self, now: Duration, playing: bool, delay: impl FnOnce(&T) -> Duration) -> Option<T> {
        if !playing || self.completed {
            self.last_frame = None;
            return None;
        }
        let current = self.current.as_ref()?;
        let Some(previous) = self.last_frame else {
            self.last_frame = Some(now);
            return None;
        };
        let delay = delay(current);
        if now.saturating_sub(previous) < delay { return None; }
        if let Some(next) = self.next.take() {
            self.last_frame = Some(previous.saturating_add(delay));
            return self.current.replace(next);
        }
        if self.decoder_ended { self.completed = true; }
        None
    }
}

struct Worker {
    request: async_channel::Sender<()>,
    results: async_channel::Receiver<std::io::Result<Option<TimedFrame>>>,
    transfer: Transfer,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    fn start(source: Resource, assets: Arc<dyn AssetSource>) -> std::io::Result<Self> {
        let transfer = Transfer::new();
        let operation = transfer.accept().map_err(std::io::Error::other)?;
        let (request, requests) = async_channel::bounded(1);
        let (sender, results) = async_channel::bounded(1);
        let thread = std::thread::Builder::new().name("resource-animation".into()).spawn(move || {
            let mut sequence = read_source(source, assets).and_then(FrameSequence::new);
            while requests.recv_blocking().is_ok() {
                if operation.check().is_err() { break; }
                let frame = match &mut sequence {
                    Ok(sequence) => sequence.next(&operation),
                    Err(error) => Err(std::io::Error::other(error.to_string())),
                };
                let done = !matches!(frame, Ok(Some(_)));
                if operation.check().is_err() || sender.send_blocking(frame).is_err() || done { break; }
            }
        })?;
        Ok(Self { request, results, transfer, thread: Some(thread) })
    }
    fn stop(&mut self, cx: &App) {
        self.transfer.cancel();
        self.request.close();
        self.results.close();
        if let Some(thread) = self.thread.take() {
            cx.background_executor().spawn(async move {
                if thread.join().is_err() { log::error!("Animation decoder worker panicked"); }
            }).detach();
        }
    }
}

struct Player {
    source: Resource,
    worker: Option<Worker>,
    completion: Option<Task<()>>,
    playback: Playback<TimedFrame>,
    error: Option<ImageCacheError>,
    origin: Instant,
    used_this_frame: bool,
}
impl Player {
    fn new(source: Resource, cx: &mut Context<Self>) -> Self {
        cx.on_release(|player, cx| player.stop(cx)).detach();
        let mut player = Self {
            source, worker: None, completion: None, playback: Playback::default(), error: None,
            origin: Instant::now(), used_this_frame: false,
        };
        player.start(cx);
        player
    }
    fn start(&mut self, cx: &mut Context<Self>) {
        let worker = match Worker::start(self.source.clone(), cx.asset_source().clone()) {
            Ok(worker) => worker,
            Err(error) => { self.error = Some(error.into()); return; }
        };
        let results = worker.results.clone();
        let transfer = worker.transfer.clone();
        self.worker = Some(worker);
        self.completion = Some(cx.spawn(async move |owner, cx| {
            while let Ok(result) = results.recv().await {
                let accepted = owner.update(cx, |player, cx| {
                    if transfer.check().is_err() || !player.worker.as_ref().is_some_and(|worker| worker.transfer.is_same(&transfer)) {
                        return false;
                    }
                    match result {
                        Ok(frame) => {
                            if player.playback.receive(frame) { player.request_next(); }
                        }
                        Err(error) => { player.error = Some(error.into()); }
                    }
                    cx.notify();
                    true
                }).unwrap_or(false);
                if !accepted { break; }
            }
        }));
        self.request_next();
    }
    fn request_next(&self) {
        if let Some(worker) = &self.worker { let _ = worker.request.try_send(()); }
    }
    fn restart(&mut self, cx: &mut Context<Self>) {
        self.stop(cx);
        self.error = None;
        self.origin = Instant::now();
        self.start(cx);
        cx.notify();
    }
    fn stop(&mut self, cx: &mut App) {
        if let Some(mut worker) = self.worker.take() { worker.stop(cx); }
        self.completion = None;
        let playback = std::mem::take(&mut self.playback);
        let images = [playback.current, playback.next].into_iter().flatten().map(|frame| frame.image).collect::<Vec<_>>();
        if !images.is_empty() {
            // Element release can run while its Window is absent from App.
            cx.defer(move |cx| { for image in images { cx.drop_image(image, None); } });
        }
    }
    fn image(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        if let Some(error) = &self.error { return Some(Err(error.clone())); }
        let playing = window.is_window_active() && !cx.reduce_motion();
        if !self.used_this_frame {
            self.used_this_frame = true;
            let owner = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| { let _ = owner.update(cx, |player, _| player.used_this_frame = false); });
            if let Some(previous) = self.playback.advance(self.origin.elapsed(), playing, |frame| frame.delay) {
                cx.drop_image(previous.image, Some(window));
                self.request_next();
            }
        }
        if playing && !self.playback.completed && self.playback.current.is_some() { window.request_animation_frame(); }
        self.playback.current.as_ref().map(|frame| Ok(frame.image.clone()))
    }
}

fn read_source(source: Resource, assets: Arc<dyn AssetSource>) -> std::io::Result<Arc<[u8]>> {
    let bytes = match source {
        Resource::Embedded(path) => assets.load(&path).map_err(std::io::Error::other)?
            .ok_or_else(|| std::io::Error::other(format!("Missing animation: {path}")))?.into_owned(),
        Resource::Path(path) => std::fs::read(path)?,
        Resource::Uri(_) => return Err(std::io::Error::other("Animation requires verified local resource bytes")),
    };
    Ok(bytes.into())
}

struct FrameSequence {
    bytes: Arc<[u8]>,
    frames: image::Frames<'static>,
    remaining_passes: Option<u32>,
    frames_in_pass: usize,
}
impl FrameSequence {
    fn decoder(bytes: Arc<[u8]>) -> std::io::Result<(image::Frames<'static>, Option<u32>)> {
        match image::guess_format(&bytes).map_err(std::io::Error::other)? {
            image::ImageFormat::Gif => {
                let passes = gif_passes(&bytes)?;
                let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(std::io::Error::other)?;
                Ok((decoder.into_frames(), passes))
            }
            image::ImageFormat::WebP => {
                let mut decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(std::io::Error::other)?;
                if !decoder.has_animation() {
                    let mut image = image::DynamicImage::from_decoder(decoder).map_err(std::io::Error::other)?;
                    // Static image decoding retains the same orientation as GPUI.
                    // Animated WebP applies disposal in its codec frame iterator.
                    let frame = image::Frame::new(image.to_rgba8());
                    return Ok((image::Frames::new(Box::new(std::iter::once(Ok(frame)))), Some(1)));
                }
                let passes = match decoder.loop_count() {
                    image::metadata::LoopCount::Infinite => None,
                    image::metadata::LoopCount::Finite(count) => Some(count.get()),
                };
                decoder.set_background_color(image::Rgba([0, 0, 0, 0])).map_err(std::io::Error::other)?;
                Ok((decoder.into_frames(), passes))
            }
            _ => Err(std::io::Error::other("Animation resource has an unexpected image format")),
        }
    }
    fn new(bytes: Arc<[u8]>) -> std::io::Result<Self> {
        let (frames, remaining_passes) = Self::decoder(bytes.clone())?;
        Ok(Self { bytes, frames, remaining_passes, frames_in_pass: 0 })
    }
    fn next(&mut self, operation: &Operation) -> std::io::Result<Option<TimedFrame>> {
        operation.check().map_err(std::io::Error::other)?;
        let frame = match self.frames.next() {
            Some(frame) => frame,
            None => {
                if self.frames_in_pass == 0 { return Err(std::io::Error::other("Animation has no frames")); }
                if let Some(remaining) = &mut self.remaining_passes {
                    if *remaining <= 1 { return Ok(None); }
                    *remaining -= 1;
                }
                self.frames = Self::decoder(self.bytes.clone())?.0;
                self.frames_in_pass = 0;
                self.frames.next().ok_or_else(|| std::io::Error::other("Animation loop has no frames"))?
            }
        };
        let mut frame = frame.map_err(std::io::Error::other)?;
        self.frames_in_pass += 1;
        let delay = Duration::from(frame.delay());
        for pixels in frame.buffer_mut().chunks_mut(16 * 1024) {
            operation.check().map_err(std::io::Error::other)?;
            for pixel in pixels.chunks_exact_mut(4) { pixel.swap(0, 2); }
        }
        Ok(Some(TimedFrame { image: Arc::new(RenderImage::new(vec![frame])), delay }))
    }
}

/// GIF without an application loop extension plays once. Netscape's finite
/// count gives additional repetitions, whereas WebP counts complete passes.
fn gif_passes(bytes: &[u8]) -> std::io::Result<Option<u32>> {
    let invalid = || std::io::Error::other("Invalid GIF block structure");
    if !bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a") { return Err(invalid()); }
    let packed = *bytes.get(10).ok_or_else(invalid)?;
    let mut offset = 13 + if packed & 0x80 != 0 { 3usize << ((packed & 7) + 1) } else { 0 };
    let mut passes = Some(1);
    loop {
        let tag = *bytes.get(offset).ok_or_else(invalid)?;
        offset += 1;
        match tag {
            0x3b => return Ok(passes),
            0x21 => {
                let label = *bytes.get(offset).ok_or_else(invalid)?;
                offset += 1;
                let mut application = false;
                let mut first = true;
                loop {
                    let length = usize::from(*bytes.get(offset).ok_or_else(invalid)?);
                    offset += 1;
                    if length == 0 { break; }
                    let data = bytes.get(offset..offset + length).ok_or_else(invalid)?;
                    if label == 0xff && first { application = data == b"NETSCAPE2.0" || data == b"ANIMEXTS1.0"; }
                    else if application && data.len() == 3 && data[0] == 1 {
                        let repeats = u16::from_le_bytes([data[1], data[2]]);
                        passes = if repeats == 0 { None } else { Some(u32::from(repeats) + 1) };
                    }
                    first = false;
                    offset += length;
                }
            }
            0x2c => {
                let packed = *bytes.get(offset + 8).ok_or_else(invalid)?;
                offset += 9 + if packed & 0x80 != 0 { 3usize << ((packed & 7) + 1) } else { 0 };
                bytes.get(offset).ok_or_else(invalid)?;
                offset += 1;
                loop {
                    let length = usize::from(*bytes.get(offset).ok_or_else(invalid)?);
                    offset += 1;
                    if length == 0 { break; }
                    bytes.get(offset..offset + length).ok_or_else(invalid)?;
                    offset += length;
                }
            }
            _ => return Err(invalid()),
        }
    }
}

#[cfg(test)]
mod tests;
