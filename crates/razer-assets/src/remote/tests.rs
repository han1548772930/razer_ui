use super::*;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

pub(super) struct TemporaryDirectory(pub(super) PathBuf);
impl TemporaryDirectory {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "razer-resource-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
}
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("razer-resource-tests-")
        );
        let _ = fs::remove_dir_all(actual);
    }
}

pub(super) fn file(path: &str, bytes: &[u8]) -> ResourceFile {
    ResourceFile {
        path: path.into(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}

#[test]
fn cache_is_beside_the_installation_and_scoped_by_product_and_content() {
    let installation = std::env::temp_dir().join("razer-resource-installation");
    let cache = Cache::alongside(&installation.join("razer_ui.exe")).unwrap();
    assert!(Cache::alongside(Path::new("installation/razer_ui.exe")).is_err());
    let first = file("assets/synapse/products/182/icon.svg", b"one");
    let next = file("assets/synapse/products/182/icon.svg", b"two");
    let other = file("assets/synapse/products/214/icon.svg", b"one");
    assert!(
        cache
            .path(&first)
            .unwrap()
            .starts_with(installation.join("assets/cache/synapse/products/182"))
    );
    assert_ne!(cache.path(&first).unwrap(), cache.path(&next).unwrap());
    assert_ne!(cache.path(&first).unwrap(), cache.path(&other).unwrap());
}

#[test]
fn corrupt_truncated_and_wrong_version_bytes_are_never_loaded() {
    let directory = TemporaryDirectory::new();
    let cache = Cache {
        root: directory.0.clone(),
    };
    let entry = file("assets/synapse/products/182/icon.svg", b"correct");
    assert!(cache.publish(&entry, b"incorrect").is_err());
    cache.publish(&entry, b"correct").unwrap();
    assert_eq!(cache.read(&entry).unwrap().unwrap(), b"correct");
    fs::write(cache.path(&entry).unwrap(), b"bad").unwrap();
    assert!(cache.read(&entry).unwrap().is_none());
    cache.publish(&entry, b"correct").unwrap();
    assert_eq!(cache.read(&entry).unwrap().unwrap(), b"correct");
}

#[test]
fn unrelated_products_are_neither_created_nor_requested() {
    let directory = TemporaryDirectory::new();
    let cache = Cache {
        root: directory.0.clone(),
    };
    let first = file("assets/synapse/products/182/icon.svg", b"one");
    cache.publish(&first, b"one").unwrap();
    assert!(!directory.0.join("synapse/products/214").exists());
    assert_eq!(cache.read(&first).unwrap().unwrap(), b"one");
}

#[test]
fn untrusted_paths_cannot_escape_or_address_alternate_streams() {
    for path in [
        "../x",
        "assets/../../x",
        "C:/x",
        "assets/x:stream",
        "assets/./x",
        "assets//x",
        "assets/x?y",
        "assets/%2e%2e/x",
        "assets/x.",
        "assets\\x",
    ] {
        assert!(safe_relative(path).is_err(), "{path}");
    }
}

#[test]
fn metadata_keeps_first_alias_original_list_order_and_native_fallback() {
    for (key, _) in crate::window_icons::ICONS {
        assert!(available(key).unwrap());
    }
    let manifest = manifest().unwrap();
    assert!(manifest.entries.len() > 11_000);
    assert_eq!(manifest.fonts.len(), 14);
    for (key, ix) in &manifest.keys {
        assert_eq!(
            *ix,
            manifest
                .entries
                .iter()
                .position(|entry| entry.key == *key)
                .unwrap()
        );
    }
    assert!(list("synapse/audio-demo-control-play").unwrap().is_empty());
    assert!(
        list("synapse/")
            .unwrap()
            .iter()
            .any(|key| *key == "synapse/audio-demo-control-play.svg")
    );
    for file in manifest
        .files
        .iter()
        .filter(|file| embedded(&file.path).is_some())
    {
        assert!(verified(file, embedded(&file.path).unwrap()));
    }
}

#[test]
fn remote_uses_the_pinned_revision_and_never_downloads_an_archive() {
    let storage = Storage {
        provider: "huggingface".into(),
        repository: "owner/assets".into(),
        revision: Some("a".repeat(40)),
    };
    let file = file("assets/synapse/products/182/image.png", b"image");
    assert_eq!(
        remote_url(&storage, &file).unwrap(),
        format!(
            "https://huggingface.co/datasets/owner/assets/resolve/{}/assets/synapse/products/182/image.png",
            "a".repeat(40)
        )
    );
}
