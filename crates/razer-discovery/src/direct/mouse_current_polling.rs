//! PID190 ordinary wired/receiver polling tasks, including observed OBM slots.
use super::*;
use razer_device::mouse_current_polling::{self as polling, Change, Report};

pub fn available(observed: &ObservedDevice) -> bool {
    observed.product_id() == polling::PRODUCT_ID
        && observed.peer_product_id().is_none()
        && super::mouse_current_dpi::available(observed)
}

pub fn request(client: &mut ServiceClient, observed: &ObservedDevice, change: &Change) -> anyhow::Result<Report> {
    request_with(observed, change, |request| client.request(request))
}

fn request_with(observed: &ObservedDevice, change: &Change, mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>) -> anyhow::Result<Report> {
    ensure!(available(observed), "Polling target has no current ordinary mouse route");
    change.validate()?;
    let route = super::mouse_current_dpi::resolve_with(observed, &mut request)?;
    let query = match &route {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseCurrentPolling { node: node.clone(), product_id: *product_id, change: change.clone() },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceMouseCurrentPolling { target: target.clone(), change: change.clone() },
    };
    let reply = request(query)?;
    route.check_reply(&reply)?;
    decode_reply(&reply, change)
}

fn decode_reply(reply: &Value, change: &Change) -> anyhow::Result<Report> {
    ensure!(reply["source_class"] == "rzDevice25"
        && reply["profile_scope"] == "current_profile_1"
        && reply["obm_profiles_written"] == false,
        "Polling reply source, scope or OBM readback claim differs");
    let report: Report = serde_json::from_value(reply["report"].clone())?;
    ensure!(report.change() == change && report.runtime().verified
        && report.runtime().requested == razer_device::device_writes::DeviceWriteSetting::Polling { hz: change.hz() }
        && report.runtime().observed.as_ref().is_some_and(|value| report.runtime().requested.matches_value(value)),
        "Polling runtime reply does not independently confirm the exact request");
    let expected: Vec<_> = report.profile_slots().iter()
        .filter(|slot| Some(slot.guid.as_str()) == change.profile_guid())
        .map(|slot| slot.profile_id).collect();
    ensure!(change.profile_guid().is_some() || report.profile_slots().is_empty(), "Temporary polling task unexpectedly accessed OBM");
    ensure!(expected == report.slot_writes().iter().map(|slot| slot.profile_id()).collect::<Vec<_>>()
        && report.slot_writes().iter().all(|slot| {
            if slot.previous_hz() == Some(change.hz()) { slot.acknowledgment().is_none() }
            else { slot.acknowledgment().is_some_and(|ack| ack.command == [2, 0, 0x40] && ack.data.len() <= 80) }
        }), "Polling OBM reply does not confirm each matching observed slot");
    Ok(report)
}
