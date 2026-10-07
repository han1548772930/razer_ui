//! Worker-only vendor bindings. Each signature is paired with the original
//! `apiObj`, callback declaration and invocation in the Electron wrapper.
use super::{CALLBACK_TIMEOUT, ServiceRequest};
use crate::backend::dll::{EngineLibrary, EnginePaths};
use anyhow::{Context as _, bail};
use serde_json::{Value, json};
use std::{
    ffi::{CStr, CString, c_char},
    mem::ManuallyDrop,
    sync::{
        Mutex, OnceLock,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
};

#[path = "runtime_hid.rs"]
mod hid;
#[path = "runtime_macro.rs"]
mod macro_recorder;
#[path = "runtime_receiver.rs"]
mod receiver;
#[path = "runtime_usb.rs"]
mod usb;

// No userdata argument exists in these APIs. The worker permits one outstanding
// call and terminates after a timeout, so a late callback cannot satisfy a later
// request. Function pointers are static for the entire worker lifetime.
static PENDING: Mutex<Option<Sender<CallbackReply>>> = Mutex::new(None);
static SHORTCUT_EVENTS: OnceLock<SyncSender<Value>> = OnceLock::new();

type Callback0 = unsafe extern "C" fn();
type Callback2 = unsafe extern "C" fn(bool, *const c_char);
type Callback3 = unsafe extern "C" fn(bool, *const c_char, *const c_char);
type EventCallback = unsafe extern "C" fn(i32, *const c_char, u64);
type Initialize = unsafe extern "C" fn(Callback0);
type Query = unsafe extern "C" fn(Callback3);
type RegisterShortcut = unsafe extern "C" fn(u32, u32, *const c_char, Callback2);
type UnregisterShortcut = unsafe extern "C" fn(u32, u32, Callback2);
type SetShortcutCallback = unsafe extern "C" fn(EventCallback, Callback2);
type StorageSetItem = unsafe extern "C" fn(*const c_char, *const c_char, Callback2);

struct CallbackReply {
    success: bool,
    reason: String,
    value: Option<String>,
}

unsafe fn copy_string(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        // SAFETY: the original API callback supplies a borrowed NUL-terminated
        // string. Copy it during this invocation; the DLL retains ownership.
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned()
    }
}

fn complete(reply: CallbackReply) {
    if let Ok(mut slot) = PENDING.lock() {
        if let Some(sender) = slot.take() {
            let _ = sender.send(reply);
        }
    }
}

unsafe extern "C" fn callback0() {
    complete(CallbackReply {
        success: true,
        reason: String::new(),
        value: None,
    });
}

unsafe extern "C" fn callback2(success: bool, reason: *const c_char) {
    complete(CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: None,
    });
}

unsafe extern "C" fn callback3(success: bool, reason: *const c_char, value: *const c_char) {
    complete(CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: (!value.is_null()).then(|| unsafe { copy_string(value) }),
    });
}

unsafe extern "C" fn shortcut_event(event_type: i32, value: *const c_char, time_tick: u64) {
    if let Some(sender) = SHORTCUT_EVENTS.get() {
        let _ = sender.try_send(json!({
            "event_type": event_type,
            "event": unsafe { copy_string(value) },
            "time_tick": time_tick,
        }));
    }
}

pub(super) struct NativeRuntime {
    paths: EnginePaths,
    // Never unload a library while its background callbacks may still be active.
    // Shutdown is explicit; process exit reclaims the module after this returns.
    mapping: Option<ManuallyDrop<EngineLibrary>>,
    simple: Option<ManuallyDrop<EngineLibrary>>,
    mapping_initialized: bool,
    simple_initialized: bool,
    event_callback_installed: bool,
    shortcut_events: Receiver<Value>,
    macro_recorder: macro_recorder::Recorder,
    poisoned: bool,
}

impl NativeRuntime {
    pub(super) fn new() -> Self {
        let (sender, shortcut_events) = mpsc::sync_channel(512);
        let _ = SHORTCUT_EVENTS.set(sender);
        Self {
            paths: EnginePaths::discover(),
            mapping: None,
            simple: None,
            mapping_initialized: false,
            simple_initialized: false,
            event_callback_installed: false,
            shortcut_events,
            macro_recorder: macro_recorder::Recorder::new(),
            poisoned: false,
        }
    }

    pub(super) fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn call(&mut self, invoke: impl FnOnce()) -> anyhow::Result<CallbackReply> {
        if self.poisoned {
            bail!("设备服务回调状态已失效，请重新连接");
        }
        let (sender, receiver) = mpsc::channel();
        match PENDING.lock() {
            Ok(mut pending) if pending.is_none() => *pending = Some(sender),
            _ => {
                self.poisoned = true;
                bail!("设备服务回调状态不可用，请重新连接");
            }
        }
        invoke();
        let reply = match receiver.recv_timeout(CALLBACK_TIMEOUT) {
            Ok(reply) => reply,
            Err(error) => {
                self.poisoned = true;
                if let Ok(mut pending) = PENDING.lock() {
                    pending.take();
                }
                return Err(error).context("原生服务未在时限内返回回调");
            }
        };
        if !reply.success {
            bail!("原生服务拒绝请求：{}", reply.reason);
        }
        Ok(reply)
    }

    fn mapping(&mut self) -> anyhow::Result<()> {
        if self.mapping_initialized {
            return Ok(());
        }
        if self.mapping.is_none() {
            let path = self.paths.resolve_host_service("mapping_engine")?;
            self.mapping = Some(ManuallyDrop::new(EngineLibrary::load(&path)?));
        }
        // mapping_engine/win/index.js: ["void",["pointer"]], Callback("void",[]).
        let initialize: Initialize = unsafe { self.mapping_symbol("mappingEngineInitialize")? };
        if let Err(error) = self.call(|| unsafe { initialize(callback0) }) {
            self.poisoned = true;
            return Err(error);
        }
        self.mapping_initialized = true;
        Ok(())
    }

    fn simple(&mut self) -> anyhow::Result<()> {
        if self.simple_initialized {
            return Ok(());
        }
        if self.simple.is_none() {
            let path = self.paths.resolve_host_service("simple_service")?;
            self.simple = Some(ManuallyDrop::new(EngineLibrary::load(&path)?));
        }
        // simple_service/win/index.js: identical void(callback0) lifecycle.
        let initialize: Initialize = unsafe { self.simple_symbol("simpleServiceInitialize")? };
        if let Err(error) = self.call(|| unsafe { initialize(callback0) }) {
            self.poisoned = true;
            return Err(error);
        }
        self.simple_initialized = true;
        Ok(())
    }

    unsafe fn mapping_symbol<T: Copy>(&self, name: &str) -> anyhow::Result<T> {
        let library = self.mapping.as_ref().context("映射引擎未加载")?;
        unsafe { library.func(name) }
            .with_context(|| format!("{} 缺少当前封装所需导出 {name}", library.path().display()))
    }

    unsafe fn simple_symbol<T: Copy>(&self, name: &str) -> anyhow::Result<T> {
        let library = self.simple.as_ref().context("音频服务未加载")?;
        unsafe { library.func(name) }
            .with_context(|| format!("{} 缺少当前封装所需导出 {name}", library.path().display()))
    }

    pub(super) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.poisoned {
            bail!("设备服务回调状态已失效，请重新连接");
        }
        match request {
            ServiceRequest::HidDevices => hid::enumerate(),
            ServiceRequest::UsbDevices => usb::enumerate(),
            ServiceRequest::StartMacroRecording => self.start_macro_recording(),
            ServiceRequest::StopMacroRecording => self.stop_macro_recording(),
            ServiceRequest::MacroRecordingEvents => Ok(self.macro_recorder.events()),
            ServiceRequest::SuspendMacroMappings => self.suspend_macro_mappings(),
            ServiceRequest::ResumeMacroMappings => self.resume_macro_mappings(),
            ServiceRequest::ReceiverWirelessStatus {
                path,
                device_container_id,
            } => receiver::query(&path, &device_container_id),
            ServiceRequest::SimpleVersion | ServiceRequest::AudioDevices => {
                self.simple()?;
                let symbol = if matches!(request, ServiceRequest::SimpleVersion) {
                    "simpleGetVersionInfo"
                } else {
                    "simpleEnumerateAudioDevices"
                };
                // Both methods declare void(callback(bool,string,string)).
                let query: Query = unsafe { self.simple_symbol(symbol)? };
                let value = self
                    .call(|| unsafe { query(callback3) })?
                    .value
                    .context("原生服务未返回结果")?;
                anyhow::ensure!(!value.trim().is_empty(), "{symbol} 返回空结果");
                if matches!(request, ServiceRequest::AudioDevices) {
                    let devices: Value =
                        serde_json::from_str(&value).context("音频设备列表不是有效 JSON")?;
                    anyhow::ensure!(devices.is_array(), "音频设备列表不是数组");
                    Ok(devices)
                } else {
                    Ok(serde_json::from_str(&value).unwrap_or(Value::String(value)))
                }
            }
            ServiceRequest::GlobalMode | ServiceRequest::GlobalShortcuts => {
                self.mapping()?;
                let symbol = if matches!(request, ServiceRequest::GlobalMode) {
                    "getGlobalMode"
                } else {
                    "getGlobalShortcuts"
                };
                let query: Query = unsafe { self.mapping_symbol(symbol)? };
                let value = self
                    .call(|| unsafe { query(callback3) })?
                    .value
                    .context("映射引擎未返回结果")?;
                anyhow::ensure!(!value.trim().is_empty(), "{symbol} 返回空结果");
                Ok(serde_json::from_str(&value).unwrap_or(Value::String(value)))
            }
            ServiceRequest::RegisterShortcut {
                vkey_code,
                modifiers,
                argument,
            } => {
                self.mapping()?;
                if !self.event_callback_installed {
                    // Original createCbGlobalShortcutEvent: void(int,string,ulonglong).
                    let install: SetShortcutCallback =
                        unsafe { self.mapping_symbol("setGlobalShortcutEventCallback")? };
                    self.call(|| unsafe { install(shortcut_event, callback2) })?;
                    self.event_callback_installed = true;
                }
                let argument = CString::new(argument).context("快捷键参数含 NUL")?;
                let register: RegisterShortcut =
                    unsafe { self.mapping_symbol("registerGlobalShortcut")? };
                let result = self.call(|| unsafe {
                    register(vkey_code, modifiers, argument.as_ptr(), callback2)
                });
                if self.poisoned {
                    // The completion callback did not establish that native code
                    // has finished with the argument. Keep it until process exit.
                    std::mem::forget(argument);
                }
                result?;
                Ok(json!({"registered": true, "vkey_code": vkey_code, "modifiers": modifiers}))
            }
            ServiceRequest::UnregisterShortcut {
                vkey_code,
                modifiers,
            } => {
                self.mapping()?;
                let unregister: UnregisterShortcut =
                    unsafe { self.mapping_symbol("unregisterGlobalShortcut")? };
                self.call(|| unsafe { unregister(vkey_code, modifiers, callback2) })?;
                Ok(json!({"registered": false, "vkey_code": vkey_code, "modifiers": modifiers}))
            }
            ServiceRequest::SubmitGlobalShortcutMappings { app_engine } => {
                if !app_engine.get("mappings").is_some_and(Value::is_array)
                    || app_engine.get("hash").is_none()
                {
                    bail!("原引擎映射须包含 generateAppEngineMappings 生成的 mappings 和 hash");
                }
                self.mapping()?;
                let key = c"synapseGlobalShortcuts";
                let value = CString::new(json!({"appEngine": app_engine}).to_string())?;
                let set: StorageSetItem = unsafe { self.mapping_symbol("localStorageSetItem")? };
                let result = self.call(|| unsafe { set(key.as_ptr(), value.as_ptr(), callback2) });
                if self.poisoned {
                    std::mem::forget(value);
                }
                result?;
                Ok(json!({"accepted": true}))
            }
            ServiceRequest::ShortcutEvents => Ok(Value::Array(
                self.shortcut_events.try_iter().take(512).collect(),
            )),
            ServiceRequest::Shutdown => {
                let mut errors = Vec::new();
                self.shutdown_macro_recording(&mut errors);
                if self.mapping_initialized && !self.poisoned {
                    let result = (|| {
                        let shutdown: Initialize =
                            unsafe { self.mapping_symbol("mappingEngineShutdown")? };
                        self.call(|| unsafe { shutdown(callback0) })
                    })();
                    match result {
                        Ok(_) => self.mapping_initialized = false,
                        Err(error) => errors.push(format!("映射引擎关闭失败：{error:#}")),
                    }
                }
                // A missing export or rejected mapping shutdown must not skip
                // the independent audio service. A timed-out callback must not
                // be allowed to satisfy any later native call.
                if self.simple_initialized && !self.poisoned {
                    let result = (|| {
                        let shutdown: Initialize =
                            unsafe { self.simple_symbol("simpleServiceShutdown")? };
                        self.call(|| unsafe { shutdown(callback0) })
                    })();
                    match result {
                        Ok(_) => self.simple_initialized = false,
                        Err(error) => errors.push(format!("音频服务关闭失败：{error:#}")),
                    }
                }
                if !errors.is_empty() {
                    bail!("{}", errors.join("；"));
                }
                Ok(json!({"stopped": true}))
            }
        }
    }
}
