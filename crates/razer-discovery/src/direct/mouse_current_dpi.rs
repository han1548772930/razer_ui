use super::*;
use razer_device::mouse_current_dpi::{self as dpi, DpiChange, DpiReport};

pub fn available(observed: &ObservedDevice) -> bool {
    if observed.peer_product_id().is_some() {
        return receiver_session(observed).is_ok();
    }
    dpi::capability(observed.product_id()).is_some_and(|source| match observed.transport() {
        Some(crate::discovery::ObservedTransport::Wired) => {
            source.is_physical_branch(observed.physical_product_id(), false)
                && observed.peer_product_id().is_none()
        }
        Some(crate::discovery::ObservedTransport::Dongle) => {
            source.is_physical_branch(observed.physical_product_id(), true)
                && observed
                    .peer_product_id()
                    .is_none_or(|peer| peer == source.product_id())
        }
        _ => false,
    })
}

fn receiver_session(
    observed: &ObservedDevice,
) -> anyhow::Result<razer_device::receiver_session::ReceiverSession> {
    ensure!(
        observed.transport() == Some(crate::discovery::ObservedTransport::Dongle)
            && observed.connection()
                == razer_model::model::DeviceConnectionObservation::ReceiverPeer(1),
        "Current DPI requires an online receiver observation"
    );
    dpi::capability(observed.product_id())
        .context("Current DPI source is absent")?
        .receiver_session(
            observed.physical_product_id(),
            observed
                .peer_product_id()
                .context("Receiver peer is absent")?,
        )
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    change: Option<DpiChange>,
) -> anyhow::Result<DpiReport> {
    request_with(observed, change, |request| client.request(request))
}

pub(super) fn request_with(
    observed: &ObservedDevice,
    change: Option<DpiChange>,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DpiReport> {
    ensure!(
        available(observed),
        "Current mouse DPI has no live source route"
    );
    if let Some(change) = &change {
        dpi::validate_change_for(observed.product_id(), change)?;
    }
    if observed.peer_product_id().is_some() {
        let session = receiver_session(observed)?;
        let target = super::receiver_settings::resolve(observed, &session, &mut request)?;
        let query = match &target {
            DirectRoute::Collection { node, product_id } => {
                ServiceRequest::HidNodeMouseCurrentDpi {
                    node: node.clone(),
                    product_id: *product_id,
                    peer_product_id: observed.peer_product_id(),
                    change: change.clone(),
                }
            }
            #[cfg(windows)]
            DirectRoute::Container(target) => ServiceRequest::DeviceMouseCurrentDpi {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = request(query)?;
        target.check_reply(&reply)?;
        ensure!(
            reply["peer_product_id"] == observed.peer_product_id().unwrap(),
            "DPI response belongs to another receiver peer"
        );
        super::receiver_settings::check_peers(&reply, &session)?;
        ensure!(
            reply["obm_persistence_verified"] == false,
            "Source Mt setter acknowledgements cannot claim OBM readback"
        );
        decode_reply(observed.product_id(), &reply, change)
    } else {
        resolve_with(observed, &mut request)?.mouse_current_dpi_with(change, &mut request)
    }
}

/// Admit only the actual wired/ordinary-dongle branch of each current factory.
/// The service verifies ordinary receivers online around the device operation.
pub fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<DirectRoute> {
    resolve_with(observed, &mut |request| client.request(request))
}

pub(super) fn resolve_with(
    observed: &ObservedDevice,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DirectRoute> {
    let source = dpi::capability(observed.product_id())
        .context("Mouse DPI observation has no current source factory capability")?;
    let cap = source.transport();
    ensure!(
        match observed.transport() {
            Some(crate::discovery::ObservedTransport::Wired) =>
                source.is_physical_branch(observed.physical_product_id(), false)
                    && observed.peer_product_id().is_none(),
            Some(crate::discovery::ObservedTransport::Dongle) =>
                source.is_physical_branch(observed.physical_product_id(), true)
                    && observed
                        .peer_product_id()
                        .is_none_or(|peer| peer == source.product_id()),
            _ => false,
        },
        "Mouse DPI observation differs from its source wired/dongle branches"
    );
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == observed.physical_product_id()
                && node.interface_number
                    == i32::from(cap.claim_interface_for(observed.physical_product_id())?),
            "Mouse DPI collection interface differs"
        );
        let current = request(ServiceRequest::HidNodes)?;
        let nodes: Vec<HidNode> = serde_json::from_value(current["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Current mouse DPI collection changed or is ambiguous"
        );
        source_reports_match_with(node, cap.report_bytes, request)?;
        return Ok(DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        });
    }
    #[cfg(windows)]
    {
        windows::resolve_with(observed, cap, request).map(DirectRoute::Container)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Current mouse DPI has no observed HID collection on this platform")
    }
}

impl DirectRoute {
    pub fn mouse_current_dpi(
        &self,
        client: &mut ServiceClient,
        change: Option<DpiChange>,
    ) -> anyhow::Result<DpiReport> {
        self.mouse_current_dpi_with(change, &mut |request| client.request(request))
    }

    fn mouse_current_dpi_with(
        &self,
        change: Option<DpiChange>,
        request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<DpiReport> {
        let product_id = match self {
            DirectRoute::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            DirectRoute::Container(target) => target.product_id,
        };
        if let Some(change) = &change {
            dpi::validate_change_for(product_id, change)?;
        }
        let query = match self {
            DirectRoute::Collection { node, product_id } => {
                ServiceRequest::HidNodeMouseCurrentDpi {
                    node: node.clone(),
                    product_id: *product_id,
                    peer_product_id: None,
                    change: change.clone(),
                }
            }
            #[cfg(windows)]
            DirectRoute::Container(target) => ServiceRequest::DeviceMouseCurrentDpi {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = request(query)?;
        self.check_reply(&reply)?;
        decode_reply(product_id, &reply, change)
    }
}

fn decode_reply(
    product_id: u32,
    reply: &Value,
    change: Option<DpiChange>,
) -> anyhow::Result<DpiReport> {
    let source = dpi::capability(product_id).context("DPI source capability is absent")?;
    ensure!(
        reply["source_class"] == source.transport().source_class
            && reply["profile_scope"] == "stage_1_active_0",
        "Current mouse DPI reply source or profile scope differs"
    );
    let report: DpiReport = serde_json::from_value(reply["report"].clone())?;
    ensure!(
        report.requested == change
            && (report.verified || change.is_none())
            && report
                .acknowledgements
                .iter()
                .all(|ack| ack.transaction < 31
                    && ack.response_bytes == ack.data.len()
                    && ack.response_bytes <= 80),
        "Current mouse DPI reply does not confirm its exact request"
    );
    if let Some(change) = &change {
        ensure!(
            report.read_errors.is_empty(),
            "DPI write reply contains failed getter observations"
        );
        if change.is_identity_read() {
            ensure!(
                product_id == 182
                    && report
                        .serial_number
                        .as_deref()
                        .is_some_and(|serial| !serial.is_empty()
                            && serial.bytes().all(|byte| (32..127).contains(&byte)))
                    && report.acknowledgements.is_empty()
                    && report.stages.is_none()
                    && report.stage_level.is_none()
                    && report.active.is_none()
                    && report.profile_slots.is_empty()
                    && report.profile_slot_writes.is_empty()
                    && report.initialization_mode_error.is_none(),
                "Initialization identity reply is absent or contains unrelated device operations"
            );
        }
        if let Some(stages) = &change.stages {
            let packed = dpi::pack_for(product_id, stages)?;
            if source.is_stage_level_only() {
                ensure!(
                    report
                        .acknowledgements
                        .iter()
                        .any(|ack| ack.command == [7, 4, 5]),
                    "DPI level-only stage reply has no profile setter acknowledgement"
                );
            } else {
                ensure!(
                    report
                        .stages
                        .as_ref()
                        .is_some_and(|reading| dpi::same_stages(&packed, reading)),
                    "Current mouse DPI reply stage readback differs"
                );
            }
        }
        if let Some(active) = change.active {
            ensure!(
                report.active == Some(active),
                "Current mouse DPI reply active readback differs"
            );
        }
    } else {
        let stages_present =
            report.stages.is_some() || source.is_stage_level_only() && report.stage_level.is_some();
        ensure!(
            stages_present != report.read_errors.stages().is_some()
                && report.active.is_some() != report.read_errors.active().is_some()
                && report.verified == report.read_errors.is_empty()
                && report.acknowledgements.is_empty()
                && report.profile_slots.is_empty()
                && report.profile_slot_writes.is_empty()
                && report.serial_number.is_none()
                && report.initialization_mode_error.is_none(),
            "Current mouse DPI read reply has absent or contradictory observations"
        );
        if let Some(stages) = &report.stages {
            ensure!(
                stages.profile_id == 1,
                "DPI stages belong to another profile"
            );
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn partial_getter_reply_preserves_real_values_and_requires_explicit_errors() {
        let source = dpi::capability(190).unwrap();
        let reply = json!({"source_class":source.transport().source_class,"profile_scope":"stage_1_active_0",
            "report":{"requested":null,"stages":null,"active":{"x":2000,"y":2100},
                "acknowledgements":[],"read_errors":{"stages":"stage command rejected","active":null},"verified":false}});
        let report = decode_reply(190, &reply, None).unwrap();
        assert_eq!(report.active, Some(dpi::DpiPair { x: 2000, y: 2100 }));
        assert!(!report.verified);
        for (path, value) in [
            ("/report/verified", json!(true)),
            ("/report/read_errors/stages", json!(null)),
            ("/report/read_errors/active", json!("also failed")),
            ("/report/active", json!(null)),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(decode_reply(190, &invalid, None).is_err());
        }
    }

    #[test]
    fn a_partial_getter_cannot_confirm_a_device_write_or_invent_obm_readback() {
        let source = dpi::capability(190).unwrap();
        let change = DpiChange {
            stages: None,
            active: Some(dpi::DpiPair { x: 2000, y: 2100 }),
            profile_guid: None,
            initialize: false,
        };
        let mut reply = json!({"source_class":source.transport().source_class,"profile_scope":"stage_1_active_0",
            "report":{"requested":change,"stages":null,"active":{"x":2000,"y":2100},
                "acknowledgements":[],"read_errors":{"stages":"failed","active":null},"verified":false}});
        assert!(decode_reply(190, &reply, Some(change.clone())).is_err());
        reply["report"]["verified"] = json!(true);
        assert!(decode_reply(190, &reply, Some(change)).is_err());
    }

    #[test]
    fn identity_reply_requires_a_real_serial_without_claiming_settings_were_written() {
        let change = DpiChange {
            stages: None,
            active: None,
            profile_guid: None,
            initialize: true,
        };
        let source = dpi::capability(182).unwrap();
        let reply = json!({"source_class":source.transport().source_class,"profile_scope":"stage_1_active_0",
            "report":{"requested":change,"serial_number":"SOURCE-SERIAL","stages":null,"active":null,"acknowledgements":[],"verified":true}});
        assert!(decode_reply(182, &reply, Some(change.clone())).is_ok());
        for serial in [
            Value::Null,
            json!(""),
            json!("bad\u{0}serial"),
            json!("未读取"),
        ] {
            let mut missing = reply.clone();
            missing["report"]["serial_number"] = serial;
            assert!(decode_reply(182, &missing, Some(change.clone())).is_err());
        }
        let mut unrelated = reply;
        unrelated["report"]["active"] = json!({"x":800,"y":800});
        assert!(decode_reply(182, &unrelated, Some(change)).is_err());
    }
}
