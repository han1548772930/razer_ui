//! Direct mapping output IOCTL adapter. No vendor DLL is loaded or invoked.
//!
//! Paths must come from the caller's actual filter-driver discovery. This
//! module does not invent a path or treat a HID collection as that driver.
use razer_device::mapping_native_output::{
    CompletionPoll, DriverInputReport, DriverOperation, DriverOutputError, MappingOutputTransport,
    SubmissionId,
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

fn last_error(operation: DriverOperation) -> DriverOutputError {
    DriverOutputError::Win32 {
        operation,
        code: unsafe { GetLastError() },
    }
}

struct CompletionEvent(HANDLE);

impl Drop for CompletionEvent {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

struct PendingOutput {
    // Both allocations remain at a fixed address from submit through final
    // completion, including cancellation. Moving the map entry is harmless.
    bytes: Box<[u8; 32]>,
    overlapped: Box<OVERLAPPED>,
    event: CompletionEvent,
}

pub struct WindowsMappingOutputTransport {
    handle: HANDLE,
    next_id: u64,
    pending: BTreeMap<SubmissionId, PendingOutput>,
}

impl WindowsMappingOutputTransport {
    /// Same access/share/open/overlapped flags as RVA 0x1101D0. An explicit
    /// source driver path is mandatory; there is no first-HID fallback.
    pub fn open_discovered_driver_path(path: &str) -> Result<Self, DriverOutputError> {
        if path.is_empty() || !path.is_ascii() || path.contains('\0') {
            return Err(DriverOutputError::InvalidPath);
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

    fn open_handle(&self) -> Result<HANDLE, DriverOutputError> {
        if self.handle.is_null() || self.handle == INVALID_HANDLE_VALUE {
            Err(DriverOutputError::Closed)
        } else {
            Ok(self.handle)
        }
    }
}

impl MappingOutputTransport for WindowsMappingOutputTransport {
    fn submit(&mut self, report: DriverInputReport) -> Result<SubmissionId, DriverOutputError> {
        let handle = self.open_handle()?;
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(DriverOutputError::SubmissionIdExhausted)?;
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event.is_null() {
            return Err(last_error(DriverOperation::CompletionEvent));
        }
        let mut request = PendingOutput {
            bytes: Box::new(*report.bytes()),
            overlapped: Box::new(OVERLAPPED {
                hEvent: event,
                ..Default::default()
            }),
            event: CompletionEvent(event),
        };
        let accepted = unsafe {
            DeviceIoControl(
                handle,
                report.ioctl(),
                request.bytes.as_ptr().cast(),
                request.bytes.len() as u32,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                request.overlapped.as_mut(),
            )
        };
        if accepted == 0 {
            let code = unsafe { GetLastError() };
            if code != ERROR_IO_PENDING {
                return Err(DriverOutputError::Win32 {
                    operation: DriverOperation::Submit,
                    code,
                });
            }
        }
        let id = SubmissionId(self.next_id);
        self.next_id = next_id;
        self.pending.insert(id, request);
        // ERROR_IO_PENDING and synchronous acceptance both yield an ID. Only
        // GetOverlappedResult below produces a final driver completion result.
        Ok(id)
    }

    fn poll_completion(
        &mut self,
        id: SubmissionId,
        wait: Duration,
    ) -> Result<CompletionPoll, DriverOutputError> {
        let handle = self.open_handle()?;
        let request = self
            .pending
            .get_mut(&id)
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        let milliseconds = wait.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        let wait_result = unsafe { WaitForSingleObject(request.event.0, milliseconds) };
        if wait_result == WAIT_TIMEOUT {
            return Ok(CompletionPoll::Pending);
        }
        if wait_result == WAIT_FAILED {
            return Err(last_error(DriverOperation::Complete));
        }
        if wait_result != WAIT_OBJECT_0 {
            return Err(DriverOutputError::Win32 {
                operation: DriverOperation::Complete,
                code: wait_result,
            });
        }
        let mut returned = 0;
        let succeeded =
            unsafe { GetOverlappedResult(handle, request.overlapped.as_mut(), &mut returned, 0) };
        let result = if succeeded != 0 {
            Ok(())
        } else {
            let code = unsafe { GetLastError() };
            if code == ERROR_IO_INCOMPLETE {
                return Ok(CompletionPoll::Pending);
            }
            Err(DriverOutputError::Win32 {
                operation: DriverOperation::Complete,
                code,
            })
        };
        self.pending.remove(&id);
        Ok(CompletionPoll::Completed(result))
    }

    fn cancel(&mut self, id: SubmissionId) -> Result<(), DriverOutputError> {
        let handle = self.open_handle()?;
        let request = self
            .pending
            .get(&id)
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        if unsafe { CancelIoEx(handle, request.overlapped.as_ref()) } == 0 {
            let code = unsafe { GetLastError() };
            // Completion may have won the race. Still retain/drain the request.
            if code != ERROR_NOT_FOUND {
                return Err(DriverOutputError::Win32 {
                    operation: DriverOperation::Cancel,
                    code,
                });
            }
        }
        Ok(())
    }

    fn close(&mut self) -> Result<(), DriverOutputError> {
        if !self.pending.is_empty() {
            return Err(DriverOutputError::ExecutionPending);
        }
        let handle = self.open_handle()?;
        if unsafe { CloseHandle(handle) } == 0 {
            return Err(last_error(DriverOperation::Close));
        }
        self.handle = ptr::null_mut();
        Ok(())
    }
}

impl Drop for WindowsMappingOutputTransport {
    fn drop(&mut self) {
        let Ok(handle) = self.open_handle() else {
            return;
        };
        if !self.pending.is_empty() {
            unsafe { CancelIoEx(handle, ptr::null()) };
            // Drop is the fallback if the caller skips ordered disconnect. It
            // must retain both kernel buffers until cancellation/completion.
            // Normal callers poll with a deadline before reaching this path.
            for request in self.pending.values_mut() {
                let mut returned = 0;
                loop {
                    if unsafe {
                        GetOverlappedResult(handle, request.overlapped.as_mut(), &mut returned, 1)
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
