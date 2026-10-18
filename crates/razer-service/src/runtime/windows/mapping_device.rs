//! Current host 4.0.827 mapping_engine/win/index.js: addUsbDevice (1445),
//! removeUsbDevice (1630), localStorageSetItem (3520), cbDeviceEvent (6070).
//! A worker owns one USB registration until explicit stop/shutdown. Storage
//! callback acknowledgement is distinct from HID state or local persistence.
use super::{NativeRuntime, callback2, callback3, copy_string, native_target};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    ffi::{CString, c_char},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Instant,
};

type DeviceEvent = unsafe extern "C" fn(*const c_char, i32, *const c_char, u64);
type AddDevice = unsafe extern "C" fn(*const c_char, DeviceEvent, super::Callback3);
type RemoveDevice = unsafe extern "C" fn(*const c_char, super::Callback3);
type UnsupportedEvent = unsafe extern "C" fn(*const c_char, i32, *const c_char, *const c_char, u64);
type SetUnsupported = unsafe extern "C" fn(*const c_char, UnsupportedEvent, super::Callback3);
type SetInput = unsafe extern "C" fn(*const c_char, DeviceEvent, super::Callback3);
static EVENTS: Mutex<Option<SyncSender<Value>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);
type ModeEvent = unsafe extern "C" fn(i32, *const c_char, u64);
type InstallMode = unsafe extern "C" fn(ModeEvent, super::Callback2);
type ModeOperation = unsafe extern "C" fn(super::Callback2);
type HypershiftCallback = unsafe extern "C" fn(bool, *const c_char, bool);
type QueryHypershift = unsafe extern "C" fn(HypershiftCallback);
unsafe extern "C" fn hypershift_callback(success: bool, reason: *const c_char, enabled: bool) {
    super::complete(super::CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: Some(enabled.to_string()),
    });
}
unsafe extern "C" fn mode_event(kind: i32, data: *const c_char, tick: u64) {
    let event = json!({"source":"devicemodechanged","eventType":kind,"event":unsafe { copy_string(data) },"timeTick":tick});
    if let Ok(slot) = EVENTS.lock() {
        if let Some(sender) = slot.as_ref() {
            if sender.try_send(event).is_err() {
                OVERFLOW.store(true, Ordering::Release);
            }
        }
    }
}

unsafe extern "C" fn unsupported_event(
    info: *const c_char,
    kind: i32,
    input: *const c_char,
    output: *const c_char,
    tick: u64,
) {
    if kind != 2 {
        return;
    }
    let event = json!({"type":"unsupportedmapping", "input":unsafe { copy_string(input) }, "output":unsafe { copy_string(output) }});
    if let Ok(text) = CString::new(event.to_string()) {
        unsafe {
            device_event(info, kind, text.as_ptr(), tick);
        }
    }
}

unsafe extern "C" fn input_event(info: *const c_char, kind: i32, input: *const c_char, tick: u64) {
    // Current createCbInputNotification forwards only native type1. Preserve
    // the exact input string; assignment decoding owns its JSON semantics.
    if kind != 1 {
        return;
    }
    let event = json!({"type":"inputnotified","input":unsafe { copy_string(input) }});
    if let Ok(text) = CString::new(event.to_string()) {
        unsafe {
            device_event(info, kind, text.as_ptr(), tick);
        }
    }
}

unsafe extern "C" fn device_event(info: *const c_char, kind: i32, event: *const c_char, tick: u64) {
    let parse = |text: String| serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
    let value = json!({
        "deviceInfo": parse(unsafe { copy_string(info) }),
        "deviceEventType": kind,
        "jsonEvent": parse(unsafe { copy_string(event) }),
        "timeTick": tick,
    });
    if let Ok(slot) = EVENTS.lock() {
        if let Some(sender) = slot.as_ref() {
            if sender.try_send(value).is_err() {
                OVERFLOW.store(true, Ordering::Release);
            }
        }
    }
}

struct Registration {
    logical_product: u32,
    info: Value,
    physical: Value,
    ready: bool,
    unsupported_registered: bool,
    mode_registered: bool,
    input_registered: bool,
}
pub(super) struct State {
    registration: Option<Registration>,
    receiver: Receiver<Value>,
    pending: VecDeque<Value>,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(512);
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(sender);
        }
        OVERFLOW.store(false, Ordering::Release);
        Self {
            registration: None,
            receiver,
            pending: VecDeque::new(),
        }
    }
    pub(super) fn owner_guid(&self) -> Option<String> {
        self.registration.as_ref()?.info["guid"]
            .as_str()
            .map(str::to_owned)
    }
    fn registration(&self, owner: &str) -> anyhow::Result<&Registration> {
        let registration = self
            .registration
            .as_ref()
            .context("Mapping USB device is not registered")?;
        ensure!(
            registration.info["guid"].as_str() == Some(owner),
            "Mapping owner has changed"
        );
        Ok(registration)
    }
    fn matches(&self, event: &Value) -> bool {
        let Some(registration) = self.registration.as_ref() else {
            return false;
        };
        if event["source"] == "devicemodechanged" {
            return registration.mode_registered;
        }
        let info = &event["deviceInfo"];
        info["productId"] == registration.info["productId"]
            && info["containerId"]
                .as_str()
                .zip(registration.info["containerId"].as_str())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            && info["usbInstanceId"] == registration.info["usbInstanceId"]
            && info
                .get("guid")
                .is_none_or(|guid| guid == &registration.info["guid"])
    }
    fn retain(&mut self, event: Value) -> anyhow::Result<()> {
        if self.matches(&event) {
            ensure!(
                self.pending.len() < 512,
                "Mapping event consumer fell behind; event state is incomplete"
            );
            if event["jsonEvent"]["type"] == "error" {
                if let Some(registration) = self.registration.as_mut() {
                    registration.ready = false;
                }
            }
            self.pending.push_back(event);
        }
        Ok(())
    }
}

// Current756 Vn ->90994 X7 forwards the actual adapter to addUsbDevice;
// 97161 Ie keys localStorage by that adapter's physical product ID. A direct
// 757 receiver belongs to logical756, without relaxing ordinary DLL targets.
fn mapping_target(product: u32, container: &str) -> anyhow::Result<(u32, Value)> {
    if product != 756 {
        return native_target(container, Some(product));
    }
    ensure!(
        super::receiver::valid_container(container),
        "756 mapping requires an observed nonzero ContainerId"
    );
    let observation = super::usb::enumerate()?;
    ensure!(
        observation["complete"] == true
            && observation["failures"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "756 mapping physical USB enumeration is incomplete"
    );
    let devices = observation["devices"]
        .as_array()
        .context("756 mapping USB observations are absent")?
        .iter()
        .filter(|device| {
            device["vendor_id"] == 5426
                && device["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect::<Vec<_>>();
    if devices.is_empty() {
        let observed = super::hid::enumerate_metadata()?;
        let target = razer_device::mouse_runtime_artwork::WindowsBleTarget::from_hid(
            756, 758, container, &observed,
        )?;
        return Ok((
            758,
            json!({"product_id":758,"vendor_id":5426,"device_container_id":target.device_container_id,"device_instance_id":target.device_instance_id,"path":target.hid_path,"ble_identity":target}),
        ));
    }
    let [device] = devices.as_slice() else {
        anyhow::bail!("756 mapping physical container is offline or ambiguous")
    };
    let physical = u32::try_from(
        device["product_id"]
            .as_u64()
            .context("756 mapping physical PID is absent")?,
    )?;
    ensure!(
        matches!(physical, 756 | 757),
        "756 mapping USB owner has a foreign physical PID"
    );
    let razer_device::device_identity::IdentityLookup::Unique(identity) =
        razer_device::device_identity::lookup(physical)
    else {
        anyhow::bail!("756 mapping physical adapter attribution is ambiguous")
    };
    ensure!(
        identity.product_id == 756 && !identity.is_ble && identity.is_dongle == (physical == 757),
        "756 mapping physical attribution differs from current factory"
    );
    for field in ["path", "device_instance_id"] {
        ensure!(
            device[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "756 mapping physical identity lacks {field}"
        );
    }
    Ok((physical, (**device).clone()))
}

impl NativeRuntime {
    pub(super) fn start_mapping_device(
        &mut self,
        product: u32,
        container: &str,
        owner: &str,
    ) -> anyhow::Result<Value> {
        ensure!(
            razer_device::mapping_document::MappingProduct::for_product_id(product).is_some(),
            "Mapping product registration has not been audited"
        );
        ensure!(
            !owner.is_empty() && !owner.contains('\0'),
            "Mapping owner identity is empty or invalid"
        );
        ensure!(
            self.mapping_device.registration.is_none(),
            "Worker already owns a mapping device"
        );
        let (physical_product, physical) = mapping_target(product, container)?;
        let info = json!({"vendorId":5426,"productId":physical_product,
            "containerId":physical["device_container_id"],
            "usbInstanceId":physical["device_instance_id"],"guid":owner});
        let text = CString::new(info.to_string())?;
        self.mapping()?;
        let add: AddDevice = unsafe { self.mapping_symbol("addUsbDevice")? };
        // Retain failed registrations too: source removeUsbDevice must release
        // partially opened devices when callback/driver readiness rejects.
        self.mapping_device.registration = Some(Registration {
            logical_product: product,
            info,
            physical,
            ready: false,
            unsupported_registered: false,
            mode_registered: false,
            input_registered: false,
        });
        let result = self.call(|| unsafe { add(text.as_ptr(), device_event, callback3) });
        if self.poisoned {
            std::mem::forget(text);
        }
        result?;
        let deadline = Instant::now() + super::CALLBACK_TIMEOUT;
        loop {
            ensure!(
                !OVERFLOW.load(Ordering::Acquire),
                "Mapping device event buffer overflowed"
            );
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = self
                .mapping_device
                .receiver
                .recv_timeout(remaining)
                .context("Mapping callback completed without a correlated driver-ready event")?;
            if !self.mapping_device.matches(&event) {
                continue;
            }
            let ready = event["jsonEvent"]["type"] == "info"
                && event["jsonEvent"]["info"] == "driver ready";
            let failure =
                (event["jsonEvent"]["type"] == "error").then(|| event["jsonEvent"].to_string());
            self.mapping_device.retain(event)?;
            if let Some(error) = failure {
                anyhow::bail!("Mapping driver rejected registration: {error}");
            }
            if ready {
                self.mapping_device.registration.as_mut().unwrap().ready = true;
                break;
            }
        }
        self.validate_mapping_device(owner)?;
        let info = CString::new(self.mapping_device.registration(owner)?.info.to_string())?;
        let register: RemoveDevice = unsafe { self.mapping_symbol("registerUnsupportedMapping")? };
        let result = self.call(|| unsafe { register(info.as_ptr(), callback3) });
        if self.poisoned {
            std::mem::forget(info);
            return result.map(|_| Value::Null);
        }
        result?;
        self.mapping_device
            .registration
            .as_mut()
            .unwrap()
            .unsupported_registered = true;
        let set: SetUnsupported = unsafe { self.mapping_symbol("setUnsupportedMappingCallback")? };
        let result = self.call(|| unsafe { set(info.as_ptr(), unsupported_event, callback3) });
        if self.poisoned {
            std::mem::forget(info);
        }
        result?;
        let register: ModeOperation =
            unsafe { self.mapping_symbol("registerDeviceModeChangedEvent")? };
        self.call(|| unsafe { register(callback2) })?;
        self.mapping_device
            .registration
            .as_mut()
            .unwrap()
            .mode_registered = true;
        let install: InstallMode =
            unsafe { self.mapping_symbol("setDeviceModeChangedEventCallback")? };
        self.call(|| unsafe { install(mode_event, callback2) })?;
        Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner}))
    }

    pub(super) fn start_otfm_input(&mut self, owner: &str) -> anyhow::Result<Value> {
        self.validate_mapping_device(owner)?;
        anyhow::ensure!(
            !self.mapping_device.registration(owner)?.input_registered,
            "OTFM input notifications are already registered"
        );
        let info = CString::new(self.mapping_device.registration(owner)?.info.to_string())?;
        let register: RemoveDevice = unsafe { self.mapping_symbol("registerInputNotification")? };
        let result = self.call(|| unsafe { register(info.as_ptr(), callback3) });
        if self.poisoned {
            std::mem::forget(info);
            return result.map(|_| Value::Null);
        }
        result?;
        self.mapping_device
            .registration
            .as_mut()
            .unwrap()
            .input_registered = true;
        let install: SetInput = unsafe { self.mapping_symbol("setInputNotificationCallback")? };
        let result = self.call(|| unsafe { install(info.as_ptr(), input_event, callback3) });
        if self.poisoned {
            std::mem::forget(info);
        }
        result?;
        Ok(json!({"registered":true,"owner_guid":owner}))
    }

    fn validate_mapping_device(&self, owner: &str) -> anyhow::Result<()> {
        let registration = self.mapping_device.registration(owner)?;
        ensure!(registration.ready, "Mapping driver is not ready");
        let container = registration.info["containerId"]
            .as_str()
            .context("Missing mapping container")?;
        let (_, current) = mapping_target(registration.logical_product, container)?;
        ensure!(
            current == registration.physical,
            "Physical USB mapping device changed"
        );
        Ok(())
    }

    pub(super) fn publish_mapping_device(
        &mut self,
        owner: &str,
        document: Value,
    ) -> anyhow::Result<Value> {
        self.validate_mapping_device(owner)?;
        ensure!(
            document["productId"] == self.mapping_device.registration(owner)?.logical_product,
            "Mapping document belongs to another product"
        );
        ensure!(
            document.get("_macro_compilation_error").is_none(),
            "Local macro draft cannot be published: {}",
            document["_macro_compilation_error"]
        );
        ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Mapping event state is incomplete after overflow"
        );
        let active = document["activeProfile"]
            .as_str()
            .context("Missing active mapping profile")?;
        let profiles = document["profiles"]
            .as_array()
            .context("Missing mapping profiles")?;
        ensure!(
            profiles
                .iter()
                .filter(|p| p["guid"].as_str() == Some(active))
                .count()
                == 1,
            "Active mapping profile is not unique"
        );
        for engine in profiles
            .iter()
            .map(|p| &p["appEngine"])
            .chain(std::iter::once(&document["defaultMappings"]["appEngine"]))
        {
            ensure!(
                engine["mappings"].is_array() && engine["hash"].is_string(),
                "Mapping engine has not been compiled"
            );
            ensure!(
                engine["hash"].as_str()
                    == Some(razer_device::mapping_engine_hash::source_hash(engine)?.as_str()),
                "Mapping engine content/hash disagree"
            );
        }
        let registration = self.mapping_device.registration(owner)?;
        let key = CString::new(format!(
            "synapse_{}_{}",
            registration.info["productId"],
            registration.info["containerId"].as_str().unwrap()
        ))?;
        // The current MW publishes these independent engine lists before the
        // device document that references their generated GUIDs.
        for (field, storage_key) in [
            ("defaultTurbos", "synapseDefaultTurbos"),
            ("synapseTurbos", "synapseTurbos"),
        ] {
            ensure!(document[field].is_array(), "Missing compiled {field} list");
            self.set_mapping_storage(storage_key, &document[field])?;
        }
        if let Some(macros) = document.get("macros") {
            ensure!(macros.is_array(), "Compiled macro store is not an array");
            self.set_mapping_storage("synapseMacros", macros)?;
        }
        if let Some(macros) = document.get("phasedMacros") {
            ensure!(
                macros.is_array(),
                "Compiled phased macro store is not an array"
            );
            self.set_mapping_storage("synapsePhasedMacros", macros)?;
        }
        let value = CString::new(document.to_string())?;
        let set: super::StorageSetItem = unsafe { self.mapping_symbol("localStorageSetItem")? };
        let result = self.call(|| unsafe { set(key.as_ptr(), value.as_ptr(), callback2) });
        if self.poisoned {
            std::mem::forget(key);
            std::mem::forget(value);
        }
        result?;
        self.validate_mapping_device(owner)?;
        Ok(json!({"published":true,"owner_guid":owner,"active_profile":active}))
    }

    fn set_mapping_storage(&mut self, key: &str, data: &Value) -> anyhow::Result<()> {
        let key = CString::new(key)?;
        let value = CString::new(data.to_string())?;
        let set: super::StorageSetItem = unsafe { self.mapping_symbol("localStorageSetItem")? };
        let result = self.call(|| unsafe { set(key.as_ptr(), value.as_ptr(), callback2) });
        if self.poisoned {
            std::mem::forget(key);
            std::mem::forget(value);
        }
        result?;
        Ok(())
    }

    pub(super) fn mapping_keyboard_indicator(
        &mut self,
        owner: &str,
        enabled: bool,
    ) -> anyhow::Result<Value> {
        self.validate_mapping_device(owner)?;
        let registration = self.mapping_device.registration(owner)?;
        ensure!(
            matches!(registration.info["productId"].as_u64(), Some(515 | 679)),
            "This owner has no audited gaming LED route"
        );
        let container = registration.info["containerId"].as_str().unwrap();
        let product = registration.info["productId"].as_u64().unwrap() as u32;
        let settings = razer_device::keyboard_settings::capability(product)
            .context("Missing source keyboard wired capability")?;
        let cap = &settings.transport;
        let observation = super::hid::enumerate()?;
        let interfaces = observation["interfaces"]
            .as_array()
            .context("Missing Windows HID interfaces")?;
        let mut paths = std::collections::BTreeMap::new();
        for interface in interfaces {
            if interface["vendor_id"] == 5426
                && interface["product_id"] == product
                && interface["claim_interface"] == cap.claim_interface_for(product)?
                && interface["feature_report_bytes"] == cap.report_bytes
                && interface["device_container_id"]
                    .as_str()
                    .is_some_and(|v| v.eq_ignore_ascii_case(container))
            {
                if let Some(path) = interface["path"].as_str() {
                    paths.insert(path.to_ascii_lowercase(), path.to_owned());
                }
            }
        }
        ensure!(
            paths.len() == 1,
            "Gaming LED HID interface is absent or ambiguous"
        );
        super::device_reads::keyboard_gaming_indicator(
            &razer_device::device_reads::DeviceReadTarget {
                product_id: product,
                physical_product_id: product,
                peer_product_id: None,
                device_container_id: container.into(),
                path: paths.into_values().next().unwrap(),
            },
            enabled,
        )
    }

    pub(super) fn mapping_device_events(&mut self, owner: &str) -> anyhow::Result<Value> {
        self.mapping_device.registration(owner)?;
        ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Mapping event buffer overflowed; events are incomplete"
        );
        while let Ok(event) = self.mapping_device.receiver.try_recv() {
            self.mapping_device.retain(event)?;
        }
        let events = self.mapping_device.pending.drain(..).collect::<Vec<_>>();
        Ok(json!({"events":events,"owner_guid":owner}))
    }

    // Current827 export getMacroRecordingMode (RVA48ee0), completion48c90:
    // void(bool,char const*,char const*). The mode is the third callback value.
    pub(super) fn mapping_device_macro_recording_mode(
        &mut self,
        owner: &str,
    ) -> anyhow::Result<Value> {
        self.validate_mapping_device(owner)?;
        let query: super::Query = unsafe { self.mapping_symbol("getMacroRecordingMode")? };
        let reply = self.call(|| unsafe { query(callback3) })?;
        let mode = reply
            .value
            .context("Macro recording mode callback lacks its value")?;
        self.validate_mapping_device(owner)?;
        Ok(json!({"owner_guid":owner,"mode":mode}))
    }
    // Current827 RVA4ae40 -> completion4ac10:
    // void isHypershiftActive(void (*)(bool,char const*,bool)).
    pub(super) fn mapping_device_hypershift_active(
        &mut self,
        owner: &str,
    ) -> anyhow::Result<Value> {
        self.validate_mapping_device(owner)?;
        let query: QueryHypershift = unsafe { self.mapping_symbol("isHypershiftActive")? };
        let reply = self.call(|| unsafe { query(hypershift_callback) })?;
        let enabled = reply
            .value
            .context("Hypershift callback lacks its value")?
            .parse::<bool>()?;
        self.validate_mapping_device(owner)?;
        Ok(json!({"owner_guid":owner,"isEnabled":enabled}))
    }

    pub(super) fn stop_mapping_device(&mut self, owner: &str) -> anyhow::Result<Value> {
        let info = self.mapping_device.registration(owner)?.info.clone();
        let text = CString::new(info.to_string())?;
        let mut unregister_error = None;
        if self.mapping_device.registration(owner)?.input_registered {
            let result = (|| -> anyhow::Result<()> {
                let unregister: RemoveDevice =
                    unsafe { self.mapping_symbol("unregisterInputNotification")? };
                self.call(|| unsafe { unregister(text.as_ptr(), callback3) })?;
                Ok(())
            })();
            if self.poisoned {
                std::mem::forget(text);
                return result.map(|_| Value::Null);
            }
            if let Err(error) = result {
                unregister_error = Some(error);
            }
        }
        if self.mapping_device.registration(owner)?.mode_registered {
            let result = (|| -> anyhow::Result<()> {
                let unregister: ModeOperation =
                    unsafe { self.mapping_symbol("unregisterDeviceModeChangedEvent")? };
                self.call(|| unsafe { unregister(callback2) })?;
                Ok(())
            })();
            if self.poisoned {
                std::mem::forget(text);
                return result.map(|_| Value::Null);
            }
            if let Err(error) = result {
                unregister_error = Some(match unregister_error {
                    Some(previous) => anyhow::anyhow!("{previous:#}; {error:#}"),
                    None => error,
                });
            }
        }
        if self
            .mapping_device
            .registration(owner)?
            .unsupported_registered
        {
            let result = (|| -> anyhow::Result<()> {
                let unregister: RemoveDevice =
                    unsafe { self.mapping_symbol("unregisterUnsupportedMapping")? };
                self.call(|| unsafe { unregister(text.as_ptr(), callback3) })?;
                Ok(())
            })();
            if self.poisoned {
                std::mem::forget(text);
                return result.map(|_| Value::Null);
            }
            if let Err(error) = result {
                unregister_error = Some(match unregister_error {
                    Some(previous) => anyhow::anyhow!("{previous:#}; {error:#}"),
                    None => error,
                });
            }
        }
        let remove: RemoveDevice = unsafe { self.mapping_symbol("removeUsbDevice")? };
        let result = self.call(|| unsafe { remove(text.as_ptr(), callback3) });
        if self.poisoned {
            std::mem::forget(text);
        }
        result?;
        self.mapping_device.registration = None;
        self.mapping_device.pending.clear();
        while self.mapping_device.receiver.try_recv().is_ok() {}
        if let Some(error) = unregister_error {
            return Err(error);
        }
        Ok(json!({"removed":true,"owner_guid":owner}))
    }
}
