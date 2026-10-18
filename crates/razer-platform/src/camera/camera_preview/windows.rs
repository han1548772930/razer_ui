//! Media Foundation replacement for Chromium's local capture implementation.
//! All COM pointers stay on the capture worker or MF's callback threads. Reads
//! are asynchronous so stop, source switches and owner drop can flush them.
use super::{Command, Event, Frame, Source, VideoMode, copy_frame, finish_capture_shutdown};
use anyhow::{Context as _, Result, anyhow, ensure};
use std::{
    ptr,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
    },
    core::{GUID, HRESULT, PWSTR, Ref, implement},
};

#[path = "windows/identity.rs"]
mod identity;

const VIDEO_STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const ALL_STREAMS: u32 = MF_SOURCE_READER_ALL_STREAMS.0 as u32;

struct MediaRuntime;
impl MediaRuntime {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(error.into());
            }
        }
        Ok(Self)
    }
}
impl Drop for MediaRuntime {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}

fn attributes() -> Result<IMFAttributes> {
    let mut value = None;
    unsafe {
        MFCreateAttributes(&mut value, 4)?;
    }
    value.ok_or_else(|| anyhow!("MFCreateAttributes returned no attributes"))
}
fn string(attributes: &IMFAttributes, key: &GUID) -> Result<String> {
    let mut pointer = PWSTR::null();
    let mut length = 0;
    unsafe {
        attributes.GetAllocatedString(key, &mut pointer, &mut length)?;
    }
    if pointer.is_null() {
        return Err(anyhow!("camera attribute string is null"));
    }
    let result = if length <= 32768 {
        String::from_utf16(unsafe { std::slice::from_raw_parts(pointer.0, length as usize) })
            .context("invalid camera attribute UTF-16")
    } else {
        Err(anyhow!("oversized camera attribute"))
    };
    unsafe {
        CoTaskMemFree(Some(pointer.0.cast()));
    }
    result
}
fn activations() -> Result<Vec<(Source, IMFActivate)>> {
    let attributes = attributes()?;
    unsafe {
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )?;
    }
    let mut pointer = ptr::null_mut();
    let mut count = 0;
    unsafe {
        MFEnumDeviceSources(&attributes, &mut pointer, &mut count)?;
    }
    ensure!(
        count == 0 || !pointer.is_null(),
        "MF device enumeration returned a null array"
    );
    // MF owns the array allocation; each returned interface holds one ref. Take
    // those refs into Rust before freeing only the array with CoTaskMemFree.
    let devices: Vec<IMFActivate> = if count == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts_mut(pointer, count as usize) }
            .iter_mut()
            .filter_map(Option::take)
            .collect()
    };
    unsafe {
        CoTaskMemFree(Some(pointer.cast()));
    }
    devices
        .into_iter()
        .map(|activate| {
            let id = string(
                &activate,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
            )?;
            let name = string(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)?;
            let container_id = identity::container_id(&id).ok();
            Ok((
                Source {
                    id,
                    name,
                    container_id,
                },
                activate,
            ))
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Format {
    width: u32,
    height: u32,
    stride: i32,
}
type SampleResult = Result<Option<Frame>, String>;

#[implement(IMFSourceReaderCallback)]
struct ReaderCallback {
    format: Arc<Mutex<Option<Format>>>,
    samples: mpsc::Sender<SampleResult>,
    flushed: mpsc::Sender<()>,
}
impl IMFSourceReaderCallback_Impl for ReaderCallback_Impl {
    fn OnReadSample(
        &self,
        status: HRESULT,
        _: u32,
        flags: u32,
        _: i64,
        sample: Ref<'_, IMFSample>,
    ) -> windows::core::Result<()> {
        let read = || -> Result<Option<Frame>> {
            status.ok()?;
            ensure!(
                flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 == 0,
                "camera stream ended"
            );
            ensure!(
                flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 == 0,
                "camera media type changed during capture"
            );
            let Some(sample) = sample.as_ref() else {
                return Ok(None);
            };
            let format = self
                .format
                .lock()
                .map_err(|_| anyhow!("camera format lock failed"))?
                .ok_or_else(|| anyhow!("camera sample arrived before format selection"))?;
            let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
            let mut bytes = ptr::null_mut();
            let mut length = 0;
            unsafe {
                buffer.Lock(&mut bytes, None, Some(&mut length))?;
            }
            // Unlock even when size/stride validation fails.
            let result = if bytes.is_null() {
                Err(anyhow!("camera returned a null sample"))
            } else {
                copy_frame(
                    unsafe { std::slice::from_raw_parts(bytes, length as usize) },
                    format.width,
                    format.height,
                    format.stride,
                )
            };
            let unlock = unsafe { buffer.Unlock() };
            unlock?;
            result.map(Some)
        };
        let _ = self
            .samples
            .send(read().map_err(|error| format!("{error:#}")));
        Ok(())
    }
    fn OnFlush(&self, _: u32) -> windows::core::Result<()> {
        let _ = self.flushed.send(());
        Ok(())
    }
    fn OnEvent(&self, _: u32, _: Ref<'_, IMFMediaEvent>) -> windows::core::Result<()> {
        Ok(())
    }
}

struct Capture {
    reader: IMFSourceReader,
    source: IMFMediaSource,
    samples: mpsc::Receiver<SampleResult>,
    flushed: mpsc::Receiver<()>,
    playing: bool,
    closed: bool,
}
impl Capture {
    fn open(id: &str, requested: Option<VideoMode>) -> Result<(Self, Vec<VideoMode>)> {
        let mut matching = activations()?
            .into_iter()
            .filter(|(source, _)| source.id == id);
        let (_, activate) = matching
            .next()
            .ok_or_else(|| anyhow!("selected media source is no longer present"))?;
        ensure!(
            matching.next().is_none(),
            "duplicate selected media source identity"
        );
        let source: IMFMediaSource = unsafe { activate.ActivateObject()? };
        // A failed format negotiation must shut down the activated source too.
        let result = Self::with_source(&source, requested);
        match result {
            Ok((reader, samples, flushed, modes)) => Ok((
                Self {
                    reader,
                    source,
                    samples,
                    flushed,
                    playing: false,
                    closed: false,
                },
                modes,
            )),
            Err(error) => match unsafe { source.Shutdown() } {
                Ok(()) => Err(error),
                Err(cleanup) => Err(anyhow!("{error:#}; camera source shutdown: {cleanup}")),
            },
        }
    }
    fn with_source(
        source: &IMFMediaSource,
        requested: Option<VideoMode>,
    ) -> Result<(
        IMFSourceReader,
        mpsc::Receiver<SampleResult>,
        mpsc::Receiver<()>,
        Vec<VideoMode>,
    )> {
        let (tx, samples) = mpsc::channel();
        let (flushed_sender, flushed) = mpsc::channel();
        let format = Arc::new(Mutex::new(None));
        let callback: IMFSourceReaderCallback = ReaderCallback {
            format: format.clone(),
            samples: tx,
            flushed: flushed_sender,
        }
        .into();
        let settings = attributes()?;
        unsafe {
            settings.SetUnknown(&MF_SOURCE_READER_ASYNC_CALLBACK, &callback)?;
            settings.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        }
        let reader = unsafe { MFCreateSourceReaderFromMediaSource(source, &settings)? };
        unsafe {
            reader.SetStreamSelection(ALL_STREAMS, false)?;
            reader.SetStreamSelection(VIDEO_STREAM, true)?;
        }
        let mut modes = Vec::new();
        let mut selected = None;
        for index in 0..4096 {
            let media = match unsafe { reader.GetNativeMediaType(VIDEO_STREAM, index) } {
                Ok(media) => media,
                Err(error) if error.code() == MF_E_NO_MORE_TYPES => break,
                Err(error) => return Err(error.into()),
            };
            let size = unsafe { media.GetUINT64(&MF_MT_FRAME_SIZE)? };
            let rate = unsafe { media.GetUINT64(&MF_MT_FRAME_RATE)? };
            let denominator = rate as u32;
            if denominator == 0 {
                continue;
            }
            let mode = VideoMode {
                width: (size >> 32) as u32,
                height: size as u32,
                fps: (((rate >> 32) as f64 / denominator as f64).round()) as u32,
            };
            if !modes.contains(&mode) {
                modes.push(mode);
            }
            if requested == Some(mode) && selected.is_none() {
                selected = Some(media);
            }
        }
        if requested.is_some() {
            let media = selected
                .ok_or_else(|| anyhow!("requested camera resolution/frame rate is unavailable"))?;
            unsafe {
                reader.SetCurrentMediaType(VIDEO_STREAM, None, &media)?;
            }
        }
        let output = unsafe { MFCreateMediaType()? };
        unsafe {
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            reader.SetCurrentMediaType(VIDEO_STREAM, None, &output)?;
        }
        let current = unsafe { reader.GetCurrentMediaType(VIDEO_STREAM)? };
        let size = unsafe { current.GetUINT64(&MF_MT_FRAME_SIZE)? };
        let width = (size >> 32) as u32;
        let height = size as u32;
        let stride = unsafe { current.GetUINT32(&MF_MT_DEFAULT_STRIDE) }
            .map(|value| value as i32)
            .unwrap_or_else(|_| (width.saturating_mul(4)) as i32);
        ensure!(
            width > 0 && height > 0 && u64::from(width) * u64::from(height) * 4 <= 64 * 1024 * 1024,
            "camera negotiated an unsupported image size"
        );
        *format
            .lock()
            .map_err(|_| anyhow!("camera format lock failed"))? = Some(Format {
            width,
            height,
            stride,
        });
        unsafe {
            reader.ReadSample(VIDEO_STREAM, 0, None, None, None, None)?;
        }
        Ok((reader, samples, flushed, modes))
    }
    fn next(&self) -> Result<()> {
        unsafe {
            self.reader
                .ReadSample(VIDEO_STREAM, 0, None, None, None, None)?;
        }
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.playing = false;
        // The async reader reports completion through OnFlush. Waiting before
        // Shutdown also keeps callback-owned samples inside the MF lifetime.
        // Microsoft IMFSourceReader::Flush documents that S_OK only accepts
        // an async flush; IMFSourceReaderCallback::OnFlush completes it.
        let result = finish_capture_shutdown(
            || unsafe { self.reader.Flush(ALL_STREAMS) }.context("camera flush request"),
            &self.flushed,
            Duration::from_secs(5),
            || unsafe { self.source.Shutdown() }.context("camera source shutdown"),
        );
        for _ in self.samples.try_iter() {}
        result
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn stop_capture(capture: &mut Option<Capture>, latest: &Mutex<Option<Frame>>) -> Result<()> {
    // Remove the displayed sample before waiting for the driver. The worker
    // does not publish more frames while closing this retained capture.
    if let Ok(mut frame) = latest.lock() {
        *frame = None;
    }
    if let Some(mut active) = capture.take() {
        active.close()?;
    }
    Ok(())
}

pub(super) fn run(
    commands: mpsc::Receiver<Command>,
    events: &mpsc::Sender<Event>,
    latest: Arc<Mutex<Option<Frame>>>,
) -> Result<()> {
    let _runtime = MediaRuntime::new()?;
    let mut capture: Option<Capture> = None;
    loop {
        match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::LoadPreferences(product)) => {
                let result = super::load_preferences(product)
                    .map(Event::Preferences)
                    .unwrap_or_else(|error| {
                        Event::Diagnostic(format!("camera preferences read: {error:#}"))
                    });
                let _ = events.send(result);
            }
            Ok(Command::SavePreferences(product, preferences)) => {
                if let Err(error) = super::save_preferences(product, &preferences) {
                    let _ = events.send(Event::Diagnostic(format!(
                        "camera preferences save: {error:#}"
                    )));
                }
            }
            Ok(Command::Stop) => {
                let event = match stop_capture(&mut capture, &latest) {
                    Ok(()) => Event::Stopped,
                    Err(error) => Event::Failed(format!("camera stop: {error:#}")),
                };
                let _ = events.send(event);
            }
            Ok(Command::StopForControl(reply)) => {
                let result = stop_capture(&mut capture, &latest)
                    .map_err(|error| format!("camera control stop: {error:#}"));
                if result.is_ok() {
                    let _ = events.send(Event::Stopped);
                }
                let _ = reply.send(result);
            }
            Ok(Command::Refresh) => {
                let result = activations().map(|devices| {
                    Event::Sources(devices.into_iter().map(|(source, _)| source).collect())
                });
                let _ = events.send(result.unwrap_or_else(|error| {
                    Event::Diagnostic(format!("camera enumeration: {error:#}"))
                }));
            }
            Ok(Command::Probe(source)) => {
                // useZm acquires a temporary stream for capabilities even
                // when preview is disabled, then stops every track.
                let result = (|| -> Result<Vec<VideoMode>> {
                    stop_capture(&mut capture, &latest)?;
                    let (mut probe, modes) = Capture::open(&source, None)?;
                    probe.close()?;
                    Ok(modes)
                })();
                let _ = events.send(match result {
                    Ok(modes) => Event::Capabilities { source, modes },
                    Err(error) => Event::Diagnostic(format!("camera capability probe: {error:#}")),
                });
            }
            Ok(Command::Start { source, mode }) => {
                if let Err(error) = stop_capture(&mut capture, &latest) {
                    let _ = events.send(Event::Failed(format!("camera source switch: {error:#}")));
                    continue;
                }
                match Capture::open(&source, mode) {
                    Ok((new_capture, modes)) => {
                        capture = Some(new_capture);
                        let _ = events.send(Event::Capabilities { source, modes });
                    }
                    Err(error) => {
                        let _ =
                            events.send(Event::Failed(format!("camera acquisition: {error:#}")));
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        let Some(active) = &mut capture else {
            continue;
        };
        let outcome = match active.samples.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("camera callback disconnected".into()))
            }
        };
        if let Some(outcome) = outcome {
            let outcome = outcome.and_then(|frame| {
                if let Some(frame) = frame {
                    if let Ok(mut latest) = latest.lock() {
                        *latest = Some(frame);
                    }
                    if !active.playing {
                        active.playing = true;
                        let _ = events.send(Event::Playing);
                    }
                }
                active
                    .next()
                    .map_err(|error| format!("camera read: {error:#}"))
            });
            if let Err(error) = outcome {
                if let Err(cleanup) = stop_capture(&mut capture, &latest) {
                    let _ = events.send(Event::Diagnostic(format!("camera cleanup: {cleanup:#}")));
                }
                let _ = events.send(Event::Failed(error));
            }
        }
    }
    // Drop the media source before MFShutdown/CoUninitialize.
    let cleanup = stop_capture(&mut capture, &latest);
    if let Err(error) = &cleanup {
        let _ = events.send(Event::Diagnostic(format!("camera shutdown: {error:#}")));
    }
    cleanup
}
