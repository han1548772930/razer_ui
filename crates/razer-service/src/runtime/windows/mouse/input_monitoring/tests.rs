use super::*;
fn identity() -> identity::Identity {
    identity::Identity {
        container_id: "{observed-container}".into(),
        vendor_id: 5426,
        product_id: 70,
        usb_instance_id: Some("USB\\actual".into()),
    }
}
fn sample(path: &str, tick: u64, x: i32, y: i32, flags: u16) -> Sample {
    Sample {
        path: path.into(),
        x,
        y,
        button_flags: flags,
        button_data: 0,
        modifiers: 0x200,
        time_tick: tick,
    }
}
fn rows(event: &InputEvent) -> Vec<Value> {
    serde_json::from_str(event.json_event()).unwrap()
}
#[test]
fn source_flush_is_strictly_after_100000_microseconds_and_button_flushes_movement_first() {
    let mut state = State::default();
    state
        .sample(sample("HID/a", 1000, 3, 4, 0), |_| Ok(identity()))
        .unwrap();
    state
        .sample(sample("HID/a", 101000, 5, 6, 0), |_| {
            panic!("identity must be retained")
        })
        .unwrap();
    assert!(state.pending.is_empty());
    state
        .sample(sample("HID/a", 101001, -1, -2, 0), |_| unreachable!())
        .unwrap();
    let event = state.pending.pop_front().unwrap();
    let values = rows(&event);
    assert_eq!(values.len(), 3);
    assert_eq!(values[0]["timeTick"], "1000");
    assert_eq!(
        serde_json::from_str::<Value>(values[2]["data"].as_str().unwrap()).unwrap(),
        json!({"type":"mouseMove","x":-1,"y":-2})
    );
    state
        .sample(sample("HID/a", 101002, 7, 8, 1), |_| unreachable!())
        .unwrap();
    let values = rows(&state.pending.pop_front().unwrap());
    assert_eq!(values.len(), 2);
    assert_eq!(
        serde_json::from_str::<Value>(values[0]["data"].as_str().unwrap()).unwrap()["type"],
        "mouseMove"
    );
    assert_eq!(
        serde_json::from_str::<Value>(values[1]["data"].as_str().unwrap()).unwrap(),
        json!({"type":"mouse","id":1,"data":0,"extraInformation":0,"modifiers":512})
    );
}
#[test]
fn device_removal_flushes_only_that_identity_and_reconnect_observes_a_new_identity() {
    let mut state = State::default();
    state
        .sample(sample("HID/a", 1, 1, 0, 0), |_| Ok(identity()))
        .unwrap();
    state
        .sample(sample("HID/b", 2, 2, 0, 0), |_| {
            let mut id = identity();
            id.product_id = 71;
            Ok(id)
        })
        .unwrap();
    state.removed("hid/A").unwrap();
    assert_eq!(state.pending.len(), 1);
    assert_eq!(state.pending[0].product_id(), 70);
    assert!(state.devices.contains_key("hid/b"));
    state
        .sample(sample("HID/a", 3, 3, 0, 1), |_| {
            let mut id = identity();
            id.product_id = 72;
            Ok(id)
        })
        .unwrap();
    assert_eq!(state.pending[1].product_id(), 72);
}
#[test]
fn failed_identity_does_not_create_an_empty_or_default_device_batch() {
    let mut state = State::default();
    assert!(
        state
            .sample(sample("HID/a", 1, 2, 3, 0), |_| anyhow::bail!(
                "identity changed"
            ))
            .is_err()
    );
    assert!(state.devices.is_empty());
    assert!(state.pending.is_empty());
}

fn connected() -> (State, razer_ipc::mouse_input_monitoring::Session) {
    let mut state = State::default();
    let mut session = razer_ipc::mouse_input_monitoring::Session::new("current".into()).unwrap();
    session
        .start(|request| {
            let razer_ipc::ServiceRequest::MouseInputMonitoringStart { owner_guid } = request
            else {
                panic!("unexpected start request");
            };
            Ok(serde_json::to_value(
                state.start_with(owner_guid, |_| Ok(()))?,
            )?)
        })
        .unwrap();
    (state, session)
}

#[test]
fn failed_start_does_not_reserve_an_owner_or_report_success() {
    let mut state = State::default();
    assert!(
        state
            .start_with("failed".into(), |_| anyhow::bail!(
                "raw input registration failed"
            ))
            .is_err()
    );
    assert!(state.owner().is_none());
    state.start_with("retry".into(), |_| Ok(())).unwrap();
    assert_eq!(state.owner().as_deref(), Some("retry"));
}

#[test]
fn repeated_start_returns_native_negative_ack_without_restarting_or_losing_callbacks() {
    let (mut state, mut session) = connected();
    state
        .sample(sample("HID/a", 1000, 3, 4, 0), |_| Ok(identity()))
        .unwrap();
    let acknowledgement = session
        .repeat_start(|request| {
            let razer_ipc::ServiceRequest::MouseInputMonitoringStart { owner_guid } = request
            else {
                panic!("Repeat must send a real START request");
            };
            Ok(serde_json::to_value(
                state.start_with(owner_guid, |_| panic!("CB100 does not restart"))?,
            )?)
        })
        .unwrap();
    assert!(!acknowledgement.result());
    assert_eq!(acknowledgement.reason(), "already started");
    assert_eq!(state.owner().as_deref(), Some("current"));
    assert_eq!(state.devices["hid/a"].rows.len(), 1);
    assert_eq!(state.devices["hid/a"].last_publication, Some(1000));
    let mut observed = Vec::new();
    session
        .stop_with(
            |_| {
                Ok(serde_json::to_value(state.stop_with(
                    "current",
                    |_| Ok(vec![]),
                    |_| unreachable!(),
                )?)?)
            },
            |events| {
                observed.extend(events);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(observed.len(), 1);
    assert_eq!(rows(&observed[0])[0]["timeTick"], "1000");
    assert!(state.owner().is_none());
}

#[test]
fn repeat_cannot_take_another_owner_or_resume_a_failed_stop() {
    let (mut state, _) = connected();
    assert!(
        state
            .start_with("another".into(), |_| panic!("Cannot take the active owner"))
            .is_err()
    );
    assert_eq!(state.owner().as_deref(), Some("current"));
    assert!(
        state
            .stop_with(
                "current",
                |_| anyhow::bail!("native join failed"),
                |_| unreachable!()
            )
            .is_err()
    );
    assert!(
        state
            .start_with("current".into(), |_| panic!("Stop has already begun"))
            .is_err()
    );
    assert!(
        state
            .start_with("another".into(), |_| panic!(
                "Stop still owns its resources"
            ))
            .is_err()
    );
    assert!(state.poll("current").is_err());
    state
        .stop_with("current", |_| Ok(vec![]), |_| unreachable!())
        .unwrap();
    state.start_with("successor".into(), |_| Ok(())).unwrap();
    assert_eq!(state.owner().as_deref(), Some("successor"));
}

#[test]
fn stop_delivers_queued_removed_reconnected_and_final_device_callbacks_in_order() {
    let (mut state, mut session) = connected();
    state
        .sample(sample("HID/a", 1000, 3, 4, 1), |_| Ok(identity()))
        .unwrap();
    state
        .sample(sample("HID/a", 1001, 8, 9, 0), |_| unreachable!())
        .unwrap();
    state
        .sample(sample("HID/b", 1002, 5, 6, 0), |_| {
            let mut id = identity();
            id.product_id = 71;
            Ok(id)
        })
        .unwrap();
    let mut last = Some(vec![
        Event::Input(sample("HID/a", 1003, 9, 10, 0)),
        Event::Removed("HID/b".into()),
        Event::Input(sample("HID/b", 1004, 7, 8, 1)),
    ]);
    let mut observed = Vec::new();
    session
        .stop_with(
            |_| {
                Ok(serde_json::to_value(state.stop_with(
                    "current",
                    |_| Ok(last.take().expect("native STOP must only run once")),
                    |path| {
                        assert_eq!(path, "HID/b");
                        let mut id = identity();
                        id.product_id = 72;
                        Ok(id)
                    },
                )?)?)
            },
            |events| {
                observed.extend(events);
                Ok(())
            },
        )
        .unwrap();
    assert!(last.is_none());
    assert_eq!(
        observed
            .iter()
            .map(InputEvent::product_id)
            .collect::<Vec<_>>(),
        [70, 71, 72, 70]
    );
    let ticks = observed
        .iter()
        .map(|event| {
            rows(event)
                .into_iter()
                .map(|row| row["timeTick"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ticks,
        [
            vec!["1000", "1000"],
            vec!["1002"],
            vec!["1004", "1004"],
            vec!["1001", "1003"]
        ]
    );
    assert!(!session.possibly_started());
    assert!(state.owner().is_none());
    assert!(state.pending.is_empty() && state.devices.is_empty());
}

#[test]
fn stop_pages_reserve_the_same_owner_and_never_stop_the_native_thread_twice() {
    let (mut state, _) = connected();
    let last = (0..129)
        .map(|tick| Event::Input(sample("HID/a", 1000 + tick, 1, 1, 1)))
        .collect();
    let first = state
        .stop_with("current", |_| Ok(last), |_| Ok(identity()))
        .unwrap();
    assert!(first.has_more());
    assert_eq!(
        serde_json::to_value(first).unwrap()["events"]
            .as_array()
            .unwrap()
            .len(),
        128
    );
    assert_eq!(state.owner().as_deref(), Some("current"));
    assert!(
        state
            .start_with("replacement".into(), |_| panic!("owner is still reserved"))
            .is_err()
    );
    assert!(
        state
            .stop_with("old", |_| panic!("foreign owner"), |_| unreachable!())
            .is_err()
    );
    assert!(state.poll("current").is_err());
    let last = state
        .stop_with(
            "current",
            |_| panic!("a continuation only drains callbacks"),
            |_| unreachable!(),
        )
        .unwrap();
    assert!(!last.has_more());
    assert_eq!(
        serde_json::to_value(last).unwrap()["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(state.owner().is_none());
    state.start_with("replacement".into(), |_| Ok(())).unwrap();
}

#[test]
fn stop_pages_respect_encoded_frame_capacity_without_truncating_callbacks() {
    let (mut state, mut session) = connected();
    let body = format!("[{}]", " ".repeat(900_000));
    let event = InputEvent::from_native(
        r#"{"containerId":"physical","productId":226,"vendorId":5426}"#,
        1,
        body.clone(),
    )
    .unwrap();
    for _ in 0..4 {
        state.publish(Some(event.clone())).unwrap();
    }
    let mut requests = 0;
    let mut counts = Vec::new();
    session
        .stop_with(
            |_| {
                requests += 1;
                let page = state.stop_with(
                    "current",
                    |_| {
                        assert_eq!(requests, 1);
                        Ok(vec![])
                    },
                    |_| unreachable!(),
                )?;
                assert!(serde_json::to_vec(&page)?.len() < razer_ipc::MAX_FRAME_BYTES);
                Ok(serde_json::to_value(page)?)
            },
            |events| {
                for event in &events {
                    assert_eq!(event.json_event(), body);
                }
                counts.push(events.len());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(requests, 2);
    assert_eq!(counts, [3, 1]);
    assert!(state.owner().is_none());
}

#[test]
fn stop_streams_a_full_publication_queue_before_accepting_final_raw_input() {
    let (mut state, mut session) = connected();
    for tick in 0..512 {
        state
            .sample(sample("HID/a", tick, 1, 1, 1), |_| Ok(identity()))
            .unwrap();
    }
    assert_eq!(state.pending.len(), 512);
    let mut last = Some(
        (10_000..10_130)
            .map(|tick| Event::Input(sample("HID/a", tick, 1, 1, 1)))
            .collect(),
    );
    let mut requests = 0;
    let mut ticks = Vec::new();
    session
        .stop_with(
            |_| {
                requests += 1;
                Ok(serde_json::to_value(state.stop_with(
                    "current",
                    |_| Ok(last.take().expect("one actual STOP")),
                    |_| unreachable!(),
                )?)?)
            },
            |events| {
                for event in events {
                    ticks.push(rows(&event)[0]["timeTick"].as_str().unwrap().to_owned());
                }
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(requests, 6);
    assert_eq!(
        ticks,
        (0..512)
            .chain(10_000..10_130)
            .map(|tick| tick.to_string())
            .collect::<Vec<_>>()
    );
    assert!(state.owner().is_none());
}

#[test]
fn failed_native_stop_keeps_pending_callbacks_and_owner_until_a_real_retry() {
    let (mut state, mut session) = connected();
    state
        .sample(sample("HID/a", 88, 3, 4, 0), |_| Ok(identity()))
        .unwrap();
    assert!(
        session
            .stop(|_| {
                Ok(serde_json::to_value(state.stop_with(
                    "current",
                    |_| anyhow::bail!("native join failed"),
                    |_| unreachable!(),
                )?)?)
            })
            .is_err()
    );
    assert!(session.possibly_started());
    assert_eq!(state.owner().as_deref(), Some("current"));
    assert!(state.stopped_events.is_none());
    assert_eq!(state.devices["hid/a"].rows.len(), 1);
    let mut observed = Vec::new();
    session
        .stop_with(
            |_| {
                Ok(serde_json::to_value(state.stop_with(
                    "current",
                    |_| Ok(vec![]),
                    |_| unreachable!(),
                )?)?)
            },
            |events| {
                observed.extend(events);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(rows(&observed[0])[0]["timeTick"], "88");
    assert!(!session.possibly_started());
    assert!(state.owner().is_none());
}

#[test]
fn final_identity_failures_are_reported_after_stopping_and_releasing_the_owner() {
    let (mut state, mut session) = connected();
    let mut last = Some(vec![
        Event::Input(sample("HID/b", 88, 3, 4, 0)),
        Event::Input(sample("HID/c", 89, 5, 6, 0)),
    ]);
    let error = session
        .stop_with(
            |_| {
                Ok(serde_json::to_value(state.stop_with(
                    "current",
                    |_| Ok(last.take().unwrap()),
                    |path| anyhow::bail!("{path} identity failed"),
                )?)?)
            },
            |_| panic!("failed final observations must not appear successful"),
        )
        .unwrap_err();
    assert!(error.to_string().contains("HID/b identity failed"));
    assert!(error.to_string().contains("HID/c identity failed"));
    assert!(!session.possibly_started());
    assert!(state.owner().is_none());
    assert!(state.devices.is_empty());
}

#[test]
fn final_flush_failure_does_not_invent_success_or_keep_a_stopped_native_owner() {
    let (mut state, mut session) = connected();
    for _ in 0..16_384 {
        state
            .sample(
                sample("HID/a", 1, i32::MIN, i32::MIN, 0),
                |_| Ok(identity()),
            )
            .unwrap();
    }
    assert!(state.pending.is_empty());
    let error = session
        .stop(|_| {
            Ok(serde_json::to_value(state.stop_with(
                "current",
                |_| Ok(vec![]),
                |_| unreachable!(),
            )?)?)
        })
        .unwrap_err();
    assert!(error.to_string().contains("Flush monitor:"));
    assert!(error.to_string().contains("transport limit"));
    assert!(!session.possibly_started());
    assert!(state.owner().is_none());
}

#[test]
fn shutdown_drains_every_remaining_page_without_stopping_an_already_stopped_owner() {
    let (mut state, _) = connected();
    let last = (0..257)
        .map(|tick| Event::Input(sample("HID/a", tick, 1, 1, 1)))
        .collect();
    assert!(
        state
            .stop_with("current", |_| Ok(last), |_| Ok(identity()))
            .unwrap()
            .has_more()
    );
    state
        .shutdown_with(
            |_| panic!("shutdown must continue the existing drain"),
            |_| unreachable!(),
        )
        .unwrap();
    assert!(state.owner().is_none());
    assert!(state.pending.is_empty() && state.devices.is_empty());
    state
        .shutdown_with(|_| panic!("the owner is gone"), |_| unreachable!())
        .unwrap();
}

#[test]
fn shutdown_keeps_final_capture_failure_visible_after_draining_all_pages() {
    let (mut state, _) = connected();
    for tick in 0..128 {
        state
            .sample(sample("HID/a", tick, 1, 1, 1), |_| Ok(identity()))
            .unwrap();
    }
    let mut stops = 0;
    let error = state
        .shutdown_with(
            |_| {
                stops += 1;
                Ok(vec![Event::Input(sample("HID/b", 88, 3, 4, 0))])
            },
            |_| anyhow::bail!("final identity failed during shutdown"),
        )
        .unwrap_err();
    assert_eq!(stops, 1);
    assert!(
        error
            .to_string()
            .contains("final identity failed during shutdown")
    );
    assert!(state.owner().is_none());
    assert!(state.pending.is_empty() && state.devices.is_empty());
    assert!(!state.cleanup_pending());
    let repeated = state
        .shutdown_with(
            |_| panic!("the released native owner must not be stopped again"),
            |_| unreachable!(),
        )
        .unwrap_err();
    assert!(
        repeated
            .to_string()
            .contains("final identity failed during shutdown")
    );
}
