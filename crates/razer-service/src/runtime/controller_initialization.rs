//! One retained collection owns root fields, the real directory and profile reads.
use razer_device::{
    backend::HidNode,
    controller_initialization::{InitializationPlan, InitializationProtocol},
};
use razer_ipc::controller_initialization::InitializationResponse;
use serde_json::Value;
use std::time::Duration;

pub(super) fn execute(
    node: &HidNode,
    product_id: u32,
    plan: &InitializationPlan,
) -> anyhow::Result<Value> {
    plan.validate(product_id)?;
    super::controller_sleep::with_transport(
        node,
        product_id,
        Duration::from_secs(300),
        |transport, wait| {
            let result = InitializationProtocol::new(transport, product_id, wait)?
                .execute(plan, || false)?;
            serde_json::to_value(InitializationResponse::new(
                node.clone(),
                product_id,
                plan.clone(),
                result,
                transport.metadata(),
            )?)
            .map_err(Into::into)
        },
    )
}
