use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "razer-app-remove-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, operation: UserAppFilesOperation) -> Removal {
        remove(&self.0, &operation).unwrap().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
const ABC: &str = "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD";
fn remove_file(hash: &str) -> UserAppFilesOperation {
    UserAppFilesOperation::RemoveFile {
        folder_name: "Common".into(),
        file_path: "test.bin".into(),
        file_hash: hash.into(),
    }
}

#[test]
fn file_removal_requires_current_exact_digest_and_publishes_source_event() {
    let root = Fixture::new();
    fs::create_dir(root.0.join("Common")).unwrap();
    let path = root.0.join("Common/test.bin");
    fs::write(&path, b"abc").unwrap();
    for hash in ["", &ABC.to_ascii_lowercase(), "wrong"] {
        let result = root.run(remove_file(hash));
        assert!(!result.response.result);
        assert!(result.event.is_none());
        assert_eq!(fs::read(&path).unwrap(), b"abc");
    }
    let result = root.run(remove_file(ABC));
    assert!(result.response.result);
    assert!(!path.exists());
    let event = result.event.unwrap();
    assert_eq!(event.event_id, "onsimpleremoveuserappfile");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&event.event_data).unwrap(),
        serde_json::json!({"app":"Common","filePath":"test.bin","fileHash":ABC})
    );
    let absent = root.run(remove_file(ABC));
    assert!(!absent.response.result);
    assert_eq!(
        absent.response.reason,
        "Error : User App file does not exist."
    );
    assert!(absent.event.is_none());
}

#[test]
fn directories_are_removed_only_after_empty_check_and_refresh_sees_actual_results() {
    let root = Fixture::new();
    fs::create_dir_all(root.0.join("Common/nested")).unwrap();
    fs::write(root.0.join("Common/nested/keep.bin"), b"abc").unwrap();
    let app = UserAppFilesOperation::RemoveApp {
        folder_name: "Common".into(),
    };
    let dir = UserAppFilesOperation::RemoveDirectory {
        folder_name: "Common".into(),
        dir_path: "nested".into(),
    };
    let result = root.run(app.clone());
    assert_eq!(result.response.reason, "Error : App is not empty.");
    assert!(result.event.is_none());
    let result = root.run(dir.clone());
    assert_eq!(
        result.response.reason,
        "Error : User App directory is not empty."
    );
    assert!(result.event.is_none());
    let file = root.run(UserAppFilesOperation::RemoveFile {
        folder_name: "Common".into(),
        file_path: "nested/keep.bin".into(),
        file_hash: ABC.into(),
    });
    assert!(file.response.result);
    let result = root.run(dir);
    assert!(result.response.result);
    assert_eq!(
        result.event.unwrap().event_id,
        "onsimpleremoveuserappdirectory"
    );
    let result = root.run(app);
    assert!(result.response.result);
    assert_eq!(result.event.unwrap().event_id, "onsimpleremoveuserapp");
    let inventory = razer_storage::user_app_files::read(&root.0, "").unwrap();
    assert!(!inventory.result);
    assert_eq!(inventory.user_apps, "");
}

#[test]
fn source_delete_clears_readonly_attribute_and_hash_failure_never_deletes() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = Fixture::new();
    fs::create_dir(root.0.join("Common")).unwrap();
    let path = root.0.join("Common/test.bin");
    fs::write(&path, b"abc").unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let result = root.run(remove_file(ABC));
    assert!(!result.response.result);
    assert!(
        result
            .response
            .reason
            .contains("Cannot read User App file hash")
    );
    assert!(result.event.is_none());
    drop(held);
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let result = root.run(remove_file(ABC));
    assert!(result.response.result);
    assert!(!path.exists());
}

#[test]
fn scope_rejects_root_parent_wildcard_and_alternate_stream_targets() {
    let root = Fixture::new();
    fs::create_dir(root.0.join("Common")).unwrap();
    for (folder, path) in [
        ("..", "outside"),
        ("Common", "../outside"),
        ("Common", "*"),
        ("Common", "a:stream"),
        (".", ""),
    ] {
        assert!(
            scoped_path(&root.0, folder, path).is_err(),
            "{folder}/{path}"
        );
    }
    let result = root.run(UserAppFilesOperation::RemoveDirectory {
        folder_name: "Common".into(),
        dir_path: "".into(),
    });
    assert_eq!(result.response.reason, "Error : Directory path is empty.");
    assert!(root.0.join("Common").exists());
}
