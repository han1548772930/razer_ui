//! Wire-schema regression only: no worker, native library, service or device.
use super::{RequestEnvelope, ServiceRequest};
use razer_device::{
    backend::HidNode,
    controller_triggers::{AnalogTrigger, DigitalTrigger, TriggerOperation},
};
use serde_json::Value;

#[test]
fn controller_trigger_requests_roundtrip_without_overwriting_the_ipc_discriminator() {
    let node = HidNode {
        path: vec![0xff, 0, b'"', b'\\', 0x80],
        vendor_id: 5426,
        product_id: 2685,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 1,
        release_number: 0x1234,
        serial_number: Some("controller \"scope\"\n1".into()),
        manufacturer: None,
        product: Some("test controller".into()),
    };
    for (kind, trigger_operation) in [
        ("read", TriggerOperation::Read { profile_slot: 1 }),
        (
            "analog",
            TriggerOperation::Analog {
                profile_slot: 2,
                settings: vec![
                    AnalogTrigger::new(1, 10, 90, true).unwrap(),
                    AnalogTrigger::new(2, 0, 100, false).unwrap(),
                ],
            },
        ),
        (
            "digital",
            TriggerOperation::Digital {
                profile_slot: 3,
                settings: vec![
                    DigitalTrigger::new(1, 0, false, false).unwrap(),
                    DigitalTrigger::new(2, 45, true, true).unwrap(),
                ],
            },
        ),
    ] {
        let envelope = RequestEnvelope {
            id: 17,
            request: ServiceRequest::ControllerTriggers {
                node: node.clone(),
                product_id: 2684,
                trigger_operation: trigger_operation.clone(),
            },
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["request"]["operation"], "controller_triggers");
        assert_eq!(wire["request"]["trigger_operation"]["kind"], kind);
        let restored: RequestEnvelope = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.id, 17);
        let ServiceRequest::ControllerTriggers {
            node: restored_node,
            product_id,
            trigger_operation: restored_operation,
        } = restored.request
        else {
            panic!("controller request discriminator changed")
        };
        assert_eq!(restored_node, node);
        assert_eq!(product_id, 2684);
        assert_eq!(restored_operation, trigger_operation);
    }
}
