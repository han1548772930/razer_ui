//! Explicit installation commands. Ordinary sensor discovery never calls these.
use super::{Connection, Processor, finish};
use crate::hardware_driver::{
    DRIVER_FILENAME, SERVICE_NAME, SYSTEM_DRIVER_DIRECTORY, packaged_driver,
};
use anyhow::{Context as _, bail, ensure};
use razer_storage::file_transfer::{Publication, Transfer};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SERVICE_DOES_NOT_EXIST, ERROR_SERVICE_EXISTS},
    Security::WinTrust::*,
    Storage::FileSystem::FILE_SHARE_READ,
    System::{Services::*, SystemInformation::GetSystemDirectoryW},
};

const MAX_DRIVER_BYTES: usize = 1024 * 1024;
const DELETE_SERVICE: u32 = 0x10000;

struct Service(SC_HANDLE);
impl Drop for Service {
    fn drop(&mut self) {
        // SAFETY: this object uniquely owns an SCM/service handle.
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}

fn wide(value: &std::ffi::OsStr) -> anyhow::Result<Vec<u16>> {
    let units: Vec<_> = value.encode_wide().collect();
    ensure!(!units.contains(&0), "Driver installation path contains NUL");
    Ok(units.into_iter().chain([0]).collect())
}

fn system_driver() -> anyhow::Result<PathBuf> {
    let mut buffer = vec![0; 32768];
    // SAFETY: the complete writable buffer is passed with its actual capacity.
    let count = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    ensure!(
        count != 0 && count < buffer.len(),
        "Cannot locate the protected system driver directory"
    );
    use std::os::windows::ffi::OsStringExt;
    Ok(
        PathBuf::from(std::ffi::OsString::from_wide(&buffer[..count]))
            .join("drivers")
            .join(SYSTEM_DRIVER_DIRECTORY)
            .join(DRIVER_FILENAME),
    )
}

fn reject_links(path: &Path) -> anyhow::Result<()> {
    for part in path.ancestors() {
        let info = match fs::symlink_metadata(part) {
            Ok(info) => info,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("Cannot inspect driver installation path"),
        };
        ensure!(
            !info.file_type().is_symlink() && info.file_attributes() & 0x400 == 0,
            "Driver installation path traverses a reparse point"
        );
    }
    Ok(())
}

fn source_file(path: &Path) -> anyhow::Result<(File, Vec<u8>)> {
    reject_links(path)?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .context("Packaged hardware driver is missing or cannot be locked for verification")?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() <= MAX_DRIVER_BYTES as u64,
        "Packaged hardware driver has an invalid length"
    );
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_DRIVER_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_DRIVER_BYTES,
        "Packaged hardware driver grew during verification"
    );
    validate_image(&bytes)?;
    verify_trust(&file)?;
    Ok((file, bytes))
}

fn validate_image(bytes: &[u8]) -> anyhow::Result<()> {
    fn word(bytes: &[u8], at: usize) -> anyhow::Result<u16> {
        Ok(u16::from_le_bytes(
            bytes
                .get(at..at + 2)
                .context("Truncated driver header")?
                .try_into()?,
        ))
    }
    ensure!(
        bytes.get(..2) == Some(b"MZ"),
        "Hardware driver has no DOS header"
    );
    let pe = u32::from_le_bytes(
        bytes
            .get(60..64)
            .context("Truncated driver PE header")?
            .try_into()?,
    ) as usize;
    ensure!(
        pe <= bytes.len().saturating_sub(96),
        "Truncated driver PE header"
    );
    ensure!(
        bytes.get(pe..pe + 4) == Some(b"PE\0\0"),
        "Invalid driver PE signature"
    );
    ensure!(
        word(bytes, pe + 4)? == 0x8664
            && word(bytes, pe + 24)? == 0x20b
            && word(bytes, pe + 24 + 68)? == 1
            && word(bytes, pe + 22)? & 0x2000 == 0,
        "Hardware monitoring requires the packaged x64 kernel driver"
    );
    Ok(())
}

fn verify_trust(file: &File) -> anyhow::Result<()> {
    let mut input = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        hFile: file.as_raw_handle(),
        ..Default::default()
    };
    let mut state = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut input },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: the owned file denies writes/deletion and all provider input
    // buffers remain alive until the explicit close call below.
    let status = unsafe {
        WinVerifyTrust(
            -1_isize as _,
            &mut action,
            (&mut state as *mut WINTRUST_DATA).cast(),
        )
    };
    state.dwStateAction = WTD_STATEACTION_CLOSE;
    let close = unsafe {
        WinVerifyTrust(
            -1_isize as _,
            &mut action,
            (&mut state as *mut WINTRUST_DATA).cast(),
        )
    };
    ensure!(
        status == 0 && close == 0,
        "Hardware driver signature verification failed ({status:#x}; close {close:#x})"
    );
    Ok(())
}

fn manager(create: bool) -> anyhow::Result<Service> {
    // SAFETY: null names select only the local service control manager.
    let handle = unsafe {
        OpenSCManagerW(
            ptr::null(),
            ptr::null(),
            SC_MANAGER_CONNECT | if create { SC_MANAGER_CREATE_SERVICE } else { 0 },
        )
    };
    ensure!(
        !handle.is_null(),
        "Cannot access local driver installation services: {}",
        std::io::Error::last_os_error()
    );
    Ok(Service(handle))
}

fn open_service(manager: &Service, access: u32) -> anyhow::Result<Option<Service>> {
    let name = wide(SERVICE_NAME.as_ref())?;
    let handle = unsafe { OpenServiceW(manager.0, name.as_ptr(), access) };
    if handle.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
            return Ok(None);
        }
        return Err(error).context("Cannot open hardware driver service");
    }
    Ok(Some(Service(handle)))
}

fn is_owner(service: &Service, expected: &Path) -> anyhow::Result<bool> {
    let mut needed = 0;
    let first = unsafe { QueryServiceConfigW(service.0, ptr::null_mut(), 0, &mut needed) };
    ensure!(
        first == 0
            && std::io::Error::last_os_error().raw_os_error()
                == Some(ERROR_INSUFFICIENT_BUFFER as i32)
            && needed as usize >= size_of::<QUERY_SERVICE_CONFIGW>()
            && needed <= 65536,
        "Cannot size hardware driver service configuration"
    );
    let mut buffer = vec![0_usize; (needed as usize).div_ceil(size_of::<usize>())];
    let capacity = buffer.len() * size_of::<usize>();
    let config = buffer.as_mut_ptr().cast::<QUERY_SERVICE_CONFIGW>();
    ensure!(
        unsafe { QueryServiceConfigW(service.0, config, capacity as u32, &mut needed) } != 0,
        "Cannot read hardware driver service configuration: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: the aligned buffer contains a successful SCM structure. Its
    // variable string pointer is bounds-checked before any UTF-16 read.
    let config = unsafe { &*config };
    if config.dwServiceType != SERVICE_KERNEL_DRIVER || config.dwStartType != SERVICE_DEMAND_START {
        return Ok(false);
    }
    let start = config.lpBinaryPathName as usize;
    let base = buffer.as_ptr() as usize;
    ensure!(
        start >= base && start < base + capacity && start % 2 == 0,
        "Invalid service binary path pointer"
    );
    let units = unsafe {
        std::slice::from_raw_parts(config.lpBinaryPathName, (base + capacity - start) / 2)
    };
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .context("Unterminated service binary path")?;
    let text = String::from_utf16(&units[..end]).context("Invalid service binary path encoding")?;
    let actual = Path::new(text.trim_matches('"'));
    // The protected file can be absent after a failed installation; compare
    // the normalized absolute configuration without substituting another path.
    let owned = actual.is_absolute()
        && actual
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy());
    if owned {
        reject_links(actual)?;
    }
    Ok(owned)
}

fn check_owner(service: &Service, expected: &Path) -> anyhow::Result<()> {
    ensure!(
        is_owner(service, expected)?,
        "Another installation owns the hardware driver service"
    );
    Ok(())
}

fn status(service: &Service) -> anyhow::Result<SERVICE_STATUS_PROCESS> {
    let mut state = SERVICE_STATUS_PROCESS::default();
    let mut needed = 0;
    ensure!(
        unsafe {
            QueryServiceStatusEx(
                service.0,
                SC_STATUS_PROCESS_INFO,
                (&mut state as *mut SERVICE_STATUS_PROCESS).cast(),
                size_of::<SERVICE_STATUS_PROCESS>() as u32,
                &mut needed,
            )
        } != 0,
        "Cannot read hardware driver service state: {}",
        std::io::Error::last_os_error()
    );
    Ok(state)
}

fn wait_for(service: &Service, expected: u32) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = status(service)?;
        if state.dwCurrentState == expected {
            return Ok(());
        }
        ensure!(
            matches!(
                state.dwCurrentState,
                SERVICE_START_PENDING | SERVICE_STOP_PENDING
            ),
            "Hardware driver entered state {}, expected {} (Win32 {}, service {})",
            state.dwCurrentState,
            expected,
            state.dwWin32ExitCode,
            state.dwServiceSpecificExitCode
        );
        ensure!(
            Instant::now() < deadline,
            "Hardware driver state transition timed out"
        );
        std::thread::sleep(Duration::from_millis(u64::from(
            (state.dwWaitHint / 10).clamp(10, 100),
        )));
    }
}

fn stop(service: &Service) -> anyhow::Result<()> {
    let state = status(service)?;
    if state.dwCurrentState == SERVICE_STOPPED {
        return Ok(());
    }
    if state.dwCurrentState != SERVICE_STOP_PENDING {
        let mut result = SERVICE_STATUS::default();
        ensure!(
            unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut result) } != 0,
            "Cannot stop hardware driver service: {}",
            std::io::Error::last_os_error()
        );
    }
    wait_for(service, SERVICE_STOPPED)
}

fn remove_service(manager: &Service, service: Service) -> anyhow::Result<()> {
    stop(&service)?;
    ensure!(
        unsafe { DeleteService(service.0) } != 0,
        "Cannot delete hardware driver service: {}",
        std::io::Error::last_os_error()
    );
    drop(service);
    ensure!(
        open_service(manager, SERVICE_QUERY_STATUS)?.is_none(),
        "Hardware driver service deletion is still pending"
    );
    Ok(())
}

pub fn install_from_application_directory() -> anyhow::Result<()> {
    let package = packaged_driver(&std::env::current_exe()?)?;
    let (_source, bytes) = source_file(&package)?;
    let destination = system_driver()?;
    reject_links(&destination)?;
    let mut connection = Connection::new()?;
    let result = {
        let mut lease = connection.acquire()?;
        let result = install_locked(&destination, &bytes);
        finish(
            result,
            lease.release(),
            "release hardware driver installation mutex",
        )
    };
    finish(
        result,
        connection.close(),
        "close hardware driver installation mutex",
    )
}

fn install_locked(destination: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let manager = manager(true)?;
    let access =
        SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS | SERVICE_START | SERVICE_STOP | DELETE_SERVICE;
    let existing = open_service(&manager, access)?;
    if let Some(service) = &existing {
        check_owner(service, destination)?;
    }
    let directory = destination
        .parent()
        .context("Hardware driver has no protected directory")?;
    fs::create_dir_all(directory).context("Cannot create protected hardware driver directory")?;
    reject_links(destination)?;
    let created_file = match fs::read(destination) {
        Ok(current) => {
            ensure!(
                current == bytes,
                "The protected hardware driver differs; uninstall its owned service before replacement"
            );
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Transfer::new()
                .accept()
                .map_err(anyhow::Error::msg)?
                .write(destination, bytes, MAX_DRIVER_BYTES, Publication::CreateNew)
                .map_err(anyhow::Error::msg)?;
            true
        }
        Err(error) => return Err(error).context("Cannot inspect protected hardware driver"),
    };
    let mut created_service = false;
    let mut started_service = false;
    let result = (|| {
        let (_protected, current) = source_file(destination)?;
        ensure!(
            current == bytes,
            "Published hardware driver differs from verified package"
        );
        let service = if let Some(service) = existing {
            service
        } else {
            let name = wide(SERVICE_NAME.as_ref())?;
            let path = wide(destination.as_os_str())?;
            let handle = unsafe {
                CreateServiceW(
                    manager.0,
                    name.as_ptr(),
                    name.as_ptr(),
                    access,
                    SERVICE_KERNEL_DRIVER,
                    SERVICE_DEMAND_START,
                    SERVICE_ERROR_NORMAL,
                    path.as_ptr(),
                    ptr::null(),
                    ptr::null_mut(),
                    ptr::null(),
                    ptr::null(),
                    ptr::null(),
                )
            };
            if handle.is_null() {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_SERVICE_EXISTS as i32) {
                    bail!("The hardware driver service was concurrently replaced");
                }
                return Err(error).context("Cannot create hardware driver service");
            }
            created_service = true;
            Service(handle)
        };
        check_owner(&service, destination)?;
        let state = status(&service)?;
        if state.dwCurrentState != SERVICE_RUNNING {
            ensure!(
                state.dwCurrentState == SERVICE_STOPPED,
                "Hardware driver is already changing state"
            );
            ensure!(
                unsafe { StartServiceW(service.0, 0, ptr::null()) } != 0,
                "Cannot start hardware driver service: {}",
                std::io::Error::last_os_error()
            );
            started_service = true;
            wait_for(&service, SERVICE_RUNNING)?;
        }
        // The global mutex is recursive on this same thread. Probe the actual
        // driver response before the installation command may report success.
        let mut probe = Connection::new()?;
        let processor = Processor::active()?
            .into_iter()
            .next()
            .context("No active processor")?;
        let observation = probe.sample(processor, |driver| driver.protocol_version());
        let version = finish(
            observation,
            probe.close(),
            "close installed driver verification",
        )?;
        ensure!(
            version == 0x165,
            "Installed hardware driver reports unexpected protocol version {version:#x}"
        );
        Ok(())
    })();
    if result.is_err() {
        let cleanup = (|| {
            if created_service || started_service {
                if let Some(service) = open_service(&manager, access)? {
                    check_owner(&service, destination)?;
                    if created_service {
                        remove_service(&manager, service)?;
                    } else {
                        stop(&service)?;
                    }
                }
            }
            if created_file && open_service(&manager, SERVICE_QUERY_STATUS)?.is_none() {
                fs::remove_file(destination)
                    .context("Cannot remove failed driver installation payload")?;
            }
            Ok(())
        })();
        return finish(result, cleanup, "clean failed hardware driver installation");
    }
    result
}

pub fn uninstall_owned_installation() -> anyhow::Result<()> {
    let destination = system_driver()?;
    reject_links(&destination)?;
    let mut connection = Connection::new()?;
    let result = {
        let mut lease = connection.acquire()?;
        let result = (|| {
            let manager = manager(false)?;
            if let Some(observation) =
                open_service(&manager, SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS)?
            {
                let owned = is_owner(&observation, &destination)?;
                drop(observation);
                if owned {
                    if let Some(service) = open_service(
                        &manager,
                        SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS | SERVICE_STOP | DELETE_SERVICE,
                    )? {
                        check_owner(&service, &destination)?;
                        remove_service(&manager, service)?;
                    }
                }
            }
            match fs::remove_file(&destination) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => {
                    return Err(error).context("Cannot remove owned hardware driver payload");
                }
            }
            if let Some(parent) = destination.parent() {
                match fs::remove_dir(parent) {
                    Ok(()) => (),
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                        ) =>
                    {
                        ()
                    }
                    Err(error) => {
                        return Err(error).context("Cannot remove owned driver directory");
                    }
                }
            }
            Ok(())
        })();
        finish(
            result,
            lease.release(),
            "release hardware driver uninstall mutex",
        )
    };
    finish(
        result,
        connection.close(),
        "close hardware driver uninstall mutex",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Vec<u8> {
        let mut bytes = vec![0; 256];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64_u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes[156..158].copy_from_slice(&1_u16.to_le_bytes());
        bytes
    }

    #[test]
    fn kernel_payload_validation_rejects_user_mode_wrong_architecture_and_truncation() {
        let original = image();
        validate_image(&original).unwrap();
        for (offset, value) in [(68, 0xaa64_u16), (88, 0x10b), (156, 2), (86, 0x2000)] {
            let mut bytes = original.clone();
            bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            assert!(validate_image(&bytes).is_err());
        }
        for length in [0, 1, 60, 63, 67, 100, 157] {
            assert!(validate_image(&original[..length]).is_err());
        }
        let mut bytes = original;
        bytes[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_image(&bytes).is_err());
    }

    #[test]
    fn failed_observation_and_failed_cleanup_are_both_retained() {
        let error = finish::<()>(
            Err(anyhow::anyhow!("response mismatch")),
            Err(anyhow::anyhow!("service stop failed")),
            "release driver",
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("response mismatch"));
        assert!(message.contains("service stop failed"));
        assert!(
            finish(
                Ok(0x165_u32),
                Err(anyhow::anyhow!("close failed")),
                "close driver"
            )
            .is_err()
        );
    }
}
