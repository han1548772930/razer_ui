//! Local OS equivalent of current 3594 enumerateDevices/getUserMedia/video.
//! The platform owns acquisition and shutdown; product filtering, selected
//! source, requested resolution and preview UI remain owned by the page.
use std::sync::{Arc, Mutex, mpsc};

/// Flush returns before the asynchronous source reader has released pending
/// samples. A stop receipt requires its callback, not just an accepted Flush.
/// This wait only runs on the capture worker; timeout is a diagnostic failure.
fn wait_for_flush(
    flushed: &mpsc::Receiver<()>,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    match flushed.recv_timeout(timeout) {
        Ok(()) => Ok(()),
        Err(mpsc::RecvTimeoutError::Timeout) => anyhow::bail!("camera flush completion timed out"),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            anyhow::bail!("camera flush callback disconnected")
        }
    }
}

/// Keep shutdown unconditional, but never turn a failed/unfinished flush into
/// a successful stop receipt. Closures keep COM confined to the Windows owner.
fn finish_capture_shutdown(
    request_flush: impl FnOnce() -> anyhow::Result<()>,
    flushed: &mpsc::Receiver<()>,
    timeout: std::time::Duration,
    shutdown: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let flush = request_flush().and_then(|_| wait_for_flush(flushed, timeout));
    match (flush, shutdown()) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(flush), Err(shutdown)) => Err(anyhow::anyhow!("{flush:#}; {shutdown:#}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}

#[cfg(windows)]
#[path = "camera_preview/windows.rs"]
mod windows;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub container_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoMode {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Top-down BGRA, opaque alpha; this is the native GPUI upload format.
    pub bgra: Vec<u8>,
}

#[derive(Debug)]
pub enum Event {
    Sources(Vec<Source>),
    Capabilities {
        source: String,
        modes: Vec<VideoMode>,
    },
    Playing,
    Stopped,
    Failed(String),
    Preferences(Preferences),
    Diagnostic(String),
}

/// Source ON_SET_IS_PREVIEW persists by PID; cameraHardwareId persists by
/// container. Keep these local preferences separate from device profile data.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub enabled: Option<bool>,
    pub sources: std::collections::BTreeMap<String, String>,
}
fn preference_path(product: u16) -> std::path::PathBuf {
    razer_storage::store_path().with_file_name(format!("camera-preview-{product}.json"))
}
fn load_preferences(product: u16) -> anyhow::Result<Preferences> {
    Ok(
        razer_storage::read_document(&preference_path(product), |text| {
            Ok(serde_json::from_str(text)?)
        })?
        .unwrap_or_default(),
    )
}
fn save_preferences(product: u16, preferences: &Preferences) -> anyhow::Result<()> {
    razer_storage::write_document(&preference_path(product), preferences, |text| {
        serde_json::from_str::<Preferences>(text)?;
        Ok(())
    })
}

enum Command {
    Refresh,
    Probe(String),
    Start {
        source: String,
        mode: Option<VideoMode>,
    },
    Stop,
    StopForControl(mpsc::SyncSender<Result<(), String>>),
    Shutdown,
    LoadPreferences(u16),
    SavePreferences(u16, Preferences),
}

/// One worker owns COM, MF, the source reader, callbacks and pending samples.
/// Only the latest frame is retained, so a hidden/slow UI cannot grow a queue.
pub struct Session {
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<Event>,
    latest: Arc<Mutex<Option<Frame>>>,
    worker: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
}
impl Session {
    pub fn new() -> anyhow::Result<Self> {
        #[cfg(windows)]
        {
            let (commands, incoming) = mpsc::channel();
            let (outgoing, events) = mpsc::channel();
            let latest = Arc::new(Mutex::new(None));
            let frames = latest.clone();
            let worker = std::thread::Builder::new()
                .name("camera-preview".into())
                .spawn(move || {
                    let result = windows::run(incoming, &outgoing, frames);
                    if let Err(error) = &result {
                        let _ = outgoing.send(Event::Failed(format!("{error:#}")));
                    }
                    result
                })?;
            Ok(Self {
                commands,
                events,
                latest,
                worker: Some(worker),
            })
        }
        #[cfg(not(windows))]
        anyhow::bail!("local camera preview requires the Windows media adapter")
    }

    pub fn refresh(&self) -> anyhow::Result<()> {
        self.commands.send(Command::Refresh).map_err(Into::into)
    }
    pub fn probe(&self, source: String) -> anyhow::Result<()> {
        self.commands
            .send(Command::Probe(source))
            .map_err(Into::into)
    }
    pub fn start(&self, source: String, mode: Option<VideoMode>) -> anyhow::Result<()> {
        anyhow::ensure!(
            !source.is_empty() && !source.contains('\0'),
            "invalid media source identity"
        );
        self.commands
            .send(Command::Start { source, mode })
            .map_err(Into::into)
    }
    pub fn stop(&self) -> anyhow::Result<()> {
        self.commands.send(Command::Stop).map_err(Into::into)
    }
    /// A private receipt for one control submission. An earlier ordinary stop
    /// event cannot release this lock, and channel loss never counts as success.
    pub fn stop_for_control(&self) -> anyhow::Result<mpsc::Receiver<Result<(), String>>> {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands.send(Command::StopForControl(reply))?;
        Ok(result)
    }
    pub fn load_preferences(&self, product: u16) -> anyhow::Result<()> {
        self.commands
            .send(Command::LoadPreferences(product))
            .map_err(Into::into)
    }
    pub fn save_preferences(&self, product: u16, preferences: Preferences) -> anyhow::Result<()> {
        self.commands
            .send(Command::SavePreferences(product, preferences))
            .map_err(Into::into)
    }
    pub fn events(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
    pub fn take_frame(&self) -> Option<Frame> {
        self.latest.lock().ok()?.take()
    }

    pub fn request_shutdown(&self) -> anyhow::Result<()> {
        self.commands.send(Command::Shutdown).map_err(Into::into)
    }

    /// Consume on the camera I/O owner after requesting shutdown. Completion
    /// includes source flush, MF/COM release and the actual capture thread join.
    pub fn shutdown(mut self) -> anyhow::Result<()> {
        let _ = self.commands.send(Command::Shutdown);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| anyhow::anyhow!("camera preview worker panicked"))?,
            None => Ok(()),
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // Never join a camera driver on the UI thread. The worker consumes this
        // command, flushes asynchronous reads, shuts down MF and releases COM.
        let _ = self.commands.send(Command::Shutdown);
    }
}

/// Copy RGB32 while handling padding and bottom-up image storage. Checked
/// dimensions prevent a malformed media type from indexing driver memory.
fn copy_frame(bytes: &[u8], width: u32, height: u32, stride: i32) -> anyhow::Result<Frame> {
    let row = (width as usize)
        .checked_mul(4)
        .ok_or_else(|| anyhow::anyhow!("camera row overflow"))?;
    let pitch = stride.unsigned_abs() as usize;
    let length = row
        .checked_mul(height as usize)
        .ok_or_else(|| anyhow::anyhow!("camera frame overflow"))?;
    anyhow::ensure!(
        width > 0 && height > 0 && length <= 64 * 1024 * 1024 && pitch >= row,
        "invalid camera RGB32 dimensions/stride"
    );
    let needed = pitch
        .checked_mul(height as usize - 1)
        .and_then(|n| n.checked_add(row))
        .ok_or_else(|| anyhow::anyhow!("camera sample size overflow"))?;
    anyhow::ensure!(
        bytes.len() >= needed,
        "camera sample is shorter than its media type"
    );
    let mut bgra = vec![0; length];
    for y in 0..height as usize {
        let input_y = if stride < 0 { height as usize - 1 - y } else { y };
        bgra[y * row..(y + 1) * row]
            .copy_from_slice(&bytes[input_y * pitch..input_y * pitch + row]);
    }
    for pixel in bgra.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    Ok(Frame {
        width,
        height,
        bgra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_shutdown_awaits_its_owned_thread_and_retains_release_failure() {
        let (commands, incoming) = mpsc::channel();
        let (_events, events) = mpsc::channel();
        let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_released = released.clone();
        let worker = std::thread::spawn(move || {
            assert!(matches!(incoming.recv().unwrap(), Command::Shutdown));
            worker_released.store(true, std::sync::atomic::Ordering::Release);
            anyhow::bail!("simulated MF release failure")
        });
        let session = Session {
            commands,
            events,
            latest: Arc::new(Mutex::new(None)),
            worker: Some(worker),
        };
        let error = session.shutdown().unwrap_err().to_string();
        assert!(error.contains("simulated MF release failure"));
        assert!(released.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn padded_bottom_up_samples_are_upright_and_opaque() {
        let source = [1, 2, 3, 0, 90, 90, 90, 90, 4, 5, 6, 0];
        let frame = copy_frame(&source, 1, 2, -8).unwrap();
        assert_eq!(frame.bgra, [4, 5, 6, 255, 1, 2, 3, 255]);
    }
    #[test]
    fn malformed_samples_cannot_be_published() {
        assert!(copy_frame(&[0; 4], 1, 2, 4).is_err());
        assert!(copy_frame(&[0; 4], 2, 1, 4).is_err());
        assert!(copy_frame(&[], u32::MAX, u32::MAX, i32::MAX).is_err());
    }
    #[test]
    fn stop_requires_the_asynchronous_flush_callback() {
        let (sender, receiver) = mpsc::channel();
        let timeout = std::time::Duration::from_millis(1);
        assert!(
            wait_for_flush(&receiver, timeout)
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        sender.send(()).unwrap();
        wait_for_flush(&receiver, timeout).unwrap();
        // The receipt can only complete the one flush which consumed it.
        assert!(wait_for_flush(&receiver, timeout).is_err());
        drop(sender);
        assert!(
            wait_for_flush(&receiver, timeout)
                .unwrap_err()
                .to_string()
                .contains("disconnected")
        );
    }
    #[test]
    fn shutdown_follows_flush_completion_and_keeps_both_failures() {
        use std::{cell::RefCell, time::Duration};
        let (sender, receiver) = mpsc::channel();
        let calls = RefCell::new(Vec::new());
        finish_capture_shutdown(
            || {
                calls.borrow_mut().push("flush");
                sender.send(()).unwrap();
                Ok(())
            },
            &receiver,
            Duration::from_millis(1),
            || {
                // The asynchronous completion has already been consumed.
                assert!(receiver.try_recv().is_err());
                calls.borrow_mut().push("shutdown");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*calls.borrow(), ["flush", "shutdown"]);
        let error = finish_capture_shutdown(
            || anyhow::bail!("request failed"),
            &receiver,
            Duration::from_secs(30),
            || anyhow::bail!("shutdown failed"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("request failed") && error.contains("shutdown failed"));
    }
    #[test]
    fn missing_flush_completion_still_shuts_down_without_success() {
        use std::{cell::Cell, time::Duration};
        let (_sender, receiver) = mpsc::channel();
        let closed = Cell::new(false);
        let error = finish_capture_shutdown(
            || Ok(()),
            &receiver,
            Duration::from_millis(1),
            || {
                closed.set(true);
                Ok(())
            },
        )
        .unwrap_err()
        .to_string();
        assert!(closed.get());
        assert!(error.contains("timed out"));
    }
}
