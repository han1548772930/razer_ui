//! A capture lifetime owns a current-thread WH_GETMESSAGE hook for one HWND.
//! This preserves VK/location data GPUI discards. No low-level/global hook,
//! device redirect, text recording, background capture or message injection.
use super::keyboard::RawKey;
use gpui_kit::Window;

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        cell::{Cell, RefCell},
        ptr,
        rc::{Rc, Weak},
        time::Instant,
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::*,
    };

    struct State {
        hwnd: HWND,
        latest: Option<RawKey>,
        stream: Option<Vec<RawKey>>,
        overflowed: bool,
    }
    thread_local! {
        static ACTIVE: RefCell<Weak<RefCell<State>>> = const { RefCell::new(Weak::new()) };
        static UNHOOK_FAILED: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) struct Capture {
        hook: HHOOK,
        state: Rc<RefCell<State>>,
    }
    impl Capture {
        pub(super) fn start(window: &Window) -> Option<Self> {
            Self::start_inner(window, false)
        }
        pub(super) fn start_stream(window: &Window) -> Option<Self> {
            Self::start_inner(window, true)
        }
        fn start_inner(window: &Window, stream: bool) -> Option<Self> {
            let RawWindowHandle::Win32(handle) =
                HasWindowHandle::window_handle(window).ok()?.as_raw()
            else {
                return None;
            };
            // At most one editor on this UI thread owns the hook.
            if UNHOOK_FAILED.try_with(Cell::get).ok()?
                || ACTIVE
                    .try_with(|slot| slot.borrow().upgrade().is_some())
                    .ok()?
            {
                return None;
            }
            let state = Rc::new(RefCell::new(State {
                hwnd: handle.hwnd.get() as HWND,
                latest: None,
                stream: stream.then(Vec::new),
                overflowed: false,
            }));
            let hook = unsafe {
                SetWindowsHookExW(
                    WH_GETMESSAGE,
                    Some(message),
                    ptr::null_mut(),
                    GetCurrentThreadId(),
                )
            };
            if hook.is_null() {
                return None;
            }
            ACTIVE.with(|slot| *slot.borrow_mut() = Rc::downgrade(&state));
            Some(Self { hook, state })
        }
        pub(super) fn take(&mut self) -> Option<RawKey> {
            self.state.borrow_mut().latest.take()
        }
        pub(super) fn take_all(&mut self) -> Option<Vec<RawKey>> {
            let mut state = self.state.borrow_mut();
            if state.overflowed {
                return None;
            }
            state.stream.as_mut().map(std::mem::take)
        }
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            // Rc keeps the guard on the installing thread. Clear callback data
            // first; even an OS unhook failure leaves a forwarding-only hook.
            let _ = ACTIVE.try_with(|slot| *slot.borrow_mut() = Weak::new());
            if unsafe { UnhookWindowsHookEx(self.hook) } == 0 {
                // Never let a surviving old callback bind to a future session.
                let _ = UNHOOK_FAILED.try_with(|failed| failed.set(true));
            }
        }
    }

    unsafe extern "system" fn message(code: i32, removed: WPARAM, value: LPARAM) -> LRESULT {
        if code >= 0 && removed == PM_REMOVE as usize && value != 0 {
            // WH_GETMESSAGE supplies a live MSG pointer for this callback only.
            let msg = unsafe { &mut *(value as *mut MSG) };
            let _ = ACTIVE.try_with(|slot| {
                let Ok(slot) = slot.try_borrow() else { return };
                let Some(state) = slot.upgrade() else { return };
                let Ok(mut state) = state.try_borrow_mut() else {
                    return;
                };
                if msg.hwnd != state.hwnd {
                    return;
                }
                let down = matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN);
                let up = matches!(msg.message, WM_KEYUP | WM_SYSKEYUP);
                let active = unsafe { GetForegroundWindow() } == state.hwnd;
                if (down || up) && (active || state.stream.is_none() && up && msg.wParam == 44) {
                    let event = RawKey {
                        code: msg.wParam as u16,
                        scan: ((msg.lParam as usize >> 16) & 0xff) as u8,
                        extended: msg.lParam as usize & (1 << 24) != 0,
                        down,
                        at: Instant::now(),
                    };
                    if let Some(stream) = &mut state.stream {
                        // Do not silently lose modifier releases. The owner
                        // cancels capture after overflow rather than committing
                        // a partial shortcut from a truncated input sequence.
                        if stream.len() < 1024 {
                            stream.push(event);
                        } else {
                            state.overflowed = true;
                        }
                    } else if super::super::keyboard::accepts(&event) {
                        state.latest = Some(event);
                    }
                    // Equivalent to the source's capture preventDefault and
                    // no-browser-input handler, only for this focused editor.
                    msg.message = WM_NULL;
                } else if active
                    && matches!(
                        msg.message,
                        WM_CHAR | WM_SYSCHAR | WM_DEADCHAR | WM_SYSDEADCHAR
                    )
                {
                    msg.message = WM_NULL;
                }
            });
        }
        unsafe { CallNextHookEx(ptr::null_mut(), code, removed, value) }
    }
}

pub(super) struct Capture {
    #[cfg(target_os = "windows")]
    inner: native::Capture,
}
impl Capture {
    pub(super) fn start_stream(window: &Window) -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            Some(Self {
                inner: native::Capture::start_stream(window)?,
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
            None
        }
    }
    pub(super) fn take_all(&mut self) -> Option<Vec<RawKey>> {
        #[cfg(target_os = "windows")]
        {
            self.inner.take_all()
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    pub(super) fn start(window: &Window) -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            Some(Self {
                inner: native::Capture::start(window)?,
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
            None
        }
    }
    pub(super) fn take(&mut self) -> Option<RawKey> {
        #[cfg(target_os = "windows")]
        {
            self.inner.take()
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
}
