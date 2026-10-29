use anyhow::{Context as _, ensure};
use std::{
    ffi::{OsStr, OsString},
    os::windows::{
        ffi::OsStringExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    ptr,
};
use tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, HANDLE, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    System::{
        Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
        Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
    },
};

#[derive(Clone, Debug)]
pub(super) struct Endpoint {
    identity: String,
    user_sid: String,
    installation: PathBuf,
    name: OsString,
}

impl Endpoint {
    pub fn new(installation: &Path) -> anyhow::Result<Self> {
        // SAFETY: GetCurrentProcess returns a borrowed pseudo-handle.
        let user_sid = process_sid(unsafe { GetCurrentProcess() })?;
        let installation_key = installation
            .to_str()
            .context("broker installation path is not valid Unicode")?
            .to_lowercase();
        let identity = format!(
            "{:x}",
            md5::compute(format!("razer-ui-hid-v1\0{user_sid}\0{installation_key}"))
        );
        Ok(Self {
            name: OsString::from(format!(r"\\.\pipe\RazerUi.HidBroker.v1.{identity}")),
            identity,
            user_sid,
            installation: installation.to_path_buf(),
        })
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn name(&self) -> &OsStr {
        &self.name
    }

    pub fn bind(&self, first_instance: bool) -> anyhow::Result<NamedPipeServer> {
        // The protected DACL grants only this user read/write pipe access.
        let sddl: Vec<u16> = format!("D:P(A;;GRGW;;;{})\0", self.user_sid)
            .encode_utf16()
            .collect();
        let mut descriptor = ptr::null_mut();
        // SAFETY: SDDL is NUL-terminated; descriptor is an initialized out pointer.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error())
                .context("broker pipe ACL could not be created");
        }
        let descriptor = LocalAllocation(descriptor);
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first_instance)
            .reject_remote_clients(true);
        // SAFETY: attributes and its descriptor remain valid through synchronous
        // CreateNamedPipeW. Windows copies security data; no handle is inherited.
        unsafe {
            options.create_with_security_attributes_raw(
                &self.name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            )
        }
        .context("broker pipe could not be bound")
    }

    pub fn verify_client(&self, pipe: &NamedPipeServer) -> anyhow::Result<()> {
        let mut process_id = 0;
        // SAFETY: pipe owns a connected pipe handle and process_id is writable.
        if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut process_id) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("broker client process identity is unavailable");
        }
        self.verify_process(process_id)
    }

    pub fn verify_server(&self, pipe: &NamedPipeClient) -> anyhow::Result<()> {
        let mut process_id = 0;
        // SAFETY: pipe owns the connected client handle and the out pointer is valid.
        if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut process_id) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("broker server process identity is unavailable");
        }
        self.verify_process(process_id)
    }

    fn verify_process(&self, process_id: u32) -> anyhow::Result<()> {
        ensure!(process_id != 0, "broker peer process identity is invalid");
        // SAFETY: the PID comes from the connected pipe; only query rights are requested.
        let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        ensure!(
            !raw.is_null(),
            "broker peer process could not be opened: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: successful OpenProcess transfers an owned process handle.
        let process = unsafe { OwnedHandle::from_raw_handle(raw) };
        ensure!(
            process_sid(process.as_raw_handle())? == self.user_sid,
            "broker peer belongs to a different user"
        );
        let mut image = vec![0u16; 32768];
        let mut length = image.len() as u32;
        // SAFETY: the process is retained; image supplies length writable UTF-16 units.
        if unsafe {
            QueryFullProcessImageNameW(process.as_raw_handle(), 0, image.as_mut_ptr(), &mut length)
        } == 0
        {
            return Err(std::io::Error::last_os_error())
                .context("broker peer executable path is unavailable");
        }
        let executable =
            PathBuf::from(OsString::from_wide(&image[..length as usize])).canonicalize()?;
        let peer_directory = executable
            .parent()
            .context("broker peer has no installation directory")?;
        ensure!(
            peer_directory
                .to_str()
                .context("broker peer path is not valid Unicode")?
                .to_lowercase()
                == self
                    .installation
                    .to_str()
                    .context("broker installation path is not valid Unicode")?
                    .to_lowercase(),
            "broker peer belongs to a different installation"
        );
        Ok(())
    }
}

struct LocalAllocation(*mut std::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: the pointer was allocated by an API documented to use LocalFree.
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn process_sid(process: HANDLE) -> anyhow::Result<String> {
    let mut raw_token = ptr::null_mut();
    // SAFETY: process is a retained query handle or current-process pseudo-handle.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(std::io::Error::last_os_error())
            .context("broker peer token could not be opened");
    }
    // SAFETY: OpenProcessToken returned an owned token handle.
    let token = unsafe { OwnedHandle::from_raw_handle(raw_token) };
    let mut needed = 0;
    // SAFETY: a null buffer with zero length queries the required size only.
    let result = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    ensure!(
        result == 0
            && std::io::Error::last_os_error().raw_os_error()
                == Some(ERROR_INSUFFICIENT_BUFFER as i32)
            && needed as usize >= std::mem::size_of::<TOKEN_USER>(),
        "broker peer token has an invalid user record"
    );
    let mut storage = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: usize storage supplies TOKEN_USER alignment and at least needed bytes.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error())
            .context("broker peer token user could not be read");
    }
    // SAFETY: successful TokenUser query initialized a complete aligned TOKEN_USER.
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut text = ptr::null_mut();
    // SAFETY: SID borrows the retained token buffer; text receives a LocalAlloc string.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
        return Err(std::io::Error::last_os_error())
            .context("broker peer SID could not be decoded");
    }
    let allocation = LocalAllocation(text.cast());
    let mut length = 0;
    // SAFETY: ConvertSidToStringSidW supplies a NUL-terminated UTF-16 SID string.
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
    }
    // SAFETY: the scan determined the initialized string length; allocation is retained.
    let value = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })?;
    drop(allocation);
    Ok(value)
}
