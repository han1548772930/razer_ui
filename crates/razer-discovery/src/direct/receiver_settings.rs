use super::*;
use crate::discovery::ObservedTransport;
use razer_device::receiver_session::ReceiverSession;
use razer_model::model::DeviceConnectionObservation;

pub(super) fn session(
    observed: &ObservedDevice,
    keyboard: bool,
) -> anyhow::Result<ReceiverSession> {
    ensure!(
        observed.transport() == Some(ObservedTransport::Dongle)
            && observed.connection() == DeviceConnectionObservation::ReceiverPeer(1),
        "Setting requires a live receiver peer observation"
    );
    let peer = observed
        .peer_product_id()
        .context("Receiver observation has no raw peer")?;
    if keyboard {
        ReceiverSession::keyboard(observed.product_id(), observed.physical_product_id(), peer)
    } else {
        ReceiverSession::mouse(observed.product_id(), observed.physical_product_id(), peer)
    }
}

pub(super) fn resolve(
    observed: &ObservedDevice,
    session: &ReceiverSession,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DirectRoute> {
    let cap = session.transport();
    let Some(node) = observed.hid_node() else {
        #[cfg(windows)]
        {
            let mut target = windows::resolve_with(observed, cap, request)?;
            target.peer_product_id = observed.peer_product_id();
            return Ok(DirectRoute::Container(target));
        }
        #[cfg(not(windows))]
        anyhow::bail!("Receiver collection was not retained");
    };
    ensure!(
        node.vendor_id == cap.vendor_id
            && u32::from(node.product_id) == observed.physical_product_id()
            && node.interface_number == i32::from(cap.claim_interface),
        "Receiver collection differs from the source factory"
    );
    let nodes: Vec<HidNode> =
        serde_json::from_value(request(ServiceRequest::HidNodes)?["nodes"].clone())?;
    ensure!(
        nodes.iter().filter(|current| *current == node).count() == 1,
        "Receiver collection changed or is ambiguous"
    );
    super::source_reports_match_with(node, cap.report_bytes, request)?;
    Ok(DirectRoute::Collection {
        node: node.clone(),
        product_id: observed.product_id(),
    })
}

pub(super) fn check_peers(reply: &Value, session: &ReceiverSession) -> anyhow::Result<()> {
    for name in ["receiver_before", "receiver_after"] {
        let peers: Vec<(u16, u8)> = serde_json::from_value(reply[name].clone())
            .with_context(|| format!("Receiver setting is missing {name}"))?;
        session.validate_peer(&peers)?;
    }
    ensure!(
        reply["transport_source_class"] == session.transport().source_class,
        "Receiver response sender differs from the current source factory"
    );
    Ok(())
}

pub(super) fn keyboard(
    observed: &ObservedDevice,
    percent: Option<u8>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<KeyboardBrightness> {
    if let Some(percent) = percent {
        razer_device::keyboard_settings::encode_percent(percent)?;
    }
    let session = session(observed, true)?;
    let route = resolve(observed, &session, request)?;
    let peer = observed
        .peer_product_id()
        .context("Receiver peer is absent")?;
    let command = match (&route, percent) {
        (DirectRoute::Collection { node, product_id }, percent) => {
            ServiceRequest::HidNodeKeyboardReceiverBrightness {
                node: node.clone(),
                product_id: *product_id,
                peer_product_id: peer,
                percent,
            }
        }
        #[cfg(windows)]
        (DirectRoute::Container(target), None) => ServiceRequest::DeviceKeyboardBrightnessRead {
            target: target.clone(),
        },
        #[cfg(windows)]
        (DirectRoute::Container(target), Some(percent)) => {
            ServiceRequest::DeviceKeyboardBrightnessWrite {
                target: target.clone(),
                percent,
            }
        }
    };
    let reply = request(command)?;
    route.check_reply(&reply)?;
    ensure!(
        reply["peer_product_id"] == peer
            && reply["method"]
                == if percent.is_some() {
                    "setBrightness"
                } else {
                    "getBrightness"
                }
            && reply["source_class"]
                == razer_device::keyboard_settings::capability(observed.product_id())
                    .context("Keyboard brightness source is absent")?
                    .source_class,
        "Keyboard response belongs to another peer or command owner"
    );
    check_peers(&reply, &session)?;
    super::decode_keyboard_brightness_reply(&reply, percent)
}

pub(super) fn dpi_stages(
    observed: &ObservedDevice,
    draft: Option<DpiStagesDraft>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DpiStagesReading> {
    let session = session(observed, false)?;
    let route = resolve(observed, &session, request)?;
    let peer = observed
        .peer_product_id()
        .context("Receiver peer is absent")?;
    let command = match (&route, &draft) {
        (DirectRoute::Collection { node, product_id }, draft) => {
            ServiceRequest::HidNodeMouseReceiverDpiStages {
                node: node.clone(),
                product_id: *product_id,
                peer_product_id: peer,
                draft: draft.clone(),
            }
        }
        #[cfg(windows)]
        (DirectRoute::Container(target), None) => ServiceRequest::DeviceDpiStagesRead {
            target: target.clone(),
        },
        #[cfg(windows)]
        (DirectRoute::Container(target), Some(draft)) => ServiceRequest::DeviceDpiStagesWrite {
            target: target.clone(),
            draft: draft.clone(),
        },
    };
    let reply = request(command)?;
    route.check_reply(&reply)?;
    ensure!(
        reply["peer_product_id"] == peer
            && reply["method"]
                == if draft.is_some() {
                    "setDPIStages"
                } else {
                    "getDPIStages"
                },
        "DPI stage response belongs to another peer"
    );
    check_peers(&reply, &session)?;
    super::decode_dpi_stages_reply(
        &reply,
        mouse_dpi_stages::capability(observed.product_id())
            .context("DPI stage source is absent")?,
        draft.as_ref(),
    )
}

#[cfg(test)]
#[path = "receiver_settings_tests.rs"]
mod tests;
