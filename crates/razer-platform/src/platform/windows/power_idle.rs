//! SysUtils RVA 0x6dd40 registers a 5000 ms idle timer and console-display/lid
//! power notifications; 0x6c270 maps display 0/1/2 without conflating dimmed/off.
use crate::power_idle::Event;
use async_channel::Sender;
use std::{
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR},
    System::{
        LibraryLoader::GetModuleHandleW,
        Power::{
            POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
            UnregisterPowerSettingNotification,
        },
        SystemInformation::GetTickCount64,
        SystemServices::{GUID_CONSOLE_DISPLAY_STATE, GUID_LIDSWITCH_STATE_CHANGE},
    },
    UI::{
        Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        WindowsAndMessaging::*,
    },
};
const STOP: u32 = WM_APP + 174;
const IDLE_TIMER: usize = 1001;
const LID_TIMER: usize = 1002;
static NEXT: AtomicUsize = AtomicUsize::new(1);
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
    pub(crate) fn start(events: Sender<Event>) -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let state = shared.clone();
        let worker = std::thread::Builder::new()
            .name("razer-power-idle".into())
            .spawn(move || {
                let result = run(&state, &events);
                if let Err(error) = &result {
                    let _ = events.try_send(Event::Failed(error.clone()));
                }
                result
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }
    fn signal_stop(&self) {
        self.shared.stopped.store(true, Ordering::Release);
        let owned = self
            .shared
            .hwnd
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *owned != 0 {
            // The mutex excludes DestroyWindow and HWND reuse until posting ends.
            unsafe {
                PostMessageW(*owned as HWND, STOP, 0, 0);
            }
        }
    }
    pub(crate) fn stop(mut self) -> Result<(), String> {
        self.signal_stop();
        self.worker
            .take()
            .expect("monitor owns worker")
            .join()
            .map_err(|_| "Power-idle worker panicked".to_owned())?
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.signal_stop();
    }
}
struct Context<'a> {
    events: &'a Sender<Event>,
    stopped: &'a AtomicBool,
    display: Option<u8>,
    lid_closed: bool,
}
fn same_guid(left: &windows_sys::core::GUID, right: &windows_sys::core::GUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}
impl Context<'_> {
    fn display(&mut self, value: u8) {
        self.display = Some(value);
        let event = match value {
            0 => Event::DisplayOff,
            1 => Event::DisplayOn,
            2 => Event::DisplayDimmed,
            _ => {
                let _ = self.events.try_send(Event::Failed(format!(
                    "Unknown display power state {value}"
                )));
                return;
            }
        };
        let _ = self.events.try_send(event);
    }
}
unsafe extern "system" fn count_monitor(
    _: HMONITOR,
    _: HDC,
    _: *mut windows_sys::Win32::Foundation::RECT,
    count: LPARAM,
) -> i32 {
    // EnumDisplayMonitors invokes this synchronously with our live u32 pointer.
    unsafe {
        *(count as *mut u32) += 1;
    }
    1
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        // lpCreateParams points to run's stack context until after DestroyWindow.
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
    let context = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Context<'_>;
    if !context.is_null() {
        let context = unsafe { &mut *context };
        if context.stopped.load(Ordering::Acquire) {
            unsafe {
                PostQuitMessage(0);
            }
            return 0;
        }
        match message {
            WM_TIMER if w == IDLE_TIMER => {
                let mut input = LASTINPUTINFO {
                    cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                    dwTime: 0,
                };
                if unsafe { GetLastInputInfo(&mut input) } == 0 {
                    let _ = context
                        .events
                        .try_send(Event::Failed(std::io::Error::last_os_error().to_string()));
                } else {
                    let milliseconds =
                        (unsafe { GetTickCount64() } as u32).wrapping_sub(input.dwTime);
                    let _ = context.events.try_send(Event::Idle { milliseconds });
                }
            }
            WM_TIMER if w == LID_TIMER => {
                unsafe {
                    KillTimer(hwnd, LID_TIMER);
                }
                if context.lid_closed {
                    let mut count = 0u32;
                    let ok = unsafe {
                        EnumDisplayMonitors(
                            null_mut(),
                            null(),
                            Some(count_monitor),
                            &mut count as *mut _ as LPARAM,
                        )
                    } != 0;
                    if (!ok || count == 0) && context.display != Some(0) {
                        context.display(0);
                    }
                }
            }
            WM_POWERBROADCAST if w == PBT_POWERSETTINGCHANGE as usize && l != 0 => {
                // Windows owns this synchronous POWERBROADCAST_SETTING payload;
                // DataLength validates the trailing DWORD before it is read.
                let setting = unsafe { &*(l as *const POWERBROADCAST_SETTING) };
                if setting.DataLength >= 4 {
                    let value =
                        unsafe { std::ptr::read_unaligned(setting.Data.as_ptr() as *const u32) };
                    if same_guid(&setting.PowerSetting, &GUID_CONSOLE_DISPLAY_STATE) {
                        if let Ok(value) = u8::try_from(value) {
                            context.display(value);
                        }
                    } else if same_guid(&setting.PowerSetting, &GUID_LIDSWITCH_STATE_CHANGE) {
                        context.lid_closed = value == 0;
                        if value == 1 {
                            context.display(1);
                        } else if value == 0 {
                            // The source waits 150 ms before counting external
                            // monitors. A timer keeps this callback responsive.
                            if unsafe { SetTimer(hwnd, LID_TIMER, 150, None) } == 0 {
                                let _ = context.events.try_send(Event::Failed(
                                    std::io::Error::last_os_error().to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            WM_QUERYENDSESSION => {
                let _ = context.events.try_send(Event::Shutdown);
                return 1;
            }
            _ => {}
        }
    }
    if message == WM_NCDESTROY {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
    }
    unsafe { DefWindowProcW(hwnd, message, w, l) }
}
fn run(shared: &Shared, events: &Sender<Event>) -> Result<(), String> {
    let class = format!(
        "RazerUiPowerIdle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
    .encode_utf16()
    .chain(Some(0))
    .collect::<Vec<_>>();
    let instance = unsafe { GetModuleHandleW(null()) };
    if instance.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let wc = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let mut context = Context {
        events,
        stopped: &shared.stopped,
        display: None,
        lid_closed: false,
    };
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
            &mut context as *mut _ as *const _,
        )
    };
    if hwnd.is_null() {
        let error = std::io::Error::last_os_error().to_string();
        unsafe {
            UnregisterClassW(class.as_ptr(), instance);
        }
        return Err(error);
    }
    let display = unsafe {
        RegisterPowerSettingNotification(
            hwnd,
            &GUID_CONSOLE_DISPLAY_STATE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    };
    let lid = unsafe {
        RegisterPowerSettingNotification(
            hwnd,
            &GUID_LIDSWITCH_STATE_CHANGE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    };
    let timer = unsafe { SetTimer(hwnd, IDLE_TIMER, 5000, None) };
    let mut result = if display == 0 || lid == 0 || timer == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    };
    *shared
        .hwnd
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = hwnd as usize;
    if result.is_ok() && !shared.stopped.load(Ordering::Acquire) {
        let mut message = MSG::default();
        loop {
            let received = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
            if received <= 0 {
                if received < 0 {
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
    let mut owned = shared
        .hwnd
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *owned = 0;
    // No callback can outlive context: the registration, timers, HWND and class
    // are all released on their owning thread before run returns.
    unsafe {
        KillTimer(hwnd, IDLE_TIMER);
        KillTimer(hwnd, LID_TIMER);
        if display != 0 && UnregisterPowerSettingNotification(display) == 0 {
            result = Err(std::io::Error::last_os_error().to_string());
        }
        if lid != 0 && UnregisterPowerSettingNotification(lid) == 0 {
            result = Err(std::io::Error::last_os_error().to_string());
        }
        if DestroyWindow(hwnd) == 0 {
            result = Err(std::io::Error::last_os_error().to_string());
        }
        if UnregisterClassW(class.as_ptr(), instance) == 0 {
            result = Err(std::io::Error::last_os_error().to_string());
        }
    }
    result
}
