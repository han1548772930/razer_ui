//! Cross-process camera locks. A source task lock spans its runner; the
//! separate native lock spans one standard call or one complete ISP exchange.
use anyhow::{Result, ensure};
use razer_device::camera_submission::CameraScope;
use std::{marker::PhantomData, rc::Rc};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

pub(crate) struct Guard {
    handle: HANDLE,
    // Win32 mutex ownership belongs to the acquiring thread.
    _thread: PhantomData<Rc<()>>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

pub(crate) fn native_name(scope: &CameraScope) -> String {
    format!(
        "Local\\RazerUi-CameraNative-{}",
        scope.container_id.to_ascii_lowercase()
    )
}

pub(crate) fn acquire(name: &str, validate: &dyn Fn() -> Result<()>) -> Result<Guard> {
    validate()?;
    let name: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    ensure!(
        !handle.is_null(),
        "camera lock creation failed: {}",
        std::io::Error::last_os_error()
    );
    let result = (|| {
        loop {
            validate()?;
            match unsafe { WaitForSingleObject(handle, 40) } {
                WAIT_OBJECT_0 | WAIT_ABANDONED => return Ok(()),
                WAIT_TIMEOUT => {}
                status => anyhow::bail!("camera lock wait failed: {status}"),
            }
        }
    })();
    if let Err(error) = result {
        unsafe {
            CloseHandle(handle);
        }
        return Err(error);
    }
    let guard = Guard {
        handle,
        _thread: PhantomData,
    };
    validate()?;
    Ok(guard)
}
