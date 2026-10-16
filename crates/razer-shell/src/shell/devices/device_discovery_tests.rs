//! Receiver scope replacement never treats another container as the same mouse.
use super::{
    receiver_owner_matches, receiver_scope_matches, replace_receiver_observations,
    same_receiver_peer,
};
use razer_device::receiver_capabilities;
use razer_discovery::discovery;
use razer_model::model::DeviceConnectionObservation;
use serde_json::json;

const FIRST: &str = "{7A1F7C04-2B84-4BA3-A66C-17023B63E99F}";
const SECOND: &str = "{7A1F7C04-2B84-4BA3-A66C-17023B63E990}";

fn projection(container: &str, peers: &[(u32, u8)]) -> discovery::ReceiverQueryProjection {
    let capability = receiver_capabilities::capability(179).unwrap();
    discovery::project_receiver_query(179, container, &json!({
        "query":"receiver_wireless_status_v2",
        "vendor_id":capability.vendor_id,
        "product_id":179,
        "claim_interface":capability.claim_interface,
        "report_bytes":capability.report_bytes,
        "path":"test-observation-path",
        "device_instance_id":"test-observation-instance",
        "device_container_id":container,
        "device_count":peers.len(),
        "devices":peers.iter().map(|(pid,status)| json!({"product_id":pid,"status":status})).collect::<Vec<_>>()
    })).unwrap()
}

#[test]
fn repeated_binding_queries_replace_one_scope_without_accumulation() {
    let first = projection(FIRST, &[(183, 1)]);
    let other = projection(SECOND, &[(183, 1)]);
    let mut observations = first.devices().to_vec();
    observations.extend_from_slice(other.devices());
    let offline = projection(FIRST, &[(183, 0)]);
    for _ in 0..3 {
        let removed = replace_receiver_observations(
            &mut observations,
            &FIRST.to_ascii_lowercase(),
            179,
            offline.devices(),
        );
        assert_eq!(removed.len(), 1);
        assert_eq!(observations.len(), 2);
        assert_eq!(
            observations
                .iter()
                .find(|peer| receiver_scope_matches(peer, SECOND, 179))
                .unwrap()
                .connection(),
            DeviceConnectionObservation::ReceiverPeer(1)
        );
        assert_eq!(
            observations
                .iter()
                .find(|peer| receiver_scope_matches(peer, FIRST, 179))
                .unwrap()
                .connection(),
            DeviceConnectionObservation::ReceiverPeer(0)
        );
    }
    let removed = replace_receiver_observations(&mut observations, FIRST, 179, &[]);
    assert_eq!(removed.len(), 1);
    assert_eq!(observations.len(), 1);
    assert!(receiver_scope_matches(&observations[0], SECOND, 179));
    assert!(!receiver_scope_matches(&observations[0], SECOND, 241));
}

#[test]
fn partial_queries_keep_known_incoming_peers_and_expire_previous_scope() {
    let first = projection(FIRST, &[(183, 1)]);
    let partial = projection(FIRST, &[(183, 0), (60000, 1)]);
    assert!(!partial.errors().is_empty());
    let mut observations = first.devices().to_vec();
    replace_receiver_observations(&mut observations, FIRST, 179, partial.devices());
    assert_eq!(observations.len(), 1);
    assert_eq!(
        observations[0].connection(),
        DeviceConnectionObservation::ReceiverPeer(0)
    );
}

#[test]
fn metadata_requeries_preserve_reads_only_for_the_same_live_peer() {
    let original = projection(FIRST, &[(183, 1)]);
    let repeat = projection(&FIRST.to_ascii_lowercase(), &[(183, 1)]);
    let offline = projection(FIRST, &[(183, 0)]);
    let different_receiver = projection(SECOND, &[(183, 1)]);
    assert!(same_receiver_peer(
        &original.devices()[0],
        &repeat.devices()[0]
    ));
    assert!(!same_receiver_peer(
        &original.devices()[0],
        &offline.devices()[0]
    ));
    assert!(!same_receiver_peer(
        &original.devices()[0],
        &different_receiver.devices()[0]
    ));
}

#[test]
fn owner_guard_rejects_cached_preview_disconnected_and_replaced_receivers() {
    let mut device = projection(FIRST, &[(183, 1)]).devices()[0]
        .clone()
        .into_device()
        .unwrap();
    device.real_product_id = 179;
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::UsbPresent);
    assert!(receiver_owner_matches(&device, FIRST, 179));
    assert!(!receiver_owner_matches(&device, SECOND, 179));
    assert!(!receiver_owner_matches(&device, FIRST, 241));
    device.dashboard.connection_observation = None;
    assert!(!receiver_owner_matches(&device, FIRST, 179));
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::ReceiverPeer(1));
    assert!(!receiver_owner_matches(&device, FIRST, 179));
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::HidPresent);
    for serial in ["PREVIEW-179", "demo-device", "preview-device"] {
        device.serial_number = serial.into();
        assert!(!receiver_owner_matches(&device, FIRST, 179));
    }
    device.serial_number.clear();
    device.device_container_id = "{00000000-0000-0000-0000-000000000000}".into();
    assert!(!receiver_owner_matches(
        &device,
        &device.device_container_id,
        179
    ));
}
