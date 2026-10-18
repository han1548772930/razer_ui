//! Exact physical Uma target resolution and typed worker receipts.
use super::*;
use razer_device::hyperpolling_receiver::{self as uma, Command, Observation, Reading, Target};

pub fn resolve(client: &mut ServiceClient, route: &ReceiverRoute) -> anyhow::Result<Target> {
    ensure!(
        route.product_id() == u32::from(uma::PRODUCT_ID),
        "Uma owner is another receiver"
    );
    match route {
        ReceiverRoute::Collection(node) => {
            uma::validate_node(node)?;
            let reply = client.request(ServiceRequest::HidNodes)?;
            let nodes: Vec<HidNode> = serde_json::from_value(reply["nodes"].clone())?;
            ensure!(
                nodes.iter().filter(|candidate| *candidate == node).count() == 1,
                "Uma collection changed or is ambiguous"
            );
            direct::source_reports_match(client, node, uma::source_capability().report_bytes)?;
            Ok(Target::Collection(node.clone()))
        }
        #[cfg(windows)]
        ReceiverRoute::Container {
            product_id,
            container,
        } => {
            let snapshot = client.request(ServiceRequest::HidDevices)?;
            let path = discovery::source_receiver_path(&snapshot, container, *product_id, None)?;
            Ok(Target::WindowsContainer {
                path,
                container: container.clone(),
            })
        }
    }
}

pub fn execute(
    client: &mut ServiceClient,
    target: &Target,
    command: Command,
) -> anyhow::Result<Reading> {
    let reply: Observation =
        serde_json::from_value(client.request(ServiceRequest::HyperpollingReceiver {
            target: target.clone(),
            command,
        })?)?;
    reply.validate_for(target, command)?;
    Ok(reply.reading)
}
