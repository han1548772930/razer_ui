use super::*;
use razer_asset_pack::{PackReader, write_pack_from_loader};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct TemporaryPack(PathBuf);

impl TemporaryPack {
    fn create() -> io::Result<(Self, File)> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "razer-assets-roundtrip-{}-{sequence}.pack",
                std::process::id()
            ));
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok((Self(path), file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "No unique test pack path",
        ))
    }
}

impl Drop for TemporaryPack {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn complete_asset_source_list_and_load_roundtrip_through_pack() {
    let source = SynapseAssets;
    let listed = source.list("").unwrap();
    let loaded = listed
        .iter()
        .map(|key| key.to_string())
        .collect::<BTreeSet<_>>();
    assert!(loaded.len() > 1_000);
    for key in [
        "synapse/audio-volume-external-sprite.svg",
        "synapse/audio-volume-external-sprite.svg#link",
        "synapse/audio-volume-external-sprite.svg#hover",
        "synapse/audio-demo-control-play.svg",
        "synapse/audio-demo-control-fullscreen.svg",
    ] {
        assert!(loaded.contains(key), "Special provider omitted: {key}");
    }
    let inputs = loaded.iter().map(String::as_str).collect::<Vec<_>>();
    let (_temporary, file) = TemporaryPack::create().unwrap();
    let mut writer = BufWriter::new(file);
    let summary = write_pack_from_loader(&mut writer, &inputs, |key| {
        source
            .load(key)
            .map_err(|error| io::Error::other(error.to_string()))?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Listed asset is absent: {key}"),
                )
            })
    })
    .unwrap();
    let file = writer.into_inner().unwrap();
    let mut pack = PackReader::open(BufReader::new(file)).unwrap();
    assert_eq!(summary, pack.summary());
    assert_eq!(
        pack.keys().collect::<Vec<_>>(),
        loaded.iter().map(String::as_str).collect::<Vec<_>>()
    );
    for key in &listed {
        let packed = pack
            .read(key)
            .unwrap()
            .unwrap_or_else(|| panic!("Pack omitted listed alias: {key}"));
        assert_eq!(
            packed,
            source.load(key).unwrap().unwrap().as_ref(),
            "Pack changes source load: {key}"
        );
    }
    let fallback_keys = gpui_kit::assets::AllAssets.list("").unwrap();
    assert!(!fallback_keys.is_empty());
    for key in fallback_keys {
        let original = gpui_kit::assets::AllAssets.load(&key).unwrap().unwrap();
        let selected = source.load(&key).unwrap().unwrap();
        assert_eq!(selected, original, "Fallback byte identity changed: {key}");
        assert_eq!(pack.read(&key).unwrap().unwrap(), selected.as_ref());
    }
    assert_eq!(source.list("").unwrap(), listed);
    assert!(
        pack.read("synapse/absent-resource-for-pack-test")
            .unwrap()
            .is_none()
    );
    println!(
        "asset_pack_roundtrip listed={} unique_keys={} blobs={} payload_bytes={} pack_bytes={}",
        listed.len(),
        summary.entry_count,
        summary.blob_count,
        summary.payload_bytes,
        summary.file_bytes
    );
}

#[test]
fn dynamic_sprite_fragment_selection_remains_distinct_from_raw_source() {
    let base = "synapse/audio-volume-external-sprite.svg";
    let raw = SynapseAssets.load(base).unwrap().unwrap();
    let link = SynapseAssets
        .load(&format!("{base}#link"))
        .unwrap()
        .unwrap();
    let hover = SynapseAssets
        .load(&format!("{base}#hover"))
        .unwrap()
        .unwrap();
    assert_ne!(raw, link);
    assert_ne!(raw, hover);
    assert_ne!(link, hover);
    for (fragment, bytes) in [("link", link), ("hover", hover)] {
        let original = std::str::from_utf8(&raw).unwrap();
        let marker = format!("<view id=\"{fragment}\" viewBox=\"");
        let viewport = original
            .split_once(&marker)
            .unwrap()
            .1
            .split('"')
            .next()
            .unwrap();
        let selected = std::str::from_utf8(&bytes).unwrap();
        assert!(
            selected
                .split_once('>')
                .unwrap()
                .0
                .contains(&format!("viewBox=\"{viewport}\""))
        );
        assert!(selected.contains(&marker));
    }
    let unknown = format!("{base}#unknown");
    assert_eq!(
        SynapseAssets
            .load(&unknown)
            .map_err(|error| error.to_string()),
        gpui_kit::assets::AllAssets
            .load(&unknown)
            .map_err(|error| error.to_string())
    );
}
