//! PID70 focused recorder 2048, and current host mapping_engine/win/index.js
//! addUsbDevice, enableInputRedirectGeneric, createCbInputRedirect, removeUsbDevice.
//! Registration, redirects and temporary mapping suppression have one owner.
use super::{Callback2, Callback3, NativeRuntime, callback2, callback3, copy_string};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{
    ffi::{CString, c_char},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Instant,
};

type Event = unsafe extern "C" fn(*const c_char, i32, *const c_char, u64);
type Add = unsafe extern "C" fn(*const c_char, Event, Callback3);
type Operation = unsafe extern "C" fn(*const c_char, Callback3);
type Install = unsafe extern "C" fn(*const c_char, Event, Callback3);
type Global = unsafe extern "C" fn(Callback2);
static EVENTS: Mutex<Option<SyncSender<Value>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);
unsafe extern "C" fn event(info: *const c_char, kind: i32, data: *const c_char, tick: u64) {
    let parse = |text: String| serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
    let value = json!({"deviceInfo":parse(unsafe{copy_string(info)}),"eventType":kind,
        "input":parse(unsafe{copy_string(data)}),"timeTick":tick});
    if let Ok(slot) = EVENTS.lock() {
        if let Some(sender) = slot.as_ref() {
            if sender.try_send(value).is_err() {
                OVERFLOW.store(true, Ordering::Release);
            }
        }
    }
}
fn mouse_product(pid: u32) -> bool {
    let razer_device::device_identity::IdentityLookup::Unique(identity) =
        razer_device::device_identity::lookup(pid)
    else {
        return false;
    };
    matches!(
        razer_device::device_identity::middleware_category(identity.product_id),
        Some("MOUSE" | "MOUSEPLUSMAT")
    )
}
fn ble_identity(parent: &str) -> Option<(u32, u32)> {
    if !parent.contains("BTHLEDevice") {
        return None;
    }
    let vendor_at = parent.find("_VID&")? + 7;
    let product_at = parent.find("_PID&")? + 5;
    let vendor = u32::from_str_radix(parent.get(vendor_at..vendor_at + 4)?, 16).ok()?;
    let product = u32::from_str_radix(parent.get(product_at..product_at + 4)?, 16).ok()?;
    if !matches!(vendor, 0x1532 | 0x068e) || product == 0 {
        return None;
    }
    let suffix = parent.get(parent.find("_REV&")?..)?;
    let mac = suffix.get(suffix.rfind('_')? + 1..)?.get(..12)?;
    if !mac.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some((vendor, product))
}

fn owner_guid() -> anyhow::Result<String> {
    let mut guid = windows_sys::core::GUID::default();
    let result = unsafe { windows_sys::Win32::System::Com::CoCreateGuid(&mut guid) };
    ensure!(
        result >= 0,
        "Capture owner UUID generation failed: {result:#x}"
    );
    Ok(format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7]
    ))
}
struct Registration {
    info: Value,
    redirected: bool,
    keyboard_only: bool,
}
pub(super) struct State {
    registrations: Vec<Registration>,
    events: Receiver<Value>,
    suspended: bool,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, events) = mpsc::sync_channel(512);
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(sender);
        }
        Self {
            registrations: Vec::new(),
            events,
            suspended: false,
        }
    }
    pub(super) fn active(&self) -> bool {
        self.suspended || !self.registrations.is_empty()
    }
}
impl NativeRuntime {
    pub(super) fn start_mapping_key_capture(&mut self) -> anyhow::Result<Value> {
        self.start_mapping_key_capture_for(None)
    }
    pub(super) fn start_keyboard_calibration_capture(
        &mut self,
        product_id: u32,
        container_id: &str,
    ) -> anyhow::Result<Value> {
        ensure!(
            [740, 746].contains(&product_id) && super::receiver::valid_container(container_id),
            "Calibration capture has no current keyboard/container identity"
        );
        self.start_mapping_key_capture_for(Some((product_id, container_id)))
    }
    fn start_mapping_key_capture_for(
        &mut self,
        capture_target: Option<(u32, &str)>,
    ) -> anyhow::Result<Value> {
        ensure!(
            !self.mapping_key_capture.active(),
            "Mapping key capture is already active"
        );
        ensure!(
            !self.shortcut_capture.active() && !self.macro_recorder.active(),
            "Another input recorder owns temporary mapping suppression"
        );
        let observation = super::usb::enumerate()?;
        ensure!(
            observation["complete"] == true
                && observation["failures"]
                    .as_array()
                    .is_some_and(Vec::is_empty),
            "USB capture enumeration is incomplete"
        );
        let pairing = razer_storage::receiver_pairing::PairingCache::load(
            &razer_storage::receiver_pairing::local_path(),
        )?
        .records();
        let dual_link = pairing
            .as_object()
            .into_iter()
            .flat_map(|rows| rows.values())
            .filter_map(|row| row["master"]["dongleId"].as_u64())
            .collect::<Vec<_>>();
        let mut targets = Vec::new();
        let mut omitted = Vec::new();
        for physical in observation["devices"]
            .as_array()
            .context("USB capture devices missing")?
        {
            let Some(vendor) = physical["vendor_id"]
                .as_u64()
                .filter(|vendor| [0x1532, 0x068e].contains(vendor))
            else {
                continue;
            };
            let pid = u32::try_from(
                physical["product_id"]
                    .as_u64()
                    .context("USB capture product id missing")?,
            )?;
            let keyboard_only = capture_target.is_some() || dual_link.contains(&u64::from(pid));
            if !keyboard_only && mouse_product(pid) {
                continue;
            }
            let container = physical["device_container_id"]
                .as_str()
                .context("USB capture container missing")?;
            if capture_target.is_some_and(|(target_pid, target_container)| {
                target_pid != pid || !target_container.eq_ignore_ascii_case(container)
            }) {
                continue;
            }
            let instance = physical["device_instance_id"]
                .as_str()
                .context("USB capture instance missing")?;
            ensure!(
                super::receiver::valid_container(container) && !instance.is_empty(),
                "Invalid USB capture identity"
            );
            targets.push((json!({"vendorId":vendor,"productId":pid,"containerId":container,"usbInstanceId":instance,"guid":owner_guid()?}),keyboard_only));
        }
        // Current noble/util.js 206..258 identifies already installed BLE HID
        // endpoints by the immediate BTHLEDevice parent. No synthetic MAC,
        // advertisement, cached online flag or USB instance is substituted.
        if capture_target.is_none() {
            let hid = super::hid::enumerate_metadata()?;
            ensure!(
                hid["complete"] == true,
                "BLE HID endpoint enumeration is incomplete"
            );
            for endpoint in hid["interfaces"]
                .as_array()
                .context("BLE HID endpoint list missing")?
            {
                let Some(parent) = endpoint["parent_instance_id"].as_str() else {
                    continue;
                };
                let Some((vendor, pid)) = ble_identity(parent) else {
                    continue;
                };
                if mouse_product(pid) {
                    continue;
                }
                let Some(container) = endpoint["device_container_id"]
                    .as_str()
                    .filter(|container| super::receiver::valid_container(container))
                else {
                    omitted.push(json!({"product_id":pid,"reason":"Observed BLE HID endpoint has no valid container"}));
                    continue;
                };
                if targets.iter().any(|(info, _)| {
                    info["productId"] == pid
                        && info["containerId"]
                            .as_str()
                            .is_some_and(|id| id.eq_ignore_ascii_case(container))
                }) {
                    continue;
                }
                targets.push((json!({"vendorId":vendor,"productId":pid,"containerId":container.to_uppercase(),"guid":owner_guid()?}),false));
            }
        }
        if capture_target.is_some() {
            ensure!(
                targets.len() == 1,
                "Calibration capture target disappeared or has ambiguous physical observations"
            );
        }
        self.mapping()?;
        OVERFLOW.store(false, Ordering::Release);
        while self.mapping_key_capture.events.try_recv().is_ok() {}
        let started = (|| -> anyhow::Result<()> {
            let disable: Global = unsafe { self.mapping_symbol("disableMapping")? };
            self.call(|| unsafe { disable(callback2) })?;
            self.mapping_key_capture.suspended = true;
            for (info, keyboard_only) in targets {
                let text = CString::new(info.to_string())?;
                self.mapping_key_capture.registrations.push(Registration {
                    info: info.clone(),
                    redirected: false,
                    keyboard_only,
                });
                let add: Add = unsafe { self.mapping_symbol("addUsbDevice")? };
                let added = self.call(|| unsafe { add(text.as_ptr(), event, callback3) });
                if self.poisoned {
                    std::mem::forget(text);
                    return added.map(|_| ());
                }
                added?;
                let deadline = Instant::now() + super::CALLBACK_TIMEOUT;
                loop {
                    ensure!(
                        !OVERFLOW.load(Ordering::Acquire),
                        "Mapping key capture buffer overflowed"
                    );
                    let value = self
                        .mapping_key_capture
                        .events
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .context("Capture registration had no correlated driver-ready event")?;
                    if value["deviceInfo"]["guid"] != info["guid"] {
                        continue;
                    }
                    ensure!(
                        value["input"]["type"] != "error",
                        "Capture registration rejected: {}",
                        value["input"]
                    );
                    if value["input"]["type"] == "info" && value["input"]["info"] == "driver ready"
                    {
                        break;
                    }
                }
                let enable: Operation = unsafe {
                    self.mapping_symbol(if keyboard_only {
                        "enableKeyboardInputRedirect"
                    } else {
                        "enableInputRedirect"
                    })?
                };
                let enabled = self.call(|| unsafe { enable(text.as_ptr(), callback3) });
                if self.poisoned {
                    std::mem::forget(text);
                    return enabled.map(|_| ());
                }
                enabled?;
                self.mapping_key_capture
                    .registrations
                    .last_mut()
                    .unwrap()
                    .redirected = true;
                let install: Install = unsafe { self.mapping_symbol("setInputRedirectCallback")? };
                let installed = self.call(|| unsafe { install(text.as_ptr(), event, callback3) });
                if self.poisoned {
                    std::mem::forget(text);
                    return installed.map(|_| ());
                }
                installed?;
            }
            Ok(())
        })();
        if let Err(error) = started {
            return match self.stop_mapping_key_capture() {
                Ok(_) => Err(error),
                Err(cleanup) => Err(anyhow::anyhow!(
                    "{error:#}; capture cleanup failed: {cleanup:#}"
                )),
            };
        }
        Ok(
            json!({"capturing":true,"redirected_devices":self.mapping_key_capture.registrations.len(),
                "devices":self.mapping_key_capture.registrations.iter().map(|registration| registration.info.clone()).collect::<Vec<_>>(),
                "unresolved_devices":omitted}),
        )
    }
    pub(super) fn mapping_key_capture_events(&mut self) -> anyhow::Result<Value> {
        ensure!(
            self.mapping_key_capture.suspended,
            "Mapping key capture is inactive"
        );
        ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Mapping key capture buffer overflowed"
        );
        let mut events = Vec::new();
        for value in self.mapping_key_capture.events.try_iter() {
            if !self
                .mapping_key_capture
                .registrations
                .iter()
                .any(|r| r.info["guid"] == value["deviceInfo"]["guid"])
            {
                continue;
            }
            ensure!(
                value["input"]["type"] != "error",
                "Capture driver event: {}",
                value["input"]
            );
            if value["eventType"] == 3 {
                events.push(value);
            }
        }
        Ok(json!({"events":events}))
    }
    pub(super) fn stop_mapping_key_capture(&mut self) -> anyhow::Result<Value> {
        if !self.mapping_key_capture.active() {
            return Ok(json!({"capturing":false}));
        }
        ensure!(
            !self.poisoned,
            "Capture callback timed out; cleanup cannot be confirmed"
        );
        let mut errors = Vec::new();
        if self.mapping_key_capture.suspended {
            let result = (|| -> anyhow::Result<()> {
                let enable: Global = unsafe { self.mapping_symbol("enableMapping")? };
                self.call(|| unsafe { enable(callback2) })?;
                Ok(())
            })();
            match result {
                Ok(()) => self.mapping_key_capture.suspended = false,
                Err(e) => errors.push(format!("Restore mappings: {e:#}")),
            }
        }
        for index in (0..self.mapping_key_capture.registrations.len()).rev() {
            if self.poisoned {
                break;
            }
            let info = self.mapping_key_capture.registrations[index].info.clone();
            let text = CString::new(info.to_string())?;
            if self.mapping_key_capture.registrations[index].redirected {
                let result = (|| -> anyhow::Result<()> {
                    let disable: Operation = unsafe {
                        self.mapping_symbol(
                            if self.mapping_key_capture.registrations[index].keyboard_only {
                                "disableKeyboardInputRedirect"
                            } else {
                                "disableInputRedirect"
                            },
                        )?
                    };
                    self.call(|| unsafe { disable(text.as_ptr(), callback3) })?;
                    Ok(())
                })();
                match result {
                    Ok(()) => self.mapping_key_capture.registrations[index].redirected = false,
                    Err(e) => errors.push(format!("Disable input redirect: {e:#}")),
                }
            }
            if self.poisoned {
                std::mem::forget(text);
                break;
            }
            let result = (|| -> anyhow::Result<()> {
                let remove: Operation = unsafe { self.mapping_symbol("removeUsbDevice")? };
                self.call(|| unsafe { remove(text.as_ptr(), callback3) })?;
                Ok(())
            })();
            if self.poisoned {
                std::mem::forget(text);
            }
            match result {
                Ok(()) => {
                    self.mapping_key_capture.registrations.remove(index);
                }
                Err(e) => errors.push(format!("Remove capture device: {e:#}")),
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(json!({"capturing":false}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ble_endpoint_identity_requires_observed_bluetooth_parent() {
        assert_eq!(
            ble_identity("BTHLEDevice\\{service}_Dev_VID&021532_PID&0280_REV&0100_001122aabbcc"),
            Some((0x1532, 0x0280))
        );
        assert_eq!(
            ble_identity("BTHLEDevice\\{service}_Dev_VID&02068e_PID&0001_REV&0100_001122aabbcc"),
            Some((0x068e, 1))
        );
        assert_eq!(ble_identity("HID\\VID_1532&PID_0280"), None);
        assert_eq!(
            ble_identity("BTHLEDevice\\{service}_Dev_VID&021234_PID&0280_REV&0100_001122aabbcc"),
            None
        );
        assert_eq!(
            ble_identity("BTHLEDevice\\{service}_Dev_VID&021532_PID&0280_REV&0100_bad"),
            None
        );
    }
}
