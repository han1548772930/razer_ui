//! Retain the discovered collection and raw wireless peer through portable IPC.
use super::*;
use crate::discovery::ObservedTransport;
use razer_device::mouse_receiver_polling::ReceiverRead;
use razer_model::model::DeviceConnectionObservation;

pub fn available(observed: &ObservedDevice) -> bool {
    observed.product_id() == 182
        && observed.peer_product_id() == Some(183)
        && matches!(observed.transport(), Some(ObservedTransport::Dongle))
        && observed.connection() == DeviceConnectionObservation::ReceiverPeer(1)
        && ReceiverRead::new(
            182,
            observed.physical_product_id(),
            Some(183),
            DeviceReadKind::Polling,
        )
        .is_ok()
}

pub fn write(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    hz: u32,
) -> anyhow::Result<razer_device::mouse_receiver_polling::PollingWriteResult> {
    write_with(observed, hz, |request| client.request(request))
}

pub fn write_idle(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    raw_time: u16,
) -> anyhow::Result<u16> {
    write_idle_with(observed, raw_time, |request| client.request(request))
}

fn write_idle_with(
    observed: &ObservedDevice,
    raw_time: u16,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<u16> {
    let setting = DeviceWriteSetting::Idle { raw_time };
    let setter = razer_device::device_writes::prepare(
        razer_device::device_writes::capability(182).context("PID182 idle setter is absent")?,
        &setting,
    )?;
    let (source, target) = resolve(observed, DeviceReadKind::Idle, &mut request)?;
    let command = match &target {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseReceiverIdle {
            node: node.clone(),
            product_id: *product_id,
            peer_product_id: 183,
            raw_time,
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceWrite {
            target: target.clone(),
            setting: setting.clone(),
        },
    };
    let reply = request(command)?;
    target.check_reply(&reply)?;
    ensure!(
        (!matches!(target, DirectRoute::Collection { .. }) || reply["peer_product_id"] == 183)
            && reply["source_class"] == source.transport().source_class
            && reply["method"] == "setTimeToSleep",
        "Idle response does not belong to its source receiver task"
    );
    let result: razer_device::device_writes::DeviceWriteResult =
        serde_json::from_value(reply["result"].clone())?;
    ensure!(
        result.requested == setting
            && result.previous.is_none()
            && result.changed
            && result.verified
            && result
                .observed
                .as_ref()
                .is_some_and(|value| setting.matches_value(value))
            && result
                .acknowledged
                .as_ref()
                .is_some_and(|ack| ack.command == setter.command && ack.data.len() <= 80),
        "Idle result does not confirm the setter and independent readback"
    );
    Ok(raw_time)
}

fn write_with(
    observed: &ObservedDevice,
    hz: u32,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<razer_device::mouse_receiver_polling::PollingWriteResult> {
    ensure!(
        available(observed),
        "Receiver polling has no live current-source mouse route"
    );
    let (source, target) = resolve(observed, DeviceReadKind::Polling, &mut request)?;
    let query = match &target {
        DirectRoute::Collection { node, product_id } => {
            ServiceRequest::HidNodeMouseReceiverPolling {
                node: node.clone(),
                product_id: *product_id,
                peer_product_id: 183,
                hz,
            }
        }
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceWrite {
            target: target.clone(),
            setting: DeviceWriteSetting::Polling { hz },
        },
    };
    let reply = request(query)?;
    target.check_reply(&reply)?;
    ensure!(
        (!matches!(target, DirectRoute::Collection { .. }) || reply["peer_product_id"] == 183)
            && reply["source_class"] == source.transport().source_class
            && reply["profile_scope"] == "current_active_profile"
            && reply["obm_profiles_written"] == false,
        "Polling write response source,peer or profile scope differs"
    );
    let result: razer_device::mouse_receiver_polling::PollingWriteResult =
        serde_json::from_value(reply["result"].clone())?;
    let effective = if source.method() == "getUSBHighSpeedPollingRate" {
        hz
    } else {
        hz.min(1000)
    };
    ensure!(
        result.requested_hz == hz
            && result.effective_hz == effective
            && result.observed_hz == effective
            && result.changed == (result.previous_hz != effective)
            && match (&result.acknowledgement, result.changed) {
                (Some(ack), true) => ack.len() <= 80,
                (None, false) => true,
                _ => false,
            },
        "Polling result does not confirm its source task/readback"
    );
    Ok(result)
}

pub(super) fn resolve(
    observed: &ObservedDevice,
    kind: DeviceReadKind,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<(ReceiverRead, DirectRoute)> {
    ensure!(
        matches!(observed.transport(), Some(ObservedTransport::Dongle))
            && observed.connection() == DeviceConnectionObservation::ReceiverPeer(1),
        "Mouse read requires a live receiver peer observation"
    );
    let peer = observed
        .peer_product_id()
        .context("Mouse receiver observation has no raw peer identity")?;
    let route = ReceiverRead::new(
        observed.product_id(),
        observed.physical_product_id(),
        Some(peer),
        kind,
    )?;
    let cap = route.transport();
    let node = match observed.hid_node() {
        Some(node) => node,
        None => {
            #[cfg(windows)]
            {
                let mut target = windows::resolve_with(observed, cap, request)?;
                target.peer_product_id = Some(peer);
                return Ok((route, DirectRoute::Container(target)));
            }
            #[cfg(not(windows))]
            anyhow::bail!("Portable mouse read has no retained receiver collection");
        }
    };
    ensure!(
        node.vendor_id == cap.vendor_id
            && u32::from(node.product_id) == observed.physical_product_id()
            && node.interface_number == i32::from(cap.claim_interface),
        "Mouse receiver collection differs from its current source factory"
    );
    let current = request(ServiceRequest::HidNodes)?;
    let nodes: Vec<HidNode> = serde_json::from_value(current["nodes"].clone())?;
    ensure!(
        nodes.iter().filter(|current| *current == node).count() == 1,
        "Mouse receiver collection changed or is ambiguous"
    );
    super::source_reports_match_with(node, cap.report_bytes, request)?;
    Ok((
        route,
        DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        },
    ))
}

pub(super) fn read(
    observed: &ObservedDevice,
    kind: DeviceReadKind,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DeviceReadValue> {
    let (route, target) = resolve(observed, kind, &mut request)?;
    let peer = observed
        .peer_product_id()
        .context("Missing retained mouse peer")?;
    let query = match &target {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseReceiverRead {
            node: node.clone(),
            product_id: *product_id,
            peer_product_id: peer,
            kind,
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceRead {
            target: target.clone(),
            kind,
        },
    };
    let reply = request(query)?;
    target.check_reply(&reply)?;
    ensure!(
        (!matches!(target, DirectRoute::Collection { .. }) || reply["peer_product_id"] == peer)
            && reply["method"] == route.method()
            && reply["source_class"] == route.transport().source_class,
        "Read response does not belong to the retained receiver and mouse peer"
    );
    let reading = serde_json::from_value(reply["reading"].clone())?;
    ensure!(
        matches!(
            (&reading, kind),
            (DeviceReadValue::Firmware { .. }, DeviceReadKind::Firmware)
                | (DeviceReadValue::Battery { .. }, DeviceReadKind::Battery)
                | (DeviceReadValue::Charging { .. }, DeviceReadKind::Charging)
                | (DeviceReadValue::Polling { .. }, DeviceReadKind::Polling)
                | (DeviceReadValue::Dpi { .. }, DeviceReadKind::Dpi)
                | (DeviceReadValue::Idle { .. }, DeviceReadKind::Idle)
        ),
        "Receiver response contains another observation kind"
    );
    Ok(reading)
}

pub(super) fn dpi_stages(
    observed: &ObservedDevice,
    draft: Option<DpiStagesDraft>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DpiStagesReading> {
    let (_, target) = resolve(observed, DeviceReadKind::Dpi, request)?;
    let peer = observed
        .peer_product_id()
        .context("Missing retained mouse peer")?;
    let query = match (&target, &draft) {
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
    let reply = request(query)?;
    target.check_reply(&reply)?;
    ensure!(
        (!matches!(target, DirectRoute::Collection { .. }) || reply["peer_product_id"] == peer)
            && reply["method"]
                == if draft.is_some() {
                    "setDPIStages"
                } else {
                    "getDPIStages"
                },
        "DPI stage response belongs to another receiver operation"
    );
    let settings = mouse_dpi_stages::capability(observed.product_id())
        .context("DPI stage source is absent")?;
    decode_dpi_stages_reply(&reply, settings, draft.as_ref())
}

#[cfg(test)]
#[path = "mouse_receiver_polling_tests.rs"]
mod tests;
