//! Fake COM vtables exercise the actual adapter ABI without opening Windows
//! devices, running DLLs, or calling the system camera enumerator.
use super::*;
use razer_device::camera_extensions::{
    CAMERA_EXTENSION_GET_FLAGS, CAMERA_EXTENSION_SET_FLAGS, CameraExtensionPhase,
    CameraExtensionRequest, CameraExtensionValue, execute_camera_extension,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Filter,
    Topology,
    Control,
}

#[derive(Default)]
struct State {
    events: Vec<String>,
    header: Vec<[u8; 32]>,
    packets: Vec<[u8; 8]>,
    table: Option<Box<[*const c_void; 11]>>,
    qi_failure: bool,
    create_failure: bool,
}

#[repr(C)]
struct FakeCom {
    table: *const *const c_void,
    state: Rc<RefCell<State>>,
    kind: Kind,
}

fn fake(state: &Rc<RefCell<State>>, kind: Kind) -> *mut c_void {
    let table = state.borrow().table.as_ref().unwrap().as_ptr();
    Box::into_raw(Box::new(FakeCom {
        table,
        state: state.clone(),
        kind,
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
    assert_eq!(iid.data1, 0x720d4ac0);
    assert_eq!(iid.data2, 0x7533);
    assert_eq!(iid.data3, 0x11d0);
    assert_eq!(iid.data4, [0xa5, 0xd6, 0x28, 0xdb, 0x04, 0xc1, 0, 0]);
    object.state.borrow_mut().events.push("QI Topology".into());
    unsafe {
        *output = fake(&object.state, Kind::Topology);
    }
    if object.state.borrow().qi_failure {
        0x8000_4005u32 as i32
    } else {
        0
    }
}

unsafe extern "system" fn release(raw: *mut c_void) -> u32 {
    let object = unsafe { Box::from_raw(raw as *mut FakeCom) };
    object
        .state
        .borrow_mut()
        .events
        .push(format!("Release {:?}", object.kind));
    drop(object);
    0
}

unsafe extern "system" fn create(
    raw: *mut c_void,
    node: u32,
    iid: *const GUID,
    output: *mut *mut c_void,
) -> i32 {
    let object = unsafe { &*(raw as *const FakeCom) };
    assert_eq!(object.kind, Kind::Topology);
    assert_eq!(node, 3);
    let iid = unsafe { &*iid };
    assert_eq!(iid.data1, 0x28f54685);
    assert_eq!(iid.data2, 0x06fd);
    assert_eq!(iid.data3, 0x11d2);
    assert_eq!(iid.data4, [0xb2, 0x7a, 0, 0xa0, 0xc9, 0x22, 0x31, 0x96]);
    object.state.borrow_mut().events.push("CreateNode 3".into());
    unsafe {
        *output = fake(&object.state, Kind::Control);
    }
    if object.state.borrow().create_failure {
        0x8000_4005u32 as i32
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
    assert_eq!(header_len, 32);
    assert_eq!(data_len, 8);
    assert_eq!((header as usize) % 4, 0);
    let header = unsafe { *(header as *const [u8; 32]) };
    let data = unsafe { &mut *(data as *mut [u8; 8]) };
    assert_eq!(&header[..16], b"RAZER_USBCAM_EXT");
    assert_eq!(&header[16..20], &1u32.to_le_bytes());
    assert_eq!(&header[24..28], &3u32.to_le_bytes());
    assert_eq!(&header[28..32], &[0; 4]);
    let flags = u32::from_le_bytes(header[20..24].try_into().unwrap());
    let mut state = object.state.borrow_mut();
    state.header.push(header);
    if flags == CAMERA_EXTENSION_SET_FLAGS {
        state.packets.push(*data);
        state.events.push("KS SET".into());
        unsafe {
            *returned = 0;
        }
    } else {
        assert_eq!(flags, CAMERA_EXTENSION_GET_FLAGS);
        assert_eq!(data[0], 255);
        state.events.push("KS GET".into());
        *data = [0, 0, 0, 1, 0, 0, 0, 0];
        unsafe {
            *returned = 8;
        }
    }
    0
}

fn context(qi_failure: bool, create_failure: bool) -> (Rc<RefCell<State>>, Com) {
    let mut table = Box::new([std::ptr::null(); 11]);
    table[0] = query as *const c_void;
    table[2] = release as *const c_void;
    table[3] = property as *const c_void;
    table[10] = create as *const c_void;
    let state = Rc::new(RefCell::new(State {
        table: Some(table),
        qi_failure,
        create_failure,
        ..Default::default()
    }));
    let filter = unsafe { Com::from_result(fake(&state, Kind::Filter), 0, "fake filter").unwrap() };
    (state, filter)
}

#[test]
fn adapter_uses_topology_slot10_control_slot3_and_releases_in_order() {
    let (state, filter) = context(false, false);
    let mut transport = CameraKsSession::from_filter(&filter);
    // Bypass only the delay in this ABI test by calling the low-level methods.
    transport.begin_node(3).unwrap();
    let mut command = [0xc0, 9, 10, 0, 0, 0, 0, 0];
    let set = transport
        .ks_property(
            &razer_device::camera_extensions::camera_extension_header(true),
            &mut command,
        )
        .unwrap();
    assert_eq!(
        set,
        CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 0
        }
    );
    let mut response = [255, 0, 0, 0, 0, 0, 0, 0];
    let get = transport
        .ks_property(
            &razer_device::camera_extensions::camera_extension_header(false),
            &mut response,
        )
        .unwrap();
    assert_eq!(
        get,
        CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 8
        }
    );
    assert_eq!(response[3], 1);
    transport.end_node();
    assert_eq!(
        state.borrow().events,
        [
            "QI Topology",
            "CreateNode 3",
            "KS SET",
            "KS GET",
            "Release Control",
            "Release Topology"
        ]
    );
    drop(transport);
    drop(filter);
    assert_eq!(state.borrow().events.last().unwrap(), "Release Filter");
}

#[test]
fn failed_query_nonnull_output_is_adopted_and_released() {
    let (state, filter) = context(true, false);
    let mut transport = CameraKsSession::from_filter(&filter);
    let result = execute_camera_extension(
        &mut transport,
        3594,
        CameraExtensionRequest::GetStreamingStatus,
        &|| Ok(()),
    );
    assert_eq!(
        result.failure.unwrap().phase,
        CameraExtensionPhase::OpenControl
    );
    assert_eq!(state.borrow().events, ["QI Topology", "Release Topology"]);
    assert!(state.borrow().packets.is_empty());
}

#[test]
fn failed_create_nonnull_control_releases_before_topology() {
    let (state, filter) = context(false, true);
    let mut transport = CameraKsSession::from_filter(&filter);
    let result = execute_camera_extension(
        &mut transport,
        3594,
        CameraExtensionRequest::SetHdr { enabled: true },
        &|| Ok(()),
    );
    assert_eq!(
        result.failure.unwrap().phase,
        CameraExtensionPhase::OpenControl
    );
    assert_eq!(
        state.borrow().events,
        [
            "QI Topology",
            "CreateNode 3",
            "Release Control",
            "Release Topology"
        ]
    );
    assert!(state.borrow().packets.is_empty());
}

#[test]
fn dropping_open_node_releases_control_then_topology_and_open_rejects_duplicates() {
    let (state, filter) = context(false, false);
    {
        let mut transport = CameraKsSession::from_filter(&filter);
        assert!(transport.begin_node(4).is_err());
        assert!(state.borrow().events.is_empty());
        transport.begin_node(3).unwrap();
        assert!(transport.begin_node(3).is_err());
        assert_eq!(state.borrow().events.len(), 2);
    }
    assert_eq!(
        &state.borrow().events[2..],
        ["Release Control", "Release Topology"]
    );
}

#[test]
fn protocol_executor_and_com_adapter_return_real_fake_response() {
    let (state, filter) = context(false, false);
    let mut transport = CameraKsSession::from_filter(&filter);
    // The sole delay here is the Rust protocol delay; there is no device I/O.
    let result = execute_camera_extension(
        &mut transport,
        3594,
        CameraExtensionRequest::GetStreamingStatus,
        &|| Ok(()),
    );
    assert!(result.completed);
    assert_eq!(
        result.value,
        Some(CameraExtensionValue::StreamingStatus {
            raw_code: 1,
            is_streaming: true
        })
    );
    assert_eq!(state.borrow().packets, [[0xc0, 9, 10, 0, 0, 0, 0, 0]]);
}
