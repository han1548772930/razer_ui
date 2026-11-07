//! Current SysUtils RVAs 0x6c080/0x6a5b0 own idle, display and resume events;
//! 0x6bc60 counts only active child displays. No vendor DLL is loaded.
use super::session_changes::{pump_until_stopped, wait_and_dispatch};
use crate::power_idle::Event;
use async_channel::Sender;
use std::{
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{
        DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICEW, EnumDisplayDevicesW, EnumDisplayMonitors,
        GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    },
    System::{
        LibraryLoader::GetModuleHandleW,
        Power::{
            POWERBROADCAST_SETTING, RegisterPowerSettingNotification,
            RegisterSuspendResumeNotification, UnregisterPowerSettingNotification,
            UnregisterSuspendResumeNotification,
        },
        RemoteDesktop::{
            NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
            WTSUnRegisterSessionNotification,
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
const SESSION_UNLOCK: usize = 8;
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
        Self::start_worker(events, run)
    }
    fn start_worker(
        events: Sender<Event>,
        run: impl FnOnce(&Shared, &Sender<Event>, &SyncSender<Result<(), String>>) -> Result<(), String>
        + Send
        + 'static,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let state = shared.clone();
        let (ready, startup) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("razer-power-idle".into())
            .spawn(move || {
                let result = run(&state, &events, &ready);
                if let Err(error) = &result {
                    let _ = events.try_send(Event::Failed(error.clone()));
                    let _ = ready.send(Err(error.clone()));
                }
                result
            })
            .map_err(|error| error.to_string())?;
        let mut owner = Self {
            shared,
            worker: Some(worker),
        };
        match startup
            .recv()
            .map_err(|_| "Power-idle worker ended before initialization".to_owned())
            .and_then(|result| result)
        {
            Ok(()) => Ok(owner),
            Err(error) => {
                // The worker result retains initialization and release errors.
                owner.finish(post_stop).and(Err(error))
            }
        }
    }
    fn signal_stop(&self, wake: impl FnOnce(HWND) -> Result<(), String>) -> Result<(), String> {
        self.shared.stopped.store(true, Ordering::Release);
        let owned = match self.shared.hwnd.try_lock() {
            Ok(owned) => owned,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
        };
        // The mutex excludes DestroyWindow and HWND reuse until posting ends.
        wake(*owned as HWND)
    }
    pub(crate) fn stop(mut self) -> Result<(), String> {
        self.finish(post_stop)
    }
    fn finish(&mut self, wake: impl FnOnce(HWND) -> Result<(), String>) -> Result<(), String> {
        let signaled = self.signal_stop(wake);
        let completed = self
            .worker
            .take()
            .expect("monitor owns worker")
            .join()
            .map_err(|_| "Power-idle worker panicked".to_owned())
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
            if let Err(error) = self.finish(post_stop) {
                eprintln!("[power-idle observer] {error}");
            }
        }
    }
}
fn post_stop(hwnd: HWND) -> Result<(), String> {
    if !hwnd.is_null() && unsafe { PostMessageW(hwnd, STOP, 0, 0) } == 0 {
        return Err(api_error("PostMessageW"));
    }
    Ok(())
}
fn api_error(api: &str) -> String {
    format!("{api}: {}", std::io::Error::last_os_error())
}
struct Context<'a> {
    events: &'a Sender<Event>,
    stopped: &'a AtomicBool,
    display: Option<u8>,
    lid_closed: bool,
    lid_timer_active: bool,
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
    fn lid_monitors(&mut self, count: Result<u32, String>) {
        match count {
            Ok(0) if self.display.is_some_and(|state| matches!(state, 1 | 2)) => self.display(0),
            Ok(_) => {}
            Err(error) => {
                let _ = self.events.try_send(Event::Failed(error));
            }
        }
    }
    fn lifecycle(&mut self, message: u32, parameter: usize) {
        if let Some(event) = lifecycle_event(message, parameter) {
            if event == Event::Resume && self.display == Some(0) {
                self.display = Some(1);
            }
            let _ = self.events.try_send(event);
        }
    }
}
#[derive(Default)]
struct MonitorCount {
    active: u32,
    failure: Option<String>,
}
impl MonitorCount {
    fn observe(&mut self, flags: Result<u32, String>) {
        match flags {
            Ok(flags) if flags & DISPLAY_DEVICE_ACTIVE != 0 => self.active += 1,
            Ok(_) => {}
            Err(error) => {
                self.failure.get_or_insert(error);
            }
        }
    }
    fn finish(self) -> Result<u32, String> {
        match self.failure {
            Some(error) => Err(error),
            None => Ok(self.active),
        }
    }
}
fn idle_event(tick: u64, last_input: u32) -> Event {
    let milliseconds = (tick as u32).wrapping_sub(last_input);
    if milliseconds > i32::MAX as u32 {
        Event::Failed(format!(
            "Idle duration {milliseconds} exceeds the source signed range"
        ))
    } else {
        Event::Idle { milliseconds }
    }
}
fn lifecycle_event(message: u32, parameter: usize) -> Option<Event> {
    match (message, parameter) {
        (WM_POWERBROADCAST, value) if value == PBT_APMRESUMESUSPEND as usize => Some(Event::Resume),
        (WM_WTSSESSION_CHANGE, SESSION_UNLOCK) => Some(Event::ScreenUnlocked),
        _ => None,
    }
}
fn monitor_flags(monitor: HMONITOR) -> Result<u32, String> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) } == 0 {
        return Err(api_error("GetMonitorInfoW"));
    }
    let mut device = DISPLAY_DEVICEW::default();
    device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    if unsafe { EnumDisplayDevicesW(info.szDevice.as_ptr(), 0, &mut device, 0) } == 0 {
        return Err(api_error("EnumDisplayDevicesW"));
    }
    Ok(device.StateFlags)
}
unsafe extern "system" fn count_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut windows_sys::Win32::Foundation::RECT,
    count: LPARAM,
) -> i32 {
    // EnumDisplayMonitors invokes this synchronously with our live count owner.
    unsafe {
        (&mut *(count as *mut MonitorCount)).observe(monitor_flags(monitor));
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
        context.lifecycle(message, w);
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
                    let _ = context
                        .events
                        .try_send(idle_event(unsafe { GetTickCount64() }, input.dwTime));
                }
            }
            WM_TIMER if w == LID_TIMER => {
                if context.lid_timer_active {
                    if unsafe { KillTimer(hwnd, LID_TIMER) } == 0 {
                        let _ = context
                            .events
                            .try_send(Event::Failed(api_error("KillTimer(lid)")));
                        return 0;
                    }
                    context.lid_timer_active = false;
                }
                if context.lid_closed {
                    let mut count = MonitorCount::default();
                    let ok = unsafe {
                        EnumDisplayMonitors(
                            null_mut(),
                            null(),
                            Some(count_monitor),
                            &mut count as *mut _ as LPARAM,
                        )
                    } != 0;
                    let count = if ok {
                        count.finish()
                    } else {
                        Err(api_error("EnumDisplayMonitors"))
                    };
                    context.lid_monitors(count);
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
                        match u8::try_from(value) {
                            Ok(value) => context.display(value),
                            Err(_) => {
                                let _ = context.events.try_send(Event::Failed(format!(
                                    "Unknown display power state {value}"
                                )));
                            }
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
                            } else {
                                context.lid_timer_active = true;
                            }
                        } else {
                            let _ = context
                                .events
                                .try_send(Event::Failed(format!("Unknown lid state {value}")));
                        }
                    }
                } else {
                    let _ = context
                        .events
                        .try_send(Event::Failed("Power setting omits its DWORD state".into()));
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
fn run(
    shared: &Shared,
    events: &Sender<Event>,
    ready: &SyncSender<Result<(), String>>,
) -> Result<(), String> {
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
        lid_timer_active: false,
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
        if unsafe { UnregisterClassW(class.as_ptr(), instance) } == 0 {
            return Err(format!("{error}; {}", api_error("UnregisterClassW")));
        }
        return Err(error);
    }
    let mut errors = Vec::new();
    let display = unsafe {
        RegisterPowerSettingNotification(
            hwnd,
            &GUID_CONSOLE_DISPLAY_STATE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    };
    if display == 0 {
        errors.push(api_error("RegisterPowerSettingNotification(display)"));
    }
    let lid = unsafe {
        RegisterPowerSettingNotification(
            hwnd,
            &GUID_LIDSWITCH_STATE_CHANGE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )
    };
    if lid == 0 {
        errors.push(api_error("RegisterPowerSettingNotification(lid)"));
    }
    let resume = unsafe { RegisterSuspendResumeNotification(hwnd, DEVICE_NOTIFY_WINDOW_HANDLE) };
    if resume == 0 {
        errors.push(api_error("RegisterSuspendResumeNotification"));
    }
    let session = unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) } != 0;
    if !session {
        errors.push(api_error("WTSRegisterSessionNotification"));
    }
    let timer = unsafe { SetTimer(hwnd, IDLE_TIMER, 5000, None) };
    if timer == 0 {
        errors.push(api_error("SetTimer(idle)"));
    }
    *shared
        .hwnd
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = hwnd as usize;
    if errors.is_empty() {
        if ready.send(Ok(())).is_err() {
            shared.stopped.store(true, Ordering::Release);
        }
        if let Err(error) = pump_until_stopped(&shared.stopped, wait_and_dispatch) {
            errors.push(error);
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
        if timer != 0 && KillTimer(hwnd, IDLE_TIMER) == 0 {
            errors.push(api_error("KillTimer(idle)"));
        }
        if context.lid_timer_active && KillTimer(hwnd, LID_TIMER) == 0 {
            errors.push(api_error("KillTimer(lid)"));
        }
        if session && WTSUnRegisterSessionNotification(hwnd) == 0 {
            errors.push(api_error("WTSUnRegisterSessionNotification"));
        }
        if resume != 0 && UnregisterSuspendResumeNotification(resume) == 0 {
            errors.push(api_error("UnregisterSuspendResumeNotification"));
        }
        if display != 0 && UnregisterPowerSettingNotification(display) == 0 {
            errors.push(api_error("UnregisterPowerSettingNotification(display)"));
        }
        if lid != 0 && UnregisterPowerSettingNotification(lid) == 0 {
            errors.push(api_error("UnregisterPowerSettingNotification(lid)"));
        }
        // Clear callback access even if DestroyWindow reports failure. Windows
        // also destroys thread-owned windows when this worker exits.
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        if DestroyWindow(hwnd) == 0 {
            errors.push(api_error("DestroyWindow"));
        }
        if UnregisterClassW(class.as_ptr(), instance) == 0 {
            errors.push(api_error("UnregisterClassW"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
#[path = "power_idle_tests.rs"]
mod tests;
