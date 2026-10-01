//! Read-only Windows HID interface discovery; no feature/output reports are sent.
use anyhow::Context as _;
use serde_json::{Value, json};
use std::{ffi::c_void, mem::size_of, ptr};
use windows_sys::{
    Win32::{
        Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError, HANDLE,
            INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
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

struct HidHandle(HANDLE);
impl Drop for HidHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct PreparsedData(PHIDP_PREPARSED_DATA);
impl Drop for PreparsedData {
    fn drop(&mut self) {
        unsafe {
            HidD_FreePreparsedData(self.0);
        }
    }
}

type ReadString = unsafe extern "system" fn(HANDLE, *mut c_void, u32) -> bool;

fn hid_string(handle: HANDLE, read: ReadString) -> std::io::Result<String> {
    let mut buffer = [0u16; 256];
    // SAFETY: writable UTF-16 buffer and its byte size; all readers are the
    // corresponding HidD_Get*String declarations from windows-sys 0.61.2.
    if unsafe {
        read(
            handle,
            buffer.as_mut_ptr().cast(),
            size_of_val(&buffer) as u32,
        )
    } {
        let length = buffer
            .iter()
            .position(|ch| *ch == 0)
            .unwrap_or(buffer.len());
        Ok(String::from_utf16_lossy(&buffer[..length]))
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub(super) fn enumerate() -> anyhow::Result<Value> {
    let mut guid = GUID::default();
    unsafe {
        HidD_GetHidGuid(&mut guid);
    }
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &guid,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if raw == -1 {
        return Err(std::io::Error::last_os_error()).context("无法枚举 HID 接口");
    }
    let set = DeviceSet(raw);
    let mut interfaces = Vec::new();
    let mut failures = Vec::new();
    let mut complete = false;
    for index in 0..u32::MAX {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, ptr::null(), &guid, index, &mut interface) }
            == 0
        {
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_ITEMS {
                complete = true;
                break;
            }
            failures.push(json!({
                "interface_index": index,
                "operation": "enumerate_interface",
                "error": std::io::Error::from_raw_os_error(error as i32).to_string(),
            }));
            break;
        }
        let mut required = 0;
        unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                ptr::null_mut(),
                0,
                &mut required,
                ptr::null_mut(),
            );
        }
        let error = unsafe { GetLastError() };
        if error != ERROR_INSUFFICIENT_BUFFER {
            failures.push(json!({
                "interface_index": index,
                "operation": "query_interface_path_size",
                "error": std::io::Error::from_raw_os_error(error as i32).to_string(),
            }));
            continue;
        }
        if required < size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32
            || required > 1024 * 1024
        {
            failures.push(json!({
                "interface_index": index,
                "operation": "query_interface_path_size",
                "error": "HID 接口路径长度异常",
            }));
            continue;
        }
        // u32 storage provides the required alignment; cbSize comes from the
        // platform-specific windows-sys layout (8 on x64, 6 on x86).
        let mut storage = vec![0u32; (required as usize).div_ceil(size_of::<u32>())];
        let detail = storage
            .as_mut_ptr()
            .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        unsafe {
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        }
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                detail,
                required,
                &mut required,
                ptr::null_mut(),
            )
        } == 0
        {
            failures.push(json!({
                "interface_index": index,
                "operation": "read_interface_path",
                "error": std::io::Error::last_os_error().to_string(),
            }));
            continue;
        }
        let path_pointer = unsafe { ptr::addr_of!((*detail).DevicePath).cast::<u16>() };
        let offset = path_pointer as usize - storage.as_ptr() as usize;
        if (required as usize) < offset + size_of::<u16>()
            || required as usize > storage.len() * size_of::<u32>()
        {
            failures.push(json!({
                "interface_index": index,
                "operation": "read_interface_path",
                "error": "HID 接口路径返回长度异常",
            }));
            continue;
        }
        let path_slice =
            unsafe { std::slice::from_raw_parts(path_pointer, (required as usize - offset) / 2) };
        let Some(length) = path_slice.iter().position(|ch| *ch == 0) else {
            failures.push(json!({
                "interface_index": index,
                "operation": "read_interface_path",
                "error": "HID 接口路径缺少终止符",
            }));
            continue;
        };
        let path = String::from_utf16_lossy(&path_slice[..length]);
        // Access=0 allows metadata queries without claiming read/write access.
        let raw = unsafe {
            CreateFileW(
                path_pointer,
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                0,
                ptr::null_mut(),
            )
        };
        if raw == INVALID_HANDLE_VALUE {
            failures.push(json!({
                "path": path,
                "operation": "open_interface",
                "error": std::io::Error::last_os_error().to_string(),
            }));
            continue;
        }
        let handle = HidHandle(raw);
        let mut attributes = HIDD_ATTRIBUTES {
            Size: size_of::<HIDD_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        if !unsafe { HidD_GetAttributes(handle.0, &mut attributes) } {
            failures.push(json!({
                "path": path,
                "operation": "read_attributes",
                "error": std::io::Error::last_os_error().to_string(),
            }));
            continue;
        }
        // The source mapping engine accepts the Razer and acquired-device VIDs.
        if !matches!(attributes.VendorID, 0x1532 | 0x068e) {
            continue;
        }
        let mut caps = None;
        let mut preparsed = 0;
        if unsafe { HidD_GetPreparsedData(handle.0, &mut preparsed) } {
            let preparsed = PreparsedData(preparsed);
            let mut value = HIDP_CAPS::default();
            let status = unsafe { HidP_GetCaps(preparsed.0, &mut value) };
            if status == HIDP_STATUS_SUCCESS {
                caps = Some(value);
            } else {
                failures.push(json!({
                    "path": path,
                    "operation": "read_capabilities",
                    "error": format!("HidP_GetCaps 返回 NTSTATUS 0x{:08X}", status as u32),
                }));
            }
        } else {
            failures.push(json!({
                "path": path,
                "operation": "read_preparsed_data",
                "error": std::io::Error::last_os_error().to_string(),
            }));
        }
        let mut read_string = |operation: &str, read: ReadString| match hid_string(handle.0, read) {
            Ok(value) => Some(value),
            Err(error) => {
                failures.push(
                    json!({"path": path, "operation": operation, "error": error.to_string()}),
                );
                None
            }
        };
        let manufacturer = read_string("read_manufacturer", HidD_GetManufacturerString);
        let product = read_string("read_product", HidD_GetProductString);
        let serial_number = read_string("read_serial_number", HidD_GetSerialNumberString);
        interfaces.push(json!({
            "path": path,
            "vendor_id": attributes.VendorID,
            "product_id": attributes.ProductID,
            "version_number": attributes.VersionNumber,
            "manufacturer": manufacturer,
            "product": product,
            "serial_number": serial_number,
            "usage_page": caps.as_ref().map(|caps| caps.UsagePage),
            "usage": caps.as_ref().map(|caps| caps.Usage),
            "input_report_bytes": caps.as_ref().map(|caps| caps.InputReportByteLength),
            "output_report_bytes": caps.as_ref().map(|caps| caps.OutputReportByteLength),
            "feature_report_bytes": caps.as_ref().map(|caps| caps.FeatureReportByteLength),
        }));
    }
    // Multiple collections can belong to one physical device. No fabricated
    // profile/container identity or driver connection status is attached here.
    Ok(json!({"interfaces": interfaces, "failures": failures, "complete": complete}))
}
