//! Source-selected direct device routes over IPC. No OS calls or UI state.
//! Collection identities are application observation policy, not firmware IDs.
use crate::discovery::ObservedDevice;
use anyhow::ensure;
use razer_device::{
    backend::HidNode,
    device_reads::{DeviceReadCapability, DeviceReadKind, DeviceReadValue, DeviceReadValues},
    device_writes::DeviceWriteSetting,
    keyboard_settings::KeyboardBrightness,
    mouse_dpi_stages::{self, DpiStagesDraft, DpiStagesReading, DpiStagesWriteResult},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use serde_json::Value;

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
    let reply = client.request(ServiceRequest::HidNodeReports { node: node.clone() })?;
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

pub fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
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
        let current = client.request(ServiceRequest::HidNodes)?;
        let nodes: Vec<HidNode> = serde_json::from_value(current["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "直接设备 collection 已变化或不唯一"
        );
        reports_match(client, node, cap.report_id, cap.report_bytes)?;
        return Ok(DirectRoute::Collection {
            node: node.clone(),
            product_id: cap.product_id,
        });
    }
    #[cfg(windows)]
    {
        windows::resolve(client, observed, cap).map(DirectRoute::Container)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("此设备尚无当前平台的真实 HID collection 观察")
    }
}

impl DirectRoute {
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
        let request = match self {
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
        let reply = client.request(request)?;
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

    pub fn read_keyboard(&self, client: &mut ServiceClient) -> anyhow::Result<KeyboardBrightness> {
        self.keyboard(client, None)
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
        let product_id = match self {
            Self::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            Self::Container(target) => target.product_id,
        };
        let cap = mouse_dpi_stages::capability(product_id).ok_or_else(|| {
            anyhow::anyhow!("Product has no current source-proven DPI stage capability")
        })?;
        let packed = draft
            .as_ref()
            .map(|draft| mouse_dpi_stages::pack(cap, draft))
            .transpose()?;
        let request = match (self, &draft) {
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
        let reply = client.request(request)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["source_class"] == cap.source_class
                && reply["profile_scope"] == "current_active_table"
                && reply["obm_profiles_written"] == false,
            "DPI stage reply source or profile scope does not match"
        );
        let reading = if let Some(draft) = draft {
            let result: DpiStagesWriteResult = serde_json::from_value(reply["result"].clone())?;
            ensure!(
                result.requested == draft
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
        let reading = if let Some(percent) = percent {
            ensure!(
                reply["result"]["requested_percent"] == percent
                    && reply["result"]["verified"] == true,
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
}

pub(crate) fn read_values(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
) -> DeviceReadValues {
    let mut values = DeviceReadValues::default();
    let route = resolve(client, observed, cap);
    for command in &cap.queries {
        let result = route
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error:#}"))
            .and_then(|route| route.read(client, command.name))
            .and_then(|reading| values.insert(command.name, reading));
        if let Err(error) = result {
            values
                .errors
                .insert(command.name.name().into(), format!("{error:#}"));
        }
    }
    values
}
