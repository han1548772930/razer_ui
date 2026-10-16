use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Transport {
    replies: RefCell<VecDeque<Vec<u8>>>,
    sent: RefCell<Vec<Vec<u8>>>,
    gets: RefCell<Vec<u8>>,
    reads: Cell<usize>,
}
impl Transport {
    fn new(replies: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().collect()),
            sent: RefCell::new(vec![]),
            gets: RefCell::new(vec![]),
            reads: Cell::new(0),
        }
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Source must use collection-sized SET")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Source must use collection-sized GET")
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.gets.borrow_mut().push(report[0]);
        report.copy_from_slice(self.sent.borrow().last().unwrap());
        report[0] = 93; // The host discards this returned prefix.
        report[1] = 2;
        let data = self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("Unexpected GET");
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(&data);
        self.reads.set(self.reads.get() + 1);
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn peer(pid: u16, status: u8) -> Vec<u8> {
    let [hi, lo] = pid.to_be_bytes();
    vec![1, status, hi, lo]
}
fn fast(mut session: ReceiverSession) -> ReceiverSession {
    session.transport.sleep_between_out_ms = 0;
    session.transport.sleep_between_out_in_ms = 0;
    session.transport.sleep_between_in_ms = 0;
    session.transport.max_retry_in = 1;
    session.transport.max_retry_out = 1;
    session.receiver.sleep_between_out_ms = 0;
    session.receiver.sleep_between_out_in_ms = 0;
    session.receiver.sleep_between_in_ms = 0;
    session.receiver.max_retry_in = 1;
    session.receiver.max_retry_out = 1;
    session
}
fn next(session: &ReceiverSession) -> impl FnMut() -> anyhow::Result<u8> + use<> {
    let prefix = session.transport.transaction_prefix;
    let mut counter = 0;
    move || {
        let value = prefix | counter;
        counter = (counter + 1) % 31;
        Ok(value)
    }
}
fn brightness(session: &ReceiverSession) -> crate::keyboard_settings::KeyboardSettingsCapability {
    let mut cap = crate::keyboard_settings::capability(session.transport.product_id)
        .unwrap()
        .clone();
    cap.transport = session.transport.clone();
    cap
}

#[test]
fn keyboard_own_and_shared_receivers_keep_keyboard_namespace_and_real_confirmation() {
    for (product, radio) in [
        (600, 625),
        (602, 604),
        (631, 635),
        (658, 656),
        (664, 662),
        (697, 698),
        (708, 706),
        (716, 713),
        (717, 730),
        (724, 722),
        (727, 725),
        (756, 757),
    ] {
        for physical in [radio, 241] {
            let session = fast(ReceiverSession::keyboard(product, physical, radio).unwrap());
            let cap = brightness(&session);
            let mut replies = vec![peer(radio as u16, 1), vec![1, cap.all_region, 127]];
            let region = if cap.all_region == 0 {
                replies.push(vec![2, 0, 0, 0, 0]);
                2
            } else {
                cap.all_region
            };
            replies.push(vec![1, region, 127]);
            replies.push(peer(radio as u16, 1));
            let count = replies.len();
            let device = Transport::new(replies);
            let observation = session
                .observe(
                    &device,
                    next(&session),
                    || Ok(()),
                    |next, guard| crate::keyboard_settings::apply(&device, &cap, 50, next, guard),
                )
                .unwrap();
            assert!(observation.reading.verified);
            assert_eq!(observation.reading.observed.percent, 50);
            assert_eq!(observation.receiver_before, vec![(radio as u16, 1)]);
            assert_eq!(observation.receiver_after, observation.receiver_before);
            let sent = device.sent.borrow();
            assert_eq!(sent.len(), count);
            assert!(
                sent.iter()
                    .all(|packet| packet[0] == cap.transport.claim_interface)
            );
            assert_eq!(
                sent.iter().map(|packet| packet[2]).collect::<Vec<_>>(),
                (128..128 + count as u8).collect::<Vec<_>>()
            );
            assert_eq!(&sent[1][6..12], &[3, 15, 4, 1, cap.all_region, 127]);
            assert!(
                device
                    .gets
                    .borrow()
                    .iter()
                    .all(|id| *id == cap.transport.claim_interface)
            );
        }
    }
}

#[test]
fn profile_identity_read_on_shared_master_keeps_both_live_peer_checks() {
    let session = fast(
        crate::mouse_current_dpi::capability(182)
            .unwrap()
            .receiver_session(662, 183)
            .unwrap(),
    );
    let request = crate::mouse_current_dpi::DpiChange {
        stages: None,
        active: None,
        profile_guid: None,
        initialize: true,
    };
    let device = Transport::new([peer(183, 1), b"SOURCE-SERIAL".to_vec(), peer(183, 1)]);
    let observed = crate::mouse_current_dpi::execute_in_session(
        &session,
        &device,
        Some(&request),
        next(&session),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        observed.reading.serial_number.as_deref(),
        Some("SOURCE-SERIAL")
    );
    let sent = device.sent.borrow();
    assert_eq!(
        sent.iter()
            .map(|packet| packet[7..9].to_vec())
            .collect::<Vec<_>>(),
        [[0, 191], [0, 130], [0, 191]]
    );
    assert!(
        sent.iter()
            .all(|packet| packet[0] == session.transport.claim_interface && packet[2] < 31)
    );
}

#[test]
fn current_dpi_radio_keeps_stage_and_transient_queries_in_one_live_peer_session() {
    for (product, peer_id) in [(182, 183), (226, 227), (235, 234), (45066, 45067)] {
        let source = crate::mouse_current_dpi::capability(product).unwrap();
        for master in [peer_id, 241, 662] {
            let session = fast(source.receiver_session(master, peer_id).unwrap());
            let device = Transport::new([
                peer(peer_id as u16, 1),
                vec![1, 1, 1, 1, 3, 32, 3, 32, 0, 0],
                vec![0, 7, 208, 7, 208, 0, 0],
                peer(peer_id as u16, 1),
            ]);
            let observation = crate::mouse_current_dpi::execute_in_session(
                &session,
                &device,
                None,
                next(&session),
                || Ok(()),
            )
            .unwrap();
            assert!(observation.reading.verified);
            assert_eq!(observation.reading.stages.unwrap().records[0].x, 800);
            assert_eq!(observation.reading.active.unwrap().x, 2000);
            let sent = device.sent.borrow();
            assert_eq!(
                sent.iter()
                    .map(|packet| packet[7..9].to_vec())
                    .collect::<Vec<_>>(),
                [[0, 191], [4, 134], [4, 133], [0, 191]]
            );
            assert_eq!(sent[1][9], 1);
            assert_eq!(sent[2][9], 0);
            assert!(
                sent.iter()
                    .all(|packet| packet[0] == session.transport.claim_interface && packet[2] < 31)
            );
            assert_eq!(
                device.gets.borrow().as_slice(),
                &[session.transport.claim_interface; 4]
            );
        }
    }
}

#[test]
fn current_dpi_radio_requires_the_same_online_peer_after_the_actual_task() {
    let session = fast(
        crate::mouse_current_dpi::capability(226)
            .unwrap()
            .receiver_session(241, 227)
            .unwrap(),
    );
    for changed in [peer(227, 0), peer(183, 1), vec![0]] {
        let device = Transport::new([
            peer(227, 1),
            vec![1, 1, 1, 1, 3, 32, 3, 32, 0, 0],
            vec![0, 7, 208, 7, 208, 0, 0],
            changed,
        ]);
        assert!(
            crate::mouse_current_dpi::execute_in_session(
                &session,
                &device,
                None,
                next(&session),
                || Ok(()),
            )
            .is_err()
        );
        assert_eq!(device.sent.borrow().len(), 4);
    }
    let device = Transport::new([peer(227, 1)]);
    assert!(
        crate::mouse_current_dpi::execute_in_session(
            &session,
            &device,
            None,
            next(&session),
            || {
                ensure!(device.reads.get() == 0, "Cancelled after receiver query");
                Ok(())
            },
        )
        .is_err()
    );
    assert_eq!(device.sent.borrow().len(), 1);
}

#[test]
fn keyboard_missing_changed_duplicate_or_offline_peer_never_confirms_a_setting() {
    let session = fast(ReceiverSession::keyboard(664, 241, 662).unwrap());
    let cap = brightness(&session);
    for bad in [
        vec![0],
        peer(664, 1),
        peer(662, 0),
        vec![2, 1, 2, 150, 1, 2, 150],
        vec![2, 1, 2, 150, 1, 2, 152],
    ] {
        let device = Transport::new([bad.clone()]);
        assert!(
            session
                .observe(
                    &device,
                    next(&session),
                    || Ok(()),
                    |next, guard| crate::keyboard_settings::read(&device, &cap, next, guard)
                )
                .is_err()
        );
        assert_eq!(device.sent.borrow().len(), 1);
        let device = Transport::new([peer(662, 1), vec![1, 0, 0, 0, 0], vec![1, 1, 127], bad]);
        assert!(
            session
                .observe(
                    &device,
                    next(&session),
                    || Ok(()),
                    |next, guard| crate::keyboard_settings::read(&device, &cap, next, guard)
                )
                .is_err()
        );
        assert_eq!(device.sent.borrow().len(), 4);
    }
}

#[test]
fn cancellation_after_receiver_or_set_ack_stops_later_io() {
    let session = fast(ReceiverSession::keyboard(600, 625, 625).unwrap());
    let cap = brightness(&session);
    for cancel_after in [1, 2] {
        let device = Transport::new([peer(625, 1), vec![1, 0, 127]]);
        let guard = || {
            ensure!(device.reads.get() < cancel_after, "Canceled");
            Ok(())
        };
        assert!(
            session
                .observe(&device, next(&session), guard, |next, guard| {
                    crate::keyboard_settings::apply(&device, &cap, 50, next, guard)
                })
                .is_err()
        );
        assert_eq!(device.sent.borrow().len(), cancel_after);
    }
}

#[test]
fn dpi_own_and_shared_receiver_keep_mouse_namespace() {
    for physical in [227, 241] {
        let session = fast(ReceiverSession::mouse(226, physical, 227).unwrap());
        let mut cap = crate::mouse_dpi_stages::capability(226).unwrap().clone();
        cap.transport = session.transport.clone();
        let device = Transport::new([
            peer(227, 1),
            vec![1, 1, 1, 1, 3, 32, 3, 32, 0, 0],
            peer(227, 1),
        ]);
        let report = session
            .observe(
                &device,
                next(&session),
                || Ok(()),
                |next, guard| crate::mouse_dpi_stages::read_current(&device, &cap, next, guard),
            )
            .unwrap();
        assert_eq!(report.reading.records[0].x, 800);
        assert!(
            device
                .sent
                .borrow()
                .iter()
                .all(|packet| packet[0] == 3 && packet[2] < 31)
        );
        assert_eq!(*device.gets.borrow(), [3, 3, 3]);
    }
}

#[test]
fn receiver_route_rejects_unproven_physical_or_peer_identity() {
    for (product, physical, peer) in [
        (664, 664, 662),
        (664, 241, 663),
        (664, 241, 625),
        (600, 999, 625),
    ] {
        assert!(ReceiverSession::keyboard(product, physical, peer).is_err());
    }
    for (product, physical, peer) in [
        (226, 226, 227),
        (226, 241, 226),
        (226, 241, 183),
        (226, 756, 227),
    ] {
        assert!(ReceiverSession::mouse(product, physical, peer).is_err());
    }
}
