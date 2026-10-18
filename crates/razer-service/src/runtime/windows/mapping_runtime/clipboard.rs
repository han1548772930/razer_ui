//! ClipboardService's serialized save/write/paste/restore lifecycle.
#[path = "clipboard_dib.rs"]
mod dib;
#[path = "clipboard_formats.rs"]
mod formats;
#[path = "clipboard_keys.rs"]
mod keys;
use super::{Cancellation, input::Input};
use anyhow::{Context, Result, ensure};
use formats::Snapshot;
use std::{
    collections::VecDeque,
    ptr,
    sync::{Arc, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, GlobalFree, HGLOBAL, HWND},
    Globalization::MultiByteToWideChar,
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
        Shell::DragQueryFileW,
        WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, DispatchMessageW, HWND_MESSAGE, MSG, PM_REMOVE,
            PeekMessageW, TranslateMessage, WM_QUIT,
        },
    },
};

enum Command {
    Paste {
        text: String,
        route: Option<super::driver::Route>,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Stop,
}
#[derive(Clone)]
pub(super) struct Client(mpsc::Sender<Command>);
impl Client {
    pub fn paste(&self, text: String, route: Option<super::driver::Route>) -> Result<()> {
        let (reply, result) = mpsc::channel();
        self.0
            .send(Command::Paste { text, route, reply })
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
    let available = |format| unsafe { IsClipboardFormatAvailable(format) } != 0;
    if available(13) {
        return Ok(Some(Snapshot::Text(formats::terminated_utf16(&read(13)?)?)));
    }
    let html = registered("HTML Format")?;
    if available(html) {
        return Ok(Some(formats::parse_html(&read(html)?)?));
    }
    let rtf = registered("Rich Text Format")?;
    if available(rtf) {
        return Ok(Some(Snapshot::Rtf(
            formats::nul_bytes(&read(rtf)?).to_vec(),
        )));
    }
    if available(15) {
        let handle = unsafe { GetClipboardData(15) };
        ensure!(!handle.is_null(), "GetClipboardData(CF_HDROP) failed");
        let count = unsafe { DragQueryFileW(handle, u32::MAX, ptr::null_mut(), 0) };
        let mut paths = Vec::new();
        for index in 0..count {
            let mut path = [0u16; 4096];
            let length = unsafe { DragQueryFileW(handle, index, path.as_mut_ptr(), 4096) };
            if length != 0 {
                let end = path
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(path.len());
                paths.push(path[..end].to_vec());
            }
        }
        return Ok(Some(Snapshot::Files(paths)));
    }
    let wide_file = registered("FileNameW")?;
    if available(wide_file) {
        return Ok(Some(Snapshot::Files(vec![formats::terminated_utf16(
            &read(wide_file)?,
        )?])));
    }
    let ansi_file = registered("FileName")?;
    if available(ansi_file) {
        let bytes = read(ansi_file)?;
        let bytes = formats::nul_bytes(&bytes);
        let length =
            i32::try_from(bytes.len()).context("Clipboard file name exceeds Win32 length")?;
        let size = unsafe { MultiByteToWideChar(0, 0, bytes.as_ptr(), length, ptr::null_mut(), 0) };
        ensure!(size > 0, "Clipboard ANSI filename conversion failed");
        let mut wide = vec![0; size as usize];
        ensure!(
            unsafe { MultiByteToWideChar(0, 0, bytes.as_ptr(), length, wide.as_mut_ptr(), size) }
                == size,
            "Clipboard ANSI filename conversion changed length"
        );
        return Ok(Some(Snapshot::Files(vec![wide])));
    }
    let png = registered("PNG")?;
    if available(png) {
        let bytes = read(png)?;
        if !bytes.is_empty() {
            return Ok(Some(Snapshot::Png(bytes)));
        }
    }
    if available(8) {
        return Ok(Some(Snapshot::Png(dib::png(&read(8)?)?)));
    }
    Ok(None)
}

fn replace(window: HWND, snapshot: Option<&Snapshot>, mutated: &mut bool) -> Result<()> {
    let payloads = match snapshot {
        None => Vec::new(),
        Some(Snapshot::Text(text)) => vec![(13, formats::text_bytes(text))],
        Some(Snapshot::Html { html, source_url }) => vec![(
            registered("HTML Format")?,
            formats::html_bytes(html, source_url)?,
        )],
        Some(Snapshot::Rtf(bytes)) => vec![(registered("Rich Text Format")?, bytes.clone())],
        Some(Snapshot::Files(paths)) => vec![(15, formats::files_bytes(paths))],
        Some(Snapshot::Png(png)) => {
            let (png, dib) = formats::image_payloads(png)?;
            vec![(registered("PNG")?, png), (17, dib)]
        }
    };
    let allocations = payloads
        .iter()
        .map(|(format, bytes)| Ok((*format, Allocation::copy(bytes)?)))
        .collect::<Result<Vec<_>>>()?;
    let _clipboard = Clipboard::acquire(window)?;
    ensure!(unsafe { EmptyClipboard() } != 0, "EmptyClipboard failed");
    *mutated = true;
    let errors = allocations
        .into_iter()
        .filter_map(|(format, allocation)| {
            allocation
                .publish(format)
                .err()
                .map(|error| format!("{error:#}"))
        })
        .collect::<Vec<_>>();
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

fn paste_keys(route: Option<&super::driver::Route>) -> Result<()> {
    let mut input = Input::open(
        route,
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
    .map(|(_, scan, flag)| (scan, flag))
    .collect::<Vec<_>>();
    let result = keys::run(
        &modifiers,
        |operation| match operation {
            keys::Operation::Key { key, retain } => {
                input.retain_held_outputs = retain;
                input.keyboard(&key)
            }
            keys::Operation::ReleaseHeld => {
                input.release_held().map_err(|error| format!("{error:#}"))
            }
        },
        |ms| thread::sleep(Duration::from_millis(ms)),
    );
    let mut errors = result.err().into_iter().collect::<Vec<_>>();
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
    let result = keys::guarded(|| -> Result<()> {
        loop {
            // This thread owns the message-only clipboard window. Service incoming
            // window messages even when no macro command is pending.
            for _ in 0..64 {
                let mut message = MSG::default();
                if unsafe { PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) } == 0 {
                    break;
                }
                if message.message == WM_QUIT {
                    return Ok(());
                }
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            let wait = restoration
                .pending
                .front()
                .map_or(Duration::from_millis(16), |deadline| {
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(16))
                });
            let command = match receiver.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => Some(Command::Stop),
            };
            match command {
                Some(Command::Paste { text, route, reply }) => {
                    let mut began = false;
                    let mut mutated = false;
                    let result = keys::guarded(|| {
                        restoration.begin(|| snapshot(window.0))?;
                        began = true;
                        let text = text.encode_utf16().collect();
                        // Source writes text before opening the paste endpoint. A
                        // failed endpoint still owns restoration of that mutation.
                        let written = replace(window.0, Some(&Snapshot::Text(text)), &mut mutated);
                        written.and_then(|()| paste_keys(route.as_ref()))
                    });
                    if began {
                        restoration.changed(mutated, Instant::now());
                    }
                    let panicked = result.is_err();
                    let result = result.map_err(anyhow::Error::msg).and_then(|result| result);
                    let _ = reply.send(result.map_err(|error| format!("{error:#}")));
                    ensure!(!panicked, "Clipboard paste owner panicked");
                }
                Some(Command::Stop) => return Ok(()),
                None => {
                    // Keep a failed restore and its bytes owned until shutdown;
                    // subsequent pastes report that failure instead of overwriting
                    // the recovery snapshot or pretending restoration succeeded.
                    if restoration
                        .pending
                        .front()
                        .is_some_and(|deadline| *deadline <= Instant::now())
                        && let Err(error) = restoration.restore(Instant::now(), false, |original| {
                            replace(window.0, original, &mut false)
                        })
                    {
                        delayed_failure = Some(format!("{error:#}"));
                    }
                }
            }
        }
    })
    .map_err(anyhow::Error::msg)
    .and_then(|result| result);
    let cleanup = keys::guarded(|| {
        restoration.restore(Instant::now(), true, |original| {
            replace(window.0, original, &mut false)
        })
    })
    .map_err(anyhow::Error::msg)
    .and_then(|result| result);
    let mut errors = Vec::new();
    if let Err(error) = result {
        errors.push(format!("Clipboard worker: {error:#}"));
    }
    if let Some(error) = delayed_failure {
        errors.push(format!("Earlier clipboard restoration: {error}"));
    }
    if let Err(error) = cleanup {
        errors.push(format!("Shutdown clipboard restoration: {error:#}"));
    }
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(text: &str) -> Option<Snapshot> {
        Some(Snapshot::Text(text.encode_utf16().collect()))
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
                let Snapshot::Text(text) = saved.unwrap() else {
                    panic!()
                };
                assert_eq!(String::from_utf16(text).unwrap(), "original");
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
                let Snapshot::Text(text) = saved.unwrap() else {
                    panic!()
                };
                assert_eq!(String::from_utf16(text).unwrap(), "original");
                Ok(())
            })
            .unwrap();
        assert!(state.failed.is_none());
    }
}
