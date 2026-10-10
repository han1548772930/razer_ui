//! Windows ContainerId → actual hidapi collection bridge. Paths are never made up.
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;

fn belongs(interface: &Value, observation: &ObservedDevice) -> bool {
    interface["vendor_id"].as_u64() == Some(0x1532)
        && interface["product_id"].as_u64() == Some(u64::from(observation.physical_product_id()))
        && interface["device_container_id"]
            .as_str()
            .is_some_and(|id| id.eq_ignore_ascii_case(observation.container()))
}

fn matches(interface: &Value, node: &HidNode) -> anyhow::Result<bool> {
    // hidapi stores Windows device-interface paths as UTF-8 C-string bytes.
    // Only case folding is allowed for the Win32 path; no synthetic MI/usage.
    let path = std::str::from_utf8(&node.path).context("Windows HID 路径不是 UTF-8")?;
    Ok(interface["path"]
        .as_str()
        .is_some_and(|observed| observed.eq_ignore_ascii_case(path))
        && interface["vendor_id"].as_u64() == Some(u64::from(node.vendor_id))
        && interface["product_id"].as_u64() == Some(u64::from(node.product_id))
        && interface["usage_page"].as_u64() == Some(u64::from(node.usage_page))
        && interface["usage"].as_u64() == Some(u64::from(node.usage))
        && interface["version_number"].as_u64() == Some(u64::from(node.release_number))
        // MI metadata can be absent; that does not authorize guessing zero.
        && interface["claim_interface"]
            .as_i64()
            .is_none_or(|number| node.interface_number == -1 || number == i64::from(node.interface_number))
        && interface["device_instance_id"].as_str().is_some_and(|id| !id.is_empty()))
}

fn enumeration(client: &mut ServiceClient) -> anyhow::Result<(Value, Vec<HidNode>)> {
    let hid = client.request(ServiceRequest::HidDevices)?;
    ensure!(
        hid["complete"].as_bool() == Some(true),
        "Windows HID 枚举未完成"
    );
    let failures = hid["failures"]
        .as_array()
        .context("Windows HID 枚举缺少 failures")?;
    // Optional display strings cannot affect selection. Structural/capability
    // failures can conceal another eligible collection and must fail closed.
    ensure!(
        failures.iter().all(|failure| matches!(
            failure["operation"].as_str(),
            Some("read_manufacturer" | "read_product" | "read_serial_number")
        )),
        "Windows HID 身份或能力枚举不完整，不能证明唯一 Mixer collection"
    );
    let nodes = client.request(ServiceRequest::HidNodes)?;
    Ok((hid, serde_json::from_value(nodes["nodes"].clone())?))
}

pub(super) fn resolve(
    client: &mut ServiceClient,
    observation: &ObservedDevice,
    target: &MixerTarget,
) -> anyhow::Result<HidNode> {
    let (hid, nodes) = enumeration(client)?;
    let interfaces = hid["interfaces"]
        .as_array()
        .context("Windows HID 枚举缺少 interfaces")?;
    ensure!(
        interfaces
            .iter()
            .filter(|interface| {
                interface["vendor_id"].as_u64() == Some(0x1532)
                    && interface["product_id"].as_u64()
                        == Some(u64::from(observation.physical_product_id()))
            })
            .all(|interface| interface["device_container_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty())),
        "Mixer 接口容器观察不完整，不能证明唯一设备归属"
    );
    let mut candidates = BTreeMap::new();
    for interface in interfaces
        .iter()
        .filter(|interface| belongs(interface, observation))
    {
        let path = interface["path"]
            .as_str()
            .filter(|path| !path.is_empty())
            .context("Mixer 同容器接口缺少实际路径")?;
        let joined = nodes
            .iter()
            .filter(|node| matches(interface, node).unwrap_or(false))
            .collect::<Vec<_>>();
        ensure!(
            joined.len() == 1,
            "Mixer Windows/hidapi collection 身份缺失或歧义"
        );
        let node = joined[0];
        ensure!(
            accepts(observation.product_id(), node.vendor_id, node.product_id),
            "Mixer collection 产品身份不匹配"
        );
        // A descriptor mismatch excludes that collection; an IPC/open failure
        // is unknown, so it must not be mistaken for an incompatible report.
        let reply = client.request(ServiceRequest::HidNodeReports { node: node.clone() })?;
        ensure!(
            reply["node"] == serde_json::to_value(node)?,
            "Mixer descriptor collection 不匹配"
        );
        let lengths: ReportLengths = serde_json::from_value(reply["reports"].clone())?;
        if validate_report_lengths(target, &lengths).is_ok() {
            ensure!(
                candidates
                    .insert(path.to_ascii_lowercase(), node.clone())
                    .is_none(),
                "Mixer 接口路径重复，身份观察不唯一"
            );
        }
    }
    ensure!(
        candidates.len() == 1,
        "同容器没有唯一符合源报文的 Mixer collection"
    );
    let selected = candidates.into_values().next().expect("unique collection");
    let previous_instance = interfaces
        .iter()
        .find(|interface| {
            belongs(interface, observation) && matches(interface, &selected).unwrap_or(false)
        })
        .and_then(|interface| interface["device_instance_id"].as_str())
        .context("Mixer collection 缺少实际实例 ID")?;
    // Descriptor access opens a real handle. Retain a second metadata receipt
    // afterward so a hot-unplug/replacement cannot silently change its owner.
    let (current, nodes) = enumeration(client)?;
    let interfaces = current["interfaces"]
        .as_array()
        .context("Windows HID 枚举缺少 interfaces")?;
    ensure!(
        nodes.iter().filter(|node| **node == selected).count() == 1
            && interfaces
                .iter()
                .filter(|interface| belongs(interface, observation)
                    && matches(interface, &selected).unwrap_or(false)
                    && interface["device_instance_id"]
                        .as_str()
                        .is_some_and(|id| id.eq_ignore_ascii_case(previous_instance)))
                .count()
                == 1,
        "Mixer descriptor 观察期间容器或 collection 身份变化"
    );
    Ok(selected)
}
