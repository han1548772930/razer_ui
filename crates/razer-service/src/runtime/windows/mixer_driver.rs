//! CmMixerLib driver calls, statically recovered from the current DLL.
//! OS-only adapter; matrix layout/encoding and stream sequence stay in device.
use super::hid;
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer::{self, MixerDriverTransport, MixerRoute},
    backend::{HidBackend, HidNode},
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    mem::size_of,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::*,
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GetLastError, HANDLE,
            INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING},
        System::IO::DeviceIoControl,
    },
    core::GUID,
};

#[derive(Clone, PartialEq, Eq)]
struct Interface {
    path: Vec<u16>,
    instance: String,
    container: String,
}

fn class_guid() -> anyhow::Result<GUID> {
    Ok(GUID::from_u128(u128::from_str_radix(
        &audio_mixer::driver_spec()
            .device_interface_guid
            .replace('-', ""),
        16,
    )?))
}

fn select_interface(container: &str) -> anyhow::Result<Interface> {
    let guid = class_guid()?;
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
        "Mixer 驱动接口枚举失败：{}",
        std::io::Error::last_os_error()
    );
    let set = hid::DeviceSet(raw);
    let mut found = Vec::new();
    for index in 0..u32::MAX {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, ptr::null(), &guid, index, &mut interface) }
            == 0
        {
            let error = unsafe { GetLastError() };
            ensure!(
                error == ERROR_NO_MORE_ITEMS,
                "Mixer 驱动枚举未完整结束：{}",
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
            "Mixer 驱动接口路径大小无效"
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
            "Mixer 驱动路径读取失败：{}",
            std::io::Error::last_os_error()
        );
        let observed_container = hid::device_container_id(set.0, &device)?;
        if !observed_container.eq_ignore_ascii_case(container) {
            continue;
        }
        let pointer = unsafe { ptr::addr_of!((*detail).DevicePath).cast::<u16>() };
        let offset = pointer as usize - storage.as_ptr() as usize;
        ensure!(
            required as usize >= offset + 2 && required as usize <= storage.len() * 4,
            "Mixer 驱动路径长度异常"
        );
        let slice =
            unsafe { std::slice::from_raw_parts(pointer, (required as usize - offset) / 2) };
        let end = slice
            .iter()
            .position(|v| *v == 0)
            .context("Mixer 驱动路径缺少终止符")?;
        ensure!(end > 0, "Mixer 驱动路径为空");
        found.push(Interface {
            path: slice[..=end].to_vec(),
            instance: hid::device_instance_id(set.0, &device)?,
            container: observed_container,
        });
    }
    ensure!(
        found.len() == 1,
        "当前 Mixer 容器没有唯一的源驱动接口；不回退到首个接口或广播全部设备"
    );
    Ok(found.remove(0))
}

fn observe_node(node: &HidNode, product_id: u32) -> anyhow::Result<Value> {
    ensure!(
        audio_mixer::accepts(product_id, node.vendor_id, node.product_id),
        "目标不是源 Audio Mixer 产品"
    );
    let nodes = with_backend(|b| b.enumerate())?;
    ensure!(
        nodes.iter().filter(|n| *n == node).count() == 1,
        "Mixer HID collection 身份变化或不唯一"
    );
    let path =
        std::str::from_utf8(&node.path).context("Mixer Windows HID 路径编码无法对应当前身份")?;
    ensure!(
        path.is_ascii() && !path.contains('\0'),
        "Mixer HID 路径无法准确匹配 Windows 元数据"
    );
    let snapshot = hid::enumerate_for_product(node.vendor_id, node.product_id)?;
    ensure!(snapshot["complete"] == true, "Mixer HID 身份枚举未完成");
    let items = snapshot["interfaces"]
        .as_array()
        .context("Mixer HID 身份列表缺失")?;
    let matches = items
        .iter()
        .filter(|i| {
            i["path"]
                .as_str()
                .is_some_and(|p| p.eq_ignore_ascii_case(path))
        })
        .collect::<Vec<_>>();
    ensure!(matches.len() == 1, "Mixer HID 路径未对应唯一 Windows 接口");
    let identity = matches[0];
    ensure!(
        identity["device_container_id"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
            && identity["device_instance_id"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
            && identity["vendor_id"] == node.vendor_id
            && identity["product_id"] == node.product_id,
        "Mixer 缺少真实容器/实例身份"
    );
    Ok(identity.clone())
}

struct Driver(HANDLE);
impl Drop for Driver {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

impl Driver {
    fn open(interface: &Interface, attributes: u32) -> anyhow::Result<Self> {
        // Original exclusive synchronous CreateFileW; no downloaded DLL.
        let handle = unsafe {
            CreateFileW(
                interface.path.as_ptr(),
                0xc0000000,
                0,
                ptr::null(),
                OPEN_EXISTING,
                attributes,
                ptr::null_mut(),
            )
        };
        ensure!(
            handle != INVALID_HANDLE_VALUE,
            "Mixer 驱动打开失败：{}",
            std::io::Error::last_os_error()
        );
        Ok(Self(handle))
    }
}

impl MixerDriverTransport for Driver {
    fn read_matrix(&self) -> anyhow::Result<Vec<u8>> {
        let spec = audio_mixer::driver_spec();
        let mut bytes = vec![0u8; spec.matrix_bytes];
        let mut returned = 0;
        ensure!(
            unsafe {
                DeviceIoControl(
                    self.0,
                    spec.matrix_read_ioctl,
                    ptr::null(),
                    0,
                    bytes.as_mut_ptr().cast(),
                    bytes.len() as u32,
                    &mut returned,
                    ptr::null_mut(),
                )
            } != 0,
            "Mixer 矩阵查询失败：{}",
            std::io::Error::last_os_error()
        );
        ensure!(
            returned as usize == bytes.len(),
            "Mixer 矩阵实际返回长度与原 112 字节不符"
        );
        Ok(bytes)
    }
    fn write_matrix(&self, bytes: &[u8]) -> anyhow::Result<()> {
        let spec = audio_mixer::driver_spec();
        ensure!(
            bytes.len() == spec.matrix_bytes,
            "Mixer 矩阵写入长度无原码依据"
        );
        let mut returned = 0;
        ensure!(
            unsafe {
                DeviceIoControl(
                    self.0,
                    spec.matrix_write_ioctl,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    ptr::null_mut(),
                    0,
                    &mut returned,
                    ptr::null_mut(),
                )
            } != 0,
            "Mixer 矩阵写入未确认：{}",
            std::io::Error::last_os_error()
        );
        Ok(())
    }
    fn reset_stream(&self, index: u32) -> anyhow::Result<u32> {
        let spec = audio_mixer::driver_spec();
        ensure!(
            spec.stream_indices.contains(&index),
            "流索引不在原 restartAudioDriver 序列中"
        );
        let input = index.to_le_bytes();
        let mut output = [0u8; 4];
        let mut returned = 0;
        let succeeded = unsafe {
            DeviceIoControl(
                self.0,
                spec.reset_stream_ioctl,
                input.as_ptr().cast(),
                4,
                output.as_mut_ptr().cast(),
                4,
                &mut returned,
                ptr::null_mut(),
            )
        } != 0;
        // The original caller ignores this buffer. Do not invent state fields.
        let _ = (output, returned);
        Ok(if succeeded { 0 } else { 0x10003 })
    }
}

struct StreamDriver<'a>(&'a Interface);

impl MixerDriverTransport for StreamDriver<'_> {
    fn read_matrix(&self) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("流重置适配不支持矩阵查询")
    }

    fn write_matrix(&self, _: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("流重置适配不支持矩阵设置")
    }

    fn reset_stream(&self, index: u32) -> anyhow::Result<u32> {
        // Each original property call opens and closes its own driver handle.
        let driver = match Driver::open(self.0, FILE_ATTRIBUTE_NORMAL) {
            Ok(driver) => driver,
            Err(_) => return Ok(0x10001),
        };
        driver.reset_stream(index)
    }
}

fn request(
    node: HidNode,
    product_id: u32,
    perform: impl FnOnce(&Interface, &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<Value>,
) -> anyhow::Result<Value> {
    let started = Instant::now();
    let before = observe_node(&node, product_id)?;
    let container = before["device_container_id"]
        .as_str()
        .context("Mixer 容器缺失")?;
    let selected = select_interface(container)?;
    // Same portable path lock as DSP operations; driver handle is also exclusive.
    let _hid_lock = with_backend(|b| b.open(&node))?;
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Mixer 驱动操作超过确认期限"
        );
        ensure!(
            observe_node(&node, product_id)? == before,
            "Mixer HID 身份变化，未确认操作结果"
        );
        ensure!(
            select_interface(container)? == selected,
            "Mixer 驱动实例身份变化，未确认操作结果"
        );
        Ok(())
    };
    validate()?;
    let result = perform(&selected, &validate)?;
    validate()?;
    Ok(json!({"node":node,"product_id":product_id,"result":result,
        "device_container_id":container,"driver_instance_id":selected.instance,
        "transport":"windows_scoped_driver_ioctl","vendor_dll_loaded":false,
        "identity_policy":"unique_observed_container_and_driver_interface",
        "elapsed_ms":started.elapsed().as_millis() as u64,
        "evidence":"docs/re/audio-mixer-controls-current-evidence.json"}))
}

pub(super) fn route(
    node: HidNode,
    product_id: u32,
    route: MixerRoute,
    enabled: Option<bool>,
) -> anyhow::Result<Value> {
    ensure!(
        matches!(
            audio_mixer::matrix_route(&route)?,
            audio_mixer::MatrixRoute::Driver { .. }
        ),
        "HID 路由不能改走驱动"
    );
    request(node, product_id, |interface, validate| {
        let driver = Driver::open(interface, 0)?;
        let result = if let Some(enabled) = enabled {
            serde_json::to_value(audio_mixer::write_driver_route(
                &driver, &route, enabled, validate,
            )?)?
        } else {
            json!({"enabled":audio_mixer::read_driver_route(&driver, &route, validate)?})
        };
        Ok(json!({"route":route,"value":result,"source_property":"RazerT2MixerSettingControl"}))
    })
}

pub(super) fn restart(node: HidNode, product_id: u32) -> anyhow::Result<Value> {
    request(node, product_id, |interface, validate| {
        let driver = StreamDriver(interface);
        Ok(
            json!({"reset":audio_mixer::restart_streams(&driver, validate)?,"source_property":"RazerT2ResetStream"}),
        )
    })
}
