use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "razer-app-inventory-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn actual_file_bytes_and_nested_relative_paths_enter_original_response() {
    let root = Fixture::new();
    fs::create_dir_all(root.0.join("Common/nested")).unwrap();
    fs::write(root.0.join("Common/one.bin"), b"abc").unwrap();
    fs::write(root.0.join("Common/nested/two.bin"), b"").unwrap();
    fs::write(root.0.join("ignored.bin"), b"not an app").unwrap();
    let result = read(&root.0, "Common").unwrap();
    assert!(result.result);
    assert_eq!(result.reason, "");
    let value: serde_json::Value = serde_json::from_str(&result.user_apps).unwrap();
    assert_eq!(
        value[0]["Common"][0],
        serde_json::json!({
            "name": "one.bin", "sha256": "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        })
    );
    assert_eq!(
        value[0]["Common"][1],
        serde_json::json!({
            "name": Path::new("nested").join("two.bin").to_str().unwrap(),
            "sha256": "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855"
        })
    );
}

#[test]
fn empty_inventory_empty_app_and_absent_root_remain_distinct() {
    let root = Fixture::new();
    let empty = read(&root.0, "").unwrap();
    assert!(!empty.result);
    assert_eq!(empty.user_apps, "");
    assert_eq!(empty.reason, "");
    fs::create_dir(root.0.join("Empty")).unwrap();
    let app = read(&root.0, "Empty").unwrap();
    assert!(app.result);
    assert_eq!(app.user_apps, "[{\"Empty\":[]}]");
    let missing = read(&root.0.join("absent"), "").unwrap();
    assert!(!missing.result);
    assert_eq!(missing.reason, "Error : Apps directory does not exist.");
    assert_eq!(missing.user_apps, "");
    let missing = read(&root.0, "absent").unwrap();
    assert_eq!(missing.reason, "Error : App does not exist.");
    assert!(!missing.result);
}

#[test]
fn observation_rehashes_current_bytes_without_cached_digest() {
    let root = Fixture::new();
    fs::create_dir(root.0.join("Common")).unwrap();
    fs::write(root.0.join("Common/current.bin"), b"abc").unwrap();
    let before = read(&root.0, "").unwrap();
    fs::write(root.0.join("Common/current.bin"), b"").unwrap();
    let after = read(&root.0, "").unwrap();
    assert_ne!(before.user_apps, after.user_apps);
    assert!(
        after
            .user_apps
            .contains("E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855")
    );
    assert!(read(&root.0, "bad\0name").is_err());
}

#[cfg(windows)]
#[test]
fn locked_file_is_a_failed_read_instead_of_empty_successful_hash() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = Fixture::new();
    fs::create_dir(root.0.join("Common")).unwrap();
    let path = root.0.join("Common/locked.bin");
    fs::write(&path, b"abc").unwrap();
    fs::write(root.0.join("Common/observed.bin"), b"abc").unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let partial = read(&root.0, "Common").unwrap();
    assert!(!partial.result);
    assert!(partial.reason.contains("Open installed app file"));
    assert!(partial.reason.contains("locked.bin"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&partial.user_apps).unwrap(),
        serde_json::json!([{"Common":[{"name":"observed.bin","sha256":"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"}]}])
    );
    drop(held);
    assert!(read(&root.0, "Common").unwrap().result);
}

#[cfg(windows)]
#[test]
fn application_filter_is_case_sensitive_despite_windows_path_lookup() {
    let root = Fixture::new();
    fs::create_dir(root.0.join("Common")).unwrap();
    let result = read(&root.0, "common").unwrap();
    assert!(!result.result);
    assert_eq!(result.reason, "");
    assert_eq!(result.user_apps, "");
}

#[test]
fn existing_file_name_is_filtered_without_an_absent_app_error() {
    let root = Fixture::new();
    fs::write(root.0.join("plain.bin"), b"abc").unwrap();
    let result = read(&root.0, "plain.bin").unwrap();
    assert!(!result.result);
    assert_eq!(result.reason, "");
    assert_eq!(result.user_apps, "");
}
