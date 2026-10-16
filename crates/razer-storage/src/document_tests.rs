use super::*;
use serde::{Serialize, Serializer};

struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new() -> Self {
        static NEXT_TEST: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-document-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn entries(&self) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn simultaneous_staging_does_not_truncate_or_remove_another_save() {
    let temp = TempDirectory::new();
    let first_target = temp.0.join("profile.json");
    let mut first = TemporaryDocument::create(&first_target).unwrap();
    first.write(b"first").unwrap();
    // Same target and same stem with another extension formerly shared a path.
    for target in [&first_target, &temp.0.join("profile.other")] {
        let mut second = TemporaryDocument::create(target).unwrap();
        second.write(b"second").unwrap();
        assert_eq!(std::fs::read(&first.path).unwrap(), b"first");
        assert_ne!(first.path, second.path);
        drop(second);
        assert_eq!(std::fs::read(&first.path).unwrap(), b"first");
    }
    drop(first);
    assert!(temp.entries().is_empty());
}

#[test]
fn abandoned_staging_closes_its_handle_before_removal() {
    let temp = TempDirectory::new();
    let staging = TemporaryDocument::create(&temp.0.join("profile.json")).unwrap();
    assert!(staging.path.is_file());
    drop(staging);
    assert!(temp.entries().is_empty());
}

/// Simulate an external edit between reading the previous version and the
/// final conflict check, without timing-dependent threads or external tools.
struct ConcurrentEdit<'a>(&'a Path);
impl Serialize for ConcurrentEdit<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        std::fs::write(self.0, b"\"external edit\"").unwrap();
        "new draft".serialize(serializer)
    }
}

#[test]
fn rejected_conflict_preserves_external_edit_and_removes_staging() {
    let temp = TempDirectory::new();
    let path = temp.0.join("profile.json");
    std::fs::write(&path, b"\"old draft\"").unwrap();
    assert!(write_document(&path, &ConcurrentEdit(&path), |_| Ok(())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"\"external edit\"");
    assert_eq!(temp.entries(), [std::ffi::OsString::from("profile.json")]);
}

#[test]
fn backup_failure_keeps_previous_document_and_removes_staging() {
    let temp = TempDirectory::new();
    let path = temp.0.join("profile.json");
    std::fs::write(&path, b"\"old draft\"").unwrap();
    // A directory cannot be overwritten by the backup's ordinary file write.
    std::fs::create_dir(path.with_extension("json.bak")).unwrap();
    assert!(write_document(&path, &"new draft", |_| Ok(())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"\"old draft\"");
    assert_eq!(
        temp.entries(),
        ["profile.json", "profile.json.bak"].map(std::ffi::OsString::from)
    );
}

#[test]
fn failed_install_removes_only_owned_staging() {
    let temp = TempDirectory::new();
    let destination = temp.0.join("profile.json");
    let mut staging = TemporaryDocument::create(&destination).unwrap();
    staging.write(b"new draft").unwrap();
    std::fs::create_dir(&destination).unwrap();
    assert!(staging.install(&destination).is_err());
    assert!(destination.is_dir());
    assert_eq!(temp.entries(), [std::ffi::OsString::from("profile.json")]);
}

#[test]
fn save_preserves_unowned_temporary_file_and_previous_backup() {
    let temp = TempDirectory::new();
    let path = temp.0.join("profile.json");
    let unowned = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&unowned, b"unowned staging").unwrap();
    std::fs::write(&path, b"\"old draft\"").unwrap();
    write_document(&path, &"new draft", |_| Ok(())).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"\"new draft\"");
    assert_eq!(std::fs::read(&unowned).unwrap(), b"unowned staging");
    assert_eq!(
        std::fs::read(path.with_extension("json.bak")).unwrap(),
        b"\"old draft\""
    );
    assert_eq!(temp.entries().len(), 3);
}
