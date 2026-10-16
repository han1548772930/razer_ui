//! Portable collection observations. No invented physical grouping or GUIDs.
use super::*;
use razer_device::backend::HidNode;

pub fn discover_portable(
    client: &mut ServiceClient,
    nodes: &Result<Value, String>,
) -> anyhow::Result<DiscoverySnapshot> {
    let mut snapshot = DiscoverySnapshot::default();
    let result = nodes
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))?;
    let nodes: Vec<HidNode> =
        serde_json::from_value(result["nodes"].clone()).context("跨平台 HID 节点响应格式无效")?;
    // hidapi supplies no completeness guarantee. Absence remains unknown.
    snapshot
        .errors
        .push("HID 后端未报告枚举完整性；未发现的设备状态保持未知".into());
    let mut scopes = BTreeSet::new();
    for node in &nodes {
        let pid = u32::from(node.product_id);
        if node.vendor_id != 0x1532 {
            continue;
        }
        let IdentityLookup::Unique(identity) = device_identity::lookup(pid) else {
            snapshot
                .errors
                .push(format!("PID {pid} 的当前产品身份尚未唯一确认"));
            continue;
        };
        let read_cap = razer_device::keyboard_settings::capability(identity.product_id)
            .map(|cap| &cap.transport)
            .or_else(|| razer_device::keyboard_metadata::capability(identity.product_id))
            .or_else(|| razer_device::device_reads::capability(identity.product_id));
        let receiver = razer_device::receiver_capabilities::capability(node.product_id);
        let indicator = razer_device::mouse_indicator_led::capability(identity.product_id)
            .filter(|source| source.accepts(pid, None))
            .map(|source| source.transport(pid));
        let legacy_indicator = razer_device::mouse_receiver_legacy::capability(identity.product_id)
            .filter(|_| {
                razer_device::mouse_receiver_legacy::accepts(identity.product_id, pid, None)
            });
        let keyboard_metadata =
            razer_device::keyboard_metadata::hid_capability(identity.product_id, pid);
        let specification = if let Some(capability) = keyboard_metadata {
            Some(capability.claim_interface_for(pid).map(|interface| {
                (
                    capability.vendor_id,
                    interface,
                    capability.report_id,
                    capability.report_bytes,
                )
            }))
        } else if let Some(capability) = indicator {
            Some(capability.and_then(|cap| {
                cap.claim_interface_for(pid)
                    .map(|interface| (cap.vendor_id, interface, cap.report_id, cap.report_bytes))
            }))
        } else if let Some(cap) = legacy_indicator {
            Some(
                cap.claim_interface_for(pid)
                    .map(|interface| (cap.vendor_id, interface, cap.report_id, cap.report_bytes)),
            )
        } else if identity.is_dongle || receiver.is_some() {
            receiver.map(|cap| {
                Ok((
                    cap.vendor_id,
                    cap.claim_interface,
                    cap.report_id,
                    cap.report_bytes,
                ))
            })
        } else {
            read_cap
                .filter(|cap| cap.direct_pids.contains(&pid))
                .map(|cap| {
                    cap.claim_interface_for(pid).map(|interface| {
                        (cap.vendor_id, interface, cap.report_id, cap.report_bytes)
                    })
                })
        };
        let (vendor, interface, _, report_bytes) = match specification.transpose() {
            Ok(Some(specification)) => specification,
            Ok(None) => {
                snapshot
                    .errors
                    .push(format!("PID {pid} 尚无源核实的 collection 选择规格"));
                continue;
            }
            Err(error) => {
                snapshot
                    .errors
                    .push(format!("PID {pid} 的源接口映射未确认：{error:#}"));
                continue;
            }
        };
        if node.interface_number < 0 {
            snapshot.errors.push(format!(
                "PID {pid} 的系统后端未提供接口号；不能推定源接口 {interface}"
            ));
            continue;
        }
        if node.vendor_id != vendor || node.interface_number != i32::from(interface) {
            continue;
        }
        let scope = crate::direct::collection_scope(node);
        if !scopes.insert(scope.clone())
            || nodes
                .iter()
                .filter(|other| crate::direct::collection_scope(other) == scope)
                .count()
                != 1
        {
            snapshot
                .errors
                .push(format!("PID {pid} 的 collection 身份不唯一"));
            continue;
        }
        if let Err(error) = crate::direct::source_reports_match(client, node, report_bytes) {
            snapshot
                .errors
                .push(format!("PID {pid} 的 collection 未确认：{error:#}"));
            continue;
        }
        snapshot.insert(ObservedDevice {
            product_id: identity.product_id,
            real_product_id: pid,
            physical_product_id: pid,
            peer_product_id: None,
            read_values: None,
            keyboard_metadata: None,
            keyboard_serial: None,
            hid_node: Some(node.clone()),
            container: scope.clone(),
            serial: if identity.is_dongle {
                String::new()
            } else {
                node.serial_number.clone().unwrap_or_default()
            },
            use_ble: identity.is_ble,
            transport: if identity.is_ble {
                Some(ObservedTransport::Ble)
            } else if identity.is_dongle {
                Some(ObservedTransport::Dongle)
            } else if identity.real_product_id == identity.source_product_id {
                Some(ObservedTransport::Wired)
            } else {
                None
            },
            connection: DeviceConnectionObservation::HidPresent,
        });
        if receiver.is_none() {
            continue;
        }
        let queried = startup_receiver_query_with(
            pid,
            || client.request(ServiceRequest::HidNodeReceiverStatus { node: node.clone() }),
            std::thread::sleep,
        )
        .and_then(|reply| receiver_projection::project_hid_receiver_query(node, &reply));
        match queried {
            Ok(projected) => {
                let projected = projected.into_snapshot();
                snapshot.errors.extend(projected.errors);
                for peer in projected.devices {
                    snapshot.insert(peer);
                }
            }
            Err(error) => snapshot
                .errors
                .push(format!("PID {pid} 接收器状态未确认：{error:#}")),
        }
    }
    Ok(snapshot)
}
