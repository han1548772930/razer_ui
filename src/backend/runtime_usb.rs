//! Explicit read-only physical USB enumeration, matching the current native
//! detection.node USB_DEVICE scope without loading its Node/NAN entry point.
use super::hid::{DeviceSet, device_container_id, device_instance_id};
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeSet, mem::size_of, ptr};
use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::*,
        Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError},
        System::Registry::{REG_MULTI_SZ, REG_SZ},
    },
    core::GUID,
};

// Current detection.node RVA 0x45cc0; see usb-native-current-evidence.json.
const USB_DEVICE: GUID = GUID::from_u128(0xa5dcbf10_6530_11d2_901f_00c04fb951ed);

fn registry_strings(
    set: HDEVINFO,
    device: &SP_DEVINFO_DATA,
    property: u32,
    multi: bool,
) -> anyhow::Result<Vec<String>> {
    let mut kind = 0;
    let mut required = 0;
    let result = unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set,
            device,
            property,
            &mut kind,
            ptr::null_mut(),
            0,
            &mut required,
        )
    };
    let error = unsafe { GetLastError() };
    ensure!(
        result == 0 && error == ERROR_INSUFFICIENT_BUFFER,
        "读取USB属性 {property} 长度失败：{}",
        std::io::Error::from_raw_os_error(error as i32)
    );
    ensure!(
        (2..=1024 * 1024).contains(&required) && required % 2 == 0,
        "USB属性 {property} 长度异常"
    );
    let mut buffer = vec![0u16; required as usize / 2];
    let capacity = required;
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set,
            device,
            property,
            &mut kind,
            buffer.as_mut_ptr().cast(),
            capacity,
            &mut required,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("读取USB设备属性失败");
    }
    ensure!(
        required <= capacity && required >= 2 && required % 2 == 0,
        "USB属性返回长度异常"
    );
    ensure!(
        kind == if multi { REG_MULTI_SZ } else { REG_SZ },
        "USB属性类型不匹配"
    );
    buffer.truncate(required as usize / 2);
    ensure!(buffer.last() == Some(&0), "USB属性缺少终止符");
    if multi {
        ensure!(
            buffer.len() >= 2 && buffer[buffer.len() - 2] == 0,
            "USB多字符串缺少双终止符"
        );
        buffer
            .split(|value| *value == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf16(part).map_err(Into::into))
            .collect()
    } else {
        let end = buffer.iter().position(|value| *value == 0).unwrap();
        Ok(vec![String::from_utf16(&buffer[..end])?])
    }
}

fn usb_ids(value: &str) -> Option<(u16, u16)> {
    let upper = value.to_ascii_uppercase();
    let mut parts = upper.split('\\');
    if parts.next()? != "USB" {
        return None;
    }
    let identity = parts.next()?;
    let mut vendor = None;
    let mut product = None;
    for component in identity.split('&') {
        let (target, value) = if let Some(value) = component.strip_prefix("VID_") {
            (&mut vendor, value)
        } else if let Some(value) = component.strip_prefix("PID_") {
            (&mut product, value)
        } else {
            continue;
        };
        if value.len() != 4 || target.is_some() {
            return None;
        }
        *target = Some(u16::from_str_radix(value, 16).ok()?);
    }
    Some((vendor?, product?))
}

fn interface_detail(
    set: HDEVINFO,
    interface: &SP_DEVICE_INTERFACE_DATA,
) -> anyhow::Result<(String, SP_DEVINFO_DATA)> {
    let mut required = 0;
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            set,
            interface,
            ptr::null_mut(),
            0,
            &mut required,
            ptr::null_mut(),
        );
    }
    let error = unsafe { GetLastError() };
    ensure!(
        error == ERROR_INSUFFICIENT_BUFFER,
        "读取USB路径长度失败：{}",
        std::io::Error::from_raw_os_error(error as i32)
    );
    ensure!(
        required >= size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32
            && required <= 1024 * 1024,
        "USB路径长度异常"
    );
    let mut storage = vec![0u32; (required as usize).div_ceil(size_of::<u32>())];
    let detail = storage
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
            set,
            interface,
            detail,
            required,
            &mut required,
            &mut device,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("读取USB接口路径失败");
    }
    let pointer = unsafe { ptr::addr_of!((*detail).DevicePath).cast::<u16>() };
    let offset = pointer as usize - storage.as_ptr() as usize;
    ensure!(
        required as usize >= offset + 2 && required as usize <= storage.len() * 4,
        "USB路径返回长度异常"
    );
    let slice = unsafe { std::slice::from_raw_parts(pointer, (required as usize - offset) / 2) };
    let end = slice
        .iter()
        .position(|value| *value == 0)
        .context("USB路径缺少终止符")?;
    let path = String::from_utf16(&slice[..end])?;
    ensure!(!path.is_empty(), "USB路径为空");
    Ok((path, device))
}

pub(super) fn enumerate() -> anyhow::Result<Value> {
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &USB_DEVICE,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if raw == -1 {
        return Err(std::io::Error::last_os_error()).context("无法枚举USB设备");
    }
    let set = DeviceSet(raw);
    let mut devices = vec![];
    let mut failures = vec![];
    let mut reached_end = false;
    for index in 0..u32::MAX {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe {
            SetupDiEnumDeviceInterfaces(set.0, ptr::null(), &USB_DEVICE, index, &mut interface)
        } == 0
        {
            let error = unsafe { GetLastError() };
            if error == ERROR_NO_MORE_ITEMS {
                reached_end = true;
                break;
            }
            failures.push(
                json!({"interface_index":index,"operation":"enumerate_usb_interface",
                "error":std::io::Error::from_raw_os_error(error as i32).to_string()}),
            );
            break;
        }
        let (path, device) = match interface_detail(set.0, &interface) {
            Ok(value) => value,
            Err(error) => {
                failures.push(json!({"interface_index":index,"operation":"read_usb_interface","error":format!("{error:#}")}));
                continue;
            }
        };
        let hardware_ids = match registry_strings(set.0, &device, SPDRP_HARDWAREID, true) {
            Ok(value) => value,
            Err(error) => {
                failures.push(json!({"path":path,"operation":"read_usb_hardware_ids","error":format!("{error:#}")}));
                continue;
            }
        };
        let identities: BTreeSet<_> = hardware_ids
            .iter()
            .filter_map(|value| usb_ids(value))
            .collect();
        if identities.is_empty() {
            continue;
        } // Root hubs need not carry a VID/PID.
        if identities.len() != 1 {
            failures.push(json!({"path":path,"operation":"read_usb_identity","error":"USB硬件ID包含冲突的VID/PID"}));
            continue;
        }
        let (vendor_id, product_id) = *identities.first().unwrap();
        let mut observe = |operation: &str, result: anyhow::Result<String>| match result {
            Ok(value) => Some(value),
            Err(error) => {
                failures
                    .push(json!({"path":path,"operation":operation,"error":format!("{error:#}")}));
                None
            }
        };
        let container = observe("read_usb_container", device_container_id(set.0, &device));
        let instance = observe("read_usb_instance", device_instance_id(set.0, &device));
        if instance
            .as_deref()
            .and_then(usb_ids)
            .is_some_and(|identity| identity != (vendor_id, product_id))
        {
            failures.push(json!({"path":path,"operation":"validate_usb_identity","error":"USB实例与硬件ID不一致"}));
            continue;
        }
        // Optional Windows display strings are not device serial/firmware queries.
        let string = |property| {
            registry_strings(set.0, &device, property, false)
                .ok()
                .and_then(|values| values.into_iter().next())
                .filter(|value| !value.is_empty())
        };
        let product = string(SPDRP_FRIENDLYNAME).or_else(|| string(SPDRP_DEVICEDESC));
        let manufacturer = string(SPDRP_MFG);
        devices.push(json!({"vendor_id":vendor_id,"product_id":product_id,
            "device_container_id":container,"device_instance_id":instance,"path":path,
            "hardware_ids":hardware_ids,"product":product,"manufacturer":manufacturer,
            "serial_number":Value::Null,"identity_source":"windows_usb_device_interface"}));
    }
    Ok(
        json!({"devices":devices,"failures":failures,"complete":reached_end,
        "enumeration":"USB_DEVICE","guid":"a5dcbf10-6530-11d2-901f-00c04fb951ed"}),
    )
}
