//! Current mapping-engine RegisterHotKey/UnregisterHotKey semantics in Rust.
//! A dedicated thread owns registrations and events. No target DLL is loaded.
use crate::global_shortcuts::{Shortcut, os_modifiers};
use anyhow::{Context as _, ensure};
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::c_void,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::Duration,
};

#[repr(C)]
struct Message {
    hwnd: *mut c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: [i32; 2],
    private: u32,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterHotKey(window: *mut c_void, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(window: *mut c_void, id: i32) -> i32;
    fn PeekMessageW(
        message: *mut Message,
        window: *mut c_void,
        min: u32,
        max: u32,
        remove: u32,
    ) -> i32;
    fn GetKeyState(key: i32) -> i16;
}
type Reply = mpsc::Sender<Result<Vec<Shortcut>, String>>;
enum Command {
    Register(Shortcut, Reply),
    Unregister(u32, u32, Reply),
    Enable(bool, Reply),
    Registered(Reply),
    Stop,
}
pub(crate) struct Listener {
    command: mpsc::Sender<Command>,
    events: Arc<Mutex<VecDeque<Shortcut>>>,
    thread: Option<JoinHandle<()>>,
}
impl Listener {
    pub(crate) fn start() -> anyhow::Result<Self> {
        let (command, receiver) = mpsc::channel();
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let worker_events = events.clone();
        let thread = thread::Builder::new()
            .name("razer-global-shortcuts".into())
            .spawn(move || run(receiver, worker_events))
            .context("Unable to start shortcut owner thread")?;
        Ok(Self {
            command,
            events,
            thread: Some(thread),
        })
    }
    fn call(&self, make: impl FnOnce(Reply) -> Command) -> anyhow::Result<Vec<Shortcut>> {
        let (sender, receiver) = mpsc::channel();
        self.command
            .send(make(sender))
            .context("Shortcut owner has exited")?;
        receiver
            .recv_timeout(Duration::from_secs(4))
            .context("Shortcut owner did not confirm the OS operation")?
            .map_err(anyhow::Error::msg)
    }
    pub(crate) fn register(&self, shortcut: Shortcut) -> anyhow::Result<()> {
        self.call(|reply| Command::Register(shortcut, reply))
            .map(|_| ())
    }
    pub(crate) fn unregister(&self, key: u32, modifiers: u32) -> anyhow::Result<()> {
        self.call(|reply| Command::Unregister(key, modifiers, reply))
            .map(|_| ())
    }
    pub(crate) fn enable(&self, enable: bool) -> anyhow::Result<()> {
        self.call(|reply| Command::Enable(enable, reply))
            .map(|_| ())
    }
    pub(crate) fn registered(&self) -> anyhow::Result<Vec<Shortcut>> {
        self.call(Command::Registered)
    }
    pub(crate) fn drain(&self) -> anyhow::Result<Vec<Shortcut>> {
        Ok(self
            .events
            .lock()
            .map_err(|_| anyhow::anyhow!("Shortcut event lock poisoned"))?
            .drain(..)
            .collect())
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.command.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
struct Accelerator {
    id: i32,
    bindings: Vec<Shortcut>,
}
pub(crate) fn side_modifiers() -> u32 {
    [
        (0xa4, 256),
        (0xa2, 512),
        (0xa0, 1024),
        (0x5b, 2048),
        (0xa5, 16),
        (0xa3, 32),
        (0xa1, 64),
        (0x5c, 128),
    ]
    .into_iter()
    .fold(0, |bits, (key, mask)| {
        bits | if unsafe { GetKeyState(key) } < 0 {
            mask
        } else {
            0
        }
    })
}
fn run(receiver: mpsc::Receiver<Command>, events: Arc<Mutex<VecDeque<Shortcut>>>) {
    let mut registrations = BTreeMap::<(u32, u32), Accelerator>::new();
    let mut next_id = 0i32;
    let mut unused = Vec::<i32>::new();
    let mut enabled = false;
    loop {
        let mut message: Message = unsafe { std::mem::zeroed() };
        // PeekMessage creates/owns this thread's hidden message queue. Windows
        // routes the actual WM_HOTKEY from thread-bound registrations here.
        while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, 1) } != 0 {
            if message.message != 0x312 || !enabled {
                continue;
            }
            let sides = side_modifiers();
            if let Some(accelerator) = registrations
                .values()
                .find(|a| a.id == message.wparam as i32)
            {
                // Current 0x189a0 picks the first matching original side mask.
                if let Some(binding) = accelerator
                    .bindings
                    .iter()
                    .find(|b| (b.modifiers & 0xff0) & !sides == 0)
                {
                    if let Ok(mut queue) = events.lock() {
                        if queue.len() < 512 {
                            queue.push_back(binding.clone());
                        }
                    }
                }
            }
        }
        let command = match receiver.recv_timeout(Duration::from_millis(16)) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        match command {
            Command::Stop => break,
            Command::Enable(value, reply) => {
                enabled = value;
                if !value {
                    if let Ok(mut queue) = events.lock() {
                        queue.clear();
                    }
                }
                let _ = reply.send(Ok(Vec::new()));
            }
            Command::Registered(reply) => {
                let _ = reply.send(Ok(registrations
                    .values()
                    .flat_map(|a| a.bindings.clone())
                    .collect()));
            }
            Command::Register(shortcut, reply) => {
                let result = (|| {
                    let os = os_modifiers(shortcut.modifiers)?;
                    let key = (shortcut.virtual_key, os);
                    if let Some(accelerator) = registrations.get_mut(&key) {
                        if let Some(binding) = accelerator
                            .bindings
                            .iter_mut()
                            .find(|b| b.modifiers == shortcut.modifiers)
                        {
                            *binding = shortcut;
                        } else {
                            accelerator.bindings.push(shortcut);
                        }
                        return Ok(Vec::new());
                    }
                    let id = unused.pop().unwrap_or_else(|| {
                        let id = next_id;
                        next_id += 1;
                        id
                    });
                    ensure!(id <= 0xbfff, "Hotkey ID space exhausted");
                    if unsafe { RegisterHotKey(std::ptr::null_mut(), id, os, shortcut.virtual_key) }
                        == 0
                    {
                        unused.push(id);
                        anyhow::bail!(
                            "OS shortcut registration rejected: {}",
                            std::io::Error::last_os_error()
                        );
                    }
                    registrations.insert(
                        key,
                        Accelerator {
                            id,
                            bindings: vec![shortcut],
                        },
                    );
                    Ok(Vec::new())
                })()
                .map_err(|error: anyhow::Error| format!("{error:#}"));
                let _ = reply.send(result);
            }
            Command::Unregister(key, modifiers, reply) => {
                let result = (|| {
                    let identity = (key, os_modifiers(modifiers)?);
                    let accelerator = registrations
                        .get_mut(&identity)
                        .context("Shortcut not found")?;
                    let index = accelerator
                        .bindings
                        .iter()
                        .position(|b| b.modifiers == modifiers)
                        .context("Shortcut not found")?;
                    if accelerator.bindings.len() == 1 {
                        ensure!(
                            unsafe { UnregisterHotKey(std::ptr::null_mut(), accelerator.id) } != 0,
                            "OS shortcut removal failed: {}",
                            std::io::Error::last_os_error()
                        );
                        unused.push(accelerator.id);
                        registrations.remove(&identity);
                    } else {
                        accelerator.bindings.remove(index);
                    }
                    Ok(Vec::new())
                })()
                .map_err(|error: anyhow::Error| format!("{error:#}"));
                let _ = reply.send(result);
            }
        }
    }
    for accelerator in registrations.values() {
        unsafe { UnregisterHotKey(std::ptr::null_mut(), accelerator.id) };
    }
}
