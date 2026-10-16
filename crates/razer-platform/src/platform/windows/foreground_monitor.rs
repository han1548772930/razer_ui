//! Static-source replacement of SysUtilsNative monitor thread/window/hook.
use crate::foreground_monitor::ForegroundWindow;
use anyhow::{Context, Result, ensure};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::c_void,
    ptr::{null, null_mut},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

type Handle = *mut c_void;
type Window = *mut c_void;
type Hook = *mut c_void;
type WndProc = unsafe extern "system" fn(Window, u32, usize, isize) -> isize;
type EventProc = unsafe extern "system" fn(Hook, u32, Window, i32, i32, u32, u32);

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Message {
    hwnd: Window,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct WindowClass {
    size: u32,
    style: u32,
    proc: Option<WndProc>,
    class_extra: i32,
    window_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu: *const u16,
    name: *const u16,
    small_icon: Handle,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(class: *const WindowClass) -> u16;
    fn UnregisterClassW(name: *const u16, instance: Handle) -> i32;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Window,
        menu: Handle,
        instance: Handle,
        data: *const c_void,
    ) -> Window;
    fn DefWindowProcW(window: Window, msg: u32, w: usize, l: isize) -> isize;
    fn DestroyWindow(window: Window) -> i32;
    fn PostQuitMessage(code: i32);
    fn PostMessageW(window: Window, msg: u32, w: usize, l: isize) -> i32;
    fn GetMessageW(msg: *mut Message, window: Window, minimum: u32, maximum: u32) -> i32;
    fn TranslateMessage(msg: *const Message) -> i32;
    fn DispatchMessageW(msg: *const Message) -> isize;
    fn ShowWindow(window: Window, show: i32) -> i32;
    fn UpdateWindow(window: Window) -> i32;
    fn SetLayeredWindowAttributes(window: Window, color: u32, alpha: u8, flags: u32) -> i32;
    fn SetWindowPos(
        window: Window,
        after: Window,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn SetWinEventHook(
        min: u32,
        max: u32,
        module: Handle,
        proc: Option<EventProc>,
        pid: u32,
        tid: u32,
        flags: u32,
    ) -> Hook;
    fn UnhookWinEvent(hook: Hook) -> i32;
    fn GetForegroundWindow() -> Window;
    fn GetWindowThreadProcessId(window: Window, pid: *mut u32) -> u32;
    fn EnumChildWindows(
        window: Window,
        proc: Option<unsafe extern "system" fn(Window, isize) -> i32>,
        data: isize,
    ) -> i32;
    fn SetTimer(
        window: Window,
        id: usize,
        time: u32,
        proc: Option<unsafe extern "system" fn(Window, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(window: Window, id: usize) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        path: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
}

struct ThreadState {
    window: Window,
    previous: String,
    events: Arc<Mutex<VecDeque<(BTreeSet<String>, ForegroundWindow)>>>,
    subscribers: Arc<Mutex<BTreeSet<String>>>,
    failures: Arc<Mutex<Vec<String>>>,
}
thread_local! { static STATE: RefCell<Option<ThreadState>> = const { RefCell::new(None) }; }

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn image(window: Window) -> Option<String> {
    let mut pid = 0;
    if unsafe { GetWindowThreadProcessId(window, &mut pid) } == 0 {
        return None;
    }
    let process = unsafe { OpenProcess(0x1000, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut buffer = [0u16; 260];
    let mut length = 260;
    let result =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe { CloseHandle(process) };
    if result == 0 || length as usize >= buffer.len() {
        return None;
    }
    String::from_utf16(&buffer[..length as usize]).ok()
}

unsafe extern "system" fn child(window: Window, data: isize) -> i32 {
    let Some(path) = image(window) else {
        return 1;
    };
    if path.to_ascii_lowercase().contains("applicationframehost") {
        return 1;
    }
    unsafe { *(data as *mut Option<String>) = Some(path) };
    0
}

fn failure(message: String) {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            if let Ok(mut failures) = state.failures.lock() {
                failures.push(message);
            }
        }
    });
}

unsafe extern "system" fn event(
    _: Hook,
    kind: u32,
    mut window: Window,
    object: i32,
    child_id: i32,
    _: u32,
    _: u32,
) {
    if !matches!(kind, 3 | 9) || object != 0 || child_id != 0 || window.is_null() {
        return;
    }
    let Some(mut path) = image(window) else {
        return;
    };
    let previous = STATE.with(|state| state.borrow().as_ref().map(|s| s.previous.clone()));
    let Some(previous) = previous else {
        return;
    };
    let foreground = unsafe { GetForegroundWindow() };
    if path != previous {
        if path.eq_ignore_ascii_case("c:\\windows\\explorer.exe") {
            thread::sleep(Duration::from_millis(100));
            let current = unsafe { GetForegroundWindow() };
            if current != window {
                if let Some(current_path) = image(current) {
                    path = current_path;
                    window = current;
                }
                if path == previous {
                    return;
                }
            }
        }
    } else if foreground != window {
        if let Some(current_path) = image(foreground) {
            path = current_path;
        }
        if path == previous {
            return;
        }
    } else if path.eq_ignore_ascii_case("c:\\windows\\explorer.exe") {
        thread::sleep(Duration::from_millis(100));
        let current = unsafe { GetForegroundWindow() };
        if current == window {
            return;
        }
        if let Some(current_path) = image(current) {
            path = current_path;
        }
        if path == previous {
            return;
        }
    } else {
        return;
    }

    let lowered = path.to_ascii_lowercase();
    if !window.is_null()
        && (lowered.contains("applicationframehost") || lowered.contains("wwahost"))
    {
        let mut selected: Option<String> = None;
        for attempt in 0..30 {
            let result = unsafe {
                EnumChildWindows(
                    window,
                    Some(child),
                    (&mut selected as *mut Option<String>) as isize,
                )
            };
            if result == 0 {
                // Native copies the (possibly empty) child buffer once Enum
                // stops; the child helper skips applicationframehost only.
                let resolved = selected.unwrap_or_default();
                if resolved.eq_ignore_ascii_case(&previous) {
                    return;
                }
                path = resolved;
                break;
            }
            thread::sleep(Duration::from_millis(100));
            if attempt == 29 {
                break;
            }
        }
    }
    let window = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.as_mut().map(|state| {
            state.previous = path;
            state.window
        })
    });
    if let Some(window) = window {
        if !window.is_null() && unsafe { SetTimer(window, 1, 300, None) } == 0 {
            failure(format!("foreground SetTimer failed: {}", unsafe {
                GetLastError()
            }));
        }
    }
}

unsafe extern "system" fn wnd(window: Window, message: u32, w: usize, l: isize) -> isize {
    match message {
        0x113 if w == 1 => {
            unsafe { KillTimer(window, 1) };
            STATE.with(|state| {
                if let Some(state) = state.borrow().as_ref() {
                    if let Ok(mut events) = state.events.lock() {
                        if let Ok(subscribers) = state.subscribers.lock() {
                            events.push_back((
                                subscribers.clone(),
                                ForegroundWindow::from_executable(&state.previous),
                            ));
                        }
                    }
                }
            });
            0
        }
        0x12 => {
            unsafe { DestroyWindow(window) };
            0
        }
        2 => {
            unsafe {
                KillTimer(window, 1);
                PostQuitMessage(0)
            };
            0
        }
        0x10 => 0, // Source WM_CLOSE logs without destroying the listener.
        _ => unsafe { DefWindowProcW(window, message, w, l) },
    }
}

pub(crate) struct Listener {
    window: usize,
    worker: Option<JoinHandle<()>>,
    events: Arc<Mutex<VecDeque<(BTreeSet<String>, ForegroundWindow)>>>,
    subscribers: Arc<Mutex<BTreeSet<String>>>,
    failures: Arc<Mutex<Vec<String>>>,
    pending: BTreeMap<String, Vec<ForegroundWindow>>,
}

impl Listener {
    pub(crate) fn start() -> Result<Self> {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let failures = Arc::new(Mutex::new(Vec::new()));
        let subscribers = Arc::new(Mutex::new(BTreeSet::new()));
        let audiences = subscribers.clone();
        let queue = events.clone();
        let errors = failures.clone();
        let (ready, reply) = mpsc::channel::<Result<usize, String>>();
        let worker = thread::Builder::new()
            .name("razer-foreground".into())
            .spawn(move || {
                let class = wide("RzMonitorForegroundWindowClass");
                let title = wide("RzMonitorForegroundWindow");
                let instance = unsafe { GetModuleHandleW(null()) };
                let descriptor = WindowClass {
                    size: std::mem::size_of::<WindowClass>() as u32,
                    style: 3,
                    proc: Some(wnd),
                    class_extra: 0,
                    window_extra: 0,
                    instance,
                    icon: null_mut(),
                    cursor: null_mut(),
                    background: null_mut(),
                    menu: null(),
                    name: class.as_ptr(),
                    small_icon: null_mut(),
                };
                if unsafe { RegisterClassExW(&descriptor) } == 0 {
                    let _ = ready.send(Err(format!("RegisterClassExW failed: {}", unsafe {
                        GetLastError()
                    })));
                    return;
                }
                let window = unsafe {
                    CreateWindowExW(
                        0x80088,
                        class.as_ptr(),
                        title.as_ptr(),
                        0x80000000,
                        i32::MIN,
                        i32::MIN,
                        440,
                        400,
                        null_mut(),
                        null_mut(),
                        instance,
                        null(),
                    )
                };
                if window.is_null() {
                    let _ = ready.send(Err(format!("CreateWindowExW failed: {}", unsafe {
                        GetLastError()
                    })));
                    unsafe { UnregisterClassW(class.as_ptr(), instance) };
                    return;
                }
                STATE.with(|s| {
                    *s.borrow_mut() = Some(ThreadState {
                        window,
                        previous: String::new(),
                        events: queue,
                        subscribers: audiences,
                        failures: errors,
                    })
                });
                unsafe {
                    ShowWindow(window, 1);
                    UpdateWindow(window);
                    SetLayeredWindowAttributes(window, 0, 1, 2);
                    SetWindowPos(window, null_mut(), 0, 0, 0, 0, 4)
                };
                let hook = unsafe { SetWinEventHook(3, 0x17, null_mut(), Some(event), 0, 0, 0) };
                if hook.is_null() {
                    let _ = ready.send(Err(format!("SetWinEventHook failed: {}", unsafe {
                        GetLastError()
                    })));
                    unsafe {
                        DestroyWindow(window);
                        UnregisterClassW(class.as_ptr(), instance)
                    };
                    STATE.with(|s| s.borrow_mut().take());
                    return;
                }
                if ready.send(Ok(window as usize)).is_ok() {
                    let mut message: Message = unsafe { std::mem::zeroed() };
                    loop {
                        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
                        if result <= 0 {
                            if result < 0 {
                                failure(format!("foreground GetMessageW failed: {}", unsafe {
                                    GetLastError()
                                }));
                            }
                            break;
                        }
                        unsafe {
                            TranslateMessage(&message);
                            DispatchMessageW(&message)
                        };
                    }
                } else {
                    unsafe { DestroyWindow(window) };
                }
                unsafe {
                    UnhookWinEvent(hook);
                    DestroyWindow(window);
                    UnregisterClassW(class.as_ptr(), instance)
                };
                STATE.with(|s| s.borrow_mut().take());
            })
            .context("foreground monitor thread creation failed")?;
        match reply
            .recv()
            .context("foreground monitor stopped before initialization")?
        {
            Ok(window) => Ok(Self {
                window,
                worker: Some(worker),
                events,
                subscribers,
                failures,
                pending: BTreeMap::new(),
            }),
            Err(error) => {
                let _ = worker.join();
                anyhow::bail!(error)
            }
        }
    }

    pub(crate) fn drain(
        &mut self,
        view: &str,
        subscribers: &BTreeSet<String>,
    ) -> Result<Vec<ForegroundWindow>> {
        let errors = self
            .failures
            .lock()
            .map_err(|_| anyhow::anyhow!("foreground diagnostics poisoned"))?;
        ensure!(
            errors.is_empty(),
            "foreground monitor failed: {}",
            errors.join("; ")
        );
        drop(errors);
        let events: Vec<_> = self
            .events
            .lock()
            .map_err(|_| anyhow::anyhow!("foreground queue poisoned"))?
            .drain(..)
            .collect();
        self.pending.retain(|url, _| subscribers.contains(url));
        for (audience, event) in events {
            for url in audience.intersection(subscribers) {
                self.pending
                    .entry(url.clone())
                    .or_default()
                    .push(event.clone());
            }
        }
        Ok(self.pending.remove(view).unwrap_or_default())
    }

    pub(crate) fn set_subscribers(&mut self, subscribers: &BTreeSet<String>) -> Result<()> {
        *self
            .subscribers
            .lock()
            .map_err(|_| anyhow::anyhow!("foreground subscription queue poisoned"))? =
            subscribers.clone();
        Ok(())
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        let Some(worker) = self.worker.as_ref() else {
            return Ok(());
        };
        if !worker.is_finished() {
            ensure!(
                unsafe { PostMessageW(self.window as Window, 0x12, 0, 0) } != 0,
                "foreground stop PostMessageW failed: {}",
                unsafe { GetLastError() }
            );
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("foreground monitor thread panicked"))?;
        }
        self.window = 0;
        Ok(())
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
