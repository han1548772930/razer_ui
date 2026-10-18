//! ClipboardService's serialized save/write/paste/restore lifecycle.
use super::{Cancellation, input::Input};
use anyhow::{Context, Result, ensure};
use razer_device::macro_runtime::Keyboard;
use std::{
    collections::VecDeque,
    ptr,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, GlobalFree, HGLOBAL, HWND},
    Security::{ImpersonateAnonymousToken, RevertToSelf},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
            OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
        },
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Threading::GetCurrentThread,
    },
    UI::{
        Input::KeyboardAndMouse::GetKeyState,
        WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
    },
};

enum Command {
    Paste {
        text: String,
        path: Option<String>,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Stop,
}
#[derive(Clone)]
pub(super) struct Client(mpsc::Sender<Command>);
impl Client {
    pub fn paste(&self, text: String, path: Option<String>) -> Result<()> {
        let (reply, result) = mpsc::channel();
        self.0
            .send(Command::Paste { text, path, reply })
            .context("Clipboard worker stopped")?;
        result
            .recv()
            .context("Clipboard worker dropped paste completion")?
            .map_err(anyhow::Error::msg)
    }
}

pub(super) struct Service {
    sender: mpsc::Sender<Command>,
    thread: Option<JoinHandle<Result<()>>>,
}
impl Service {
    pub fn start() -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let (ready, initialized) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("razer-macro-clipboard".into())
            .spawn(move || {
                let window = match Window::create() {
                    Ok(window) => {
                        let _ = ready.send(Ok(()));
                        window
                    }
                    Err(error) => {
                        let _ = ready.send(Err(format!("{error:#}")));
                        return Err(error);
                    }
                };
                work(window, receiver)
            })?;
        let result = initialized
            .recv()
            .context("Clipboard worker initialization channel closed");
        match result {
            Ok(Ok(())) => Ok(Self {
                sender,
                thread: Some(thread),
            }),
            error => {
                let _ = thread.join();
                anyhow::bail!("Clipboard worker initialization failed: {error:?}")
            }
        }
    }
    pub fn client(&self) -> Client {
        Client(self.sender.clone())
    }
    pub fn shutdown(&mut self) -> Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        let _ = self.sender.send(Command::Stop);
        thread
            .join()
            .map_err(|_| anyhow::anyhow!("Clipboard worker panicked"))?
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Clipboard cleanup: {error:#}");
        }
    }
}

struct Window(HWND);
impl Window {
    fn create() -> Result<Self> {
        let class = "STATIC\0".encode_utf16().collect::<Vec<_>>();
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        ensure!(
            !hwnd.is_null(),
            "Create clipboard owner window: {}",
            unsafe { GetLastError() }
        );
        Ok(Self(hwnd))
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

struct Clipboard;
impl Clipboard {
    fn acquire(window: HWND) -> Result<Self> {
        for _ in 0..5 {
            if unsafe { OpenClipboard(window) } != 0 {
                return Ok(Self);
            }
            thread::sleep(Duration::from_millis(5));
        }
        anyhow::bail!("OpenClipboard: {}", unsafe { GetLastError() })
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        // ClipboardWin releases clipboard access under an anonymous thread
        // token; restore the worker's token immediately after CloseClipboard.
        let impersonated = unsafe { ImpersonateAnonymousToken(GetCurrentThread()) };
        unsafe {
            CloseClipboard();
        }
        if impersonated != 0 {
            unsafe {
                RevertToSelf();
            }
        }
    }
}

struct Allocation(HGLOBAL);
impl Allocation {
    fn copy(bytes: &[u8]) -> Result<Self> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) };
        ensure!(!handle.is_null(), "GlobalAlloc clipboard: {}", unsafe {
            GetLastError()
        });
        let allocation = Self(handle);
        let pointer = unsafe { GlobalLock(handle) };
        ensure!(!pointer.is_null(), "GlobalLock clipboard: {}", unsafe {
            GetLastError()
        });
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast(), bytes.len());
            GlobalUnlock(handle);
        }
        Ok(allocation)
    }
    fn publish(mut self, format: u32) -> Result<()> {
        let result = unsafe { SetClipboardData(format, self.0) };
        ensure!(
            !result.is_null(),
            "SetClipboardData({format}): {}",
            unsafe { GetLastError() }
        );
        self.0 = ptr::null_mut();
        Ok(())
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}

struct Snapshot {
    format: u32,
    bytes: Vec<u8>,
}
fn registered(name: &str) -> Result<u32> {
    let name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let format = unsafe { RegisterClipboardFormatW(name.as_ptr()) };
    ensure!(format != 0, "RegisterClipboardFormatW failed");
    Ok(format)
}
fn read(format: u32) -> Result<Vec<u8>> {
    let handle = unsafe { GetClipboardData(format) };
    ensure!(!handle.is_null(), "GetClipboardData({format}) failed");
    let size = unsafe { GlobalSize(handle) };
    ensure!(
        size != 0,
        "Clipboard format {format} has no readable allocation"
    );
    let pointer = unsafe { GlobalLock(handle) };
    ensure!(!pointer.is_null(), "Clipboard format {format} lock failed");
    let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec() };
    unsafe {
        GlobalUnlock(handle);
    }
    Ok(bytes)
}

fn snapshot(window: HWND) -> Result<Option<Snapshot>> {
    let _clipboard = Clipboard::acquire(window)?;
    // bcdd0 chooses the first retained semantic format in this order. Retain
    // its external Windows payload, including HTML source URL and file names.
    let formats = [
        13,
        registered("HTML Format")?,
        registered("Rich Text Format")?,
        15,
        registered("FileNameW")?,
        registered("FileName")?,
        17,
        8,
        registered("PNG")?,
    ];
    for format in formats {
        if unsafe { IsClipboardFormatAvailable(format) } != 0 {
            return Ok(Some(Snapshot {
                format,
                bytes: read(format)?,
            }));
        }
    }
    Ok(None)
}

fn replace(window: HWND, snapshot: Option<&Snapshot>, mutated: &mut bool) -> Result<()> {
    let allocation = snapshot
        .map(|snapshot| Allocation::copy(&snapshot.bytes))
        .transpose()?;
    let _clipboard = Clipboard::acquire(window)?;
    ensure!(unsafe { EmptyClipboard() } != 0, "EmptyClipboard failed");
    *mutated = true;
    if let (Some(snapshot), Some(allocation)) = (snapshot, allocation) {
        allocation.publish(snapshot.format)?;
    }
    Ok(())
}

fn paste_keys(path: Option<&str>) -> Result<()> {
    let mut input = Input::open(
        path,
        Arc::new(Cancellation::default()),
        super::acceleration::Owner::default(),
    )?;
    ensure!(
        input.has_driver(),
        "Clipboard paste requires an available mapping input endpoint"
    );
    input.cancellable = false;
    input.retain_held_outputs = false;
    // 11c460 observes both sides independently. 11b849/11be52 build chained
    // tasks in reverse table order, preserving extended scan flags.
    let modifiers = [
        (164, 56, 0),
        (165, 56, 2),
        (162, 29, 0),
        (163, 29, 2),
        (160, 42, 0),
        (161, 54, 0),
        (91, 91, 2),
        (92, 92, 2),
    ]
    .into_iter()
    .filter(|(vk, _, _)| unsafe { GetKeyState(*vk) } < 0)
    .collect::<Vec<_>>();
    let mut errors = Vec::new();
    let mut send = |scancode, flag| {
        if let Err(error) = input.keyboard(&Keyboard {
            scancode,
            flag,
            unit: None,
            modifiers: 0,
            delay_ms: None,
            release_after_ms: None,
        }) {
            errors.push(error);
            false
        } else {
            true
        }
    };
    for (_, scan, flag) in modifiers.iter().rev() {
        send(*scan, *flag | 1);
    }
    for (scan, flag, delay) in [(29, 0, 0), (47, 0, 50), (47, 1, 0), (29, 1, 10)] {
        if send(scan, flag) && delay != 0 {
            thread::sleep(Duration::from_millis(delay));
        }
    }
    for (_, scan, flag) in modifiers.iter().rev() {
        send(*scan, *flag);
    }
    if let Err(error) = input.close() {
        errors.push(format!("{error:#}"));
    }
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

#[derive(Default)]
struct Restoration {
    original: Option<Option<Snapshot>>,
    pending: VecDeque<Instant>,
    failed: Option<String>,
}
impl Restoration {
    fn begin(&mut self, capture: impl FnOnce() -> Result<Option<Snapshot>>) -> Result<()> {
        ensure!(
            self.failed.is_none(),
            "Clipboard restoration remains incomplete: {}",
            self.failed.as_deref().unwrap_or("")
        );
        if self.original.is_none() {
            self.original = Some(capture()?);
        }
        Ok(())
    }
    fn changed(&mut self, mutated: bool, now: Instant) {
        if mutated {
            self.pending.push_back(now + Duration::from_millis(500));
        } else if self.pending.is_empty() {
            self.original = None;
        }
    }
    fn restore(
        &mut self,
        now: Instant,
        closing: bool,
        mut write: impl FnMut(Option<&Snapshot>) -> Result<()>,
    ) -> Result<()> {
        while self
            .pending
            .front()
            .is_some_and(|deadline| *deadline <= now)
        {
            self.pending.pop_front();
        }
        if !closing && !self.pending.is_empty() {
            return Ok(());
        }
        let Some(original) = self.original.as_ref() else {
            return Ok(());
        };
        if let Err(error) = write(original.as_ref()) {
            self.failed = Some(format!("{error:#}"));
            return Err(error);
        }
        self.original = None;
        self.failed = None;
        self.pending.clear();
        Ok(())
    }
}

fn work(window: Window, receiver: mpsc::Receiver<Command>) -> Result<()> {
    let mut restoration = Restoration::default();
    let mut delayed_failure = None;
    loop {
        let command = if let Some(deadline) = restoration.pending.front().copied() {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => Some(Command::Stop),
            }
        } else {
            Some(receiver.recv().unwrap_or(Command::Stop))
        };
        match command {
            Some(Command::Paste { text, path, reply }) => {
                let result = (|| {
                    restoration.begin(|| snapshot(window.0))?;
                    let bytes = text
                        .encode_utf16()
                        .chain(Some(0))
                        .flat_map(u16::to_le_bytes)
                        .collect();
                    let mut mutated = false;
                    // Source writes text before opening the paste endpoint. A
                    // failed endpoint still owns restoration of that mutation.
                    let written = replace(
                        window.0,
                        Some(&Snapshot { format: 13, bytes }),
                        &mut mutated,
                    );
                    let result = written.and_then(|()| paste_keys(path.as_deref()));
                    restoration.changed(mutated, Instant::now());
                    result
                })();
                let _ = reply.send(result.map_err(|error| format!("{error:#}")));
            }
            Some(Command::Stop) => {
                let result = restoration.restore(Instant::now(), true, |original| {
                    replace(window.0, original, &mut false)
                });
                return match (result, delayed_failure) {
                    (Ok(()), None) => Ok(()),
                    (Ok(()), Some(error)) => Err(anyhow::anyhow!(
                        "Earlier clipboard restoration failed ({error}); shutdown restored the retained clipboard"
                    )),
                    (Err(error), None) => Err(error),
                    (Err(error), Some(previous)) => Err(anyhow::anyhow!(
                        "Earlier clipboard restoration: {previous}; shutdown restoration: {error:#}"
                    )),
                };
            }
            None => {
                // Keep a failed restore and its bytes owned until shutdown;
                // subsequent pastes report that failure instead of overwriting
                // the recovery snapshot or pretending restoration succeeded.
                if let Err(error) = restoration.restore(Instant::now(), false, |original| {
                    replace(window.0, original, &mut false)
                }) {
                    delayed_failure = Some(format!("{error:#}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(text: &str) -> Option<Snapshot> {
        Some(Snapshot {
            format: 13,
            bytes: text.as_bytes().to_vec(),
        })
    }

    #[test]
    fn overlapping_pastes_restore_the_first_snapshot_after_the_last_deadline() {
        let start = Instant::now();
        let mut state = Restoration::default();
        state.begin(|| Ok(snapshot("original"))).unwrap();
        state.changed(true, start);
        state
            .begin(|| panic!("overlap must not replace original snapshot"))
            .unwrap();
        state.changed(true, start + Duration::from_millis(100));
        state
            .restore(start + Duration::from_millis(500), false, |_| {
                panic!("last paste still owns clipboard")
            })
            .unwrap();
        state
            .restore(start + Duration::from_millis(600), false, |saved| {
                assert_eq!(saved.unwrap().bytes, b"original");
                Ok(())
            })
            .unwrap();
        assert!(state.original.is_none());
    }

    #[test]
    fn failed_open_before_empty_does_not_restore_unmodified_clipboard() {
        let start = Instant::now();
        let mut state = Restoration::default();
        state.begin(|| Ok(snapshot("original"))).unwrap();
        state.changed(false, start);
        state
            .restore(start, true, |_| panic!("clipboard never changed"))
            .unwrap();
    }

    #[test]
    fn failed_restore_keeps_recovery_and_blocks_new_paste_until_shutdown_confirms_restore() {
        let start = Instant::now();
        let mut state = Restoration::default();
        state.begin(|| Ok(snapshot("original"))).unwrap();
        state.changed(true, start);
        assert!(
            state
                .restore(
                    start + Duration::from_millis(500),
                    false,
                    |_| anyhow::bail!("clipboard busy")
                )
                .is_err()
        );
        assert!(
            state
                .begin(|| panic!("must not overwrite retained recovery"))
                .is_err()
        );
        state
            .restore(start + Duration::from_millis(600), true, |saved| {
                assert_eq!(saved.unwrap().bytes, b"original");
                Ok(())
            })
            .unwrap();
        assert!(state.failed.is_none());
    }
}
