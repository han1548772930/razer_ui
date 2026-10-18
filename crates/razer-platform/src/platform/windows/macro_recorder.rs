//! Thread-owned WH_KEYBOARD_LL / WH_MOUSE_LL input and cleanup.
use super::input_clock::RecorderClock;
use crate::macro_recorder::{Event, EventKind, Events, Mode, Session};
use anyhow::Context as _;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::GetLastError,
    System::Diagnostics::Debug::{
        FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW,
        IsDebuggerPresent,
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PM_REMOVE,
        PeekMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
        WH_MOUSE_LL, WM_QUIT,
    },
};

type Reply<T> = mpsc::Sender<Result<T, String>>;
enum Command {
    Start(Mode, Reply<()>),
    Stop(Reply<()>),
    Mode(Reply<Option<Mode>>),
    Events(Reply<Events>),
    Shutdown,
}

pub(crate) struct Owner {
    commands: mpsc::Sender<Command>,
    thread: Option<JoinHandle<anyhow::Result<()>>>,
}
impl Owner {
    pub(crate) fn spawn() -> anyhow::Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("razer-macro-recorder".into())
            .spawn(move || run(receiver))
            .context("Unable to start macro input owner")?;
        Ok(Self {
            commands,
            thread: Some(thread),
        })
    }
    fn call<T>(&self, command: impl FnOnce(Reply<T>) -> Command) -> anyhow::Result<T> {
        let (reply, receiver) = mpsc::channel();
        self.commands
            .send(command(reply))
            .context("Macro input owner exited")?;
        // Each reply belongs to this request. A timeout must not abandon an
        // accepted start that can subsequently acquire hooks without an owner.
        receiver
            .recv()
            .context("Macro input owner did not complete the operation")?
            .map_err(anyhow::Error::msg)
    }
    pub(crate) fn start(&self, mode: Mode) -> anyhow::Result<()> {
        self.call(|reply| Command::Start(mode, reply))
    }
    pub(crate) fn stop(&self) -> anyhow::Result<()> {
        self.call(Command::Stop)
    }
    pub(crate) fn mode(&self) -> anyhow::Result<Option<Mode>> {
        self.call(Command::Mode)
    }
    pub(crate) fn events(&self) -> anyhow::Result<Events> {
        self.call(Command::Events)
    }
    pub(crate) fn shutdown(&mut self) -> anyhow::Result<()> {
        if let Some(thread) = self.thread.take() {
            let _ = self.commands.send(Command::Shutdown);
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("Macro input owner panicked during cleanup"))??;
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Macro input cleanup: {error:#}");
        }
    }
}

/// A failed release keeps the actual handle for the next cleanup attempt.
struct Hooks<H> {
    keyboard: Option<H>,
    mouse: Option<H>,
}
impl<H: Copy> Hooks<H> {
    fn new() -> Self {
        Self {
            keyboard: None,
            mouse: None,
        }
    }
    fn install(
        &mut self,
        keyboard_only: bool,
        mut install: impl FnMut(bool) -> anyhow::Result<H>,
        unhook: impl FnMut(H) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.keyboard.is_none() && self.mouse.is_none(),
            "Macro input cleanup is incomplete"
        );
        self.keyboard = Some(install(true)?);
        if !keyboard_only {
            match install(false) {
                Ok(mouse) => self.mouse = Some(mouse),
                Err(error) => {
                    return match self.release(unhook) {
                        Ok(()) => Err(error),
                        Err(cleanup) => Err(anyhow::anyhow!(
                            "{error:#}; partial hook cleanup: {cleanup:#}"
                        )),
                    };
                }
            }
        }
        Ok(())
    }
    fn release(&mut self, mut unhook: impl FnMut(H) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        for hook in [&mut self.keyboard, &mut self.mouse] {
            if let Some(handle) = *hook {
                match unhook(handle) {
                    Ok(()) => *hook = None,
                    Err(error) => errors.push(format!("{error:#}")),
                }
            }
        }
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

fn hook_error(code: u32) -> anyhow::Error {
    let mut text = [0u16; 2048];
    // Source 121890 requests English system text, trims its trailing whitespace
    // and appends the original hexadecimal Win32 code.
    let size = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            code,
            0x409,
            text.as_mut_ptr(),
            text.len() as u32,
            std::ptr::null(),
        )
    };
    let description = if size == 0 {
        format!(
            "Error (0x{:X}) while retrieving error. (0x{code:X})",
            unsafe { GetLastError() }
        )
    } else {
        format!(
            "{} (0x{code:X})",
            String::from_utf16_lossy(&text[..size as usize]).trim()
        )
    };
    anyhow::anyhow!(
        serde_json::json!({"errorCode":code as i32,"errorString":description}).to_string()
    )
}
fn unhook(hook: HHOOK) -> anyhow::Result<()> {
    // The hook belongs to this thread, whose TLS context remains alive through
    // this call and whose callbacks cannot run concurrently with this command.
    if unsafe { UnhookWindowsHookEx(hook) } == 0 {
        return Err(hook_error(unsafe { GetLastError() }));
    }
    Ok(())
}

struct State {
    hooks: Hooks<HHOOK>,
    session: Session,
    clock: RecorderClock,
    events: VecDeque<Event>,
    overflow: bool,
}
impl State {
    fn new() -> Self {
        Self {
            hooks: Hooks::new(),
            session: Session::default(),
            clock: RecorderClock::new(),
            events: VecDeque::new(),
            overflow: false,
        }
    }
    fn emit(&mut self, kind: EventKind, tick: u64) {
        let arrival = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        if self.events.len() < 4096 {
            self.events.push_back(Event::new(kind, tick, arrival));
        } else {
            self.overflow = true;
        }
    }
    fn start(&mut self, mode: Mode) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.session.mode().is_none(),
            "macro recorder has already started"
        );
        // d9d60 omits the mouse hook when a debugger is already attached.
        let keyboard_only = unsafe { IsDebuggerPresent() } != 0;
        self.hooks.install(
            keyboard_only,
            |keyboard| {
                let (kind, callback) = if keyboard {
                    (
                        WH_KEYBOARD_LL,
                        Some(
                            keyboard_hook as unsafe extern "system" fn(i32, usize, isize) -> isize,
                        ),
                    )
                } else {
                    (
                        WH_MOUSE_LL,
                        Some(mouse_hook as unsafe extern "system" fn(i32, usize, isize) -> isize),
                    )
                };
                // Low-level hooks execute on the installing thread's message loop.
                // Both procedures have process lifetime; no vendor module is loaded.
                let handle = unsafe { SetWindowsHookExW(kind, callback, std::ptr::null_mut(), 0) };
                if handle.is_null() {
                    Err(hook_error(unsafe { GetLastError() }))
                } else {
                    Ok(handle)
                }
            },
            unhook,
        )?;
        let event = self.session.start(mode)?;
        let tick = self.clock.tick();
        self.emit(event, tick);
        Ok(())
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.session.mode().is_some(),
            "macro recorder has not started"
        );
        self.hooks.release(unhook)?;
        let tick = self.clock.tick();
        for event in self.session.stop()? {
            self.emit(event, tick);
        }
        Ok(())
    }
    fn cleanup(&mut self) -> anyhow::Result<()> {
        if self.session.mode().is_some() {
            self.stop()
        } else {
            self.hooks.release(unhook)
        }
    }
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static CALLBACK_LOSS: Cell<bool> = const { Cell::new(false) };
}

fn callback(run: impl FnOnce(&mut State)) {
    // Never unwind across the OS ABI. A lost event invalidates the recording;
    // the consumer receives overflow instead of silently saving a partial macro.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        STATE.with(|cell| match cell.try_borrow_mut() {
            Ok(mut state) => {
                if let Some(state) = state.as_mut() {
                    if state.session.mode().is_some() {
                        run(state);
                    }
                }
            }
            Err(_) => CALLBACK_LOSS.set(true),
        });
    }));
    if result.is_err() {
        CALLBACK_LOSS.set(true);
    }
}
unsafe extern "system" fn keyboard_hook(code: i32, message: usize, data: isize) -> isize {
    if code >= 0 && data != 0 {
        // Windows supplies this structure for nonnegative low-level hook codes;
        // it is copied before returning and never retained as a raw pointer.
        let input = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        callback(|state| {
            let wall = state.clock.wall_tick();
            let event = state.session.batch.keyboard(
                message,
                input.vkCode,
                input.scanCode,
                input.flags,
                wall,
            );
            state.emit(EventKind::Item { event }, wall as u64);
        });
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, message, data) }
}
unsafe extern "system" fn mouse_hook(code: i32, message: usize, data: isize) -> isize {
    if code >= 0 && data != 0 {
        // The MSLLHOOKSTRUCT is valid only for this callback invocation.
        let input = unsafe { &*(data as *const MSLLHOOKSTRUCT) };
        callback(|state| {
            let wall = state.clock.wall_tick();
            if let Some(event) =
                state
                    .session
                    .batch
                    .mouse(message, input.pt.x, input.pt.y, input.mouseData, wall)
            {
                state.emit(EventKind::Item { event }, wall as u64);
            }
        });
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, message, data) }
}

fn command(command: Command) -> bool {
    STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut().expect("Recorder thread owns its state");
        match command {
            Command::Start(mode, reply) => {
                let _ = reply.send(state.start(mode).map_err(|error| format!("{error:#}")));
            }
            Command::Stop(reply) => {
                let _ = reply.send(state.stop().map_err(|error| format!("{error:#}")));
            }
            Command::Mode(reply) => {
                let _ = reply.send(Ok(state.session.mode()));
            }
            Command::Events(reply) => {
                let events = state.events.drain(..state.events.len().min(512)).collect();
                let overflow = std::mem::take(&mut state.overflow) | CALLBACK_LOSS.replace(false);
                let _ = reply.send(Ok(Events { events, overflow }));
            }
            Command::Shutdown => return false,
        }
        true
    })
}

fn run(receiver: mpsc::Receiver<Command>) -> anyhow::Result<()> {
    STATE.with(|cell| *cell.borrow_mut() = Some(State::new()));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            // Bound each pump pass so sustained input cannot starve Stop.
            for _ in 0..128 {
                let mut message: MSG = unsafe { std::mem::zeroed() };
                if unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } == 0
                {
                    break;
                }
                if message.message == WM_QUIT {
                    return;
                }
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            match receiver.recv_timeout(Duration::from_millis(4)) {
                Ok(next) => {
                    if !command(next) {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }));
    // Stop and flush precede TLS removal and join. Cleanup errors are returned
    // to the lifecycle owner; a failed ordinary Stop retains the live handles.
    let cleanup = STATE.with(|cell| {
        let mut state = cell
            .borrow_mut()
            .take()
            .expect("Recorder thread owns its state");
        state.cleanup()
    });
    if result.is_err() {
        anyhow::bail!("Macro input owner panicked; cleanup: {cleanup:?}");
    }
    cleanup
}

#[cfg(test)]
#[path = "macro_recorder_tests.rs"]
mod tests;
