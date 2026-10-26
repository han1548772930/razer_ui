//! One identity-checked Feature collection owns the mounted generic SOCD SET.
use razer_device::{
    backend::HidNode,
    controller_socd::{SocdOperation, SocdProtocol},
};
use razer_ipc::controller_socd::SocdResponse;
use serde_json::Value;
use std::time::Duration;

pub(super) fn execute(
    node: &HidNode,
    product_id: u32,
    operation: SocdOperation,
) -> anyhow::Result<Value> {
    operation.validate(product_id)?;
    super::controller_sleep::with_transport(
        node,
        product_id,
        Duration::from_secs(90),
        |transport, wait| {
            let result =
                SocdProtocol::new(transport, wait).execute(product_id, operation, || false)?;
            serde_json::to_value(SocdResponse::new(
                node.clone(),
                product_id,
                operation,
                result,
                transport.metadata(),
            )?)
            .map_err(Into::into)
        },
    )
}
