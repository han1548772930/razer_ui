use super::*;
use std::cell::Cell;

#[test]
fn connection_probe_uses_success_status_without_requiring_synthetic_version_bytes() {
    // Current 14397.mf is [4, 0, 147]; 12844.Qp's version projection is
    // optional, and the controller factory only consumes the SUCCESS status.
    for raw_data in [vec![], vec![17], vec![17, 21, 8], vec![17, 21, 8, 3]] {
        let mut calls = 0;
        let observation = read_connection_probe(2636, &mut |header, payload| {
            calls += 1;
            assert_eq!(header, [4, 0, 147]);
            assert!(payload.is_empty());
            Ok(raw_data.clone())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(observation.raw_data, raw_data);
        assert_eq!(
            observation.firmware_version(),
            (raw_data.len() >= 4).then_some(DockFirmwareVersion {
                major: 17,
                minor: 21,
                internal: 8,
                reserved: 3,
            })
        );
        observation.validate_for(2636).unwrap();
        assert!(observation.validate_for(2676).is_err());
    }
    let mut raw_data = vec![0; 80];
    raw_data[..4].copy_from_slice(&[4, 7, 19, 5]);
    let observed = read_connection_probe(2636, &mut |_, _| Ok(raw_data.clone())).unwrap();
    assert_eq!(observed.raw_data, raw_data);
    assert_eq!(
        observed.firmware_version(),
        Some(DockFirmwareVersion {
            major: 4,
            minor: 7,
            internal: 19,
            reserved: 5,
        })
    );
}

#[test]
fn connection_probe_failure_cancellation_and_invalid_branches_cannot_become_wired() {
    for error in [
        "Command not supported",
        "Feature transfer failed",
        "Canceled",
    ] {
        let result = read_connection_probe(2636, &mut |_, _| anyhow::bail!("{error}"));
        assert_eq!(format!("{:#}", result.unwrap_err()), error);
    }
    assert!(read_connection_probe(2636, &mut |_, _| Ok(vec![0; 81])).is_err());
    for product in [164, 241, 2676, 4115] {
        assert!(
            read_connection_probe(product, &mut |_, _| panic!(
                "Unverified product branch must not transfer"
            ))
            .is_err()
        );
    }
}

mod connection_feature_tests {
    use super::*;
    use crate::{backend::FeatureTransport, controller_feature};
    use std::{cell::RefCell, time::Duration};

    struct Mock {
        response: [u8; 91],
        returned: usize,
        requests: RefCell<Vec<Vec<u8>>>,
    }
    impl FeatureTransport for Mock {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.requests.borrow_mut().push(report.to_vec());
            Ok(())
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            assert_eq!(report[0], 4);
            report.copy_from_slice(&self.response);
            Ok(self.returned)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::json!({"simulated":true})
        }
    }
    fn successful_reply() -> [u8; 91] {
        let mut reply = [0; 91];
        reply[0] = 4;
        reply[1] = 2;
        reply[2] = 7;
        reply[6] = 4;
        reply[7] = 0;
        reply[8] = 147;
        reply[9..13].copy_from_slice(&[2, 17, 0, 29]);
        reply
    }
    fn probe(mock: &Mock) -> anyhow::Result<ConnectionProbeObservation> {
        read_connection_probe(2636, &mut |header, payload| {
            let request = controller_feature::report(4, 7, header, payload)?;
            controller_feature::exchange(mock, &request, Duration::ZERO, &mut |_| Ok(()))
        })
    }

    #[test]
    fn connection_probe_feature_request_matches_the_independent_source_packet() {
        let mock = Mock {
            response: successful_reply(),
            returned: 91,
            requests: RefCell::default(),
        };
        let observation = probe(&mock).unwrap();
        assert_eq!(observation.raw_data, [2, 17, 0, 29]);
        let mut expected = [0; 91];
        expected[0] = 4;
        expected[2] = 7;
        expected[6] = 4;
        expected[8] = 147;
        expected[89] = 151;
        assert_eq!(*mock.requests.borrow(), vec![expected.to_vec()]);
    }

    #[test]
    fn connection_probe_rejects_failed_unknown_stale_and_truncated_feature_replies() {
        let original = successful_reply();
        let mut replies = Vec::new();
        for status in [0, 1, 3, 4, 5, 255] {
            let mut reply = original;
            reply[1] = status;
            replies.push((reply, 91));
        }
        for (offset, value) in [(2, 8), (7, 1), (8, 135), (6, 81)] {
            let mut reply = original;
            reply[offset] = value;
            replies.push((reply, 91));
        }
        replies.push((original, 8));
        for (response, returned) in replies {
            let mock = Mock {
                response,
                returned,
                requests: RefCell::default(),
            };
            assert!(probe(&mock).is_err());
            assert!(
                mock.requests
                    .borrow()
                    .iter()
                    .all(|request| request[6..9] == [4, 0, 147])
            );
        }
    }

    #[test]
    fn connection_probe_preserves_only_payload_bytes_actually_returned_after_the_header() {
        for returned in [9, 10, 12, 13, 90, 91] {
            let mock = Mock {
                response: successful_reply(),
                returned,
                requests: RefCell::default(),
            };
            let observation = probe(&mock).unwrap();
            let payload = [2, 17, 0, 29];
            assert_eq!(observation.raw_data, payload[..(returned - 9).min(4)]);
            assert_eq!(mock.requests.borrow().len(), 1);
        }
    }

    #[test]
    fn connection_probe_cancellation_stops_before_device_io() {
        let mock = Mock {
            response: successful_reply(),
            returned: 91,
            requests: RefCell::default(),
        };
        let result = read_connection_probe(2636, &mut |header, payload| {
            let request = controller_feature::report(4, 7, header, payload)?;
            controller_feature::exchange(&mock, &request, Duration::ZERO, &mut |_| {
                anyhow::bail!("Connection generation canceled")
            })
        });
        assert!(result.is_err());
        assert!(mock.requests.borrow().is_empty());
    }
}

// Independent expectations from current MW modules 14397/82929 (polling),
// hn/An (ten-byte assignment decoder), and obmEngineController's read loop.
#[test]
fn current_polling_mount_selects_profile_period_or_high_speed_and_retains_unknown_codes() {
    for (product, command, rates) in [
        (
            2636,
            0x8e,
            vec![
                (1, 1000),
                (2, 500),
                (3, 333),
                (4, 250),
                (5, 200),
                (8, 125),
                (10, 100),
            ],
        ),
        (
            2676,
            0xc0,
            vec![
                (1, 8000),
                (2, 4000),
                (4, 2000),
                (8, 1000),
                (16, 500),
                (32, 250),
                (64, 125),
            ],
        ),
    ] {
        for (raw_code, hertz) in rates {
            let mut calls = 0;
            let observed = read_polling(product, 3, &mut |header, payload| {
                calls += 1;
                assert_eq!(header, [2, 0, command]);
                assert_eq!(payload, [3]);
                Ok(vec![3, raw_code])
            })
            .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(observed.raw_code, raw_code);
            assert_eq!(observed.hertz(), Some(hertz));
            observed.validate_for(product, 3).unwrap();
            assert!(observed.validate_for(product, 2).is_err());
        }
        for raw in [0, 7, 255] {
            let observed = read_polling(product, 3, &mut |_, _| Ok(vec![3, raw])).unwrap();
            assert_eq!(observed.raw_code, raw);
            assert_eq!(observed.hertz(), None);
        }
    }
    assert!(
        read_polling(4115, 1, &mut |_, _| panic!(
            "Unverified product must not transfer"
        ))
        .is_err()
    );
}

#[test]
fn polling_errors_do_not_produce_default_values_or_accept_another_slot() {
    for response in [vec![], vec![3], vec![2, 1], vec![3, 1, 0]] {
        assert!(read_polling(2636, 3, &mut |_, _| Ok(response.clone())).is_err());
    }
    assert!(
        read_polling(2676, 1, &mut |_, _| anyhow::bail!(
            "Feature transfer failed"
        ))
        .is_err()
    );
    let observation = PollingObservation {
        profile_slot: 3,
        method: PollingMethod::ProfilePeriod,
        raw_code: 1,
    };
    assert!(observation.validate_for(2676, 3).is_err());
}

#[test]
fn mapping_gets_real_buttons_in_order_and_keeps_default_unknown_and_failed_rows() {
    for product in [2636, 2676] {
        let mut calls = Vec::new();
        let result = read_mappings(
            product,
            2,
            &[16, 17, 18, 19],
            &mut |header, request| {
                assert_eq!(header, [80, 2, 0x8c]);
                calls.push(request.to_vec());
                match request {
                    [2, 16, 0] => Ok(vec![2, 16, 0, 16, 1, 16, 0, 0, 0, 0]),
                    [2, 17, 0] => anyhow::bail!("Independent button failure"),
                    [2, 18, 0] => Ok(vec![2, 18, 0, 250, 5, 1, 2, 3, 4, 5]),
                    [2, 19, 0] => Ok(vec![2, 19, 0, 0, 0, 0, 0, 0, 0, 0]),
                    _ => panic!("Unexpected mapping request"),
                }
            },
            &mut || false,
        )
        .unwrap();
        assert_eq!(
            calls,
            vec![
                vec![2, 16, 0],
                vec![2, 17, 0],
                vec![2, 18, 0],
                vec![2, 19, 0]
            ]
        );
        assert!(!result.canceled);
        assert_eq!(result.buttons.len(), 4);
        let AssignmentRead::Observed { assignment } = &result.buttons[0].result else {
            panic!("First mapping was read")
        };
        assert_eq!(assignment.function(), AssignmentFunction::Controller);
        assert!(assignment.source_omits_assignment());
        assert_eq!(assignment.function_data().unwrap(), [16]);
        assert_eq!(
            result.buttons[1].result,
            AssignmentRead::Failed {
                error: "Independent button failure".into()
            }
        );
        let AssignmentRead::Observed { assignment } = &result.buttons[2].result else {
            panic!("Unknown mapping must retain its raw reply")
        };
        assert_eq!(assignment.function(), AssignmentFunction::Unknown(250));
        assert_eq!(assignment.function_data().unwrap(), [1, 2, 3, 4, 5]);
        assert!(!assignment.source_omits_assignment());
        let AssignmentRead::Observed { assignment } = &result.buttons[3].result else {
            panic!("Disabled mapping must remain an observed row")
        };
        assert_eq!(assignment.function(), AssignmentFunction::Disabled);
        assert!(!assignment.source_omits_assignment());
        assert!(assignment.function_data().unwrap().is_empty());
        result.validate_for(2, &[16, 17, 18, 19]).unwrap();
        assert!(result.validate_for(2, &[16, 18, 17, 19]).is_err());
    }
}

#[test]
fn mapping_cancellation_preserves_completed_reads_and_never_starts_another_button() {
    let canceled = Cell::new(false);
    let mut calls = 0;
    let observed = read_mappings(
        2636,
        1,
        &[16, 17],
        &mut |_, _| {
            calls += 1;
            canceled.set(true);
            Ok(vec![1, 16, 0, 16, 1, 17, 0, 0, 0, 0])
        },
        &mut || canceled.get(),
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert!(observed.canceled);
    assert_eq!(observed.buttons.len(), 1);
    assert!(matches!(
        observed.buttons[0].result,
        AssignmentRead::Observed { .. }
    ));
    observed.validate_for(1, &[16, 17]).unwrap();
    let mut malformed = observed.clone();
    malformed.canceled = false;
    assert!(malformed.validate_for(1, &[16, 17]).is_err());
    let observed = read_mappings(
        2676,
        1,
        &[16],
        &mut |_, _| panic!("Canceled before first transfer"),
        &mut || true,
    )
    .unwrap();
    assert!(observed.canceled);
    assert!(observed.buttons.is_empty());
}

#[test]
fn mapping_identity_and_length_failures_remain_per_button_and_never_restore_defaults() {
    for payload in [
        vec![],
        vec![1, 16, 0],
        vec![2, 16, 0, 16, 1, 17, 0, 0, 0, 0],
        vec![1, 17, 0, 16, 1, 17, 0, 0, 0, 0],
        vec![1, 16, 1, 16, 1, 17, 0, 0, 0, 0],
        vec![0; 81],
    ] {
        let observed = read_mappings(2636, 1, &[16], &mut |_, _| Ok(payload.clone()), &mut || {
            false
        })
        .unwrap();
        assert!(matches!(
            observed.buttons[0].result,
            AssignmentRead::Failed { .. }
        ));
    }
    let assignment = ButtonAssignment::parse(1, 16, &[1, 16, 0, 250, 6, 1, 2, 3, 4, 5]).unwrap();
    assert_eq!(assignment.data_size, 6);
    assert_eq!(assignment.data, [1, 2, 3, 4, 5]);
    assert!(assignment.function_data().is_err());
    for ids in [vec![], vec![0], vec![16, 16], vec![1; 80]] {
        assert!(
            read_mappings(
                2636,
                1,
                &ids,
                &mut |_, _| panic!("Invalid directory must not transfer"),
                &mut || false
            )
            .is_err()
        );
    }
}
