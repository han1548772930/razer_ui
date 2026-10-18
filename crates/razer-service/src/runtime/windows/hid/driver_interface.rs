//! Select a present driver interface by its class and observed physical container.
use anyhow::{Context as _, ensure};
use std::{mem::size_of, ptr};
use windows_sys::{Win32::{Devices::DeviceAndDriverInstallation::*, Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError}}, core::GUID};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DriverInterface {
    pub(crate) path: Vec<u16>,
    pub(crate) instance: String,
    pub(crate) container: String,
}

pub(crate) fn select(container: &str, guid: &GUID) -> anyhow::Result<DriverInterface> {
    let raw = unsafe {
        SetupDiGetClassDevsW(
            guid,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    ensure!(
        raw != -1,
        "Audio 驱动接口枚举失败：{}",
        std::io::Error::last_os_error()
    );
    let set = super::DeviceSet(raw);
    let mut found = Vec::new();
    for index in 0..u32::MAX {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, ptr::null(), guid, index, &mut interface) }
            == 0
        {
            let error = unsafe { GetLastError() };
            ensure!(
                error == ERROR_NO_MORE_ITEMS,
                "Audio 驱动枚举未完整结束：{}",
                std::io::Error::from_raw_os_error(error as i32)
            );
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
        ensure!(
            unsafe { GetLastError() } == ERROR_INSUFFICIENT_BUFFER
                && required >= size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32
                && required <= 1024 * 1024,
            "Audio 驱动接口路径大小无效"
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
        ensure!(
            unsafe {
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    detail,
                    required,
                    &mut required,
                    &mut device,
                )
            } != 0,
            "Audio 驱动路径读取失败：{}",
            std::io::Error::last_os_error()
        );
        let observed_container = super::device_container_id(set.0, &device)?;
        if !observed_container.eq_ignore_ascii_case(container) {
            continue;
        }
        let pointer = unsafe { ptr::addr_of!((*detail).DevicePath).cast::<u16>() };
        let offset = pointer as usize - storage.as_ptr() as usize;
        ensure!(
            required as usize >= offset + 2 && required as usize <= storage.len() * 4,
            "Audio 驱动路径长度异常"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(pointer, (required as usize - offset) / 2) };
        let end = slice
            .iter()
            .position(|v| *v == 0)
            .context("Audio 驱动路径缺少终止符")?;
        ensure!(end > 0, "Audio 驱动路径为空");
        found.push(DriverInterface {
            path: slice[..=end].to_vec(),
            instance: super::device_instance_id(set.0, &device)?,
            container: observed_container,
        });
    }
    ensure!(
        found.len() == 1,
        "当前 Audio 容器没有唯一的源驱动接口；不回退到首个接口或广播全部设备"
    );
    Ok(found.remove(0))
}

