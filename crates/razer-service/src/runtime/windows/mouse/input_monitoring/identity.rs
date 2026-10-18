//! CC9E0 identifies the RawInput path through SetupAPI and HID attributes.
//! VID/PID parsing is permitted only when opening that HID path fails.
use super::super::hid;
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
};
use windows_sys::Win32::{
    Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
    Foundation::{ERROR_INSUFFICIENT_BUFFER, GetLastError, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Identity {
    pub container_id: String,
    pub vendor_id: u32,
    pub product_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usb_instance_id: Option<String>,
}
fn path_ids(path: &str) -> Result<(u32, u32)> {
    let path = path.to_ascii_lowercase();
    let id = |prefix: &str| -> Result<u32> {
        let start = path
            .find(prefix)
            .context("RawInput HID path lacks product identity")?
            + prefix.len();
        let digits = path
            .get(start..start + 4)
            .context("RawInput HID identity is truncated")?;
        ensure!(
            digits.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "RawInput HID identity is not hexadecimal"
        );
        let value = u32::from_str_radix(digits, 16)?;
        ensure!(value != 0, "RawInput HID identity is zero");
        Ok(value)
    };
    Ok((id("vid_")?, id("pid_")?))
}

pub(super) fn observe(path: &str) -> Result<Identity> {
    ensure!(
        !path.is_empty() && !path.contains('\0'),
        "Invalid RawInput HID path"
    );
    let path_wide = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut guid = windows_sys::core::GUID::default();
    // SAFETY: buffers and structures are owned by this call; the device set
    // remains alive until all dependent identity queries complete.
    unsafe { HidD_GetHidGuid(&mut guid) };
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &guid,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    ensure!(
        raw != -1,
        "RawInput SetupAPI discovery failed: {}",
        std::io::Error::last_os_error()
    );
    let set = hid::DeviceSet(raw);
    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    ensure!(
        unsafe { SetupDiOpenDeviceInterfaceW(set.0, path_wide.as_ptr(), 0, &mut interface) } != 0,
        "RawInput interface is absent: {}",
        std::io::Error::last_os_error()
    );
    let mut required = 0;
    let queried = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set.0,
            &interface,
            ptr::null_mut(),
            0,
            &mut required,
            ptr::null_mut(),
        )
    };
    ensure!(
        queried == 0 && unsafe { GetLastError() } == ERROR_INSUFFICIENT_BUFFER,
        "RawInput interface size query failed"
    );
    ensure!(
        required as usize >= size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>()
            && required <= 1024 * 1024,
        "Invalid RawInput interface size"
    );
    let mut storage = vec![0u32; (required as usize).div_ceil(size_of::<u32>())];
    let detail = storage
        .as_mut_ptr()
        .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    unsafe { (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 };
    let mut device = SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    };
    ensure!(
        unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                detail,
                required,
                ptr::null_mut(),
                &mut device,
            )
        } != 0,
        "RawInput interface identity read failed: {}",
        std::io::Error::last_os_error()
    );
    let container_id = hid::device_container_id(set.0, &device)?;
    let raw = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    let (vendor_id, product_id) = if raw == INVALID_HANDLE_VALUE {
        path_ids(path)?
    } else {
        // SAFETY: CreateFileW returned a new valid handle, transferred once.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        let mut attrs = HIDD_ATTRIBUTES {
            Size: size_of::<HIDD_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        ensure!(
            unsafe { HidD_GetAttributes(handle.as_raw_handle(), &mut attrs) },
            "RawInput attributes failed: {}",
            std::io::Error::last_os_error()
        );
        ensure!(
            attrs.VendorID != 0 && attrs.ProductID != 0,
            "RawInput attributes contain zero identity"
        );
        (u32::from(attrs.VendorID), u32::from(attrs.ProductID))
    };
    Ok(Identity {
        container_id,
        vendor_id,
        product_id,
        usb_instance_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_path_ids_include_reference_mice_and_reject_incomplete_values() {
        assert_eq!(
            path_ids(r"\\?\HID#VID_046D&PID_C077#collection").unwrap(),
            (0x046d, 0xc077)
        );
        for path in [
            "hid#vid_0000&pid_0046",
            "hid#vid_1532&pid_",
            "hid#vid_1532&pid_x046",
            "hid#vid_1532",
        ] {
            assert!(path_ids(path).is_err());
        }
    }
}
