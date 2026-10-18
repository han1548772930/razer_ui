//! Read-only Windows HID interface discovery; no feature/output reports are sent.
use anyhow::Context as _;
use serde_json::{Value, json};
use std::{mem::size_of, ptr};
use windows_sys::{
    Win32::{
        Devices::{
            DeviceAndDriverInstallation::*,
            HumanInterfaceDevice::*,
            Properties::{
                DEVPKEY_Device_ContainerId, DEVPKEY_Device_DeviceDesc, DEVPKEY_Device_Manufacturer,
                DEVPROP_TYPE_GUID, DEVPROP_TYPE_STRING,
            },
        },
        Foundation::{
            CloseHandle, DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError,
            HANDLE, INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
    },
    core::GUID,
};

mod driver_interface;
pub(crate) use driver_interface::{DriverInterface, select as select_driver_interface};

pub(super) struct DeviceSet(pub(super) HDEVINFO);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

struct HidHandle(HANDLE);

/// RzNative current SetMicSetup -> hid_set_output_report sends exactly two
/// bytes through HidD_SetOutputReport. The caller retains the collection lock.
pub(super) fn mixer_control_output(
    node: &razer_device::backend::HidNode,
    report: &[u8],
) -> anyhow::Result<()> {
    use anyhow::ensure;
    ensure!(
        node.vendor_id == 0x1532
            && node.product_id == 1342
            && report.len() == 2
            && report[0] == 10
            && [2, 5, 6].contains(&report[1]),
        "Unexpected native MicSetup control report"
    );
    let path = std::str::from_utf8(&node.path).context("Mixer HID path is not UTF-8")?;
    ensure!(!path.contains('\0'), "Mixer HID path contains NUL");
    let path = path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let handle = HidHandle(unsafe {
        CreateFileW(
            path.as_ptr(),
            0xc0000000,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    });
    ensure!(
        handle.0 != INVALID_HANDLE_VALUE,
        "Open native Mixer control handle failed: {}",
        std::io::Error::last_os_error()
    );
    let mut attrs = HIDD_ATTRIBUTES {
        Size: size_of::<HIDD_ATTRIBUTES>() as u32,
        ..Default::default()
    };
    ensure!(
        unsafe { HidD_GetAttributes(handle.0, &mut attrs) },
        "Read native Mixer identity failed: {}",
        std::io::Error::last_os_error()
    );
    ensure!(
        attrs.VendorID == node.vendor_id
            && attrs.ProductID == node.product_id
            && attrs.VersionNumber == node.release_number,
        "Native Mixer identity changed"
    );
    let mut raw = 0;
    ensure!(
        unsafe { HidD_GetPreparsedData(handle.0, &mut raw) },
        "Read native Mixer collection failed"
    );
    let data = PreparsedData(raw);
    let mut caps = HIDP_CAPS::default();
    ensure!(
        unsafe { HidP_GetCaps(data.0, &mut caps) } == HIDP_STATUS_SUCCESS,
        "Read native Mixer caps failed"
    );
    ensure!(
        caps.Usage == node.usage
            && caps.UsagePage == node.usage_page
            && caps.OutputReportByteLength >= 2,
        "Native Mixer collection changed"
    );
    ensure!(
        unsafe { HidD_SetOutputReport(handle.0, report.as_ptr().cast(), report.len() as u32) },
        "MicSetup control Output failed: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
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

/// Current HID.node hid_enumerate (RVA 0x16aa0) reads DeviceDesc and
/// Manufacturer from SetupAPI. It never sends HidD_Get*String requests here.
/// The property keys are retained at RVAs 0x51f50 and 0x51f68 respectively.
fn device_string_property(
    set: HDEVINFO,
    device: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> anyhow::Result<String> {
    device_string_property_with(|buffer, property_type, required| {
        if unsafe {
            SetupDiGetDevicePropertyW(
                set,
                device,
                key,
                property_type,
                buffer.as_mut_ptr().cast(),
                size_of_val(buffer) as u32,
                required,
                0,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    })
}

fn device_string_property_with(
    read: impl FnOnce(&mut [u16], &mut u32, &mut u32) -> std::io::Result<()>,
) -> anyhow::Result<String> {
    let mut buffer = [0u16; 256];
    let mut property_type = 0;
    let mut required = 0;
    read(&mut buffer, &mut property_type, &mut required)?;
    anyhow::ensure!(
        property_type == DEVPROP_TYPE_STRING
            && required >= 2
            && required as usize <= size_of_val(&buffer)
            && required % 2 == 0,
        "设备字符串属性类型或长度错误"
    );
    let returned = &buffer[..required as usize / 2];
    let length = returned
        .iter()
        .position(|ch| *ch == 0)
        .context("设备字符串属性缺少终止符")?;
    Ok(String::from_utf16(&returned[..length])?)
}

fn inventory_display_metadata_with(
    enabled: bool,
    mut read: impl FnMut(&DEVPROPKEY) -> anyhow::Result<String>,
) -> (Option<String>, Option<String>, Vec<Value>) {
    if !enabled {
        return (None, None, Vec::new());
    }
    let mut diagnostics = Vec::new();
    let mut field = |operation, key: &DEVPROPKEY| match read(key) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.push(json!({"operation": operation, "error": format!("{error:#}")}));
            None
        }
    };
    let manufacturer = field("read_manufacturer_property", &DEVPKEY_Device_Manufacturer);
    let product = field("read_product_property", &DEVPKEY_Device_DeviceDesc);
    (manufacturer, product, diagnostics)
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

/// Current noble/util.hidDetectDeviceInfoParser inspects the immediate parent
/// devnode to distinguish a real BTHLEDevice endpoint from USB/HID collections.
fn parent_instance_id(device: &SP_DEVINFO_DATA) -> anyhow::Result<String> {
    let mut parent = 0;
    let status = unsafe { CM_Get_Parent(&mut parent, device.DevInst, 0) };
    anyhow::ensure!(status == CR_SUCCESS, "CM_Get_Parent returned {status:#x}");
    let mut size = 0;
    let status = unsafe { CM_Get_Device_ID_Size(&mut size, parent, 0) };
    anyhow::ensure!(
        status == CR_SUCCESS && size < 32768,
        "CM_Get_Device_ID_Size returned {status:#x}"
    );
    let mut buffer = vec![0u16; size as usize + 1];
    let status = unsafe { CM_Get_Device_IDW(parent, buffer.as_mut_ptr(), buffer.len() as u32, 0) };
    anyhow::ensure!(
        status == CR_SUCCESS,
        "CM_Get_Device_IDW returned {status:#x}"
    );
    Ok(String::from_utf16(&buffer[..size as usize])?)
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
    read_display_metadata: bool,
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
        let (parent_instance, parent_error) = match parent_instance_id(&device_info) {
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
        // Optional Windows display properties are distinct from transport
        // failures. Preserve their diagnostics without claiming a device read
        // failed. Source HID enumeration does not obtain a hardware serial;
        // that remains the device protocol reader's responsibility.
        let (manufacturer, product, metadata_errors) =
            inventory_display_metadata_with(read_display_metadata, |key| {
                device_string_property(set.0, &device_info, key)
            });
        interfaces.push(json!({
            "path": path,
            "vendor_id": attributes.VendorID,
            "product_id": attributes.ProductID,
            "version_number": attributes.VersionNumber,
            "manufacturer": manufacturer,
            "product": product,
            "serial_number": Value::Null,
            "metadata_source": "windows_device_properties",
            "metadata_errors": metadata_errors,
            "device_container_id": container_id,
            "container_id_error": container_error,
            "device_instance_id": instance_id,
            "parent_instance_id": parent_instance,
            "parent_instance_id_error": parent_error,
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

#[cfg(test)]
mod inventory_tests {
    use super::*;

    #[test]
    fn inventory_reads_only_the_source_display_properties_and_keeps_failures_separate() {
        let mut properties = Vec::new();
        let (manufacturer, product, diagnostics) = inventory_display_metadata_with(true, |key| {
            assert_eq!(key.fmtid.data1, 0xa45c254e);
            assert_eq!(key.fmtid.data2, 0xdf1c);
            assert_eq!(key.fmtid.data3, 0x4efd);
            assert_eq!(
                key.fmtid.data4,
                [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0]
            );
            properties.push(key.pid);
            if key.pid == 13 {
                Ok("Razer".into())
            } else {
                anyhow::bail!("property unavailable")
            }
        });
        assert_eq!(properties, vec![13, 2]);
        assert_eq!(manufacturer.as_deref(), Some("Razer"));
        assert!(product.is_none());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["operation"], "read_product_property");
        assert_eq!(diagnostics[0]["error"], "property unavailable");
    }

    #[test]
    fn target_revalidation_does_not_read_display_properties() {
        let (manufacturer, product, diagnostics) = inventory_display_metadata_with(false, |_| {
            panic!("metadata is not needed to revalidate a transport target")
        });
        assert!(manufacturer.is_none() && product.is_none() && diagnostics.is_empty());
    }

    #[test]
    fn property_reader_preserves_utf16_and_rejects_invalid_or_failed_reads() {
        let name = "Razer 炼狱蝰蛇";
        let text = device_string_property_with(|buffer, kind, size| {
            assert_eq!(size_of_val(buffer), 512);
            let data: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
            buffer[..data.len()].copy_from_slice(&data);
            *kind = DEVPROP_TYPE_STRING;
            *size = (data.len() * 2) as u32;
            Ok(())
        })
        .unwrap();
        assert_eq!(text, name);

        for (kind, bytes, first) in [
            (DEVPROP_TYPE_GUID, 2, 0),
            (DEVPROP_TYPE_STRING, 0, 0),
            (DEVPROP_TYPE_STRING, 3, 0),
            (DEVPROP_TYPE_STRING, 514, 0),
            (DEVPROP_TYPE_STRING, 2, 65),
            (DEVPROP_TYPE_STRING, 4, 0xd800),
        ] {
            assert!(
                device_string_property_with(|buffer, actual_kind, actual_bytes| {
                    buffer[0] = first;
                    *actual_kind = kind;
                    *actual_bytes = bytes;
                    Ok(())
                })
                .is_err()
            );
        }
        let error =
            device_string_property_with(|_, _, _| Err(std::io::Error::from_raw_os_error(5)))
                .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(5)
        );
    }
}
