//! Brightness uses the current physical factory and actual matching OBM slots.
use super::*;
use razer_device::{
    mouse_brightness::{Submission, TaskReport},
    receiver_session::ReceiverSession,
};

pub fn available(observed: &ObservedDevice) -> bool {
    razer_device::mouse_brightness::capability(observed.product_id()).is_some()
        && super::mouse_current_dpi::feature_available(observed)
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    submission: Submission,
) -> anyhow::Result<TaskReport> {
    request_with(observed, submission, &mut |request| client.request(request))
}

fn request_with(
    observed: &ObservedDevice,
    submission: Submission,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<TaskReport> {
    submission.validate(observed.product_id())?;
    ensure!(
        available(observed),
        "Brightness has no live current physical route"
    );
    let session = observed
        .peer_product_id()
        .map(|peer| {
            ReceiverSession::mouse(observed.product_id(), observed.physical_product_id(), peer)
        })
        .transpose()?;
    let route = if let Some(session) = &session {
        super::receiver_settings::resolve(observed, session, request)?
    } else {
        super::mouse_current_dpi::resolve_with(observed, request)?
    };
    let query = match &route {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseBrightness {
            node: node.clone(),
            product_id: *product_id,
            peer_product_id: observed.peer_product_id(),
            submission: submission.clone(),
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceMouseBrightness {
            target: target.clone(),
            submission: submission.clone(),
        },
    };
    let reply = request(query)?;
    route.check_reply(&reply)?;
    ensure!(
        reply["setting_scope"] == "active_and_matching_obm_brightness",
        "Brightness service response scope differs"
    );
    if let Some(session) = &session {
        ensure!(
            reply["peer_product_id"]
                == observed
                    .peer_product_id()
                    .context("Brightness peer is absent")?,
            "Brightness response peer differs"
        );
        super::receiver_settings::check_peers(&reply, session)?;
    }
    let report: TaskReport = serde_json::from_value(reply["report"].clone())?;
    ensure!(
        report.requested() == &submission,
        "Brightness report belongs to another request"
    );
    Ok(report)
}
