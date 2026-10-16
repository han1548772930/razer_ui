//! Scripted KS transport only: no COM, process, DLL or real device access.
use super::*;
use std::cell::Cell;

const E_FAIL: i32 = 0x8000_4005u32 as i32;
const REQUEST: CameraFamilyFocusRequest = CameraFamilyFocusRequest::SetFdafMode {
    mode: 2,
    light_condition: 3,
};
fn call(hresult: i32, bytes_returned: u32) -> CameraKsCallOutcome {
    CameraKsCallOutcome {
        hresult,
        bytes_returned,
    }
}
#[derive(Clone)]
struct Node {
    open_error: bool,
    set: CameraKsCallOutcome,
    get: CameraKsCallOutcome,
    response: [u8; 8],
    error: Option<CameraFamilyFocusPhase>,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            open_error: false,
            set: call(0, 0),
            get: call(0, 8),
            response: [0, 0x99, 2, 0, 0, 0, 0, 3],
            error: None,
        }
    }
}
struct Mock {
    count: u32,
    topology_error: bool,
    nodes: Vec<Node>,
    current: Option<usize>,
    events: Vec<String>,
    packets: Vec<([u8; 32], [u8; 8])>,
    closed: bool,
}
impl Mock {
    fn new(nodes: Vec<Node>) -> Self {
        Self {
            count: nodes.len() as u32,
            topology_error: false,
            nodes,
            current: None,
            events: vec![],
            packets: vec![],
            closed: false,
        }
    }
}
impl CameraFamilyFocusTransport for Mock {
    fn begin_topology(&mut self) -> Result<u32> {
        self.events.push("topology".into());
        if self.topology_error {
            anyhow::bail!("simulated topology error");
        }
        Ok(self.count)
    }
    fn begin_node(&mut self, node: u32) -> Result<()> {
        assert!(self.current.is_none());
        self.current = Some(node as usize);
        self.events.push(format!("open {node}"));
        if self.nodes[node as usize].open_error {
            anyhow::bail!("simulated node-open error");
        }
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        let i = self.current.unwrap();
        let node = self.nodes[i].clone();
        let phase = if header[20] == 2 {
            CameraFamilyFocusPhase::Set
        } else {
            CameraFamilyFocusPhase::Get
        };
        self.events.push(format!("{phase:?} {i}"));
        self.packets.push((*header, *data));
        if phase == CameraFamilyFocusPhase::Get {
            *data = node.response;
        }
        if node.error == Some(phase) {
            anyhow::bail!("simulated {phase:?} error");
        }
        Ok(if phase == CameraFamilyFocusPhase::Set {
            node.set
        } else {
            node.get
        })
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        assert_eq!(milliseconds, 10);
        let i = self.current.unwrap();
        self.events.push(format!("wait {i}"));
        if self.nodes[i].error == Some(CameraFamilyFocusPhase::Wait) {
            anyhow::bail!("simulated wait error");
        }
        Ok(())
    }
    fn end_node(&mut self) {
        if let Some(i) = self.current.take() {
            self.events.push(format!("close {i}"));
        }
    }
    fn end_topology(&mut self) {
        assert!(self.current.is_none());
        self.events.push("close topology".into());
        self.closed = true;
    }
}
fn execute(mock: &mut Mock) -> CameraFamilyFocusProgress {
    let p = execute_camera_family_focus(mock, 3595, REQUEST, &|| Ok(()));
    validate_camera_family_focus_progress(3595, REQUEST, &p).unwrap();
    p
}

#[test]
fn camera_family_focus_packets_are_independent_low_byte_native_fixtures() {
    let base = REQUEST.command(3595).unwrap();
    assert_eq!(base.bytes, [0xc0, 0x0a, 1, 2, 0, 0, 0, 3]);
    assert_eq!(base.native_rva, 0x4da20);
    let x = REQUEST.command(3596).unwrap();
    assert_eq!(x.bytes, [0x2e, 0, 1, 0, 0, 0, 0, 0]);
    assert_eq!(x.native_rva, 0x53c80);
    let narrowed = CameraFamilyFocusRequest::SetFdafMode {
        mode: 257,
        light_condition: -1,
    };
    assert_eq!(
        narrowed.command(3595).unwrap().bytes,
        [0xc0, 0x0a, 1, 1, 0, 0, 0, 255]
    );
    assert_eq!(
        narrowed.command(3596).unwrap().bytes,
        [0x2e, 0, 0, 0, 0, 0, 0, 0]
    );
    for pid in [3595, 3596] {
        let tracking = CameraFamilyFocusRequest::SetAutoFocusLowSensitivity { value: 258 }
            .command(pid)
            .unwrap();
        assert_eq!(tracking.bytes, [255, 6, 2, 0, 0, 0, 0, 0]);
        assert_eq!(tracking.native_rva, 0x4d600);
    }
}
#[test]
fn camera_family_autoframing_uses_each_current_vtable_and_shared_isp_lifecycle() {
    for (pid, request, bytes, rva) in [
        (
            3595,
            CameraFamilyFocusRequest::SetAutoframing { mode: 258 },
            [1, 0, 1, 2, 0, 0, 0, 0],
            0x4e380,
        ),
        (
            3596,
            CameraFamilyFocusRequest::SetAutoframing { mode: -1 },
            [0x2b, 0, 255, 0, 0, 0, 0, 0],
            0x54fd0,
        ),
        (
            3595,
            CameraFamilyFocusRequest::SetAutoframingSpeedAndSize {
                speed: 258,
                size: -1,
            },
            [0xc0, 0x0e, 0x0d, 2, 255, 0, 0, 0],
            0x4e570,
        ),
    ] {
        let mut mock = Mock::new(vec![Node::default()]);
        let progress = execute_camera_family_focus(&mut mock, pid, request, &|| Ok(()));
        validate_camera_family_focus_progress(pid, request, &progress).unwrap();
        assert!(progress.completed && mock.closed);
        assert_eq!(progress.command.unwrap().bytes, bytes);
        assert_eq!(progress.command.unwrap().native_rva, rva);
        assert_eq!(mock.packets[0].1, bytes);
    }
}
#[test]
fn camera_family_focus_headers_have_distinct_guid_and_set_get_selectors() {
    for (pid, guid, set_id, get_id) in [
        (
            3595,
            [
                0xd0, 0x9e, 0xe4, 0x23, 0x78, 0x11, 0x31, 0x4f, 0xae, 0x52, 0xd2, 0xfb, 0x8a, 0x8d,
                0x3b, 0x48,
            ],
            1u32,
            2u32,
        ),
        (
            3596,
            [
                0xc3, 0x85, 0xb8, 0x0f, 0xc2, 0x68, 0x47, 0x45, 0x90, 0xf7, 0x8f, 0x47, 0x57, 0x9d,
                0x95, 0xfc,
            ],
            5,
            5,
        ),
    ] {
        for (is_set, property, flags) in [
            (true, set_id, 0x1000_0002u32),
            (false, get_id, 0x1000_0001u32),
        ] {
            let h = camera_family_focus_header(pid, 7, is_set).unwrap();
            assert_eq!(h[..16], guid);
            assert_eq!(h[16..20], property.to_le_bytes());
            assert_eq!(h[20..24], flags.to_le_bytes());
            assert_eq!(h[24..28], [7, 0, 0, 0]);
            assert_eq!(h[28..32], [0; 4]);
        }
    }
}
#[test]
fn camera_family_focus_refuses_other_products_before_transport() {
    for pid in [3594, 3597, 0] {
        let mut mock = Mock::new(vec![Node::default()]);
        let p = execute_camera_family_focus(&mut mock, pid, REQUEST, &|| Ok(()));
        assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Target);
        assert!(p.command.is_none() && mock.events.is_empty());
        assert!(camera_family_focus_header(pid, 0, true).is_err());
    }
}
#[test]
fn camera_family_focus_rejects_zero_or_excessive_node_counts_and_cleans_up() {
    for count in [0, 257, u32::MAX] {
        let mut mock = Mock::new(vec![]);
        mock.count = count;
        let p = execute(&mut mock);
        assert_eq!(p.topology_nodes, Some(count));
        assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Topology);
        assert_eq!(mock.events, ["topology", "close topology"]);
    }
}
#[test]
fn camera_family_focus_ack_retains_raw_response_without_synthesized_getter() {
    for pid in [3595, 3596] {
        let mut mock = Mock::new(vec![Node::default()]);
        let p = execute_camera_family_focus(&mut mock, pid, REQUEST, &|| Ok(()));
        validate_camera_family_focus_progress(pid, REQUEST, &p).unwrap();
        assert!(p.completed && p.failure.is_none());
        assert_eq!(p.attempts[0].response, Some([0, 0x99, 2, 0, 0, 0, 0, 3]));
        assert!(p.attempts[0].acknowledged);
        assert_eq!(mock.packets[0].1, REQUEST.command(pid).unwrap().bytes);
        assert_eq!(mock.packets[1].1, [255, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            mock.events,
            [
                "topology",
                "open 0",
                "Set 0",
                "wait 0",
                "Get 0",
                "close 0",
                "close topology"
            ]
        );
    }
}
#[test]
fn camera_family_focus_scans_after_failed_get_and_keeps_every_possible_write() {
    let mut mock = Mock::new(vec![
        Node {
            open_error: true,
            ..Default::default()
        },
        Node {
            get: call(E_FAIL, 3),
            ..Default::default()
        },
        Node {
            get: call(1, 8),
            ..Default::default()
        },
        Node::default(),
    ]);
    let p = execute(&mut mock);
    assert!(p.completed);
    assert_eq!(p.attempts.len(), 4);
    assert!(!p.attempts[0].set_attempted);
    for i in [1, 2] {
        assert!(p.attempts[i].set_attempted && p.attempts[i].get_attempted);
        assert!(!p.attempts[i].acknowledged);
        assert_eq!(p.attempts[i].response, Some(mock.nodes[i].response));
    }
    assert!(p.attempts[3].acknowledged);
    assert_eq!(mock.packets.len(), 6);
    assert_eq!(
        mock.events
            .iter()
            .filter(|e| e.starts_with("close "))
            .count(),
        5
    );
}
#[test]
fn camera_family_focus_exhaustion_is_failure_with_unconfirmed_sets() {
    let mut mock = Mock::new(vec![
        Node {
            get: call(1, 8),
            ..Default::default()
        },
        Node {
            open_error: true,
            ..Default::default()
        },
    ]);
    let p = execute(&mut mock);
    assert!(!p.completed);
    assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Exhausted);
    assert_eq!(p.attempts.len(), 2);
    assert!(p.attempts[0].set_attempted && !p.attempts[0].acknowledged);
    assert!(mock.closed);
}
#[test]
fn camera_family_focus_negative_set_stops_before_any_get_or_later_node() {
    let mut mock = Mock::new(vec![
        Node {
            set: call(E_FAIL, 0),
            ..Default::default()
        },
        Node::default(),
    ]);
    let p = execute(&mut mock);
    assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Set);
    assert_eq!(p.attempts.len(), 1);
    assert_eq!(p.attempts[0].set_outcome, Some(call(E_FAIL, 0)));
    assert!(!p.attempts[0].get_attempted);
    assert_eq!(mock.packets.len(), 1);
    let mut mock = Mock::new(vec![Node {
        set: call(1, 7),
        ..Default::default()
    }]);
    assert!(execute(&mut mock).completed);
}
#[test]
fn camera_family_focus_short_and_oversize_replies_cannot_be_acknowledgements() {
    for count in [0, 7, 9, u32::MAX] {
        let mut mock = Mock::new(vec![
            Node {
                get: call(0, count),
                ..Default::default()
            },
            Node::default(),
        ]);
        let p = execute(&mut mock);
        assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Get);
        assert_eq!(p.attempts.len(), 1);
        assert_eq!(p.attempts[0].get_outcome, Some(call(0, count)));
        assert!(!p.attempts[0].acknowledged && mock.closed);
    }
}
#[test]
fn camera_family_focus_nonzero_device_status_stops_node_scan() {
    for status in [1, 2, 0x84, 0x85, 0x86, 255] {
        let mut response = Node::default().response;
        response[0] = status;
        let mut mock = Mock::new(vec![
            Node {
                response,
                ..Default::default()
            },
            Node::default(),
        ]);
        let p = execute(&mut mock);
        assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Decode);
        assert_eq!(p.attempts.len(), 1);
        assert_eq!(p.attempts[0].response.unwrap()[0], status);
        assert!(!p.attempts[0].acknowledged);
    }
}
#[test]
fn camera_family_focus_transport_failure_retains_attempt_and_releases_all_phases() {
    let mut mock = Mock::new(vec![Node::default()]);
    mock.topology_error = true;
    let p = execute(&mut mock);
    assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Topology);
    assert!(p.topology_nodes.is_none() && mock.closed);
    for phase in [
        CameraFamilyFocusPhase::Set,
        CameraFamilyFocusPhase::Wait,
        CameraFamilyFocusPhase::Get,
    ] {
        let mut mock = Mock::new(vec![Node {
            error: Some(phase),
            ..Default::default()
        }]);
        let p = execute(&mut mock);
        assert_eq!(p.failure.unwrap().phase, phase);
        assert!(p.attempts[0].set_attempted && mock.closed);
        if phase == CameraFamilyFocusPhase::Get {
            assert!(p.attempts[0].response.is_some() && p.attempts[0].get_outcome.is_none());
        }
        assert_eq!(
            &mock.events[mock.events.len() - 2..],
            ["close 0", "close topology"]
        );
    }
}
#[test]
fn camera_family_focus_cancel_at_every_boundary_retains_last_ack_or_attempt() {
    // Before topology, before open, before SET, after SET, after wait, after ACK.
    for cancel_check in 1..=6 {
        let mut mock = Mock::new(vec![Node::default()]);
        let calls = Cell::new(0);
        let p = execute_camera_family_focus(&mut mock, 3595, REQUEST, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == cancel_check {
                anyhow::bail!("simulated cancellation");
            }
            Ok(())
        });
        validate_camera_family_focus_progress(3595, REQUEST, &p).unwrap();
        assert!(!p.completed);
        assert_eq!(
            p.failure.as_ref().unwrap().phase,
            CameraFamilyFocusPhase::Validation
        );
        assert_eq!(mock.closed, cancel_check > 1);
        if cancel_check >= 4 {
            assert!(p.attempts[0].set_attempted);
        }
        if cancel_check == 6 {
            assert!(p.attempts[0].acknowledged);
        }
        if cancel_check < 6 {
            assert!(p.attempts.iter().all(|a| !a.acknowledged));
        }
    }
}
#[test]
fn camera_family_focus_cancel_before_next_node_keeps_unconfirmed_prior_node() {
    let mut mock = Mock::new(vec![
        Node {
            get: call(1, 8),
            ..Default::default()
        },
        Node::default(),
    ]);
    let calls = Cell::new(0);
    let p = execute_camera_family_focus(&mut mock, 3595, REQUEST, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 6 {
            anyhow::bail!("cancel next node");
        }
        Ok(())
    });
    validate_camera_family_focus_progress(3595, REQUEST, &p).unwrap();
    assert_eq!(p.failure.unwrap().node_id, Some(1));
    assert_eq!(p.attempts.len(), 1);
    assert!(p.attempts[0].set_attempted && !p.attempts[0].acknowledged && mock.closed);
}
#[test]
fn camera_family_focus_validator_rejects_forged_identity_and_ack_paths() {
    let valid = execute(&mut Mock::new(vec![Node::default()]));
    let mut bad = vec![];
    let mut p = valid.clone();
    p.product_id = 3596;
    bad.push(p);
    let mut p = valid.clone();
    p.request = CameraFamilyFocusRequest::SetAutoFocusLowSensitivity { value: 2 };
    bad.push(p);
    let mut p = valid.clone();
    p.command.as_mut().unwrap().bytes[0] = 0xa0;
    bad.push(p);
    let mut p = valid.clone();
    p.command.as_mut().unwrap().native_rva = 0;
    bad.push(p);
    let mut p = valid.clone();
    p.topology_nodes = None;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].node_id = 3;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].opened = false;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].set_attempted = false;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].set_outcome.as_mut().unwrap().hresult = E_FAIL;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].get_attempted = false;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].get_outcome.as_mut().unwrap().bytes_returned = 7;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].response = None;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].response.as_mut().unwrap()[0] = 1;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts[0].acknowledged = false;
    bad.push(p);
    let mut p = valid.clone();
    p.attempts.push(p.attempts[0].clone());
    bad.push(p);
    let mut p = valid.clone();
    p.failure = Some(CameraFamilyFocusFailure {
        phase: CameraFamilyFocusPhase::Get,
        node_id: Some(0),
        message: "fake".into(),
    });
    bad.push(p);
    for p in bad {
        assert!(
            validate_camera_family_focus_progress(3595, REQUEST, &p).is_err(),
            "accepted {p:?}"
        );
    }
}
#[test]
fn camera_family_focus_validator_rejects_failure_phases_and_continuing_after_ack() {
    let mut failed = execute(&mut Mock::new(vec![Node {
        get: call(1, 8),
        ..Default::default()
    }]));
    for phase in [
        CameraFamilyFocusPhase::Set,
        CameraFamilyFocusPhase::Wait,
        CameraFamilyFocusPhase::Get,
        CameraFamilyFocusPhase::Decode,
        CameraFamilyFocusPhase::Topology,
        CameraFamilyFocusPhase::Target,
    ] {
        let mut p = failed.clone();
        p.failure.as_mut().unwrap().phase = phase;
        assert!(validate_camera_family_focus_progress(3595, REQUEST, &p).is_err());
    }
    failed.failure.as_mut().unwrap().message.clear();
    assert!(validate_camera_family_focus_progress(3595, REQUEST, &failed).is_err());
    let mut p = execute(&mut Mock::new(vec![Node::default()]));
    p.completed = false;
    p.topology_nodes = Some(2);
    p.failure = Some(CameraFamilyFocusFailure {
        phase: CameraFamilyFocusPhase::Validation,
        node_id: Some(1),
        message: "forged next node".into(),
    });
    assert!(validate_camera_family_focus_progress(3595, REQUEST, &p).is_err());
    let mut p = execute(&mut Mock::new(vec![Node {
        open_error: true,
        ..Default::default()
    }]));
    p.failure = Some(CameraFamilyFocusFailure {
        phase: CameraFamilyFocusPhase::Validation,
        node_id: Some(0),
        message: "forged after failed open".into(),
    });
    assert!(validate_camera_family_focus_progress(3595, REQUEST, &p).is_err());
}
