//! Current host mapping_engine/win/index.js input-monitor callbacks. The DLL
//! and callback code remain owned by NativeRuntime through engine shutdown.
use super::{Callback2, NativeRuntime, callback2};
use anyhow::{Context as _, ensure};
use razer_ipc::mouse_input_monitoring::{InputEvent, MAX_CALLBACK_BYTES, Poll, Receipt};
use serde_json::Value;
use std::{
    collections::VecDeque,
    ffi::c_char,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};

type EventCallback = unsafe extern "C" fn(*const c_char, i32, *const c_char, u64);
type Register = unsafe extern "C" fn(EventCallback, Callback2);
type Operation = unsafe extern "C" fn(Callback2);

static EVENTS: Mutex<Option<SyncSender<Result<InputEvent, String>>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);

/// The current callback ABI supplies borrowed, NUL-terminated UTF-8 buffers.
/// Copy at most the remaining IPC budget while those buffers are still valid.
unsafe fn copy_callback_text(pointer: *const c_char, limit: usize) -> anyhow::Result<String> {
    ensure!(!pointer.is_null(), "Input monitor callback string is null");
    let mut bytes = Vec::new();
    for offset in 0..=limit {
        // The vendor callback keeps the whole NUL-terminated buffer alive for
        // this call; no pointer or slice escapes the callback gate.
        let byte = unsafe { *pointer.add(offset) } as u8;
        if byte == 0 {
            return String::from_utf8(bytes).context("Input monitor callback is not UTF-8");
        }
        ensure!(
            offset < limit,
            "Input monitor callback exceeds the transport limit"
        );
        bytes.push(byte);
    }
    unreachable!("the callback copy returns at its size boundary")
}

unsafe extern "C" fn input_event(
    device_info: *const c_char,
    event_type: i32,
    json_event: *const c_char,
    _time_tick: u64,
) {
    match EVENTS.lock() {
        Ok(slot) => {
            if let Some(sender) = slot.as_ref() {
                // Acceptance, temporary-string copying and publication share
                // this gate. Stop waits for accepted callbacks; callbacks
                // arriving after closure do not read vendor buffers.
                let event = (|| {
                    let info = unsafe { copy_callback_text(device_info, MAX_CALLBACK_BYTES) }?;
                    let events =
                        unsafe { copy_callback_text(json_event, MAX_CALLBACK_BYTES - info.len()) }?;
                    InputEvent::from_native(&info, event_type, events)
                })()
                .map_err(|error| format!("{error:#}"));
                if sender.try_send(event).is_err() {
                    OVERFLOW.store(true, Ordering::Release);
                }
            }
        }
        Err(_) => OVERFLOW.store(true, Ordering::Release),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_copy_rejects_null_invalid_utf8_and_overlong_buffers() {
        assert!(unsafe { copy_callback_text(std::ptr::null(), 3) }.is_err());
        assert!(unsafe { copy_callback_text(b"abcd\0".as_ptr().cast(), 3) }.is_err());
        assert!(unsafe { copy_callback_text([0xffu8, 0].as_ptr().cast(), 3) }.is_err());
        assert_eq!(
            unsafe { copy_callback_text(b"abc\0".as_ptr().cast(), 3) }.unwrap(),
            "abc"
        );
        assert_eq!(
            unsafe { copy_callback_text(b"\0".as_ptr().cast(), 0) }.unwrap(),
            ""
        );
    }
}

#[derive(Default)]
pub(super) struct State {
    owner: Option<String>,
    events: Option<Receiver<Result<InputEvent, String>>>,
    pending: VecDeque<InputEvent>,
    possibly_started: bool,
}

impl State {
    pub(super) fn owner(&self) -> Option<String> {
        self.owner.clone()
    }

    fn check_owner(&self, owner: &str) -> anyhow::Result<()> {
        ensure!(
            !owner.is_empty() && self.owner.as_deref() == Some(owner),
            "Input monitor request belongs to another owner"
        );
        Ok(())
    }

    fn poll(&mut self, owner: &str) -> anyhow::Result<Poll> {
        self.check_owner(owner)?;
        let receiver = self
            .events
            .as_ref()
            .context("Input monitor inbox is absent")?;
        let mut failure = OVERFLOW
            .load(Ordering::Acquire)
            .then(|| "Input monitor queue overflow; incomplete movement was rejected".into());
        let mut events = Vec::new();
        let mut bytes = 0usize;
        // JSON string escaping can multiply the raw byte size. Measure the
        // serialized events before they enter the four-MiB framed response.
        while events.len() < 128 {
            let next = if let Some(event) = self.pending.pop_front() {
                Some(event)
            } else {
                match receiver.try_recv() {
                    Ok(Ok(event)) => Some(event),
                    Ok(Err(error)) => {
                        failure = Some(error);
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        failure = Some("Input monitor callback inbox disconnected".into());
                        None
                    }
                }
            };
            let Some(event) = next else { break };
            let encoded = serde_json::to_vec(&event)?.len();
            ensure!(
                encoded < 3 * 1024 * 1024,
                "Input monitor event exceeds the IPC frame"
            );
            if bytes + encoded > 3 * 1024 * 1024 {
                self.pending.push_front(event);
                break;
            }
            bytes += encoded;
            events.push(event);
        }
        Ok(Poll::new(owner.to_owned(), events, failure))
    }
}

impl NativeRuntime {
    pub(super) fn start_mouse_input_monitoring(&mut self, owner: String) -> anyhow::Result<Value> {
        ensure!(!owner.is_empty(), "Input monitor owner is empty");
        ensure!(
            self.mouse_input_monitoring.owner.is_none(),
            "Input monitor is already owned"
        );
        self.mapping()?;
        let register: Register = unsafe { self.mapping_symbol("setInputMonitoringCallback")? };
        let start: Operation = unsafe { self.mapping_symbol("startMouseInputMonitoring")? };
        let (sender, receiver) = mpsc::sync_channel(512);
        {
            let mut slot = EVENTS
                .lock()
                .map_err(|_| anyhow::anyhow!("Input monitor callback lock poisoned"))?;
            ensure!(
                slot.is_none(),
                "Input monitor callback is already installed"
            );
            OVERFLOW.store(false, Ordering::Release);
            *slot = Some(sender);
        }
        self.mouse_input_monitoring = State {
            owner: Some(owner.clone()),
            events: Some(receiver),
            pending: VecDeque::new(),
            possibly_started: false,
        };
        self.call(|| unsafe { register(input_event, callback2) })?;
        // A rejected or lost acknowledgement cannot establish that the native
        // monitor did not start. Retain ownership until stop is acknowledged.
        self.mouse_input_monitoring.possibly_started = true;
        self.call(|| unsafe { start(callback2) })?;
        serde_json::to_value(Receipt::new(owner, true)).map_err(Into::into)
    }

    pub(super) fn poll_mouse_input_monitoring(&mut self, owner: &str) -> anyhow::Result<Value> {
        serde_json::to_value(self.mouse_input_monitoring.poll(owner)?).map_err(Into::into)
    }

    pub(super) fn stop_mouse_input_monitoring(&mut self, owner: &str) -> anyhow::Result<Value> {
        self.mouse_input_monitoring.check_owner(owner)?;
        if self.mouse_input_monitoring.possibly_started {
            let stop: Operation = unsafe { self.mapping_symbol("stopMouseInputMonitoring")? };
            self.call(|| unsafe { stop(callback2) })?;
            self.mouse_input_monitoring.possibly_started = false;
        }
        // Taking the inbox under the callback's lock also waits for a callback
        // already publishing a batch. Callback code and DLL stay loaded until
        // the independent engine shutdown completes.
        EVENTS
            .lock()
            .map_err(|_| anyhow::anyhow!("Input monitor callback lock poisoned"))?
            .take();
        self.mouse_input_monitoring = State::default();
        serde_json::to_value(Receipt::new(owner.to_owned(), false)).map_err(Into::into)
    }
}
