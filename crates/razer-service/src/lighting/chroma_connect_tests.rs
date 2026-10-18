use super::*;
use std::{cell::Cell, collections::VecDeque};

#[derive(Default)]
struct Mock {
    sent: Vec<String>,
    incoming: VecDeque<String>,
}
impl Channel for Mock {
    fn send(
        &mut self,
        text: String,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        guard(validate, deadline)?;
        self.sent.push(text);
        Ok(())
    }
    fn receive(
        &mut self,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<String> {
        guard(validate, deadline)?;
        self.incoming
            .pop_front()
            .context("Simulated local service did not respond")
    }
    fn finish(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
fn token_matching_ignores_notifications_and_preserves_actual_vendor_response() {
    let mut channel = Mock {
        incoming: [
            r#"{"type":2,"value":true}"#,
            r#"{"token":{"id":49},"result":0}"#,
            r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"result":1247,"extension":"kept"}"#,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        ..Default::default()
    };
    let reply = exchange(
        &mut channel,
        &mut 0,
        "SetAttribute",
        &AttributeParameters {
            pid: 221,
            attribute: 2,
            value: 0,
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        reply.json,
        r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"result":1247,"extension":"kept"}"#
    );
    assert_eq!(reply.result, 1247);
    let sent: serde_json::Value = serde_json::from_str(&channel.sent[0]).unwrap();
    assert_eq!(
        sent,
        serde_json::json!({"func":"SetAttribute","param":{"pid":221,"attribute":2,"value":0},"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}}})
    );
    assert!(channel.incoming.is_empty());
}

#[test]
fn init_uses_result_and_does_not_infer_success_from_return_or_missing_field() {
    for (response, expected) in [
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0,"return":9}"#,
            Some(0),
        ),
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"return":0}"#,
            None,
        ),
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":true}"#,
            None,
        ),
    ] {
        let mut channel = Mock {
            incoming: [response.to_owned()].into(),
            ..Default::default()
        };
        let reply = exchange(
            &mut channel,
            &mut 0,
            "Init",
            &AppParameters { appid: APP_ID },
            &|| Ok(()),
        );
        assert_eq!(reply.ok().map(|reply| reply.result), expected);
        let sent: serde_json::Value = serde_json::from_str(&channel.sent[0]).unwrap();
        assert_eq!(
            sent["param"]["appid"],
            "{9BF2B22C-8F41-42C0-822D-3E811070315C}"
        );
    }
}

#[test]
fn request_counter_wraps_to_zero_and_old_response_is_not_accepted() {
    let mut channel = Mock {
        incoming: [
            r#"{"token":{"id":65535},"return":0}"#.to_owned(),
            r#"{"token":{"id":0,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#.to_owned(),
        ]
        .into(),
        ..Default::default()
    };
    let mut counter = u16::MAX;
    let reply = exchange(
        &mut channel,
        &mut counter,
        "UnInit",
        &AppParameters { appid: APP_ID },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(counter, 0);
    assert_eq!(reply.result, 0);
    assert!(reply.json.contains(r#""id":0"#));
    assert!(channel.sent[0].contains(r#""id":0"#));
}

#[test]
fn cancellation_prevents_submission_and_rejects_inflight_response() {
    let mut channel = Mock {
        incoming: [r#"{"token":{"id":1},"return":0}"#.to_owned()].into(),
        ..Default::default()
    };
    assert!(
        exchange(
            &mut channel,
            &mut 0,
            "Init",
            &AppParameters { appid: APP_ID },
            &|| anyhow::bail!("owner retired")
        )
        .unwrap_err()
        .downcast_ref::<Canceled>()
        .is_some()
    );
    assert!(channel.sent.is_empty());
    let checks = Cell::new(0);
    let canceled = exchange(
        &mut channel,
        &mut 0,
        "Init",
        &AppParameters { appid: APP_ID },
        &|| {
            checks.set(checks.get() + 1);
            ensure!(
                checks.get() < 4,
                "owner retired while awaiting its response"
            );
            Ok(())
        },
    );
    assert!(canceled.unwrap_err().downcast_ref::<Canceled>().is_some());
    assert_eq!(channel.sent.len(), 1);
}

#[test]
fn absent_response_and_malformed_token_are_errors_without_success_receipts() {
    for response in [None, Some(r#"{"token":{"id":"1"},"return":0}"#)] {
        let mut channel = Mock {
            incoming: response.into_iter().map(str::to_owned).collect(),
            ..Default::default()
        };
        assert!(
            exchange(
                &mut channel,
                &mut 0,
                "Init",
                &AppParameters { appid: APP_ID },
                &|| Ok(())
            )
            .is_err()
        );
    }
}

#[test]
fn canceled_or_closed_session_does_not_connect_or_claim_a_mutation() {
    let mut session = Session::new();
    assert!(matches!(
        session.set_attribute(221, 2, 0, &|| anyhow::bail!("retired")),
        ChromaAttributeReceipt::Canceled
    ));
    session.finish().unwrap();
    session.finish().unwrap();
    assert!(matches!(
        session.set_attribute(221, 2, 0, &|| Ok(())),
        ChromaAttributeReceipt::NotSent { .. }
    ));
    assert!(session.socket.is_none());
}

#[test]
fn lost_connection_or_uninit_failure_does_not_erase_registration_release() {
    let mut registration = Registration::default();
    let mut counter = 0;
    let mut first = Mock {
        incoming: [r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#.to_owned()].into(),
        ..Default::default()
    };
    assert_eq!(
        registration
            .initialize(&mut first, &mut counter, &|| Ok(()))
            .unwrap()
            .result,
        0
    );
    assert!(registration.ensure_released().is_err());
    assert!(
        exchange(
            &mut first,
            &mut counter,
            "SetAttribute",
            &AttributeParameters {
                pid: 221,
                attribute: 2,
                value: 0
            },
            &|| Ok(())
        )
        .is_err()
    );
    drop(first);
    assert!(registration.ensure_released().is_err());
    let mut recovery = Mock {
        incoming: [
            r#"{"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":5}"#.to_owned(),
            r#"{"token":{"id":4,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#.to_owned(),
        ]
        .into(),
        ..Default::default()
    };
    assert!(
        registration
            .uninitialize(&mut recovery, &mut counter, &|| Ok(()))
            .is_err()
    );
    assert!(registration.ensure_released().is_err());
    registration
        .uninitialize(&mut recovery, &mut counter, &|| Ok(()))
        .unwrap();
    registration.ensure_released().unwrap();
}

#[test]
fn canceled_init_retains_the_uncertain_registration_until_real_cleanup() {
    let mut registration = Registration::default();
    let mut channel = Mock::default();
    let checks = Cell::new(0);
    assert!(
        registration
            .initialize(&mut channel, &mut 0, &|| {
                checks.set(checks.get() + 1);
                ensure!(checks.get() < 4, "owner retired while awaiting Init");
                Ok(())
            })
            .is_err()
    );
    assert_eq!(channel.sent.len(), 1);
    // No success is inferred from an absent response or a closed socket.
    assert!(registration.ensure_released().is_err());
}

#[test]
fn uninit_requires_native_result_zero_before_releasing_registration() {
    for (response, released) in [
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0,"return":9}"#,
            true,
        ),
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":126,"return":0}"#,
            false,
        ),
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"return":0}"#,
            false,
        ),
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":true}"#,
            false,
        ),
        (
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":"0"}"#,
            false,
        ),
        (r#"{"token":{"id":1},"result":0}"#, false),
        (
            r#"{"token":{"id":1,"param":{"appid":"another-app"}},"result":0}"#,
            false,
        ),
    ] {
        let mut registration = Registration::default();
        let mut initialization = Mock::default();
        // A request accepted without a response leaves a real cleanup debt.
        assert!(
            registration
                .initialize(&mut initialization, &mut 0, &|| Ok(()))
                .is_err()
        );
        let mut channel = Mock {
            incoming: [response.to_owned()].into(),
            ..Default::default()
        };
        assert_eq!(
            registration
                .uninitialize(&mut channel, &mut 0, &|| Ok(()))
                .is_ok(),
            released
        );
        assert_eq!(registration.ensure_released().is_ok(), released);
    }
}

#[test]
fn attribute_replies_require_the_copied_parameters_and_native_integer_result() {
    for response in [
        r#"{"token":{"id":1},"result":0}"#,
        r#"{"token":{"id":1,"param":{"pid":222,"attribute":2,"value":0}},"result":0}"#,
        r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"return":0}"#,
        r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"result":false}"#,
        r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"result":-2147483649}"#,
        r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"result":4294967296}"#,
    ] {
        let mut channel = Mock {
            incoming: [response.to_owned()].into(),
            ..Default::default()
        };
        assert!(
            exchange(
                &mut channel,
                &mut 0,
                "SetAttribute",
                &AttributeParameters {
                    pid: 221,
                    attribute: 2,
                    value: 0
                },
                &|| Ok(())
            )
            .is_err(),
            "Accepted invalid native response: {response}"
        );
    }
}

#[test]
fn new_endpoint_loads_its_module_before_releasing_the_existing_registration() {
    let mut registration = Registration::default();
    let mut disconnected = Mock::default();
    assert!(
        registration
            .initialize(&mut disconnected, &mut 0, &|| Ok(()))
            .is_err()
    );
    let mut channel = Mock {
        incoming: [
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":1247}"#.to_owned(),
            r#"{"token":{"id":2,"param":{"online":false}},"result":0}"#.to_owned(),
            r#"{"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#.to_owned(),
        ].into(),
        ..Default::default()
    };
    registration
        .release_on_new_connection(&mut channel, &mut 0, &|| Ok(()))
        .unwrap();
    assert_eq!(
        channel.sent,
        [
            r#"{"func":"Init","param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"},"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}}}"#,
            r#"{"func":"SetSynapseStatus","param":{"online":false},"token":{"id":2,"param":{"online":false}}}"#,
            r#"{"func":"UnInit","param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"},"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}}}"#,
        ]
    );
    registration.ensure_released().unwrap();
}

#[test]
fn cleanup_attempts_uninit_after_a_lost_init_response_and_retains_failures() {
    for (response, released) in [
        (
            r#"{"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#,
            true,
        ),
        (
            r#"{"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":126}"#,
            false,
        ),
    ] {
        let mut registration = Registration::default();
        let mut channel = Mock {
            incoming: [
                "invalid-init-response".to_owned(),
                r#"{"token":{"id":2,"param":{"online":false}},"result":0}"#.to_owned(),
                response.to_owned(),
            ]
            .into(),
            ..Default::default()
        };
        let error = registration
            .release_on_new_connection(&mut channel, &mut 0, &|| Ok(()))
            .unwrap_err();
        assert!(format!("{error:#}").contains("cleanup Init failed"));
        assert_eq!(
            format!("{error:#}").contains("UnInit was not confirmed"),
            !released
        );
        assert_eq!(channel.sent.len(), 3);
        assert!(channel.sent[2].starts_with(r#"{"func":"UnInit","#));
        assert_eq!(registration.ensure_released().is_ok(), released);
    }
}

#[test]
fn source_status_calls_preserve_the_online_flag_and_actual_native_result() {
    for online in [true, false] {
        for result in [0, 5] {
            let mut channel = Mock {
                incoming: [serde_json::json!({
                    "token": {"id":1, "param":{"online":online}}, "result":result
                })
                .to_string()]
                .into(),
                ..Default::default()
            };
            assert_eq!(
                set_synapse_status(&mut channel, &mut 0, online, &|| Ok(())).is_ok(),
                result == 0
            );
            assert_eq!(channel.sent.len(), 1);
            let sent: serde_json::Value = serde_json::from_str(&channel.sent[0]).unwrap();
            assert_eq!(
                sent,
                serde_json::json!({
                    "func":"SetSynapseStatus", "param":{"online":online},
                    "token":{"id":1,"param":{"online":online}}
                })
            );
        }
    }
}

#[test]
fn application_init_announces_online_without_gating_attributes_on_status_failures() {
    for status_result in [0, 5] {
        let mut registration = Registration::default();
        let mut channel = Mock {
            incoming: [
                r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#.to_owned(),
                serde_json::json!({"token":{"id":2,"param":{"online":true}},"result":status_result}).to_string(),
                r#"{"token":{"id":3,"param":{"pid":221,"attribute":2,"value":0}},"result":0}"#.to_owned(),
            ].into(),
            ..Default::default()
        };
        let mut counter = 0;
        let initialization = registration
            .initialize_application(&mut channel, &mut counter, &|| Ok(()))
            .unwrap();
        assert_eq!(initialization.result, 0);
        assert_eq!(initialization.status_error.is_some(), status_result != 0);
        assert!(registration.ensure_released().is_err());
        assert_eq!(
            exchange(
                &mut channel,
                &mut counter,
                "SetAttribute",
                &AttributeParameters {
                    pid: 221,
                    attribute: 2,
                    value: 0
                },
                &|| Ok(())
            )
            .unwrap()
            .result,
            0
        );
        let sent: Vec<serde_json::Value> = channel
            .sent
            .iter()
            .map(|json| serde_json::from_str(json).unwrap())
            .collect();
        assert_eq!(
            sent.iter()
                .map(|request| request["func"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Init", "SetSynapseStatus", "SetAttribute"]
        );
        assert_eq!(sent[1]["param"]["online"], true);
    }
}

#[test]
fn unsuccessful_init_does_not_announce_application_online() {
    let mut registration = Registration::default();
    let mut channel = Mock {
        incoming: [r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":1247}"#.to_owned()].into(),
        ..Default::default()
    };
    let initialization = registration
        .initialize_application(&mut channel, &mut 0, &|| Ok(()))
        .unwrap();
    assert_eq!(initialization.result, 1247);
    assert!(initialization.status_error.is_none());
    assert_eq!(channel.sent.len(), 1);
    assert!(registration.ensure_released().is_err());
}

#[test]
fn offline_failure_does_not_skip_uninit_or_hide_its_actual_release_receipt() {
    let mut registration = Registration::default();
    let mut channel = Mock {
        incoming: [
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":1247}"#.to_owned(),
            r#"{"token":{"id":2,"param":{"online":false}},"result":5}"#.to_owned(),
            r#"{"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":0}"#.to_owned(),
        ].into(),
        ..Default::default()
    };
    let error = registration
        .release_on_new_connection(&mut channel, &mut 0, &|| Ok(()))
        .unwrap_err();
    assert!(format!("{error:#}").contains("offline status"));
    assert_eq!(channel.sent.len(), 3);
    registration.ensure_released().unwrap();
}

#[test]
fn cleanup_preserves_each_failed_phase_and_keeps_the_unconfirmed_registration() {
    let mut registration = Registration::default();
    let mut channel = Mock {
        incoming: [
            r#"{"token":{"id":1,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":5}"#.to_owned(),
            r#"{"token":{"id":2,"param":{"online":false}},"result":126}"#.to_owned(),
            r#"{"token":{"id":3,"param":{"appid":"{9BF2B22C-8F41-42C0-822D-3E811070315C}"}},"result":6}"#.to_owned(),
        ].into(),
        ..Default::default()
    };
    let error = registration
        .release_on_new_connection(&mut channel, &mut 0, &|| Ok(()))
        .unwrap_err();
    let error = format!("{error:#}");
    assert!(error.contains("cleanup Init returned 5"));
    assert!(error.contains("SetSynapseStatus was not confirmed"));
    assert!(error.contains("UnInit was not confirmed"));
    assert_eq!(channel.sent.len(), 3);
    assert!(registration.ensure_released().is_err());
}
