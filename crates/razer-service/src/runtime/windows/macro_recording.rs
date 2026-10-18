//! Current host 4.0.827 mapping_engine recorder callbacks. Worker process only.
//! Registration owns observation callbacks; no macro/configuration is written.
use super::{Callback2, CallbackReply, NativeRuntime, callback2, callback3, copy_string};
use razer_ipc::macro_recording::{ActionReply, RestoreReceipt, SuspensionReceipt};
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

fn resume_operations(
    mut call: impl FnMut(&'static str) -> anyhow::Result<CallbackReply>,
) -> anyhow::Result<(CallbackReply, CallbackReply)> {
    // Current software RECORDING.enablingMapping invokes Promise.all. A
    // failed first operation must not prevent submitting the second one.
    let mapping = call("enableMapping");
    let shortcuts = call("enableGlobalShortcut");
    match (mapping, shortcuts) {
        (Ok(mapping), Ok(shortcuts)) => Ok((mapping, shortcuts)),
        (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
        (Err(mapping), Err(shortcuts)) => Err(anyhow::anyhow!(
            "mapping: {mapping:#}; global shortcut: {shortcuts:#}"
        )),
    }
}

fn confirm_mapping_restoration((mapping, _): (CallbackReply, CallbackReply)) -> anyhow::Result<()> {
    anyhow::ensure!(
        mapping.success,
        "enableMapping did not confirm restoration: {}",
        mapping.reason
    );
    Ok(())
}

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
    shortcuts_suspended: bool,
    mode: Option<&'static str>,
}
impl Recorder {
    pub(super) fn active(&self) -> bool {
        self.registered || self.recording || self.mappings_suspended || self.shortcuts_suspended
    }
    pub(super) fn new() -> Self {
        let (sender, events) = mpsc::sync_channel(4096);
        let _ = EVENTS.set(sender);
        Self {
            events,
            registered: false,
            recording: false,
            mappings_suspended: false,
            shortcuts_suspended: false,
            mode: None,
        }
    }

    pub(super) fn events(&mut self) -> Value {
        let events: Vec<_> = self.events.try_iter().take(512).collect();
        // A native stop event can end recording before the explicit Stop call.
        for event in &events {
            if event["kind"] == "stopped" && event["mode"].as_str() == self.mode {
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
        Ok(serde_json::to_value(SuspensionReceipt::new(true))?)
    }

    pub(super) fn resume_macro_mappings(&mut self) -> anyhow::Result<Value> {
        let (mapping, shortcuts) = self.resume_macro_mapping_receipts()?;
        Ok(serde_json::to_value(RestoreReceipt::new(
            self.macro_recorder.mappings_suspended,
            ActionReply::new(mapping.success, mapping.reason),
            ActionReply::new(shortcuts.success, shortcuts.reason),
        ))?)
    }

    fn resume_macro_mapping_receipts(&mut self) -> anyhow::Result<(CallbackReply, CallbackReply)> {
        resume_operations(|name| {
            anyhow::ensure!(
                !self.poisoned,
                "Recorder callback owner is unavailable during restoration"
            );
            let enable: Operation = unsafe { self.mapping_symbol(name)? };
            // callMappingEngineAction returns callback {result,reason} without
            // rejecting false. Keep those separate from transport exceptions.
            let reply = self.call_source(|| unsafe { enable(callback2) })?;
            if reply.success {
                if name == "enableMapping" {
                    self.macro_recorder.mappings_suspended = false;
                } else {
                    self.macro_recorder.shortcuts_suspended = false;
                }
            }
            Ok(reply)
        })
    }

    pub(super) fn start_macro_recording(&mut self) -> anyhow::Result<Value> {
        self.register_macro_recorder()?;
        let result = self.start_recorder_mode("kSoftware", c"kSoftware")?;
        anyhow::ensure!(
            result["accepted"] == true,
            "Software recorder rejected start: {}",
            result["reason"]
        );
        Ok(result)
    }

    fn register_macro_recorder(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.shortcut_capture.active(),
            "Shortcut capture already owns temporary mapping suppression"
        );
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
        Ok(())
    }

    fn start_recorder_mode(
        &mut self,
        mode: &'static str,
        native: &'static std::ffi::CStr,
    ) -> anyhow::Result<Value> {
        let start: Start = unsafe { self.mapping_symbol("startMacroRecording")? };
        // Current host and background machine branch on result, not a later
        // macrorecordingstarted event. A rejected result is retained verbatim.
        let result = self.call_source(|| unsafe { start(native.as_ptr(), callback2) })?;
        if !result.success {
            return Ok(json!({"result":false,"reason":result.reason,"accepted":false}));
        }
        self.macro_recorder.recording = true;
        self.macro_recorder.mode = Some(mode);
        // Acceptance is not the asynchronous macrorecordingstarted event.
        Ok(json!({"accepted":true,"result":true,"reason":result.reason}))
    }

    pub(super) fn prepare_otfm_recorder(&mut self) -> anyhow::Result<Value> {
        self.register_macro_recorder()?;
        Ok(json!({"registered":true}))
    }

    pub(super) fn start_otfm_recording(&mut self) -> anyhow::Result<Value> {
        anyhow::ensure!(
            self.macro_recorder.registered && !self.macro_recorder.recording,
            "OTFM requires its prepared, inactive recorder listener owner"
        );
        self.start_recorder_mode("kOTFM", c"kOTFM")
    }

    pub(super) fn query_macro_recording_started(&mut self) -> anyhow::Result<Value> {
        self.mapping()?;
        let query: Operation = unsafe { self.mapping_symbol("hasMacroRecordingStarted")? };
        // This callback's bool IS the observed recording state. False is a
        // valid observation, unlike the success flag of ordinary operations.
        let reply = self.call_source(|| unsafe { query(callback2) })?;
        Ok(json!({"result":reply.success,"reason":reply.reason}))
    }

    pub(super) fn macro_recorder_global_state(&mut self) -> anyhow::Result<Value> {
        self.mapping()?;
        let mode: super::Query = unsafe { self.mapping_symbol("getMacroRecordingMode")? };
        let observed = self.call(|| unsafe { mode(callback3) })?;
        let mode = observed
            .value
            .ok_or_else(|| anyhow::anyhow!("Native recorder mode callback has no value"))?;
        let global: super::Query = unsafe { self.mapping_symbol("getGlobalMode")? };
        let observed = self.call(|| unsafe { global(callback3) })?;
        let global = observed
            .value
            .ok_or_else(|| anyhow::anyhow!("Native global mode callback has no value"))?;
        Ok(json!({"macroRecordingMode":mode,"globalMode":global}))
    }

    pub(super) fn set_otfm_mappings_suspended(&mut self, suspended: bool) -> anyhow::Result<Value> {
        anyhow::ensure!(
            self.macro_recorder.registered,
            "OTFM listener owner is absent"
        );
        let mut errors = Vec::new();
        // Promise.all in the current source submits BOTH operations, including
        // the global shortcut operation when disableMapping rejects.
        let operation: Operation = unsafe {
            self.mapping_symbol(if suspended {
                "disableMapping"
            } else {
                "enableMapping"
            })?
        };
        match self.call(|| unsafe { operation(callback2) }) {
            Ok(_) => self.macro_recorder.mappings_suspended = suspended,
            Err(error) => errors.push(format!("mapping: {error:#}")),
        }
        if !self.poisoned {
            let operation: Operation = unsafe {
                self.mapping_symbol(if suspended {
                    "disableGlobalShortcut"
                } else {
                    "enableGlobalShortcut"
                })?
            };
            match self.call(|| unsafe { operation(callback2) }) {
                Ok(_) => self.macro_recorder.shortcuts_suspended = suspended,
                Err(error) => errors.push(format!("global shortcut: {error:#}")),
            }
        } else {
            errors.push("Native callback timed out before global shortcut operation".into());
        }
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(json!({"suspended":suspended}))
    }

    pub(super) fn macro_recorder_storage_write(
        &mut self,
        key: &str,
        value: &Value,
    ) -> anyhow::Result<Value> {
        anyhow::ensure!(
            self.macro_recorder.registered,
            "Recorder storage writer owner is absent"
        );
        anyhow::ensure!(
            key == "synapseMacros" || key.starts_with("synapse_"),
            "Recorder storage key is outside current local chain"
        );
        let key_text = std::ffi::CString::new(key)?;
        let value_text = std::ffi::CString::new(value.to_string())?;
        let set: super::StorageSetItem = unsafe { self.mapping_symbol("localStorageSetItem")? };
        let result =
            self.call(|| unsafe { set(key_text.as_ptr(), value_text.as_ptr(), callback2) });
        if self.poisoned {
            std::mem::forget(key_text);
            std::mem::forget(value_text);
        }
        result?;
        Ok(json!({"published":true,"key":key}))
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
            } else {
                // Source false is response data during ordinary resume. An
                // owned shutdown must still report unconfirmed restoration.
                if let Err(error) = self
                    .resume_macro_mapping_receipts()
                    .and_then(confirm_mapping_restoration)
                {
                    errors.push(format!("恢复录制前映射状态失败：{error:#}"));
                }
            }
        }
        if self.macro_recorder.shortcuts_suspended {
            if self.poisoned {
                errors.push(
                    "Recorder connection timed out; shortcut restoration is unconfirmed".into(),
                );
            } else {
                let result = (|| {
                    let enable: Operation = unsafe { self.mapping_symbol("enableGlobalShortcut")? };
                    self.call(|| unsafe { enable(callback2) })
                })();
                match result {
                    Ok(_) => self.macro_recorder.shortcuts_suspended = false,
                    Err(error) => {
                        errors.push(format!("Restore recorder global shortcuts: {error:#}"))
                    }
                }
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

#[cfg(test)]
mod shortcut_resume_tests {
    use super::*;

    #[test]
    fn recorder_resume_submits_global_shortcuts_after_mapping_transport_failure() {
        let mut calls = Vec::new();
        let result = resume_operations(|name| {
            calls.push(name);
            if name == "enableMapping" {
                anyhow::bail!("mapping transport failure");
            }
            Ok(CallbackReply {
                success: true,
                reason: String::new(),
                value: None,
            })
        });
        assert!(result.is_err());
        assert_eq!(calls, ["enableMapping", "enableGlobalShortcut"]);
    }

    #[test]
    fn recorder_resume_preserves_native_false_as_source_response_data() {
        let (mapping, shortcuts) = resume_operations(|_| {
            Ok(CallbackReply {
                success: false,
                reason: "hotkey was not disabled".into(),
                value: None,
            })
        })
        .unwrap();
        assert!(!mapping.success);
        assert!(!shortcuts.success);
        assert_eq!(shortcuts.reason, "hotkey was not disabled");
    }

    #[test]
    fn recorder_cleanup_rejects_a_second_false_mapping_restore_without_skipping_shortcuts() {
        let mut calls = Vec::new();
        let mut restore = || {
            resume_operations(|name| {
                calls.push(name);
                Ok(CallbackReply {
                    success: name == "enableGlobalShortcut",
                    reason: if name == "enableMapping" {
                        "mapping service rejected restore".into()
                    } else {
                        String::new()
                    },
                    value: None,
                })
            })
        };
        let (mapping, shortcuts) = restore().unwrap();
        assert!(!mapping.success);
        assert!(shortcuts.success);
        let cleanup = restore().and_then(confirm_mapping_restoration);
        assert!(
            cleanup
                .unwrap_err()
                .to_string()
                .contains("mapping service rejected restore")
        );
        assert_eq!(
            calls,
            [
                "enableMapping",
                "enableGlobalShortcut",
                "enableMapping",
                "enableGlobalShortcut"
            ]
        );
    }

    #[test]
    fn recorder_cleanup_accepts_confirmed_restore_after_a_source_false() {
        let mut attempt = 0;
        let mut calls = Vec::new();
        let mut restore = || {
            attempt += 1;
            resume_operations(|name| {
                calls.push(name);
                Ok(CallbackReply {
                    success: attempt == 2 || name == "enableGlobalShortcut",
                    reason: if attempt == 1 && name == "enableMapping" {
                        "temporary mapping restore failure".into()
                    } else {
                        String::new()
                    },
                    value: None,
                })
            })
        };
        let (mapping, _) = restore().unwrap();
        assert!(!mapping.success);
        restore().and_then(confirm_mapping_restoration).unwrap();
        assert_eq!(
            calls,
            [
                "enableMapping",
                "enableGlobalShortcut",
                "enableMapping",
                "enableGlobalShortcut"
            ]
        );
    }
}
