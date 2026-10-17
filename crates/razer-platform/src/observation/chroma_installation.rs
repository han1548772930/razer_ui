//! Current AppFolderInstallationChecker.checkIsFullyInstall and the installed
//! SysUtilsNative GetInstalledRazerAppEngineProduct registry projection.
#[derive(Clone, Debug)]
pub struct Installation {
    folder_exists: bool,
    engine_registered: Option<bool>,
}
impl Installation {
    pub fn folder_exists(&self) -> bool { self.folder_exists }
    pub fn engine_registered(&self) -> Option<bool> { self.engine_registered }
    pub fn fully_installed(&self) -> bool {
        self.folder_exists && self.engine_registered.unwrap_or(true)
    }
}

pub fn inspect() -> anyhow::Result<Installation> {
    let folder_exists = crate::native_paths::host_user_data_root()?.join("Apps").join("ChromaApp").try_exists()?;
    let engine_registered = engine_products()?.map(|products| products.iter().any(|product| product.replace(' ', "").to_lowercase().contains("chroma")));
    Ok(Installation { folder_exists, engine_registered })
}

#[cfg(windows)]
fn engine_products() -> anyhow::Result<Option<Vec<String>>> {
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, RegCloseKey, RegEnumValueW, RegOpenKeyExW},
    };
    struct Key(HKEY);
    impl Drop for Key { fn drop(&mut self) { unsafe { RegCloseKey(self.0); } } }
    let path = "SOFTWARE\\WOW6432Node\\Razer\\RazerAppEngine\\Dependencies\0".encode_utf16().collect::<Vec<_>>();
    let mut key = std::ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut key) };
    if matches!(status, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) { return Ok(None); }
    anyhow::ensure!(status == ERROR_SUCCESS, "Cannot inspect installed Chroma engine registration: Windows error {status}");
    let key = Key(key);
    let mut products = Vec::new();
    let mut index = 0;
    loop {
        let mut name = vec![0_u16; 256];
        loop {
            let mut length = name.len().try_into()?;
            let status = unsafe { RegEnumValueW(key.0, index, name.as_mut_ptr(), &mut length, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
            if status == ERROR_NO_MORE_ITEMS { return Ok(Some(products)); }
            if status == ERROR_MORE_DATA {
                anyhow::ensure!(name.len() < 32768, "Installed engine registration name exceeds the registry limit");
                name.resize((name.len() * 2).min(32768), 0);
                continue;
            }
            anyhow::ensure!(status == ERROR_SUCCESS, "Cannot enumerate installed Chroma engine registration: Windows error {status}");
            products.push(String::from_utf16(&name[..length as usize])?);
            break;
        }
        index += 1;
    }
}

#[cfg(not(windows))]
fn engine_products() -> anyhow::Result<Option<Vec<String>>> { Ok(None) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_folder_and_optional_native_result_are_both_respected() {
        for (folder_exists, engine_registered, expected) in [
            (false, None, false), (true, None, true),
            (true, Some(false), false), (true, Some(true), true),
            (false, Some(true), false),
        ] {
            assert_eq!(Installation { folder_exists, engine_registered }.fully_installed(), expected);
        }
    }
}
