use super::{CameraApp, package_app_id, vcam_executable};
use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows::{
    Win32::{
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize, IPersistFile, STGM_READ,
        },
        UI::Shell::{
            FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop,
            IShellLinkW, KF_FLAG_DEFAULT, SHGFP_TYPE_CURRENT, SHGetFolderPathW,
            SHGetKnownFolderPath, ShellLink,
        },
    },
    core::{Interface, PCWSTR},
};
use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::{
            CM_Get_DevNode_Status, DIGCF_PRESENT, SP_DEVINFO_DATA, SPDRP_DEVICEDESC,
            SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
            SetupDiGetDeviceRegistryPropertyW,
        },
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
            RegQueryValueExW,
        },
    },
    core::GUID,
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn open(parent: HKEY, path: &str) -> Result<Option<Key>> {
    let mut key = null_mut();
    let status = unsafe { RegOpenKeyExW(parent, wide(path).as_ptr(), 0, KEY_READ, &mut key) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    ensure!(
        status == ERROR_SUCCESS,
        "camera application registry open failed: {status}"
    );
    Ok(Some(Key(key)))
}
fn children(key: &Key) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut length = name.len() as u32;
        let status = unsafe {
            RegEnumKeyExW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut length,
                null(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            break;
        }
        ensure!(
            status == ERROR_SUCCESS,
            "camera application registry enumeration failed: {status}"
        );
        result.push(String::from_utf16(&name[..length as usize])?);
    }
    Ok(result)
}
fn value(key: &Key, name: &str) -> Result<Option<String>> {
    let name = wide(name);
    let mut kind = 0;
    let mut count = 0;
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            null(),
            &mut kind,
            null_mut(),
            &mut count,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    ensure!(
        status == ERROR_SUCCESS && count <= 65536 && count % 2 == 0,
        "camera application registry value is invalid ({status}, type {kind}, bytes {count})"
    );
    if kind != REG_SZ {
        return Ok(None);
    }
    let mut data = vec![0u16; count as usize / 2];
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            null(),
            &mut kind,
            data.as_mut_ptr().cast(),
            &mut count,
        )
    };
    ensure!(
        status == ERROR_SUCCESS
            && kind == REG_SZ
            && count as usize <= data.len() * 2
            && count % 2 == 0,
        "camera application registry value changed during read"
    );
    data.truncate(count as usize / 2);
    while data.last() == Some(&0) {
        data.pop();
    }
    let text = String::from_utf16(&data)?;
    ensure!(
        !text.contains('\0'),
        "camera application registry value contains an embedded NUL"
    );
    Ok(Some(text))
}
fn uninstall_keys() -> Result<Vec<Key>> {
    let mut result = Vec::new();
    for path in [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ] {
        if let Some(parent) = open(HKEY_LOCAL_MACHINE, path)? {
            for child in children(&parent)? {
                if let Some(key) = open(parent.0, &child)? {
                    result.push(key);
                }
            }
        }
    }
    Ok(result)
}
fn package() -> Result<Option<String>> {
    let Some(key) = open(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\WOW6432Node\Microsoft\SecurityManager\CapAuthz\ApplicationsEx",
    )?
    else {
        return Ok(None);
    };
    Ok(children(&key)?
        .into_iter()
        .find(|name| name.contains("ReincubateLtd.CamoStudio") && name.contains("x64")))
}
fn camo_driver_enabled() -> Result<bool> {
    // Actual ClassGuid bytes at RVA b8b28, not a guessed camera category.
    const CLASS: GUID = GUID::from_u128(0xca3e7ab9_b4c3_4ae6_8251_579ef933890f);
    let devices = unsafe { SetupDiGetClassDevsW(&CLASS, null(), null_mut(), DIGCF_PRESENT) };
    ensure!(
        devices != -1,
        "cannot enumerate the installed Camo driver: {}",
        std::io::Error::last_os_error()
    );
    struct Devices(isize);
    impl Drop for Devices {
        fn drop(&mut self) {
            unsafe {
                SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }
    let devices = Devices(devices);
    let mut enabled = false;
    for index in 0.. {
        let mut info = SP_DEVINFO_DATA {
            cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInfo(devices.0, index, &mut info) } == 0 {
            let error = std::io::Error::last_os_error();
            ensure!(
                error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32),
                "Camo driver enumeration failed: {error}"
            );
            break;
        }
        let mut description = [0u16; 256];
        let mut kind = 0;
        let mut needed = 0;
        if unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                devices.0,
                &mut info,
                SPDRP_DEVICEDESC,
                &mut kind,
                description.as_mut_ptr().cast(),
                512,
                &mut needed,
            )
        } == 0
        {
            continue;
        }
        let end = description
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(description.len());
        if String::from_utf16(&description[..end])? != "Camo" {
            continue;
        }
        let mut status = 0;
        let mut problem = 0;
        let result = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, info.DevInst, 0) };
        // The source defaults enabled when CM cannot report status.
        enabled = result != 0 || status & 0x400 == 0 || problem != 22;
    }
    Ok(enabled)
}
struct Apartment;
impl Apartment {
    fn open() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn shortcut(filename: &str) -> Result<Option<PathBuf>> {
    let _apartment = Apartment::open()?;
    let mut folders = Vec::new();
    for id in [
        FOLDERID_Programs,
        FOLDERID_CommonPrograms,
        FOLDERID_Desktop,
        FOLDERID_PublicDesktop,
    ] {
        if let Ok(pointer) = unsafe { SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, None) } {
            let path = unsafe { pointer.to_string() };
            unsafe {
                CoTaskMemFree(Some(pointer.0.cast()));
            }
            folders.push(PathBuf::from(path?));
        }
    }
    let mut fallback = [0u16; 260];
    if unsafe { SHGetFolderPathW(None, 25, None, SHGFP_TYPE_CURRENT.0 as u32, &mut fallback) }
        .is_ok()
    {
        let end = fallback
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(fallback.len());
        folders.push(PathBuf::from(OsString::from_wide(&fallback[..end])));
    }
    for folder in folders {
        let path = folder.join(filename);
        if !path.is_file() {
            continue;
        }
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };
        let persist: IPersistFile = link.cast()?;
        let name = wide(&path.to_string_lossy());
        unsafe {
            persist.Load(PCWSTR(name.as_ptr()), STGM_READ)?;
        }
        let mut target = [0u16; 260];
        unsafe {
            link.GetPath(&mut target, null_mut(), 0)?;
        }
        let end = target.iter().position(|c| *c == 0).unwrap_or(target.len());
        if end != 0 {
            return Ok(Some(PathBuf::from(OsString::from_wide(&target[..end]))));
        }
    }
    Ok(None)
}
fn launch_path(path: PathBuf) -> Result<()> {
    ensure!(
        path.is_file(),
        "camera application executable is absent: {}",
        path.display()
    );
    std::process::Command::new(&path)
        .spawn()
        .with_context(|| format!("launch camera application {}", path.display()))?;
    Ok(())
}
pub(super) fn launch(app: CameraApp) -> Result<()> {
    let keys = uninstall_keys()?;
    let names = keys
        .iter()
        .map(|k| {
            value(k, "DisplayName").unwrap_or_else(|error| {
                eprintln!("camera application registry entry: {error:#}");
                None
            })
        })
        .collect::<Vec<_>>();
    let camo_package = if app == CameraApp::Camo { package()? } else { None };
    let installed = match app {
        CameraApp::Camo => {
            (camo_package.is_some() || names.iter().flatten().any(|s| s == "Camo Studio"))
                && camo_driver_enabled()?
        }
        CameraApp::NvidiaBroadcast => names
            .iter()
            .flatten()
            .any(|s| s.contains("Camera (NVIDIA Broadcast)")),
        CameraApp::XSplitVCam => names.iter().flatten().any(|s| s.contains("VCam")),
    };
    ensure!(
        installed,
        "the selected camera application is not installed or its camera driver is disabled"
    );
    if let Some(package) = camo_package.and_then(|name| package_app_id(&name)) {
        // No shell parsing: the AppsFolder activation string is one argument.
        let explorer = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .context("Windows directory is unavailable")?
            .join("explorer.exe");
        if explorer.is_file()
            && std::process::Command::new(explorer)
                .arg(format!(r"shell:AppsFolder\{package}"))
                .spawn()
                .is_ok()
        {
            return Ok(());
        }
    }
    let filename = match app {
        CameraApp::Camo => "Camo Studio.lnk",
        CameraApp::NvidiaBroadcast => "NVIDIA Broadcast.lnk",
        CameraApp::XSplitVCam => "VCam.lnk",
    };
    match shortcut(filename).and_then(|path| path.map(launch_path).transpose()) {
        Ok(Some(())) => return Ok(()),
        Ok(None) => {}
        Err(error) => eprintln!("camera application shortcut launch: {error:#}"),
    }
    for (key, name) in keys.iter().zip(names) {
        let Some(name) = name else {
            continue;
        };
        let path = match app {
            CameraApp::NvidiaBroadcast if name.contains("NVIDIA Broadcast") => {
                value(key, "ProductHostExeCmd")?
            }
            CameraApp::XSplitVCam if name.contains("VCam") => {
                value(key, "UninstallString")?.and_then(|s| vcam_executable(&s))
            }
            _ => None,
        };
        if let Some(path) = path {
            return launch_path(PathBuf::from(path));
        }
    }
    anyhow::bail!("installed camera application has no usable local launch target")
}
