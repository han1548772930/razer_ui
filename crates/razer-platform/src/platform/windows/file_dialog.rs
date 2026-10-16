//! The COM dialog and cancellation timer stay on one dedicated STA thread.
use super::{Arc, AtomicBool, Ordering, PathBuf, Result};
use std::{cell::RefCell, os::windows::ffi::OsStringExt};
use windows::{
    Win32::{
        Foundation::HWND,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST,
            FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FileOpenDialog, IFileOpenDialog,
            SIGDN_FILESYSPATH,
        },
    },
    core::{HRESULT, PCWSTR, w},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};
const CANCELLED: HRESULT = HRESULT(0x800704c7u32 as i32);
thread_local! {
    static ACTIVE: RefCell<Option<(IFileOpenDialog, Arc<AtomicBool>)>> = const { RefCell::new(None) };
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
struct CancellationTimer(usize);
impl Drop for CancellationTimer {
    fn drop(&mut self) {
        unsafe {
            KillTimer(std::ptr::null_mut(), self.0);
        }
        ACTIVE.with(|active| {
            active.borrow_mut().take();
        });
    }
}
unsafe extern "system" fn poll_cancel(
    _: windows_sys::Win32::Foundation::HWND,
    _: u32,
    _: usize,
    _: u32,
) {
    // Release the TLS borrow before Close, which may synchronously dispatch messages.
    let dialog = ACTIVE.with(|active| {
        active
            .borrow()
            .as_ref()
            .and_then(|(dialog, cancel)| cancel.load(Ordering::Acquire).then(|| dialog.clone()))
    });
    if let Some(dialog) = dialog {
        let _ = unsafe { dialog.Close(CANCELLED) };
    }
}
pub fn show(
    owner: isize,
    title: &str,
    filter: &str,
    multiple: bool,
    cancelled: Arc<AtomicBool>,
) -> Result<Option<Vec<PathBuf>>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    let _apartment = Apartment;
    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };
    let title: Vec<u16> = title.encode_utf16().chain([0]).collect();
    let filter: Vec<u16> = filter.encode_utf16().chain([0]).collect();
    unsafe {
        dialog.SetTitle(PCWSTR(title.as_ptr()))?;
        // Current host uses the literal label All Files with the supplied extensions.
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: w!("All Files"),
            pszSpec: PCWSTR(filter.as_ptr()),
        }])?;
        let mut options =
            dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST;
        if multiple {
            options |= FOS_ALLOWMULTISELECT;
        }
        dialog.SetOptions(options)?;
    }
    if cancelled.load(Ordering::Acquire) {
        return Ok(None);
    }
    ACTIVE.with(|active| *active.borrow_mut() = Some((dialog.clone(), cancelled.clone())));
    let timer = unsafe { SetTimer(std::ptr::null_mut(), 0, 50, Some(poll_cancel)) };
    if timer == 0 {
        ACTIVE.with(|active| {
            active.borrow_mut().take();
        });
        return Err(std::io::Error::last_os_error().into());
    }
    let _timer = CancellationTimer(timer);
    match unsafe { dialog.Show(Some(HWND(owner as *mut _))) } {
        Err(error) if error.code() == CANCELLED => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(()) => {}
    }
    if cancelled.load(Ordering::Acquire) {
        return Ok(None);
    }
    let items = unsafe { dialog.GetResults()? };
    let mut paths = Vec::new();
    for index in 0..unsafe { items.GetCount()? } {
        let item = unsafe { items.GetItemAt(index)? };
        let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
        let value = unsafe { std::ffi::OsString::from_wide(path.as_wide()) };
        unsafe {
            CoTaskMemFree(Some(path.0.cast()));
        }
        paths.push(PathBuf::from(value));
    }
    Ok(Some(paths))
}
