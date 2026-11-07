//! Portable collection observations. No invented physical grouping or GUIDs.
use super::*;
use razer_device::backend::HidNode;

pub fn discover_portable(
    client: &mut ServiceClient,
    nodes: &Result<Value, String>,
) -> anyhow::Result<DiscoverySnapshot> {
    discover_with(nodes, &mut |request| client.request(request))
}

fn discover_with(
    nodes: &Result<Value, String>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
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
    let mut queried_receivers = BTreeSet::new();
    for node in &nodes {
        let pid = u32::from(node.product_id);
        if node.vendor_id != 0x1532 {
            continue;
        }
        let identities = device_identity::lookup(pid);
        if identities.candidates().is_empty() {
            snapshot
                .errors
                .push(format!("PID {pid} 的当前产品身份尚未唯一确认"));
            continue;
        }
        for identity in identities.candidates() {
            let read_cap = razer_device::keyboard_settings::capability(identity.product_id)
                .map(|cap| &cap.transport)
                .or_else(|| razer_device::keyboard_metadata::capability(identity.product_id))
                .or_else(|| razer_device::device_reads::capability(identity.product_id));
            let receiver = razer_device::receiver_capabilities::capability(node.product_id);
            let indicator = razer_device::mouse_indicator_led::capability(identity.product_id)
                .filter(|source| source.accepts(pid, None))
                .map(|source| source.transport(pid));
            let legacy_indicator =
                razer_device::mouse_receiver_legacy::capability(identity.product_id).filter(|_| {
                    razer_device::mouse_receiver_legacy::accepts(identity.product_id, pid, None)
                });
            let keyboard_metadata =
                razer_device::keyboard_metadata::hid_capability(identity.product_id, pid);
            let mouse_power = razer_device::mouse_current_power::capability(identity.product_id)
                .filter(|cap| cap.accepts(pid, None) && !cap.is_ble(pid))
                .map(|cap| cap.transport(pid, false));
            let specification =
                if let Some(capability) = keyboard_metadata {
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
                        cap.claim_interface_for(pid).map(|interface| {
                            (cap.vendor_id, interface, cap.report_id, cap.report_bytes)
                        })
                    }))
                } else if let Some(cap) = legacy_indicator {
                    Some(cap.claim_interface_for(pid).map(|interface| {
                        (cap.vendor_id, interface, cap.report_id, cap.report_bytes)
                    }))
                } else if let Some(capability) = mouse_power {
                    Some(capability.and_then(|cap| {
                        cap.claim_interface_for(pid).map(|interface| {
                            (cap.vendor_id, interface, cap.report_id, cap.report_bytes)
                        })
                    }))
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
            if !scopes.insert((identity.product_id, scope.clone()))
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
            if let Err(error) =
                crate::direct::source_reports_match_with(node, report_bytes, request)
            {
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
            if receiver.is_none() || !queried_receivers.insert(scope.clone()) {
                continue;
            }
            let queried = startup_receiver_query_with(
                pid,
                || request(ServiceRequest::HidNodeReceiverStatus { node: node.clone() }),
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
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mika_shared_collection_publishes_both_products_without_guessing_a_peer() {
        for (product, physical) in [(574, 2308), (2595, 2593)] {
            let node = HidNode {
                path: b"mika-current-interface".to_vec(),
                vendor_id: 0x1532,
                product_id: physical,
                interface_number: 0,
                usage_page: 0xff00,
                usage: 1,
                release_number: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            };
            let mut request = |request| match request {
                ServiceRequest::HidNodeReports { node } => {
                    Ok(json!({"node":node,"reports":{"feature":{"0":91}}}))
                }
                _ => panic!("Mika collection presence is not a DualLink peer query"),
            };
            let snapshot = discover_with(&Ok(json!({"nodes":[node]})), &mut request).unwrap();
            let devices = snapshot
                .devices()
                .iter()
                .filter(|row| row.product_id() == product)
                .collect::<Vec<_>>();
            assert_eq!(devices.len(), 1);
            let observed = devices[0];
            assert_eq!(observed.physical_product_id(), u32::from(physical));
            assert_eq!(observed.peer_product_id(), None);
            assert_eq!(
                observed.connection(),
                DeviceConnectionObservation::HidPresent
            );
            assert!(crate::direct::keyboard_brightness_available(observed));
            assert!(observed.keyboard_serial().is_none());
            let mouse = if product == 574 { 117 } else { 2594 };
            assert!(
                snapshot
                    .devices()
                    .iter()
                    .any(|row| row.product_id() == mouse),
                "Shared Mika collection must retain its qualified mouse owner"
            );
            let duplicated =
                discover_with(&Ok(json!({"nodes":[node,node]})), &mut request).unwrap();
            assert!(duplicated.devices().is_empty());
            let failed = discover_with(&Ok(json!({"nodes":[node]})), &mut |_| {
                anyhow::bail!("descriptor unavailable")
            })
            .unwrap();
            assert!(failed.devices().is_empty());
            let mut wrong = node.clone();
            wrong.interface_number = 8;
            let rejected = discover_with(&Ok(json!({"nodes":[wrong]})), &mut |_| {
                panic!("wrong interface must not query descriptors")
            })
            .unwrap();
            assert!(rejected.devices().is_empty());
        }
    }
}
