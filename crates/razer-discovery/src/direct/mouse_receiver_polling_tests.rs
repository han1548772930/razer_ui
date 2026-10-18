//! Projected receiver observations and simulated IPC only.
use super::*;
use razer_device::receiver_capabilities;
use serde_json::json;

fn observation(physical: u16, status: u8) -> ObservedDevice {
    let cap = receiver_capabilities::capability(physical).unwrap();
    let node = HidNode {
        path: format!("simulated-receiver-{physical}").into_bytes(),
        vendor_id: cap.vendor_id,
        product_id: physical,
        interface_number: i32::from(cap.claim_interface),
        usage_page: 0xff00,
        usage: 1,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    let projection = crate::discovery::project_hid_receiver_query(
        &node,
        &json!({"node":node,"identity_scope":"hid_collection",
            "query":"receiver_wireless_status_v2","source_class":cap.source_class,
            "device_count":1,"devices":[{"product_id":183,"status":status}]}),
    )
    .unwrap();
    projection.devices()[0].clone()
}

fn response(request: ServiceRequest) -> Value {
    let ServiceRequest::HidNodeMouseReceiverRead {
        node,
        product_id,
        peer_product_id,
        kind,
    } = request
    else {
        panic!("unexpected polling request")
    };
    let route = ReceiverRead::new(
        product_id,
        u32::from(node.product_id),
        Some(peer_product_id),
        kind,
    )
    .unwrap();
    json!({"node":node,"product_id":product_id,"peer_product_id":peer_product_id,
        "method":route.method(),"source_class":route.transport().source_class,
        "reading":expected(kind)})
}

fn expected(kind: DeviceReadKind) -> DeviceReadValue {
    match kind {
        DeviceReadKind::Firmware => DeviceReadValue::Firmware {
            version: "1.2.3.4".into(),
        },
        DeviceReadKind::Battery => DeviceReadValue::Battery { percent: 73 },
        DeviceReadKind::Charging => DeviceReadValue::Charging {
            status: "Charging".into(),
        },
        DeviceReadKind::Polling => DeviceReadValue::Polling { hz: 1000 },
        DeviceReadKind::Dpi => DeviceReadValue::Dpi { x: 800, y: 900 },
        DeviceReadKind::Idle => DeviceReadValue::Idle { raw_time: 600 },
    }
}

fn descriptor(node: &HidNode) -> Value {
    let report = ReceiverRead::new(
        182,
        u32::from(node.product_id),
        Some(183),
        DeviceReadKind::Polling,
    )
    .unwrap();
    json!({"node":node,"reports":{"feature":{report.transport().report_id.to_string():91}}})
}

#[test]
fn idle_write_retains_receiver_identity_and_requires_independent_confirmation() {
    for physical in [183, 164, 179, 241] {
        let observed = observation(physical, 1);
        let node = observed.hid_node().unwrap();
        for mode in 0..6 {
            let result = write_idle_with(&observed, 300, |request| {
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { node: requested } => {
                        assert_eq!(&requested, node);
                        descriptor(node)
                    }
                    ServiceRequest::HidNodeMouseReceiverIdle {
                        node: requested,
                        product_id,
                        peer_product_id,
                        raw_time,
                    } => {
                        assert_eq!(
                            (&requested, product_id, peer_product_id, raw_time),
                            (node, 182, 183, 300)
                        );
                        let route = ReceiverRead::new(
                            182,
                            u32::from(physical),
                            Some(183),
                            DeviceReadKind::Idle,
                        )
                        .unwrap();
                        let mut reply = json!({"node":node,"product_id":182,"peer_product_id":183,
                            "method":"setTimeToSleep","source_class":route.transport().source_class,
                            "result":{"requested":{"kind":"idle","raw_time":300},"previous":null,
                                "observed":{"kind":"idle","raw_time":300},
                                "acknowledged":{"command":[2,7,3],"data":[]},"changed":true,"verified":true}});
                        match mode {
                            1 => reply["peer_product_id"] = json!(182),
                            2 => reply["result"]["acknowledged"] = Value::Null,
                            3 => reply["result"]["observed"] = Value::Null,
                            4 => reply["result"]["observed"]["raw_time"] = json!(301),
                            5 => reply["method"] = json!("getTimeToSleep"),
                            _ => {}
                        }
                        reply
                    }
                    _ => panic!("idle must use its retained receiver write"),
                })
            });
            assert_eq!(result.is_ok(), mode == 0);
        }
    }
    let offline = observation(183, 0);
    assert!(write_idle_with(&offline, 300, |_| panic!("offline peer must not dispatch")).is_err());
}

#[test]
fn polling_write_requires_exact_source_identity_and_a_distinct_pre_read_outcome() {
    for physical in [183, 164, 179, 241] {
        let observed = observation(physical, 1);
        let node = observed.hid_node().unwrap();
        let route = ReceiverRead::new(182, u32::from(physical), Some(183), DeviceReadKind::Polling)
            .unwrap();
        let effective = if matches!(physical, 164 | 179) { 8000 } else { 1000 };
        for mode in 0..12 {
            let result = write_with(&observed, 8000, |request| {
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { .. } => descriptor(node),
                    ServiceRequest::HidNodeMouseReceiverPolling {
                        node: selected,
                        product_id,
                        peer_product_id,
                        hz,
                    } => {
                        assert_eq!(
                            (&selected, product_id, peer_product_id, hz),
                            (node, 182, 183, 8000)
                        );
                        let mut reply = json!({"node":node,"product_id":182,"peer_product_id":183,
                            "method":route.method(),"source_class":route.transport().source_class,
                            "profile_scope":"current_active_profile","obm_profiles_written":false,
                            "result":{"requested_hz":8000,"effective_hz":effective,
                                "previous":{"status":"observed","hz":500},"observed_hz":effective,
                                "changed":true,"acknowledgement":[]}});
                        match mode {
                            1 => {
                                reply["result"]["previous"] =
                                    json!({"status":"observed","hz":effective});
                                reply["result"]["changed"] = json!(false);
                                reply["result"]["acknowledgement"] = Value::Null;
                            }
                            2 => {
                                reply["result"]["previous"] = json!({"status":"failed","message":"simulated pre-read failure"});
                            }
                            3 => {
                                reply["result"]["previous"] = json!({"status":"failed","message":"simulated pre-read failure"});
                                reply["result"]["changed"] = json!(false);
                                reply["result"]["acknowledgement"] = Value::Null;
                            }
                            4 => {
                                reply["result"]["previous"] =
                                    json!({"status":"observed","hz":effective});
                            }
                            5 => reply["result"]["acknowledgement"] = Value::Null,
                            6 => reply["result"]["acknowledgement"] = json!(vec![0u8; 81]),
                            7 => reply["result"]["previous"] = Value::Null,
                            8 => reply["result"]["observed_hz"] = json!(125),
                            9 => reply["peer_product_id"] = json!(182),
                            10 => reply["method"] = json!("getTimeToSleep"),
                            11 => reply["node"]["path"] = json!([0]),
                            _ => {}
                        }
                        reply
                    }
                    _ => panic!("polling must retain its receiver setter route"),
                })
            });
            assert_eq!(
                result.is_ok(),
                mode <= 2,
                "physical master {physical}, response case {mode}"
            );
        }
    }
}

#[test]
fn projected_own_and_shared_mouse_peers_reach_the_exact_portable_polling_request() {
    for physical in [183, 164, 179, 241] {
        let observed = observation(physical, 1);
        assert_eq!(observed.product_id(), 182);
        let node = observed.hid_node().unwrap();
        let mut calls = 0;
        let result = read(&observed, DeviceReadKind::Polling, |request| {
            calls += 1;
            Ok(match request {
                ServiceRequest::HidNodes => {
                    assert_eq!(calls, 1);
                    json!({"nodes":[node]})
                }
                ServiceRequest::HidNodeReports { node: requested } => {
                    assert_eq!(calls, 2);
                    assert_eq!(&requested, node);
                    descriptor(node)
                }
                ServiceRequest::HidNodeMouseReceiverRead {
                    ref node,
                    product_id,
                    peer_product_id,
                    kind,
                } => {
                    assert_eq!(calls, 3);
                    assert_eq!(node.product_id, physical);
                    assert_eq!((product_id, peer_product_id), (182, 183));
                    assert_eq!(kind, DeviceReadKind::Polling);
                    response(request)
                }
                _ => panic!("polling must not use the direct device request"),
            })
        })
        .unwrap();
        assert!(matches!(result, DeviceReadValue::Polling { hz: 1000 }));
        assert_eq!(calls, 3);
    }
}

#[test]
fn stale_ambiguous_or_wrong_descriptor_stops_before_the_receiver_request() {
    let observed = observation(179, 1);
    let node = observed.hid_node().unwrap();
    for mode in 0..4 {
        let mut calls = 0;
        let result = read(&observed, DeviceReadKind::Polling, |request| {
            calls += 1;
            match request {
                ServiceRequest::HidNodes => Ok(match mode {
                    0 => json!({"nodes":[]}),
                    1 => json!({"nodes":[node,node]}),
                    _ => json!({"nodes":[node]}),
                }),
                ServiceRequest::HidNodeReports { .. } => {
                    let mut reply = descriptor(node);
                    if mode == 2 {
                        reply["node"]["path"] = json!([0]);
                    } else {
                        reply["reports"]["feature"]["0"] = json!(90);
                    }
                    Ok(reply)
                }
                _ => panic!("invalid receiver must not receive a polling query"),
            }
        });
        assert!(result.is_err());
        assert_eq!(calls, if mode < 2 { 1 } else { 2 });
    }
}

#[test]
fn mismatched_peer_source_kind_or_failed_reply_cannot_become_a_polling_value() {
    let observed = observation(183, 1);
    let node = observed.hid_node().unwrap();
    for mode in 0..7 {
        let result = read(&observed, DeviceReadKind::Polling, |request| {
            Ok(match request {
                ServiceRequest::HidNodes => json!({"nodes":[node]}),
                ServiceRequest::HidNodeReports { .. } => descriptor(node),
                request => {
                    if mode == 6 {
                        anyhow::bail!("canceled while awaiting polling response");
                    }
                    let mut reply = response(request);
                    match mode {
                        0 => reply["peer_product_id"] = json!(182),
                        1 => reply["node"]["path"] = json!([0]),
                        2 => reply["product_id"] = json!(180),
                        3 => reply["method"] = json!("getUSBHighSpeedPollingRate"),
                        4 => reply["source_class"] = json!("rzDevice25Linker"),
                        5 => reply["reading"] = json!(DeviceReadValue::Battery { percent: 100 }),
                        _ => unreachable!(),
                    }
                    reply
                }
            })
        });
        assert!(result.is_err());
    }
}

#[test]
fn offline_observation_or_canceled_revalidation_cannot_start_a_polling_request() {
    assert!(
        read(&observation(183, 0), DeviceReadKind::Polling, |_| panic!(
            "offline peer must not query"
        ))
        .is_err()
    );
    let mut calls = 0;
    assert!(
        read(&observation(179, 1), DeviceReadKind::Polling, |_| {
            calls += 1;
            anyhow::bail!("enumeration canceled")
        })
        .is_err()
    );
    assert_eq!(calls, 1);
}

#[test]
fn all_six_basic_reads_preserve_the_requested_kind_across_receiver_ipc() {
    for physical in [183, 179, 241] {
        let observed = observation(physical, 1);
        let node = observed.hid_node().unwrap();
        for kind in [
            DeviceReadKind::Firmware,
            DeviceReadKind::Battery,
            DeviceReadKind::Charging,
            DeviceReadKind::Polling,
            DeviceReadKind::Dpi,
            DeviceReadKind::Idle,
        ] {
            let value = read(&observed, kind, |request| {
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { .. } => descriptor(node),
                    ServiceRequest::HidNodeMouseReceiverRead {
                        kind: requested, ..
                    } => {
                        assert_eq!(requested, kind);
                        response(request)
                    }
                    _ => panic!("Receiver read must not use a direct-only request"),
                })
            })
            .unwrap();
            assert_eq!(json!(value), json!(expected(kind)));
        }
    }
}

#[test]
fn production_read_values_dispatches_projected_peers_through_the_actual_wire_decoder() {
    use razer_device::backend::FeatureTransport;
    use std::cell::RefCell;

    struct Transport {
        sent: RefCell<Vec<Vec<u8>>>,
    }
    impl FeatureTransport for Transport {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.sent.borrow_mut().push(report.to_vec());
            Ok(())
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            report.copy_from_slice(self.sent.borrow().last().unwrap());
            let data: &[u8] = match &report[6..9] {
                [80, 0, 191] => &[1, 1, 0, 183],
                [4, 0, 135] => &[1, 2, 3, 4],
                [2, 7, 128] => &[0, 187],
                [2, 7, 132] => &[0, 1],
                [2, 0, 142] => &[1, 1],
                [2, 0, 192] => &[1, 8],
                [7, 4, 133] => &[0, 3, 32, 3, 132, 0, 0],
                [2, 7, 131] => &[2, 88],
                command => panic!("unexpected production getter {command:?}"),
            };
            report[1] = 2;
            report[6] = data.len() as u8;
            report[9..9 + data.len()].copy_from_slice(data);
            Ok(report.len())
        }
        fn metadata(&self) -> Value {
            json!({"simulated":true})
        }
    }

    for physical in [183, 164, 179, 241] {
        let observed = observation(physical, 1);
        let node = observed.hid_node().unwrap();
        let transport = Transport {
            sent: RefCell::default(),
        };
        let mut queried = Vec::new();
        let values = super::super::read_values_with(
            &observed,
            razer_device::device_reads::capability(182).unwrap(),
            |request| {
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { .. } => descriptor(node),
                    ServiceRequest::HidNodeMouseReceiverRead {
                        node: selected,
                        product_id,
                        peer_product_id,
                        kind,
                    } => {
                        assert_eq!(&selected, node);
                        let route = ReceiverRead::new(
                            product_id,
                            u32::from(physical),
                            Some(peer_product_id),
                            kind,
                        )?;
                        let mut transaction = 0;
                        let result = route.read(
                            &transport,
                            || {
                                let current = transaction;
                                transaction += 1;
                                Ok(current)
                            },
                            || Ok(()),
                        )?;
                        queried.push(kind);
                        json!({"node":selected,"product_id":product_id,"peer_product_id":peer_product_id,
                        "method":route.method(),"source_class":route.transport().source_class,"reading":result.reading})
                    }
                    _ => panic!("production receiver selection reached a wired-only request"),
                })
            },
        );
        assert!(values.errors.is_empty(), "{physical}: {:?}", values.errors);
        assert_eq!(values.firmware.as_deref(), Some("1.2.3.4"));
        assert_eq!(values.battery_percent, Some(73));
        assert_eq!(values.charging_status.as_deref(), Some("Charging"));
        assert_eq!(values.polling_hz, Some(1000));
        assert_eq!(values.dpi, Some((800, 900)));
        assert_eq!(values.idle_raw_time, Some(600));
        assert_eq!(queried.len(), 6);
        let sent = transport.sent.borrow();
        assert_eq!(sent.len(), 18);
        assert!(
            sent.iter()
                .all(|report| report[0] == if physical == 241 { 3 } else { 0 })
        );
        assert!(sent.iter().all(|report| report[2] < 31));
        for operation in sent.chunks_exact(3) {
            assert_eq!(&operation[0][6..9], &[80, 0, 191]);
            assert_eq!(&operation[2][6..9], &[80, 0, 191]);
        }
    }
}

#[test]
fn production_read_values_keeps_a_failed_receiver_getter_separate_from_other_readings() {
    let observed = observation(179, 1);
    let node = observed.hid_node().unwrap();
    let values = super::super::read_values_with(
        &observed,
        razer_device::device_reads::capability(182).unwrap(),
        |request| {
            Ok(match request {
                ServiceRequest::HidNodes => json!({"nodes":[node]}),
                ServiceRequest::HidNodeReports { .. } => descriptor(node),
                ServiceRequest::HidNodeMouseReceiverRead {
                    kind: DeviceReadKind::Polling,
                    ..
                } => {
                    anyhow::bail!("simulated polling transport failure")
                }
                ServiceRequest::HidNodeMouseReceiverRead { .. } => response(request),
                _ => panic!("receiver was sent to the direct-only branch"),
            })
        },
    );
    assert_eq!(values.polling_hz, None);
    assert_eq!(values.errors.len(), 1);
    assert!(values.errors["polling"].contains("simulated polling transport failure"));
    assert_eq!(values.battery_percent, Some(73));
    assert_eq!(values.dpi, Some((800, 900)));
}

fn dpi_draft() -> DpiStagesDraft {
    DpiStagesDraft {
        enabled: true,
        active_stage: 2,
        stages: [400, 2000]
            .into_iter()
            .map(|value| razer_device::mouse_dpi_stages::DpiStage {
                x: value,
                y: value,
                visible: true,
                independent: false,
            })
            .collect(),
    }
}

#[derive(Default)]
struct DpiTransport {
    reports: std::cell::RefCell<Vec<Vec<u8>>>,
    reads: std::cell::Cell<usize>,
}
impl razer_device::backend::FeatureTransport for DpiTransport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.reports.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(report[0], self.reports.borrow().last().unwrap()[0]);
        report.copy_from_slice(self.reports.borrow().last().unwrap());
        let data: &[u8] = match &report[6..9] {
            [80, 0, 191] => &[1, 1, 0, 183],
            [80, 4, 134] => &[1, 2, 2, 1, 1, 144, 1, 144, 0, 0, 2, 7, 208, 7, 208, 0, 0],
            [7, 4, 133] => &[0, 3, 32, 3, 132, 0, 0],
            [17, 4, 6] => &[], // Source setter success is an ACK, not a table.
            command => panic!("unexpected DPI command {command:?}"),
        };
        report[1] = 2;
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(data);
        self.reads.set(self.reads.get() + 1);
        Ok(report.len())
    }
    fn metadata(&self) -> Value {
        json!({"simulated":true})
    }
}

fn dpi_reply(
    request: ServiceRequest,
    transport: &DpiTransport,
    cancel_after_ack: bool,
) -> anyhow::Result<Value> {
    let ServiceRequest::HidNodeMouseReceiverDpiStages {
        node,
        product_id,
        peer_product_id,
        draft,
    } = request
    else {
        panic!("DPI receiver request reached a direct-only path")
    };
    let route = ReceiverRead::new(
        product_id,
        u32::from(node.product_id),
        Some(peer_product_id),
        DeviceReadKind::Dpi,
    )?;
    let mut transaction = 0;
    let next = || {
        transaction += 1;
        Ok(transaction - 1)
    };
    let validate = || {
        ensure!(
            !cancel_after_ack || transport.reads.get() < 2,
            "simulated cancellation after ACK"
        );
        Ok(())
    };
    let mut value = json!({"node":node,"product_id":product_id,"peer_product_id":peer_product_id,
        "source_class":route.transport().source_class,"profile_scope":"current_active_table","obm_profiles_written":false});
    if let Some(draft) = draft {
        value["method"] = json!("setDPIStages");
        value["result"] = serde_json::to_value(
            route
                .apply_dpi_stages(transport, &draft, next, validate)?
                .reading,
        )?;
    } else {
        value["method"] = json!("getDPIStages");
        value["reading"] =
            serde_json::to_value(route.read_dpi_stages(transport, next, validate)?.reading)?;
    }
    Ok(value)
}

#[test]
fn production_dpi_get_and_write_use_projected_peer_routes_and_real_protocol_readback() {
    for physical in [183, 164, 179, 241] {
        let observed = observation(physical, 1);
        let node = observed.hid_node().unwrap();
        assert!(super::super::mouse_dpi_stages_available(&observed));
        for draft in [None, Some(dpi_draft())] {
            let writing = draft.is_some();
            let transport = DpiTransport::default();
            let reading =
                super::super::mouse_dpi_stages_with(&observed, draft, |request| match request {
                    ServiceRequest::HidNodes => Ok(json!({"nodes":[node]})),
                    ServiceRequest::HidNodeReports { .. } => Ok(descriptor(node)),
                    request => dpi_reply(request, &transport, false),
                })
                .unwrap();
            assert_eq!(reading.active_stage, 2);
            assert_eq!((reading.records[1].x, reading.records[1].y), (2000, 2000));
            let reports = transport.reports.borrow();
            assert_eq!(reports.len(), if writing { 4 } else { 3 });
            assert!(
                reports
                    .iter()
                    .all(|report| report[0] == if physical == 241 { 3 } else { 0 })
            );
            assert!(reports.iter().all(|report| report[2] < 31));
            assert_eq!(&reports.first().unwrap()[6..9], &[80, 0, 191]);
            assert_eq!(&reports.last().unwrap()[6..9], &[80, 0, 191]);
            if writing {
                assert_eq!(&reports[1][6..13], &[17, 4, 6, 1, 2, 2, 0]);
            }
        }
    }
}

#[test]
fn production_dpi_rejects_wrong_peer_scope_table_or_cancellation_after_ack() {
    let observed = observation(241, 1);
    let node = observed.hid_node().unwrap();
    for mode in 0..5 {
        let transport = DpiTransport::default();
        let result = super::super::mouse_dpi_stages_with(&observed, Some(dpi_draft()), |request| {
            match request {
                ServiceRequest::HidNodes => Ok(json!({"nodes":[node]})),
                ServiceRequest::HidNodeReports { .. } => Ok(descriptor(node)),
                request => {
                    let mut reply = dpi_reply(request, &transport, mode == 4)?;
                    match mode {
                        0 => reply["peer_product_id"] = json!(190),
                        1 => reply["profile_scope"] = json!("onboard_profile"),
                        2 => reply["obm_profiles_written"] = json!(true),
                        3 => reply["result"]["observed"]["records"][1]["x"] = json!(1600),
                        _ => {}
                    }
                    Ok(reply)
                }
            }
        });
        assert!(result.is_err(), "mode {mode}");
        if mode == 4 {
            assert_eq!(transport.reports.borrow().len(), 2);
        }
    }
}

#[test]
fn production_dpi_offline_peer_stops_before_any_ipc_request() {
    let observed = observation(179, 0);
    assert!(!super::super::mouse_dpi_stages_available(&observed));
    let result = super::super::mouse_dpi_stages_with(&observed, None, |_| {
        panic!("offline peer sent to IPC")
    });
    assert!(result.is_err());
}

#[test]
fn mounted_source_dpi_dispatch_reaches_shared_receiver_without_transient_dpi_setter() {
    use razer_device::mouse_current_dpi::DpiChange;
    for physical in [179, 183, 164, 241] {
        let observed = observation(physical, 1);
        let node = observed.hid_node().unwrap();
        assert!(super::super::mouse_current_dpi::available(&observed));
        let mut stages = dpi_draft();
        stages
            .stages
            .extend([3200, 6400, 12800].into_iter().map(|dpi| {
                razer_device::mouse_dpi_stages::DpiStage {
                    x: dpi,
                    y: dpi,
                    visible: false,
                    independent: false,
                }
            }));
        for change in [
            None,
            Some(DpiChange {
                profile_guid: None,
                initialize: false,
                stages: Some(stages),
                active: None,
            }),
        ] {
            let writing = change.is_some();
            let transport = DpiTransport::default();
            let result = super::super::mouse_current_dpi::request_with(&observed, change.clone(), |request| {
                match request {
                    ServiceRequest::HidNodes => Ok(json!({"nodes":[node]})),
                    ServiceRequest::HidNodeReports { .. } => Ok(descriptor(node)),
                    ServiceRequest::HidNodeMouseCurrentDpi { node: selected, product_id, peer_product_id, change } => {
                        assert_eq!(&selected, node);
                        assert_eq!((product_id, peer_product_id), (182, Some(183)));
                        let route = ReceiverRead::new(product_id, u32::from(physical), peer_product_id, DeviceReadKind::Dpi)?;
                        let mut next = 0;
                        let observed = route.current_dpi(&transport, change.as_ref(), || { next += 1; Ok(next - 1) }, || Ok(()))?;
                        Ok(json!({"node":selected,"product_id":product_id,"peer_product_id":peer_product_id,
                            "source_class":route.transport().source_class,"profile_scope":"stage_1_active_0",
                            "transport_source_class":route.transport().source_class,
                            "receiver_before":observed.receiver_before,"receiver_after":observed.receiver_after,
                            "obm_persistence_verified":false,"report":observed.reading}))
                    }
                    _ => panic!("actual Source DPI request reached a wired-only or fallback stage path"),
                }
            }).unwrap();
            assert_eq!(result.requested, change);
            assert!(result.verified);
            assert_eq!(result.stages.as_ref().unwrap().records[1].x, 2000);
            assert_eq!(
                result.active.map(|value| (value.x, value.y)),
                if writing { None } else { Some((800, 900)) }
            );
            let reports = transport.reports.borrow();
            assert_eq!(reports.len(), 4);
            assert!(
                reports
                    .iter()
                    .all(|report| report[0] == if physical == 241 { 3 } else { 0 })
            );
            assert!(reports.iter().all(|report| report[7..9] != [4, 5]));
        }
    }
}

#[test]
fn source_dpi_dispatch_rejects_foreign_descriptor_offline_peer_and_cancelled_ack() {
    let mut stages = dpi_draft();
    stages.stages.extend(vec![stages.stages[0].clone(); 3]);
    for row in &mut stages.stages[2..] {
        row.visible = false;
    }
    let change = Some(razer_device::mouse_current_dpi::DpiChange {
        profile_guid: None,
        initialize: false,
        stages: Some(stages),
        active: None,
    });
    for mode in 0..4 {
        let observed = observation(179, if mode == 0 { 0 } else { 1 });
        let node = observed.hid_node().unwrap();
        let transport = DpiTransport::default();
        let result = super::super::mouse_current_dpi::request_with(
            &observed,
            change.clone(),
            |request| match request {
                ServiceRequest::HidNodes => Ok(json!({"nodes":[node]})),
                ServiceRequest::HidNodeReports { .. } => {
                    let mut reply = descriptor(node);
                    if mode == 1 {
                        reply["reports"]["feature"]["0"] = json!(65);
                    }
                    Ok(reply)
                }
                ServiceRequest::HidNodeMouseCurrentDpi {
                    node: selected,
                    product_id,
                    peer_product_id,
                    change,
                } => {
                    let route =
                        ReceiverRead::new(product_id, 179, peer_product_id, DeviceReadKind::Dpi)?;
                    let mut next = 0;
                    let observed = route.current_dpi(
                        &transport,
                        change.as_ref(),
                        || {
                            next += 1;
                            Ok(next - 1)
                        },
                        || {
                            ensure!(mode != 2 || transport.reads.get() < 2, "cancel after ACK");
                            Ok(())
                        },
                    )?;
                    Ok(
                        json!({"node":selected,"product_id":product_id,"peer_product_id":190,
                        "source_class":route.transport().source_class,"profile_scope":"stage_1_active_0",
                        "obm_persistence_verified":false,"report":observed.reading}),
                    )
                }
                _ => panic!("wrong Source DPI production request"),
            },
        );
        assert!(result.is_err(), "mode {mode}");
        assert_eq!(
            transport.reports.borrow().len(),
            match mode {
                0 | 1 => 0,
                2 => 2,
                _ => 4,
            }
        );
    }
}
