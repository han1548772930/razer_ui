//! Current AudioMarie FFI table plus current 058e/05a6 native exports. The local
//! product resource is loaded only in the isolated service process at runtime.
//! Development verification never loads the DLL or calls its exports.
use super::*;
use crate::dll::EngineLibrary;
use razer_device::seiren_eq::{NativeOperation, NativeReply, NativeTransport};
use razer_platform::native_paths::EnginePaths;
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString, c_char, c_void},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

type Lifecycle = unsafe extern "C" fn(*const c_char) -> u8;
type Mode = unsafe extern "C" fn(*const c_char) -> i32;
type DeviceLevel = unsafe extern "C" fn(*const c_char, u32, u32) -> u8;
type ReadDeviceLevel = unsafe extern "C" fn(*const c_char, u32) -> i32;
type ReadDeviceMute = unsafe extern "C" fn(*const c_char, u32) -> u8;
// Current exports read full 32-bit registers for these BOOL-like arguments,
// unlike the C++ bool return in AL. IDA boundaries and instructions are retained.
type DeviceMute = unsafe extern "C" fn(*const c_char, u32, i32) -> u8;
type Monitor = unsafe extern "C" fn(*const c_char, i32) -> u8;
type MixLevel = unsafe extern "C" fn(*const c_char, u32, u32, u32) -> u8;
type MixEnable = unsafe extern "C" fn(*const c_char, u32, u32, i32) -> u8;
type Event = unsafe extern "C" fn(*const c_void) -> u8;
type Version = unsafe extern "C" fn() -> *mut c_char;
type FreeMalloc = unsafe extern "C" fn(*mut c_void);
fn events() -> &'static Mutex<BTreeMap<String, Vec<Value>>> {
    static EVENTS: OnceLock<Mutex<BTreeMap<String, Vec<Value>>>> = OnceLock::new();
    EVENTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}
// Current058e/05a6 0x4cd0/0x5d90 call the callback with one NUL-terminated
// UTF-8 JSON argument and destroy their std::string immediately afterwards.
// This borrowed pointer must be copied here, never passed to FreeMalloc.
unsafe extern "C" fn native_event(pointer: *const c_char) {
    if pointer.is_null() {
        return;
    }
    let copied = unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec();
    match serde_json::from_slice::<Value>(&copied) {
        Ok(value) => {
            if let Some(owner) = value["containerId"].as_str().map(str::to_owned) {
                if let Ok(mut queues) = events().lock() {
                    if let Some(queue) = queues.get_mut(&owner) {
                        queue.push(value);
                    }
                }
            }
        }
        Err(error) => eprintln!("[Marie native callback] invalid JSON: {error}"),
    }
}

pub(super) struct Session {
    _library: EngineLibrary,
    owner: CString,
    initialize: Lifecycle,
    terminate: Lifecycle,
    mode: Mode,
    volume: DeviceLevel,
    mute: DeviceMute,
    read_volume: ReadDeviceLevel,
    read_mute: ReadDeviceMute,
    monitor_level: Monitor,
    monitor_enabled: Monitor,
    sample_rate: Monitor,
    set_mode: Monitor,
    playback_external: Monitor,
    meter_enabled: Monitor,
    stream_monitor: Monitor,
    mix_level: MixLevel,
    mix_enable: MixEnable,
    version: Option<String>,
    stream_enabled: AtomicBool,
    active: AtomicBool,
}
impl Session {
    pub(super) fn open(product_id: u32, container: &str) -> anyhow::Result<Self> {
        ensure!(
            matches!(product_id, 1422 | 1446),
            "Marie audio product has no verified native ABI"
        );
        ensure!(
            !container.is_empty(),
            "Marie audio observed container is absent"
        );
        let owner = CString::new(container).context("Marie audio container contains a NUL")?;
        let common = EnginePaths::discover()
            .apps_common
            .context("Installed local Apps/Common directory is absent")?;
        let directory = common.join(product_id.to_string());
        ensure!(
            directory.is_dir(),
            "Installed Marie product resource directory is absent"
        );
        let paths = EnginePaths {
            common_dll: None,
            apps_common: Some(directory),
        };
        let path = paths
            .resolve(&format!("RzNative_{product_id:04x}"))
            .context("Installed current Marie native resource is absent")?;
        let library = EngineLibrary::load(&path)?;
        // SAFETY: 1422/1446 current wrapper FFI table and both complete Hex-Rays
        // captures establish narrow UTF-8 strings, 32-bit Windows long/int and
        // bool return AL. All pointers remain owned by this live library.
        let (
            initialize,
            terminate,
            mode,
            volume,
            mute,
            read_volume,
            read_mute,
            monitor_level,
            monitor_enabled,
            event,
        ) = unsafe {
            (
                library
                    .func::<Lifecycle>("DeviceInitialize")
                    .context("Marie DeviceInitialize export is absent")?,
                library
                    .func::<Lifecycle>("DeviceTerminate")
                    .context("Marie DeviceTerminate export is absent")?,
                library
                    .func::<Mode>("GetMode")
                    .context("Marie GetMode export is absent")?,
                library
                    .func::<DeviceLevel>("SetDeviceVolume")
                    .context("Marie SetDeviceVolume export is absent")?,
                library
                    .func::<DeviceMute>("SetDeviceMute")
                    .context("Marie SetDeviceMute export is absent")?,
                library
                    .func::<ReadDeviceLevel>("GetDeviceVolume")
                    .context("Marie GetDeviceVolume export is absent")?,
                library
                    .func::<ReadDeviceMute>("GetDeviceMute")
                    .context("Marie GetDeviceMute export is absent")?,
                library
                    .func::<Monitor>("SetMicMonitorLevel")
                    .context("Marie SetMicMonitorLevel export is absent")?,
                library
                    .func::<Monitor>("SetMicMonitorEnable")
                    .context("Marie SetMicMonitorEnable export is absent")?,
                library
                    .func::<Event>("SetNodeFFIEvent")
                    .context("Marie SetNodeFFIEvent export is absent")?,
            )
        };
        let sample_rate = unsafe { library.func::<Monitor>("SetMasterSampleRate") }
            .context("Marie SetMasterSampleRate export is absent")?;
        let set_mode = unsafe { library.func::<Monitor>("SetMode") }
            .context("Marie SetMode export is absent")?;
        let playback_external = unsafe { library.func::<Monitor>("SetPlaybackExternal") }
            .context("Marie SetPlaybackExternal export is absent")?;
        let meter_enabled = unsafe { library.func::<Monitor>("SetLevelMeterEnable") }
            .context("Marie SetLevelMeterEnable export is absent")?;
        let stream_monitor = unsafe { library.func::<Monitor>("SetStreamMixMonitor") }
            .context("Marie SetStreamMixMonitor export is absent")?;
        let mix_level = unsafe { library.func::<MixLevel>("SetMixLevel") }
            .context("Marie SetMixLevel export is absent")?;
        let mix_enable = unsafe { library.func::<MixEnable>("SetMixEnable") }
            .context("Marie SetMixEnable export is absent")?;
        let get_version = unsafe { library.func::<Version>("GetDLLVersion") }
            .context("Marie GetDLLVersion export is absent")?;
        let free = unsafe { library.func::<FreeMalloc>("FreeMalloc") }
            .context("Marie FreeMalloc export is absent")?;
        events()
            .lock()
            .map_err(|_| anyhow::anyhow!("Marie native event owner poisoned"))?
            .entry(container.to_owned())
            .or_default();
        // Both current DLLs allocate this string at RVA 0x34a60. Unlike the
        // event callback's borrowed buffer, this return belongs to the caller
        // and must be freed by the same DLL's FreeMalloc (RVA 0x34a40).
        let pointer = unsafe { get_version() };
        let version = if pointer.is_null() {
            None
        } else {
            let copied = unsafe { CStr::from_ptr(pointer) }
                .to_str()
                .map(str::to_owned);
            unsafe { free(pointer.cast()) };
            match copied {
                Ok(version) => Some(version),
                Err(error) => {
                    eprintln!("[Marie version] invalid UTF-8: {error}");
                    None
                }
            }
        };
        // ConfigureFFI succeeded once all exports resolve. The source logs
        // this bool but still calls DeviceInitialize when it returns false.
        if unsafe { event(native_event as *const () as *const c_void) } == 0 {
            eprintln!("[Marie native callback] SetNodeFFIEvent returned false");
        }
        Ok(Self {
            _library: library,
            owner,
            initialize,
            terminate,
            mode,
            volume,
            mute,
            read_volume,
            read_mute,
            monitor_level,
            monitor_enabled,
            sample_rate,
            set_mode,
            playback_external,
            meter_enabled,
            stream_monitor,
            mix_level,
            mix_enable,
            version,
            stream_enabled: AtomicBool::new(false),
            active: AtomicBool::new(false),
        })
    }
    pub(super) fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    pub(super) fn drain(&self) -> anyhow::Result<Vec<Value>> {
        let owner = self.owner.to_str()?;
        let mut queues = events()
            .lock()
            .map_err(|_| anyhow::anyhow!("Marie native event owner poisoned"))?;
        Ok(queues
            .get_mut(owner)
            .map(std::mem::take)
            .unwrap_or_default())
    }
}
impl NativeTransport for Session {
    fn stream_mixer_enabled(&self) -> bool {
        self.stream_enabled.load(Ordering::Acquire)
    }
    fn call(&self, operation: &NativeOperation) -> anyhow::Result<NativeReply> {
        let owner = self.owner.as_ptr();
        // SAFETY: exports/signatures validated before initialization; CString
        // and the library remain alive. The container session is serialized.
        let result = unsafe {
            match *operation {
                NativeOperation::DeviceInitialize => {
                    // Terminate partially initialized owners even when the native
                    // constructor returns false or request cancellation follows.
                    self.active.store(true, Ordering::Release);
                    NativeReply::Boolean((self.initialize)(owner) != 0)
                }
                NativeOperation::GetMode => {
                    let mode = (self.mode)(owner);
                    if mode == 0 || mode == 1 {
                        self.stream_enabled.store(mode == 1, Ordering::Release);
                    }
                    NativeReply::Long(mode)
                }
                NativeOperation::SetMasterSampleRate { rate } => {
                    NativeReply::Boolean((self.sample_rate)(owner, rate as i32) != 0)
                }
                NativeOperation::SetMode { enabled } => {
                    self.stream_enabled.store(enabled, Ordering::Release);
                    NativeReply::Boolean((self.set_mode)(owner, i32::from(enabled)) != 0)
                }
                NativeOperation::SetPlaybackExternal { enabled } => {
                    NativeReply::Boolean((self.playback_external)(owner, i32::from(enabled)) != 0)
                }
                NativeOperation::SetLevelMeterEnable { enabled } => {
                    NativeReply::Boolean((self.meter_enabled)(owner, i32::from(enabled)) != 0)
                }
                NativeOperation::SetStreamMixMonitor { enabled } => {
                    NativeReply::Boolean((self.stream_monitor)(owner, i32::from(enabled)) != 0)
                }
                NativeOperation::SetMixLevel {
                    output_id,
                    input_id,
                    level,
                } => NativeReply::Boolean((self.mix_level)(owner, output_id, input_id, level) != 0),
                NativeOperation::SetMixEnable {
                    output_id,
                    input_id,
                    enabled,
                } => NativeReply::Boolean(
                    (self.mix_enable)(owner, output_id, input_id, i32::from(enabled)) != 0,
                ),
                NativeOperation::GetDeviceVolume { device_id } => {
                    NativeReply::Long((self.read_volume)(owner, device_id))
                }
                NativeOperation::GetDeviceMute { device_id } => {
                    NativeReply::Boolean((self.read_mute)(owner, device_id) != 0)
                }
                NativeOperation::SetDeviceVolume { device_id, level } => {
                    NativeReply::Boolean((self.volume)(owner, device_id, level) != 0)
                }
                NativeOperation::SetDeviceMute { device_id, muted } => {
                    NativeReply::Boolean((self.mute)(owner, device_id, i32::from(muted)) != 0)
                }
                NativeOperation::SetMicMonitorLevel { level } => {
                    NativeReply::Boolean((self.monitor_level)(owner, level) != 0)
                }
                NativeOperation::SetMicMonitorEnable { enabled } => {
                    NativeReply::Boolean((self.monitor_enabled)(owner, i32::from(enabled)) != 0)
                }
                NativeOperation::DeviceTerminate => {
                    let result = (self.terminate)(owner) != 0;
                    if result {
                        self.active.store(false, Ordering::Release);
                    }
                    NativeReply::Boolean(result)
                }
            }
        };
        Ok(result)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if self.active.swap(false, Ordering::AcqRel) {
            // Last-resort cleanup runs before unloading code or captured owner.
            unsafe {
                (self.terminate)(self.owner.as_ptr());
            }
        }
        if let Ok(owner) = self.owner.to_str() {
            if let Ok(mut queues) = events().lock() {
                queues.remove(owner);
            }
        }
    }
}
