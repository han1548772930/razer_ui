use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Transport {
    replies: RefCell<VecDeque<anyhow::Result<Vec<u8>>>>,
    sent: RefCell<Vec<Vec<u8>>>,
    send_error: bool,
    get_selectors: RefCell<Vec<u8>>,
}
impl FeatureTransport for Transport {
    fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(bytes.to_vec());
        if self.send_error {
            anyhow::bail!("send uncertain");
        }
        Ok(())
    }
    fn get_feature(&self, data: &mut [u8]) -> anyhow::Result<usize> {
        self.get_selectors.borrow_mut().push(data[0]);
        let bytes = self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("unexpected GET")?;
        data[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn transport(replies: Vec<anyhow::Result<Vec<u8>>>) -> Transport {
    Transport {
        replies: RefCell::new(replies.into()),
        sent: RefCell::new(Vec::new()),
        send_error: false,
        get_selectors: RefCell::new(Vec::new()),
    }
}
fn cap() -> ReceiverCapability {
    let mut cap = source_capability().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 3;
    cap.max_retry_out = 2;
    cap
}
fn reply(status: u8, transaction: u8, class: u8, id: u8, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut data = vec![0; 91];
    data[1] = status;
    data[2] = transaction;
    data[6] = bytes.len() as u8;
    data[7] = class;
    data[8] = id;
    data[9..9 + bytes.len()].copy_from_slice(bytes);
    Ok(data)
}
#[test]
fn hyperpolling_receiver_serial_preserves_ascii_after_zero_and_empty_identity_semantics() {
    let device = transport(vec![reply(2, 224, 0, 130, &[0, b'A', 31, b'B', 127, b'C'])]);
    let Reading::Serial(serial) =
        execute(&device, &cap(), Command::Serial, 224, &|| Ok(())).unwrap()
    else {
        panic!()
    };
    assert_eq!(serial.ascii(), "ABC");
    assert_eq!(serial.source_identity(), "ABC");
    assert_eq!(&device.sent.borrow()[0][6..9], &[22, 0, 130]);
    assert_eq!(
        SerialNumber {
            bytes: vec![0, 1, 255]
        }
        .source_identity(),
        "NOSERIALNUMBER"
    );
    assert_eq!(*device.get_selectors.borrow(), [0]);
}
#[test]
fn hyperpolling_receiver_indicator_get_keeps_zero_absent_and_unknown_values() {
    for bytes in [vec![], vec![0], vec![3], vec![254]] {
        let device = transport(vec![reply(2, 225, 7, 144, &bytes)]);
        assert_eq!(
            execute(&device, &cap(), Command::Indicator, 225, &|| Ok(())).unwrap(),
            Reading::Indicator {
                status: bytes.first().copied()
            }
        );
        assert_eq!(&device.sent.borrow()[0][6..10], &[1, 7, 144, 0]);
    }
}
#[test]
fn hyperpolling_receiver_setter_uses_primary_namespace_and_no_extra_getter() {
    let device = transport(vec![reply(2, 226, 7, 16, &[2])]);
    let Reading::Submitted(report) = execute(
        &device,
        &cap(),
        Command::SetIndicator { status: 254 },
        226,
        &|| Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    assert!(report.acknowledged && report.write_attempted && report.write_sent);
    assert_eq!(report.requested_status, 254);
    assert_eq!(report.response_status, Some(2));
    assert!(report.error.is_none());
    let sent = device.sent.borrow();
    assert_eq!(sent.len(), 1);
    assert_eq!(&sent[0][6..10], &[1, 7, 16, 254]);
    assert_eq!(sent[0][89], sent[0][3..89].iter().fold(0, |x, b| x ^ b));
}
#[test]
fn hyperpolling_receiver_failed_send_retains_uncertain_write_and_exits_source_loop() {
    let mut device = transport(Vec::new());
    device.send_error = true;
    let Reading::Submitted(report) = execute(
        &device,
        &cap(),
        Command::SetIndicator { status: 2 },
        224,
        &|| Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    assert!(report.write_attempted && !report.write_sent && !report.acknowledged);
    assert!(report.error.unwrap().contains("send uncertain"));
    assert_eq!(device.sent.borrow().len(), 1);
}
#[test]
fn hyperpolling_receiver_busy_in_failure_retries_out_without_fabricating_ack() {
    let device = transport(vec![
        reply(1, 224, 7, 16, &[]),
        Err(anyhow::anyhow!("busy getter failure")),
        reply(2, 224, 7, 16, &[3]),
    ]);
    let Reading::Submitted(report) = execute(
        &device,
        &cap(),
        Command::SetIndicator { status: 3 },
        224,
        &|| Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    assert!(report.acknowledged);
    assert_eq!(device.sent.borrow().len(), 2);
}
#[test]
fn hyperpolling_receiver_rejects_other_product_transaction_and_truncated_metadata() {
    let device = transport(Vec::new());
    let mut other = cap();
    other.product_id = 164;
    assert!(execute(&device, &other, Command::Serial, 224, &|| Ok(())).is_err());
    assert!(execute(&device, &cap(), Command::Serial, 31, &|| Ok(())).is_err());
    assert!(device.sent.borrow().is_empty());
    let device = transport(vec![reply(2, 224, 0, 134, &[0, 0])]);
    assert!(execute(&device, &cap(), Command::Edition, 224, &|| Ok(())).is_err());
}
#[test]
fn hyperpolling_receiver_cancellation_stops_before_io_and_preserves_uncertain_send() {
    let device = transport(Vec::new());
    assert!(
        execute(&device, &cap(), Command::Serial, 224, &|| anyhow::bail!(
            "canceled"
        ))
        .is_err()
    );
    assert!(device.sent.borrow().is_empty());
    let device = transport(vec![reply(2, 224, 7, 16, &[3])]);
    let checks = Cell::new(0);
    let Reading::Submitted(report) = execute(
        &device,
        &cap(),
        Command::SetIndicator { status: 3 },
        224,
        &|| {
            let n = checks.get() + 1;
            checks.set(n);
            if n == 3 {
                anyhow::bail!("canceled after send");
            }
            Ok(())
        },
    )
    .unwrap() else {
        panic!()
    };
    assert!(report.write_attempted && report.write_sent && !report.acknowledged);
    assert!(report.error.unwrap().contains("canceled after send"));
}

#[test]
fn hyperpolling_receiver_primary_sequence_survives_get_set_and_idle_cancel() {
    let mut sequence = Sequence::default();
    let device = transport(vec![
        reply(2, 224, 7, 144, &[3]),
        reply(2, 225, 7, 16, &[3]),
        reply(2, 2, 0, 70, &[4]),
        reply(2, 227, 0, 191, &[2, 1, 2, 5, 254, 0, 183]),
    ]);
    execute(
        &device,
        &cap(),
        Command::Indicator,
        sequence.allocate(),
        &|| Ok(()),
    )
    .unwrap();
    execute(
        &device,
        &cap(),
        Command::SetIndicator { status: 3 },
        sequence.allocate(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        execute(
            &device,
            &cap(),
            Command::CancelScan,
            sequence.allocate(),
            &|| Ok(())
        )
        .unwrap(),
        Reading::ScanCanceled
    );
    let Reading::Bindings(rows) = execute(
        &device,
        &cap(),
        Command::Bindings,
        sequence.allocate(),
        &|| Ok(()),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(
        rows.iter()
            .map(|row| (row.product_id, row.status))
            .collect::<Vec<_>>(),
        [(517, 1), (183, 254)]
    );
    let packets = device.sent.borrow();
    assert_eq!(
        packets.iter().map(|packet| packet[2]).collect::<Vec<_>>(),
        [224, 225, 2, 227]
    );
    assert_eq!(&packets[2][6..10], &[1, 0, 70, 4]);
    assert_eq!(&packets[3][6..9], &[80, 0, 191]);
    for _ in 4..31 {
        sequence.allocate();
    }
    assert_eq!(sequence.allocate(), 224);
}

#[test]
fn hyperpolling_receiver_binding_count_and_records_are_required() {
    for data in [vec![], vec![1, 1, 2]] {
        let device = transport(vec![reply(2, 224, 0, 191, &data)]);
        assert!(execute(&device, &cap(), Command::Bindings, 224, &|| Ok(())).is_err());
    }
}
