//! Source-selected direct device routes over IPC. No OS calls or UI state.
//! Collection identities are application observation policy, not firmware IDs.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::HidNode,
    device_reads::{DeviceReadCapability, DeviceReadKind, DeviceReadValue, DeviceReadValues},
    device_writes::DeviceWriteSetting,
    keyboard_settings::KeyboardBrightness,
    mouse_dpi_stages::{self, DpiStagesDraft, DpiStagesReading, DpiStagesWriteResult},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde_json::Value;
mod feature_submission;
pub mod keyboard_indicator;
pub mod keyboard_latest_restore;
pub mod keyboard_legacy_calibration;
pub mod keyboard_metadata;
pub mod keyboard_onboard_profiles;
pub mod keyboard_profile_inventory;
pub mod leviathan;
pub mod mouse_brightness;
pub mod mouse_current_dpi;
pub mod mouse_current_polling;
pub mod mouse_current_power;
pub mod mouse_dynamic;
pub mod mouse_indicator_led;
pub mod mouse_philips;
pub mod mouse_receiver_legacy;
pub mod mouse_receiver_polling;
pub mod mouse_rotation;
pub mod mouse_runtime_artwork;
pub mod mouse_scroll;
pub mod mouse_tracking;
pub mod philips_measurement;
mod receiver_settings;
pub mod seiren_eq;

#[cfg(windows)]
#[path = "platform/windows.rs"]
mod windows;

#[derive(Clone)]
pub enum DirectRoute {
    Collection {
        node: HidNode,
        product_id: u32,
    },
    #[cfg(windows)]
    Container(razer_device::device_reads::DeviceReadTarget),
}

/// An opaque collection key preserves exact path bytes and usage identity.
/// Never substitute it into a Windows API which expects a ContainerId.
pub(crate) fn collection_scope(node: &HidNode) -> String {
    use std::fmt::Write as _;
    let mut scope = format!(
        "hid-collection:{:04x}:{:04x}:{}:{}:{}:",
        node.vendor_id, node.product_id, node.interface_number, node.usage_page, node.usage
    );
    for byte in &node.path {
        let _ = write!(scope, "{byte:02x}");
    }
    scope
}

pub(crate) fn reports_match(
    client: &mut ServiceClient,
    node: &HidNode,
    report_id: u8,
    report_bytes: usize,
) -> anyhow::Result<()> {
    reports_match_with(node, report_id, report_bytes, &mut |request| {
        client.request(request)
    })
}

fn reports_match_with(
    node: &HidNode,
    report_id: u8,
    report_bytes: usize,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<()> {
    let reply = request(ServiceRequest::HidNodeReports { node: node.clone() })?;
    ensure!(
        reply["node"] == serde_json::to_value(node)?,
        "HID 描述符响应 collection 不匹配"
    );
    ensure!(
        reply["reports"]["feature"][report_id.to_string()].as_u64() == Some(report_bytes as u64),
        "实际 HID 描述符与当前源 Feature 长度不匹配"
    );
    Ok(())
}

/// The current UsbRzDeviceAction/HID.node path sizes Feature transfers from
/// the collection capabilities. Its selector need not be an advertised ID.
pub(crate) fn source_reports_match(
    client: &mut ServiceClient,
    node: &HidNode,
    report_bytes: usize,
) -> anyhow::Result<()> {
    source_reports_match_with(node, report_bytes, &mut |request| client.request(request))
}

pub(crate) fn source_reports_match_with(
    node: &HidNode,
    report_bytes: usize,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<()> {
    let reply = request(ServiceRequest::HidNodeReports { node: node.clone() })?;
    ensure!(
        reply["node"] == serde_json::to_value(node)?,
        "HID descriptor response belongs to another collection"
    );
    let feature: std::collections::BTreeMap<u8, usize> =
        serde_json::from_value(reply["reports"]["feature"].clone())?;
    ensure!(
        feature.values().max().copied() == Some(report_bytes),
        "HID collection maximum differs from the current source Feature length"
    );
    Ok(())
}

#[cfg(test)]
mod source_collection_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_feature_size_uses_the_exact_collection_maximum_not_the_selector_key() {
        let node = HidNode {
            path: b"simulated-interface-3".to_vec(),
            vendor_id: 0x1532,
            product_id: 226,
            usage_page: 1,
            usage: 2,
            interface_number: 3,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        };
        for feature in [json!({"0":91}), json!({"3":91}), json!({"0":4,"3":91})] {
            let mut request = |request| {
                assert!(
                    matches!(request, ServiceRequest::HidNodeReports { node: requested } if requested == node)
                );
                Ok(json!({"node":node,"reports":{"feature":feature}}))
            };
            source_reports_match_with(&node, 91, &mut request).unwrap();
        }
        for feature in [
            json!({}),
            json!({"0":90}),
            json!({"0":92,"3":91}),
            json!({"0":"91"}),
        ] {
            assert!(
                source_reports_match_with(&node, 91, &mut |_| {
                    Ok(json!({"node":node,"reports":{"feature":feature}}))
                })
                .is_err()
            );
        }
        assert!(
            reports_match_with(&node, 3, 91, &mut |_| {
                Ok(json!({"node":node,"reports":{"feature":{"0":91}}}))
            })
            .is_err()
        );
        let mut other = node.clone();
        other.path.push(1);
        assert!(
            source_reports_match_with(&node, 91, &mut |_| {
                Ok(json!({"node":other,"reports":{"feature":{"0":91}}}))
            })
            .is_err()
        );
    }
}

pub fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
) -> anyhow::Result<DirectRoute> {
    resolve_with(observed, cap, &mut |request| client.request(request))
}

fn resolve_with(
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DirectRoute> {
    ensure!(
        observed.product_id() == cap.product_id
            && cap.direct_pids.contains(&observed.physical_product_id())
            && observed.peer_product_id().is_none()
            && matches!(
                observed.transport(),
                Some(crate::discovery::ObservedTransport::Wired)
            ),
        "此操作只接受源核实的直接有线设备；接收器和 BLE 写入链尚未闭合"
    );
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && cap.direct_pids.contains(&u32::from(node.product_id))
                && node.interface_number
                    == i32::from(cap.claim_interface_for(u32::from(node.product_id))?),
            "HID collection 与产品源接口不匹配；未知接口号不能猜测"
        );
        let current = request(ServiceRequest::HidNodes)?;
        let nodes: Vec<HidNode> = serde_json::from_value(current["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "直接设备 collection 已变化或不唯一"
        );
        source_reports_match_with(node, cap.report_bytes, request)?;
        return Ok(DirectRoute::Collection {
            node: node.clone(),
            product_id: cap.product_id,
        });
    }
    #[cfg(windows)]
    {
        windows::resolve_with(observed, cap, request).map(DirectRoute::Container)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("此设备尚无当前平台的真实 HID collection 观察")
    }
}

impl DirectRoute {
    pub fn keyboard_wristrest_target(
        &self,
    ) -> anyhow::Result<razer_device::keyboard_wristrest::Target> {
        use razer_device::keyboard_wristrest::{self, Target};
        let target = match self {
            Self::Collection { node, product_id } => Target::Collection {
                node: node.clone(),
                product_id: *product_id,
            },
            #[cfg(windows)]
            Self::Container(target) => Target::WindowsContainer(target.clone()),
        };
        ensure!(
            keyboard_wristrest::capability(target.product_id()).is_some(),
            "No source wristrest reader for this product"
        );
        Ok(target)
    }

    fn check_reply(&self, reply: &Value) -> anyhow::Result<()> {
        match self {
            Self::Collection { node, product_id } => {
                ensure!(
                    reply["node"] == serde_json::to_value(node)?
                        && reply["product_id"] == *product_id,
                    "设备响应 collection 或产品不匹配"
                );
            }
            #[cfg(windows)]
            Self::Container(target) => {
                ensure!(
                    reply["target"] == serde_json::to_value(target)?,
                    "设备响应容器或接口不匹配"
                );
            }
        }
        Ok(())
    }

    pub fn read(
        &self,
        client: &mut ServiceClient,
        kind: DeviceReadKind,
    ) -> anyhow::Result<DeviceReadValue> {
        self.read_with(kind, &mut |request| client.request(request))
    }

    fn read_with(
        &self,
        kind: DeviceReadKind,
        request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<DeviceReadValue> {
        let query = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeRead {
                node: node.clone(),
                product_id: *product_id,
                kind,
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceRead {
                target: target.clone(),
                kind,
            },
        };
        let reply = request(query)?;
        self.check_reply(&reply)?;
        Ok(serde_json::from_value(reply["reading"].clone())?)
    }

    pub fn write(
        &self,
        client: &mut ServiceClient,
        setting: DeviceWriteSetting,
    ) -> anyhow::Result<DeviceReadValue> {
        let request = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeWrite {
                node: node.clone(),
                product_id: *product_id,
                setting: setting.clone(),
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceWrite {
                target: target.clone(),
                setting: setting.clone(),
            },
        };
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["result"]["requested"] == serde_json::to_value(&setting)?
                && reply["result"]["verified"] == true,
            "设备写入请求不匹配或未取得真实回读确认"
        );
        let observed: DeviceReadValue =
            serde_json::from_value(reply["result"]["observed"].clone())?;
        ensure!(setting.matches_value(&observed), "设备写入回读与请求不一致");
        Ok(observed)
    }

    /// PID70's original task completes on a matching successful setter ACK.
    /// This is intentionally distinct from `write`, which requires readback.
    pub fn submit_legacy_dpi(
        &self,
        client: &mut ServiceClient,
        x: u16,
        y: u16,
    ) -> anyhow::Result<razer_device::device_writes::DeviceWriteAck> {
        let product_id = match self {
            Self::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            Self::Container(target) => target.product_id,
        };
        ensure!(product_id == 70, "Legacy DPI submission requires PID70");
        let setting = DeviceWriteSetting::Dpi { x, y };
        razer_device::device_writes::prepare(
            razer_device::mouse_legacy_polling::write_capability(),
            &setting,
        )?;
        let request = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeWrite {
                node: node.clone(),
                product_id: *product_id,
                setting: setting.clone(),
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceWrite {
                target: target.clone(),
                setting: setting.clone(),
            },
        };
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["source_class"] == "rzDevice25Legacy",
            "Unexpected DPI source class"
        );
        let result: razer_device::device_writes::DeviceWriteResult =
            serde_json::from_value(reply["result"].clone())?;
        ensure!(
            result.requested == setting
                && result.changed
                && !result.verified
                && result.previous.is_none()
                && result.observed.is_none(),
            "Legacy DPI reply does not match the source ACK contract"
        );
        let ack = result
            .acknowledged
            .context("Legacy DPI reply has no setter acknowledgment")?;
        ensure!(
            ack.command == [7, 4, 5] && ack.data.len() <= 80,
            "Legacy DPI acknowledgment has an unexpected command or payload size"
        );
        Ok(ack)
    }

    pub fn read_keyboard(&self, client: &mut ServiceClient) -> anyhow::Result<KeyboardBrightness> {
        self.keyboard(client, None)
    }

    pub fn submit_keyboard_control(
        &self,
        client: &mut ServiceClient,
        profile_guid: &str,
        intent: razer_device::keyboard_submission::Intent,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Value> {
        use std::{
            sync::atomic::{AtomicU64, Ordering},
            time::{Duration, Instant},
        };
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let operation_id = format!(
            "keyboard-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let target = match self {
            Self::Collection { node, product_id } => {
                razer_device::keyboard_submission::Target::Collection {
                    node: node.clone(),
                    product_id: *product_id,
                }
            }
            #[cfg(windows)]
            Self::Container(target) => {
                razer_device::keyboard_submission::Target::WindowsContainer(target.clone())
            }
        };
        validate()?;
        let accepted = client.request(ServiceRequest::KeyboardAnalogStart {
            operation_id: operation_id.clone(),
            target,
            profile_guid: profile_guid.into(),
            intent: intent.clone(),
        })?;
        ensure!(
            accepted["operation_id"] == operation_id && accepted["started"] == true,
            "Keyboard task acceptance identity differs"
        );
        let started = Instant::now();
        let result = (|| -> anyhow::Result<Value> {
            loop {
                validate()?;
                ensure!(
                    started.elapsed() < Duration::from_secs(60),
                    "Keyboard task polling deadline expired"
                );
                let reply = client.request(ServiceRequest::KeyboardAnalogPoll {
                    operation_id: operation_id.clone(),
                })?;
                ensure!(
                    reply["operation_id"] == operation_id,
                    "Keyboard task reply identity differs"
                );
                if reply["terminal"] == true {
                    ensure!(
                        reply["state"] == "completed",
                        "{}",
                        reply["error"]
                            .as_str()
                            .unwrap_or("Keyboard task did not complete")
                    );
                    let result = &reply["result"];
                    self.check_reply(result)?;
                    ensure!(
                        result["receipt"]["intent"] == serde_json::to_value(&intent)?
                            && result["receipt"]["profile_guid"] == profile_guid
                            && result["receipt"]["completed"] == true,
                        "Keyboard completion differs from the submitted profile/intent"
                    );
                    validate()?;
                    return Ok(result["receipt"].clone());
                }
                std::thread::sleep(Duration::from_millis(16));
            }
        })();
        if result.is_err() {
            let _ = client.request(ServiceRequest::KeyboardAnalogCancel { operation_id });
        }
        result
    }

    pub fn read_dpi_stages(&self, client: &mut ServiceClient) -> anyhow::Result<DpiStagesReading> {
        self.dpi_stages(client, None)
    }

    pub fn write_dpi_stages(
        &self,
        client: &mut ServiceClient,
        draft: DpiStagesDraft,
    ) -> anyhow::Result<DpiStagesReading> {
        self.dpi_stages(client, Some(draft))
    }

    fn dpi_stages(
        &self,
        client: &mut ServiceClient,
        draft: Option<DpiStagesDraft>,
    ) -> anyhow::Result<DpiStagesReading> {
        self.dpi_stages_with(draft, &mut |request| client.request(request))
    }

    fn dpi_stages_with(
        &self,
        draft: Option<DpiStagesDraft>,
        request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<DpiStagesReading> {
        let product_id = match self {
            Self::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            Self::Container(target) => target.product_id,
        };
        let cap = mouse_dpi_stages::capability(product_id).ok_or_else(|| {
            anyhow::anyhow!("Product has no current source-proven DPI stage capability")
        })?;
        if let Some(draft) = &draft {
            mouse_dpi_stages::pack(cap, draft)?;
        }
        let query = match (self, &draft) {
            (Self::Collection { node, product_id }, None) => ServiceRequest::HidNodeDpiStagesRead {
                node: node.clone(),
                product_id: *product_id,
            },
            (Self::Collection { node, product_id }, Some(draft)) => {
                ServiceRequest::HidNodeDpiStagesWrite {
                    node: node.clone(),
                    product_id: *product_id,
                    draft: draft.clone(),
                }
            }
            #[cfg(windows)]
            (Self::Container(target), None) => ServiceRequest::DeviceDpiStagesRead {
                target: target.clone(),
            },
            #[cfg(windows)]
            (Self::Container(target), Some(draft)) => ServiceRequest::DeviceDpiStagesWrite {
                target: target.clone(),
                draft: draft.clone(),
            },
        };
        let reply = request(query)?;
        self.check_reply(&reply)?;
        decode_dpi_stages_reply(&reply, cap, draft.as_ref())
    }

    pub fn write_keyboard(
        &self,
        client: &mut ServiceClient,
        percent: u8,
    ) -> anyhow::Result<KeyboardBrightness> {
        self.keyboard(client, Some(percent))
    }

    fn keyboard(
        &self,
        client: &mut ServiceClient,
        percent: Option<u8>,
    ) -> anyhow::Result<KeyboardBrightness> {
        let request = match (self, percent) {
            (Self::Collection { node, product_id }, None) => {
                ServiceRequest::HidNodeKeyboardBrightnessRead {
                    node: node.clone(),
                    product_id: *product_id,
                }
            }
            (Self::Collection { node, product_id }, Some(percent)) => {
                ServiceRequest::HidNodeKeyboardBrightnessWrite {
                    node: node.clone(),
                    product_id: *product_id,
                    percent,
                }
            }
            #[cfg(windows)]
            (Self::Container(target), None) => ServiceRequest::DeviceKeyboardBrightnessRead {
                target: target.clone(),
            },
            #[cfg(windows)]
            (Self::Container(target), Some(percent)) => {
                ServiceRequest::DeviceKeyboardBrightnessWrite {
                    target: target.clone(),
                    percent,
                }
            }
        };
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        decode_keyboard_brightness_reply(&reply, percent)
    }
}

fn decode_keyboard_brightness_reply(
    reply: &Value,
    percent: Option<u8>,
) -> anyhow::Result<KeyboardBrightness> {
    let reading = if let Some(percent) = percent {
        ensure!(
            reply["result"]["requested_percent"] == percent && reply["result"]["verified"] == true,
            "键盘亮度写入请求不匹配或未取得回读确认"
        );
        &reply["result"]["observed"]
    } else {
        &reply["reading"]
    };
    let observed: KeyboardBrightness = serde_json::from_value(reading.clone())?;
    ensure!(
        observed.percent <= 100 && percent.is_none_or(|percent| observed.percent == percent),
        "键盘亮度响应范围或请求不匹配"
    );
    Ok(observed)
}

pub fn keyboard_brightness_available(observed: &ObservedDevice) -> bool {
    let Some(cap) = razer_device::keyboard_settings::capability(observed.product_id()) else {
        return false;
    };
    if observed.peer_product_id().is_some() {
        return receiver_settings::session(observed, true).is_ok();
    }
    matches!(
        observed.transport(),
        Some(crate::discovery::ObservedTransport::Wired)
    ) && cap
        .transport
        .direct_pids
        .contains(&observed.physical_product_id())
}

pub fn request_keyboard_brightness(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    percent: Option<u8>,
) -> anyhow::Result<KeyboardBrightness> {
    ensure!(
        keyboard_brightness_available(observed),
        "No current source brightness route for this observation"
    );
    if observed.peer_product_id().is_some() {
        receiver_settings::keyboard(observed, percent, &mut |request| client.request(request))
    } else {
        let cap = razer_device::keyboard_settings::capability(observed.product_id())
            .context("Keyboard brightness source is absent")?;
        resolve(client, observed, &cap.transport)?.keyboard(client, percent)
    }
}

fn decode_dpi_stages_reply(
    reply: &Value,
    cap: &mouse_dpi_stages::MouseDpiStagesCapability,
    draft: Option<&DpiStagesDraft>,
) -> anyhow::Result<DpiStagesReading> {
    let packed = draft
        .map(|draft| mouse_dpi_stages::pack(cap, draft))
        .transpose()?;
    ensure!(
        reply["source_class"] == cap.source_class
            && reply["profile_scope"] == "current_active_table"
            && reply["obm_profiles_written"] == false,
        "DPI stage reply source or profile scope does not match"
    );
    let reading = if let Some(draft) = draft {
        let result: DpiStagesWriteResult = serde_json::from_value(reply["result"].clone())?;
        ensure!(
            &result.requested == draft
                && result.verified
                && Some(&result.packed) == packed.as_ref()
                && mouse_dpi_stages::same_table(&result.packed, &result.observed)
                && result.acknowledged.response_bytes <= 80
                && result.acknowledged.raw_data.len() == 80,
            "DPI stage write request or confirmed readback does not match"
        );
        result.observed
    } else {
        serde_json::from_value(reply["reading"].clone())?
    };
    mouse_dpi_stages::validate_current_reading(cap, &reading)?;
    Ok(reading)
}

pub fn mouse_dpi_stages_available(observed: &ObservedDevice) -> bool {
    let Some(settings) = mouse_dpi_stages::capability(observed.product_id()) else {
        return false;
    };
    if observed.product_id() == razer_device::mouse_receiver_polling::PRODUCT_ID
        && observed.peer_product_id().is_some()
    {
        return matches!(
            observed.transport(),
            Some(crate::discovery::ObservedTransport::Dongle)
        ) && observed.connection()
            == razer_model::model::DeviceConnectionObservation::ReceiverPeer(1)
            && razer_device::mouse_receiver_polling::ReceiverRead::new(
                observed.product_id(),
                observed.physical_product_id(),
                observed.peer_product_id(),
                DeviceReadKind::Dpi,
            )
            .is_ok();
    }
    if observed.peer_product_id().is_some() {
        return receiver_settings::session(observed, false).is_ok();
    }
    observed.peer_product_id().is_none()
        && matches!(
            observed.transport(),
            Some(crate::discovery::ObservedTransport::Wired)
        )
        && settings
            .transport
            .direct_pids
            .contains(&observed.physical_product_id())
}

pub fn request_mouse_dpi_stages(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    draft: Option<DpiStagesDraft>,
) -> anyhow::Result<DpiStagesReading> {
    mouse_dpi_stages_with(observed, draft, |request| client.request(request))
}

fn mouse_dpi_stages_with(
    observed: &ObservedDevice,
    draft: Option<DpiStagesDraft>,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DpiStagesReading> {
    ensure!(
        mouse_dpi_stages_available(observed),
        "No current source DPI stage route for this observation"
    );
    let settings = mouse_dpi_stages::capability(observed.product_id())
        .context("DPI stage capability is absent")?;
    if let Some(draft) = &draft {
        mouse_dpi_stages::pack(settings, draft)?;
    }
    if observed.peer_product_id().is_some() {
        if observed.product_id() == razer_device::mouse_receiver_polling::PRODUCT_ID {
            mouse_receiver_polling::dpi_stages(observed, draft, &mut request)
        } else {
            receiver_settings::dpi_stages(observed, draft, &mut request)
        }
    } else {
        resolve_with(observed, &settings.transport, &mut request)?
            .dpi_stages_with(draft, &mut request)
    }
}

pub(crate) fn read_values(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
) -> DeviceReadValues {
    read_values_with(observed, cap, |request| client.request(request))
}

// Production dispatcher with an injectable IPC boundary. Tests exercise the
// same receiver selection and exact collection checks as the service caller.
fn read_values_with(
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> DeviceReadValues {
    let mut values = DeviceReadValues::default();
    let receiver = observed.product_id() == razer_device::mouse_receiver_polling::PRODUCT_ID
        && observed.peer_product_id().is_some();
    let route = if receiver {
        None
    } else {
        Some(resolve_with(observed, cap, &mut request))
    };
    for command in &cap.queries {
        let reading = if receiver {
            mouse_receiver_polling::read(observed, command.name, &mut request)
        } else {
            route
                .as_ref()
                .expect("direct route selected")
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))
                .and_then(|route| route.read_with(command.name, &mut request))
        };
        let result = reading.and_then(|reading| values.insert(command.name, reading));
        if let Err(error) = result {
            values
                .errors
                .insert(command.name.name().into(), format!("{error:#}"));
        }
    }
    values
}
