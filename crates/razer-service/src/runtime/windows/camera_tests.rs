//! Synthetic COM objects only. These tests never initialize COM or open a device.
use super::*;
use std::cell::{Cell, RefCell};

#[repr(C)]
struct FakeCom {
    vtable: *const *const c_void,
    releases: Cell<usize>,
    hr: i32,
    query_hr: i32,
    flags: i32,
    value: i32,
    seen: RefCell<Vec<(u32, i32, u32)>>,
    query_iids: RefCell<Vec<(u32, u16, u16, [u8; 8])>>,
}

unsafe extern "system" fn query(
    pointer: *mut c_void,
    iid: *const GUID,
    output: *mut *mut c_void,
) -> i32 {
    let object = unsafe { &*(pointer as *const FakeCom) };
    let guid = unsafe { &*iid };
    object
        .query_iids
        .borrow_mut()
        .push((guid.data1, guid.data2, guid.data3, guid.data4));
    unsafe { *output = pointer };
    object.query_hr
}
unsafe extern "system" fn release(pointer: *mut c_void) -> u32 {
    let object = unsafe { &*(pointer as *const FakeCom) };
    object.releases.set(object.releases.get() + 1);
    LIFECYCLE.with(|v| v.borrow_mut().push("Release"));
    0
}
unsafe extern "system" fn set(pointer: *mut c_void, property: u32, value: i32, flags: u32) -> i32 {
    let object = unsafe { &*(pointer as *const FakeCom) };
    object.seen.borrow_mut().push((property, value, flags));
    object.hr
}
unsafe extern "system" fn get(
    pointer: *mut c_void,
    property: u32,
    value: *mut i32,
    flags: *mut i32,
) -> i32 {
    let object = unsafe { &*(pointer as *const FakeCom) };
    object.seen.borrow_mut().push((property, 0, 0));
    unsafe {
        *value = object.value;
        *flags = object.flags
    };
    object.hr
}

fn fake(hr: i32) -> ([*const c_void; 6], Box<FakeCom>) {
    let table = [
        query as *const c_void,
        std::ptr::null(),
        release as *const c_void,
        std::ptr::null(),
        set as *const c_void,
        get as *const c_void,
    ];
    let object = Box::new(FakeCom {
        vtable: std::ptr::null(),
        releases: Cell::new(0),
        hr,
        query_hr: 0,
        flags: 3,
        value: -11,
        seen: RefCell::new(vec![]),
        query_iids: RefCell::new(vec![]),
    });
    (table, object)
}
unsafe fn owned(object: &mut FakeCom) -> Com {
    unsafe { Com::from_raw(object as *mut _ as *mut c_void, "test object").unwrap() }
}

#[test]
fn twenty_properties_use_actual_com_slots_and_queried_iids() {
    use razer_device::camera::StandardCameraProperty;
    let (table, mut object) = fake(1);
    object.vtable = table.as_ptr();
    let filter = unsafe { owned(&mut object) };
    for property in StandardCameraProperty::ALL {
        let iid = match property {
            StandardCameraProperty::VideoProcAmp(_) => &IID_IAM_VIDEO_PROC_AMP,
            StandardCameraProperty::CameraControl(_) => &IID_IAM_CAMERA_CONTROL,
        };
        let read = get_property(&filter, iid, property.id(), "fake").unwrap();
        assert_eq!(
            (read.value, read.native_flags, read.native_hresult),
            (-11, 3, 1)
        );
        assert!(!read.is_auto());
        assert_eq!(
            set_property(&filter, iid, property.id(), -9, true, "fake")
                .unwrap()
                .native_hresult,
            1
        );
        set_property(&filter, iid, property.id(), -8, false, "fake").unwrap();
        let seen = object.seen.borrow();
        assert_eq!(
            seen[seen.len() - 3..],
            [
                (property.id(), 0, 0),
                (property.id(), -9, 1),
                (property.id(), -8, 2)
            ]
        );
        let guids = object.query_iids.borrow();
        assert_eq!(
            guids.last(),
            Some(&(iid.data1, iid.data2, iid.data3, iid.data4))
        );
    }
    assert_eq!(object.releases.get(), 60);
    drop(filter);
    assert_eq!(object.releases.get(), 61);
}

#[test]
fn failed_property_operations_release_queried_interfaces() {
    let (table, mut object) = fake(-1);
    object.vtable = table.as_ptr();
    let filter = unsafe { owned(&mut object) };
    assert!(get_property(&filter, &IID_IAM_CAMERA_CONTROL, 4, "fake").is_err());
    assert!(set_property(&filter, &IID_IAM_VIDEO_PROC_AMP, 0, 42, false, "fake").is_err());
    assert_eq!(object.releases.get(), 2);
    drop(filter);
    assert_eq!(object.releases.get(), 3);
}

#[test]
fn failed_set_interface_query_keeps_its_hresult_without_calling_set() {
    let (table, mut object) = fake(0);
    object.query_hr = 0x8000_4002u32 as i32;
    object.vtable = table.as_ptr();
    let filter = unsafe { owned(&mut object) };
    let error = set_property(&filter, &IID_IAM_CAMERA_CONTROL, 4, 42, false, "fake").unwrap_err();
    let rejection = error
        .downcast_ref::<razer_device::camera::CameraSetRejection>()
        .unwrap();
    assert_eq!(rejection.native_hresult, object.query_hr);
    assert_eq!(
        rejection.phase,
        razer_device::camera::CameraSetPhase::QueryInterface
    );
    assert!(object.seen.borrow().is_empty());
    assert_eq!(object.releases.get(), 1);
    drop(filter);
    assert_eq!(object.releases.get(), 2);
}

#[test]
fn nonnull_failure_output_is_released_and_null_success_is_rejected() {
    let (table, mut object) = fake(0);
    object.vtable = table.as_ptr();
    assert!(
        unsafe { Com::from_result(&mut *object as *mut _ as *mut c_void, -1, "failure output") }
            .is_err()
    );
    assert_eq!(object.releases.get(), 1);
    assert!(unsafe { Com::from_result(null_mut(), 0, "null success") }.is_err());
}

thread_local! { static LIFECYCLE: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }
unsafe extern "system" fn fake_uninitialize() {
    LIFECYCLE.with(|v| v.borrow_mut().push("Uninitialize"));
}

#[test]
fn session_releases_filter_before_balancing_apartment() {
    let (table, mut object) = fake(0);
    object.vtable = table.as_ptr();
    let filter = unsafe { owned(&mut object) };
    // Use the actual CameraComSession drop fields, with a synthetic uninitializer.
    LIFECYCLE.with(|v| v.borrow_mut().clear());
    let session = CameraComSession {
        filter,
        _apartment: Apartment {
            uninitialize: fake_uninitialize,
            _thread: PhantomData,
        },
    };
    drop(session);
    assert_eq!(object.releases.get(), 1);
    LIFECYCLE.with(|v| assert_eq!(*v.borrow(), ["Release", "Uninitialize"]));
}
