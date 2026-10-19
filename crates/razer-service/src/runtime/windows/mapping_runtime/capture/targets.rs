//! Current host addUsbDevice/noble identity and focused-recorder selection.
use super::super::super::{hid, receiver, usb};
use anyhow::{Context, Result, ensure};
use razer_device::mapping_native_session::hooks::RedirectKind;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub vendor_id: u32,
    pub product_id: u32,
    pub container_id: String,
    pub usb_instance_id: Option<String>,
    pub guid: Uuid,
    pub keyboard_only: bool,
}
impl Target {
    pub fn info(&self) -> Value {
        let mut value = json!({"vendorId":self.vendor_id,"productId":self.product_id,
            "containerId":self.container_id,"guid":self.guid.to_string()});
        if let Some(instance) = &self.usb_instance_id {
            value["usbInstanceId"] = instance.clone().into();
        }
        value
    }
    pub(super) fn redirects(&self) -> &'static [RedirectKind] {
        // mapping_engine 6cfd0 invokes 6f240 (mouse) before 6e390 (keyboard).
        // Mouse movement has its own registration and is not included here.
        if self.keyboard_only {
            &[RedirectKind::Keyboard]
        } else {
            &[RedirectKind::Mouse, RedirectKind::Keyboard]
        }
    }
}

#[derive(Default)]
pub(super) struct Selection {
    pub targets: Vec<Target>,
    pub omitted: Vec<Omitted>,
}
#[derive(Serialize)]
pub(super) struct Omitted {
    product_id: u32,
    reason: &'static str,
}

#[derive(Deserialize)]
struct UsbObservation {
    complete: bool,
    failures: Vec<Value>,
    devices: Vec<UsbDevice>,
}
#[derive(Deserialize)]
struct UsbDevice {
    vendor_id: u32,
    product_id: u32,
    device_container_id: Option<String>,
    device_instance_id: Option<String>,
}
#[derive(Deserialize)]
struct HidObservation {
    complete: bool,
    interfaces: Vec<HidEndpoint>,
}
#[derive(Deserialize)]
struct HidEndpoint {
    parent_instance_id: Option<String>,
    device_container_id: Option<String>,
}
#[derive(Deserialize)]
struct Pairing {
    master: Option<Master>,
}
#[derive(Deserialize)]
struct Master {
    #[serde(rename = "dongleId")]
    dongle_id: Option<u32>,
}

fn category(pid: u32) -> Option<&'static str> {
    let razer_device::device_identity::IdentityLookup::Unique(identity) =
        razer_device::device_identity::lookup(pid)
    else {
        return None;
    };
    razer_device::device_identity::middleware_category(identity.product_id)
}
fn mouse_product(pid: u32) -> bool {
    matches!(category(pid), Some("MOUSE" | "MOUSEPLUSMAT"))
}
fn global_input_product(pid: u32) -> bool {
    matches!(category(pid), Some("KEYBOARD" | "KEYPAD" | "SYSTEM"))
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
    mac.bytes()
        .all(|byte| byte.is_ascii_hexdigit())
        .then_some((vendor, product))
}

pub(super) fn observe(calibration: Option<(u32, &str)>, global: bool) -> Result<Selection> {
    if let Some((pid, container)) = calibration {
        ensure!(
            [740, 746].contains(&pid) && receiver::valid_container(container),
            "Calibration capture has no current keyboard/container identity"
        );
    }
    let usb: UsbObservation = serde_json::from_value(usb::enumerate()?)?;
    ensure!(
        usb.complete && usb.failures.is_empty(),
        "USB capture enumeration is incomplete"
    );
    let paired = if global {
        Vec::new()
    } else {
        let cache = razer_storage::receiver_pairing::PairingCache::load(
            &razer_storage::receiver_pairing::local_path()?,
        )?;
        let records: BTreeMap<String, Pairing> = serde_json::from_value(cache.records())?;
        records
            .values()
            .filter_map(|row| row.master.as_ref()?.dongle_id)
            .collect()
    };
    let hid = if calibration.is_none() {
        Some(serde_json::from_value::<HidObservation>(
            hid::enumerate_metadata()?,
        )?)
    } else {
        None
    };
    select(usb, hid, &paired, calibration, global)
}

fn select(
    usb: UsbObservation,
    hid: Option<HidObservation>,
    paired: &[u32],
    calibration: Option<(u32, &str)>,
    global: bool,
) -> Result<Selection> {
    ensure!(
        usb.complete && usb.failures.is_empty(),
        "USB capture enumeration is incomplete"
    );
    let mut selection = Selection::default();
    for device in usb.devices {
        if !matches!(device.vendor_id, 0x1532 | 0x068e) {
            continue;
        }
        let pid = device.product_id;
        if global && !global_input_product(pid) {
            continue;
        }
        let keyboard_only = !global && (calibration.is_some() || paired.contains(&pid));
        if !keyboard_only && mouse_product(pid) {
            continue;
        }
        let container = device
            .device_container_id
            .context("USB capture container missing")?;
        if calibration.is_some_and(|(target_pid, target_container)| {
            target_pid != pid || !target_container.eq_ignore_ascii_case(&container)
        }) {
            continue;
        }
        let instance = device
            .device_instance_id
            .context("USB capture instance missing")?;
        ensure!(
            receiver::valid_container(&container) && !instance.is_empty(),
            "Invalid USB capture identity"
        );
        selection.targets.push(Target {
            vendor_id: device.vendor_id,
            product_id: pid,
            container_id: container,
            usb_instance_id: Some(instance),
            guid: Uuid::new_v4(),
            keyboard_only,
        });
    }
    if let Some(hid) = hid {
        ensure!(hid.complete, "BLE HID endpoint enumeration is incomplete");
        for endpoint in hid.interfaces {
            let Some((vendor, pid)) = endpoint
                .parent_instance_id
                .as_deref()
                .and_then(ble_identity)
            else {
                continue;
            };
            if (global && !global_input_product(pid)) || mouse_product(pid) {
                continue;
            }
            let Some(container) = endpoint
                .device_container_id
                .filter(|value| receiver::valid_container(value))
            else {
                selection.omitted.push(Omitted {
                    product_id: pid,
                    reason: "Observed BLE HID endpoint has no valid container",
                });
                continue;
            };
            if selection.targets.iter().any(|row| {
                row.product_id == pid && row.container_id.eq_ignore_ascii_case(&container)
            }) {
                continue;
            }
            selection.targets.push(Target {
                vendor_id: vendor,
                product_id: pid,
                container_id: container.to_uppercase(),
                usb_instance_id: None,
                guid: Uuid::new_v4(),
                keyboard_only: false,
            });
        }
    }
    if calibration.is_some() {
        ensure!(
            selection.targets.len() == 1,
            "Calibration target disappeared or has ambiguous physical observations"
        );
    }
    Ok(selection)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONTAINER: &str = "{11111111-2222-3333-4444-555555555555}";
    fn usb(devices: Value) -> UsbObservation {
        serde_json::from_value(json!({"complete":true,"failures":[],"devices":devices})).unwrap()
    }
    fn device(pid: u32) -> Value {
        json!({"vendor_id":5426,"product_id":pid,"device_container_id":CONTAINER,"device_instance_id":"USB\\observed"})
    }
    #[test]
    fn global_categories_exclude_mouse_and_calibration_requires_unique_observed_target() {
        let global = select(usb(json!([device(740), device(70)])), None, &[], None, true).unwrap();
        assert_eq!(global.targets.len(), 1);
        assert_eq!(global.targets[0].product_id, 740);
        assert_eq!(
            global.targets[0].redirects(),
            [RedirectKind::Mouse, RedirectKind::Keyboard]
        );
        let selected = select(
            usb(json!([device(740), device(70)])),
            None,
            &[],
            Some((740, CONTAINER)),
            false,
        )
        .unwrap();
        assert!(selected.targets[0].keyboard_only);
        assert!(
            select(
                usb(json!([device(740), device(740)])),
                None,
                &[],
                Some((740, CONTAINER)),
                false
            )
            .is_err()
        );
        assert!(select(usb(json!([])), None, &[], Some((740, CONTAINER)), false).is_err());
    }
    #[test]
    fn ble_identity_uses_observed_parent_and_never_synthesizes_usb_instance() {
        assert_eq!(ble_identity("HID\\VID_1532&PID_0280"), None);
        let hid = serde_json::from_value(json!({"complete":true,"interfaces":[{"parent_instance_id":"BTHLEDevice\\{service}_Dev_VID&021532_PID&0280_REV&0100_001122aabbcc","device_container_id":CONTAINER}]})).unwrap();
        let selection = select(usb(json!([])), Some(hid), &[], None, false).unwrap();
        assert_eq!(selection.targets.len(), 1);
        assert!(selection.targets[0].usb_instance_id.is_none());
        assert!(selection.targets[0].info().get("usbInstanceId").is_none());
    }
}
