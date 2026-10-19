//! Pure observations and simulated IPC. No worker, driver or device is run.
use super::*;
use razer_device::{
    backend::HidNode,
    mouse_current_dpi,
    mouse_scroll::{ScrollChange, ScrollProfileObservation, ScrollValue, Submission},
};
use serde_json::json;

fn observation(product_id: u32, physical: u16, peer: Option<u16>, status: u8) -> ObservedDevice {
    let source = mouse_current_dpi::capability(product_id).unwrap();
    let interface = if peer.is_some() {
        razer_device::receiver_capabilities::capability(physical)
            .unwrap()
            .claim_interface
    } else {
        source
            .transport()
            .claim_interface_for(u32::from(physical))
            .unwrap()
    };
    let node = HidNode {
        path: format!("simulated-scroll-{physical}").into_bytes(),
        vendor_id: 5426,
        product_id: physical,
        interface_number: i32::from(interface),
        usage_page: 0xff00,
        usage: 1,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    if let Some(peer) = peer {
        let receiver = razer_device::receiver_capabilities::capability(physical).unwrap();
        return project_hid_receiver_query(
            &node,
            &json!({"node":node,"identity_scope":"hid_collection",
            "query":"receiver_wireless_status_v2","source_class":receiver.source_class,
            "device_count":1,"devices":[{"product_id":peer,"status":status}]}),
        )
        .unwrap()
        .devices()[0]
            .clone();
    }
    ObservedDevice {
        product_id,
        real_product_id: u32::from(physical),
        physical_product_id: u32::from(physical),
        peer_product_id: None,
        container: crate::direct::collection_scope(&node),
        serial: String::new(),
        use_ble: false,
        transport: Some(ObservedTransport::Wired),
        connection: DeviceConnectionObservation::HidPresent,
        read_values: None,
        keyboard_metadata: None,
        keyboard_serial: None,
        hid_node: Some(node),
    }
}

fn submission() -> Submission {
    Submission::new(
        vec![ScrollChange::VirtualAcceleration(true)],
        ScrollProfileObservation::default(),
    )
}

fn reply(observed: &ObservedDevice, request: &Submission) -> Value {
    let source = mouse_current_dpi::capability(observed.product_id()).unwrap();
    let mut reply = json!({"node":observed.hid_node(),"product_id":observed.product_id(),
        "setting_scope":"source_scroll_profile","source_class":source.transport().source_class,
        "report":{"requested":request,"reports":[{"steps":[{"profile_id":1,
            "requested":ScrollValue::VirtualAcceleration(true),"acknowledgment_received":true,
            "acknowledged":null,"observed":ScrollValue::VirtualAcceleration(true)}]}]}});
    if let Some(peer) = observed.peer_product_id() {
        let session = source
            .receiver_session(observed.physical_product_id(), peer)
            .unwrap();
        reply["peer_product_id"] = json!(peer);
        reply["receiver_before"] = json!([[peer, 1]]);
        reply["receiver_after"] = json!([[peer, 1]]);
        reply["transport_source_class"] = json!(session.transport().source_class);
    }
    reply
}

#[test]
fn mouse_scroll_wired_own_and_shared_receivers_keep_exact_physical_and_profile_identity() {
    for (product, physical, peer) in [
        (153, 153, None),
        (170, 170, None),
        (203, 203, None),
        (204, 204, None),
        (214, 214, None),
        (226, 226, None),
        (45066, 45066, None),
        (170, 171, Some(171)),
        (204, 205, Some(205)),
        (214, 215, Some(215)),
        (226, 227, Some(227)),
        (45066, 45067, Some(45067)),
        (226, 241, Some(227)),
    ] {
        let observed = observation(product, physical, peer, 1);
        assert_eq!(observed.product_id(), product);
        assert!(crate::direct::mouse_scroll::available(&observed));
        let expected = submission();
        let node = observed.hid_node().unwrap();
        let mut calls = 0;
        let report =
            crate::direct::mouse_scroll::request_with(&observed, expected.clone(), |request| {
                calls += 1;
                Ok(match request {
                    ServiceRequest::HidNodes => json!({"nodes":[node]}),
                    ServiceRequest::HidNodeReports { node: actual } => {
                        assert_eq!(&actual, node);
                        json!({"node":node,"reports":{"feature":{"0":91}}})
                    }
                    ServiceRequest::HidNodeMouseScroll {
                        node: actual,
                        product_id,
                        peer_product_id,
                        submission,
                    } => {
                        assert_eq!(
                            (&actual, product_id, peer_product_id),
                            (node, product, peer.map(u32::from))
                        );
                        assert_eq!(submission, expected);
                        reply(&observed, &submission)
                    }
                    _ => panic!("scroll must retain the current physical route"),
                })
            })
            .unwrap();
        assert_eq!(calls, 3);
        assert!(report.verified_for(product));
    }
}

#[test]
fn mouse_scroll_rejects_wrong_task_confirmation_stale_identity_peer_and_cancellation() {
    for observed in [
        observation(153, 153, None, 1),
        observation(226, 241, Some(227), 1),
    ] {
        let node = observed.hid_node().unwrap();
        for failure in 0..11 {
            let result = crate::direct::mouse_scroll::request_with(
                &observed,
                submission(),
                |request| {
                    Ok(match request {
                        ServiceRequest::HidNodes => json!({"nodes":[node]}),
                        ServiceRequest::HidNodeReports { .. } => {
                            json!({"node":node,"reports":{"feature":{"0":91}}})
                        }
                        ServiceRequest::HidNodeMouseScroll { submission, .. } => {
                            if failure == 10 {
                                anyhow::bail!("canceled awaiting scroll reply");
                            }
                            let mut value = reply(&observed, &submission);
                            match failure {
                                0 => value["node"]["path"] = json!([0]),
                                1 => value["product_id"] = json!(182),
                                2 => value["source_class"] = json!("rzDevice30"),
                                3 => value["setting_scope"] = json!("active_dpi"),
                                4 => {
                                    value["report"]["reports"][0]["steps"][0]["profile_id"] =
                                        json!(3)
                                }
                                5 => {
                                    value["report"]["reports"][0]["steps"][0]["acknowledgment_received"] =
                                        json!(false)
                                }
                                6 => {
                                    value["report"]["reports"][0]["steps"][0]["observed"] =
                                        Value::Null
                                }
                                7 => {
                                    value["report"]["requested"]["changes"] =
                                        json!([ScrollChange::VirtualAcceleration(false)])
                                }
                                8 if observed.peer_product_id().is_some() => {
                                    value["receiver_after"] = json!([[227, 0]])
                                }
                                9 if observed.peer_product_id().is_some() => {
                                    value["transport_source_class"] = json!("rzDevice25Linker")
                                }
                                _ => value["report"]["reports"] = json!([]),
                            }
                            value
                        }
                        _ => panic!("unexpected scroll route"),
                    })
                },
            );
            if failure == 6 {
                let report = result.expect("a missing readback is valid partial progress");
                assert!(!report.verified_for(observed.product_id()));
                assert!(report.reports[0].steps[0].acknowledgment_received);
                assert!(report.reports[0].steps[0].observed.is_none());
            } else {
                assert!(result.is_err(), "accepted invalid scroll reply {failure}");
            }
        }
        for failure in 0..3 {
            assert!(
                crate::direct::mouse_scroll::request_with(&observed, submission(), |request| {
                    Ok(match request {
                        ServiceRequest::HidNodes => match failure {
                            0 => json!({"nodes":[]}),
                            1 => json!({"nodes":[node,node]}),
                            _ => json!({"nodes":[node]}),
                        },
                        ServiceRequest::HidNodeReports { .. } => {
                            json!({"node":node,"reports":{"feature":{"0":90}}})
                        }
                        _ => panic!("stale collection must stop before mutation"),
                    })
                })
                .is_err()
            );
        }
    }
    let offline = observation(226, 241, Some(227), 0);
    assert!(!crate::direct::mouse_scroll::available(&offline));
    assert!(
        crate::direct::mouse_scroll::request_with(&offline, submission(), |_| panic!(
            "offline peer dispatched"
        ))
        .is_err()
    );
}

#[test]
fn mouse_scroll_partial_and_mismatching_readbacks_survive_the_exact_route_boundary() {
    for observed in [
        observation(153, 153, None, 1),
        observation(226, 241, Some(227), 1),
    ] {
        let node = observed.hid_node().unwrap();
        for mismatch in [false, true] {
            let expected = Submission::new(
                vec![
                    ScrollChange::VirtualAcceleration(true),
                    ScrollChange::SmartReel(true),
                ],
                ScrollProfileObservation::default(),
            );
            let report =
                crate::direct::mouse_scroll::request_with(&observed, expected.clone(), |request| {
                    Ok(match request {
                        ServiceRequest::HidNodes => json!({"nodes":[node]}),
                        ServiceRequest::HidNodeReports { .. } => {
                            json!({"node":node,"reports":{"feature":{"0":91}}})
                        }
                        ServiceRequest::HidNodeMouseScroll { submission, .. } => {
                            assert_eq!(submission, expected);
                            let mut value = reply(&observed, &submission);
                            value["report"]["reports"][0]["steps"][0]["observed"] = if mismatch {
                                json!(ScrollValue::VirtualAcceleration(false))
                            } else {
                                Value::Null
                            };
                            value["report"]["reports"].as_array_mut().unwrap().push(json!({
                            "steps":[{"profile_id":1,"requested":ScrollValue::SmartReel(true),
                            "acknowledgment_received":true,"acknowledged":null,
                            "observed":ScrollValue::SmartReel(true)}]
                        }));
                            value["report"]["errors"] = json!(["first task was not confirmed"]);
                            value
                        }
                        _ => panic!("partial scroll reply must retain its exact route"),
                    })
                })
                .unwrap();
            assert!(!report.verified_for(observed.product_id()));
            assert_eq!(report.requested, expected);
            assert_eq!(report.errors, ["first task was not confirmed"]);
            assert_eq!(
                report.reports[0].steps[0].observed,
                mismatch.then_some(ScrollValue::VirtualAcceleration(false))
            );
            assert!(report.reports[1].verified());
        }
    }
}
