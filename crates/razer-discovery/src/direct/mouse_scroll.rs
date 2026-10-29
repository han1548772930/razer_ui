//! Scroll tasks use the same current physical factory as DPI, without using a
//! DPI write as a transport or projecting an ACK into source editor state.
use super::*;
use razer_device::mouse_scroll::{self as scroll, Submission, SubmissionReport};

pub fn available(observed: &ObservedDevice) -> bool {
    scroll::capability(observed.product_id()).is_some()
        && super::mouse_current_dpi::feature_available(observed)
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    submission: Submission,
) -> anyhow::Result<SubmissionReport> {
    request_with(observed, submission, |request| client.request(request))
}

pub(crate) fn request_with(
    observed: &ObservedDevice,
    submission: Submission,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<SubmissionReport> {
    ensure!(
        available(observed),
        "Mouse scrolling has no current physical source route"
    );
    submission.validate(observed.product_id())?;
    let source = razer_device::mouse_current_dpi::capability(observed.product_id())
        .context("Current scroll factory is absent")?;
    let session = observed
        .peer_product_id()
        .map(|peer| source.receiver_session(observed.physical_product_id(), peer))
        .transpose()?;
    let route = if let Some(session) = &session {
        super::receiver_settings::resolve(observed, session, &mut request)?
    } else {
        super::mouse_current_dpi::resolve_with(observed, &mut request)?
    };
    let query = match &route {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseScroll {
            node: node.clone(),
            product_id: *product_id,
            peer_product_id: observed.peer_product_id(),
            submission: submission.clone(),
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceMouseScroll {
            target: target.clone(),
            submission: submission.clone(),
        },
    };
    let reply = request(query)?;
    route.check_reply(&reply)?;
    if let Some(session) = &session {
        ensure!(
            reply["peer_product_id"] == observed.peer_product_id().unwrap(),
            "Scroll reply has another receiver peer"
        );
        super::receiver_settings::check_peers(&reply, session)?;
    }
    ensure!(
        reply["setting_scope"] == "source_scroll_profile"
            && reply["source_class"] == source.transport().source_class,
        "Scroll reply source or scope differs"
    );
    let report: SubmissionReport = serde_json::from_value(reply["report"].clone())?;
    ensure!(
        report.requested == submission,
        "Scroll reply belongs to another source submission"
    );
    report.validate_for(observed.product_id())?;
    Ok(report)
}
