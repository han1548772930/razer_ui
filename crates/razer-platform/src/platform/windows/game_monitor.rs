//! Read-only OS adapters for the local GMS replacement. Current vendor
//! TraceEventProcessExtractor uses OpenProcess(0x1000), image-name query and
//! CloseHandle. All handles belong to this synchronous background snapshot.
use crate::game_monitor::{CatalogLocations, ProcessSnapshot, RunningProcess, executable_stem};
use anyhow::{Context as _, Result, ensure};
use std::{collections::BTreeSet, path::PathBuf};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, ERROR_PATH_NOT_FOUND, GetLastError,
        HANDLE, INVALID_HANDLE_VALUE,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW},
        SystemInformation::{
            ComputerNameNetBIOS, GetComputerNameExW, GetNativeSystemInfo,
            PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64,
            PROCESSOR_ARCHITECTURE_IA64, SYSTEM_INFO,
        },
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn process_snapshot(
    candidates: &BTreeSet<String>,
    guard: &impl Fn() -> Result<()>,
) -> Result<ProcessSnapshot> {
    guard()?;
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    ensure!(
        snapshot != INVALID_HANDLE_VALUE,
        "Create process snapshot: {}",
        std::io::Error::last_os_error()
    );
    let snapshot = Handle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut present = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    let mut processes = Vec::new();
    let mut diagnostics = Vec::new();
    loop {
        guard()?;
        if !present {
            let error = unsafe { GetLastError() };
            ensure!(
                error == ERROR_NO_MORE_FILES,
                "Enumerate process snapshot: {}",
                std::io::Error::from_raw_os_error(error as i32)
            );
            break;
        }
        let pid = entry.th32ProcessID;
        let end = entry
            .szExeFile
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(entry.szExeFile.len());
        let candidate = String::from_utf16(&entry.szExeFile[..end])
            .ok()
            .is_some_and(|name| candidates.contains(&executable_stem(&name).to_uppercase()));
        if !matches!(pid, 0 | 4) && candidate {
            match process_path(pid) {
                Ok(path) => processes.push(RunningProcess::new(pid, path)),
                // Original skips unavailable paths. Keep Rust diagnostics
                // separate from both source memory state and device replies.
                Err(error) => diagnostics.push(format!("PID {pid}: {error:#}")),
            }
        }
        present = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    Ok(ProcessSnapshot::new(processes, diagnostics))
}

fn process_path(pid: u32) -> Result<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    ensure!(
        !process.is_null(),
        "Open process image: {}",
        std::io::Error::last_os_error()
    );
    let process = Handle(process);
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    ensure!(
        unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length) } != 0,
        "Read process image: {}",
        std::io::Error::last_os_error()
    );
    ensure!(
        length > 0 && length as usize <= buffer.len(),
        "Invalid process image length"
    );
    String::from_utf16(&buffer[..length as usize]).context("Process image is not valid UTF-16")
}

pub(crate) fn catalog_locations() -> Result<(CatalogLocations, bool)> {
    let root = std::env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .context("ProgramData directory is unavailable")?
        .join("Razer/GameManager3");
    let fallback = service_directory()?
        .or_else(|| {
            std::env::var_os("ProgramFiles(x86)")
                .map(PathBuf::from)
                .map(|path| path.join("Razer/Razer Services/GMS3"))
        })
        .map(|path| path.join("MasterGameList.xml"));
    let mut machine = [0u16; 256];
    let mut length = machine.len() as u32;
    ensure!(
        unsafe { GetComputerNameExW(ComputerNameNetBIOS, machine.as_mut_ptr(), &mut length) } != 0,
        "Read local catalog machine name: {}",
        std::io::Error::last_os_error()
    );
    let machine = String::from_utf16(&machine[..length as usize])?;
    let mut info = SYSTEM_INFO::default();
    unsafe { GetNativeSystemInfo(&mut info) };
    let is_64_bit = matches!(
        unsafe { info.Anonymous.Anonymous.wProcessorArchitecture },
        PROCESSOR_ARCHITECTURE_AMD64 | PROCESSOR_ARCHITECTURE_ARM64 | PROCESSOR_ARCHITECTURE_IA64
    );
    Ok((CatalogLocations::new(root, fallback, &machine), is_64_bit))
}

fn service_directory() -> Result<Option<PathBuf>> {
    let wide = |value: &str| value.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let key = wide(r"SYSTEM\CurrentControlSet\Services\Razer Game Manager Service 3");
    let name = wide("ImagePath");
    let mut buffer = vec![0u16; 32768];
    let mut bytes = (buffer.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if matches!(result, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
        return Ok(None);
    }
    ensure!(
        result == 0,
        "Read installed GMS catalog location: {}",
        std::io::Error::from_raw_os_error(result as i32)
    );
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .context("Installed GMS ImagePath is not terminated")?;
    let command = String::from_utf16(&buffer[..end])?;
    let executable = if let Some(quoted) = command.trim().strip_prefix('"') {
        quoted
            .split_once('"')
            .context("Installed GMS ImagePath quote is incomplete")?
            .0
    } else {
        let end = command
            .to_ascii_lowercase()
            .find(".exe")
            .context("Installed GMS ImagePath is not an executable")?
            + 4;
        &command[..end]
    };
    let executable = PathBuf::from(executable.trim());
    ensure!(
        executable.is_absolute(),
        "Installed GMS ImagePath is not absolute"
    );
    Ok(executable.parent().map(PathBuf::from))
}
