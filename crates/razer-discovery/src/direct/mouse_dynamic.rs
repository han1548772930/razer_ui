//! The dynamic editor shares the verified physical mouse factory and receiver
//! session with DPI; command and response semantics remain dynamic-owned.
use super::*;
use razer_device::mouse_dynamic::{self as dynamic, Report, Setting};

pub fn available(observed: &ObservedDevice) -> bool {
    dynamic::capability(observed.product_id()).is_some()
        && super::mouse_current_dpi::available(observed)
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    change: Option<Setting>,
) -> anyhow::Result<Report> {
    request_with(observed, change, &mut |request| client.request(request))
}

fn request_with(
    observed: &ObservedDevice,
    change: Option<Setting>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Report> {
    ensure!(
        available(observed),
        "Dynamic sensitivity has no current wired or receiver route"
    );
    if let Some(change) = &change {
        change.validate()?;
    }
    let session = observed
        .peer_product_id()
        .map(|peer| {
            dynamic::capability(observed.product_id())
                .context("Dynamic factory is absent")?
                .receiver_session(observed.physical_product_id(), peer)
        })
        .transpose()?;
    let target = if let Some(session) = &session {
        super::receiver_settings::resolve(observed, session, request)?
    } else {
        super::mouse_current_dpi::resolve_with(observed, request)?
    };
    let query = match &target {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseDynamic {
            node: node.clone(),
            product_id: *product_id,
            peer_product_id: observed.peer_product_id(),
            change: change.clone(),
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceMouseDynamic {
            target: target.clone(),
            change: change.clone(),
        },
    };
    let reply = request(query)?;
    target.check_reply(&reply)?;
    if let Some(session) = &session {
        ensure!(
            reply["peer_product_id"] == observed.peer_product_id().unwrap(),
            "Dynamic reply belongs to another receiver peer"
        );
        super::receiver_settings::check_peers(&reply, session)?;
    }
    decode(observed.product_id(), &reply, change.as_ref())
}

fn decode(product_id: u32, reply: &Value, change: Option<&Setting>) -> anyhow::Result<Report> {
    let source = dynamic::capability(product_id).context("Dynamic source is absent")?;
    ensure!(
        reply["source_class"] == source.transport().source_class
            && reply["profile_scope"] == "dynamic_profile_1",
        "Dynamic reply source or profile scope differs"
    );
    let report: Report = serde_json::from_value(
        reply
            .get("report")
            .context("Dynamic reply has no typed report")?
            .clone(),
    )?;
    report.validate_reply(change)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn mouse_dynamic_reply_requires_real_getters_and_exact_request() {
        let reply = json!({"source_class":"rzDevice25","profile_scope":"dynamic_profile_1",
            "report":{"requested":null,"writes":null,"reading":{"raw_state":1,"raw_mode":2,"custom_sensor_accelerations":null},"read_failure":null,"write_failure":null}});
        assert_eq!(
            decode(190, &reply, None).unwrap().reading.unwrap().raw_mode,
            2
        );
        for (path, value) in [
            ("/report/reading", Value::Null),
            ("/profile_scope", json!("other")),
            ("/report/read_failure", json!("failed")),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(decode(190, &invalid, None).is_err());
        }
        assert!(decode(70, &reply, None).is_err());
    }
}
