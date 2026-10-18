use super::{
    registry::{self, Key},
    semantics,
};
use anyhow::ensure;
use std::{
    collections::BTreeMap,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Devices::{
            DeviceAndDriverInstallation::*,
            Display::{GUID_DEVINTERFACE_DISPLAY_ADAPTER, GUID_DEVINTERFACE_MONITOR},
            Properties::*,
        },
        Foundation::{
            DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError,
            INVALID_HANDLE_VALUE,
        },
        System::Registry::{HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY, REG_SZ},
    },
    core::GUID,
};

struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}
impl Devices {
    fn open(class: Option<&GUID>, flags: u32) -> anyhow::Result<Self> {
        let handle = unsafe {
            SetupDiGetClassDevsW(class.map_or(null(), |guid| guid), null(), null_mut(), flags)
        };
        ensure!(
            handle != INVALID_HANDLE_VALUE as isize,
            "Enumerate report devices: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self(handle))
    }
    fn interfaces(
        &self,
        class: &GUID,
        mut visit: impl FnMut(&SP_DEVINFO_DATA) -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        for index in 0.. {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..unsafe { std::mem::zeroed() }
            };
            if unsafe { SetupDiEnumDeviceInterfaces(self.0, null(), class, index, &mut interface) }
                == 0
            {
                let code = unsafe { GetLastError() };
                if code == ERROR_NO_MORE_ITEMS {
                    break;
                }
                registry::status(code, "Enumerate report interface")?;
            }
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..unsafe { std::mem::zeroed() }
            };
            let result = unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    self.0,
                    &interface,
                    null_mut(),
                    0,
                    null_mut(),
                    &mut info,
                )
            };
            if result == 0 {
                let code = unsafe { GetLastError() };
                if code != ERROR_INSUFFICIENT_BUFFER {
                    registry::status(code, "Read report interface device")?;
                }
            }
            if !visit(&info)? {
                break;
            }
        }
        Ok(())
    }
    fn property(&self, device: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> anyhow::Result<String> {
        let mut data = [0u16; 2048];
        let mut kind = 0;
        let mut length = 0;
        ensure!(
            unsafe {
                SetupDiGetDevicePropertyW(
                    self.0,
                    device,
                    key,
                    &mut kind,
                    data.as_mut_ptr().cast(),
                    size_of_val(&data) as u32,
                    &mut length,
                    0,
                )
            } != 0,
            "Read report device property: {}",
            std::io::Error::last_os_error()
        );
        ensure!(
            matches!(kind, DEVPROP_TYPE_STRING | DEVPROP_TYPE_STRING_LIST),
            "Report property is not a string"
        );
        ensure!(
            length as usize <= size_of_val(&data) && length % 2 == 0,
            "Report property length is invalid"
        );
        registry::text(&data[..length as usize / 2])
    }
    fn registry_property(&self, device: &SP_DEVINFO_DATA, property: u32) -> anyhow::Result<String> {
        let mut data = [0u16; 4096];
        let mut kind = 0;
        let mut length = 0;
        ensure!(
            unsafe {
                SetupDiGetDeviceRegistryPropertyW(
                    self.0,
                    device,
                    property,
                    &mut kind,
                    data.as_mut_ptr().cast(),
                    size_of_val(&data) as u32,
                    &mut length,
                )
            } != 0,
            "Read report device registry property: {}",
            std::io::Error::last_os_error()
        );
        ensure!(kind == REG_SZ, "Report device property is not a string");
        ensure!(
            length as usize <= size_of_val(&data) && length % 2 == 0,
            "Report device property length is invalid"
        );
        registry::text(&data[..length as usize / 2])
    }
}

/// The current CameraFW source enumerates KSCATEGORY_CAPTURE. These GUID
/// bytes are constructed directly in all four captured native functions.
const CAPTURE_INTERFACE: GUID = GUID::from_u128(0x65e8773d_8f56_11d0_a3b9_00a0c9223196);
pub(super) fn camera(diagnostics: &mut Vec<String>) -> anyhow::Result<Option<String>> {
    let devices = Devices::open(
        None,
        DIGCF_PRESENT | DIGCF_ALLCLASSES | DIGCF_DEVICEINTERFACE,
    )?;
    let mut revision = None;
    let result = devices.interfaces(&CAPTURE_INTERFACE, |device| {
        let id = devices.property(device, &DEVPKEY_Device_HardwareIds)?;
        if let Some(value) = semantics::camera_revision(&id)? {
            revision = Some(value);
        }
        Ok(true)
    });
    if let Err(error) = result {
        diagnostics.push(format!("Camera interfaces: {error:#}"));
    }
    Ok(revision)
}
pub(super) fn vbios(diagnostics: &mut Vec<String>) -> anyhow::Result<Option<String>> {
    let devices = Devices::open(
        None,
        DIGCF_PRESENT | DIGCF_ALLCLASSES | DIGCF_DEVICEINTERFACE,
    )?;
    let mut driver = None;
    let result = devices.interfaces(&GUID_DEVINTERFACE_DISPLAY_ADAPTER, |device| {
        if devices
            .property(device, &DEVPKEY_Device_Manufacturer)?
            .contains("NVIDIA")
        {
            driver = Some(devices.property(device, &DEVPKEY_Device_Driver)?);
        }
        Ok(true)
    });
    if let Err(error) = result {
        diagnostics.push(format!("VBIOS interfaces: {error:#}"));
    }
    let Some(driver) = driver else {
        return Ok(None);
    };
    let value = registry::software(
        HKEY_LOCAL_MACHINE,
        &format!("{}\\{driver}", registry::DRIVER_CLASS),
        "HardwareInformation.BiosString",
    )?;
    Ok(value.and_then(|value| value.split_once("Version").map(|(_, tail)| tail.to_owned())))
}

pub(super) fn edid(device_id: &str) -> anyhow::Result<Option<(Option<String>, String)>> {
    let devices = Devices::open(
        None,
        DIGCF_PRESENT | DIGCF_ALLCLASSES | DIGCF_DEVICEINTERFACE,
    )?;
    let mut result = None;
    devices.interfaces(&GUID_DEVINTERFACE_MONITOR, |device| {
        let id = format!(
            "{}\\{}",
            devices.property(device, &DEVPKEY_Device_HardwareIds)?,
            devices.property(device, &DEVPKEY_Device_Driver)?
        );
        if id != device_id {
            return Ok(true);
        }
        let handle = unsafe {
            SetupDiOpenDevRegKey(devices.0, device, DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ)
        };
        ensure!(
            !handle.is_null() && handle != INVALID_HANDLE_VALUE,
            "Open monitor EDID registry: {}",
            std::io::Error::last_os_error()
        );
        let key = Key(handle);
        if let Some((kind, bytes)) = key.bytes("EDID")? {
            ensure!(
                kind == REG_BINARY,
                "Monitor EDID registry value is not binary"
            );
            result = Some(semantics::monitor_edid(&bytes)?);
        }
        Ok(false)
    })?;
    Ok(result)
}

struct DriverList<'a> {
    devices: &'a Devices,
    device: &'a SP_DEVINFO_DATA,
}
impl Drop for DriverList<'_> {
    fn drop(&mut self) {
        unsafe { SetupDiDestroyDriverInfoList(self.devices.0, self.device, SPDIT_COMPATDRIVER) };
    }
}

pub(super) fn drivers(diagnostics: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    for index in 0.. {
        let mut class = GUID::from_u128(0);
        let code = unsafe { CM_Enumerate_Classes(index, &mut class, 0) };
        if code == CR_NO_SUCH_VALUE {
            break;
        }
        if code != CR_SUCCESS {
            diagnostics.push(format!("Enumerate driver classes: CONFIGRET {code}"));
            break;
        }
        let devices = match Devices::open(Some(&class), DIGCF_PRESENT) {
            Ok(devices) => devices,
            Err(error) => {
                diagnostics.push(format!("Driver class: {error:#}"));
                break;
            }
        };
        for device_index in 0.. {
            let mut device = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..unsafe { std::mem::zeroed() }
            };
            if unsafe { SetupDiEnumDeviceInfo(devices.0, device_index, &mut device) } == 0 {
                let code = unsafe { GetLastError() };
                if code != ERROR_NO_MORE_ITEMS {
                    diagnostics.push(format!("Enumerate driver device: Win32 {code}"));
                }
                break;
            }
            let mut id_length = 0;
            let code = unsafe { CM_Get_Device_ID_Size(&mut id_length, device.DevInst, 0) };
            if code != CR_SUCCESS {
                diagnostics.push(format!("Read driver device ID size: CONFIGRET {code}"));
                break;
            }
            let description = match devices.registry_property(&device, SPDRP_DEVICEDESC) {
                Ok(description) => description,
                Err(error) => {
                    diagnostics.push(format!("Driver description: {error:#}"));
                    break;
                }
            };
            // FriendlyName is optional in the original; DeviceDesc is another
            // actual property, not a cached/default substitute for a read.
            let name = match devices.registry_property(&device, SPDRP_FRIENDLYNAME) {
                Ok(name) if !name.is_empty() => name,
                Ok(_) => description,
                Err(error) => {
                    diagnostics.push(format!("Driver friendly name: {error:#}"));
                    description
                }
            };
            match compatible_driver_version(&devices, &mut device, &name, diagnostics) {
                Ok(Some(version)) if semantics::driver_vendor(&name) => {
                    versions
                        .entry(name)
                        .or_insert_with(|| semantics::driver_version(version));
                }
                Ok(Some(_)) => {}
                Ok(None) => {
                    diagnostics.push(format!(
                        "Driver {name}: compatible driver list has no entries"
                    ));
                }
                Err(error) => {
                    diagnostics.push(format!("Driver {name}: {error:#}"));
                }
            }
        }
    }
    versions
}

fn compatible_driver_version(
    devices: &Devices,
    device: &mut SP_DEVINFO_DATA,
    name: &str,
    diagnostics: &mut Vec<String>,
) -> anyhow::Result<Option<u64>> {
    ensure!(
        unsafe { SetupDiBuildDriverInfoList(devices.0, device, SPDIT_COMPATDRIVER) } != 0,
        "Build report compatible driver list: {}",
        std::io::Error::last_os_error()
    );
    let _list = DriverList { devices, device };
    let mut version = None;
    for driver_index in 0.. {
        let mut info = SP_DRVINFO_DATA_V2_W {
            cbSize: size_of::<SP_DRVINFO_DATA_V2_W>() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        if unsafe {
            SetupDiEnumDriverInfoW(
                devices.0,
                device,
                SPDIT_COMPATDRIVER,
                driver_index,
                &mut info,
            )
        } == 0
        {
            let code = unsafe { GetLastError() };
            if code == ERROR_NO_MORE_ITEMS {
                break;
            }
            diagnostics.push(format!(
                "Driver {name}: enumerate compatible drivers: Win32 {code}"
            ));
            break;
        }
        version = Some(info.DriverVersion);
    }
    Ok(version)
}
