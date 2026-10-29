use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;

struct Transport {
    sends: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<Vec<u8>>>,
}
impl FeatureTransport for Transport {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Source controller SET must use the collection Feature framing")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Source controller GET must use the host-compatible byte count")
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sends.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(report[0], self.sends.borrow().last().unwrap()[0]);
        let bytes = self
            .replies
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("Simulated replies exhausted"))?;
        report[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn reply(command: Command, report_id: u8, transaction: u8, status: u8, data: &[u8]) -> Vec<u8> {
    let mut report = command.report(report_id, transaction).to_vec();
    report[1] = status;
    report[6] = data.len() as u8;
    report[9..9 + data.len()].copy_from_slice(data);
    report
}
#[test]
fn every_controller_uses_source_report_selector_and_discards_the_returned_prefix() {
    for product_id in [2629, 2636, 2647, 2650, 2676, 2684, 4133, 4144] {
        let cap = capability(product_id).unwrap();
        let replies = [1, 2].into_iter().enumerate().map(|(transaction, side)| {
            let mut bytes = reply(
                Command::SetCircularity(1, side, true),
                cap.report_id(),
                transaction as u8,
                2,
                &[],
            );
            bytes[0] = 0x7f;
            bytes
        });
        let io = Transport {
            sends: RefCell::new(vec![]),
            replies: RefCell::new(replies.collect()),
        };
        let result = TriggerProtocol::new(&io, cap, |_| Ok(()))
            .execute(&TriggerOperation::Circularity {
                profile_slot: 1,
                circular: true,
            })
            .unwrap();
        assert!(result.is_none(), "Setter ACK cannot invent a readback");
        let sends = io.sends.borrow();
        assert_eq!(sends.len(), 2);
        for (ix, sent) in sends.iter().enumerate() {
            assert_eq!(sent[0], cap.report_id());
            assert_eq!(sent[2], ix as u8);
            assert_eq!(sent[6..12], [3, 12, 18, 1, ix as u8 + 1, 1]);
        }
        assert!(io.replies.borrow().is_empty());
    }
}

#[test]
fn a_setter_release_error_keeps_its_ack_and_does_not_write_the_second_side() {
    let mut accepted = vec![0; 91];
    accepted[1] = 2;
    accepted[6..11].copy_from_slice(&[2, 12, 18, 1, 1]);
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new([accepted].into()),
    };
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&io);
    let error = TriggerProtocol::new(&probe, capability(4133).unwrap(), |_| Ok(()))
        .execute(&TriggerOperation::Circularity {
            profile_slot: 1,
            circular: true,
        })
        .unwrap_err();
    let completed = crate::CompletedFeatureExchange::from_error(&error).unwrap();
    assert_eq!(completed.command(), [3, 12, 18]);
    assert_eq!(completed.data(), [1, 1]);
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(io.sends.borrow().len(), 1);
    assert!(io.replies.borrow().is_empty());
}
#[test]
fn digital_write_sets_mode_before_value_and_skips_inactive_side() {
    let mode = Command::SetMode(3, 2, 1);
    let value = Command::SetDigital(3, 2, 18, true);
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new([reply(mode, 4, 0, 2, &[]), reply(value, 4, 1, 2, &[])].into()),
    };
    let mut delays = vec![];
    let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
    let operation = TriggerOperation::Digital {
        profile_slot: 3,
        settings: vec![
            DigitalTrigger::new(1, 0, false, false).unwrap(),
            DigitalTrigger::new(2, 18, true, true).unwrap(),
        ],
    };
    let result = TriggerProtocol::new(&probe, capability(4133).unwrap(), |delay| {
        delays.push(delay);
        Ok(())
    })
    .execute(&operation)
    .unwrap();
    assert!(
        result.is_none(),
        "A write must not invent a getter observation"
    );
    let sends = io.sends.borrow();
    assert_eq!(sends.len(), 2);
    assert_eq!(&sends[0][6..12], &[3, 12, 21, 3, 2, 1]);
    assert_eq!(&sends[1][6..13], &[4, 12, 22, 3, 2, 18, 1]);
    assert_eq!(
        probe.timings(),
        vec![
            crate::backend::ExchangeTiming::MutexOut {
                interval: Duration::from_millis(10)
            };
            2
        ]
    );
    assert_eq!(
        delays,
        [
            Duration::ZERO,
            Duration::from_millis(10),
            Duration::ZERO,
            Duration::from_millis(10)
        ]
    );
}
#[test]
fn analog_write_preserves_all_active_entries_in_order_and_same_retry_transaction() {
    let commands = [
        Command::SetMode(1, 1, 0),
        Command::SetAnalog(1, 1, 5, 90),
        Command::SetMode(1, 2, 0),
        Command::SetAnalog(1, 2, 10, 80),
    ];
    let mut replies = vec![
        reply(commands[0], 10, 0, 1, &[]),
        reply(commands[0], 10, 9, 2, &[]),
    ];
    replies.extend(
        commands
            .iter()
            .enumerate()
            .map(|(i, c)| reply(*c, 10, i as u8, 2, &[])),
    );
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(replies.into()),
    };
    let operation = TriggerOperation::Analog {
        profile_slot: 1,
        settings: vec![
            AnalogTrigger::new(1, 5, 90, true).unwrap(),
            AnalogTrigger::new(2, 10, 80, true).unwrap(),
        ],
    };
    TriggerProtocol::new(&io, capability(2676).unwrap(), |_| Ok(()))
        .execute(&operation)
        .unwrap();
    let sends = io.sends.borrow();
    assert_eq!(sends.len(), 5);
    assert_eq!(sends[0], sends[1]);
    assert_eq!(&sends[4][6..13], &[4, 12, 19, 1, 2, 10, 80]);
    for s in sends.iter() {
        assert_eq!(s[89], s[3..89].iter().fold(0, |a, b| a ^ b));
    }
}
#[test]
fn read_keeps_both_inactive_settings_and_source_getter_order() {
    let responses = [
        (Command::GetAnalog(1, 1), vec![1, 1, 8, 98]),
        (Command::GetMode(1, 1), vec![1, 1, 1]),
        (Command::GetAnalog(1, 2), vec![1, 2, 0, 100]),
        (Command::GetMode(1, 2), vec![1, 2, 0]),
        (Command::GetDigital(1, 1), vec![1, 1, 12, 1]),
        (Command::GetMode(1, 1), vec![1, 1, 1]),
        (Command::GetDigital(1, 2), vec![1, 2, 0, 0]),
        (Command::GetMode(1, 2), vec![1, 2, 0]),
    ];
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(
            responses
                .iter()
                .enumerate()
                .map(|(i, (c, d))| reply(*c, 4, i as u8, 2, d))
                .collect(),
        ),
    };
    let profile = TriggerProtocol::new(&io, capability(2629).unwrap(), |_| Ok(()))
        .execute(&TriggerOperation::Read { profile_slot: 1 })
        .unwrap()
        .unwrap();
    assert_eq!(
        profile.analog_actuations[0],
        AnalogTrigger::new(1, 8, 98, false).unwrap()
    );
    assert_eq!(
        profile.digital_actuations[0],
        DigitalTrigger::new(1, 12, true, true).unwrap()
    );
    assert_eq!(
        profile.digital_actuations[1],
        DigitalTrigger::new(2, 0, false, false).unwrap()
    );
}
#[test]
fn unsupported_reply_stops_the_write_and_cancellation_prevents_io() {
    let op = TriggerOperation::Analog {
        profile_slot: 1,
        settings: vec![AnalogTrigger::new(1, 0, 100, true).unwrap()],
    };
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new([reply(Command::SetMode(1, 1, 0), 4, 0, 5, &[])].into()),
    };
    assert!(
        TriggerProtocol::new(&io, capability(2629).unwrap(), |_| Ok(()))
            .execute(&op)
            .is_err()
    );
    assert_eq!(io.sends.borrow().len(), 1);
    assert!(
        TriggerProtocol::new(&io, capability(2629).unwrap(), |_| anyhow::bail!(
            "canceled"
        ))
        .execute(&op)
        .is_err()
    );
    assert_eq!(io.sends.borrow().len(), 1);
}
#[test]
fn truncated_read_and_duplicate_profile_side_are_rejected() {
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(
            [
                reply(Command::GetAnalog(1, 1), 10, 0, 2, &[1, 1]),
                reply(Command::GetMode(1, 1), 10, 1, 2, &[1, 1, 0]),
            ]
            .into(),
        ),
    };
    assert!(
        TriggerProtocol::new(&io, capability(2684).unwrap(), |_| Ok(()))
            .execute(&TriggerOperation::Read { profile_slot: 1 })
            .is_err()
    );
    let side = AnalogTrigger::new(1, 0, 100, true).unwrap();
    assert!(
        TriggerOperation::Analog {
            profile_slot: 1,
            settings: vec![side.clone(), side]
        }
        .validate()
        .is_err()
    );
    assert!(capability(2636).unwrap().accepts(2636));
    assert!(!capability(2636).unwrap().accepts(2649));
}

#[test]
fn thumbstick_deadzone_preserves_source_array_order_big_endian_and_shared_flag() {
    let commands = [
        Command::SetZones(2, 2, 13, true),
        Command::SetZones(2, 1, 7, true),
    ];
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(
            commands
                .iter()
                .enumerate()
                .map(|(i, command)| reply(*command, 4, i as u8, 2, &[]))
                .collect(),
        ),
    };
    let result = TriggerProtocol::new(&io, capability(2650).unwrap(), |_| Ok(()))
        .execute(&TriggerOperation::ThumbstickDeadzone {
            profile_slot: 2,
            settings: vec![
                ThumbstickValue::new(2, 13).unwrap(),
                ThumbstickValue::new(1, 7).unwrap(),
            ],
            prevent_double_deadzone: true,
        })
        .unwrap();
    assert!(result.is_none());
    let sends = io.sends.borrow();
    assert_eq!(sends.len(), 2);
    assert_eq!(&sends[0][6..18], &[9, 12, 5, 2, 2, 0, 13, 0, 0, 0, 0, 1]);
    assert_eq!(&sends[1][9..18], &[2, 1, 0, 7, 0, 0, 0, 0, 1]);
    assert!(capability(2650).unwrap().accepts(2650));
    assert!(!capability(2650).unwrap().accepts(2647));
}

#[test]
fn thumbstick_read_requires_all_replies_in_source_order_and_keeps_left_prevention() {
    let replies = [
        (Command::GetCircularity(1, 1), vec![1, 1, 1]),
        (Command::GetZones(1, 1), vec![1, 1, 0, 5, 0, 0, 0, 0, 1]),
        (Command::GetZones(1, 2), vec![1, 2, 0, 15, 0, 0, 0, 0, 0]),
        (Command::GetSensitivity(1, 1), vec![1, 1, 0]),
        (Command::GetSensitivity(1, 2), vec![1, 2, 80]),
    ];
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(
            replies
                .iter()
                .enumerate()
                .map(|(i, (command, data))| reply(*command, 10, i as u8, 2, data))
                .collect(),
        ),
    };
    let profile = TriggerProtocol::new(&io, capability(2684).unwrap(), |_| Ok(()))
        .execute(&TriggerOperation::ReadThumbsticks { profile_slot: 1 })
        .unwrap()
        .unwrap();
    let thumbsticks = profile.thumbsticks().unwrap();
    assert!(thumbsticks.circular());
    assert!(thumbsticks.prevent_double_deadzone());
    assert_eq!(
        thumbsticks.deadzones(),
        &[
            ThumbstickValue::new(1, 5).unwrap(),
            ThumbstickValue::new(2, 15).unwrap()
        ]
    );
    assert_eq!(
        thumbsticks.sensitivity_values(),
        &[
            ThumbstickValue::new(1, 0).unwrap(),
            ThumbstickValue::new(2, 80).unwrap()
        ]
    );
    assert!(profile.analog_actuations().is_empty());
    assert_eq!(io.sends.borrow().len(), 5);
}

#[test]
fn circularity_writes_both_sides_without_getter_and_cancellation_stops_second_side() {
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(
            [
                reply(Command::SetCircularity(4, 1, false), 4, 0, 2, &[]),
                reply(Command::SetCircularity(4, 2, false), 4, 1, 2, &[]),
            ]
            .into(),
        ),
    };
    let mut waits = 0;
    let result = TriggerProtocol::new(&io, capability(2636).unwrap(), |_| {
        waits += 1;
        ensure!(waits <= 2, "Canceled between controller sides");
        Ok(())
    })
    .execute(&TriggerOperation::Circularity {
        profile_slot: 4,
        circular: false,
    });
    assert!(result.is_err());
    assert_eq!(io.sends.borrow().len(), 1);
    assert_eq!(&io.sends.borrow()[0][6..12], &[3, 12, 18, 4, 1, 0]);
}

#[test]
fn malformed_thumbstick_response_and_invalid_sensitivity_are_not_success() {
    let io = Transport {
        sends: RefCell::new(vec![]),
        replies: RefCell::new(
            [
                reply(Command::GetCircularity(1, 1), 4, 0, 2, &[1, 1, 0]),
                reply(Command::GetZones(1, 1), 4, 1, 2, &[1, 1, 0, 7]),
            ]
            .into(),
        ),
    };
    assert!(
        TriggerProtocol::new(&io, capability(2629).unwrap(), |_| Ok(()))
            .execute(&TriggerOperation::ReadThumbsticks { profile_slot: 1 })
            .is_err()
    );
    assert_eq!(io.sends.borrow().len(), 2);
    let invalid: TriggerOperation = serde_json::from_value(serde_json::json!({
        "kind":"thumbstick_sensitivity", "profile_slot":1, "settings":[{"controllerId":1,"value":101}]
    })).unwrap();
    assert!(invalid.validate().is_err());
}
