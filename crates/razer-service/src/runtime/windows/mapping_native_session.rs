//! Windows adapter for the route-bound mapping-driver session.
//!
//! This module is a static Rust implementation of the recovered native
//! `CreateFileA`/overlapped `DeviceIoControl` chain.  It is intentionally not
//! called by tests and never loads the vendor DLL.

use razer_device::mapping_native_session::{
    FILTER_DRIVER_INTERFACE_CLASS, NativeCompletion, NativeDiscoveryFailure, NativeDiscoveryQuery,
    NativeDiscoveryReport, NativeDriverPath, NativeGuid, NativeOperation, NativeSessionTransport,
    NativeSubmissionId, NativeTransportError,
};
use std::{collections::BTreeMap, ffi::CString, mem::size_of, ptr, time::Duration};
use windows_sys::Win32::{
    Devices::{
        DeviceAndDriverInstallation::*,
        Properties::{DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID},
    },
    Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING,
        ERROR_NO_MORE_ITEMS, ERROR_NOT_FOUND, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
        WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{
        CreateFileA, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{
        IO::{CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED},
        Threading::{CreateEventW, WaitForSingleObject},
    },
};
use windows_sys::core::GUID;

fn windows_guid(guid: NativeGuid) -> GUID {
    GUID {
        data1: guid.data1,
        data2: guid.data2,
        data3: guid.data3,
        data4: guid.data4,
    }
}

/// Enumerate the recovered filter-driver interface class. This only obtains
/// SetupDi metadata; it opens no device handle and emits no IOCTL. Matching
/// container/PID collections are returned in the native enumeration order.
/// Capabilities and product kind remain caller-verified metadata.
pub fn discover_mapping_native_routes(
    query: NativeDiscoveryQuery,
) -> anyhow::Result<NativeDiscoveryReport> {
    query.validate()?;
    let class = windows_guid(FILTER_DRIVER_INTERFACE_CLASS);
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &class,
            ptr::null(),
            ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    anyhow::ensure!(
        raw != -1,
        "mapping filter-driver discovery failed: {}",
        std::io::Error::last_os_error()
    );
    let set = super::hid::DeviceSet(raw);
    let mut report = NativeDiscoveryReport::default();
    for index in 0..u32::MAX {
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInterfaces(set.0, ptr::null(), &class, index, &mut interface) }
            == 0
        {
            let code = unsafe { GetLastError() };
            if code == ERROR_NO_MORE_ITEMS {
                report.enumeration_complete = true;
            } else {
                report.failures.push(NativeDiscoveryFailure {
                    interface_index: index,
                    operation: "enumerate_interface",
                    win32_code: Some(code),
                });
            }
            break;
        }
        let mut required = 0;
        let success = unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                ptr::null_mut(),
                0,
                &mut required,
                ptr::null_mut(),
            )
        };
        let code = unsafe { GetLastError() };
        if success != 0 || code != ERROR_INSUFFICIENT_BUFFER {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "query_interface_size",
                win32_code: (success == 0).then_some(code),
            });
            continue;
        }
        if required < size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32
            || required > 1024 * 1024
        {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "invalid_interface_size",
                win32_code: None,
            });
            continue;
        }
        // u32 storage satisfies the structure's alignment. windows-sys
        // supplies cbSize=8 for x64 and cbSize=6 for x86.
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
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                set.0,
                &interface,
                detail,
                required,
                &mut required,
                &mut device,
            )
        } == 0
        {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "read_interface_path",
                win32_code: Some(unsafe { GetLastError() }),
            });
            continue;
        }
        let mut container = GUID::default();
        let mut property_type = 0;
        let mut property_bytes = 0;
        if unsafe {
            SetupDiGetDevicePropertyW(
                set.0,
                &device,
                &DEVPKEY_Device_ContainerId,
                &mut property_type,
                ptr::addr_of_mut!(container).cast(),
                size_of::<GUID>() as u32,
                &mut property_bytes,
                0,
            )
        } == 0
        {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "read_container_guid",
                win32_code: Some(unsafe { GetLastError() }),
            });
            continue;
        }
        if property_type != DEVPROP_TYPE_GUID || property_bytes as usize != size_of::<GUID>() {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "invalid_container_property",
                win32_code: None,
            });
            continue;
        }
        let container = NativeGuid::new(
            container.data1,
            container.data2,
            container.data3,
            container.data4,
        );
        if container != query.container {
            continue;
        }
        let pointer = unsafe { ptr::addr_of!((*detail).DevicePath).cast::<u16>() };
        let offset = pointer as usize - storage.as_ptr() as usize;
        if required as usize > capacity || (required as usize) < offset + size_of::<u16>() {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "invalid_path_length",
                win32_code: None,
            });
            continue;
        }
        let units = unsafe {
            std::slice::from_raw_parts(pointer, (required as usize - offset) / size_of::<u16>())
        };
        let Some(end) = units.iter().position(|unit| *unit == 0) else {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "unterminated_path",
                win32_code: None,
            });
            continue;
        };
        let Ok(path) = String::from_utf16(&units[..end]) else {
            report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "invalid_utf16_path",
                win32_code: None,
            });
            continue;
        };
        match query.route_for(FILTER_DRIVER_INTERFACE_CLASS, container, &path) {
            Ok(Some(route)) => report.routes.push(route),
            Ok(None) => {}
            Err(_) => report.failures.push(NativeDiscoveryFailure {
                interface_index: index,
                operation: "invalid_native_path",
                win32_code: None,
            }),
        }
    }
    Ok(report)
}

struct CompletionEvent(HANDLE);

impl Drop for CompletionEvent {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct PendingNative {
    input: Option<Box<[u8]>>,
    output: Box<[u8]>,
    overlapped: Box<OVERLAPPED>,
    event: CompletionEvent,
}

pub struct WindowsMappingNativeSessionTransport {
    handle: HANDLE,
    next_id: u64,
    pending: BTreeMap<NativeSubmissionId, PendingNative>,
    completed: BTreeMap<NativeSubmissionId, Vec<u8>>,
}

impl Default for WindowsMappingNativeSessionTransport {
    fn default() -> Self {
        Self {
            handle: ptr::null_mut(),
            next_id: 1,
            pending: BTreeMap::new(),
            completed: BTreeMap::new(),
        }
    }
}

impl WindowsMappingNativeSessionTransport {
    fn handle(&self) -> Result<HANDLE, NativeTransportError> {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            Err(NativeTransportError::Closed)
        } else {
            Ok(self.handle)
        }
    }

    fn error(kind: fn(u32) -> NativeTransportError) -> NativeTransportError {
        kind(unsafe { GetLastError() })
    }
}

impl NativeSessionTransport for WindowsMappingNativeSessionTransport {
    fn open(&mut self, path: &NativeDriverPath) -> Result<(), NativeTransportError> {
        if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            return Err(NativeTransportError::Closed);
        }
        let path = CString::new(path.as_str()).map_err(|_| NativeTransportError::InvalidPath)?;
        // RVA 0x1101D0: desired access 0, share read/write, OPEN_EXISTING,
        // FILE_FLAG_OVERLAPPED, and the caller's explicit driver path.
        let handle = unsafe {
            CreateFileA(
                path.as_ptr().cast(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(Self::error(|code| NativeTransportError::Open { code }));
        }
        self.handle = handle;
        Ok(())
    }

    fn submit(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError> {
        let handle = self.handle()?;
        let id = NativeSubmissionId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(NativeTransportError::Submit { code: u32::MAX })?;
        if let NativeOperation::Ioctl {
            ioctl,
            input,
            output_bytes,
        } = &operation
        {
            if *ioctl == razer_device::mapping_native_rotation::GET_ROTATION_IOCTL {
                if !input.is_empty() || *output_bytes != 4 {
                    return Err(NativeTransportError::InvalidBufferLength {
                        input_bytes: input.len(),
                        output_bytes: *output_bytes,
                    });
                }
                let mut angle = [0_u8; 4];
                let mut returned = 0;
                // 113CA0 performs this read synchronously on the retained
                // driver thread. No OVERLAPPED or second handle is created.
                // Both output pointers remain valid through the API call.
                let succeeded = unsafe {
                    DeviceIoControl(
                        handle,
                        *ioctl,
                        ptr::null(),
                        0,
                        angle.as_mut_ptr().cast(),
                        4,
                        &mut returned,
                        ptr::null_mut(),
                    )
                };
                if succeeded == 0 {
                    return Err(Self::error(|code| NativeTransportError::Submit { code }));
                }
                if returned != 4 {
                    return Err(NativeTransportError::Poll { code: 13 });
                }
                self.completed.insert(id, angle.to_vec());
                return Ok(id);
            }
        }
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event.is_null() {
            return Err(Self::error(|code| NativeTransportError::Submit { code }));
        }
        let event = CompletionEvent(event);
        let (ioctl, input, output_bytes) = match &operation {
            NativeOperation::ReadInput {
                ioctl,
                output_bytes,
            } => (*ioctl, None, *output_bytes),
            NativeOperation::Ioctl {
                ioctl,
                input,
                output_bytes,
            } => (
                *ioctl,
                Some(input.clone().into_boxed_slice()),
                *output_bytes,
            ),
        };
        let input_bytes = input.as_ref().map_or(0, |bytes| bytes.len());
        let input_len =
            u32::try_from(input_bytes).map_err(|_| NativeTransportError::InvalidBufferLength {
                input_bytes,
                output_bytes,
            })?;
        let output_len =
            u32::try_from(output_bytes).map_err(|_| NativeTransportError::InvalidBufferLength {
                input_bytes,
                output_bytes,
            })?;
        let mut request = PendingNative {
            input,
            output: vec![0; output_bytes].into_boxed_slice(),
            overlapped: Box::new(OVERLAPPED {
                hEvent: event.0,
                ..Default::default()
            }),
            event,
        };
        let input_ptr = request
            .input
            .as_ref()
            .map_or(ptr::null(), |bytes| bytes.as_ptr().cast());
        let output_ptr = if request.output.is_empty() {
            ptr::null_mut()
        } else {
            request.output.as_mut_ptr().cast()
        };
        let accepted = unsafe {
            DeviceIoControl(
                handle,
                ioctl,
                input_ptr,
                input_len,
                output_ptr,
                output_len,
                ptr::null_mut(),
                request.overlapped.as_mut(),
            )
        };
        if accepted == 0 {
            let code = unsafe { GetLastError() };
            if code != ERROR_IO_PENDING {
                return Err(NativeTransportError::Submit { code });
            }
        }
        self.pending.insert(id, request);
        Ok(id)
    }

    fn poll(
        &mut self,
        id: NativeSubmissionId,
        wait: Duration,
    ) -> Result<NativeCompletion, NativeTransportError> {
        let handle = self.handle()?;
        if let Some(output) = self.completed.remove(&id) {
            return Ok(NativeCompletion::Completed(Ok(output)));
        }
        let request = self
            .pending
            .get_mut(&id)
            .ok_or(NativeTransportError::UnknownSubmission(id))?;
        let milliseconds = wait.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        match unsafe { WaitForSingleObject(request.event.0, milliseconds) } {
            WAIT_TIMEOUT => return Ok(NativeCompletion::Pending),
            WAIT_FAILED => return Err(Self::error(|code| NativeTransportError::Poll { code })),
            WAIT_OBJECT_0 => {}
            code => return Err(NativeTransportError::Poll { code }),
        }
        let mut returned = 0;
        let succeeded =
            unsafe { GetOverlappedResult(handle, request.overlapped.as_mut(), &mut returned, 0) };
        if succeeded == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_IO_INCOMPLETE {
                return Ok(NativeCompletion::Pending);
            }
            self.pending.remove(&id);
            return Ok(NativeCompletion::Completed(Err(
                NativeTransportError::Poll { code },
            )));
        }
        // Return only bytes reported by the native completion. Retain no
        // uninitialized padding and never replace input with a fake event.
        let output_len = request.output.len();
        if returned as usize > output_len {
            self.pending.remove(&id);
            return Ok(NativeCompletion::Completed(Err(
                NativeTransportError::Poll { code: 13 },
            )));
        }
        let output = request.output[..returned as usize].to_vec();
        self.pending.remove(&id);
        Ok(NativeCompletion::Completed(Ok(output)))
    }

    fn cancel(&mut self, id: NativeSubmissionId) -> Result<(), NativeTransportError> {
        let handle = self.handle()?;
        if self.completed.contains_key(&id) {
            return Ok(());
        }
        let request = self
            .pending
            .get(&id)
            .ok_or(NativeTransportError::UnknownSubmission(id))?;
        if unsafe { CancelIoEx(handle, request.overlapped.as_ref()) } == 0 {
            let code = unsafe { GetLastError() };
            // Completion can win the cancellation race.  The request remains
            // retained and is drained by a subsequent poll.
            if code != ERROR_NOT_FOUND {
                return Err(NativeTransportError::Cancel { code });
            }
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), NativeTransportError> {
        if !self.pending.is_empty() || !self.completed.is_empty() {
            return Err(NativeTransportError::Close { code: 997 });
        }
        let handle = self.handle()?;
        if unsafe { CloseHandle(handle) } == 0 {
            return Err(Self::error(|code| NativeTransportError::Close { code }));
        }
        self.handle = ptr::null_mut();
        Ok(())
    }
}

impl Drop for WindowsMappingNativeSessionTransport {
    fn drop(&mut self) {
        let Ok(handle) = self.handle() else {
            return;
        };
        if !self.pending.is_empty() {
            unsafe {
                CancelIoEx(handle, ptr::null());
            }
            for request in self.pending.values_mut() {
                let mut returned = 0;
                loop {
                    let result = unsafe {
                        GetOverlappedResult(handle, request.overlapped.as_mut(), &mut returned, 1)
                    };
                    if result != 0 || unsafe { GetLastError() } != ERROR_IO_INCOMPLETE {
                        break;
                    }
                }
            }
            self.pending.clear();
        }
        unsafe {
            CloseHandle(handle);
        }
        self.handle = ptr::null_mut();
    }
}
