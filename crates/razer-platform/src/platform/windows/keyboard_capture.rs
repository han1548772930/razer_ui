//! WH_GETMESSAGE on the current UI thread preserves the original window's
//! WM_KEY* identity before GPUI normalizes it. This is a Rust OS adapter for
//! current UI5235 browser keyCode/location, not a vendor protocol extension.
use crate::keyboard_capture::KeyboardEvent;
use anyhow::{Context as _, ensure};
use std::{cell::RefCell, collections::VecDeque, marker::PhantomData, ptr::null_mut, rc::Rc};
use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetForegroundWindow, GetWindowThreadProcessId, HHOOK, MSG,
        SetWindowsHookExW, UnhookWindowsHookEx, WH_GETMESSAGE,
    },
};

struct State {
    target: usize,
    events: VecDeque<KeyboardEvent>,
    overflow: bool,
}
thread_local! { static CAPTURE: RefCell<Option<State>> = const { RefCell::new(None) }; }

unsafe extern "system" fn observe(code: i32, removed: WPARAM, raw: LPARAM) -> LRESULT {
    if code >= 0 && removed == 1 && raw != 0 {
        let message = unsafe { &*(raw as *const MSG) };
        CAPTURE.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                return;
            };
            let Some(state) = slot.as_mut() else {
                return;
            };
            if message.hwnd as usize != state.target
                || unsafe { GetForegroundWindow() } as usize != state.target
            {
                return;
            }
            if let Some(event) = KeyboardEvent::from_windows_message(
                message.message,
                message.wParam as u32,
                message.lParam as usize,
            ) {
                if state.events.len() >= 512 {
                    state.overflow = true;
                } else {
                    state.events.push_back(event);
                }
            }
        });
    }
    unsafe { CallNextHookEx(null_mut(), code, removed, raw) }
}

pub(crate) struct Hook {
    handle: HHOOK,
    _ui_thread: PhantomData<Rc<()>>,
}
impl Hook {
    pub(crate) fn start() -> anyhow::Result<Self> {
        ensure!(
            CAPTURE.with(|slot| slot.borrow().is_none()),
            "Another focused key capture owns this UI thread"
        );
        let target = unsafe { GetForegroundWindow() };
        ensure!(
            !target.is_null(),
            "Keyboard capture has no foreground window"
        );
        ensure!(
            unsafe { GetWindowThreadProcessId(target, null_mut()) }
                == unsafe { GetCurrentThreadId() },
            "Keyboard capture foreground window belongs to another UI thread"
        );
        let handle = unsafe {
            SetWindowsHookExW(
                WH_GETMESSAGE,
                Some(observe),
                null_mut(),
                GetCurrentThreadId(),
            )
        };
        ensure!(
            !handle.is_null(),
            "Install focused keyboard message observer: {}",
            std::io::Error::last_os_error()
        );
        CAPTURE.with(|slot| {
            *slot.borrow_mut() = Some(State {
                target: target as usize,
                events: VecDeque::new(),
                overflow: false,
            })
        });
        Ok(Self {
            handle,
            _ui_thread: PhantomData,
        })
    }
    pub(crate) fn drain(&mut self) -> anyhow::Result<Vec<KeyboardEvent>> {
        CAPTURE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let state = slot
                .as_mut()
                .context("Focused keyboard capture is closed")?;
            ensure!(!state.overflow, "Focused keyboard event buffer overflowed");
            Ok(state.events.drain(..).collect())
        })
    }
}
impl Drop for Hook {
    fn drop(&mut self) {
        CAPTURE.with(|slot| *slot.borrow_mut() = None);
        if unsafe { UnhookWindowsHookEx(self.handle) } == 0 {
            eprintln!(
                "Focused keyboard observer cleanup failed: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}
