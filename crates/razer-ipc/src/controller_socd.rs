//! SOCD acknowledgement boundary; no setter reply is a new getter observation.
use anyhow::ensure;
use razer_device::backend::HidNode;
pub use razer_device::controller_socd::{SocdOperation, SocdResult, SocdWriteStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SocdResponse {
    pub node: HidNode,
    pub product_id: u32,
    pub operation: SocdOperation,
    pub result: SocdResult,
    pub transport_metadata: Value,
}
impl SocdResponse {
    pub fn new(
        node: HidNode,
        product_id: u32,
        operation: SocdOperation,
        result: SocdResult,
        transport_metadata: Value,
    ) -> anyhow::Result<Self> {
        operation.validate(product_id)?;
        result.validate()?;
        Ok(Self {
            node,
            product_id,
            operation,
            result,
            transport_metadata,
        })
    }
    pub fn decode(
        value: Value,
        node: &HidNode,
        product_id: u32,
        operation: SocdOperation,
    ) -> anyhow::Result<Self> {
        let reply: Self = serde_json::from_value(value)?;
        operation.validate(product_id)?;
        ensure!(
            &reply.node == node
                && reply.product_id == product_id
                && reply.operation == operation
                && node.vendor_id == 5426
                && matches!(node.product_id, 2676 | 2677 | 2680 | 2681),
            "Controller SOCD response belongs to another collection, slot or request"
        );
        reply.result.validate()?;
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::controller_initialization::SocdMode;
    use serde_json::json;

    fn node() -> HidNode {
        HidNode {
            path: vec![0xff, 0, 0x80],
            vendor_id: 5426,
            product_id: 2676,
            usage_page: 1,
            usage: 5,
            interface_number: 1,
            release_number: 1,
            serial_number: Some("SOCD owner".into()),
            manufacturer: None,
            product: None,
        }
    }
    #[test]
    fn controller_socd_reply_binds_real_collection_profile_slot_mode_and_result() {
        let operation = SocdOperation {
            slot: 3,
            mode: SocdMode::First,
        };
        let value = serde_json::to_value(
            SocdResponse::new(
                node(),
                2676,
                operation,
                SocdResult {
                    write: SocdWriteStatus::Acknowledged {
                        response_payload: vec![],
                    },
                    canceled: false,
                },
                json!({"simulated":true}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(SocdResponse::decode(value.clone(), &node(), 2676, operation).is_ok());
        for (path, changed) in [
            ("/node/path", json!([1])),
            ("/node/product_id", json!(2636)),
            ("/product_id", json!(2636)),
            ("/operation/slot", json!(4)),
            ("/operation/mode", json!("last")),
            ("/result/write", json!({"status":"not_attempted"})),
            ("/result/canceled", json!(null)),
        ] {
            let mut forged = value.clone();
            *forged.pointer_mut(path).unwrap() = changed;
            assert!(
                SocdResponse::decode(forged, &node(), 2676, operation).is_err(),
                "{path}"
            );
        }
        for mode in [json!("unknown"), json!(0)] {
            let mut forged = value.clone();
            forged["operation"]["mode"] = mode;
            assert!(SocdResponse::decode(forged, &node(), 2676, operation).is_err());
        }
    }
    #[test]
    fn controller_socd_wire_tag_and_payload_keep_the_observed_slot_and_collection_scope() {
        use crate::ServiceRequest;
        let request = ServiceRequest::ControllerSocd {
            node: node(),
            product_id: 2676,
            socd_operation: SocdOperation {
                slot: 0,
                mode: SocdMode::First,
            },
        };
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["operation"], "controller_socd");
        assert_eq!(value["socd_operation"], json!({"slot":0,"mode":"first"}));
        let restored: ServiceRequest = serde_json::from_value(value).unwrap();
        assert_eq!(
            restored.synchronous_device_task().unwrap(),
            Some(crate::device_tasks::Scope::collection(2676, &node()))
        );
        let ServiceRequest::ControllerSocd { socd_operation, .. } = restored else {
            panic!("wrong request")
        };
        assert_eq!(socd_operation.slot, 0);
        assert_eq!(socd_operation.wire_slot(), 1);
    }
}
