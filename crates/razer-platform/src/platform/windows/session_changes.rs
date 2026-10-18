//! Windows adapter for Electron powerMonitor lock-screen/unlock-screen.
//! The worker owns an invisible window and its WTS registration until shutdown.
//! Suspend/resume: current SysUtilsNative.dll RVA 0x6c270/0x6bd20.
//! See docs/re/evidence/sysutils-lifecycle-ida-current.json. No vendor DLL loads.
use crate::session_changes::SessionChange;
use async_channel::Sender;
use std::{
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};
use windows_sys::Win32::System::Power::{
    RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        RemoteDesktop::{
            NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
            WTSUnRegisterSessionNotification,
        },
    },
    UI::WindowsAndMessaging::*,
};

const STOP: u32 = WM_APP + 173;
const SESSION_LOCK: usize = 0x7;
const SESSION_UNLOCK: usize = 0x8;
// A missed wakeup must not leave shutdown waiting in a native message receive.
const STOP_RECHECK_MS: u32 = 100;
static NEXT_WINDOW: AtomicUsize = AtomicUsize::new(1);

#[derive(Default)]
struct Shared {
    hwnd: Mutex<usize>,
    stopped: AtomicBool,
}
pub(crate) struct Owner {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<Result<(), String>>>,
}
impl Owner {
    pub(crate) fn start(events: Sender<SessionChange>) -> Result<Self, String> {
        Self::start_worker(events, run)
    }

    fn start_worker(
        events: Sender<SessionChange>,
        run: impl FnOnce(&Shared, &Sender<SessionChange>) -> Result<(), String> + Send + 'static,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let state = shared.clone();
        let worker = std::thread::Builder::new()
            .name("razer-session-events".into())
            .spawn(move || {
                let result = run(&state, &events);
                if let Err(error) = &result {
                    let _ = events.try_send(SessionChange::Failed(error.clone()));
                }
                result
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    fn signal_stop(&self) -> Result<(), String> {
        self.signal_stop_with(post_stop)
    }

    fn signal_stop_with(
        &self,
        wake: impl FnOnce(HWND) -> Result<(), String>,
    ) -> Result<(), String> {
        self.shared.stopped.store(true, Ordering::Release);
        let owned = match self.shared.hwnd.try_lock() {
            Ok(owned) => owned,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            // Publication and destruction remain on the worker. Its bounded
            // message wait observes stopped even when this wake cannot be sent.
            Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
        };
        // The mutex excludes DestroyWindow and HWND reuse until posting ends.
        wake(*owned as HWND)
    }

    pub(crate) fn shutdown(self) -> Result<(), String> {
        self.shutdown_with(post_stop)
    }

    fn shutdown_with(
        mut self,
        wake: impl FnOnce(HWND) -> Result<(), String>,
    ) -> Result<(), String> {
        let signaled = self.signal_stop_with(wake);
        let completed = self
            .worker
            .take()
            .expect("Session monitor owns its worker")
            .join()
            .map_err(|_| "Session event worker panicked".to_owned())
            .and_then(|result| result);
        match (signaled, completed) {
            (Ok(()), result) | (result, Ok(())) => result,
            (Err(signal), Err(cleanup)) => Err(format!("{signal}; {cleanup}")),
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if self.worker.is_some() {
            if let Err(error) = self.signal_stop() {
                eprintln!("[session observer] stop notification failed: {error}");
            }
        }
    }
}

fn post_stop(hwnd: HWND) -> Result<(), String> {
    if !hwnd.is_null() && unsafe { PostMessageW(hwnd, STOP, 0, 0) } == 0 {
        return Err(format!("PostMessageW: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

enum MessageCycle {
    Continue,
    Quit,
}

fn pump_until_stopped(
    stopped: &AtomicBool,
    mut wait_and_dispatch: impl FnMut(u32) -> Result<MessageCycle, String>,
) -> Result<(), String> {
    while !stopped.load(Ordering::Acquire) {
        if matches!(wait_and_dispatch(STOP_RECHECK_MS)?, MessageCycle::Quit) {
            break;
        }
    }
    Ok(())
}

fn wait_and_dispatch(milliseconds: u32) -> Result<MessageCycle, String> {
    // No wait handles are borrowed; native input wakes promptly and timeout
    // gives the owner another stopped check if posting the STOP message failed.
    match unsafe {
        MsgWaitForMultipleObjectsEx(0, null(), milliseconds, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
    } {
        WAIT_OBJECT_0 => {}
        WAIT_TIMEOUT => return Ok(MessageCycle::Continue),
        WAIT_FAILED => {
            return Err(format!(
                "MsgWaitForMultipleObjectsEx: {}",
                std::io::Error::last_os_error()
            ));
        }
        status => return Err(format!("Session message wait returned {status:#x}")),
    }
    let mut message: MSG = unsafe { std::mem::zeroed() };
    if unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } == 0 {
        return Ok(MessageCycle::Continue);
    }
    if message.message == WM_QUIT {
        return Ok(MessageCycle::Quit);
    }
    // One dispatch per gate check also keeps a busy queue from starving stop.
    unsafe {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    Ok(MessageCycle::Continue)
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(l as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    if message == STOP {
        unsafe {
            PostQuitMessage(0);
        }
        return 0;
    }
    if message == WM_WTSSESSION_CHANGE {
        let events =
            unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Sender<SessionChange>;
        if !events.is_null() {
            let event = match w {
                SESSION_LOCK => Some(SessionChange::Locked),
                SESSION_UNLOCK => Some(SessionChange::Unlocked),
                _ => None,
            };
            if let Some(event) = event {
                let _ = unsafe { &*events }.try_send(event);
            }
        }
        return 0;
    }
    if message == WM_POWERBROADCAST {
        let events =
            unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Sender<SessionChange>;
        // The DLL emits resume only for 7, not PBT_APMRESUMEAUTOMATIC (18).
        let event = match w {
            value if value == PBT_APMSUSPEND as usize => Some(SessionChange::Suspended),
            value if value == PBT_APMRESUMESUSPEND as usize => Some(SessionChange::Resumed),
            _ => None,
        };
        if !events.is_null() {
            if let Some(event) = event {
                let _ = unsafe { &*events }.try_send(event);
            }
        }
    }
    if message == WM_NCDESTROY {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
    }
    unsafe { DefWindowProcW(hwnd, message, w, l) }
}

fn run(shared: &Shared, events: &Sender<SessionChange>) -> Result<(), String> {
    let class: Vec<u16> = format!(
        "RazerUiSessionEvents-{}-{}",
        std::process::id(),
        NEXT_WINDOW.fetch_add(1, Ordering::Relaxed)
    )
    .encode_utf16()
    .chain(Some(0))
    .collect();
    let instance = unsafe { GetModuleHandleW(null()) };
    if instance.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let wc = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // The borrowed sender address stays valid on this worker until after DestroyWindow.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            events as *const Sender<SessionChange> as *const _,
        )
    };
    if hwnd.is_null() {
        let error = std::io::Error::last_os_error().to_string();
        unsafe {
            UnregisterClassW(class.as_ptr(), instance);
        }
        return Err(error);
    }
    let registered = unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) } != 0;
    let mut result = if registered {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    };
    let power = unsafe { RegisterSuspendResumeNotification(hwnd, DEVICE_NOTIFY_WINDOW_HANDLE) };
    // windows-sys models HPOWERNOTIFY as isize; zero is the failed-registration handle.
    if power == 0 && result.is_ok() {
        result = Err(std::io::Error::last_os_error().to_string());
    }
    *shared
        .hwnd
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = hwnd as usize;
    if result.is_ok() {
        result = pump_until_stopped(&shared.stopped, wait_and_dispatch);
    }
    // Serialize destruction with Drop posting to avoid a recycled HWND race.
    let mut owned = shared
        .hwnd
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *owned = 0;
    let mut cleanup_errors = Vec::new();
    unsafe {
        if power != 0 && UnregisterSuspendResumeNotification(power) == 0 {
            cleanup_errors.push(format!(
                "UnregisterSuspendResumeNotification: {}",
                std::io::Error::last_os_error()
            ));
        }
        if registered && WTSUnRegisterSessionNotification(hwnd) == 0 {
            cleanup_errors.push(format!(
                "WTSUnRegisterSessionNotification: {}",
                std::io::Error::last_os_error()
            ));
        }
        if DestroyWindow(hwnd) == 0 {
            cleanup_errors.push(format!(
                "DestroyWindow: {}",
                std::io::Error::last_os_error()
            ));
            // Message dispatch has stopped on this owning thread. A failed
            // destroy must not leave the borrowed sender reachable after return.
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        if UnregisterClassW(class.as_ptr(), instance) == 0 {
            cleanup_errors.push(format!(
                "UnregisterClassW: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    if cleanup_errors.is_empty() {
        result
    } else {
        let cleanup = cleanup_errors.join("; ");
        Err(match result {
            Ok(()) => cleanup,
            Err(error) => format!("{error}; {cleanup}"),
        })
    }
}

#[cfg(test)]
#[path = "session_changes/tests.rs"]
mod tests;
