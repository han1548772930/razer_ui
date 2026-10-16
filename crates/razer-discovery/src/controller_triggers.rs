//! Current controller feature collection selected by observed container/path.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::{backend::HidNode, controller_triggers::capability};
use razer_ipc::ServiceClient;

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<HidNode> {
    let source =
        capability(observed.product_id()).context("No current controller trigger route")?;
    ensure!(
        source.accepts(observed.physical_product_id()) && observed.peer_product_id().is_none(),
        "Controller trigger physical identity differs"
    );
    super::controller_feature::resolve(client, observed, source.report_id(), 1)
}
