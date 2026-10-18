//! Direct Marie USB audio IOCTL owner. SetupDi identity is checked before and
//! after each operation; pending I/O is drained before its buffers are freed.
use super::super::hid;
use anyhow::{Context as _, Result, ensure};
use razer_device::seiren_driver::{self as protocol, Product, Transport};
use std::{
    cell::Cell,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_IO_PENDING, GetLastError, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Storage::FileSystem::{CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
        System::{
            IO::{CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED},
            Threading::{CreateEventW, WaitForSingleObject},
        },
    },
    core::GUID,
};

pub(super) struct Driver {
    handle: OwnedHandle,
    identity: hid::DriverInterface,
    product: Product,
    deadline: Cell<Option<Instant>>,
}
impl Driver {
    pub(super) fn set_deadline(&self, deadline: Option<Instant>) { self.deadline.set(deadline); }
    pub(super) fn control(&self, request: &protocol::AudioControlWrite) -> Result<()> {
        // METHOD_IN_DIRECT maps this second buffer as input to the driver.
        // Keep a mutable owned copy alive through overlapped completion.
        let mut payload = request.payload.clone();
        self.transfer(protocol::SET_AUDIO_CONTROL, &request.header, &mut payload)?;
        Ok(())
    }
    pub(super) fn open(product_id: u32, container: &str) -> Result<Self> {
        let product = Product::from_id(product_id)?;
        let identity = hid::select_driver_interface(container, &GUID::from_u128(product.interface_guid()))?;
        // Current API 10b20 -> 5020 -> 5160 opens the selected interface with
        // GENERIC_READ | GENERIC_WRITE, shared read/write and overlapped I/O.
        let raw = unsafe {
            CreateFileW(identity.path.as_ptr(), 0xc000_0000,
                FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED, null_mut())
        };
        ensure!(raw != INVALID_HANDLE_VALUE && !raw.is_null(),
            "Open Marie audio driver: {}", std::io::Error::last_os_error());
        // SAFETY: successful CreateFileW transfers this handle to this owner.
        let driver = Self { handle: unsafe { OwnedHandle::from_raw_handle(raw) }, identity, product, deadline: Cell::new(None) };
        protocol::verify_driver(&driver, product)?;
        Ok(driver)
    }

    fn current(&self) -> Result<()> {
        let observed = hid::select_driver_interface(&self.identity.container, &GUID::from_u128(self.product.interface_guid()))?;
        ensure!(observed == self.identity, "Marie audio driver identity changed");
        Ok(())
    }

    fn transfer(&self, code: u32, input: &[u8], output: &mut [u8]) -> Result<usize> {
        ensure!(self.deadline.get().is_none_or(|deadline| Instant::now() < deadline), "Marie operation deadline exceeded");
        self.current()?;
        let input_size = u32::try_from(input.len())?;
        let output_size = u32::try_from(output.len())?;
        let raw_event = unsafe { CreateEventW(null(), 1, 0, null()) };
        ensure!(!raw_event.is_null(), "Create Marie driver completion event: {}", std::io::Error::last_os_error());
        // SAFETY: the event is exclusively owned and kept until I/O completes.
        let event = unsafe { OwnedHandle::from_raw_handle(raw_event) };
        let mut overlapped = OVERLAPPED { hEvent: event.as_raw_handle(), ..Default::default() };
        let mut returned = 0;
        // SAFETY: buffers and OVERLAPPED remain at stable addresses until the
        // operation completes, including cancellation/drain on every exit.
        let sent = unsafe {
            DeviceIoControl(self.handle.as_raw_handle(), code,
                if input.is_empty() { null() } else { input.as_ptr().cast() }, input_size,
                if output.is_empty() { null_mut() } else { output.as_mut_ptr().cast() }, output_size,
                &mut returned, &mut overlapped)
        };
        if sent == 0 {
            let error = unsafe { GetLastError() };
            ensure!(error == ERROR_IO_PENDING, "Marie audio IOCTL {code:#x}: {}", std::io::Error::from_raw_os_error(error as i32));
            let deadline = self.deadline.get().unwrap_or(Instant::now() + Duration::from_secs(20))
                .min(Instant::now() + Duration::from_secs(20));
            loop {
                let wait = unsafe { WaitForSingleObject(event.as_raw_handle(), 100) };
                if wait == WAIT_OBJECT_0 { break; }
                let failure = if wait != WAIT_TIMEOUT {
                    Some(format!("Marie audio completion wait failed: {}", std::io::Error::last_os_error()))
                } else if Instant::now() >= deadline {
                    Some("Marie audio IOCTL exceeded deadline".into())
                } else {
                    self.current().err().map(|error| format!("{error:#}"))
                };
                if let Some(failure) = failure {
                    // Cancel only this request. Regardless of cancellation's
                    // result, wait for the kernel to release all borrowed data.
                    unsafe {
                        CancelIoEx(self.handle.as_raw_handle(), &overlapped);
                        GetOverlappedResult(self.handle.as_raw_handle(), &overlapped, &mut returned, 1);
                    }
                    anyhow::bail!("{failure}");
                }
            }
            ensure!(unsafe { GetOverlappedResult(self.handle.as_raw_handle(), &overlapped, &mut returned, 0) } != 0,
                "Marie audio IOCTL completion: {}", std::io::Error::last_os_error());
        }
        ensure!(returned <= output_size, "Marie audio driver returned an oversized response");
        self.current().context("Recheck Marie driver after I/O")?;
        Ok(returned as usize)
    }
}
impl Transport for Driver {
    fn exchange(&self, code: u32, input: &[u8], capacity: usize) -> Result<Vec<u8>> {
        ensure!(capacity <= 1024 * 1024, "Marie audio output capacity exceeds limit");
        let mut output = vec![0; capacity];
        let returned = self.transfer(code, input, &mut output)?;
        output.truncate(returned);
        Ok(output)
    }
}
