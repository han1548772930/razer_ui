use super::*;
use serde_json::json;

const CONTAINER: &str = "{7A1F7C04-2B84-4BA3-A66C-17023B63E99F}";

fn observations(physical: u32, peers: &[(u32, u8)]) -> Vec<ObservedDevice> {
    let Some(cap) = razer_device::receiver_capabilities::capability(physical.try_into().unwrap())
    else {
        assert_eq!(peers, &[(physical, 1)]);
        return crate::discovery::discover_with(
            &Ok(json!({"complete":true,"devices":[{"vendor_id":5426,"product_id":physical,"device_container_id":CONTAINER}]})),
            &Ok(json!({"complete":true,"interfaces":[]})),
            &mut |_| panic!("ordinary dongle is qualified by its single-device query at registration"),
        ).unwrap().devices().to_vec();
    };
    let reply = json!({
        "query":"receiver_wireless_status_v2", "vendor_id":5426,
        "product_id":physical, "claim_interface":cap.claim_interface,
        "report_bytes":cap.report_bytes, "path":"observed-collection",
        "device_instance_id":"observed-instance", "device_container_id":CONTAINER,
        "device_count":peers.len(),
        "devices":peers.iter().map(|(pid,status)|json!({"product_id":pid,"status":status})).collect::<Vec<_>>()
    });
    crate::discovery::discover_with(
        &Ok(json!({"complete":true,"devices":[{"vendor_id":5426,"product_id":physical,"device_container_id":CONTAINER}]})),
        &Ok(json!({"complete":true,"interfaces":[{"vendor_id":5426,"product_id":physical,"device_container_id":CONTAINER,
            "claim_interface":cap.claim_interface,"feature_report_bytes":cap.report_bytes,"path":"observed-collection"}]})),
        &mut |_| Ok(reply.clone()),
    ).unwrap().devices().to_vec()
}

#[test]
fn own_dongle_requires_current_online_identity_and_observed_pairing() {
    let observed = observations(191, &[(191, 1)]);
    let empty = json!([]);
    assert!(plans(&observed, None).unwrap().is_empty());
    let selected = plans(&observed, Some(&empty)).unwrap();
    assert_eq!(selected.len(), 1);
    let target = selected[0].target();
    assert_eq!(target.physical_product_id(), 191);
    assert_eq!(target.members(), &[Member::direct(190)]);
    assert!(
        plans(&observations(193, &[(193, 0)]), Some(&empty))
            .unwrap()
            .is_empty()
    );
    assert!(
        plans(&observations(193, &[(193, 2)]), Some(&empty))
            .unwrap()
            .is_empty()
    );
    assert!(
        plans(&observations(179, &[(191, 1)]), Some(&empty))
            .unwrap()
            .is_empty()
    );
    assert!(plans(&observed, Some(&json!([{"dongleId":191}]))).is_err());
}

#[test]
fn actual_ordinary_compiler_radio_aliases_share_the_retained_physical_owner() {
    for (logical, physical) in [
        (89, 90),
        (112, 111),
        (115, 114),
        (148, 148),
        (158, 159),
        (185, 185),
        (190, 191),
        (192, 193),
        (194, 195),
        (196, 197),
        (222, 223),
        (224, 224),
        (229, 230),
        (239, 240),
    ] {
        let selected = plans(&observations(physical, &[(physical, 1)]), Some(&json!([]))).unwrap();
        assert_eq!(selected.len(), 1, "logical {logical}, physical {physical}");
        assert_eq!(selected[0].target().physical_product_id(), physical);
        assert!(selected[0].target().contains_product(logical));
        assert!(
            matches!(selected[0].publication,Publication::Ordinary { product } if product.product_id()==logical)
        );
    }
    let selected = plans(&observations(105, &[(105, 1)]), Some(&json!([]))).unwrap();
    let [plan] = selected.as_slice() else {
        panic!("one mounted105 physical owner required")
    };
    assert_eq!(plan.target().members()[0].product_id(), 105);
    assert_eq!(plan.target().members()[0].source_product_id(), 104);
    let documents = BTreeMap::from([(
        105,
        Document::new(
            json!({"productId":104,"activeProfile":"source-profile"}),
            None,
        ),
    )]);
    let publication = plan.project(&documents).unwrap().unwrap();
    assert_eq!(publication.products(), &[105]);
    assert_eq!(publication.document()["productId"], 104);
    let wrong = BTreeMap::from([(105, Document::new(json!({"productId":105}), None))]);
    assert!(plan.project(&wrong).is_err());
}

#[test]
fn slave_role_does_not_start_a_second_physical_publication() {
    let observed = observations(191, &[(191, 1)]);
    let records = json!([{"productId":190,"dongleId":191,"deviceContainerId":observed[0].container(),
        "master":{"productId":756,"dongleId":757}}]);
    assert!(plans(&observed, Some(&records)).unwrap().is_empty());
    let unrelated = json!([{"productId":190,"dongleId":191,"deviceContainerId":"other-container",
        "master":{"productId":756,"dongleId":757}}]);
    assert_eq!(plans(&observed, Some(&unrelated)).unwrap().len(), 1);
}

#[test]
fn ordinary_dongle_keeps_saved_logical_document_and_routes_only_its_callback() {
    let plan = plans(&observations(191, &[(191, 1)]), Some(&json!([])))
        .unwrap()
        .remove(0);
    let mut documents = BTreeMap::new();
    assert!(plan.project(&documents).unwrap().is_none());
    let document = json!({"productId":190,"profiles":[],"version":37});
    documents.insert(190, Document::new(document.clone(), None));
    let projected = plan.project(&documents).unwrap().unwrap();
    assert_eq!(projected.document(), &document);
    assert_eq!(projected.products(), &[190]);
    assert_eq!(
        plan.event_targets(
            &json!({"deviceEventType":2,"jsonEvent":{"type":"unsupportedmapping",
        "input":"{\"type\":\"razerKey\",\"reportId\":4}",
        "output":"{\"type\":\"navigateProfile\",\"name\":\"next\"}"}})
        )
        .unwrap(),
        vec![190]
    );
    assert!(
        plan.event_targets(
            &json!({"jsonEvent":{"type":"unsupportedmapping","input":"{", "output":"{}"}})
        )
        .is_err()
    );
    assert_eq!(
        plan.event_targets(
            &json!({"source":"devicemodechanged","eventType":4,"event":"{\"isEnabled\":true}"})
        )
        .unwrap(),
        vec![190]
    );
    assert!(
        plan.event_targets(&json!({"source":"devicemodechanged","eventType":1}))
            .unwrap()
            .is_empty()
    );
    documents.insert(
        190,
        Document::new(json!({"productId":192}), Some("serial".into())),
    );
    assert!(plan.project(&documents).is_err());
}

#[test]
fn normal_receiver_callback_uses_source_channels_and_explicit_ownership() {
    let plan = Plan {
        target: Target::new(
            159,
            CONTAINER.into(),
            vec![Member::peer(158, 159), Member::peer(190, 191)],
        )
        .unwrap(),
        publication: Publication::Single {
            master: 158,
            peer: 190,
            peer_dongle: 191,
        },
    };
    let event = |input: Value, output: Value| json!({"deviceEventType":2,"jsonEvent":{"type":"unsupportedmapping","input":input.to_string(),"output":output.to_string()}});
    assert_eq!(
        plan.event_targets(&event(json!({"type":"mouse","unitId":0}), json!({})))
            .unwrap(),
        vec![158]
    );
    assert_eq!(
        plan.event_targets(&event(json!({"type":"keyboard","unitId":43981}), json!({})))
            .unwrap(),
        vec![190]
    );
    assert_eq!(
        plan.event_targets(&event(json!({"type":"razerKey","reportId":8}), json!({})))
            .unwrap(),
        vec![190]
    );
    assert_eq!(
        plan.event_targets(&event(
            json!({"unitId":43981}),
            json!({"multiDevicesInfo":{"dongleId":159}})
        ))
        .unwrap(),
        vec![158]
    );
    assert!(
        plan.event_targets(&event(
            json!({"unitId":0}),
            json!({"multiDevicesInfo":{"productId":999}})
        ))
        .unwrap()
        .is_empty()
    );
}

#[test]
fn projection_reports_only_members_whose_saved_document_and_serial_were_consumed() {
    let plan = Plan {
        target: Target::new(
            159,
            CONTAINER.into(),
            vec![Member::peer(158, 159), Member::peer(190, 191)],
        )
        .unwrap(),
        publication: Publication::Single {
            master: 158,
            peer: 190,
            peer_dongle: 191,
        },
    };
    let source = |product| {
        json!({
            "version":1,"vendorId":5426,"productId":product,"category":"MOUSE",
            "reportIDs":{"4":"razerKeyReportID"},"activeProfile":"profile-a",
            "profiles":[{"guid":"profile-a","name":"Saved","mappings":[],"appEngine":{"mappings":[],"hash":""}}],
            "defaultMappings":{"appEngine":{"mappings":[],"hash":""}},
            "defaultTurbos":[],"synapseTurbos":[]
        })
    };
    let mut documents = BTreeMap::from([(158, Document::new(source(158), Some("master".into())))]);
    let projection = plan.project(&documents).unwrap().unwrap();
    assert_eq!(projection.products(), &[158]);
    assert_eq!(projection.document()["productId"], 159);
    assert_eq!(projection.document()["dualLinked"], true);
    documents.insert(190, Document::new(source(190), None));
    assert_eq!(
        plan.project(&documents).unwrap().unwrap().products(),
        &[158]
    );
    documents.insert(190, Document::new(source(190), Some("peer-current".into())));
    assert_eq!(
        plan.project(&documents).unwrap().unwrap().products(),
        &[158, 190]
    );
    assert_eq!(documents[&158].value["productId"], 158);
    assert!(documents[&158].value.get("dualLinked").is_none());
    documents.get_mut(&190).unwrap().value["productId"] = json!(192);
    assert!(plan.project(&documents).is_err());
}
