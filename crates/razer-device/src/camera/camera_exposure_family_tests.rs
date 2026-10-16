//! Scripted transport only. No vendor code, COM activation or device access.
use super::*;
use crate::camera_extensions::CameraKsCallOutcome;
use std::cell::Cell;

const E_FAIL: i32 = 0x8000_4005u32 as i32;
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
    error: Option<CameraFamilyExposurePhase>,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            open_error: false,
            set: call(0, 0),
            get: call(0, 8),
            response: [0, 0x88, 5, 1, 2, 3, 4, 0x99],
            error: None,
        }
    }
}
struct Mock {
    nodes: Vec<Node>,
    node: Option<u32>,
    topology: bool,
    topology_error: bool,
    count: u32,
    events: Vec<String>,
    headers: Vec<[u8; 32]>,
    packets: Vec<[u8; 8]>,
}
impl Mock {
    fn new(nodes: Vec<Node>) -> Self {
        Self {
            count: nodes.len() as u32,
            nodes,
            node: None,
            topology: false,
            topology_error: false,
            events: vec![],
            headers: vec![],
            packets: vec![],
        }
    }
}
impl CameraFamilyExposureTransport for Mock {
    fn begin_topology(&mut self) -> Result<u32> {
        assert!(!self.topology && self.node.is_none());
        self.topology = true;
        self.events.push("topology".into());
        ensure!(!self.topology_error, "scripted topology error");
        Ok(self.count)
    }
    fn begin_node(&mut self, node: u32) -> Result<()> {
        assert!(self.topology && self.node.is_none());
        self.node = Some(node);
        self.events.push(format!("open {node}"));
        ensure!(!self.nodes[node as usize].open_error, "scripted node error");
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        let node = self.node.unwrap();
        let set = header[20] == 2;
        self.events
            .push(format!("{} {node}", if set { "set" } else { "get" }));
        self.headers.push(*header);
        let node = &self.nodes[node as usize];
        let phase = if set {
            CameraFamilyExposurePhase::Set
        } else {
            CameraFamilyExposurePhase::Get
        };
        if set {
            self.packets.push(*data);
        } else {
            assert_eq!(*data, [255, 0, 0, 0, 0, 0, 0, 0]);
            *data = node.response;
        }
        ensure!(node.error != Some(phase), "scripted {phase:?} error");
        Ok(if set { node.set } else { node.get })
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        assert_eq!(milliseconds, 10);
        self.events.push("wait 10".into());
        ensure!(
            self.nodes[self.node.unwrap() as usize].error != Some(CameraFamilyExposurePhase::Wait),
            "scripted wait error"
        );
        Ok(())
    }
    fn end_node(&mut self) {
        self.events.push(format!("close {:?}", self.node.take()));
    }
    fn end_topology(&mut self) {
        assert!(self.node.is_none());
        self.topology = false;
        self.events.push("end topology".into());
    }
}
fn execute(
    mock: &mut Mock,
    pid: u32,
    request: CameraFamilyExposureRequest,
) -> CameraFamilyExposureProgress {
    let p = execute_camera_family_exposure(mock, pid, request, &|| Ok(()));
    validate_camera_family_exposure_progress(pid, request, &p).unwrap();
    assert!(!mock.topology && mock.node.is_none());
    p
}

#[test]
fn family_exposure_commands_match_independent_base_and_x_dispatch() {
    use CameraFamilyExposureRequest::*;
    let cases = [
        (
            3595,
            GetIsoSensitivity,
            [0xc0, 9, 8, 0, 0, 0, 0, 0],
            false,
            0x4d990,
        ),
        (
            3595,
            SetIsoSensitivity { code: 258 },
            [0xc0, 9, 1, 2, 0, 0, 0, 0],
            true,
            0x4d910,
        ),
        (
            3595,
            GetAutoExposureMetering,
            [0xc0, 0x0e, 4, 0x10, 0, 0, 0, 0],
            false,
            0x4de10,
        ),
        (
            3595,
            SetAutoExposureMetering { mode: -1 },
            [0xc0, 0x0e, 4, 255, 0, 0, 0, 0],
            true,
            0x4dd90,
        ),
        (
            3595,
            GetShutterSpeed,
            [0xc0, 9, 6, 0, 0, 0, 0, 0],
            false,
            0x4df40,
        ),
        (
            3595,
            SetShutterSpeed { value: 0x12345678 },
            [0xc0, 9, 5, 0x12, 0x34, 0x56, 0x78, 0],
            true,
            0x4de80,
        ),
        (
            3595,
            SetShutterSpeed { value: -2 },
            [0xc0, 9, 5, 255, 255, 255, 254, 0],
            true,
            0x4de80,
        ),
        (
            3595,
            GetEvCompensation,
            [0xc0, 0x0e, 6, 0, 0, 0, 0, 0],
            false,
            0x4e050,
        ),
        (
            3595,
            SetEvCompensation { value: 258 },
            [0xc0, 0x0e, 5, 2, 0, 0, 0, 0],
            true,
            0x4dfd0,
        ),
        (
            3596,
            GetAutoExposureMetering,
            [5, 0, 0, 0, 0, 0, 0, 0],
            false,
            0x54760,
        ),
        (
            3596,
            SetAutoExposureMetering { mode: 256 },
            [0x25, 0, 2, 0, 0, 0, 0, 0],
            true,
            0x54510,
        ),
        (
            3596,
            SetAutoExposureMetering { mode: 1 },
            [0x25, 0, 1, 0, 0, 0, 0, 0],
            true,
            0x54510,
        ),
        (
            3596,
            SetAutoExposureMetering { mode: 261 },
            [0x25, 0, 3, 0, 0, 0, 0, 0],
            true,
            0x54510,
        ),
        (
            3596,
            GetShutterSpeed,
            [7, 0, 0, 0, 0, 0, 0, 0],
            false,
            0x54a40,
        ),
        (
            3596,
            SetShutterSpeed { value: -2 },
            [0x27, 0, 254, 0, 0, 0, 0, 0],
            true,
            0x548d0,
        ),
        (
            3596,
            GetEvCompensation,
            [0xc0, 0x0e, 6, 0, 0, 0, 0, 0],
            false,
            0x4e050,
        ),
        (
            3596,
            SetEvCompensation { value: 30 },
            [0xc0, 0x0e, 5, 30, 0, 0, 0, 0],
            true,
            0x4dfd0,
        ),
    ];
    for (pid, request, bytes, mutates_state, rva) in cases {
        let mut mock = Mock::new(vec![Node::default()]);
        let progress = execute(&mut mock, pid, request);
        assert!(progress.completed);
        assert_eq!(mock.packets, [bytes]);
        assert_eq!(
            progress.command,
            Some(CameraFamilyExposureCommand {
                bytes,
                mutates_state,
                native_rva: rva
            })
        );
        for (index, header) in mock.headers.iter().enumerate() {
            assert_eq!(
                *header,
                camera_family_exposure_header(pid, 0, index == 0).unwrap()
            );
        }
    }
}

#[test]
fn family_exposure_rejects_known_vendor_iso_noop_and_invalid_x_modes_before_io() {
    use CameraFamilyExposureRequest::*;
    for (pid, request) in [
        (3596, GetIsoSensitivity),
        (3596, SetIsoSensitivity { code: 3 }),
        (3596, SetAutoExposureMetering { mode: 2 }),
        (3596, SetAutoExposureMetering { mode: -1 }),
        (3594, GetShutterSpeed),
        (3597, GetEvCompensation),
    ] {
        let mut mock = Mock::new(vec![]);
        let p = execute(&mut mock, pid, request);
        assert!(!p.completed && p.value.is_none() && p.command.is_none());
        assert_eq!(p.failure.unwrap().phase, CameraFamilyExposurePhase::Target);
        assert!(mock.events.is_empty());
    }
    assert!(
        GetIsoSensitivity
            .command(3596)
            .unwrap_err()
            .to_string()
            .contains("no-op")
    );
}

#[test]
fn family_exposure_getters_preserve_raw_iso_be32_shutter_and_x_mode_without_mapping() {
    use CameraFamilyExposureRequest::*;
    use CameraFamilyExposureValue as V;
    for (pid, request, value) in [
        (
            3595,
            GetIsoSensitivity,
            V::IsoSensitivity { iso: 0x81020304 },
        ),
        (
            3595,
            GetAutoExposureMetering,
            V::AutoExposureMetering { mode: 0x81 },
        ),
        (
            3595,
            GetShutterSpeed,
            V::ShutterSpeed {
                value: 0x81020304u32 as i32,
            },
        ),
        (3595, GetEvCompensation, V::EvCompensation { value: 0x81 }),
        (
            3596,
            GetAutoExposureMetering,
            V::AutoExposureMetering { mode: 3 },
        ),
        (3596, GetShutterSpeed, V::ShutterSpeed { value: 3 }),
        (3596, GetEvCompensation, V::EvCompensation { value: 0x81 }),
    ] {
        let mut mock = Mock::new(vec![Node {
            response: [0, 0xff, 3, 0x81, 2, 3, 4, 0xee],
            ..Default::default()
        }]);
        let p = execute(&mut mock, pid, request);
        assert_eq!(p.value, Some(value));
        assert!(!p.command.unwrap().mutates_state);
    }
}

#[test]
fn family_exposure_setters_preserve_native_out_without_publishing_getter_state() {
    use CameraFamilyExposureRequest::*;
    for (pid, request, native_value) in [
        (3595, SetIsoSensitivity { code: 2 }, 5),
        (3595, SetAutoExposureMetering { mode: 0 }, 0x01020304 >> 24),
        (3595, SetShutterSpeed { value: -9 }, 0x01020304),
        (3595, SetEvCompensation { value: 30 }, 1),
        (3596, SetAutoExposureMetering { mode: 5 }, 3),
        (3596, SetShutterSpeed { value: -258 }, -258),
        (3596, SetEvCompensation { value: 30 }, 1),
    ] {
        let mut mock = Mock::new(vec![Node::default()]);
        let p = execute(&mut mock, pid, request);
        assert_eq!(
            p.value,
            Some(CameraFamilyExposureValue::Acknowledged { native_value })
        );
        assert!(p.command.unwrap().mutates_state);
    }
}

#[test]
fn family_exposure_retains_attempted_writes_when_scanning_after_failed_get() {
    let mut mock = Mock::new(vec![
        Node {
            open_error: true,
            ..Default::default()
        },
        Node {
            get: call(E_FAIL, 0),
            ..Default::default()
        },
        Node::default(),
    ]);
    let p = execute(
        &mut mock,
        3596,
        CameraFamilyExposureRequest::SetShutterSpeed { value: 258 },
    );
    assert!(p.completed && p.attempts[1].set_attempted && !p.attempts[1].acknowledged);
    assert!(p.attempts[2].acknowledged);
    assert_eq!(p.attempts.len(), 3);
    assert_eq!(
        mock.events,
        [
            "topology",
            "open 0",
            "close Some(0)",
            "open 1",
            "set 1",
            "wait 10",
            "get 1",
            "close Some(1)",
            "open 2",
            "set 2",
            "wait 10",
            "get 2",
            "close Some(2)",
            "end topology"
        ]
    );
}

#[test]
fn family_exposure_failed_set_short_get_and_status_errors_never_publish_values() {
    use CameraFamilyExposurePhase::*;
    let cases = [
        (
            Node {
                set: call(E_FAIL, 0),
                ..Default::default()
            },
            Set,
        ),
        (
            Node {
                get: call(0, 7),
                ..Default::default()
            },
            Get,
        ),
        (
            Node {
                get: call(0, 9),
                ..Default::default()
            },
            Get,
        ),
        (
            Node {
                error: Some(Set),
                ..Default::default()
            },
            Set,
        ),
        (
            Node {
                error: Some(Wait),
                ..Default::default()
            },
            Wait,
        ),
        (
            Node {
                error: Some(Get),
                ..Default::default()
            },
            Get,
        ),
    ];
    for (node, phase) in cases {
        let mut mock = Mock::new(vec![node]);
        let p = execute(
            &mut mock,
            3595,
            CameraFamilyExposureRequest::GetIsoSensitivity,
        );
        assert!(!p.completed && p.value.is_none());
        assert_eq!(p.failure.unwrap().phase, phase);
        if phase == Set {
            assert!(!p.attempts[0].get_attempted);
        }
    }
    for (pid, status) in [
        (3595, 0xff),
        (3595, 0x84),
        (3595, 0x85),
        (3595, 0x86),
        (3596, 1),
        (3596, 2),
    ] {
        let mut mock = Mock::new(vec![Node {
            response: [status, 0, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        }]);
        let p = execute(
            &mut mock,
            pid,
            CameraFamilyExposureRequest::GetEvCompensation,
        );
        assert!(!p.completed && p.value.is_none());
        assert_eq!(p.failure.unwrap().phase, Decode);
    }
}

#[test]
fn family_exposure_cancellation_cleans_up_and_retains_ack_after_last_validation() {
    let request = CameraFamilyExposureRequest::SetShutterSpeed { value: 0x11223344 };
    for cancel_at in 1..=6 {
        let mut mock = Mock::new(vec![Node::default()]);
        let calls = Cell::new(0);
        let p = execute_camera_family_exposure(&mut mock, 3595, request, &|| {
            calls.set(calls.get() + 1);
            ensure!(calls.get() != cancel_at, "cancelled scripted request");
            Ok(())
        });
        validate_camera_family_exposure_progress(3595, request, &p).unwrap();
        assert!(!p.completed && !mock.topology && mock.node.is_none());
        assert_eq!(
            p.failure.as_ref().unwrap().phase,
            CameraFamilyExposurePhase::Validation
        );
        if cancel_at == 6 {
            assert!(p.attempts[0].acknowledged);
            assert_eq!(
                p.value,
                Some(CameraFamilyExposureValue::Acknowledged {
                    native_value: 0x01020304
                })
            );
        } else {
            assert!(p.value.is_none());
        }
        if cancel_at == 1 {
            assert!(mock.events.is_empty());
        }
    }
}

#[test]
fn family_exposure_topology_failures_and_exhaustion_are_explicit_and_cleaned() {
    for count in [0, 257] {
        let mut mock = Mock::new(vec![]);
        mock.count = count;
        let p = execute(
            &mut mock,
            3595,
            CameraFamilyExposureRequest::GetIsoSensitivity,
        );
        assert_eq!(
            p.failure.unwrap().phase,
            CameraFamilyExposurePhase::Topology
        );
        assert_eq!(mock.events, ["topology", "end topology"]);
    }
    let mut mock = Mock::new(vec![Node::default()]);
    mock.topology_error = true;
    let p = execute(
        &mut mock,
        3595,
        CameraFamilyExposureRequest::GetIsoSensitivity,
    );
    assert_eq!(
        p.failure.unwrap().phase,
        CameraFamilyExposurePhase::Topology
    );
    let mut mock = Mock::new(vec![Node {
        get: call(1, 8),
        ..Default::default()
    }]);
    let p = execute(
        &mut mock,
        3595,
        CameraFamilyExposureRequest::GetIsoSensitivity,
    );
    assert!(p.attempts[0].set_attempted && p.value.is_none());
    assert_eq!(
        p.failure.unwrap().phase,
        CameraFamilyExposurePhase::Exhausted
    );
}

#[test]
fn family_exposure_receipt_rejects_tampered_metadata_data_and_fabricated_readings() {
    let request = CameraFamilyExposureRequest::GetIsoSensitivity;
    let mut mock = Mock::new(vec![Node::default()]);
    let original = execute(&mut mock, 3595, request);
    let mut mutations = vec![];
    let mut p = original.clone();
    p.product_id = 3596;
    mutations.push(p);
    let mut p = original.clone();
    p.request = CameraFamilyExposureRequest::GetShutterSpeed;
    mutations.push(p);
    let mut p = original.clone();
    p.command.as_mut().unwrap().native_rva += 1;
    mutations.push(p);
    let mut p = original.clone();
    p.command.as_mut().unwrap().bytes[2] = 1;
    mutations.push(p);
    let mut p = original.clone();
    p.command.as_mut().unwrap().mutates_state = true;
    mutations.push(p);
    let mut p = original.clone();
    p.value = Some(CameraFamilyExposureValue::IsoSensitivity { iso: 0 });
    mutations.push(p);
    let mut p = original.clone();
    p.value = None;
    mutations.push(p);
    let mut p = original.clone();
    p.attempts[0].get_outcome.as_mut().unwrap().bytes_returned = 0;
    mutations.push(p);
    let mut p = original.clone();
    p.attempts[0].response.as_mut().unwrap()[3] = 9;
    mutations.push(p);
    let mut p = original.clone();
    p.attempts[0].response.as_mut().unwrap()[0] = 0xff;
    mutations.push(p);
    let mut p = original.clone();
    p.attempts[0].acknowledged = false;
    mutations.push(p);
    let mut p = original.clone();
    p.attempts.push(p.attempts[0].clone());
    mutations.push(p);
    for p in mutations {
        assert!(validate_camera_family_exposure_progress(3595, request, &p).is_err());
    }
    let mut mock = Mock::new(vec![]);
    let mut p = execute(&mut mock, 3596, request);
    p.completed = true;
    p.failure = None;
    p.value = Some(CameraFamilyExposureValue::IsoSensitivity { iso: 0 });
    assert!(validate_camera_family_exposure_progress(3596, request, &p).is_err());
}
