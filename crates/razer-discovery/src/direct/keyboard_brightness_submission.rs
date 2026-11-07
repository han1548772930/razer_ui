use super::{DirectRoute, receiver_settings};
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::keyboard_settings::{
    self, AppliedBrightness, BrightnessSubmission, KeyboardBrightness,
};
use razer_ipc::{ServiceClient, ServiceRequest};

pub(super) fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    submission: BrightnessSubmission,
) -> anyhow::Result<KeyboardBrightness> {
    submission.validate()?;
    let cap = keyboard_settings::capability(observed.product_id())
        .context("Keyboard brightness source is absent")?;
    let session = observed
        .peer_product_id()
        .map(|_| receiver_settings::session(observed, true))
        .transpose()?;
    let route = if let Some(session) = &session {
        receiver_settings::resolve(observed, session, &mut |request| client.request(request))?
    } else {
        super::resolve(client, observed, &cap.transport)?
    };
    let command = match &route {
        DirectRoute::Collection { node, product_id } => {
            ServiceRequest::HidNodeKeyboardBrightnessSubmit {
                node: node.clone(),
                product_id: *product_id,
                peer_product_id: observed.peer_product_id(),
                submission,
            }
        }
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceKeyboardBrightnessSubmit {
            target: target.clone(),
            submission,
        },
    };
    let reply = client.request(command)?;
    route.check_reply(&reply)?;
    ensure!(
        reply["source_class"] == cap.source_class,
        "Keyboard brightness response has another source owner"
    );
    if let Some(session) = session {
        ensure!(
            reply["peer_product_id"].as_u64() == observed.peer_product_id().map(u64::from)
                && reply["method"] == "applyBrightness",
            "Keyboard brightness receiver target differs"
        );
        receiver_settings::check_peers(&reply, &session)?;
    }
    let applied: AppliedBrightness = serde_json::from_value(reply["applied"].clone())?;
    applied.check_request(observed.product_id(), submission)?;
    applied
        .reading()
        .cloned()
        .context("Keyboard brightness was not read back")
}
