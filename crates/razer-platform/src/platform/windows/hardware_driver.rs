//! Read-only sensor transactions against the installed kernel driver.
use crate::hardware_driver::{DEVICE_PATH, Driver, MUTEX_NAME, Transport};
use anyhow::{Context as _, bail, ensure};
use std::{marker::PhantomData, ptr, rc::Rc};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, OPEN_EXISTING},
    System::{
        IO::DeviceIoControl,
        SystemInformation::GROUP_AFFINITY,
        Threading::{
            CreateMutexW, GetActiveProcessorCount, GetActiveProcessorGroupCount, GetCurrentThread,
            ReleaseMutex, SetThreadGroupAffinity, WaitForSingleObject,
        },
    },
};

#[path = "hardware_driver_installation.rs"]
mod installation;
pub use installation::{install_from_application_directory, uninstall_owned_installation};

/// Processor-group identity is preserved on machines with more than 64 threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Processor {
    pub group: u16,
    pub number: u8,
}

impl Processor {
    pub fn active() -> anyhow::Result<Vec<Self>> {
        // These calls only inspect the OS processor topology.
        let groups = unsafe { GetActiveProcessorGroupCount() };
        ensure!(
            groups != 0,
            "Cannot enumerate processor groups: {}",
            std::io::Error::last_os_error()
        );
        let mut processors = Vec::new();
        for group in 0..groups {
            let count = unsafe { GetActiveProcessorCount(group) };
            ensure!(
                count > 0 && count <= usize::BITS,
                "Invalid processor count {count} in group {group}"
            );
            processors.extend((0..count).map(|number| Self {
                group,
                number: number as u8,
            }));
        }
        Ok(processors)
    }
}

struct Handle(HANDLE);

impl Handle {
    fn close(&mut self) -> anyhow::Result<()> {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            // SAFETY: this owner uniquely holds an ordinary kernel handle.
            ensure!(
                unsafe { CloseHandle(self.0) } != 0,
                "Cannot close hardware driver handle: {}",
                std::io::Error::last_os_error()
            );
            self.0 = ptr::null_mut();
        }
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// This object coordinates measurements, not installation or service start.
/// Create and use it on the sensor worker. No handle is sent between threads.
pub struct Connection {
    mutex: Handle,
    thread: PhantomData<Rc<()>>,
}

impl Connection {
    pub fn new() -> anyhow::Result<Self> {
        let name: Vec<_> = MUTEX_NAME.encode_utf16().chain([0]).collect();
        // SAFETY: the name is terminated and retained for the synchronous call.
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
        ensure!(
            !handle.is_null(),
            "Cannot open hardware measurement mutex: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self {
            mutex: Handle(handle),
            thread: PhantomData,
        })
    }

    /// Match the current SDK's refresh lock (100 ms), open, measurement,
    /// close and release sequence. Missing/blocked drivers remain real errors.
    pub fn sample<R>(
        &mut self,
        processor: Processor,
        operation: impl FnOnce(&mut Driver<Session<'_>>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        ensure!(
            u32::from(processor.number) < usize::BITS,
            "Processor number exceeds the affinity mask"
        );
        let mut lease = self.acquire()?;
        let result = (|| {
            let name: Vec<_> = DEVICE_PATH.encode_utf16().chain([0]).collect();
            // Source 0x17dac opens GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING.
            let raw = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ,
                    FILE_SHARE_READ,
                    ptr::null(),
                    OPEN_EXISTING,
                    0,
                    ptr::null_mut(),
                )
            };
            ensure!(
                raw != INVALID_HANDLE_VALUE,
                "Cannot open installed hardware sensor driver: {}",
                std::io::Error::last_os_error()
            );
            let mut device = Handle(raw);
            let result = (|| {
                let mut affinity = Affinity::set(processor)?;
                let mut driver = Driver::new(Session { device: &device });
                let result = operation(&mut driver);
                finish(result, affinity.restore(), "restore sensor thread affinity")
            })();
            finish(result, device.close(), "close sensor driver")
        })();
        finish(result, lease.release(), "release sensor measurement mutex")
    }

    fn acquire(&self) -> anyhow::Result<Lease<'_>> {
        // SAFETY: this worker owns a live mutex handle for the entire lease.
        match unsafe { WaitForSingleObject(self.mutex.0, 100) } {
            WAIT_OBJECT_0 => Ok(Lease {
                mutex: &self.mutex,
                acquired: true,
            }),
            WAIT_ABANDONED => {
                // Windows grants ownership on abandonment. Release it even
                // though this sample cannot trust the preceding owner's state.
                let mut lease = Lease {
                    mutex: &self.mutex,
                    acquired: true,
                };
                finish(
                    Err(anyhow::anyhow!("Hardware measurement mutex was abandoned")),
                    lease.release(),
                    "release abandoned sensor mutex",
                )
            }
            WAIT_TIMEOUT => bail!("Hardware measurement mutex timed out"),
            _ => bail!(
                "Cannot acquire hardware measurement mutex: {}",
                std::io::Error::last_os_error()
            ),
        }
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        self.mutex.close()
    }
}

struct Lease<'a> {
    mutex: &'a Handle,
    acquired: bool,
}

impl Lease<'_> {
    fn release(&mut self) -> anyhow::Result<()> {
        if self.acquired {
            // SAFETY: the lease cannot move to another thread and it was
            // obtained by this worker's successful wait on this exact mutex.
            ensure!(
                unsafe { ReleaseMutex(self.mutex.0) } != 0,
                "Cannot release hardware measurement mutex: {}",
                std::io::Error::last_os_error()
            );
            self.acquired = false;
        }
        Ok(())
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

struct Affinity {
    previous: GROUP_AFFINITY,
    restored: bool,
}

impl Affinity {
    fn set(processor: Processor) -> anyhow::Result<Self> {
        let requested = GROUP_AFFINITY {
            Mask: 1_usize << processor.number,
            Group: processor.group,
            Reserved: [0; 3],
        };
        let mut previous = GROUP_AFFINITY::default();
        // SAFETY: valid stack buffers remain live throughout the synchronous
        // call; only this worker's affinity changes and its old value is saved.
        ensure!(
            unsafe { SetThreadGroupAffinity(GetCurrentThread(), &requested, &mut previous) } != 0,
            "Cannot select hardware sensor processor: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self {
            previous,
            restored: false,
        })
    }

    fn restore(&mut self) -> anyhow::Result<()> {
        if !self.restored {
            // SAFETY: this is the complete affinity returned by Windows on this
            // same thread; there are no asynchronous operations in the lease.
            ensure!(
                unsafe {
                    SetThreadGroupAffinity(GetCurrentThread(), &self.previous, ptr::null_mut())
                } != 0,
                "Cannot restore hardware sensor processor affinity: {}",
                std::io::Error::last_os_error()
            );
            self.restored = true;
        }
        Ok(())
    }
}

impl Drop for Affinity {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub struct Session<'a> {
    device: &'a Handle,
}

impl Transport for Session<'_> {
    fn ioctl(&mut self, code: u32, input: &[u8], output: &mut [u8]) -> anyhow::Result<usize> {
        let input_length = u32::try_from(input.len()).context("Hardware request is too large")?;
        let output_length = u32::try_from(output.len()).context("Hardware reply is too large")?;
        let mut returned = 0;
        // SAFETY: both byte slices remain valid during this synchronous call;
        // the borrowed handle cannot close until the enclosing sample finishes.
        let ok = unsafe {
            DeviceIoControl(
                self.device.0,
                code,
                if input.is_empty() {
                    ptr::null()
                } else {
                    input.as_ptr().cast()
                },
                input_length,
                output.as_mut_ptr().cast(),
                output_length,
                &mut returned,
                ptr::null_mut(),
            )
        };
        ensure!(
            ok != 0,
            "Hardware driver request {code:#010x} failed: {}",
            std::io::Error::last_os_error()
        );
        Ok(returned as usize)
    }
}

fn finish<T>(
    result: anyhow::Result<T>,
    cleanup: anyhow::Result<()>,
    operation: &str,
) -> anyhow::Result<T> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).with_context(|| operation.to_owned()),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Additionally failed to {operation}: {cleanup:#}")))
        }
    }
}
