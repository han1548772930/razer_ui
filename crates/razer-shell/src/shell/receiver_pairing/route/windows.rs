//! Windows ContainerId join for the source 164/241 command interface.
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::collections::BTreeMap;

pub(super) fn command_path(
    client: &mut ServiceClient,
    product_id: u32,
    container: &str,
) -> anyhow::Result<String> {
    let cap = u16::try_from(product_id)
        .ok()
        .and_then(razer_device::receiver_capabilities::capability)
        .context("接收器没有当前源核验的接口能力")?;
    let snapshot = client.request(ServiceRequest::HidDevices)?;
    ensure!(snapshot["complete"] == true, "接收器 HID 枚举未完成");
    let items = snapshot["interfaces"]
        .as_array()
        .context("缺少当前 HID 接口")?;
    let mut paths = BTreeMap::new();
    for item in items {
        if item["vendor_id"].as_u64() == Some(u64::from(cap.vendor_id))
            && item["product_id"].as_u64() == Some(u64::from(cap.product_id))
            && item["claim_interface"].as_u64() == Some(u64::from(cap.claim_interface))
            && item["feature_report_bytes"].as_u64() == Some(cap.report_bytes as u64)
            && item["device_container_id"]
                .as_str()
                .is_some_and(|id| id.eq_ignore_ascii_case(container))
        {
            if let Some(path) = item["path"].as_str().filter(|path| !path.is_empty()) {
                paths.insert(path.to_ascii_lowercase(), path.to_owned());
            }
        }
    }
    // Application write guard: never retry a mutation on an ambiguous collection.
    ensure!(paths.len() == 1, "接收器写入接口缺失或存在歧义");
    Ok(paths.into_values().next().unwrap())
}
