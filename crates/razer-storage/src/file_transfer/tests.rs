use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-file-transfer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        for entry in fs::read_dir(&self.0).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                fs::remove_dir(entry.path()).unwrap();
            } else {
                fs::remove_file(entry.path()).unwrap();
            }
        }
        fs::remove_dir(&self.0).unwrap();
    }
}

#[test]
fn cancellation_after_staging_preserves_the_previous_export_and_other_staging_files() {
    let directory = Directory::new();
    let path = directory.0.join("macro.xml");
    fs::write(&path, b"previous export").unwrap();
    let other = directory.0.join("another.tmp");
    fs::write(&other, b"another owner").unwrap();
    let request = Transfer::new();
    let mut staged = TemporaryDocument::create(&path).unwrap();
    let own_path = staged.path.clone();
    staged.write(b"cancelled export").unwrap();
    let operation = request.accept().unwrap();
    let clone = request.clone();
    assert!(clone.is_same(&request));
    clone.cancel();
    assert!(
        operation
            .publish(staged, &path, Publication::Replace)
            .is_err()
    );
    assert!(!own_path.exists());
    assert_eq!(fs::read(&path).unwrap(), b"previous export");
    assert_eq!(fs::read(&other).unwrap(), b"another owner");
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
fn cancelled_owner_does_not_cancel_its_replacement() {
    let directory = Directory::new();
    let path = directory.0.join("macro.xml");
    let old = Transfer::new();
    let current = Transfer::new();
    assert!(!current.is_same(&old));
    old.cancel();
    assert!(old.accept().err().unwrap().contains("取消"));
    assert!(old.accept().is_err());
    assert!(!path.exists());
    current
        .accept()
        .unwrap()
        .write(&path, b"new", 128, Publication::Replace)
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"new");
    assert!(old.accept().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"new");
}

#[test]
fn cancelling_an_accepted_publication_does_not_wait_for_its_filesystem_work() {
    use std::{sync::mpsc, thread, time::Duration};
    let request = Transfer::new();
    let worker_request = request.clone();
    let cancel_request = request.clone();
    let (accepted, wait_accepted) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (cancelled, wait_cancelled) = mpsc::channel();
    let worker = thread::spawn(move || {
        worker_request.with_publication(|| {
            accepted.send(()).unwrap();
            wait_release.recv().unwrap();
            Ok(())
        })
    });
    wait_accepted.recv().unwrap();
    let canceller = thread::spawn(move || {
        cancel_request.cancel();
        cancelled.send(()).unwrap();
    });
    let cancellation_returned = wait_cancelled.recv_timeout(Duration::from_secs(2)).is_ok();
    release.send(()).unwrap();
    canceller.join().unwrap();
    worker.join().unwrap().unwrap();
    assert!(
        cancellation_returned,
        "GUI cancellation waited on file publication"
    );
    assert!(request.accept().is_err());
}

#[test]
fn successful_export_atomically_replaces_the_selected_file() {
    let directory = Directory::new();
    let path = directory.0.join("macro.xml");
    fs::write(&path, b"previous export").unwrap();
    Transfer::new()
        .accept()
        .unwrap()
        .write(&path, "你好 <Macro/>".as_bytes(), 128, Publication::Replace)
        .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "你好 <Macro/>");
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn publication_failure_cleans_owned_staging_without_removing_the_destination() {
    let directory = Directory::new();
    let path = directory.0.join("occupied.xml");
    fs::create_dir(&path).unwrap();
    assert!(
        Transfer::new()
            .accept()
            .unwrap()
            .write(&path, b"new", 128, Publication::Replace)
            .is_err()
    );
    assert!(path.is_dir());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn create_new_publication_never_replaces_an_existing_export() {
    let directory = Directory::new();
    let path = directory.0.join("profile.json");
    let first = Transfer::new();
    first
        .accept()
        .unwrap()
        .write(&path, b"complete first", 128, Publication::CreateNew)
        .unwrap();
    let replacement = Transfer::new();
    assert!(
        replacement
            .accept()
            .unwrap()
            .write(&path, b"replacement", 128, Publication::CreateNew)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"complete first");
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn bounded_read_rejects_oversize_directory_and_cancelled_work() {
    let directory = Directory::new();
    let path = directory.0.join("profiles.synapse4");
    fs::write(&path, b"12345").unwrap();
    let request = Transfer::new();
    let operation = request.accept().unwrap();
    assert!(operation.read(&path, 4).is_err());
    assert!(operation.read(&directory.0, 64).is_err());
    assert_eq!(operation.read(&path, 5).unwrap(), b"12345");
    request.cancel();
    assert!(operation.read(&path, 5).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"12345");
}

#[test]
fn accepted_queued_operation_is_counted_before_its_worker_starts() {
    use std::{sync::mpsc, thread};
    let request = Transfer::new();
    let operation = request.accept().unwrap();
    assert!(active_io() > 0);
    let (release, waiting) = mpsc::channel();
    let worker = thread::spawn(move || {
        waiting.recv().unwrap();
        assert!(operation.check().is_err());
    });
    request.cancel();
    // The accepted permit remains owned even though cancellation has returned.
    assert!(active_io() > 0);
    release.send(()).unwrap();
    worker.join().unwrap();
}
