use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Transport {
    sent: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<Vec<u8>>>,
}
impl Transport {
    fn new(replies: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            sent: RefCell::new(Vec::new()),
            replies: RefCell::new(replies.into_iter().collect()),
        }
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Expected collection Feature SET")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Expected collection Feature GET")
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .replies
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("No simulated response"))?;
        report[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated": true})
    }
}
fn reply(status: u8, transaction: u8, command: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 91];
    bytes[0] = 10;
    bytes[1] = status;
    bytes[2] = transaction;
    bytes[6] = payload.len() as u8;
    bytes[7] = 12;
    bytes[8] = command;
    bytes[9..9 + payload.len()].copy_from_slice(payload);
    bytes
}
#[test]
fn controller_socd_set_encodes_each_option_and_does_not_read_back_or_compare_ack_payload() {
    for (mode, option) in [
        (SocdMode::Neutral, 0),
        (SocdMode::Last, 1),
        (SocdMode::First, 2),
        (SocdMode::None, 3),
        (SocdMode::AbsoluteUp, 4),
    ] {
        for payload in [vec![], vec![255, 254]] {
            let io = Transport::new([reply(2, 0, 27, &payload)]);
            let mut waits = Vec::new();
            let result = SocdProtocol::new(&io, |wait| {
                waits.push(wait);
                Ok(())
            })
            .execute(2676, SocdOperation { slot: 7, mode }, || false)
            .unwrap();
            assert!(result.acknowledged());
            assert_eq!(
                result.write,
                SocdWriteStatus::Acknowledged {
                    response_payload: payload
                }
            );
            let sent = io.sent.borrow();
            assert_eq!(sent.len(), 1, "The source setter has no following GET");
            assert_eq!(sent[0][0], 10);
            assert_eq!(&sent[0][6..11], &[2, 12, 27, 7, option]);
            assert_eq!(sent[0][89], sent[0][3..89].iter().fold(0, |a, b| a ^ b));
            assert_eq!(waits, [Duration::from_millis(30); 2]);
        }
    }
}
#[test]
fn controller_socd_rejected_or_truncated_response_never_acknowledges() {
    for bytes in [reply(5, 0, 27, &[]), vec![0; 90]] {
        let io = Transport::new([bytes]);
        let result = SocdProtocol::new(&io, |_| Ok(()))
            .execute(
                2676,
                SocdOperation {
                    slot: 1,
                    mode: SocdMode::First,
                },
                || false,
            )
            .unwrap();
        assert!(matches!(result.write, SocdWriteStatus::Failed { .. }));
        assert!(!result.acknowledged());
        assert_eq!(io.sent.borrow().len(), 1);
    }
}
#[test]
fn controller_socd_stale_response_cannot_confirm_and_cancel_never_claims_success() {
    for stale in [reply(2, 1, 27, &[]), reply(2, 0, 155, &[])] {
        let io = Transport::new([stale]);
        let result = SocdProtocol::new(&io, |_| Ok(()))
            .execute(
                2676,
                SocdOperation {
                    slot: 2,
                    mode: SocdMode::Last,
                },
                || false,
            )
            .unwrap();
        assert!(!result.acknowledged());
    }
    for after_set in [false, true] {
        let io = Transport::new([reply(2, 0, 27, &[])]);
        let calls = Cell::new(0);
        let result = SocdProtocol::new(&io, |_| Ok(()))
            .execute(
                2676,
                SocdOperation {
                    slot: 1,
                    mode: SocdMode::Neutral,
                },
                || {
                    let count = calls.get();
                    calls.set(count + 1);
                    !after_set || count != 0
                },
            )
            .unwrap();
        assert!(result.canceled);
        assert!(!result.acknowledged());
        assert_eq!(io.sent.borrow().len(), usize::from(after_set));
    }
}
#[test]
fn controller_socd_unmounted_product_and_invalid_result_are_rejected() {
    let io = Transport::new([]);
    assert!(
        SocdProtocol::new(&io, |_| Ok(()))
            .execute(
                2636,
                SocdOperation {
                    slot: 0,
                    mode: SocdMode::Neutral
                },
                || false
            )
            .is_err()
    );
    assert!(io.sent.borrow().is_empty());
    assert!(
        SocdResult {
            write: SocdWriteStatus::NotAttempted,
            canceled: false
        }
        .validate()
        .is_err()
    );
    assert!(
        SocdResult {
            write: SocdWriteStatus::Failed {
                error: String::new()
            },
            canceled: false
        }
        .validate()
        .is_err()
    );
}

#[test]
fn controller_socd_present_zero_profile_slot_is_encoded_as_wire_one_without_changing_identity() {
    let io = Transport::new([reply(2, 0, 27, &[])]);
    let operation = SocdOperation {
        slot: 0,
        mode: SocdMode::First,
    };
    assert_eq!(operation.slot, 0);
    assert_eq!(operation.wire_slot(), 1);
    let result = SocdProtocol::new(&io, |_| Ok(()))
        .execute(2676, operation, || false)
        .unwrap();
    assert!(result.acknowledged());
    assert_eq!(&io.sent.borrow()[0][6..11], &[2, 12, 27, 1, 2]);
}
