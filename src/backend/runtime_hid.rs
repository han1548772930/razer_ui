//! Read-only Windows HID interface discovery; no feature/output reports are sent.
use anyhow::Context as _;
use serde_json::{Value, json};
use std::{ffi::c_void, mem::size_of, ptr};
use windows_sys::{
    Win32::{
        Devices::{
            DeviceAndDriverInstallation::*,
            HumanInterfaceDevice::*,
            Properties::{DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID},
        },
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError, HANDLE,
            INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
    },
    core::GUID,
};

pub(super) struct DeviceSet(pub(super) HDEVINFO);
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

pub(super) fn device_container_id(
    set: HDEVINFO,
    device: &SP_DEVINFO_DATA,
) -> anyhow::Result<String> {
    let mut value = GUID::default();
    let mut property_type = 0;
    let mut required = 0;
    if unsafe {
        SetupDiGetDevicePropertyW(
            set,
            device,
            &DEVPKEY_Device_ContainerId,
            &mut property_type,
            ptr::addr_of_mut!(value).cast(),
            size_of::<GUID>() as u32,
            &mut required,
            0,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("读取设备 ContainerId 失败");
    }
    anyhow::ensure!(
        property_type == DEVPROP_TYPE_GUID && required as usize == size_of::<GUID>(),
        "设备 ContainerId 属性类型或长度错误"
    );
    anyhow::ensure!(
        value.data1 != 0 || value.data2 != 0 || value.data3 != 0 || value.data4 != [0; 8],
        "设备 ContainerId 为空"
    );
    Ok(format!(
        "{{{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}}}",
        value.data1,
        value.data2,
        value.data3,
        value.data4[0],
        value.data4[1],
        value.data4[2],
        value.data4[3],
        value.data4[4],
        value.data4[5],
        value.data4[6],
        value.data4[7]
    ))
}

pub(super) fn device_instance_id(
    set: HDEVINFO,
    device: &SP_DEVINFO_DATA,
) -> anyhow::Result<String> {
    let mut buffer = [0u16; 512];
    let mut required = 0;
    if unsafe {
        SetupDiGetDeviceInstanceIdW(
            set,
            device,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            &mut required,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("读取设备实例 ID 失败");
    }
    anyhow::ensure!(
        required > 1 && required as usize <= buffer.len(),
        "设备实例 ID 长度错误"
    );
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .ok_or_else(|| anyhow::anyhow!("设备实例 ID 缺少终止符"))?;
    Ok(String::from_utf16(&buffer[..end])?)
}

/// Only an actual MI_XX component supplies the USB interface number. The
/// source middleware also tries native interface -1; unknown metadata here
/// deliberately remains unknown instead of being guessed as interface zero.
fn interface_number(instance_id: &str) -> Option<u8> {
    let upper = instance_id.to_ascii_uppercase();
    let mut values = upper.split(['\\', '&', '#']).filter_map(|part| {
        let hex = part.strip_prefix("MI_")?;
        (hex.len() == 2)
            .then(|| u8::from_str_radix(hex, 16).ok())
            .flatten()
    });
    let first = values.next()?;
    values.all(|next| next == first).then_some(first)
}

pub(super) fn enumerate() -> anyhow::Result<Value> {
    enumerate_with_scope(None, true)
}

pub(super) fn enumerate_metadata() -> anyhow::Result<Value> {
    enumerate_with_scope(None, false)
}

pub(super) fn enumerate_for_product(vendor_id: u16, product_id: u16) -> anyhow::Result<Value> {
    enumerate_with_scope(Some((vendor_id, product_id)), false)
}

fn enumerate_with_scope(
    scope: Option<(u16, u16)>,
    read_descriptors: bool,
) -> anyhow::Result<Value> {
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
        let mut device_info = SP_DEVINFO_DATA {
            cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                detail,
                required,
                &mut required,
                &mut device_info,
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
        if scope.is_some_and(|identity| identity != (attributes.VendorID, attributes.ProductID)) {
            continue;
        }
        let (container_id, container_error) = match device_container_id(set.0, &device_info) {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let (instance_id, instance_error) = match device_instance_id(set.0, &device_info) {
            Ok(value) => (Some(value), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let claim_interface = instance_id.as_deref().and_then(interface_number);
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
        // Revalidating a receiver before/after a query needs identity and caps,
        // not unrelated USB string transfers. The ordinary inventory still
        // exposes those optional descriptors and their individual failures.
        let (manufacturer, product, serial_number) = if !read_descriptors {
            (None, None, None)
        } else {
            (
                read_string("read_manufacturer", HidD_GetManufacturerString),
                read_string("read_product", HidD_GetProductString),
                read_string("read_serial_number", HidD_GetSerialNumberString),
            )
        };
        interfaces.push(json!({
            "path": path,
            "vendor_id": attributes.VendorID,
            "product_id": attributes.ProductID,
            "version_number": attributes.VersionNumber,
            "manufacturer": manufacturer,
            "product": product,
            "serial_number": serial_number,
            "device_container_id": container_id,
            "container_id_error": container_error,
            "device_instance_id": instance_id,
            "device_instance_id_error": instance_error,
            "claim_interface": claim_interface,
            "claim_interface_error": if claim_interface.is_some() { None } else { Some("设备实例 ID 未提供唯一的 USB MI_XX 接口号") },
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
