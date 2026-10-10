//! DLL-free current simple_service audio-list consumer. Windows Core Audio and
//! SetupAPI are isolated here; record schema/order live in razer-device.
use razer_device::simple_audio::SimpleAudioEndpoint;

pub fn enumerate() -> anyhow::Result<Vec<SimpleAudioEndpoint>> {
    #[cfg(windows)]
    return windows::enumerate();
    #[cfg(not(windows))]
    anyhow::bail!("simple_service audio endpoints require the unported Windows Core Audio adapter")
}

#[cfg(windows)]
mod windows {
    use super::SimpleAudioEndpoint;
    use crate::audio_util::windows::{Com, Property, TaskString};
    use anyhow::{Context as _, Result, ensure};
    use std::{
        ffi::c_void,
        mem::size_of,
        ptr::{null, null_mut},
    };
    use windows_sys::{
        Win32::{
            Devices::{
                DeviceAndDriverInstallation::*,
                Properties::{DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID},
            },
            Foundation::INVALID_HANDLE_VALUE,
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, StructuredStorage::PROPVARIANT,
            },
        },
        core::GUID,
    };

    const CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
    const INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
    const DATA_FLOW_INTERFACE: GUID = GUID::from_u128(0x1be09788_6894_4089_8586_9a2a6c265ac5);
    const AUDIO_CLASS: GUID = GUID::from_u128(0x4d36e96c_e325_11ce_bfc1_08002be10318);
    const ENDPOINT_VOLUME: GUID = GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a);

    #[repr(C)]
    struct Key {
        fmtid: GUID,
        pid: u32,
    }
    const NAME: Key = Key {
        fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };
    const CONTAINER: Key = Key {
        fmtid: GUID::from_u128(0x8c7ed206_3f8a_4827_b3ab_ae9e1faefc6c),
        pid: 2,
    };

    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }
    fn enumerator() -> Result<(Apartment, Com)> {
        let create = || {
            let mut output = null_mut();
            let hr = unsafe { CoCreateInstance(&CLASS, null_mut(), 1, &INTERFACE, &mut output) };
            (hr, output)
        };
        let mut apartment = Apartment(false);
        let (mut hr, mut pointer) = create();
        if hr as u32 == 0x800401f0 {
            // Native 3e864 retries once with COINIT_DISABLE_OLE1DDE=4.
            // Own/undo only our successful COM initialization.
            drop(Com::from_hresult(
                hr,
                pointer,
                "CoCreateInstance before apartment",
            ));
            let initialized = unsafe { CoInitializeEx(null(), 4) };
            ensure!(
                initialized >= 0,
                "CoInitializeEx failed: 0x{:08x}",
                initialized as u32
            );
            apartment.0 = true;
            (hr, pointer) = create();
        }
        Ok((
            apartment,
            Com::from_hresult(hr, pointer, "CoCreateInstance(MMDeviceEnumerator)")?,
        ))
    }

    fn property(device: &Com, key: &Key) -> Result<Property> {
        type Open = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
        type Get = unsafe extern "system" fn(*mut c_void, *const Key, *mut PROPVARIANT) -> i32;
        let call: Open = unsafe { device.method(4) };
        let mut pointer = null_mut();
        let hr = unsafe { call(device.raw(), 0, &mut pointer) };
        let store = Com::from_hresult(hr, pointer, "IMMDevice::OpenPropertyStore")?;
        let mut value = Property(PROPVARIANT::default());
        let call: Get = unsafe { store.method(5) };
        let hr = unsafe { call(store.raw(), key, &mut value.0) };
        ensure!(
            hr >= 0,
            "IPropertyStore::GetValue failed: 0x{:08x}",
            hr as u32
        );
        Ok(value)
    }
    fn name(device: &Com) -> Result<String> {
        let property = property(device, &NAME)?;
        let raw = unsafe { property.0.Anonymous.Anonymous };
        // Original returns success and the empty initialized name for any
        // non-VT_LPWSTR/null value, rather than borrowing another field.
        if raw.vt != 31 {
            return Ok(String::new());
        }
        let pointer = unsafe { raw.Anonymous.pwszVal };
        if pointer.is_null() {
            return Ok(String::new());
        }
        std::mem::ManuallyDrop::new(TaskString(pointer)).read()
    }
    fn guid_text(guid: &GUID, uppercase: bool) -> String {
        let text = format!(
            "{{{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
            guid.data1,
            guid.data2,
            guid.data3,
            guid.data4[0],
            guid.data4[1],
            guid.data4[2],
            guid.data4[3],
            guid.data4[4],
            guid.data4[5],
            guid.data4[6],
            guid.data4[7]
        );
        if uppercase {
            text.to_ascii_uppercase()
        } else {
            text
        }
    }
    struct Devices(HDEVINFO);
    impl Drop for Devices {
        fn drop(&mut self) {
            unsafe { SetupDiDestroyDeviceInfoList(self.0) };
        }
    }
    fn setup_container(id: &str) -> Option<String> {
        let set = unsafe { SetupDiGetClassDevsW(&AUDIO_CLASS, null(), null_mut(), 0x14) };
        if set == INVALID_HANDLE_VALUE as isize {
            return None;
        }
        let set = Devices(set);
        let mut info: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        info.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
        let mut index = 0;
        while unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } != 0 {
            index += 1;
            let mut instance = [0u16; 260];
            let mut required = 0;
            if unsafe {
                SetupDiGetDeviceInstanceIdW(set.0, &info, instance.as_mut_ptr(), 260, &mut required)
            } == 0
            {
                continue;
            }
            let end = instance.iter().position(|c| *c == 0)?;
            let text = String::from_utf16(&instance[..end]).ok()?;
            let Some(start) = text.find("SWD\\MMDEVAPI\\") else {
                continue;
            };
            // Native 1d2d94/1d2de8 lowercases the instance-ID suffix in
            // place before converting to UTF-8 and comparing endpoint ID.
            if text[start + 13..].to_ascii_lowercase() != id {
                continue;
            }
            let mut guid = GUID::default();
            let mut kind = 0;
            let ok = unsafe {
                SetupDiGetDevicePropertyW(
                    set.0,
                    &info,
                    &DEVPKEY_Device_ContainerId,
                    &mut kind,
                    std::ptr::addr_of_mut!(guid).cast(),
                    16,
                    null_mut(),
                    0,
                )
            };
            if ok != 0 && kind == DEVPROP_TYPE_GUID {
                return Some(guid_text(&guid, true));
            }
        }
        None
    }
    fn container(device: &Com, id: &str) -> String {
        if let Some(value) = setup_container(id) {
            return value;
        }
        // Current 3f236 fallback is Core Audio VT_CLSID, formatted as uppercase
        // UTF-16 by 75c10 and converted to UTF-8 by 86130/85890.
        let Ok(value) = property(device, &CONTAINER) else {
            return String::new();
        };
        let raw = unsafe { value.0.Anonymous.Anonymous };
        if raw.vt != 72 {
            return String::new();
        }
        let pointer = unsafe { raw.Anonymous.puuid };
        if pointer.is_null() {
            return String::new();
        }
        guid_text(unsafe { &*pointer }, true)
    }
    fn endpoint_type(device: &Com) -> &'static str {
        type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
        type Flow = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
        let call: Query = unsafe { device.method(0) };
        let mut pointer = null_mut();
        let hr = unsafe { call(device.raw(), &DATA_FLOW_INTERFACE, &mut pointer) };
        let Ok(endpoint) = Com::from_hresult(hr, pointer, "QueryInterface(IMMEndpoint)") else {
            return "all";
        };
        let call: Flow = unsafe { endpoint.method(3) };
        let mut flow = 2;
        if unsafe { call(endpoint.raw(), &mut flow) } < 0 {
            return "all";
        }
        match flow {
            0 => "speaker",
            1 => "microphone",
            _ => "all",
        }
    }
    fn volume_access(device: &Com) -> Result<Com> {
        type Activate = unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            u32,
            *const PROPVARIANT,
            *mut *mut c_void,
        ) -> i32;
        let call: Activate = unsafe { device.method(3) };
        let mut pointer = null_mut();
        let hr = unsafe { call(device.raw(), &ENDPOINT_VOLUME, 0x17, null(), &mut pointer) };
        Com::from_hresult(hr, pointer, "IMMDevice::Activate(IAudioEndpointVolume)")
    }
    pub(super) fn enumerate() -> Result<Vec<SimpleAudioEndpoint>> {
        let (_apartment, enumerator) = enumerator()?;
        type Enum = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
        type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
        let call: Enum = unsafe { enumerator.method(3) };
        let mut pointer = null_mut();
        let hr = unsafe { call(enumerator.raw(), 2, 1, &mut pointer) };
        let collection = Com::from_hresult(hr, pointer, "EnumAudioEndpoints(eAll, ACTIVE)")?;
        let call: Count = unsafe { collection.method(3) };
        let mut count = 0;
        let hr = unsafe { call(collection.raw(), &mut count) };
        ensure!(
            hr >= 0,
            "IMMDeviceCollection::GetCount failed: 0x{:08x}",
            hr as u32
        );
        ensure!(
            count <= 65536,
            "audio endpoint count exceeds replacement bound"
        );
        let call: Item = unsafe { collection.method(4) };
        let mut entries = Vec::new();
        for index in 0..count {
            let observed = (|| -> Result<SimpleAudioEndpoint> {
                let mut pointer = null_mut();
                let hr = unsafe { call(collection.raw(), index, &mut pointer) };
                let device = Com::from_hresult(hr, pointer, "IMMDeviceCollection::Item")?;
                let id = device.id_hresult()?;
                ensure!(!id.is_empty(), "Core Audio endpoint ID is empty");
                let name = name(&device)?;
                // Native AddAudioDevice also requires endpoint-volume
                // activation; a schema-only record must not bypass that gate.
                let _volume = volume_access(&device)?;
                Ok(SimpleAudioEndpoint {
                    container_id: container(&device, &id),
                    endpoint_type: endpoint_type(&device),
                    id,
                    name,
                })
            })()
            .with_context(|| format!("audio endpoint {index}"));
            // Original initial population stops and can return a partial list
            // on Item/name failure. This replacement reports the error; never
            // present an incomplete list as an observed complete inventory.
            entries.push(observed?);
        }
        razer_device::simple_audio::source_order(entries)
    }
}
