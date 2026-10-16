//! Direct filter-driver rotation IOCTLs; no vendor DLL or assumed HID path.
use razer_device::{
    mapping_native_output::{DriverOperation, DriverOutputError, SubmissionId},
    mapping_native_rotation::{
        RotationError, RotationPoll, RotationReply, RotationRequest, RotationTransport, win32_error,
    },
};
use std::{collections::BTreeMap, ffi::CString, ptr, time::Duration};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, ERROR_NOT_FOUND, GetLastError, HANDLE,
        INVALID_HANDLE_VALUE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{
        CreateFileA, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{
        IO::{CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED},
        Threading::{CreateEventW, WaitForSingleObject},
    },
};

fn last_error(operation: DriverOperation) -> RotationError {
    win32_error(operation, unsafe { GetLastError() })
}

struct CompletionEvent(HANDLE);

impl Drop for CompletionEvent {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

struct PendingRotation {
    request: RotationRequest,
    input: Option<Box<[u8; 4]>>,
    output: Box<[u8; 4]>,
    overlapped: Box<OVERLAPPED>,
    event: CompletionEvent,
}

pub struct WindowsRotationTransport {
    handle: HANDLE,
    next_id: u64,
    pending: BTreeMap<SubmissionId, PendingRotation>,
}

impl WindowsRotationTransport {
    /// Native RVA 0x1101D0 uses access 0, share read/write, OPEN_EXISTING,
    /// FILE_FLAG_OVERLAPPED. The caller supplies the discovered driver path.
    pub fn open_discovered_driver_path(path: &str) -> Result<Self, RotationError> {
        if path.is_empty() || !path.is_ascii() || path.contains('\0') {
            return Err(DriverOutputError::InvalidPath.into());
        }
        let path = CString::new(path).map_err(|_| DriverOutputError::InvalidPath)?;
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
            return Err(last_error(DriverOperation::Open));
        }
        Ok(Self {
            handle,
            next_id: 1,
            pending: BTreeMap::new(),
        })
    }

    fn open_handle(&self) -> Result<HANDLE, RotationError> {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            Err(DriverOutputError::Closed.into())
        } else {
            Ok(self.handle)
        }
    }
}

impl RotationTransport for WindowsRotationTransport {
    fn submit(&mut self, request: RotationRequest) -> Result<SubmissionId, RotationError> {
        let handle = self.open_handle()?;
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(DriverOutputError::SubmissionIdExhausted)?;
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event.is_null() {
            return Err(last_error(DriverOperation::CompletionEvent));
        }
        let mut pending = PendingRotation {
            request,
            input: request.input().map(Box::new),
            output: Box::new([0; 4]),
            overlapped: Box::new(OVERLAPPED {
                hEvent: event,
                ..Default::default()
            }),
            event: CompletionEvent(event),
        };
        let input = pending
            .input
            .as_ref()
            .map_or(ptr::null(), |bytes| bytes.as_ptr().cast());
        let output = if request.output_size() == 0 {
            ptr::null_mut()
        } else {
            pending.output.as_mut_ptr().cast()
        };
        // Native GET uses a stack DWORD and null OVERLAPPED. Rust explicitly
        // retains an overlapped output for memory safety on this overlapped
        // handle. IOCTL, input/output size and DWORD encoding are unchanged.
        let accepted = unsafe {
            DeviceIoControl(
                handle,
                request.ioctl(),
                input,
                if pending.input.is_some() { 4 } else { 0 },
                output,
                request.output_size(),
                ptr::null_mut(),
                pending.overlapped.as_mut(),
            )
        };
        if accepted == 0 {
            let code = unsafe { GetLastError() };
            if code != ERROR_IO_PENDING {
                return Err(win32_error(DriverOperation::Submit, code));
            }
        }
        let id = SubmissionId(self.next_id);
        self.next_id = next_id;
        self.pending.insert(id, pending);
        Ok(id)
    }

    fn poll(&mut self, id: SubmissionId, wait: Duration) -> Result<RotationPoll, RotationError> {
        let handle = self.open_handle()?;
        let request = self
            .pending
            .get_mut(&id)
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        let milliseconds = wait.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        match unsafe { WaitForSingleObject(request.event.0, milliseconds) } {
            WAIT_TIMEOUT => return Ok(RotationPoll::Pending),
            WAIT_FAILED => return Err(last_error(DriverOperation::Complete)),
            WAIT_OBJECT_0 => {}
            result => return Err(win32_error(DriverOperation::Complete, result)),
        }
        let mut returned = 0;
        let succeeded =
            unsafe { GetOverlappedResult(handle, request.overlapped.as_mut(), &mut returned, 0) };
        let result = if succeeded == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_IO_INCOMPLETE {
                return Ok(RotationPoll::Pending);
            }
            Err(win32_error(DriverOperation::Complete, code))
        } else {
            match request.request {
                RotationRequest::SetAngle(_) => Ok(RotationReply::SetComplete),
                RotationRequest::GetAngle if returned != 4 => {
                    Err(RotationError::ReadLength(returned as usize))
                }
                RotationRequest::GetAngle => {
                    RotationReply::from_read_bytes(request.output.as_slice())
                }
            }
        };
        self.pending.remove(&id);
        Ok(RotationPoll::Completed(result))
    }

    fn cancel(&mut self, id: SubmissionId) -> Result<(), RotationError> {
        let handle = self.open_handle()?;
        let pending = self
            .pending
            .get(&id)
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        if unsafe { CancelIoEx(handle, pending.overlapped.as_ref()) } == 0 {
            let code = unsafe { GetLastError() };
            if code != ERROR_NOT_FOUND {
                return Err(win32_error(DriverOperation::Cancel, code));
            }
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), RotationError> {
        if !self.pending.is_empty() {
            return Err(DriverOutputError::ExecutionPending.into());
        }
        let handle = self.open_handle()?;
        if unsafe { CloseHandle(handle) } == 0 {
            return Err(last_error(DriverOperation::Close));
        }
        self.handle = ptr::null_mut();
        Ok(())
    }
}

impl Drop for WindowsRotationTransport {
    fn drop(&mut self) {
        let Ok(handle) = self.open_handle() else {
            return;
        };
        if !self.pending.is_empty() {
            unsafe { CancelIoEx(handle, ptr::null()) };
            for pending in self.pending.values_mut() {
                let mut returned = 0;
                loop {
                    if unsafe {
                        GetOverlappedResult(handle, pending.overlapped.as_mut(), &mut returned, 1)
                    } != 0
                        || unsafe { GetLastError() } != ERROR_IO_INCOMPLETE
                    {
                        break;
                    }
                }
            }
            self.pending.clear();
        }
        unsafe { CloseHandle(handle) };
        self.handle = ptr::null_mut();
    }
}
