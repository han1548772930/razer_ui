//! Direct Windows HID input; buffers stay on their creating worker until drained.
use super::super::super::hid;
use super::super::identity::DeviceInfo;
use super::{Collection, Read, Reader, Transport, driver};
use anyhow::{Context as _, Result, ensure};
use std::{io, mem::size_of, ptr};
use windows_sys::{
    Win32::{
        Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
        Foundation::{
            CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING,
            ERROR_NO_MORE_ITEMS, ERROR_NOT_FOUND, ERROR_OPERATION_ABORTED, GENERIC_READ,
            GetLastError, HANDLE, INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            ReadFile,
        },
        System::{
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
            Threading::{CreateEventW, ResetEvent},
        },
    },
    core::GUID,
};

pub(super) struct Windows {
    info: DeviceInfo,
}

impl Windows {
    pub(super) fn new(info: DeviceInfo) -> Result<Self> {
        ensure!(
            info.vendor_id == 0x1532 && u16::try_from(info.product_id).is_ok(),
            "Invalid mapping hardware device identity"
        );
        let container = uuid::Uuid::parse_str(info.container_id.trim_matches(['{', '}']))?;
        ensure!(
            !container.is_nil() && !info.usb_instance_id.is_empty(),
            "Mapping hardware physical identity is unobserved"
        );
        Ok(Self { info })
    }
}

impl Transport for Windows {
    type Reader = Input;

    fn collections(&mut self) -> Result<Vec<Collection>> {
        discover(&self.info)
    }

    fn open(&mut self, collection: &Collection) -> Result<Input> {
        let matches = |candidate: &Collection| {
            candidate.path.eq_ignore_ascii_case(&collection.path)
                && candidate
                    .instance
                    .eq_ignore_ascii_case(&collection.instance)
        };
        ensure!(
            discover(&self.info)?
                .iter()
                .filter(|item| matches(item))
                .count()
                == 1,
            "Mapping hardware collection changed before open"
        );
        let input = Input::open(&self.info, collection)?;
        // A path alone can be recycled. Both SetupDi identity observations
        // enclose the opened-handle VID/PID check before its first delivery.
        ensure!(
            discover(&self.info)?
                .iter()
                .filter(|item| matches(item))
                .count()
                == 1,
            "Mapping hardware collection changed during open"
        );
        Ok(input)
    }

    fn capture(&mut self, bytes: Vec<u8>) -> Result<driver::Packet> {
        driver::Packet::capture(bytes)
    }
}

fn discover(info: &DeviceInfo) -> Result<Vec<Collection>> {
    let mut class = GUID::default();
    unsafe {
        HidD_GetHidGuid(&mut class);
    }
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &class,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    ensure!(
        raw != -1,
        "Mapping hardware SetupDi discovery failed: {}",
        io::Error::last_os_error()
    );
    let set = hid::DeviceSet(raw);
    let mut found = Vec::new();
    for index in 0..u32::MAX {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, ptr::null(), &class, index, &mut interface) }
            == 0
        {
            let code = unsafe { GetLastError() };
            ensure!(
                code == ERROR_NO_MORE_ITEMS,
                "Mapping hardware enumeration incomplete: {}",
                io::Error::from_raw_os_error(code as i32)
            );
            return Ok(found);
        }
        let mut required = 0;
        let result = unsafe {
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
            result == 0 && unsafe { GetLastError() } == ERROR_INSUFFICIENT_BUFFER,
            "Mapping hardware path size query failed"
        );
        ensure!(
            required >= size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32
                && required <= 1024 * 1024,
            "Mapping hardware path size invalid"
        );
        // The API detail structure has u32 alignment, including its cbSize.
        let mut storage = vec![0u32; (required as usize).div_ceil(size_of::<u32>())];
        let capacity = storage.len() * size_of::<u32>();
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
            "Mapping hardware path read failed: {}",
            io::Error::last_os_error()
        );
        let pointer = unsafe { ptr::addr_of!((*detail).DevicePath).cast::<u16>() };
        let offset = pointer as usize - storage.as_ptr() as usize;
        ensure!(
            required as usize >= offset + 2 && required as usize <= capacity,
            "Mapping hardware path boundary invalid"
        );
        // The length was checked against the retained aligned allocation.
        let units =
            unsafe { std::slice::from_raw_parts(pointer, (required as usize - offset) / 2) };
        let end = units
            .iter()
            .position(|unit| *unit == 0)
            .context("Mapping hardware path has no terminator")?;
        let path = String::from_utf16(&units[..end])?;
        let lower = path.to_ascii_lowercase();
        let Some(pid_offset) = lower.find("pid_").map(|offset| offset + 4) else {
            continue;
        };
        let Some(pid) = lower
            .get(pid_offset..pid_offset + 4)
            .and_then(|digits| u16::from_str_radix(digits, 16).ok())
        else {
            continue;
        };
        if u32::from(pid) != info.product_id {
            continue;
        }
        let container = hid::device_container_id(set.0, &device)?;
        if !container.eq_ignore_ascii_case(&info.container_id) {
            continue;
        }
        let instance = hid::device_instance_id(set.0, &device)?;
        ensure!(
            !path.is_empty() && !instance.is_empty(),
            "Mapping hardware collection identity is empty"
        );
        found.push(Collection { path, instance });
    }
    anyhow::bail!("Mapping hardware enumeration did not terminate")
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

struct Request {
    overlapped: OVERLAPPED,
    bytes: [u8; 100],
    event: Handle,
    pending: bool,
}

impl Request {
    fn new() -> io::Result<Box<Self>> {
        let event = Handle(unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) });
        if event.0.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Box::new(Self {
            overlapped: OVERLAPPED {
                hEvent: event.0,
                ..Default::default()
            },
            bytes: [0; 100],
            event,
            pending: false,
        }))
    }
}

pub(super) struct Input {
    handle: Option<Handle>,
    requests: [Box<Request>; 2],
    current: usize,
    started: bool,
}

impl Input {
    fn open(info: &DeviceInfo, collection: &Collection) -> Result<Self> {
        ensure!(
            !collection.path.contains('\0'),
            "Mapping hardware path contains NUL"
        );
        let path = collection
            .path
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let handle = Handle(unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        });
        if handle.0 == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error()).context("Open mapping hardware collection");
        }
        let mut attributes = HIDD_ATTRIBUTES {
            Size: size_of::<HIDD_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        ensure!(
            unsafe { HidD_GetAttributes(handle.0, &mut attributes) },
            "Read opened mapping HID identity failed: {}",
            io::Error::last_os_error()
        );
        ensure!(
            u32::from(attributes.VendorID) == info.vendor_id
                && u32::from(attributes.ProductID) == info.product_id,
            "Opened mapping HID collection belongs to another device"
        );
        Ok(Self {
            handle: Some(handle),
            requests: [Request::new()?, Request::new()?],
            current: 0,
            started: false,
        })
    }

    fn start(&mut self, index: usize) -> io::Result<()> {
        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| io::Error::from_raw_os_error(6))?
            .0;
        let request = &mut self.requests[index];
        if request.pending {
            return Err(io::Error::from_raw_os_error(997));
        }
        request.overlapped = OVERLAPPED {
            hEvent: request.event.0,
            ..Default::default()
        };
        request.bytes.fill(0);
        if unsafe { ResetEvent(request.event.0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // Both OVERLAPPED and its output array are heap-owned and never
        // moved or released until this request's final completion is read.
        let result = unsafe {
            ReadFile(
                handle,
                request.bytes.as_mut_ptr(),
                100,
                ptr::null_mut(),
                &mut request.overlapped,
            )
        };
        if result == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(error);
            }
        }
        request.pending = true;
        self.current = index;
        Ok(())
    }
}

impl Reader for Input {
    fn poll(&mut self) -> io::Result<Option<Read>> {
        if !self.started {
            self.started = true;
            self.start(0)?;
        }
        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| io::Error::from_raw_os_error(6))?
            .0;
        let request = &mut self.requests[self.current];
        if !request.pending {
            return Ok(None);
        }
        let mut count = 0;
        let result = unsafe { GetOverlappedResult(handle, &request.overlapped, &mut count, 0) };
        if result == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_IO_INCOMPLETE as i32) {
                return Ok(None);
            }
            request.pending = false;
            return Err(error);
        }
        request.pending = false;
        if count as usize > request.bytes.len() {
            return Err(io::Error::from_raw_os_error(13));
        }
        let bytes = request.bytes[..count as usize].to_vec();
        let rearm_error = self.start(1 - self.current).err();
        Ok(Some(Read { bytes, rearm_error }))
    }

    fn close(&mut self) -> Result<()> {
        let Some(handle) = self.handle.as_ref() else {
            return Ok(());
        };
        let mut errors = Vec::new();
        for request in &mut self.requests {
            if !request.pending {
                continue;
            }
            if unsafe { CancelIoEx(handle.0, &request.overlapped) } == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_NOT_FOUND as i32) {
                    errors.push(format!("Cancel input: {error}"));
                }
            }
        }
        for request in &mut self.requests {
            if !request.pending {
                continue;
            }
            loop {
                let mut count = 0;
                let result =
                    unsafe { GetOverlappedResult(handle.0, &request.overlapped, &mut count, 1) };
                if result != 0 {
                    break;
                }
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    Some(code) if code == ERROR_IO_INCOMPLETE as i32 => continue,
                    Some(code) if code == ERROR_OPERATION_ABORTED as i32 || code == 1167 => break,
                    _ => {
                        errors.push(format!("Drain input: {error}"));
                        break;
                    }
                }
            }
            request.pending = false;
        }
        // No buffer or event is released until both overlapped requests have
        // reached terminal completion, including a cancel/completion race.
        let handle = self.handle.take().expect("retained input handle");
        if unsafe { CloseHandle(handle.0) } == 0 {
            errors.push(format!("Close input: {}", io::Error::last_os_error()));
        } else {
            std::mem::forget(handle);
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("Mapping hardware cleanup: {error:#}");
        }
    }
}
