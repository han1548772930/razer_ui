//! Rust DirectShow camera adapter for the current 3594 native property path.
//! Every interface and apartment is owned on the opening thread. No vendor DLL
//! is loaded; the public service entry is crate::camera::CameraDeviceSession.

use anyhow::{Context as _, Result, ensure};
use razer_device::camera::{
    CameraLongObservation, CameraSetting, CameraTarget, CameraWriteAck, DirectShowCameraTransport,
};
use std::{
    ffi::{OsStr, c_void},
    marker::PhantomData,
    os::windows::ffi::OsStrExt,
    ptr::{NonNull, null, null_mut},
    rc::Rc,
};
use windows_sys::{
    Win32::{
        Foundation::SysStringLen,
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CoUninitialize},
            Variant::{VARIANT, VT_BSTR, VariantClear, VariantInit},
        },
    },
    core::GUID,
};

const CLSID_SYSTEM_DEVICE_ENUM: GUID = GUID::from_u128(0x62be5d10_60eb_11d0_bd3b_00a0c911ce86);
const CLSID_VIDEO_INPUT_DEVICE_CATEGORY: GUID =
    GUID::from_u128(0x860bb310_5d01_11d0_bd3b_00a0c911ce86);
const IID_ICREATE_DEV_ENUM: GUID = GUID::from_u128(0x29840822_5b84_11d0_bd3b_00a0c911ce86);
const IID_IPROPERTY_BAG: GUID = GUID::from_u128(0x55272a00_42cb_11ce_8135_00aa004bb851);
const IID_IBASE_FILTER: GUID = GUID::from_u128(0x56a86895_0ad4_11ce_b03a_0020af0ba770);
const IID_IAM_VIDEO_PROC_AMP: GUID = GUID::from_u128(0xc6e13360_30ac_11d0_a18c_00a0c9118956);
const IID_IAM_CAMERA_CONTROL: GUID = GUID::from_u128(0xc6e13370_30ac_11d0_a18c_00a0c9118956);

// The recovered helper uses CLSCTX 3 (in-process server | handler).
const CLSCTX_INPROC: u32 = 3;

struct Apartment {
    uninitialize: unsafe extern "system" fn(),
    _thread: PhantomData<Rc<()>>,
}
impl Apartment {
    fn open() -> Result<Self> {
        Self::open_mode(false)
    }
    fn open_mode(properties: bool) -> Result<Self> {
        let hr = unsafe { CoInitializeEx(null(), if properties { 2 } else { 0 }) };
        ensure!(hr >= 0, "CoInitializeEx failed: 0x{:08x}", hr as u32);
        Ok(Self {
            uninitialize: CoUninitialize,
            _thread: PhantomData,
        })
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { (self.uninitialize)() };
    }
}

pub(super) struct Com {
    pointer: NonNull<c_void>,
    _thread: PhantomData<Rc<()>>,
}
impl Com {
    unsafe fn from_raw(pointer: *mut c_void, operation: &str) -> Result<Self> {
        NonNull::new(pointer)
            .map(|pointer| Self {
                pointer,
                _thread: PhantomData,
            })
            .with_context(|| format!("{operation} returned a null COM interface"))
    }
    /// Adopt non-null failure outputs too; all error branches release them.
    pub(super) unsafe fn from_result(
        pointer: *mut c_void,
        hr: i32,
        operation: &str,
    ) -> Result<Self> {
        let owned = if pointer.is_null() {
            None
        } else {
            Some(unsafe { Self::from_raw(pointer, operation)? })
        };
        ensure!(hr >= 0, "{operation} failed: 0x{:08x}", hr as u32);
        owned.with_context(|| format!("{operation} returned a null COM interface"))
    }
    pub(super) fn raw(&self) -> *mut c_void {
        self.pointer.as_ptr()
    }
    /// Caller supplies the ABI and a valid slot for the queried interface.
    pub(super) unsafe fn method<T: Copy>(&self, slot: usize) -> T {
        assert_eq!(
            std::mem::size_of::<T>(),
            std::mem::size_of::<*const c_void>()
        );
        let table = unsafe { *(self.raw() as *const *const *const c_void) };
        let address = unsafe { *table.add(slot) };
        unsafe { std::mem::transmute_copy(&address) }
    }
    pub(super) fn query(&self, iid: &GUID, name: &str) -> Result<Self> {
        type QueryInterface =
            unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
        let query: QueryInterface = unsafe { self.method(0) };
        let mut output = null_mut();
        let hr = unsafe { query(self.raw(), iid, &mut output) };
        unsafe { Self::from_result(output, hr, name) }
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        let release: Release = unsafe { self.method(2) };
        unsafe { release(self.raw()) };
    }
}
fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

struct CameraVariant(VARIANT);
impl CameraVariant {
    fn new() -> Self {
        let mut value = VARIANT::default();
        unsafe { VariantInit(&mut value) };
        Self(value)
    }
}
impl Drop for CameraVariant {
    fn drop(&mut self) {
        unsafe { VariantClear(&mut self.0) };
    }
}
fn variant_text(value: &VARIANT) -> Result<String> {
    let raw = unsafe { value.Anonymous.Anonymous };
    ensure!(
        raw.vt == VT_BSTR,
        "DevicePath is not a BSTR variant (vt={})",
        raw.vt
    );
    let pointer = unsafe { raw.Anonymous.bstrVal };
    ensure!(!pointer.is_null(), "DevicePath variant is null");
    let len = unsafe { SysStringLen(pointer) } as usize;
    ensure!(len <= 32_768, "DevicePath is too long");
    String::from_utf16(unsafe { std::slice::from_raw_parts(pointer, len) })
        .context("DevicePath is not valid UTF-16")
}
fn camera_monikers() -> Result<Vec<(String, Com)>> {
    let mut enumerator = null_mut();
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_SYSTEM_DEVICE_ENUM,
            null_mut(),
            CLSCTX_INPROC,
            &IID_ICREATE_DEV_ENUM,
            &mut enumerator,
        )
    };
    let enumerator =
        unsafe { Com::from_result(enumerator, hr, "CoCreateInstance(SystemDeviceEnum)")? };
    type CreateClassEnumerator =
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void, u32) -> i32;
    let create: CreateClassEnumerator = unsafe { enumerator.method(3) };
    let mut monikers = null_mut();
    let hr = unsafe {
        create(
            enumerator.raw(),
            &CLSID_VIDEO_INPUT_DEVICE_CATEGORY,
            &mut monikers,
            0,
        )
    };
    let monikers = unsafe { Com::from_result(monikers, hr, "CreateClassEnumerator")? };
    type Next = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, *mut u32) -> i32;
    type Bind = unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const GUID,
        *mut *mut c_void,
    ) -> i32;
    type Read =
        unsafe extern "system" fn(*mut c_void, *const u16, *mut VARIANT, *mut c_void) -> i32;
    let next: Next = unsafe { monikers.method(3) };
    let property = wide("DevicePath");
    let mut found = Vec::new();
    loop {
        let mut raw_moniker = null_mut();
        let mut fetched = 0u32;
        let hr = unsafe { next(monikers.raw(), 1, &mut raw_moniker, &mut fetched) };
        let moniker = if raw_moniker.is_null() {
            None
        } else {
            Some(unsafe { Com::from_raw(raw_moniker, "IMoniker")? })
        };
        ensure!(hr >= 0, "IEnumMoniker::Next failed: 0x{:08x}", hr as u32);
        if hr == 1 && fetched == 0 {
            break;
        }
        ensure!(
            hr == 0 && fetched == 1,
            "IEnumMoniker::Next returned inconsistent status/count"
        );
        let moniker = moniker.context("IEnumMoniker::Next returned null")?;
        let bind_storage: Bind = unsafe { moniker.method(9) };
        let mut bag = null_mut();
        let hr = unsafe {
            bind_storage(
                moniker.raw(),
                null_mut(),
                null_mut(),
                &IID_IPROPERTY_BAG,
                &mut bag,
            )
        };
        let bag = unsafe { Com::from_result(bag, hr, "IMoniker::BindToStorage(IPropertyBag)")? };
        let read: Read = unsafe { bag.method(3) };
        let mut value = CameraVariant::new();
        let hr = unsafe { read(bag.raw(), property.as_ptr(), &mut value.0, null_mut()) };
        // Non-USB/virtual cameras can omit this property. Successful reads are
        // parsed strictly, so a corrupt UTF-16 or VARIANT cannot hide a match.
        if hr < 0 {
            continue;
        }
        let observed = variant_text(&value.0)?;
        found.push((observed, moniker));
    }
    Ok(found)
}
fn bind_camera(path: &str) -> Result<Com> {
    type Bind = unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const GUID,
        *mut *mut c_void,
    ) -> i32;
    let matches: Vec<_> = camera_monikers()?
        .into_iter()
        .filter(|(observed, _)| observed.eq_ignore_ascii_case(path))
        .collect();
    ensure!(
        matches.len() == 1,
        "camera DevicePath is missing or has duplicate monikers"
    );
    // Complete enumeration before binding prevents choosing the first duplicate.
    let (_, moniker) = matches.into_iter().next().unwrap();
    let bind_object: Bind = unsafe { moniker.method(8) };
    let mut filter = null_mut();
    let hr = unsafe {
        bind_object(
            moniker.raw(),
            null_mut(),
            null_mut(),
            &IID_IBASE_FILTER,
            &mut filter,
        )
    };
    unsafe { Com::from_result(filter, hr, "IMoniker::BindToObject(IBaseFilter)") }
}

pub(super) fn discover(
    scope: &razer_device::camera_submission::CameraScope,
) -> Result<CameraTarget> {
    scope.validate()?;
    let _apartment = Apartment::open()?;
    let mut found = Vec::new();
    for (path, _moniker) in camera_monikers()? {
        let Ok(target) = CameraTarget::new(scope.product_id, path) else {
            continue;
        };
        // PID, full DirectShow link and the OS ContainerId are independently checked.
        if super::identity::container_id(&target.device_path)?
            .eq_ignore_ascii_case(&scope.container_id)
        {
            found.push(target);
        }
    }
    ensure!(
        found.len() == 1,
        "camera has no unique DirectShow path for its current ContainerId"
    );
    Ok(found.pop().unwrap())
}

/// Fields drop in declaration order: references release before CoUninitialize.
pub(super) struct CameraComSession {
    pub(super) filter: Com,
    _apartment: Apartment,
}
impl CameraComSession {
    pub(super) fn open(target: &CameraTarget) -> Result<Self> {
        Self::open_mode(target, false)
    }
    pub(super) fn open_mode(target: &CameraTarget, properties: bool) -> Result<Self> {
        target.validate()?;
        let apartment = Apartment::open_mode(properties)?;
        let filter = bind_camera(&target.device_path)?;
        Ok(Self {
            filter,
            _apartment: apartment,
        })
    }
}

#[path = "camera_properties.rs"]
mod properties;
fn get_property(
    filter: &Com,
    iid: &GUID,
    property_id: u32,
    name: &str,
) -> Result<CameraLongObservation> {
    type Get = unsafe extern "system" fn(*mut c_void, u32, *mut i32, *mut i32) -> i32;
    let interface = filter.query(iid, name)?;
    let get: Get = unsafe { interface.method(5) }; // native +40 slot
    let mut value = 0i32;
    let mut flags = 0i32;
    let hr = unsafe { get(interface.raw(), property_id, &mut value, &mut flags) };
    CameraLongObservation::from_native(value, flags, hr)
        .with_context(|| format!("{name}::Get({property_id})"))
}
fn set_property(
    filter: &Com,
    iid: &GUID,
    property_id: u32,
    value: i32,
    is_auto: bool,
    name: &str,
) -> Result<CameraWriteAck> {
    type Set = unsafe extern "system" fn(*mut c_void, u32, i32, u32) -> i32;
    type QueryInterface =
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let query: QueryInterface = unsafe { filter.method(0) };
    let mut output = null_mut();
    let query_hr = unsafe { query(filter.raw(), iid, &mut output) };
    let interface = unsafe { Com::from_result(output, query_hr, name) }.map_err(|error| {
        if query_hr < 0 {
            // The native setter returns false when QI fails, before Set is
            // called. Keep the actual QI HRESULT and release any failure output;
            // a source compound runner can still attempt its later methods.
            anyhow::Error::from(razer_device::camera::CameraSetRejection {
                native_hresult: query_hr,
                phase: razer_device::camera::CameraSetPhase::QueryInterface,
            })
            .context(format!("{name} QueryInterface before Set: {error:#}"))
        } else {
            error
        }
    })?;
    let set: Set = unsafe { interface.method(4) }; // native +32 slot
    let flags = CameraSetting { value, is_auto }.native_flags();
    let hr = unsafe { set(interface.raw(), property_id, value, flags) };
    CameraWriteAck::from_hresult(hr).with_context(|| format!("{name}::Set({property_id})"))
}
impl DirectShowCameraTransport for CameraComSession {
    fn get_video_proc_amp(&mut self, id: u32) -> Result<CameraLongObservation> {
        get_property(&self.filter, &IID_IAM_VIDEO_PROC_AMP, id, "IAMVideoProcAmp")
    }
    fn set_video_proc_amp(&mut self, id: u32, value: i32, is_auto: bool) -> Result<CameraWriteAck> {
        set_property(
            &self.filter,
            &IID_IAM_VIDEO_PROC_AMP,
            id,
            value,
            is_auto,
            "IAMVideoProcAmp",
        )
    }
    fn get_camera_control(&mut self, id: u32) -> Result<CameraLongObservation> {
        get_property(
            &self.filter,
            &IID_IAM_CAMERA_CONTROL,
            id,
            "IAMCameraControl",
        )
    }
    fn set_camera_control(&mut self, id: u32, value: i32, is_auto: bool) -> Result<CameraWriteAck> {
        set_property(
            &self.filter,
            &IID_IAM_CAMERA_CONTROL,
            id,
            value,
            is_auto,
            "IAMCameraControl",
        )
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
