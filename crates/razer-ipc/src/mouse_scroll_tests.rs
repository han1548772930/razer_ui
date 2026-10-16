//! Wire-only roundtrips: no process, collection or device is opened.
use super::{RequestEnvelope, ServiceRequest};
use razer_device::{
    backend::HidNode,
    mouse_scroll::{
        EnabledScrollModes, ScrollChange, ScrollMode, ScrollProfileObservation, Submission,
    },
};
use serde_json::json;

#[test]
fn mouse_scroll_wire_preserves_receiver_profile_and_mode_mask_identity() {
    let node = HidNode {
        path: vec![0xff, 0, b'"', b'\\', 0x80],
        vendor_id: 5426,
        product_id: 241,
        interface_number: 0,
        usage_page: 0xff00,
        usage: 1,
        release_number: 0x1234,
        serial_number: None,
        manufacturer: None,
        product: None,
    };
    let submission = Submission::new(
        vec![
            ScrollChange::ModeV2 {
                selected: ScrollMode::MicroTactile,
                enabled: EnabledScrollModes::new(5).unwrap(),
            },
            ScrollChange::SmartReel(true),
        ],
        ScrollProfileObservation {
            selected_obm_profile_id: Some(3),
            first_matching_obm_slot_id: Some(5),
            matching_obm_profile_ids: vec![1, 3, 5],
        },
    );
    for peer_product_id in [None, Some(227)] {
        let envelope = RequestEnvelope {
            id: 29,
            request: ServiceRequest::HidNodeMouseScroll {
                node: node.clone(),
                product_id: 226,
                peer_product_id,
                submission: submission.clone(),
            },
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["request"]["operation"], "hid_node_mouse_scroll");
        assert_eq!(
            wire["request"]["submission"]["changes"][0]["ModeV2"]["enabled"],
            json!(5)
        );
        let restored: RequestEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.id, 29);
        let ServiceRequest::HidNodeMouseScroll {
            node: actual,
            product_id,
            peer_product_id: peer,
            submission: actual_submission,
        } = restored.request
        else {
            panic!("wire operation changed");
        };
        assert_eq!(actual, node);
        assert_eq!((product_id, peer), (226, peer_product_id));
        assert_eq!(actual_submission, submission);
        actual_submission.validate(product_id).unwrap();
    }
}
