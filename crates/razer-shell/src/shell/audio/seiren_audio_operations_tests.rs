use super::{Pending, Receipt, State, current_route};
use crate::shell::audio_notifications::MixerReceipt;
use razer_device::{backend::HidNode, receiver_capabilities};
use razer_discovery::discovery::{self, ObservedDevice};
use serde_json::json;
use std::sync::mpsc;

fn observation(path: &[u8], peer: u16, status: u8) -> ObservedDevice {
    let source = receiver_capabilities::capability(241).unwrap();
    let node = HidNode {
        path: path.to_vec(),
        vendor_id: source.vendor_id,
        product_id: 241,
        interface_number: i32::from(source.claim_interface),
        usage_page: 0xff00,
        usage: 1,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    let projection = discovery::project_hid_receiver_query(
        &node,
        &json!({
            "node":node,"identity_scope":"hid_collection","query":"receiver_wireless_status_v2",
            "source_class":source.source_class,"device_count":1,
            "devices":[{"product_id":peer,"status":status}]
        }),
    )
    .unwrap();
    assert!(projection.errors().is_empty(), "{:?}", projection.errors());
    let [observed] = projection.devices() else {
        panic!("one receiver identity expected");
    };
    observed.clone()
}

#[test]
fn seiren_audio_accepted_work_is_pending_before_poll_and_until_every_receipt_releases() {
    let mut state = State::default();
    let (first, accepted) = state.accept().unwrap();
    let (second, other) = state.accept().unwrap();
    assert_ne!(first, second);
    assert!(state.pending());
    drop(accepted);
    assert!(state.pending());
    drop(other);
    assert!(!state.pending());
}

#[test]
fn seiren_audio_unpolled_delivery_drop_releases_admission() {
    let mut state = State::default();
    let (_, accepted) = state.accept().unwrap();
    let unpolled = async move {
        let _accepted = accepted;
        panic!("canceled delivery must never be polled");
    };
    assert!(state.pending());
    drop(unpolled);
    assert!(!state.pending());
}

#[test]
fn seiren_audio_exit_resume_waits_for_accepted_work_and_does_not_replay_it() {
    let mut state = State::default();
    let (old_id, accepted) = state.accept().unwrap();
    assert!(state.take_for_shutdown().is_empty());
    assert!(state.accept().is_none());
    state.resume_after_exit();
    assert!(state.accept().is_none());
    drop(accepted);
    state.resume_after_exit();
    assert!(!state.pending());
    assert!(state.operations.is_empty());
    let (new_id, accepted) = state.accept().unwrap();
    assert!(new_id > old_id);
    drop(accepted);
    assert!(!state.pending());
}

#[test]
fn seiren_audio_repeated_route_and_unrelated_discovery_preserve_accepted_work() {
    let route = observation(b"audio-route-owner", 227, 1);
    let device = route.clone().into_device().unwrap();
    let repeated = observation(b"audio-route-owner", 227, 1);
    assert!(current_route(Some(&route), &[repeated.clone()], &device));
    let unrelated = observation(b"unrelated-device", 183, 1);
    assert!(current_route(Some(&route), &[unrelated, repeated], &device));
}

#[test]
fn seiren_audio_disconnect_replacement_peer_change_and_ambiguity_reject_stale_readback() {
    let route = observation(b"audio-route-owner", 227, 1);
    let device = route.clone().into_device().unwrap();
    for next in [
        vec![],
        vec![observation(b"audio-route-owner", 227, 0)],
        vec![observation(b"audio-route-replaced", 227, 1)],
        vec![observation(b"audio-route-owner", 183, 1)],
        vec![route.clone(), route.clone()],
    ] {
        assert!(!current_route(Some(&route), &next, &device));
    }
    assert!(current_route(None, &[], &device));
    assert!(!current_route(None, &[route], &device));
}

#[test]
fn seiren_audio_real_owner_receipt_and_closed_or_failed_channels_remain_distinct() {
    let (sender, receiver) = mpsc::channel();
    sender.send(Ok(MixerReceipt::LocalSaved)).unwrap();
    assert!(matches!(
        Pending::Mixer(receiver).receive(),
        Ok(Receipt::Mixer(MixerReceipt::LocalSaved))
    ));
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Err(anyhow::anyhow!("native write rejected")))
        .unwrap();
    let result = Pending::Mixer(receiver).receive();
    assert!(matches!(result, Err(error) if error.to_string().contains("native write rejected")));
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    assert!(
        matches!(Pending::Mixer(receiver).receive(), Err(error) if error.to_string().contains("closed before its receipt"))
    );
}
