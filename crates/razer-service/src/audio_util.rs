//! Direct OS replacement for current RzAudioUtil AudioEnumerator read chain.
//! No vendor DLL is loaded. Current source/binary receipts are kept in
//! docs/re/audio-util-enumerator-current-evidence.json.
use razer_device::audio_util::{AudioEnumeration, AudioFlow};

pub fn enumerate(flow: AudioFlow) -> anyhow::Result<AudioEnumeration> {
    #[cfg(windows)]
    return windows::enumerate(flow);
    #[cfg(not(windows))]
    {
        let _ = flow;
        anyhow::bail!("RzAudioUtil Windows AudioEnumerator is unavailable on this platform")
    }
}

#[cfg(windows)]
pub(crate) mod windows {
    use anyhow::{Context, Result, ensure};
    use razer_device::audio_util::{
        AudioEndpoint, AudioEnumeration, AudioFlow, accepts_property_result,
    };
    use std::{
        ffi::c_void,
        marker::PhantomData,
        ptr::{NonNull, null, null_mut},
        rc::Rc,
    };
    use windows_sys::{
        Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            StructuredStorage::{PROPVARIANT, PropVariantClear},
        },
        core::GUID,
    };

    const ENUMERATOR_CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
    const ENUMERATOR_INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
    const DEVICE_PROPERTY_SET: GUID = GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0);

    #[repr(C)]
    struct PropertyKey {
        fmtid: GUID,
        pid: u32,
    }

    struct Apartment;
    impl Apartment {
        fn open() -> Result<Self> {
            // Replacement scopes COM ownership to this worker call. Native
            // enumeration assumes its executor already has an apartment.
            let hr = unsafe { CoInitializeEx(null(), 0) };
            ensure!(hr >= 0, "CoInitializeEx failed: 0x{:08x}", hr as u32);
            Ok(Self)
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    pub(crate) struct Com {
        pointer: NonNull<c_void>,
        // Interfaces cannot cross threads or outlive their apartment.
        thread: PhantomData<Rc<()>>,
    }
    impl Com {
        fn from_result(hr: i32, pointer: *mut c_void, operation: &str) -> Result<Self> {
            let result = Self::from_hresult(hr, pointer, operation);
            ensure!(hr == 0, "{operation} failed: 0x{:08x}", hr as u32);
            result
        }
        pub(crate) fn from_hresult(hr: i32, pointer: *mut c_void, operation: &str) -> Result<Self> {
            // Release even a non-null failure output; never retain raw outputs.
            let result = NonNull::new(pointer).map(|pointer| Self {
                pointer,
                thread: PhantomData,
            });
            ensure!(hr >= 0, "{operation} failed: 0x{:08x}", hr as u32);
            result.with_context(|| format!("{operation} returned a null COM interface"))
        }
        pub(crate) fn raw(&self) -> *mut c_void {
            self.pointer.as_ptr()
        }
        pub(crate) unsafe fn method<T: Copy>(&self, slot: usize) -> T {
            // All requested slots and signatures are fixed to IMMDevice*,
            // IMMDeviceCollection or IPropertyStore, observed below in PE.
            let table = unsafe { *(self.raw() as *const *const *const c_void) };
            let address = unsafe { *table.add(slot) };
            unsafe { std::mem::transmute_copy(&address) }
        }
        pub(crate) fn id(&self) -> Result<String> {
            self.read_id(false)
        }
        pub(crate) fn id_hresult(&self) -> Result<String> {
            self.read_id(true)
        }
        fn read_id(&self, accept_nonnegative: bool) -> Result<String> {
            type GetId = unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> i32;
            let call: GetId = unsafe { self.method(5) };
            let mut pointer = null_mut();
            let hr = unsafe { call(self.raw(), &mut pointer) };
            let text = TaskString(pointer);
            ensure!(
                (accept_nonnegative && hr >= 0) || hr == 0,
                "IMMDevice::GetId failed: 0x{:08x}",
                hr as u32
            );
            text.read()
        }
    }
    impl Drop for Com {
        fn drop(&mut self) {
            type Release = unsafe extern "system" fn(*mut c_void) -> u32;
            let call: Release = unsafe { self.method(2) };
            unsafe { call(self.raw()) };
        }
    }

    pub(crate) struct TaskString(pub(crate) *mut u16);
    impl TaskString {
        pub(crate) fn read(&self) -> Result<String> {
            ensure!(!self.0.is_null(), "COM returned a null UTF-16 string");
            // OS owns the allocated string; scan until the contractual zero.
            // A bound avoids unbounded scanning after a broken COM response.
            let mut length = 0;
            while length < 1_048_576 && unsafe { *self.0.add(length) } != 0 {
                length += 1;
            }
            ensure!(length < 1_048_576, "COM UTF-16 string exceeds limit");
            String::from_utf16(unsafe { std::slice::from_raw_parts(self.0, length) })
                .context("invalid COM UTF-16 string")
        }
    }
    impl Drop for TaskString {
        fn drop(&mut self) {
            unsafe { CoTaskMemFree(self.0.cast()) };
        }
    }

    pub(crate) struct Property(pub(crate) PROPVARIANT);
    impl Drop for Property {
        fn drop(&mut self) {
            unsafe { PropVariantClear(&mut self.0) };
        }
    }
    fn property(device: &Com, pid: u32) -> Result<String> {
        type OpenStore = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
        type GetValue =
            unsafe extern "system" fn(*mut c_void, *const PropertyKey, *mut PROPVARIANT) -> i32;
        let open: OpenStore = unsafe { device.method(4) };
        let mut pointer = null_mut();
        let hr = unsafe { open(device.raw(), 0, &mut pointer) };
        let store = Com::from_result(hr, pointer, "IMMDevice::OpenPropertyStore(STGM_READ)")?;
        let key = PropertyKey {
            fmtid: DEVICE_PROPERTY_SET,
            pid,
        };
        let mut value = Property(PROPVARIANT::default());
        let get: GetValue = unsafe { store.method(5) };
        let hr = unsafe { get(store.raw(), &key, &mut value.0) };
        ensure!(
            accepts_property_result(hr),
            "IPropertyStore::GetValue({pid}) failed: 0x{:08x}",
            hr as u32
        );
        let raw = unsafe { value.0.Anonymous.Anonymous };
        ensure!(raw.vt == 31, "endpoint property {pid} is not VT_LPWSTR");
        // Variant owns this pointer; borrow it, then clear exactly once.
        let pointer = unsafe { raw.Anonymous.pwszVal };
        let borrowed = std::mem::ManuallyDrop::new(TaskString(pointer));
        borrowed.read()
    }

    fn default_id(
        enumerator: &Com,
        flow: AudioFlow,
        role: i32,
        diagnostics: &mut Vec<String>,
    ) -> Result<Option<String>> {
        type GetDefault = unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> i32;
        let call: GetDefault = unsafe { enumerator.method(4) };
        let mut pointer = null_mut();
        let hr = unsafe { call(enumerator.raw(), flow.native_value(), role, &mut pointer) };
        let device = Com::from_result(hr, pointer, "GetDefaultAudioEndpoint");
        if hr != 0 {
            diagnostics.push(format!(
                "default role {role} unavailable: 0x{:08x}",
                hr as u32
            ));
            return Ok(None);
        }
        Ok(Some(device?.id()?))
    }

    pub(super) fn enumerate(flow: AudioFlow) -> Result<AudioEnumeration> {
        let _apartment = Apartment::open()?;
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
        let enumerator = Com::from_result(hr, pointer, "CoCreateInstance(MMDeviceEnumerator)")?;
        type Enum = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
        let call: Enum = unsafe { enumerator.method(3) };
        let mut pointer = null_mut();
        // 0x44EC7 / 0x44F29 select DEVICE_STATE_ACTIVE only.
        let hr = unsafe { call(enumerator.raw(), flow.native_value(), 1, &mut pointer) };
        let collection = Com::from_result(hr, pointer, "EnumAudioEndpoints(DEVICE_STATE_ACTIVE)")?;
        let mut diagnostics = Vec::new();
        let console_id = default_id(&enumerator, flow, 0, &mut diagnostics)?;
        let communications_id = default_id(&enumerator, flow, 2, &mut diagnostics)?;
        type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let count: Count = unsafe { collection.method(3) };
        let mut total = 0;
        let hr = unsafe { count(collection.raw(), &mut total) };
        ensure!(
            hr == 0,
            "IMMDeviceCollection::GetCount failed: 0x{:08x}",
            hr as u32
        );
        let mut endpoints = Vec::new();
        type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
        let item: Item = unsafe { collection.method(4) };
        for index in 0..total {
            let observed = (|| -> Result<AudioEndpoint> {
                let mut pointer = null_mut();
                let hr = unsafe { item(collection.raw(), index, &mut pointer) };
                let device = Com::from_result(hr, pointer, "IMMDeviceCollection::Item")?;
                let id = device.id()?;
                // Both GetValue calls are required before native record append.
                let friendly_name = property(&device, 14)?;
                let description = property(&device, 2)?;
                Ok(AudioEndpoint::observed(
                    id,
                    friendly_name,
                    description,
                    console_id.as_deref(),
                    communications_id.as_deref(),
                ))
            })();
            match observed {
                Ok(endpoint) => endpoints.push(endpoint),
                Err(error) => diagnostics.push(format!("endpoint {index} skipped: {error:#}")),
            }
        }
        Ok(AudioEnumeration {
            flow,
            endpoints,
            diagnostics,
        })
    }
}
