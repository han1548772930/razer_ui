//! Typed controller Power reply boundary. Real raw seconds remain independent
//! of the optimistic root setting and the setter's acknowledgement.
use anyhow::ensure;
use razer_device::backend::HidNode;
pub use razer_device::controller_sleep::{
    PowerSaving, SleepCompletion, SleepObservation, SleepOperation, SleepReadStatus, SleepResult,
    SleepWriteStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct SleepResponse {
    pub node: HidNode,
    pub product_id: u32,
    pub operation: SleepOperation,
    pub result: SleepResult,
    pub transport_metadata: Value,
}
impl SleepResponse {
    pub fn new(
        node: HidNode,
        product_id: u32,
        operation: SleepOperation,
        result: SleepResult,
        transport_metadata: Value,
    ) -> anyhow::Result<Self> {
        result.validate_for(&operation)?;
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
        operation: &SleepOperation,
    ) -> anyhow::Result<Self> {
        let reply: Self = serde_json::from_value(value)?;
        let capability = razer_device::controller_sleep::capability(product_id)
            .ok_or_else(|| anyhow::anyhow!("No current controller sleep capability"))?;
        ensure!(
            &reply.node == node
                && reply.product_id == product_id
                && &reply.operation == operation
                && node.vendor_id == 5426
                && capability.accepts(u32::from(node.product_id)),
            "Controller sleep response identity differs"
        );
        reply.result.validate_for(operation)?;
        Ok(reply)
    }
    pub fn result(&self) -> &SleepResult {
        &self.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node() -> HidNode {
        HidNode {
            path: vec![0xff, 0, b'"', b'\\', 0x80],
            vendor_id: 5426,
            product_id: 2636,
            usage_page: 0xff00,
            usage: 1,
            interface_number: 1,
            release_number: 0x1234,
            serial_number: Some("controller sleep scope".into()),
            manufacturer: None,
            product: None,
        }
    }
    fn response(operation: &SleepOperation) -> Value {
        let result = SleepResult::new(
            operation,
            if operation.is_read() {
                SleepWriteStatus::NotRequested
            } else {
                SleepWriteStatus::Acknowledged
            },
            SleepReadStatus::Observed {
                observation: SleepObservation::from_seconds(300),
            },
            if operation.is_read() {
                SleepCompletion::Observed
            } else {
                SleepCompletion::Confirmed
            },
        )
        .unwrap();
        serde_json::to_value(
            SleepResponse::new(
                node(),
                2636,
                operation.clone(),
                result,
                json!({"simulated":true}),
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn controller_sleep_reply_requires_identity_and_raw_seconds_confirmation() {
        let operation = SleepOperation::Set {
            power_saving: PowerSaving::new(5, true).unwrap(),
        };
        let value = response(&operation);
        assert!(SleepResponse::decode(value.clone(), &node(), 2636, &operation).is_ok());
        for (pointer, changed) in [
            ("/node/path", json!([1])),
            ("/node/product_id", json!(2676)),
            ("/product_id", json!(2676)),
            ("/operation/power_saving/value", json!(15)),
            ("/result/read/observation/seconds", json!(301)),
            ("/result/write/status", json!("not_attempted")),
        ] {
            let mut forged = value.clone();
            *forged.pointer_mut(pointer).unwrap() = changed;
            assert!(
                SleepResponse::decode(forged, &node(), 2636, &operation).is_err(),
                "{pointer}"
            );
        }
    }
    #[test]
    fn controller_sleep_reply_rejects_missing_fields_and_impossible_failure_or_cancel_states() {
        let operation = SleepOperation::Set {
            power_saving: PowerSaving::new(5, true).unwrap(),
        };
        let mut forged = response(&operation);
        forged["result"] = json!({"write":{"status":"not_attempted"},"read":{"status":"failed","error":"failure"},"completion":"failed"});
        assert!(SleepResponse::decode(forged.clone(), &node(), 2636, &operation).is_err());
        forged["result"] = json!({"write":{"status":"acknowledged"},"read":{"status":"unobserved"},"completion":"canceled_before_operation"});
        assert!(SleepResponse::decode(forged.clone(), &node(), 2636, &operation).is_err());
        forged["result"] = json!({"write":{"status":"not_attempted"},"read":{"status":"observed","observation":{"seconds":300}},"completion":"confirmed"});
        assert!(SleepResponse::decode(forged, &node(), 2636, &operation).is_err());
        let mut missing = response(&SleepOperation::Read);
        missing["result"]["read"]["observation"]
            .as_object_mut()
            .unwrap()
            .remove("seconds");
        assert!(SleepResponse::decode(missing, &node(), 2636, &SleepOperation::Read).is_err());
    }
}
