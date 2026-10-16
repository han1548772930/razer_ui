//! Current host mapping_engine/win/index.js registerDeviceModeChangedEvent
//! (4978), setDeviceModeChangedEventCallback, createCbDeviceModeChanged (5446),
//! disableMapping/enableMapping. The audio1342 GU capture owns this lifecycle;
//! it does not start, synthesize or save a macro recording.
use super::{Callback2, NativeRuntime, callback2, copy_string};
use serde_json::{Value, json};
use std::{
    ffi::c_char,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
};
type Operation = unsafe extern "C" fn(Callback2);
type ModeEvent = unsafe extern "C" fn(i32, *const c_char, u64);
type Install = unsafe extern "C" fn(ModeEvent, Callback2);
static EVENTS: Mutex<Option<SyncSender<Value>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);
unsafe extern "C" fn mode_event(kind: i32, value: *const c_char, tick: u64) {
    let event = json!({"eventType":kind,"event":unsafe { copy_string(value) },"timeTick":tick});
    if let Ok(slot) = EVENTS.lock() {
        if let Some(sender) = slot.as_ref() {
            if sender.try_send(event).is_err() {
                OVERFLOW.store(true, Ordering::Release);
            }
        }
    }
}
pub(super) struct State {
    events: Receiver<Value>,
    registered: bool,
    suspended: bool,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, events) = mpsc::sync_channel(128);
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(sender);
        }
        Self {
            events,
            registered: false,
            suspended: false,
        }
    }
    pub(super) fn active(&self) -> bool {
        self.registered || self.suspended
    }
}
impl NativeRuntime {
    pub(super) fn start_shortcut_capture(&mut self) -> anyhow::Result<Value> {
        anyhow::ensure!(
            !self.shortcut_capture.active(),
            "Shortcut capture is already active"
        );
        anyhow::ensure!(
            !self.macro_recorder.active(),
            "Macro recording owns temporary mapping suppression"
        );
        self.mapping()?;
        OVERFLOW.store(false, Ordering::Release);
        while self.shortcut_capture.events.try_recv().is_ok() {}
        let run = (|| -> anyhow::Result<()> {
            let disable: Operation = unsafe { self.mapping_symbol("disableMapping")? };
            self.call(|| unsafe { disable(callback2) })?;
            self.shortcut_capture.suspended = true;
            let register: Operation =
                unsafe { self.mapping_symbol("registerDeviceModeChangedEvent")? };
            self.call(|| unsafe { register(callback2) })?;
            self.shortcut_capture.registered = true;
            let install: Install =
                unsafe { self.mapping_symbol("setDeviceModeChangedEventCallback")? };
            self.call(|| unsafe { install(mode_event, callback2) })?;
            Ok(())
        })();
        if let Err(error) = run {
            return match self.stop_shortcut_capture() {
                Ok(_) => Err(error),
                Err(cleanup) => Err(anyhow::anyhow!(
                    "{error:#}; shortcut capture cleanup failed: {cleanup:#}"
                )),
            };
        }
        Ok(json!({"capturing":true,"mappings_suspended":true,"mode_events_registered":true}))
    }
    pub(super) fn shortcut_capture_events(&mut self) -> anyhow::Result<Value> {
        anyhow::ensure!(
            self.shortcut_capture.registered,
            "Shortcut capture mode events are not registered"
        );
        anyhow::ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Shortcut capture mode event buffer overflowed"
        );
        Ok(json!({"events":self.shortcut_capture.events.try_iter().collect::<Vec<_>>() }))
    }
    pub(super) fn stop_shortcut_capture(&mut self) -> anyhow::Result<Value> {
        if !self.shortcut_capture.active() {
            return Ok(json!({"capturing":false,"mappings_suspended":false}));
        }
        anyhow::ensure!(
            !self.poisoned,
            "Shortcut capture callback timed out; restoring mappings cannot be confirmed"
        );
        let mut errors = Vec::new();
        if self.shortcut_capture.suspended {
            let result = (|| -> anyhow::Result<()> {
                let enable: Operation = unsafe { self.mapping_symbol("enableMapping")? };
                self.call(|| unsafe { enable(callback2) })?;
                self.shortcut_capture.suspended = false;
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("{error:#}"));
            }
        }
        if self.shortcut_capture.registered && !self.poisoned {
            let result = (|| -> anyhow::Result<()> {
                let unregister: Operation =
                    unsafe { self.mapping_symbol("unregisterDeviceModeChangedEvent")? };
                self.call(|| unsafe { unregister(callback2) })?;
                self.shortcut_capture.registered = false;
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("{error:#}"));
            }
        }
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(json!({"capturing":false,"mappings_suspended":false}))
    }
}
