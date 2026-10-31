use super::*;
use crate::camera_extensions::CameraKsCallOutcome;
use std::{cell::Cell, collections::VecDeque, rc::Rc};

struct Wire {
    replies: VecDeque<[u8; 8]>,
    commands: Vec<[u8; 8]>,
    waits: Vec<u32>,
    nodes: usize,
    releases: usize,
    topology_releases: usize,
    cancelled: Rc<Cell<bool>>,
    cancel_after_get: bool,
    fail_wait: bool,
}
impl Wire {
    fn new(replies: &[[u8; 8]]) -> Self {
        Self {
            replies: replies.iter().copied().collect(),
            commands: Vec::new(),
            waits: Vec::new(),
            nodes: 0,
            releases: 0,
            topology_releases: 0,
            cancelled: Rc::new(Cell::new(false)),
            cancel_after_get: false,
            fail_wait: false,
        }
    }
    fn property(&mut self, header: &[u8; 32], data: &mut [u8; 8]) -> Result<CameraKsCallOutcome> {
        let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());
        if flags == 0x1000_0002 {
            self.commands.push(*data);
        } else {
            assert_eq!(flags, 0x1000_0001);
            *data = self.replies.pop_front().unwrap_or([0xff; 8]);
            if self.cancel_after_get {
                self.cancelled.set(true);
            }
        }
        Ok(CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 8,
        })
    }
    fn wait(&mut self, milliseconds: u32) -> Result<()> {
        self.waits.push(milliseconds);
        ensure!(!self.fail_wait, "wait interrupted");
        Ok(())
    }
}
impl CameraExtensionTransport for Wire {
    fn begin_node(&mut self, node: u32) -> Result<()> {
        assert_eq!(node, 3);
        self.nodes += 1;
        Ok(())
    }
    fn end_node(&mut self) {
        self.releases += 1;
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        self.property(header, data)
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        self.wait(milliseconds)
    }
}
impl CameraFamilyFocusTransport for Wire {
    fn begin_topology(&mut self) -> Result<u32> {
        Ok(1)
    }
    fn begin_node(&mut self, node: u32) -> Result<()> {
        assert_eq!(node, 0);
        self.nodes += 1;
        Ok(())
    }
    fn end_node(&mut self) {
        self.releases += 1;
    }
    fn end_topology(&mut self) {
        self.topology_releases += 1;
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        self.property(header, data)
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        self.wait(milliseconds)
    }
}

const REPLIES: [[u8; 8]; 4] = [
    [0, 0, 0, b'A', b'B', b'2', b'4', b'0'],
    [0, 0, 0, b'1', b'R', b'Z', b'0', b'1'],
    [0, 0, 0, b'1', b'2', b'3', b'4', b'5'],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

fn read(state: &mut CameraSerialState, wire: &mut Wire) -> CameraSerialProgress {
    let cancelled = wire.cancelled.clone();
    let validate = || {
        ensure!(!cancelled.get(), "connection cancelled");
        Ok(())
    };
    match state.product_id {
        3594 => state.read_pro(wire, &validate),
        _ => state.read_family(wire, &validate),
    }
}

#[test]
fn all_camera_serial_branches_use_their_source_chunk_order_and_cleanup() {
    for (pid, expected) in [
        (
            3594,
            vec![
                [0xa0, 2, 0, 0, 0, 0, 0, 0],
                [0xa0, 2, 1, 0, 0, 0, 0, 0],
                [0xa0, 2, 2, 0, 0, 0, 0, 0],
            ],
        ),
        (
            3595,
            vec![
                [0xc0, 1, 1, 0, 0, 0, 0, 0],
                [0xc0, 1, 2, 0, 0, 0, 0, 0],
                [0xc0, 1, 3, 0, 0, 0, 0, 0],
            ],
        ),
        (
            3596,
            vec![
                [2, 0, 1, 0, 0, 0, 0, 0],
                [2, 0, 2, 0, 0, 0, 0, 0],
                [2, 0, 3, 0, 0, 0, 0, 0],
                [2, 0, 4, 0, 0, 0, 0, 0],
            ],
        ),
    ] {
        let mut state = CameraSerialState::new(pid).unwrap();
        let mut wire = Wire::new(&REPLIES);
        let result = read(&mut state, &mut wire);
        assert_eq!(result.serial(), Some("AB2401RZ0112345"));
        assert_eq!(result.error(), None);
        assert_eq!(wire.commands, expected);
        assert_eq!(wire.waits, vec![10; expected.len() * 2]);
        assert_eq!(wire.nodes, wire.releases);
        assert_eq!(
            wire.topology_releases,
            if pid == 3594 { 0 } else { expected.len() }
        );
        assert_eq!(result.chunks().len(), expected.len());
        assert!(!result.reused_observation());
        let before = wire.commands.len();
        assert!(read(&mut state, &mut wire).reused_observation());
        assert_eq!(wire.commands.len(), before);
    }
}

#[test]
fn source_partial_serial_preserves_acknowledged_bytes_without_failed_buffer_defaults() {
    for pid in 3594..=3596 {
        let mut wire = Wire::new(&[[0xff; 8], REPLIES[1], [0xff; 8], [0xff; 8]]);
        let mut state = CameraSerialState::new(pid).unwrap();
        let result = read(&mut state, &mut wire);
        assert_eq!(result.serial(), Some("1RZ01"));
        assert_eq!(result.chunks().len(), if pid == 3596 { 4 } else { 3 });
        assert_eq!(read(&mut state, &mut wire).serial(), Some("1RZ01"));
    }
}

#[test]
fn failed_serial_attempts_retain_all_ten_source_retries_and_final_delay() {
    for pid in 3594..=3596 {
        let mut state = CameraSerialState::new(pid).unwrap();
        let mut wire = Wire::new(&[]);
        let result = read(&mut state, &mut wire);
        assert!(result.error().unwrap().contains("ten attempts"));
        assert_eq!(result.serial(), None);
        assert_eq!(wire.commands.len(), if pid == 3596 { 40 } else { 30 });
        assert_eq!(wire.waits.iter().filter(|wait| **wait == 300).count(), 10);
        assert_eq!(wire.waits.last(), Some(&300));
        assert_eq!(wire.nodes, wire.releases);
        assert!(state.observed_bytes.is_none());
    }
}

#[test]
fn cancellation_after_get_retains_diagnostic_bytes_and_releases_without_cache_or_next_set() {
    for pid in 3594..=3596 {
        let mut state = CameraSerialState::new(pid).unwrap();
        let mut wire = Wire::new(&REPLIES);
        wire.cancel_after_get = true;
        let result = read(&mut state, &mut wire);
        assert!(result.error().unwrap().contains("cancelled"));
        assert_eq!(result.serial(), None);
        assert_eq!(wire.commands.len(), 1);
        assert_eq!(wire.nodes, wire.releases);
        assert!(state.observed_bytes.is_none());
        match result.chunks()[0].exchange() {
            SerialExchange::Pro { exchange, .. } => assert_eq!(exchange.response, Some(REPLIES[0])),
            SerialExchange::Family { attempts, .. } => {
                assert_eq!(attempts[0].response, Some(REPLIES[0]))
            }
        }
    }
}

#[test]
fn interrupted_wait_and_wrong_transport_do_not_retry_or_publish_a_serial() {
    for pid in 3594..=3596 {
        let mut state = CameraSerialState::new(pid).unwrap();
        let mut wire = Wire::new(&REPLIES);
        wire.fail_wait = true;
        let result = read(&mut state, &mut wire);
        assert_eq!(result.serial(), None);
        assert!(result.error().unwrap().contains("wait interrupted"));
        assert_eq!(wire.commands.len(), 1);
        assert_eq!(wire.nodes, wire.releases);
    }
    let mut wire = Wire::new(&REPLIES);
    assert!(
        CameraSerialState::new(3595)
            .unwrap()
            .read_pro(&mut wire, &|| Ok(()))
            .error()
            .is_some()
    );
    assert!(
        CameraSerialState::new(3594)
            .unwrap()
            .read_family(&mut wire, &|| Ok(()))
            .error()
            .is_some()
    );
    assert!(wire.commands.is_empty());
}

#[test]
fn cached_observation_requires_live_validation_and_new_session_starts_unobserved() {
    let mut state = CameraSerialState::new(3594).unwrap();
    let mut wire = Wire::new(&REPLIES);
    assert!(read(&mut state, &mut wire).serial().is_some());
    wire.cancelled.set(true);
    assert!(read(&mut state, &mut wire).serial().is_none());
    let mut next = CameraSerialState::new(3594).unwrap();
    let mut failed = Wire::new(&[]);
    assert!(read(&mut next, &mut failed).serial().is_none());
    assert_eq!(failed.commands.len(), 30);
}

#[test]
fn source_string_boundary_retains_empty_ack_and_rejects_invalid_utf8() {
    assert_eq!(serial_text(b"AB\0CD").unwrap(), "AB");
    assert!(serial_text(&[0xff]).is_err());
    let mut state = CameraSerialState::new(3594).unwrap();
    let mut wire = Wire::new(&[[0; 8]; 3]);
    assert_eq!(read(&mut state, &mut wire).serial(), Some(""));
    assert_eq!(wire.commands.len(), 3);
    assert!(read(&mut state, &mut wire).reused_observation());
}
