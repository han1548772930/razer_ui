//! Current mounted lighting branch, including its product-specific USB interface.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::{backend::HidNode, controller_lighting::capability};
use razer_ipc::ServiceClient;

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<HidNode> {
    let source = capability(observed.product_id()).context("No current controller lighting route")?;
    ensure!(source.accepts(observed.physical_product_id()), "Controller lighting physical identity differs");
    super::controller_feature::resolve(client, observed, source.report_id(), source.claim_interface())
}
