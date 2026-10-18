use super::*;
use serde_json::json;

fn observation(physical: u16, peer: u16, status: u8) -> ObservedDevice {
    let source = razer_device::receiver_capabilities::capability(physical).unwrap();
    let node = HidNode {
        path: format!("simulated-power-receiver-{physical}").into_bytes(),
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
    crate::discovery::project_hid_receiver_query(&node, &json!({
        "node":node,"identity_scope":"hid_collection","query":"receiver_wireless_status_v2",
        "source_class":source.source_class,"device_count":1,"devices":[{"product_id":peer,"status":status}]
    })).unwrap().devices()[0].clone()
}

fn reply(node: &HidNode, product: u32, peer: Option<u32>, change: &Change) -> Value {
    let source = power::capability(product).unwrap();
    let cap = source
        .transport(u32::from(node.product_id), peer.is_some())
        .unwrap();
    json!({"node":node,"product_id":product,"peer_product_id":peer,
    "receiver_before":[[peer,1]],"receiver_after":[[peer,1]],
    "source_class":cap.source_class,"transport_source_class":cap.source_class,
    "setting_scope":"basic_device","report":{
        "requested":change,"acknowledgements":[{"command":[2,7,131],"transaction":1,"data":[0,75]}],
        "time_to_sleep":75,"critical_percent":null,"dim_before":null,"setter_sent":false
    }})
}

#[test]
fn receiver_power_retains_current_collection_and_raw_peer_across_product_branches() {
    for (physical, peer) in [
        (125, 125),
        (144, 144),
        (227, 227),
        (241, 227),
        (662, 234),
        (241, 45067),
    ] {
        let observed = observation(physical, peer, 1);
        let node = observed.hid_node().unwrap();
        let route = resolve_with(&observed, &mut |request| {
            Ok(match request {
                ServiceRequest::HidNodes => json!({"nodes":[node]}),
                ServiceRequest::HidNodeReports { node: actual } => {
                    assert_eq!(actual, *node);
                    json!({"node":node,"reports":{"feature":{"0":91}}})
                }
                _ => panic!("Only exact collection validation belongs to resolution"),
            })
        })
        .unwrap();
        assert!(
            matches!(&route, Route::Receiver { peer_product_id, .. } if *peer_product_id == u32::from(peer))
        );
        let report = route
            .mouse_current_power_with(Change::ReadPowerSaving, &mut |request| {
                let ServiceRequest::HidNodeMouseCurrentPower {
                    node: actual,
                    product_id,
                    peer_product_id,
                    change,
                } = request
                else {
                    panic!("The retained collection must use the collection command");
                };
                assert_eq!(
                    (&actual, product_id, peer_product_id),
                    (node, observed.product_id(), Some(u32::from(peer)))
                );
                Ok(reply(&actual, product_id, peer_product_id, &change))
            })
            .unwrap();
        assert_eq!(report.time_to_sleep, Some(75));
        assert!(!report.setter_sent);
    }
}

#[test]
fn offline_or_ambiguous_collection_does_not_dispatch_power() {
    let offline = observation(241, 227, 0);
    assert!(
        resolve_with(&offline, &mut |_| panic!(
            "Offline peer must stop before dispatch"
        ))
        .is_err()
    );
    let observed = observation(241, 227, 1);
    let node = observed.hid_node().unwrap();
    for nodes in [json!([]), json!([node, node])] {
        assert!(
            resolve_with(&observed, &mut |request| {
                assert!(matches!(request, ServiceRequest::HidNodes));
                Ok(json!({"nodes":nodes}))
            })
            .is_err()
        );
    }
    assert!(
        resolve_with(&observed, &mut |request| Ok(match request {
            ServiceRequest::HidNodes => json!({"nodes":[node]}),
            ServiceRequest::HidNodeReports { .. } =>
                json!({"node":node,"reports":{"feature":{"0":91,"7":128}}}),
            _ => panic!("Wrong Feature size must stop before power"),
        }))
        .is_err()
    );
}

#[test]
fn receiver_power_rejects_stale_identity_offline_peer_and_wrong_task_receipts() {
    let observed = observation(241, 227, 1);
    let node = observed.hid_node().unwrap();
    let route = Route::Receiver {
        route: DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        },
        peer_product_id: 227,
    };
    for failure in 0..11 {
        let result = route.mouse_current_power_with(Change::ReadPowerSaving, &mut |_| {
            let mut response = reply(node, 226, Some(227), &Change::ReadPowerSaving);
            match failure {
                0 => response["node"]["path"] = json!([1, 2, 3]),
                1 => response["product_id"] = json!(235),
                2 => response["peer_product_id"] = json!(234),
                3 => response["receiver_before"] = json!([[227, 0]]),
                4 => response["receiver_after"] = json!([[227, 0]]),
                5 => response["receiver_after"] = json!([[227, 1], [227, 1]]),
                6 => response["receiver_before"] = Value::Null,
                7 => response["transport_source_class"] = json!("rzDevice25DualLinkKeyboard"),
                8 => response["setting_scope"] = json!("source_scroll_profile"),
                9 => response["report"]["requested"] = json!({"field":"power_saving","value":2}),
                _ => response["report"]["acknowledgements"] = json!([]),
            }
            Ok(response)
        });
        assert!(result.is_err(), "Accepted invalid power receipt {failure}");
    }
}

#[test]
fn ordinary_mika_and_wired_power_keep_peer_absent() {
    for (product, physical) in [
        (117, 117),
        (117, 2308),
        (190, 191),
        (226, 226),
        (2594, 2593),
    ] {
        let cap = power::capability(product)
            .unwrap()
            .transport(physical, false)
            .unwrap();
        let node = HidNode {
            path: b"simulated-ordinary-power".to_vec(),
            vendor_id: cap.vendor_id,
            product_id: physical as u16,
            interface_number: i32::from(cap.claim_interface),
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        };
        let route = DirectRoute::Collection {
            node: node.clone(),
            product_id: product,
        };
        let report = route
            .mouse_current_power_with(None, Change::ReadPowerSaving, &mut |request| {
                let ServiceRequest::HidNodeMouseCurrentPower {
                    peer_product_id,
                    change,
                    ..
                } = request
                else {
                    panic!("Wrong ordinary request")
                };
                assert_eq!(peer_product_id, None);
                Ok(reply(&node, product, None, &change))
            })
            .unwrap();
        assert_eq!(report.time_to_sleep, Some(75));
    }
}
