//! Installed Apps Service adapter; no installer, shell command or download.
//! The current host ABI and full native bodies are retained in the device
//! crate's user-app-files-current evidence fixture. Native signature checking
//! (including the custom ICO verifier) is retained, never replaced by fs::write.
use super::{Callback2, Callback3, NativeRuntime, callback2, callback3};
use anyhow::{Context as _, ensure};
use razer_device::user_app_files::{
    AppFileInput, AppFileResponse, AppServiceEvent, UserAppFilesOperation, UserAppsResponse,
    validate_write,
};
use serde_json::Value;
use std::{
    ffi::{CString, c_char},
    io::Read,
    sync::Mutex,
};

type ReadApps = unsafe extern "C" fn(*const c_char, Callback3);
type WriteFile = unsafe extern "C" fn(*const c_char, *const c_char, *const u8, u32, Callback2);
type RemoveFile = unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, Callback2);
type RemoveDirectory = unsafe extern "C" fn(*const c_char, *const c_char, Callback2);
type RemoveApp = unsafe extern "C" fn(*const c_char, Callback2);
type Register = unsafe extern "C" fn(Callback2);
type AppsCallback = unsafe extern "C" fn(i32, *const c_char);
type SetCallback = unsafe extern "C" fn(AppsCallback, Callback2);

// Static callback storage outlives the DLL worker. Rust queue overflow is kept
// separate from the source event payload, so lost observations are explicit.
static EVENTS: Mutex<(Vec<AppServiceEvent>, u64)> = Mutex::new((Vec::new(), 0));
unsafe extern "C" fn apps_event(kind: i32, data: *const c_char) {
    if let Ok(mut queue) = EVENTS.lock() {
        if queue.0.len() < 1024 {
            queue.0.push(AppServiceEvent::from_native(kind, unsafe {
                super::copy_string(data)
            }));
        } else {
            queue.1 = queue.1.saturating_add(1);
        }
    }
}

fn file_bytes(input: AppFileInput) -> anyhow::Result<Vec<u8>> {
    match input {
        AppFileInput::Bytes { data } => Ok(data),
        AppFileInput::LocalFile { path } => {
            // Read one owned handle. Metadata and bytes therefore refer to the
            // same opened file even when another process replaces the pathname.
            let file = std::fs::File::open(&path).context("Open local Apps Service input")?;
            let metadata = file.metadata().context("Read local input metadata")?;
            ensure!(
                metadata.is_file(),
                "Apps Service input is not a regular file"
            );
            ensure!(
                metadata.len() <= u32::MAX as u64,
                "Apps Service input exceeds uint32 ABI"
            );
            let mut data = Vec::new();
            file.take(u32::MAX as u64 + 1)
                .read_to_end(&mut data)
                .context("Read local Apps Service input")?;
            ensure!(
                data.len() <= u32::MAX as usize,
                "Apps Service input grew beyond uint32 ABI"
            );
            Ok(data)
        }
    }
}

impl NativeRuntime {
    pub(super) fn user_app_files(
        &mut self,
        operation: UserAppFilesOperation,
    ) -> anyhow::Result<Value> {
        self.simple()?;
        match operation {
            UserAppFilesOperation::Subscribe { owner } => {
                if owner.is_empty() {
                    return Ok(serde_json::to_value(AppFileResponse {
                        result: false,
                        reason: "missing sender url".into(),
                    })?);
                }
                if !self.app_file_owners.is_empty() {
                    if !self.app_file_owners.contains(&owner) {
                        self.app_file_owners.push(owner);
                    }
                    return Ok(serde_json::to_value(AppFileResponse {
                        result: true,
                        reason: String::new(),
                    })?);
                }
                self.app_file_owners.push(owner.clone());
                let outcome = (|| {
                    let call: Register = unsafe { self.simple_symbol("registerAppsServiceEvent")? };
                    self.call_source(|| unsafe { call(callback2) })
                })();
                let reply = match outcome {
                    Ok(reply) => reply,
                    Err(error) => {
                        self.app_file_owners.retain(|entry| entry != &owner);
                        return Err(error);
                    }
                };
                if reply.success {
                    let set: SetCallback =
                        unsafe { self.simple_symbol("setAppsServiceEventCallback")? };
                    // Source awaits setup but returns the register result even
                    // if callback setup returns false. Preserve that distinction.
                    let setup = self.call_source(|| unsafe { set(apps_event, callback2) })?;
                    if !setup.success {
                        eprintln!("Apps event callback setup failed: {}", setup.reason);
                    }
                } else {
                    self.app_file_owners.retain(|entry| entry != &owner);
                }
                Ok(serde_json::to_value(AppFileResponse {
                    result: reply.success,
                    reason: reply.reason,
                })?)
            }
            UserAppFilesOperation::Unsubscribe { owner } => {
                self.app_file_owners.retain(|entry| entry != &owner);
                if !self.app_file_owners.is_empty() {
                    return Ok(serde_json::to_value(AppFileResponse {
                        result: true,
                        reason: String::new(),
                    })?);
                }
                let outcome = (|| {
                    let call: Register =
                        unsafe { self.simple_symbol("unregisterAppsServiceEvent")? };
                    self.call_source(|| unsafe { call(callback2) })
                })();
                let reply = match outcome {
                    Ok(reply) => reply,
                    Err(error) => {
                        self.app_file_owners.push(owner);
                        return Err(error);
                    }
                };
                if !reply.success {
                    self.app_file_owners.push(owner);
                }
                Ok(serde_json::to_value(AppFileResponse {
                    result: reply.success,
                    reason: reply.reason,
                })?)
            }
            UserAppFilesOperation::DrainEvents => {
                let mut queue = EVENTS
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Apps event queue is poisoned"))?;
                let (events, dropped_events) = std::mem::take(&mut *queue);
                Ok(serde_json::json!({"events": events, "dropped_events": dropped_events}))
            }
            UserAppFilesOperation::Read { name } => {
                let name = CString::new(name)?;
                let call: ReadApps = unsafe { self.simple_symbol("simpleGetUserApps")? };
                let reply = self.call_source(|| unsafe { call(name.as_ptr(), callback3) })?;
                // The JS wrapper returns userApps as the original JSON string,
                // including on negative callbacks. It does not parse or refresh UI.
                Ok(serde_json::to_value(UserAppsResponse {
                    result: reply.success,
                    reason: reply.reason,
                    user_apps: reply.value.unwrap_or_default(),
                })?)
            }
            UserAppFilesOperation::Write {
                folder_name,
                file_path,
                input,
            } => {
                let data = file_bytes(input)?;
                if let Some(response) = validate_write(&folder_name, &file_path, data.len() as u64)?
                {
                    return Ok(serde_json::to_value(response)?);
                }
                let folder = CString::new(folder_name)?;
                let path = CString::new(file_path)?;
                let call: WriteFile = unsafe { self.simple_symbol("simpleAddUserAppFile")? };
                // Native 0x10660 copies both strings and the entire byte vector
                // before posting its service task. Keep every argument alive
                // until the callback anyway; never retry after timeout.
                let reply = self.call_source(|| unsafe {
                    call(
                        folder.as_ptr(),
                        path.as_ptr(),
                        data.as_ptr(),
                        data.len() as u32,
                        callback2,
                    )
                })?;
                Ok(serde_json::to_value(AppFileResponse {
                    result: reply.success,
                    reason: reply.reason,
                })?)
            }
            UserAppFilesOperation::RemoveFile {
                folder_name,
                file_path,
                file_hash,
            } => {
                let folder = CString::new(folder_name)?;
                let path = CString::new(file_path)?;
                let hash = CString::new(file_hash)?;
                let call: RemoveFile = unsafe { self.simple_symbol("simpleRemoveUserAppFile")? };
                let reply = self.call_source(|| unsafe {
                    call(folder.as_ptr(), path.as_ptr(), hash.as_ptr(), callback2)
                })?;
                Ok(serde_json::to_value(AppFileResponse {
                    result: reply.success,
                    reason: reply.reason,
                })?)
            }
            UserAppFilesOperation::RemoveDirectory {
                folder_name,
                dir_path,
            } => {
                let folder = CString::new(folder_name)?;
                let path = CString::new(dir_path)?;
                let call: RemoveDirectory =
                    unsafe { self.simple_symbol("simpleRemoveUserAppDirectory")? };
                let reply = self
                    .call_source(|| unsafe { call(folder.as_ptr(), path.as_ptr(), callback2) })?;
                Ok(serde_json::to_value(AppFileResponse {
                    result: reply.success,
                    reason: reply.reason,
                })?)
            }
            UserAppFilesOperation::RemoveApp { folder_name } => {
                let folder = CString::new(folder_name)?;
                let call: RemoveApp = unsafe { self.simple_symbol("simpleRemoveUserApp")? };
                let reply = self.call_source(|| unsafe { call(folder.as_ptr(), callback2) })?;
                Ok(serde_json::to_value(AppFileResponse {
                    result: reply.success,
                    reason: reply.reason,
                })?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    static WRITTEN: Mutex<Option<(String, String, Vec<u8>)>> = Mutex::new(None);

    unsafe extern "C" fn mock_add(
        folder: *const c_char,
        path: *const c_char,
        bytes: *const u8,
        length: u32,
        callback: Callback2,
    ) {
        let args = (
            unsafe { CStr::from_ptr(folder) }
                .to_string_lossy()
                .into_owned(),
            unsafe { CStr::from_ptr(path) }
                .to_string_lossy()
                .into_owned(),
            unsafe { std::slice::from_raw_parts(bytes, length as usize) }.to_vec(),
        );
        *WRITTEN.lock().unwrap() = Some(args);
        unsafe { callback(false, c"Error: This PE file is not trusted".as_ptr()) };
    }

    #[test]
    fn simulated_native_write_retains_bytes_and_failed_callback_without_poisoning() {
        // No DLL loading or export dispatch: the callback is produced entirely
        // by this Rust mock. Runtime construction only inspects local paths.
        let mut runtime = NativeRuntime::new();
        let folder = c"Common";
        let path = c"engine.dll";
        let bytes = [0x4d, 0x5a, 0, 0xff, 0];
        let reply = runtime
            .call_source(|| unsafe {
                mock_add(
                    folder.as_ptr(),
                    path.as_ptr(),
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    callback2,
                )
            })
            .unwrap();
        assert!(!reply.success);
        assert_eq!(reply.reason, "Error: This PE file is not trusted");
        assert_eq!(
            WRITTEN.lock().unwrap().take().unwrap(),
            ("Common".into(), "engine.dll".into(), bytes.to_vec())
        );
        assert!(!runtime.is_poisoned());
        let next = runtime
            .call_source(|| unsafe { callback2(true, c"".as_ptr()) })
            .unwrap();
        assert!(next.success);
        // Existing native users still receive an error for rejected callbacks.
        assert!(
            runtime
                .call(|| unsafe { callback2(false, c"rejected".as_ptr()) })
                .is_err()
        );
        assert!(!runtime.is_poisoned());
    }
}
