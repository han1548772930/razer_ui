//! Mixer collection selection from real observations. No transport writes here.
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer::{MixerTarget, accepts, validate_report_lengths},
    backend::{HidNode, ReportLengths},
};
use razer_discovery::discovery::{ObservedDevice, ObservedTransport};
use razer_ipc::{ServiceClient, ServiceRequest};

#[cfg(windows)]
#[path = "route/windows.rs"]
mod windows;

pub(super) fn resolve(
    client: &mut ServiceClient,
    observation: &ObservedDevice,
    target: &MixerTarget,
) -> anyhow::Result<HidNode> {
    resolve_collection(client, observation, Some(target))
}

/// Driver-only operations need an actual owner identity anchor, never DSP
/// report capabilities. The Windows service separately matches its driver
/// interface to this collection's observed ContainerId and instance.
pub(super) fn resolve_identity(
    client: &mut ServiceClient,
    observation: &ObservedDevice,
) -> anyhow::Result<HidNode> {
    resolve_collection(client, observation, None)
}

fn resolve_collection(
    client: &mut ServiceClient,
    observation: &ObservedDevice,
    target: Option<&MixerTarget>,
) -> anyhow::Result<HidNode> {
    ensure!(
        observation.product_id() == 1342
            && observation.physical_product_id() == 1342
            && observation.peer_product_id().is_none()
            && matches!(observation.transport(), Some(ObservedTransport::Wired)),
        "Mixer 操作需要当前源核实的直接有线设备观察"
    );
    if let Some(node) = observation.hid_node() {
        ensure!(
            accepts(observation.product_id(), node.vendor_id, node.product_id),
            "Mixer collection 与源产品身份不匹配"
        );
        let reply = client.request(ServiceRequest::HidNodes)?;
        let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Mixer collection 已变化或不唯一"
        );
        if let Some(target) = target {
            reports_match(client, node, target)?;
        }
        return Ok(node.clone());
    }
    #[cfg(windows)]
    {
        windows::resolve(client, observation, target)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("当前平台没有 Mixer 的实际 HID collection 观察")
    }
}

pub(super) fn revalidate(
    client: &mut ServiceClient,
    observation: &ObservedDevice,
    selected: &HidNode,
    target: &MixerTarget,
) -> anyhow::Result<()> {
    ensure!(
        resolve(client, observation, target)? == *selected,
        "Mixer 操作期间容器或 collection 身份变化，结果未确认"
    );
    Ok(())
}

fn reports_match(
    client: &mut ServiceClient,
    node: &HidNode,
    target: &MixerTarget,
) -> anyhow::Result<()> {
    let reply = client.request(ServiceRequest::HidNodeReports { node: node.clone() })?;
    ensure!(
        reply["node"] == serde_json::to_value(node)?,
        "Mixer descriptor 响应 collection 不匹配"
    );
    let lengths: ReportLengths = serde_json::from_value(
        reply
            .get("reports")
            .cloned()
            .context("Mixer descriptor 响应缺少 reports")?,
    )?;
    validate_report_lengths(target, &lengths)
}
