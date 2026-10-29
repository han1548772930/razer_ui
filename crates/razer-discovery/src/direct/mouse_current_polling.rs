//! PID190 ordinary wired/receiver polling tasks, including observed OBM slots.
use super::*;
use razer_device::mouse_current_polling::{self as polling, Change, Observation, Report};

pub fn available(observed: &ObservedDevice) -> bool {
    observed.product_id() == polling::PRODUCT_ID
        && observed.peer_product_id().is_none()
        && super::mouse_current_dpi::feature_available(observed)
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    change: &Change,
) -> anyhow::Result<Report> {
    request_with(observed, change, |request| client.request(request))
}

fn request_with(
    observed: &ObservedDevice,
    change: &Change,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Report> {
    change.validate()?;
    let reply = exchange_with(observed, Some(change), &mut request)?;
    decode_reply(&reply, change)
}

pub fn read(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Observation> {
    let reply = exchange_with(observed, None, &mut |request| client.request(request))?;
    decode_read_reply(&reply)
}

fn exchange_with(
    observed: &ObservedDevice,
    change: Option<&Change>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Value> {
    ensure!(
        available(observed),
        "Polling target has no current ordinary mouse route"
    );
    let route = super::mouse_current_dpi::resolve_with(observed, request)?;
    let query = match &route {
        DirectRoute::Collection { node, product_id } => {
            ServiceRequest::HidNodeMouseCurrentPolling {
                node: node.clone(),
                product_id: *product_id,
                change: change.cloned(),
            }
        }
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceMouseCurrentPolling {
            target: target.clone(),
            change: change.cloned(),
        },
    };
    let reply = request(query)?;
    route.check_reply(&reply)?;
    Ok(reply)
}

fn decode_read_reply(reply: &Value) -> anyhow::Result<Observation> {
    ensure!(
        reply["source_class"] == "rzDevice25"
            && reply["method"] == "getUSBHighSpeedPollingRate"
            && reply["profile_scope"] == "current_profile_1"
            && reply["obm_profiles_written"] == false
            && reply["report"].is_null(),
        "Polling GET reply source or scope differs"
    );
    let reading: Observation = serde_json::from_value(reply["reading"].clone())?;
    if let Some(hz) = reading.hz() {
        polling::rate_code(hz)?;
    }
    Ok(reading)
}

fn decode_reply(reply: &Value, change: &Change) -> anyhow::Result<Report> {
    ensure!(
        reply["source_class"] == "rzDevice25"
            && reply["method"] == "setUSBHighSpeedPollingRate"
            && reply["profile_scope"] == "current_profile_1"
            && reply["obm_profiles_written"] == false,
        "Polling reply source, scope or OBM readback claim differs"
    );
    let report: Report = serde_json::from_value(reply["report"].clone())?;
    ensure!(
        report.change() == change
            && report.runtime().verified
            && report.runtime().requested
                == razer_device::device_writes::DeviceWriteSetting::Polling { hz: change.hz() }
            && report
                .runtime()
                .observed
                .as_ref()
                .is_some_and(|value| report.runtime().requested.matches_value(value)),
        "Polling runtime reply does not independently confirm the exact request"
    );
    let expected: Vec<_> = report
        .profile_slots()
        .iter()
        .filter(|slot| Some(slot.guid.as_str()) == change.profile_guid())
        .map(|slot| slot.profile_id)
        .collect();
    ensure!(
        change.profile_guid().is_some() || report.profile_slots().is_empty(),
        "Temporary polling task unexpectedly accessed OBM"
    );
    ensure!(
        expected
            == report
                .slot_writes()
                .iter()
                .map(|slot| slot.profile_id())
                .collect::<Vec<_>>()
            && report.slot_writes().iter().all(|slot| {
                if slot.previous_hz() == Some(change.hz()) {
                    slot.acknowledgment().is_none()
                } else {
                    slot.acknowledgment()
                        .is_some_and(|ack| ack.command == [2, 0, 0x40] && ack.data.len() <= 80)
                }
            }),
        "Polling OBM reply does not confirm each matching observed slot"
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reply(guid: Option<&str>) -> Value {
        json!({"source_class":"rzDevice25","method":"setUSBHighSpeedPollingRate","profile_scope":"current_profile_1","obm_profiles_written":false,
            "report":{"change":{"hz":8000,"profile_guid":guid},"runtime":{
                "requested":{"kind":"polling","hz":8000},"previous":{"kind":"polling","hz":1000},
                "observed":{"kind":"polling","hz":8000},"acknowledged":{"command":[2,0,64],"data":[]},"changed":true,"verified":true
            },"profile_slots":[],"slot_writes":[]}})
    }

    #[test]
    fn read_reply_preserves_observation_and_unknown_without_a_write_report() {
        let mut reply = json!({"source_class":"rzDevice25","method":"getUSBHighSpeedPollingRate",
            "profile_scope":"current_profile_1","obm_profiles_written":false,"report":null,
            "reading":{"kind":"rate","hz":2000}});
        assert_eq!(
            decode_read_reply(&reply).unwrap(),
            Observation::Rate { hz: 2000 }
        );
        reply["reading"] = json!({"kind":"unknown","code":127});
        assert_eq!(
            decode_read_reply(&reply).unwrap(),
            Observation::Unknown { code: 127 }
        );
        reply["reading"] = json!({"kind":"rate","hz":333});
        assert!(decode_read_reply(&reply).is_err());
        reply["reading"] = json!({"kind":"rate","hz":1000});
        reply["method"] = json!("setUSBHighSpeedPollingRate");
        assert!(decode_read_reply(&reply).is_err());
    }

    #[test]
    fn runtime_reply_requires_independent_observation_and_exact_request() {
        let change = Change::new(8000);
        assert!(decode_reply(&reply(None), &change).is_ok());
        for (field, value) in [
            ("observed", Value::Null),
            ("observed", json!({"kind":"polling","hz":1000})),
            ("verified", json!(false)),
        ] {
            let mut invalid = reply(None);
            invalid["report"]["runtime"][field] = value;
            assert!(decode_reply(&invalid, &change).is_err());
        }
        assert!(decode_reply(&reply(Some("other")), &change).is_err());
    }

    #[test]
    fn profile_reply_preserves_actual_slots_and_rejects_fabricated_obm_readback() {
        let change = Change::new(8000).with_profile_guid("active".into());
        let mut valid = reply(Some("active"));
        valid["report"]["profile_slots"] =
            json!([{"profile_id":2,"guid":"other"},{"profile_id":4,"guid":"active"}]);
        valid["report"]["slot_writes"] = json!([{"profile_id":4,"previous_hz":1000,"acknowledgment":{"command":[2,0,64],"data":[77]}}]);
        assert!(decode_reply(&valid, &change).is_ok());
        let mut wrong_slot = valid.clone();
        wrong_slot["report"]["slot_writes"][0]["profile_id"] = json!(2);
        assert!(decode_reply(&wrong_slot, &change).is_err());
        let mut no_ack = valid.clone();
        no_ack["report"]["slot_writes"][0]["acknowledgment"] = Value::Null;
        assert!(decode_reply(&no_ack, &change).is_err());
        valid["obm_profiles_written"] = json!(true);
        assert!(decode_reply(&valid, &change).is_err());
    }
}
