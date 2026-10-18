//! Local Apps inventory, signature-gated writes, removals and event ownership.
use super::NativeRuntime;
use anyhow::{Context as _, ensure};
use razer_device::user_app_files::{
    AppFileInput, AppFileResponse, AppServiceEvent, UserAppFilesOperation,
};
use serde_json::Value;
use std::io::Read;

#[path = "user_app_files_remove.rs"]
mod removal;
#[path = "user_app_signature.rs"]
mod signature;
#[path = "user_app_files_write.rs"]
mod write;

#[derive(Default)]
pub(super) struct Events {
    owners: Vec<String>,
    events: Vec<AppServiceEvent>,
    dropped_events: u64,
}
impl Events {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    fn enqueue(&mut self, event: Option<AppServiceEvent>) {
        if self.owners.is_empty() {
            return;
        }
        if let Some(event) = event {
            if self.events.len() < 1024 {
                self.events.push(event);
            } else {
                self.dropped_events = self.dropped_events.saturating_add(1);
            }
        }
    }
}

fn file_bytes(input: AppFileInput) -> anyhow::Result<Vec<u8>> {
    match input {
        AppFileInput::Bytes { data } => Ok(data),
        AppFileInput::LocalFile { path } => {
            // The same opened handle supplies metadata and bytes.
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
        match operation {
            UserAppFilesOperation::Subscribe { owner } => {
                let response = if owner.is_empty() {
                    AppFileResponse {
                        result: false,
                        reason: "missing sender url".into(),
                    }
                } else {
                    if !self.app_files.owners.contains(&owner) {
                        self.app_files.owners.push(owner);
                    }
                    AppFileResponse {
                        result: true,
                        reason: String::new(),
                    }
                };
                Ok(serde_json::to_value(response)?)
            }
            UserAppFilesOperation::Unsubscribe { owner } => {
                self.app_files.owners.retain(|entry| entry != &owner);
                Ok(serde_json::to_value(AppFileResponse {
                    result: true,
                    reason: String::new(),
                })?)
            }
            UserAppFilesOperation::DrainEvents => {
                let events = std::mem::take(&mut self.app_files.events);
                let dropped_events = std::mem::take(&mut self.app_files.dropped_events);
                Ok(serde_json::json!({"events": events, "dropped_events": dropped_events}))
            }
            UserAppFilesOperation::Read { name } => {
                let root = razer_platform::native_paths::user_apps_directory()?;
                Ok(serde_json::to_value(razer_storage::user_app_files::read(
                    &root, &name,
                )?)?)
            }
            UserAppFilesOperation::Write {
                folder_name,
                file_path,
                input,
            } => {
                let data = file_bytes(input)?;
                let root = razer_platform::native_paths::user_apps_directory()?;
                let written = write::write(&root, &folder_name, &file_path, &data)?;
                self.app_files.enqueue(written.event);
                Ok(serde_json::to_value(written.response)?)
            }
            operation => {
                let root = razer_platform::native_paths::user_apps_directory()?;
                let removed =
                    removal::remove(&root, &operation)?.expect("remaining operations are removals");
                self.app_files.enqueue(removed.event);
                Ok(serde_json::to_value(removed.response)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Callback2, callback2};
    use super::*;
    use std::{
        ffi::{CStr, CString, c_char},
        sync::Mutex,
    };

    static WRITTEN: Mutex<Option<(String, String, Vec<u8>)>> = Mutex::new(None);

    #[test]
    fn subscriptions_deduplicate_and_end_with_their_last_owner() {
        let mut runtime = NativeRuntime::new();
        let mut independent = NativeRuntime::new();
        for owner in ["page-a", "page-a", "page-b"] {
            let response = runtime
                .user_app_files(UserAppFilesOperation::Subscribe {
                    owner: owner.into(),
                })
                .unwrap();
            assert_eq!(response["result"], true);
        }
        assert_eq!(runtime.app_files.owners.len(), 2);
        let event = || {
            Some(AppServiceEvent::from_native(
                0,
                r#"{"app":"Common","filePath":"test.ico"}"#.into(),
            ))
        };
        for _ in 0..1027 {
            runtime.app_files.enqueue(event());
        }
        assert_eq!(runtime.app_files.events.len(), 1024);
        assert_eq!(runtime.app_files.dropped_events, 3);
        assert_eq!(
            independent
                .user_app_files(UserAppFilesOperation::DrainEvents)
                .unwrap()["events"],
            serde_json::json!([])
        );
        runtime
            .user_app_files(UserAppFilesOperation::Unsubscribe {
                owner: "page-a".into(),
            })
            .unwrap();
        assert_eq!(runtime.app_files.events.len(), 1024);
        runtime
            .user_app_files(UserAppFilesOperation::Unsubscribe {
                owner: "page-b".into(),
            })
            .unwrap();
        runtime.app_files.enqueue(event());
        // Native unregister clears only its enable flag. Events already posted
        // to the host callback remain deliverable to all live windows.
        assert_eq!(runtime.app_files.events.len(), 1024);
        assert_eq!(runtime.app_files.dropped_events, 3);
        let drained = runtime
            .user_app_files(UserAppFilesOperation::DrainEvents)
            .unwrap();
        assert_eq!(drained["events"].as_array().unwrap().len(), 1024);
        assert_eq!(drained["dropped_events"], 3);
        assert!(runtime.app_files.events.is_empty());
        runtime.app_files.clear();
        assert!(runtime.app_files.owners.is_empty());
        assert!(runtime.simple.is_none());
    }

    #[test]
    #[ignore = "requires audited original DLL; use tools/workspace.py"]
    fn connected_user_apps_read_only() {
        use super::super::{Callback3, Initialize, callback0, callback3};
        use crate::dll::EngineLibrary;
        use std::mem::ManuallyDrop;
        type ReadApps = unsafe extern "C" fn(*const c_char, Callback3);

        let path = std::env::var_os("RAZER_VERIFIED_SIMPLE_SERVICE")
            .expect("the maintained verifier must supply the audited original");
        let mut original = NativeRuntime::new();
        original.simple = Some(ManuallyDrop::new(
            EngineLibrary::load(std::path::Path::new(&path)).unwrap(),
        ));
        let compared = (|| -> anyhow::Result<Vec<Value>> {
            original.simple()?;
            let call: ReadApps = unsafe { original.simple_symbol("simpleGetUserApps")? };
            let root = razer_platform::native_paths::user_apps_directory()?;
            let mut results = Vec::new();
            for name in ["", "ChromaApp", "chromaapp", "__razer_inventory_missing__"] {
                let before = razer_storage::user_app_files::read(&root, name)?;
                let argument = CString::new(name)?;
                let source =
                    original.call_source(|| unsafe { call(argument.as_ptr(), callback3) })?;
                let after = razer_storage::user_app_files::read(&root, name)?;
                let source_value = source.value.unwrap_or_default();
                let parse = |text: &str| -> anyhow::Result<Value> {
                    if text.is_empty() {
                        Ok(Value::Null)
                    } else {
                        Ok(serde_json::from_str(text)?)
                    }
                };
                let unchanged = serde_json::to_value(&before)? == serde_json::to_value(&after)?;
                let matched = source.success == before.result
                    && source.reason == before.reason
                    && parse(&source_value)? == parse(&before.user_apps)?;
                results.push(serde_json::json!({
                    "name":name,"rust":before,
                    "original":{"result":source.success,"reason":source.reason,"userApps":source_value},
                    "input_unchanged":unchanged,"matched":matched
                }));
            }
            Ok(results)
        })();
        let cleanup = if original.simple_initialized && !original.poisoned {
            (|| -> anyhow::Result<()> {
                let shutdown: Initialize =
                    unsafe { original.simple_symbol("simpleServiceShutdown")? };
                original.call(|| unsafe { shutdown(callback0) })?;
                original.simple_initialized = false;
                Ok(())
            })()
        } else {
            Err(anyhow::anyhow!(
                "original did not reach a callable initialized state"
            ))
        };
        let compared = compared.map_err(|error| format!("{error:#}"));
        let cleanup = cleanup.map_err(|error| format!("{error:#}"));
        println!(
            "RAZER_APPS_RESULT {}",
            serde_json::json!({"comparisons":compared,"cleanup":cleanup})
        );
        assert!(cleanup.is_ok(), "original cleanup must complete");
        for row in compared.unwrap() {
            assert_eq!(
                row["input_unchanged"], true,
                "installed files changed during the comparison"
            );
            assert_eq!(row["matched"], true, "direct observation differs: {row}");
        }
    }

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
