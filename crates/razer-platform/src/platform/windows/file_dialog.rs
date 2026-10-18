//! The COM dialog and cancellation timer stay on one dedicated STA thread.
use super::{Arc, AtomicBool, Ordering, PathBuf, Result, SaveFileOptions};
use std::{
    cell::RefCell,
    os::windows::ffi::{OsStrExt, OsStringExt},
};
use windows::{
    Win32::{
        Foundation::HWND,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FOLDERID_Downloads, FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST,
            FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FileOpenDialog,
            FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
            KF_FLAG_DEFAULT, SHCreateItemFromParsingName, SHGetKnownFolderPath, SIGDN_FILESYSPATH,
        },
    },
    core::{HRESULT, Interface, PCWSTR, w},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};
const CANCELLED: HRESULT = HRESULT(0x800704c7u32 as i32);
thread_local! {
    static ACTIVE: RefCell<Option<(IFileDialog, Arc<AtomicBool>)>> = const { RefCell::new(None) };
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
    ACTIVE.with(|active| {
        *active.borrow_mut() = Some((
            dialog.cast().expect("IFileOpenDialog derives IFileDialog"),
            cancelled.clone(),
        ))
    });
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

pub fn save(
    owner: isize,
    options: &SaveFileOptions,
    cancelled: Arc<AtomicBool>,
) -> Result<Option<PathBuf>> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    let _apartment = Apartment;
    let dialog: IFileSaveDialog =
        unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)? };
    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<_>>();
    let title = wide(&options.title);
    let name = wide(&options.file_name);
    let button = wide(&options.button_label);
    let filter_name = wide(&options.filter_name);
    let extension = wide(options.extension.trim_start_matches('.'));
    let filter = wide(&super::extension_filter(&[&options.extension])?);
    unsafe {
        dialog.SetTitle(PCWSTR(title.as_ptr()))?;
        dialog.SetFileName(PCWSTR(name.as_ptr()))?;
        dialog.SetOkButtonLabel(PCWSTR(button.as_ptr()))?;
        dialog.SetDefaultExtension(PCWSTR(extension.as_ptr()))?;
        dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
            pszName: PCWSTR(filter_name.as_ptr()),
            pszSpec: PCWSTR(filter.as_ptr()),
        }])?;
        dialog.SetOptions(
            dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT,
        )?;
    }
    let directory = if let Some(directory) = &options.directory {
        directory.clone()
    } else {
        let path = unsafe { SHGetKnownFolderPath(&FOLDERID_Downloads, KF_FLAG_DEFAULT, None)? };
        let value = unsafe { std::ffi::OsString::from_wide(path.as_wide()) };
        unsafe { CoTaskMemFree(Some(path.0.cast())) };
        PathBuf::from(value)
    };
    let directory: Vec<u16> = directory.as_os_str().encode_wide().chain([0]).collect();
    let folder: IShellItem =
        unsafe { SHCreateItemFromParsingName(PCWSTR(directory.as_ptr()), None)? };
    unsafe { dialog.SetFolder(&folder)? };
    if cancelled.load(Ordering::Acquire) {
        return Ok(None);
    }
    ACTIVE.with(|active| {
        *active.borrow_mut() = Some((
            dialog.cast().expect("IFileSaveDialog derives IFileDialog"),
            cancelled.clone(),
        ))
    });
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
    let item = unsafe { dialog.GetResult()? };
    let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
    let value = unsafe { std::ffi::OsString::from_wide(path.as_wide()) };
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    Ok(Some(PathBuf::from(value)))
}
