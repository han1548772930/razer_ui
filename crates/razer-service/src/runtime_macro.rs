//! Current host 4.0.827 mapping_engine recorder callbacks. Worker process only.
//! Registration owns observation callbacks; no macro/configuration is written.
use super::{Callback2, NativeRuntime, callback2, copy_string};
use serde_json::{Value, json};
use std::{
    ffi::c_char,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{SystemTime, UNIX_EPOCH},
};

type LifecycleEvent = unsafe extern "C" fn(*const c_char, i32, u64);
type ItemEvent = unsafe extern "C" fn(i32, *const c_char, u64);
type SetCallbacks = unsafe extern "C" fn(LifecycleEvent, LifecycleEvent, ItemEvent, Callback2);
type Operation = unsafe extern "C" fn(Callback2);
type Start = unsafe extern "C" fn(*const c_char, Callback2);

static EVENTS: OnceLock<SyncSender<Value>> = OnceLock::new();
static OVERFLOW: AtomicBool = AtomicBool::new(false);

fn received_at_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn emit(value: Value) {
    if let Some(sender) = EVENTS.get() {
        if sender.try_send(value).is_err() {
            // Never silently save an incomplete recording after queue loss.
            OVERFLOW.store(true, Ordering::Release);
        }
    }
}

unsafe extern "C" fn started(mode: *const c_char, event_type: i32, time_tick: u64) {
    emit(
        json!({"kind":"started", "mode":unsafe { copy_string(mode) },
        "event_type":event_type, "time_tick":time_tick, "received_at_ms":received_at_ms()}),
    );
}
unsafe extern "C" fn stopped(mode: *const c_char, event_type: i32, time_tick: u64) {
    emit(
        json!({"kind":"stopped", "mode":unsafe { copy_string(mode) },
        "event_type":event_type, "time_tick":time_tick, "received_at_ms":received_at_ms()}),
    );
}
unsafe extern "C" fn item(event_type: i32, value: *const c_char, time_tick: u64) {
    let received_at_ms = received_at_ms();
    let event = if value.is_null() {
        Err("宏录制服务返回空事件".to_string())
    } else {
        serde_json::from_str::<Value>(&unsafe { copy_string(value) })
            .map_err(|error| format!("宏录制事件不是有效 JSON：{error}"))
    };
    match event {
        Ok(event) => emit(json!({"kind":"item", "event_type":event_type,
            "event":event, "time_tick":time_tick, "received_at_ms":received_at_ms})),
        Err(error) => emit(json!({"kind":"error", "error":error, "time_tick":time_tick})),
    }
}

pub(super) struct Recorder {
    events: Receiver<Value>,
    registered: bool,
    recording: bool,
    mappings_suspended: bool,
}
impl Recorder {
    pub(super) fn new() -> Self {
        let (sender, events) = mpsc::sync_channel(4096);
        let _ = EVENTS.set(sender);
        Self {
            events,
            registered: false,
            recording: false,
            mappings_suspended: false,
        }
    }

    pub(super) fn events(&mut self) -> Value {
        let events: Vec<_> = self.events.try_iter().take(512).collect();
        // A native stop event can end recording before the explicit Stop call.
        for event in &events {
            if event["kind"] == "stopped" && event["mode"] == "kSoftware" {
                self.recording = false;
            }
        }
        json!({"events":events, "overflow":OVERFLOW.swap(false, Ordering::AcqRel)})
    }
}

impl NativeRuntime {
    pub(super) fn suspend_macro_mappings(&mut self) -> anyhow::Result<Value> {
        anyhow::ensure!(self.macro_recorder.recording, "宏录制尚未开始");
        if !self.macro_recorder.mappings_suspended {
            let disable: Operation = unsafe { self.mapping_symbol("disableMapping")? };
            self.call(|| unsafe { disable(callback2) })?;
            self.macro_recorder.mappings_suspended = true;
        }
        Ok(json!({"suspended":true}))
    }

    pub(super) fn resume_macro_mappings(&mut self) -> anyhow::Result<Value> {
        if self.macro_recorder.mappings_suspended {
            let enable: Operation = unsafe { self.mapping_symbol("enableMapping")? };
            self.call(|| unsafe { enable(callback2) })?;
            self.macro_recorder.mappings_suspended = false;
        }
        Ok(json!({"suspended":false}))
    }

    pub(super) fn start_macro_recording(&mut self) -> anyhow::Result<Value> {
        anyhow::ensure!(!self.macro_recorder.recording, "当前会话已有宏录制正在进行");
        self.mapping()?;
        anyhow::ensure!(
            !self.macro_recorder.registered,
            "请结束当前录制会话后重新开始"
        );
        let register: Operation = unsafe { self.mapping_symbol("registerMacroRecorderEvent")? };
        self.call(|| unsafe { register(callback2) })?;
        self.macro_recorder.registered = true;
        let install: SetCallbacks =
            unsafe { self.mapping_symbol("setMacroRecorderEventCallback")? };
        self.call(|| unsafe { install(started, stopped, item, callback2) })?;
        let start: Start = unsafe { self.mapping_symbol("startMacroRecording")? };
        self.call(|| unsafe { start(c"kSoftware".as_ptr(), callback2) })?;
        self.macro_recorder.recording = true;
        // Acceptance is not the asynchronous macrorecordingstarted event.
        Ok(json!({"accepted":true}))
    }

    pub(super) fn stop_macro_recording(&mut self) -> anyhow::Result<Value> {
        anyhow::ensure!(self.macro_recorder.registered, "宏录制会话尚未建立");
        if self.macro_recorder.recording {
            let stop: Operation = unsafe { self.mapping_symbol("stopMacroRecording")? };
            self.call(|| unsafe { stop(callback2) })?;
            self.macro_recorder.recording = false;
        }
        // Keep callbacks registered until the caller drains the stop event.
        Ok(json!({"accepted":true}))
    }

    pub(super) fn shutdown_macro_recording(&mut self, errors: &mut Vec<String>) {
        if self.macro_recorder.recording && !self.poisoned {
            if let Err(error) = self.stop_macro_recording() {
                errors.push(format!("停止宏录制失败：{error:#}"));
            }
        }
        if self.macro_recorder.mappings_suspended {
            if self.poisoned {
                errors.push("录制连接已失效，无法确认临时映射状态是否恢复".into());
            } else if let Err(error) = self.resume_macro_mappings() {
                errors.push(format!("恢复录制前映射状态失败：{error:#}"));
            }
        }
        if self.macro_recorder.registered && !self.poisoned {
            let result = (|| {
                let unregister: Operation =
                    unsafe { self.mapping_symbol("unregisterMacroRecorderEvent")? };
                self.call(|| unsafe { unregister(callback2) })
            })();
            match result {
                Ok(_) => self.macro_recorder.registered = false,
                Err(error) => errors.push(format!("注销宏录制监听失败：{error:#}")),
            }
        }
    }
}
