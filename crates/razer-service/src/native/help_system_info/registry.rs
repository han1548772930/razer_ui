use anyhow::{Context as _, ensure};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND},
    System::Registry::*,
};

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
pub(super) fn text(buffer: &[u16]) -> anyhow::Result<String> {
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16(&buffer[..end]).context("System string is not valid UTF-16")
}
pub(super) fn status(code: u32, operation: &str) -> anyhow::Result<()> {
    ensure!(
        code == 0,
        "{operation}: {}",
        std::io::Error::from_raw_os_error(code as i32)
    );
    Ok(())
}
pub(super) struct Key(pub(super) HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}
impl Key {
    pub(super) fn open(root: HKEY, path: &str) -> anyhow::Result<Option<Self>> {
        let path = wide(path);
        let mut handle = null_mut();
        let result = unsafe {
            RegOpenKeyExW(
                root,
                path.as_ptr(),
                0,
                KEY_READ | KEY_WOW64_64KEY,
                &mut handle,
            )
        };
        if matches!(result, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
            return Ok(None);
        }
        status(result, "Open system report registry key")?;
        Ok(Some(Self(handle)))
    }
    pub(super) fn bytes(&self, name: &str) -> anyhow::Result<Option<(u32, Vec<u8>)>> {
        let name = wide(name);
        let mut kind = 0;
        let mut length = 0;
        let result = unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                null(),
                &mut kind,
                null_mut(),
                &mut length,
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status(result, "Size system report registry value")?;
        ensure!(
            length <= 1024 * 1024,
            "System report registry value exceeds its size limit"
        );
        let mut bytes = vec![0; length as usize];
        status(
            unsafe {
                RegQueryValueExW(
                    self.0,
                    name.as_ptr(),
                    null(),
                    &mut kind,
                    bytes.as_mut_ptr(),
                    &mut length,
                )
            },
            "Read system report registry value",
        )?;
        ensure!(
            length as usize <= bytes.len(),
            "Registry value changed size during read"
        );
        bytes.truncate(length as usize);
        Ok(Some((kind, bytes)))
    }
    pub(super) fn string(&self, name: &str) -> anyhow::Result<Option<String>> {
        let Some((kind, bytes)) = self.bytes(name)? else {
            return Ok(None);
        };
        ensure!(
            matches!(kind, REG_SZ | REG_EXPAND_SZ | REG_MULTI_SZ),
            "Report registry string has type {kind}"
        );
        ensure!(
            bytes.len() % 2 == 0,
            "Report registry string has an odd byte length"
        );
        text(
            &bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>(),
        )
        .map(Some)
    }
    pub(super) fn dword(&self, name: &str) -> anyhow::Result<Option<u32>> {
        let Some((kind, bytes)) = self.bytes(name)? else {
            return Ok(None);
        };
        ensure!(
            kind == REG_DWORD && bytes.len() == 4,
            "Report registry DWORD has invalid type or length"
        );
        Ok(Some(u32::from_le_bytes(
            bytes.try_into().expect("checked length"),
        )))
    }
    pub(super) fn subkeys(&self) -> anyhow::Result<Vec<String>> {
        let mut names = Vec::new();
        for index in 0.. {
            let mut name = [0u16; 256];
            let mut length = name.len() as u32;
            let result = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    name.as_mut_ptr(),
                    &mut length,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if result == ERROR_NO_MORE_ITEMS {
                break;
            }
            status(result, "Enumerate system report registry key")?;
            names.push(text(&name[..length as usize])?);
        }
        Ok(names)
    }
}

pub(super) const CAP_AUTHZ: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\SecurityManager\CapAuthz\ApplicationsEx";
pub(super) const BIOS: &str = r"Hardware\Description\System\BIOS";
pub(super) const DRIVER_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class";

pub(super) fn software(root: HKEY, path: &str, value: &str) -> anyhow::Result<Option<String>> {
    match Key::open(root, path)? {
        Some(key) => key.string(value),
        None => Ok(None),
    }
}
