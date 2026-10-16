//! Bind a complete camera interface link to its current OS ContainerId.
//! Additional Rust identity guard; no device reports, DLLs or target execution.
use anyhow::{Context as _, Result, ensure};
use std::{
    mem::{size_of, size_of_val},
    ptr,
};
use windows_sys::{
    Win32::{
        Devices::{
            DeviceAndDriverInstallation::*,
            Properties::{DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID},
        },
        Foundation::{ERROR_INSUFFICIENT_BUFFER, GetLastError, INVALID_HANDLE_VALUE},
    },
    core::GUID,
};
struct DeviceSet(HDEVINFO);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
pub(super) fn container_id(path: &str) -> Result<String> {
    ensure!(
        !path.contains('\0') && path.len() <= 32768,
        "invalid camera interface path"
    );
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let set = unsafe { SetupDiCreateDeviceInfoList(ptr::null(), ptr::null_mut()) };
    ensure!(
        set != INVALID_HANDLE_VALUE as isize,
        "camera SetupDiCreateDeviceInfoList failed"
    );
    let set = DeviceSet(set);
    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    if unsafe { SetupDiOpenDeviceInterfaceW(set.0, wide.as_ptr(), 0, &mut interface) } == 0 {
        return Err(std::io::Error::last_os_error())
            .context("camera interface is not currently present");
    }
    ensure!(
        interface.Flags & SPINT_ACTIVE != 0,
        "camera interface is inactive"
    );
    let mut needed = 0;
    let success = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set.0,
            &interface,
            ptr::null_mut(),
            0,
            &mut needed,
            ptr::null_mut(),
        )
    };
    ensure!(
        success == 0 && unsafe { GetLastError() } == ERROR_INSUFFICIENT_BUFFER,
        "camera interface detail sizing failed"
    );
    ensure!(
        (size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32..=65536).contains(&needed),
        "invalid camera interface detail size"
    );
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    let detail = buffer
        .as_mut_ptr()
        .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    unsafe {
        (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    }
    let mut device = SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    };
    if unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set.0,
            &interface,
            detail,
            needed,
            &mut needed,
            &mut device,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("camera interface detail read failed");
    }
    let mut value = GUID::default();
    let mut kind = 0;
    let mut bytes = 0;
    if unsafe {
        SetupDiGetDevicePropertyW(
            set.0,
            &device,
            &DEVPKEY_Device_ContainerId,
            &mut kind,
            ptr::addr_of_mut!(value).cast(),
            size_of_val(&value) as u32,
            &mut bytes,
            0,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("camera ContainerId read failed");
    }
    ensure!(
        kind == DEVPROP_TYPE_GUID && bytes == size_of::<GUID>() as u32,
        "invalid camera ContainerId property"
    );
    let d = value.data4;
    let id = format!(
        "{{{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
        value.data1, value.data2, value.data3, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
    );
    ensure!(
        id != "{00000000-0000-0000-0000-000000000000}",
        "camera ContainerId is nil"
    );
    Ok(id)
}
