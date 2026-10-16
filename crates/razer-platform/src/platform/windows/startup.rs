//! Current SysUtils 0x6e2d0/0x6e950/0x6e3c0, using our installation namespace.
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW,
    },
};

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
// Never replace or remove the installed vendor's RazerAppEngine value.
const VALUE_NAME: &str = "razer_ui";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn check(result: u32) -> anyhow::Result<()> {
    if result != ERROR_SUCCESS {
        return Err(std::io::Error::from_raw_os_error(result as i32).into());
    }
    Ok(())
}

pub(crate) fn read_command() -> anyhow::Result<Option<String>> {
    let mut handle = std::ptr::null_mut();
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN_KEY).as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut handle,
        )
    };
    if matches!(result, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
        return Ok(None);
    }
    check(result)?;
    let key = Key(handle);
    let mut kind = 0;
    let mut bytes = 0;
    let name = wide(VALUE_NAME);
    let result = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    check(result)?;
    anyhow::ensure!(
        kind == REG_SZ && bytes % 2 == 0 && bytes <= 65536,
        "invalid startup Run value"
    );
    let mut value = vec![0u16; bytes as usize / 2 + 1];
    check(unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            value.as_mut_ptr().cast(),
            &mut bytes,
        )
    })?;
    anyhow::ensure!(
        kind == REG_SZ && bytes % 2 == 0,
        "startup Run value changed type while reading"
    );
    let value = &value[..bytes as usize / 2];
    let end = value
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(value.len());
    Ok(Some(String::from_utf16(&value[..end])?))
}

pub(crate) struct RunKey;
impl crate::startup::Registration for RunKey {
    fn set_command(&mut self, command: &str) -> anyhow::Result<()> {
        let mut handle = std::ptr::null_mut();
        check(unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut handle,
                std::ptr::null_mut(),
            )
        })?;
        let key = Key(handle);
        let value = wide(command);
        check(unsafe {
            RegSetValueExW(
                key.0,
                wide(VALUE_NAME).as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2).try_into()?,
            )
        })
    }

    fn remove_command(&mut self) -> anyhow::Result<()> {
        let mut handle = std::ptr::null_mut();
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut handle,
            )
        };
        if matches!(result, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
            return Ok(());
        }
        check(result)?;
        let key = Key(handle);
        let result = unsafe { RegDeleteValueW(key.0, wide(VALUE_NAME).as_ptr()) };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(result)
    }
}
