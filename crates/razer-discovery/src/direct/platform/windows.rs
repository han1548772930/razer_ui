//! Windows ContainerId-to-interface adapter. Shared callers never parse Win32 paths.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::device_reads::{DeviceReadCapability, DeviceReadTarget};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::collections::BTreeMap;

pub(super) fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    cap: &DeviceReadCapability,
) -> anyhow::Result<DeviceReadTarget> {
    let hid = client.request(ServiceRequest::HidDevices)?;
    let claim_interface = cap.claim_interface_for(observed.physical_product_id())?;
    let interfaces = hid["interfaces"]
        .as_array()
        .context("缺少实际 Windows HID 接口")?;
    let mut paths = BTreeMap::new();
    for interface in interfaces {
        if interface["vendor_id"].as_u64() != Some(u64::from(cap.vendor_id))
            || interface["product_id"].as_u64() != Some(u64::from(observed.physical_product_id()))
            || interface["claim_interface"].as_i64() != Some(i64::from(claim_interface))
            || interface["feature_report_bytes"].as_u64() != Some(cap.report_bytes as u64)
            || !interface["device_container_id"]
                .as_str()
                .is_some_and(|id| id.eq_ignore_ascii_case(observed.container()))
        {
            continue;
        }
        if let Some(path) = interface["path"].as_str().filter(|path| !path.is_empty()) {
            paths.insert(path.to_ascii_lowercase(), path.to_owned());
        }
    }
    ensure!(paths.len() == 1, "直接设备接口缺失或存在歧义，未发送设置");
    Ok(DeviceReadTarget {
        product_id: observed.product_id(),
        physical_product_id: observed.physical_product_id(),
        peer_product_id: None,
        device_container_id: observed.container().to_owned(),
        path: paths.into_values().next().unwrap(),
    })
}
