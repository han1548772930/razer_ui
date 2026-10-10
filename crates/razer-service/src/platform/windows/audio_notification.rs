use super::*;
use crate::audio_util::windows::Com;
use anyhow::{Result, ensure};
use razer_device::audio_notification::EndpointChange;
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::{
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize},
    core::GUID,
};

const ENUMERATOR_CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
const ENUMERATOR_INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const UNKNOWN_INTERFACE: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
const NOTIFICATION_INTERFACE: GUID = GUID::from_u128(0x7991eec9_7e89_4d85_8390_6c703cec60c0);

struct Apartment;
impl Apartment {
    fn open() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(null(), 0) };
        ensure!(
            hr >= 0,
            "notification CoInitializeEx failed: 0x{:08x}",
            hr as u32
        );
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[repr(C)]
struct CallbackVtable {
    query: unsafe extern "system" fn(*mut Callback, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut Callback) -> u32,
    release: unsafe extern "system" fn(*mut Callback) -> u32,
    state: unsafe extern "system" fn(*mut Callback, *const u16, u32) -> i32,
    added: unsafe extern "system" fn(*mut Callback, *const u16) -> i32,
    removed: unsafe extern "system" fn(*mut Callback, *const u16) -> i32,
    default_changed: unsafe extern "system" fn(*mut Callback, i32, i32, *const u16) -> i32,
    property: unsafe extern "system" fn(*mut Callback, *const u16, PropertyKey) -> i32,
}

#[repr(C)]
struct PropertyKey {
    fmtid: GUID,
    pid: u32,
}

#[repr(C)]
struct Callback {
    vtable: &'static CallbackVtable,
    references: AtomicU32,
    queue: Arc<Mutex<NotificationQueue>>,
}

static VTABLE: CallbackVtable = CallbackVtable {
    query,
    add_ref,
    release,
    state,
    added,
    removed,
    default_changed,
    property,
};

fn guid_equal(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

unsafe extern "system" fn query(
    this: *mut Callback,
    iid: *const GUID,
    output: *mut *mut c_void,
) -> i32 {
    if output.is_null() {
        return 0x80004003u32 as i32;
    }
    unsafe { *output = null_mut() };
    if iid.is_null() {
        return 0x80004003u32 as i32;
    }
    if guid_equal(unsafe { &*iid }, &UNKNOWN_INTERFACE)
        || guid_equal(unsafe { &*iid }, &NOTIFICATION_INTERFACE)
    {
        unsafe {
            add_ref(this);
            *output = this.cast()
        };
        0
    } else {
        0x80004002u32 as i32
    }
}

unsafe extern "system" fn add_ref(this: *mut Callback) -> u32 {
    unsafe { &*this }.references.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut Callback) -> u32 {
    let remaining = unsafe { &*this }.references.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        unsafe { drop(Box::from_raw(this)) };
    }
    remaining
}

unsafe fn observe(this: *mut Callback, id: *const u16, change: EndpointChange) -> i32 {
    if id.is_null() {
        return 0;
    }
    let mut length = 0;
    while length < 1_048_576 && unsafe { *id.add(length) } != 0 {
        length += 1;
    }
    if length == 0 {
        return 0;
    }
    if length == 1_048_576 {
        return 0x80070057u32 as i32;
    }
    let Ok(endpoint) = String::from_utf16(unsafe { std::slice::from_raw_parts(id, length) }) else {
        return 0x80070057u32 as i32;
    };
    // A poisoned queue returns an HRESULT; no panic crosses the COM ABI.
    match unsafe { &*this }.queue.lock() {
        Ok(mut queue) => {
            queue.observe(change, &endpoint);
            0
        }
        Err(_) => 0x80004005u32 as i32,
    }
}

unsafe extern "system" fn state(this: *mut Callback, id: *const u16, _: u32) -> i32 {
    unsafe { observe(this, id, EndpointChange::StateChanged) }
}
unsafe extern "system" fn added(this: *mut Callback, id: *const u16) -> i32 {
    unsafe { observe(this, id, EndpointChange::Added) }
}
unsafe extern "system" fn removed(this: *mut Callback, id: *const u16) -> i32 {
    unsafe { observe(this, id, EndpointChange::Removed) }
}
unsafe extern "system" fn default_changed(_: *mut Callback, _: i32, _: i32, _: *const u16) -> i32 {
    0
}
unsafe extern "system" fn property(_: *mut Callback, _: *const u16, _: PropertyKey) -> i32 {
    0
}

pub(super) struct Registration {
    enumerator: Option<Com>,
    callback: *mut Callback,
    registered: bool,
    // Keep COM alive until interfaces and owned callback reference drop.
    _apartment: Apartment,
}

impl Registration {
    pub(super) fn open(queue: Arc<Mutex<NotificationQueue>>) -> Result<Self> {
        let apartment = Apartment::open()?;
        let mut pointer = null_mut();
        let hr = unsafe {
            CoCreateInstance(
                &ENUMERATOR_CLASS,
                null_mut(),
                0x17,
                &ENUMERATOR_INTERFACE,
                &mut pointer,
            )
        };
        let enumerator = Com::from_hresult(
            hr,
            pointer,
            "notification CoCreateInstance(MMDeviceEnumerator)",
        )?;
        ensure!(
            hr == 0,
            "notification CoCreateInstance requires S_OK: 0x{:08x}",
            hr as u32
        );
        let callback = Box::into_raw(Box::new(Callback {
            vtable: &VTABLE,
            references: AtomicU32::new(1),
            queue,
        }));
        let mut owner = Self {
            enumerator: Some(enumerator),
            callback,
            registered: false,
            _apartment: apartment,
        };
        type Register = unsafe extern "system" fn(*mut c_void, *mut Callback) -> i32;
        let enumerator = owner.enumerator.as_ref().expect("owned enumerator");
        let register: Register = unsafe { enumerator.method(6) };
        let hr = unsafe { register(enumerator.raw(), owner.callback) };
        ensure!(
            hr == 0,
            "RegisterEndpointNotificationCallback failed: 0x{:08x}",
            hr as u32
        );
        owner.registered = true;
        Ok(owner)
    }

    pub(super) fn close(&mut self) -> Result<()> {
        if !self.registered {
            return Ok(());
        }
        type Unregister = unsafe extern "system" fn(*mut c_void, *mut Callback) -> i32;
        let enumerator = self.enumerator.as_ref().expect("registered enumerator");
        let unregister: Unregister = unsafe { enumerator.method(7) };
        let hr = unsafe { unregister(enumerator.raw(), self.callback) };
        ensure!(
            hr == 0,
            "UnregisterEndpointNotificationCallback failed: 0x{:08x}",
            hr as u32
        );
        self.registered = false;
        Ok(())
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Source cleanup unregisters and releases the enumerator. Retain
        // a valid callback allocation for any remaining OS reference.
        let _ = self.close();
        self.enumerator.take();
        unsafe { release(self.callback) };
    }
}
