use super::*;
use razer_device::mouse_indicator_led::{Acknowledgement, Configuration};
use serde_json::json;

fn route() -> Route {
    Route::Collection {
        product_id: 190,
        node: HidNode {
            path: b"simulated-receiver-191".to_vec(),
            vendor_id: 0x1532,
            product_id: 191,
            interface_number: 0,
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        },
    }
}
fn response(route: &Route, change: Change) -> Value {
    let Route::Collection { node, product_id } = route else {
        panic!("simulated collection only")
    };
    let report = Report {
        requested: change,
        acknowledgements: vec![Acknowledgement {
            command: indicator::SET_CONFIGURATION,
            transaction: 0,
            data: vec![],
        }],
        configuration: None,
        raw_configuration: None,
        serial_number: None,
        battery_level: None,
        charging_status: None,
        quality: None,
        setter_attempted: true,
        setter_sent: true,
        setter_acknowledged: true,
        source_quality_update: None,
        error: None,
    };
    json!({"node":node,"product_id":product_id,"source_class":"rzDevice25",
        "setting_scope":"active_profile_receiver_indicator","report":report})
}
#[test]
fn exact_receiver_typed_request_and_partial_initialize_ack_are_retained() {
    let route = route();
    let change = Change::Set(Configuration::default());
    let report = execute_with(&route, change.clone(), &mut |request| {
        let ServiceRequest::HidNodeMouseIndicatorLed {
            product_id,
            node,
            change: requested,
        } = request
        else {
            panic!("wrong route")
        };
        assert_eq!(product_id, 190);
        assert_eq!(node.product_id, 191);
        assert_eq!(requested, change);
        Ok(response(&route, requested))
    })
    .unwrap();
    assert!(report.setter_acknowledged);
    let change = Change::Initialize(Configuration::default());
    let mut reply = response(&route, change.clone());
    reply["report"]["error"] = json!("quality getter failed");
    let report = execute_with(&route, change, &mut |_| Ok(reply.clone())).unwrap();
    assert!(report.setter_acknowledged && report.error.is_some());
}
#[test]
fn forged_reply_owner_command_and_quality_notification_are_rejected() {
    let route = route();
    let change = Change::Set(Configuration::default());
    for mode in 0..6 {
        let mut reply = response(&route, change.clone());
        match mode {
            0 => reply["node"]["path"] = json!([1]),
            1 => reply["report"]["source_quality_update"] = json!(4),
            2 => reply["report"]["acknowledgements"][0]["command"] = json!([3, 7, 149]),
            3 => reply["report"]["setter_sent"] = json!(false),
            4 => reply["setting_scope"] = json!("global"),
            _ => reply["report"]["acknowledgements"][0]["transaction"] = json!(31),
        }
        assert!(execute_with(&route, change.clone(), &mut |_| Ok(reply.clone())).is_err());
    }
}
#[test]
fn initialization_publishes_set_ack_before_quality_request_and_retains_later_failure() {
    use std::cell::Cell;
    let route = route();
    let posted = Cell::new(false);
    let mut count = 0;
    let report = initialize_with(
        &route,
        Configuration::default(),
        |set| {
            assert!(set.setter_acknowledged && set.quality.is_none());
            assert_eq!(set.acknowledgements.len(), 1);
            posted.set(true);
            Ok(())
        },
        &mut |request| {
            count += 1;
            let ServiceRequest::HidNodeMouseIndicatorLed { change, .. } = request else {
                panic!("wrong typed route")
            };
            if matches!(change, Change::Set(_)) {
                assert!(!posted.get());
                Ok(response(&route, change))
            } else {
                assert!(posted.get());
                assert_eq!(change, Change::ReadConnectionQuality);
                anyhow::bail!("simulated quality transport failure after UI post")
            }
        },
    )
    .unwrap();
    assert_eq!(count, 2);
    assert!(posted.get() && report.setter_acknowledged);
    assert!(report.error.unwrap().contains("after UI post"));
    assert_eq!(report.acknowledgements.len(), 1);
    assert!(report.source_quality_update.is_none());
}
fn read_response(route: &Route, change: Change, data: Option<Vec<u8>>) -> Value {
    let command = match change {
        Change::ReadBatteryLevel => indicator::GET_BATTERY_LEVEL,
        Change::ReadChargingStatus => indicator::GET_CHARGING_STATUS,
        Change::ReadSerialNumber => indicator::GET_SERIAL_NUMBER,
        _ => panic!("unhandled simulated read"),
    };
    let report = Report {
        requested: change.clone(),
        acknowledgements: data
            .as_ref()
            .map(|data| Acknowledgement {
                command,
                transaction: 0,
                data: data.clone(),
            })
            .into_iter()
            .collect(),
        configuration: None,
        raw_configuration: None,
        quality: None,
        serial_number: matches!(change, Change::ReadSerialNumber)
            .then(|| {
                data.as_ref().map(|data| {
                    data.iter()
                        .copied()
                        .filter(|b| (32..127).contains(b))
                        .map(char::from)
                        .collect()
                })
            })
            .flatten(),
        battery_level: matches!(change, Change::ReadBatteryLevel)
            .then(|| {
                data.as_ref()
                    .and_then(|v| v.get(1))
                    .map(|v| (u16::from(*v) * 100 / 255) as u8)
            })
            .flatten(),
        charging_status: matches!(change, Change::ReadChargingStatus)
            .then(|| {
                data.as_ref()
                    .and_then(|v| v.get(1))
                    .and_then(|v| indicator::charging_status(*v))
                    .map(str::to_owned)
            })
            .flatten(),
        setter_attempted: false,
        setter_sent: false,
        setter_acknowledged: false,
        source_quality_update: None,
        error: data
            .is_none()
            .then(|| "simulated source getter failure".into()),
    };
    let Route::Collection { node, product_id } = route else {
        panic!("collection fixture")
    };
    json!({"node":node,"product_id":product_id,"report":report,"source_class":"rzDevice25","setting_scope":"active_profile_receiver_indicator"})
}
#[test]
fn full_battery_query_uses_source_wired_and_wireless_failure_paths() {
    for (physical, charging_ok, expected, status) in [
        (
            191,
            true,
            vec![Change::ReadChargingStatus, Change::ReadBatteryLevel],
            "FastCharging",
        ),
        (
            191,
            false,
            vec![
                Change::ReadChargingStatus,
                Change::ReadBatteryLevel,
                Change::ReadBatteryLevel,
            ],
            "NoCharge_BatteryFull",
        ),
        (
            190,
            false,
            vec![Change::ReadChargingStatus, Change::ReadBatteryLevel],
            "Charging",
        ),
    ] {
        let mut route = route();
        let Route::Collection { node, .. } = &mut route else {
            panic!("collection fixture")
        };
        node.product_id = physical;
        let mut commands = vec![];
        let result = read_battery_with(&route, &|| Ok(()), &mut |request| {
            let ServiceRequest::HidNodeMouseIndicatorLed { change, .. } = request else {
                panic!("wrong route")
            };
            commands.push(change.clone());
            let data = if matches!(change, Change::ReadChargingStatus) {
                charging_ok.then(|| vec![0, 4])
            } else {
                Some(vec![0, 128])
            };
            Ok(read_response(&route, change, data))
        })
        .unwrap();
        assert_eq!(commands, expected);
        assert!(result.is_success);
        assert_eq!(result.level, Some(50));
        assert_eq!(result.charging_status.as_deref(), Some(status));
    }
}
#[test]
fn failed_battery_read_does_not_publish_a_success_or_continue_after_cancellation() {
    let route = route();
    let mut calls = 0;
    let result = read_battery_with(&route, &|| Ok(()), &mut |request| {
        calls += 1;
        let ServiceRequest::HidNodeMouseIndicatorLed { change, .. } = request else {
            panic!("wrong route")
        };
        Ok(read_response(&route, change, None))
    })
    .unwrap();
    assert_eq!(calls, 2);
    assert!(!result.is_success && result.level.is_none());
    assert_eq!(result.charging_status.as_deref(), Some("off"));
    let stopped = std::cell::Cell::new(false);
    let mut calls = 0;
    assert!(
        read_battery_with(
            &route,
            &|| {
                ensure!(!stopped.get(), "canceled");
                Ok(())
            },
            &mut |request| {
                calls += 1;
                stopped.set(true);
                let ServiceRequest::HidNodeMouseIndicatorLed { change, .. } = request else {
                    panic!("wrong route")
                };
                Ok(read_response(&route, change, Some(vec![0, 1])))
            }
        )
        .is_err()
    );
    assert_eq!(calls, 1);
}
fn source_battery_reply(
    change: Change,
    success: bool,
    data: Option<Vec<u8>>,
) -> BatterySourceReply {
    let report =
        serde_json::from_value(read_response(&route(), change, data)["report"].clone()).unwrap();
    BatterySourceReply::Returned { success, report }
}
#[test]
fn session_counter_preserves_fifth_return_boundary_and_setup_does_not_increment() {
    let mut session = BatterySession::default();
    let mut query = |change| Ok(source_battery_reply(change, false, None));
    let setup =
        read_battery_source_with(&mut session, false, true, &|| Ok(()), &mut query).unwrap();
    assert!(!setup.is_success);
    assert_eq!(session.source_counter(), 0);
    for attempt in 1..=6 {
        let result =
            read_battery_source_with(&mut session, true, true, &|| Ok(()), &mut query).unwrap();
        assert_eq!(session.source_counter(), attempt);
        assert_eq!(result.is_success, attempt == 5);
        assert_eq!(
            result.charging_status.as_deref(),
            Some("NoCharge_BatteryFull")
        );
        assert_eq!(result.source_level, -1);
        assert_eq!(result.level, None);
        assert_eq!(result.queries.len(), 1);
    }
    let success = read_battery_source_with(&mut session, true, true, &|| Ok(()), &mut |change| {
        let data = if matches!(change, Change::ReadChargingStatus) {
            vec![0, 1]
        } else {
            vec![0, 255]
        };
        Ok(source_battery_reply(change, true, Some(data)))
    })
    .unwrap();
    assert!(success.is_success);
    assert_eq!(success.level, Some(100));
    assert_eq!(session.source_counter(), 0);
}
#[test]
fn fifth_thrown_query_and_preliminary_non_success_never_get_return_fallback() {
    for preliminary_returned_failure in [false, true] {
        let mut session = BatterySession { source_counter: 4 };
        let mut queries = 0;
        let result =
            read_battery_source_with(&mut session, true, true, &|| Ok(()), &mut |change| {
                queries += 1;
                if preliminary_returned_failure && matches!(change, Change::ReadBatteryLevel) {
                    Ok(source_battery_reply(change, false, None))
                } else {
                    Ok(BatterySourceReply::Thrown {
                        report: None,
                        message: "source protocol error".into(),
                    })
                }
            })
            .unwrap();
        assert_eq!(queries, 2);
        assert_eq!(session.source_counter(), 5);
        assert!(!result.is_success);
        assert_eq!(result.source_level, -1);
        assert_eq!(result.charging_status.as_deref(), Some("off"));
    }
}
#[test]
fn final_returned_failure_preserves_charging_value_and_only_exact_fifth_is_success() {
    let mut session = BatterySession { source_counter: 4 };
    let mut commands = vec![];
    let result = read_battery_source_with(&mut session, true, true, &|| Ok(()), &mut |change| {
        commands.push(change.clone());
        Ok(if matches!(change, Change::ReadChargingStatus) {
            source_battery_reply(change, true, Some(vec![0, 4]))
        } else {
            source_battery_reply(change, false, None)
        })
    })
    .unwrap();
    assert_eq!(
        commands,
        [Change::ReadChargingStatus, Change::ReadBatteryLevel]
    );
    assert!(result.is_success);
    assert_eq!(result.charging_status.as_deref(), Some("FastCharging"));
    assert_eq!(result.source_level, -1);
    assert_eq!(session.source_counter(), 5);
}
#[test]
fn actual_protocol25_failures_remain_thrown_for_all_session_attempts() {
    let route = route();
    let mut session = BatterySession::default();
    for attempt in 1..=6 {
        let result =
            read_battery_session_with(&route, &mut session, true, &|| Ok(()), &mut |request| {
                let ServiceRequest::HidNodeMouseIndicatorLed { change, .. } = request else {
                    panic!("typed receiver read")
                };
                Ok(read_response(&route, change, None))
            })
            .unwrap();
        assert_eq!(session.source_counter(), attempt);
        assert!(!result.is_success);
        assert_eq!(result.charging_status.as_deref(), Some("off"));
    }
}
#[test]
fn current_source_pair_collection_and_feature_descriptor_are_all_revalidated() {
    let Route::Collection { node, .. } = route() else {
        panic!("simulated collection only")
    };
    for (product, physical, interface) in [
        (190, 190, 0),
        (190, 191, 0),
        (229, 229, 3),
        (229, 230, 3),
        (239, 239, 3),
        (239, 240, 3),
    ] {
        let mut node = node.clone();
        node.product_id = physical;
        node.interface_number = interface;
        let result = resolve_collection(product, u32::from(physical), &node, &mut |request| {
            Ok(match request {
                ServiceRequest::HidNodes => json!({"nodes":[node]}),
                ServiceRequest::HidNodeReports { node: requested } => {
                    assert_eq!(requested, node);
                    // Source host uses the collection maximum; report zero
                    // does not need its own descriptor entry.
                    json!({"node":node,"reports":{"feature":{"3":91}}})
                }
                _ => panic!("identity resolution must not submit a device command"),
            })
        });
        assert!(result.is_ok());
    }
    for mode in 0..5 {
        let mut calls = 0;
        assert!(
            resolve_collection(190, 191, &node, &mut |request| {
                calls += 1;
                Ok(match request {
                    ServiceRequest::HidNodes => match mode {
                        0 => json!({"nodes":[]}),
                        1 => json!({"nodes":[node,node]}),
                        _ => json!({"nodes":[node]}),
                    },
                    ServiceRequest::HidNodeReports { .. } => {
                        let mut value = json!({"node":node,"reports":{"feature":{"0":91}}});
                        match mode {
                            2 => value["node"]["path"] = json!([0]),
                            3 => value["reports"]["feature"]["0"] = json!(90),
                            _ => value["reports"]["feature"] = json!({"1":92}),
                        };
                        value
                    }
                    _ => panic!("stale identity must not send"),
                })
            })
            .is_err()
        );
        assert_eq!(calls, if mode < 2 { 1 } else { 2 });
    }
    let mut invalid = node.clone();
    invalid.interface_number = 3;
    assert!(
        resolve_collection(190, 191, &invalid, &mut |_| panic!(
            "wrong interface must not query"
        ))
        .is_err()
    );
    assert!(
        resolve_collection(190, 230, &node, &mut |_| panic!(
            "wrong source pair must not query"
        ))
        .is_err()
    );
}
