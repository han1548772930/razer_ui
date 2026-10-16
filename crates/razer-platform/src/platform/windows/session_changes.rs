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
};
use windows_sys::Win32::System::Power::{
    RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
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
static NEXT_WINDOW: AtomicUsize = AtomicUsize::new(1);

#[derive(Default)]
struct Shared {
    hwnd: Mutex<usize>,
    stopped: AtomicBool,
}
pub(crate) struct Owner(Arc<Shared>);
impl Owner {
    pub(crate) fn start(events: Sender<SessionChange>) -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("razer-session-events".into())
            .spawn(move || {
                if let Err(error) = run(&worker, &events) {
                    let _ = events.try_send(SessionChange::Failed(error));
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self(shared))
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.0.stopped.store(true, Ordering::Release);
        let owned = self
            .0
            .hwnd
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let hwnd = *owned as HWND;
        if !hwnd.is_null() {
            // Cleanup remains on the worker thread; never block the UI in Drop.
            unsafe {
                PostMessageW(hwnd, STOP, 0, 0);
            }
        }
    }
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
    if result.is_ok() && !shared.stopped.load(Ordering::Acquire) {
        let mut message: MSG = unsafe { std::mem::zeroed() };
        loop {
            let status = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
            if status <= 0 {
                if status == -1 {
                    result = Err(std::io::Error::last_os_error().to_string());
                }
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    // Serialize destruction with Drop posting to avoid a recycled HWND race.
    let mut owned = shared
        .hwnd
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *owned = 0;
    unsafe {
        if power != 0 {
            UnregisterSuspendResumeNotification(power);
        }
        if registered {
            WTSUnRegisterSessionNotification(hwnd);
        }
        DestroyWindow(hwnd);
        UnregisterClassW(class.as_ptr(), instance);
    }
    result
}
