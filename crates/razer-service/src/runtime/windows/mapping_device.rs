//! Current host 4.0.827 mapping_engine/win/index.js: addUsbDevice (1445),
//! removeUsbDevice (1630), localStorageSetItem (3520), cbDeviceEvent (6070).
//! A worker owns one USB registration until explicit stop/shutdown. Storage
//! callback acknowledgement is distinct from HID state or local persistence.
use super::{NativeRuntime, callback2, callback3, copy_string, native_target};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
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
type HypershiftCallback = unsafe extern "C" fn(bool, *const c_char, bool);
type QueryHypershift = unsafe extern "C" fn(HypershiftCallback);
unsafe extern "C" fn hypershift_callback(success: bool, reason: *const c_char, enabled: bool) {
    super::complete(super::CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: Some(enabled.to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration(owner: &str, container: &str) -> Registration {
        Registration {
            logical_product: 515,
            info: json!({"productId":515,"containerId":container,"usbInstanceId":container,"guid":owner}),
            physical: Value::Null,
            ready: true,
            unsupported_registered: false,
            mode_registered: true,
            input_registered: false,
            pending: VecDeque::new(),
        }
    }

    #[test]
    fn device_events_keep_separate_owner_queues_and_do_not_consume_another_owner() {
        let (_, receiver) = mpsc::sync_channel(1);
        let mut state = State {
            registrations: BTreeMap::from([
                ("a".into(), registration("a", "one")),
                ("b".into(), registration("b", "two")),
            ]),
            receiver,
        };
        let second = json!({"deviceInfo":state.registration("b").unwrap().info,"jsonEvent":{"type":"error","error":"driver gone"}});
        state.retain(second.clone()).unwrap();
        let mode = json!({"source":"devicemodechanged","eventType":1,"event":"{}","timeTick":10});
        state.retain(mode.clone()).unwrap();
        let first = state.registration_mut("a").unwrap();
        assert!(first.ready);
        assert_eq!(first.pending.drain(..).collect::<Vec<_>>(), [mode.clone()]);
        let second_owner = state.registration_mut("b").unwrap();
        assert!(!second_owner.ready);
        assert_eq!(
            second_owner.pending.drain(..).collect::<Vec<_>>(),
            [second, mode]
        );
    }

    #[test]
    fn previous_registration_event_cannot_enter_replacement_owner_queue() {
        let (_, receiver) = mpsc::sync_channel(1);
        let mut state = State {
            registrations: BTreeMap::from([("new".into(), registration("new", "same-device"))]),
            receiver,
        };
        state.retain(json!({"deviceInfo":registration("old", "same-device").info,"jsonEvent":{"type":"info","info":"driver ready"}})).unwrap();
        assert!(state.registration("new").unwrap().pending.is_empty());
    }
}
pub(super) unsafe extern "C" fn mode_event(kind: i32, data: *const c_char, tick: u64) {
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
    pending: VecDeque<Value>,
}
pub(super) struct State {
    registrations: BTreeMap<String, Registration>,
    receiver: Receiver<Value>,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(512);
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(sender);
        }
        OVERFLOW.store(false, Ordering::Release);
        Self {
            registrations: BTreeMap::new(),
            receiver,
        }
    }
    pub(super) fn owners(&self) -> Vec<String> {
        self.registrations.keys().cloned().collect()
    }
    pub(super) fn contains_owner(&self, owner: &str) -> bool {
        self.registrations.contains_key(owner)
    }
    fn registration(&self, owner: &str) -> anyhow::Result<&Registration> {
        self.registrations
            .get(owner)
            .context("Mapping USB owner is not registered")
    }
    fn registration_mut(&mut self, owner: &str) -> anyhow::Result<&mut Registration> {
        self.registrations
            .get_mut(owner)
            .context("Mapping USB owner is not registered")
    }
    fn retain(&mut self, event: Value) -> anyhow::Result<()> {
        for registration in self.registrations.values_mut() {
            if !registration.matches(&event) {
                continue;
            }
            ensure!(
                registration.pending.len() < 512,
                "Mapping event consumer fell behind; event state is incomplete"
            );
            if event["jsonEvent"]["type"] == "error" {
                registration.ready = false;
            }
            registration.pending.push_back(event.clone());
        }
        Ok(())
    }
}
impl Registration {
    fn matches(&self, event: &Value) -> bool {
        if event["source"] == "devicemodechanged" {
            return self.mode_registered;
        }
        let info = &event["deviceInfo"];
        info["productId"] == self.info["productId"]
            && info["containerId"]
                .as_str()
                .zip(self.info["containerId"].as_str())
                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
            && info["usbInstanceId"] == self.info["usbInstanceId"]
            && info
                .get("guid")
                .is_none_or(|guid| guid == &self.info["guid"])
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
            !self.mapping_device.contains_owner(owner),
            "Mapping device owner is already registered"
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
        self.mapping_device.registrations.insert(
            owner.to_owned(),
            Registration {
                logical_product: product,
                info,
                physical,
                ready: false,
                unsupported_registered: false,
                mode_registered: false,
                input_registered: false,
                pending: VecDeque::new(),
            },
        );
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
            let matches = self.mapping_device.registration(owner)?.matches(&event);
            if !matches {
                self.mapping_device.retain(event)?;
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
                self.mapping_device.registration_mut(owner)?.ready = true;
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
            .registration_mut(owner)?
            .unsupported_registered = true;
        let set: SetUnsupported = unsafe { self.mapping_symbol("setUnsupportedMappingCallback")? };
        let result = self.call(|| unsafe { set(info.as_ptr(), unsupported_event, callback3) });
        if self.poisoned {
            std::mem::forget(info);
        }
        result?;
        self.mapping_device.registration_mut(owner)?.mode_registered = true;
        self.retain_mapping_mode_events(super::mapping_mode_events::Owner::Device(
            owner.to_owned(),
        ))?;
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
            .registration_mut(owner)?
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
        let events = self
            .mapping_device
            .registration_mut(owner)?
            .pending
            .drain(..)
            .collect::<Vec<_>>();
        Ok(json!({"events":events,"owner_guid":owner}))
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
                self.release_mapping_mode_events(super::mapping_mode_events::Owner::Device(
                    owner.to_owned(),
                ))?;
                self.mapping_device.registration_mut(owner)?.mode_registered = false;
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
        self.mapping_device.registrations.remove(owner);
        while let Ok(event) = self.mapping_device.receiver.try_recv() {
            self.mapping_device.retain(event)?;
        }
        if let Some(error) = unregister_error {
            return Err(error);
        }
        Ok(json!({"removed":true,"owner_guid":owner}))
    }
}
