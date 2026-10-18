//! Current LaunchProcess helper (11ae38), using only system process APIs.
use anyhow::{Result, ensure};
use razer_device::macro_runtime::Launch;
use std::{mem::size_of, ptr};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError},
    Storage::FileSystem::{GetFileAttributesW, GetTempPathW, INVALID_FILE_ATTRIBUTES},
    System::Threading::{CreateProcessW, PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTUPINFOW},
    UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
};

fn wide(text: &str) -> Result<Vec<u16>> {
    ensure!(!text.contains('\0'), "Launch path contains NUL");
    Ok(text.encode_utf16().chain(Some(0)).collect())
}

pub(super) fn execute(launch: &Launch) -> Result<()> {
    if !launch.path_to_check.is_empty() {
        let path = wide(&launch.path_to_check)?;
        ensure!(
            unsafe { GetFileAttributesW(path.as_ptr()) } != INVALID_FILE_ATTRIBUTES,
            "file not found"
        );
    }
    let mut command = wide(&launch.path)?;
    let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESHOWWINDOW;
    startup.wShowWindow = u16::from(!launch.hidden);
    let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // 11ae38 explicitly calls GetTempDir for LaunchOptions.current_directory.
    // Failed lookup leaves the option empty; it does not choose the source repo.
    let mut temporary = [0u16; 260];
    let length = unsafe { GetTempPathW(temporary.len() as u32, temporary.as_mut_ptr()) };
    let directory = if (1..260).contains(&length) {
        temporary.as_ptr()
    } else {
        ptr::null()
    };
    let started = unsafe {
        CreateProcessW(
            ptr::null(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            0,
            ptr::null(),
            directory,
            &startup,
            &mut process,
        )
    };
    if started != 0 {
        unsafe {
            CloseHandle(process.hThread);
            CloseHandle(process.hProcess);
        }
        return Ok(());
    }
    let process_error = unsafe { GetLastError() };
    let path = wide(&launch.path)?;
    let verb = wide("open")?;
    let mut shell: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    shell.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    shell.fMask = SEE_MASK_NOCLOSEPROCESS;
    shell.lpVerb = verb.as_ptr();
    shell.lpFile = path.as_ptr();
    shell.nShow = i32::from(!launch.hidden);
    let started = unsafe { ShellExecuteExW(&mut shell) };
    let shell_error = unsafe { GetLastError() };
    if !shell.hProcess.is_null() {
        unsafe {
            CloseHandle(shell.hProcess);
        }
    }
    ensure!(
        started != 0,
        "fail to launch (CreateProcessW={process_error}, ShellExecuteExW={shell_error})"
    );
    Ok(())
}
