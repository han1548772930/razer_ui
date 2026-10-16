use super::*;
use razer_device::keyboard_latest_protocol::{Operation, Reply, Target};
pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    ensure!(
        observed.product_id() == 756 && observed.peer_product_id().is_none(),
        "756 restore requires a direct actual adapter"
    );
    Ok(match super::keyboard_metadata::resolve(client, observed)? {
        super::keyboard_metadata::Route::Hid(DirectRoute::Collection { node, product_id }) => {
            ensure!(product_id == 756, "756 collection logical product changed");
            Target::Collection(node)
        }
        #[cfg(windows)]
        super::keyboard_metadata::Route::Hid(DirectRoute::Container(target)) => {
            Target::WindowsContainer(target)
        }
        #[cfg(windows)]
        super::keyboard_metadata::Route::Ble(target) => Target::WindowsBle(target),
        super::keyboard_metadata::Route::RelayCollection { .. } => {
            anyhow::bail!("756 latest restore excludes cached receiver routing")
        }
    })
}
pub fn execute(
    client: &mut ServiceClient,
    target: &Target,
    operation: Operation,
) -> anyhow::Result<Reply> {
    let reply = client.request(ServiceRequest::KeyboardLatestRestore {
        target: target.clone(),
        protocol_operation: operation,
    })?;
    ensure!(
        reply["target"] == serde_json::to_value(target)?,
        "756 restore response physical owner differs"
    );
    serde_json::from_value(reply["report"].clone()).map_err(Into::into)
}
