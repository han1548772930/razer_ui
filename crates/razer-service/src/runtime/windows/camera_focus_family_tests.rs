//! Synthetic COM object tables exercise the actual Windows adapter ABI.
//! No system COM activation, camera enumeration or device operation is used.
use super::*;
use razer_device::camera_focus_family::{
    CameraFamilyFocusPhase, CameraFamilyFocusRequest, execute_camera_family_focus,
    validate_camera_family_focus_progress,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const E_FAIL: i32 = 0x8000_4005u32 as i32;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Filter,
    Topology,
    Control,
}
struct State {
    table: Box<[*const c_void; 11]>,
    product_id: u32,
    nodes: u32,
    qi_hr: i32,
    qi_null: bool,
    count_hr: i32,
    create_failure: Option<u32>,
    create_null: Option<u32>,
    get_skip: Option<u32>,
    set_failure: Option<u32>,
    events: Vec<String>,
    headers: Vec<[u8; 32]>,
    packets: Vec<[u8; 8]>,
}
#[repr(C)]
struct FakeCom {
    table: *const *const c_void,
    state: Rc<RefCell<State>>,
    kind: Kind,
    node: Option<u32>,
}
fn fake(state: &Rc<RefCell<State>>, kind: Kind, node: Option<u32>) -> *mut c_void {
    let table = state.borrow().table.as_ptr();
    Box::into_raw(Box::new(FakeCom {
        table,
        state: state.clone(),
        kind,
        node,
    }))
    .cast()
}
unsafe extern "system" fn query(
    raw: *mut c_void,
    iid: *const GUID,
    output: *mut *mut c_void,
) -> i32 {
    let object = unsafe { &*(raw as *const FakeCom) };
    assert_eq!(object.kind, Kind::Filter);
    let iid = unsafe { &*iid };
    assert_eq!(
        (iid.data1, iid.data2, iid.data3, iid.data4),
        (
            0x720d4ac0,
            0x7533,
            0x11d0,
            [0xa5, 0xd6, 0x28, 0xdb, 4, 0xc1, 0, 0]
        )
    );
    object.state.borrow_mut().events.push("QI topology".into());
    let null = object.state.borrow().qi_null;
    unsafe {
        *output = if null {
            null_mut()
        } else {
            fake(&object.state, Kind::Topology, None)
        };
    }
    object.state.borrow().qi_hr
}
unsafe extern "system" fn release(raw: *mut c_void) -> u32 {
    let object = unsafe { Box::from_raw(raw as *mut FakeCom) };
    object
        .state
        .borrow_mut()
        .events
        .push(format!("release {:?} {:?}", object.kind, object.node));
    0
}
unsafe extern "system" fn count(raw: *mut c_void, nodes: *mut u32) -> i32 {
    let object = unsafe { &*(raw as *const FakeCom) };
    assert_eq!(object.kind, Kind::Topology);
    let mut state = object.state.borrow_mut();
    state.events.push("count slot8".into());
    unsafe {
        *nodes = state.nodes;
    }
    state.count_hr
}
unsafe extern "system" fn create(
    raw: *mut c_void,
    node: u32,
    iid: *const GUID,
    output: *mut *mut c_void,
) -> i32 {
    let object = unsafe { &*(raw as *const FakeCom) };
    assert_eq!(object.kind, Kind::Topology);
    let iid = unsafe { &*iid };
    assert_eq!(
        (iid.data1, iid.data2, iid.data3, iid.data4),
        (
            0x28f54685,
            0x06fd,
            0x11d2,
            [0xb2, 0x7a, 0, 0xa0, 0xc9, 0x22, 0x31, 0x96]
        )
    );
    assert!(node < object.state.borrow().nodes);
    object
        .state
        .borrow_mut()
        .events
        .push(format!("create slot10 {node}"));
    let null = object.state.borrow().create_null == Some(node);
    unsafe {
        *output = if null {
            null_mut()
        } else {
            fake(&object.state, Kind::Control, Some(node))
        };
    }
    if object.state.borrow().create_failure == Some(node) {
        E_FAIL
    } else {
        0
    }
}
unsafe extern "system" fn property(
    raw: *mut c_void,
    header: *mut c_void,
    header_len: u32,
    data: *mut c_void,
    data_len: u32,
    returned: *mut u32,
) -> i32 {
    let object = unsafe { &*(raw as *const FakeCom) };
    assert_eq!(object.kind, Kind::Control);
    assert_eq!((header_len, data_len), (32, 8));
    assert_eq!(header as usize % 4, 0);
    let header = unsafe { *(header as *const [u8; 32]) };
    let data = unsafe { &mut *(data as *mut [u8; 8]) };
    let node = object.node.unwrap();
    assert_eq!(header[24..28], node.to_le_bytes());
    assert_eq!(header[28..32], [0; 4]);
    let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());
    let is_set = flags == 0x1000_0002;
    if !is_set {
        assert_eq!(flags, 0x1000_0001);
    }
    let mut state = object.state.borrow_mut();
    let (guid, property) = if state.product_id == 3595 {
        (
            [
                0xd0, 0x9e, 0xe4, 0x23, 0x78, 0x11, 0x31, 0x4f, 0xae, 0x52, 0xd2, 0xfb, 0x8a, 0x8d,
                0x3b, 0x48,
            ],
            if is_set { 1u32 } else { 2 },
        )
    } else {
        (
            [
                0xc3, 0x85, 0xb8, 0x0f, 0xc2, 0x68, 0x47, 0x45, 0x90, 0xf7, 0x8f, 0x47, 0x57, 0x9d,
                0x95, 0xfc,
            ],
            5,
        )
    };
    assert_eq!(header[..16], guid);
    assert_eq!(header[16..20], property.to_le_bytes());
    state.headers.push(header);
    if is_set {
        state.events.push(format!("SET slot3 {node}"));
        state.packets.push(*data);
        unsafe {
            *returned = 0;
        }
        if state.set_failure == Some(node) { E_FAIL } else { 0 }
    } else {
        state.events.push(format!("GET slot3 {node}"));
        assert_eq!(*data, [255, 0, 0, 0, 0, 0, 0, 0]);
        *data = [0, 0, 2, 0, 0, 0, 0, 3];
        unsafe {
            *returned = 8;
        }
        if state.get_skip == Some(node) { 1 } else { 0 }
    }
}
fn context(product_id: u32, nodes: u32) -> (Rc<RefCell<State>>, Com) {
    let mut table = Box::new([std::ptr::null(); 11]);
    table[0] = query as *const c_void;
    table[2] = release as *const c_void;
    table[3] = property as *const c_void;
    table[8] = count as *const c_void;
    table[10] = create as *const c_void;
    let state = Rc::new(RefCell::new(State {
        table,
        product_id,
        nodes,
        qi_hr: 0,
        qi_null: false,
        count_hr: 0,
        create_failure: None,
        create_null: None,
        get_skip: None,
        set_failure: None,
        events: vec![],
        headers: vec![],
        packets: vec![],
    }));
    let filter = unsafe {
        Com::from_result(fake(&state, Kind::Filter, None), 0, "synthetic filter").unwrap()
    };
    (state, filter)
}
struct NoWait<'a> {
    inner: CameraFamilyKsSession<'a>,
    state: Rc<RefCell<State>>,
}
impl CameraFamilyFocusTransport for NoWait<'_> {
    fn begin_topology(&mut self) -> Result<u32> {
        self.inner.begin_topology()
    }
    fn begin_node(&mut self, node: u32) -> Result<()> {
        self.inner.begin_node(node)
    }
    fn ks_property(&mut self, h: &[u8; 32], d: &mut [u8; 8]) -> Result<CameraKsCallOutcome> {
        self.inner.ks_property(h, d)
    }
    fn end_node(&mut self) {
        self.inner.end_node();
    }
    fn end_topology(&mut self) {
        self.inner.end_topology();
    }
    fn wait_ms(&mut self, ms: u32) -> Result<()> {
        assert_eq!(ms, 10);
        self.state.borrow_mut().events.push("wait 10".into());
        Ok(())
    }
}
const REQUEST: CameraFamilyFocusRequest = CameraFamilyFocusRequest::SetFdafMode {
    mode: 2,
    light_condition: 3,
};

#[test]
fn camera_family_adapter_scans_slot8_count_slot10_nodes_and_slot3_properties() {
    for pid in [3595, 3596] {
        let (state, filter) = context(pid, 3);
        state.borrow_mut().create_failure = Some(0);
        state.borrow_mut().get_skip = Some(1);
        let mut transport = NoWait {
            inner: CameraFamilyKsSession::from_filter(&filter),
            state: state.clone(),
        };
        let p = execute_camera_family_focus(&mut transport, pid, REQUEST, &|| Ok(()));
        validate_camera_family_focus_progress(pid, REQUEST, &p).unwrap();
        assert!(p.completed && p.attempts.len() == 3);
        assert!(p.attempts[0].open_error.is_some());
        assert!(p.attempts[1].set_attempted && !p.attempts[1].acknowledged);
        assert!(p.attempts[2].acknowledged);
        assert_eq!(
            state.borrow().events,
            [
                "QI topology",
                "count slot8",
                "create slot10 0",
                "release Control Some(0)",
                "create slot10 1",
                "SET slot3 1",
                "wait 10",
                "GET slot3 1",
                "release Control Some(1)",
                "create slot10 2",
                "SET slot3 2",
                "wait 10",
                "GET slot3 2",
                "release Control Some(2)",
                "release Topology None"
            ]
        );
        let packet = if pid == 3595 {
            [0xc0, 10, 1, 2, 0, 0, 0, 3]
        } else {
            [46, 0, 1, 0, 0, 0, 0, 0]
        };
        assert_eq!(state.borrow().packets, [packet, packet]);
        drop(transport);
        drop(filter);
        assert_eq!(state.borrow().events.last().unwrap(), "release Filter None");
    }
}
#[test]
fn camera_family_adapter_qi_failure_adopts_nonnull_outputs_and_null_success_fails() {
    for (hr, null, release_expected) in [
        (E_FAIL, false, true),
        (E_FAIL, true, false),
        (0, true, false),
    ] {
        let (state, filter) = context(3595, 1);
        state.borrow_mut().qi_hr = hr;
        state.borrow_mut().qi_null = null;
        let mut transport = CameraFamilyKsSession::from_filter(&filter);
        let p = execute_camera_family_focus(&mut transport, 3595, REQUEST, &|| Ok(()));
        assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Topology);
        assert_eq!(
            state.borrow().events.len(),
            if release_expected { 2 } else { 1 }
        );
        assert!(state.borrow().headers.is_empty());
    }
}
#[test]
fn camera_family_adapter_count_failure_and_bad_counts_release_before_node_creation() {
    for (hr, nodes) in [(E_FAIL, 123), (0, 0), (0, 257)] {
        let (state, filter) = context(3596, nodes);
        state.borrow_mut().count_hr = hr;
        let mut transport = CameraFamilyKsSession::from_filter(&filter);
        let p = execute_camera_family_focus(&mut transport, 3596, REQUEST, &|| Ok(()));
        validate_camera_family_focus_progress(3596, REQUEST, &p).unwrap();
        assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Topology);
        assert_eq!(
            state.borrow().events,
            ["QI topology", "count slot8", "release Topology None"]
        );
    }
}
#[test]
fn camera_family_adapter_null_successful_node_is_skipped_without_property_call() {
    let (state, filter) = context(3595, 1);
    state.borrow_mut().create_null = Some(0);
    let mut transport = CameraFamilyKsSession::from_filter(&filter);
    let p = execute_camera_family_focus(&mut transport, 3595, REQUEST, &|| Ok(()));
    validate_camera_family_focus_progress(3595, REQUEST, &p).unwrap();
    assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Exhausted);
    assert!(p.attempts[0].open_error.is_some() && !p.attempts[0].set_attempted);
    assert_eq!(
        state.borrow().events,
        [
            "QI topology",
            "count slot8",
            "create slot10 0",
            "release Topology None"
        ]
    );
}
#[test]
fn camera_family_adapter_drop_releases_control_before_topology_and_rejects_duplicate_open() {
    let (state, filter) = context(3595, 2);
    {
        let mut transport = CameraFamilyKsSession::from_filter(&filter);
        assert!(transport.begin_node(0).is_err());
        assert_eq!(transport.begin_topology().unwrap(), 2);
        assert!(transport.begin_topology().is_err());
        assert!(transport.begin_node(2).is_err());
        transport.begin_node(1).unwrap();
        assert!(transport.begin_node(0).is_err());
    }
    assert_eq!(
        state.borrow().events,
        [
            "QI topology",
            "count slot8",
            "create slot10 1",
            "release Control Some(1)",
            "release Topology None"
        ]
    );
}
#[test]
fn camera_family_adapter_negative_set_is_retained_and_never_followed_by_get() {
    let (state, filter) = context(3596, 2);
    state.borrow_mut().set_failure = Some(0);
    let mut transport = CameraFamilyKsSession::from_filter(&filter);
    let p = execute_camera_family_focus(&mut transport, 3596, REQUEST, &|| Ok(()));
    validate_camera_family_focus_progress(3596, REQUEST, &p).unwrap();
    assert_eq!(p.failure.unwrap().phase, CameraFamilyFocusPhase::Set);
    assert_eq!(p.attempts[0].set_outcome.unwrap().hresult, E_FAIL);
    assert_eq!(state.borrow().headers.len(), 1);
    assert_eq!(
        &state.borrow().events[3..],
        [
            "SET slot3 0",
            "release Control Some(0)",
            "release Topology None"
        ]
    );
}
#[test]
fn camera_family_adapter_cancellation_preserves_prior_node_write_and_cleanup() {
    let (state, filter) = context(3595, 2);
    state.borrow_mut().get_skip = Some(0);
    let mut transport = NoWait {
        inner: CameraFamilyKsSession::from_filter(&filter),
        state: state.clone(),
    };
    let checks = Cell::new(0);
    let p = execute_camera_family_focus(&mut transport, 3595, REQUEST, &|| {
        checks.set(checks.get() + 1);
        if checks.get() == 6 {
            anyhow::bail!("cancel before next node");
        }
        Ok(())
    });
    validate_camera_family_focus_progress(3595, REQUEST, &p).unwrap();
    assert_eq!(p.failure.unwrap().node_id, Some(1));
    assert!(p.attempts[0].set_attempted && !p.attempts[0].acknowledged);
    let events = state.borrow().events.clone();
    assert_eq!(
        &events[events.len() - 2..],
        ["release Control Some(0)", "release Topology None"]
    );
    assert_eq!(state.borrow().packets.len(), 1);
}

#[test]
fn camera_family_exposure_uses_actual_topology_com_adapter_for_both_families() {
    use razer_device::camera_exposure_family::{
        CameraFamilyExposureRequest as E, CameraFamilyExposureValue as V,
        execute_camera_family_exposure, validate_camera_family_exposure_progress,
    };
    for (pid, request, bytes, value) in [
        (
            3595,
            E::GetIsoSensitivity,
            [0xc0, 9, 8, 0, 0, 0, 0, 0],
            V::IsoSensitivity { iso: 0 },
        ),
        (
            3595,
            E::SetShutterSpeed { value: -2 },
            [0xc0, 9, 5, 255, 255, 255, 254, 0],
            V::Acknowledged { native_value: 0 },
        ),
        (
            3596,
            E::SetAutoExposureMetering { mode: 5 },
            [0x25, 0, 3, 0, 0, 0, 0, 0],
            V::Acknowledged { native_value: 3 },
        ),
        (
            3596,
            E::SetShutterSpeed { value: -258 },
            [0x27, 0, 254, 0, 0, 0, 0, 0],
            V::Acknowledged { native_value: -258 },
        ),
        (
            3596,
            E::GetShutterSpeed,
            [7, 0, 0, 0, 0, 0, 0, 0],
            V::ShutterSpeed { value: 2 },
        ),
        (
            3596,
            E::SetEvCompensation { value: 30 },
            [0xc0, 0x0e, 5, 30, 0, 0, 0, 0],
            V::Acknowledged { native_value: 0 },
        ),
    ] {
        let (state, filter) = context(pid, 3);
        state.borrow_mut().create_failure = Some(0);
        state.borrow_mut().get_skip = Some(1);
        let mut transport = NoWait {
            inner: CameraFamilyKsSession::from_filter(&filter),
            state: state.clone(),
        };
        let p = execute_camera_family_exposure(&mut transport, pid, request, &|| Ok(()));
        validate_camera_family_exposure_progress(pid, request, &p).unwrap();
        assert!(p.completed && p.attempts[1].set_attempted && !p.attempts[1].acknowledged);
        assert!(p.attempts[2].acknowledged);
        assert_eq!(p.value, Some(value));
        assert_eq!(state.borrow().packets, [bytes, bytes]);
        let events = state.borrow().events.clone();
        assert_eq!(
            &events[events.len() - 2..],
            ["release Control Some(2)", "release Topology None"]
        );
    }
}

#[test]
fn camera_family_exposure_vendor_iso_noop_does_not_query_com() {
    use razer_device::camera_exposure_family::{
        CameraFamilyExposureRequest as E, execute_camera_family_exposure,
        validate_camera_family_exposure_progress,
    };
    for request in [E::GetIsoSensitivity, E::SetIsoSensitivity { code: 3 }] {
        let (state, filter) = context(3596, 1);
        let mut transport = CameraFamilyKsSession::from_filter(&filter);
        let p = execute_camera_family_exposure(&mut transport, 3596, request, &|| Ok(()));
        validate_camera_family_exposure_progress(3596, request, &p).unwrap();
        assert!(!p.completed && p.command.is_none() && p.value.is_none());
        assert!(state.borrow().events.is_empty());
    }
}
