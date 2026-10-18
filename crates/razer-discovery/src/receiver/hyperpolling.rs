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
    transaction: u8,
) -> anyhow::Result<Reading> {
    Ok(observe(client, target, command, transaction)?.reading)
}

fn observe(
    client: &mut ServiceClient,
    target: &Target,
    command: Command,
    transaction: u8,
) -> anyhow::Result<Observation> {
    let reply: Observation =
        serde_json::from_value(client.request(ServiceRequest::HyperpollingReceiver {
            target: target.clone(),
            command,
            transaction,
        })?)?;
    reply.validate_for(target, command)?;
    Ok(reply)
}

/// A primary binding GET shares the same factory sequence as initialization,
/// setters and Cancel. JSON exists only at the existing projection boundary.
pub fn bindings(
    client: &mut ServiceClient,
    target: &Target,
    transaction: u8,
) -> anyhow::Result<discovery::ReceiverQueryProjection> {
    let reply = observe(client, target, Command::Bindings, transaction)?;
    let Reading::Bindings(devices) = reply.reading else {
        anyhow::bail!("Uma binding response type differs");
    };
    let cap = uma::source_capability();
    let mut value = serde_json::json!({
        "query":"receiver_wireless_status_v2", "source_class":cap.source_class,
        "source_product_id":null, "vendor_id":cap.vendor_id, "product_id":cap.product_id,
        "claim_interface":cap.claim_interface, "report_bytes":cap.report_bytes,
        "device_count":devices.len(), "devices":devices,
    });
    match target {
        Target::Collection(node) => {
            value["node"] = serde_json::to_value(node)?;
            value["identity_scope"] = "hid_collection".into();
            discovery::project_hid_receiver_query(node, &value)
        }
        Target::WindowsContainer { path, container } => {
            let instance = reply
                .physical_instance
                .filter(|instance| !instance.trim().is_empty())
                .context("Uma binding reply has no physical instance identity")?;
            value["path"] = path.clone().into();
            value["device_instance_id"] = instance.into();
            value["device_container_id"] = container.clone().into();
            discovery::project_receiver_query(u32::from(uma::PRODUCT_ID), container, &value)
        }
    }
}
