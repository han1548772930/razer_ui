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
    use super::*;

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
    }

    #[test]
    #[ignore = "requires audited local user-app files; use tools/workspace.py"]
    fn direct_user_apps_read_only() {
        let root=razer_platform::native_paths::user_apps_directory().unwrap();
        let rows=["","ChromaApp","chromaapp","__razer_inventory_missing__"].into_iter().map(|name| {
            serde_json::json!({"name":name,"rust":razer_storage::user_app_files::read(&root,name).unwrap()})
        }).collect::<Vec<_>>();
        println!("RAZER_APPS_RESULT {}",serde_json::to_string(&rows).unwrap());
    }
}
