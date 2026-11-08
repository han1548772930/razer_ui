//! Local installed THX application entry used by the current earbud Sound card.
use anyhow::{Result, ensure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const EXECUTABLE: &str = r"C:\Program Files\Razer\RzAppEngine\rzappengine.exe";
const INSTALLATION: &str = r"C:\Program Files\Razer\THX Spatial Audio Setup";
const ARGUMENT: &str = "--url-params=apps=spatial-audio";

#[cfg(windows)]
#[path = "earbuds_activation_windows.rs"]
mod elevation;

/// Call on a background worker; a false observation does not download or install.
pub fn installed() -> Result<bool> {
    #[cfg(windows)]
    {
        Ok(std::path::Path::new(EXECUTABLE).try_exists()?
            && std::path::Path::new(INSTALLATION).try_exists()?)
    }
    #[cfg(not(windows))]
    anyhow::bail!("Local THX launch is only available on Windows")
}

/// Cancellation releases the waiting owner without terminating the launched app.
/// This adapter owns the ordinary process route and its handles.
pub fn launch(active: Arc<AtomicBool>) -> Result<Option<i32>> {
    #[cfg(windows)]
    {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, time::Duration};
        ensure!(active.load(Ordering::Acquire), "THX launch was canceled");
        ensure!(installed()?, "Local THX application was not found");
        let file = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(EXECUTABLE)?;
        crate::runtime::native::user_app_files::signature::verify_launch(
            &file,
            std::path::Path::new(EXECUTABLE),
        )?;
        ensure!(active.load(Ordering::Acquire), "THX launch was canceled");
        if elevation::required(std::path::Path::new(EXECUTABLE))? {
            if let Ok(completion) = elevation::launch(EXECUTABLE, ARGUMENT) {
                ensure!(
                    completion.cleanup_error.is_none(),
                    "THX application returned {}, but event unregistration failed: {}",
                    completion.exit_code,
                    completion.cleanup_error.as_deref().unwrap_or("")
                );
                return Ok(Some(completion.exit_code));
            }
            ensure!(active.load(Ordering::Acquire), "THX launch was canceled");
            if let Ok(child) = windows::Process::elevated() {
                return wait_for_exit(
                    &active,
                    || child.poll(),
                    || std::thread::sleep(Duration::from_millis(50)),
                );
            }
        }
        ensure!(
            active.load(Ordering::Acquire),
            "THX launch was canceled before ordinary fallback"
        );
        let child = windows::Process::start()?;
        wait_for_exit(
            &active,
            || child.poll(),
            || std::thread::sleep(Duration::from_millis(50)),
        )
    }
    #[cfg(not(windows))]
    {
        let _ = active;
        anyhow::bail!("Local THX launch is only available on Windows")
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        mem::size_of,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{
            CreateProcessW, GetExitCodeProcess, PROCESS_INFORMATION, STARTF_USESHOWWINDOW,
            STARTUPINFOW, WaitForSingleObject,
        },
    };

    pub(super) struct Process(OwnedHandle);
    impl Process {
        pub(super) fn elevated() -> Result<Self> {
            use windows_sys::Win32::UI::Shell::{
                SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
            };
            let path: Vec<u16> = EXECUTABLE.encode_utf16().chain(Some(0)).collect();
            let args: Vec<u16> = ARGUMENT.encode_utf16().chain(Some(0)).collect();
            let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
            let mut info = SHELLEXECUTEINFOW {
                cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS,
                lpVerb: verb.as_ptr(),
                lpFile: path.as_ptr(),
                lpParameters: args.as_ptr(),
                nShow: 1,
                ..Default::default()
            };
            // All UTF-16 buffers remain live through the synchronous system request.
            if unsafe { ShellExecuteExW(&mut info) } == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            ensure!(
                !info.hProcess.is_null(),
                "Elevated launch returned no process handle"
            );
            Ok(Self(unsafe { OwnedHandle::from_raw_handle(info.hProcess) }))
        }
        pub(super) fn start() -> Result<Self> {
            let mut command: Vec<u16> = format!("\"{}\" {}", EXECUTABLE, ARGUMENT)
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let startup = STARTUPINFOW {
                cb: size_of::<STARTUPINFOW>() as u32,
                dwFlags: STARTF_USESHOWWINDOW,
                wShowWindow: 1,
                ..Default::default()
            };
            let mut process = PROCESS_INFORMATION::default();
            // The source ordinary process branch uses no inherited handles,
            // custom environment or working directory. Windows owns output handles.
            let ok = unsafe {
                CreateProcessW(
                    ptr::null(),
                    command.as_mut_ptr(),
                    ptr::null(),
                    ptr::null(),
                    0,
                    0,
                    ptr::null(),
                    ptr::null(),
                    &startup,
                    &mut process,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            // Successful CreateProcess returns distinct owned process/thread handles.
            let thread = unsafe { OwnedHandle::from_raw_handle(process.hThread) };
            let process = unsafe { OwnedHandle::from_raw_handle(process.hProcess) };
            drop(thread);
            Ok(Self(process))
        }
        pub(super) fn poll(&self) -> Result<Option<i32>> {
            let handle = self.0.as_raw_handle();
            // The owned process handle outlives the non-blocking wait and exit query.
            match unsafe { WaitForSingleObject(handle, 0) } {
                WAIT_TIMEOUT => Ok(None),
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    if unsafe { GetExitCodeProcess(handle, &mut code) } == 0 {
                        return Err(std::io::Error::last_os_error().into());
                    }
                    Ok(Some(code as i32))
                }
                _ => Err(std::io::Error::last_os_error().into()),
            }
        }
    }
}

fn wait_for_exit(
    active: &AtomicBool,
    mut poll: impl FnMut() -> Result<Option<i32>>,
    mut wait: impl FnMut(),
) -> Result<Option<i32>> {
    while active.load(Ordering::Acquire) {
        if let Some(code) = poll()? {
            return Ok(Some(code));
        }
        wait();
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_wait_distinguishes_exit_cancel_and_failure() {
        let active = AtomicBool::new(true);
        let mut polls = 0;
        assert_eq!(
            wait_for_exit(
                &active,
                || {
                    polls += 1;
                    Ok(Some(7))
                },
                || panic!("no wait after exit")
            )
            .unwrap(),
            Some(7)
        );
        assert_eq!(polls, 1);
        assert!(
            wait_for_exit(
                &active,
                || anyhow::bail!("wait failed"),
                || panic!("no wait after failure")
            )
            .is_err()
        );
        assert_eq!(
            wait_for_exit(
                &active,
                || Ok(None),
                || active.store(false, Ordering::Release)
            )
            .unwrap(),
            None
        );
        assert_eq!(
            wait_for_exit(&active, || panic!("canceled before polling"), || {}).unwrap(),
            None
        );
    }
}
