use super::*;
use serde_json::json;

fn observation(physical: u16, peer: u16, status: u8) -> ObservedDevice {
    let source = razer_device::receiver_capabilities::capability(physical).unwrap();
    let node = HidNode {
        path: format!("simulated-receiver-{physical}").into_bytes(),
        vendor_id: source.vendor_id,
        product_id: physical,
        interface_number: i32::from(source.claim_interface),
        usage_page: 0xff00,
        usage: 1,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    crate::discovery::project_hid_receiver_query(&node, &json!({"node":node,"identity_scope":"hid_collection","query":"receiver_wireless_status_v2","source_class":source.source_class,"device_count":1,"devices":[{"product_id":peer,"status":status}]})).unwrap().devices()[0].clone()
}

fn keyboard_reply(node: &HidNode, product: u32, peer: u32, percent: Option<u8>) -> Value {
    let reading = json!({"profile_id":1,"region_id":1,"raw_value":127,"percent":50});
    let mut reply = json!({"node":node,"product_id":product,"peer_product_id":peer,"source_class":"rzDevice25","transport_source_class":"rzDevice25DualLinkKeyboard","receiver_before":[[peer,1]],"receiver_after":[[peer,1]],"method":if percent.is_some(){"setBrightness"}else{"getBrightness"}});
    if percent.is_some() {
        reply["result"] = json!({"requested_percent":50,"verified":true,"observed":reading});
    } else {
        reply["reading"] = reading;
    }
    reply
}

#[test]
fn keyboard_typed_ipc_retains_actual_receiver_and_logical_peer() {
    for (physical, peer) in [
        (625, 625),
        (604, 604),
        (635, 635),
        (656, 656),
        (757, 757),
        (662, 662),
        (241, 662),
        (241, 635),
        (241, 656),
        (241, 757),
    ] {
        let observed = observation(physical, peer, 1);
        assert!(super::super::keyboard_brightness_available(&observed));
        for percent in [None, Some(50)] {
            let node = observed.hid_node().unwrap();
            let result = keyboard(&observed, percent, &mut |request| {
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { node: actual } => {
                        assert_eq!(&actual, node);
                        json!({"node":node,"reports":{"feature":{"0":91,"9":20}}})
                    }
                    ServiceRequest::HidNodeKeyboardReceiverBrightness {
                        node: actual,
                        product_id,
                        peer_product_id,
                        percent: actual_percent,
                    } => {
                        assert_eq!(
                            (&actual, product_id, peer_product_id, actual_percent),
                            (node, observed.product_id(), u32::from(peer), percent)
                        );
                        keyboard_reply(node, product_id, peer_product_id, percent)
                    }
                    _ => panic!("Unexpected keyboard request"),
                })
            })
            .unwrap();
            assert_eq!(result.percent, 50);
        }
    }
}

#[test]
fn keyboard_rejects_stale_peer_or_unconfirmed_reply() {
    let observed = observation(241, 662, 1);
    let node = observed.hid_node().unwrap();
    for failure in 0..6 {
        assert!(
            keyboard(&observed, Some(50), &mut |request| Ok(match request {
                ServiceRequest::HidNodes => json!({"nodes":[node]}),
                ServiceRequest::HidNodeReports { .. } =>
                    json!({"node":node,"reports":{"feature":{"0":91}}}),
                ServiceRequest::HidNodeKeyboardReceiverBrightness { .. } => {
                    let mut reply = keyboard_reply(node, 664, 662, Some(50));
                    match failure {
                        0 => reply["receiver_before"] = json!([[662, 0]]),
                        1 => reply["receiver_after"] = json!([[625, 1]]),
                        2 => reply["receiver_after"] = Value::Null,
                        3 => reply["peer_product_id"] = json!(664),
                        4 => reply["result"]["verified"] = json!(false),
                        _ => {
                            reply["transport_source_class"] = json!("rzDevice25LinkerMultiDevices")
                        }
                    }
                    reply
                }
                _ => panic!("Unexpected request"),
            }))
            .is_err()
        );
    }
    let offline = observation(241, 662, 0);
    assert!(!super::super::keyboard_brightness_available(&offline));
    assert!(
        keyboard(&offline, None, &mut |_| panic!(
            "Offline observation must not dispatch"
        ))
        .is_err()
    );
}

#[test]
fn keyboard_stale_collection_or_wrong_feature_maximum_stops_before_task() {
    let observed = observation(241, 662, 1);
    let node = observed.hid_node().unwrap();
    for stale in [false, true] {
        assert!(
            keyboard(&observed, None, &mut |request| Ok(match request {
                ServiceRequest::HidNodes =>
                    if stale {
                        json!({"nodes":[]})
                    } else {
                        json!({"nodes":[node]})
                    },
                ServiceRequest::HidNodeReports { .. } =>
                    json!({"node":node,"reports":{"feature":{"3":91,"9":128}}}),
                _ => panic!("No setting may run after stale identity or descriptor"),
            }))
            .is_err()
        );
    }
}

#[test]
fn generic_dpi_receiver_read_reuses_typed_ipc_and_checks_both_peer_observations() {
    for physical in [227, 241] {
        let observed = observation(physical, 227, 1);
        assert!(super::super::mouse_dpi_stages_available(&observed));
        let node = observed.hid_node().unwrap();
        for stale in [false, true] {
            let result = dpi_stages(&observed, None, &mut |request| {
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { .. } => {
                        json!({"node":node,"reports":{"feature":{"0":91}}})
                    }
                    ServiceRequest::HidNodeMouseReceiverDpiStages {
                        node: actual,
                        product_id,
                        peer_product_id,
                        draft,
                    } => {
                        assert_eq!(
                            (&actual, product_id, peer_product_id, draft.is_none()),
                            (node, 226, 227, true)
                        );
                        let cap = mouse_dpi_stages::capability(226).unwrap();
                        json!({"node":node,"product_id":226,"peer_product_id":227,"source_class":cap.source_class,"transport_source_class":"rzDevice25DualLinkMouse","method":"getDPIStages","profile_scope":"current_active_table","obm_profiles_written":false,"receiver_before":[[227,1]],"receiver_after":[[227,if stale {0}else{1}]],"reading":{"profile_id":1,"active_stage":1,"number_of_active_stages":1,"records":[{"index":1,"x":800,"y":800,"z":0}],"response_bytes":10}})
                    }
                    _ => panic!("Unexpected DPI request"),
                })
            });
            assert_eq!(result.is_ok(), !stale);
        }
    }
}

#[test]
fn current_dpi_receiver_read_keeps_separate_stage_and_transient_values_and_checks_scope() {
    for (physical, peer) in [(183, 183), (179, 183), (227, 227), (241, 227), (662, 227)] {
        let observed = observation(physical, peer, 1);
        assert!(super::super::mouse_current_dpi::available(&observed));
        let node = observed.hid_node().unwrap();
        for failure in 0..6 {
            let result = super::super::mouse_current_dpi::request_with(
                &observed,
                None,
                |request| {
                    Ok(match request {
                        ServiceRequest::HidNodes => json!({"nodes":[node]}),
                        ServiceRequest::HidNodeReports { .. } => {
                            json!({"node":node,"reports":{"feature":{"3":91}}})
                        }
                        ServiceRequest::HidNodeMouseCurrentDpi {
                            node: actual,
                            product_id,
                            peer_product_id,
                            change,
                        } => {
                            assert_eq!(
                                (&actual, product_id, peer_product_id, change.is_none()),
                                (node, observed.product_id(), Some(u32::from(peer)), true)
                            );
                            let source =
                                razer_device::mouse_current_dpi::capability(product_id).unwrap();
                            let mut reply = json!({"node":node,"product_id":product_id,"peer_product_id":peer,
                            "source_class":source.transport().source_class,"transport_source_class":"rzDevice25DualLinkMouse",
                            "profile_scope":"stage_1_active_0","obm_persistence_verified":false,
                            "receiver_before":[[peer,1]],"receiver_after":[[peer,1]],
                            "report":{"requested":null,"stages":{"profile_id":1,"active_stage":1,"number_of_active_stages":1,
                                "records":[{"index":1,"x":800,"y":800,"z":0}],"response_bytes":10},
                                "active":{"x":2000,"y":2000},"acknowledgements":[],"verified":true}});
                            match failure {
                                1 => reply["receiver_after"] = json!([[peer, 0]]),
                                2 => reply["peer_product_id"] = json!(65535),
                                3 => {
                                    reply["transport_source_class"] =
                                        json!("rzDevice25DualLinkKeyboard")
                                }
                                4 => reply["receiver_before"] = json!([[peer, 1], [peer, 1]]),
                                5 => reply["report"]["active"] = Value::Null,
                                _ => {}
                            }
                            reply
                        }
                        _ => panic!("Unexpected current DPI request"),
                    })
                },
            );
            assert_eq!(result.is_ok(), failure == 0);
            if let Ok(report) = result {
                assert_eq!(report.stages.unwrap().records[0].x, 800);
                assert_eq!(report.active.unwrap().x, 2000);
            }
        }
    }
}
