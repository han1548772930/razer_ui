use super::*;

fn varint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while n >= 128 {
        out.push(n as u8 | 128);
        n >>= 7;
    }
    out.push(n as u8);
    out
}
fn field(bytes: &[u8]) -> Vec<u8> {
    [varint(bytes.len() as u64), bytes.to_vec()].concat()
}
fn record(bytes: &[u8]) -> Vec<u8> {
    let body = [&[1], bytes].concat();
    [
        masked_crc(&body).to_le_bytes().as_slice(),
        &(bytes.len() as u16).to_le_bytes(),
        &body,
    ]
    .concat()
}
fn batch(sequence: u64, key: &[u8], value: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = [
        sequence.to_le_bytes().as_slice(),
        &1_u32.to_le_bytes(),
        &[u8::from(value.is_some())],
        &field(key),
    ]
    .concat();
    if let Some(value) = value {
        bytes.extend(field(value));
    }
    record(&bytes)
}
struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-leveldb-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn fixture(&self, bytes: &[u8]) {
        fs::write(self.0.join("CURRENT"), b"MANIFEST-000001\n").unwrap();
        let manifest = [
            &[1],
            field(b"leveldb.BytewiseComparator").as_slice(),
            &[2, 2, 3, 3, 4, 0],
        ]
        .concat();
        fs::write(self.0.join("MANIFEST-000001"), record(&manifest)).unwrap();
        fs::write(self.0.join("000002.log"), bytes).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn latest_wal_and_tombstone_do_not_resurrect_old_values_or_other_origins() {
    let directory = Directory::new();
    let mut bytes = batch(3, b"target", Some(b"disabled"));
    bytes.extend(batch(4, b"other-origin", Some(b"enabled")));
    directory.fixture(&bytes);
    assert_eq!(
        read(&directory.0, b"target").unwrap().unwrap().bytes,
        b"disabled"
    );
    bytes.extend(batch(5, b"target", None));
    directory.fixture(&bytes);
    assert!(read(&directory.0, b"target").unwrap().is_none());
}
#[test]
fn corruption_and_concurrent_snapshot_change_are_errors() {
    let directory = Directory::new();
    let bytes = batch(3, b"target", Some(b"disabled"));
    directory.fixture(&bytes);
    assert!(
        read_with(&directory.0, b"target", || {
            fs::write(
                directory.0.join("000002.log"),
                batch(4, b"target", Some(b"enabled")),
            )
            .unwrap();
        })
        .unwrap_err()
        .to_string()
        .contains("changed")
    );
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    directory.fixture(&corrupt);
    assert!(
        read(&directory.0, b"target")
            .unwrap_err()
            .to_string()
            .contains("checksum")
    );
    directory.fixture(&bytes[..bytes.len() - 1]);
    assert!(read(&directory.0, b"target").is_err());
}
#[test]
fn missing_directory_is_absent_but_missing_live_files_and_unknown_tags_fail() {
    let directory = Directory::new();
    assert!(
        read(&directory.0.join("absent"), b"target")
            .unwrap()
            .is_none()
    );
    assert!(read(&directory.0, b"target").is_err());
    assert!(Manifest::parse(&record(&[99])).is_err());
    let mut selected = Selected::default();
    selected.accept(1, Some(b"a")).unwrap();
    assert!(selected.accept(1, Some(b"b")).is_err());
}

fn internal(key: &[u8], sequence: u64) -> Vec<u8> {
    [key, &((sequence << 8) | 1).to_le_bytes()].concat()
}
fn entry(key: &[u8], value: &[u8]) -> Vec<u8> {
    [
        vec![0],
        varint(key.len() as u64),
        varint(value.len() as u64),
        key.to_vec(),
        value.to_vec(),
        vec![0, 0, 0, 0, 1, 0, 0, 0],
    ]
    .concat()
}
fn block(out: &mut Vec<u8>, bytes: &[u8], compressed: bool) -> Vec<u8> {
    let bytes = if compressed {
        snap::raw::Encoder::new().compress_vec(bytes).unwrap()
    } else {
        bytes.to_vec()
    };
    let handle = [varint(out.len() as u64), varint(bytes.len() as u64)].concat();
    let mut payload = bytes;
    payload.push(u8::from(compressed));
    let crc = masked_crc(&payload);
    out.extend(payload);
    out.extend(crc.to_le_bytes());
    handle
}
fn sst(key: &[u8], value: &[u8], compressed: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let data = block(&mut out, &entry(key, value), compressed);
    let meta = block(&mut out, &[0, 0, 0, 0, 1, 0, 0, 0], false);
    let index = block(&mut out, &entry(key, &data), false);
    let mut footer = [meta, index].concat();
    footer.resize(40, 0);
    footer.extend(0xdb4775248b80fb57_u64.to_le_bytes());
    out.extend(footer);
    out
}
fn add_table(directory: &Directory, bytes: &[u8]) {
    let manifest = directory.0.join("MANIFEST-000001");
    let mut previous = fs::read(&manifest).unwrap();
    let key = internal(b"target", 7);
    previous.extend(record(
        &[
            vec![7, 0, 3],
            varint(bytes.len() as u64),
            field(&key),
            field(&key),
        ]
        .concat(),
    ));
    fs::write(manifest, previous).unwrap();
    fs::write(directory.0.join("000003.ldb"), bytes).unwrap();
}
#[test]
fn manifest_live_tables_support_raw_and_snappy_and_wal_supersedes_them() {
    for compressed in [false, true] {
        let directory = Directory::new();
        directory.fixture(&batch(8, b"other-origin", Some(b"enabled")));
        add_table(
            &directory,
            &sst(&internal(b"target", 7), b"disabled", compressed),
        );
        fs::write(
            directory.0.join("000001.ldb"),
            b"obsolete corrupt table ignored by manifest",
        )
        .unwrap();
        let value = read(&directory.0, b"target").unwrap().unwrap();
        assert_eq!(value.sequence, 7);
        assert_eq!(value.bytes, b"disabled");
        fs::write(directory.0.join("000002.log"), batch(9, b"target", None)).unwrap();
        assert!(read(&directory.0, b"target").unwrap().is_none());
    }
}
#[test]
fn table_corruption_short_keys_and_missing_referenced_tables_do_not_mean_absence() {
    let directory = Directory::new();
    directory.fixture(&[]);
    let mut corrupt = sst(&internal(b"target", 7), b"disabled", false);
    corrupt[3] ^= 1;
    add_table(&directory, &corrupt);
    assert!(
        read(&directory.0, b"target")
            .unwrap_err()
            .to_string()
            .contains("checksum")
    );
    directory.fixture(&[]);
    add_table(&directory, &sst(b"short", b"value", false));
    assert!(read(&directory.0, b"target").is_err());
    fs::remove_file(directory.0.join("000003.ldb")).unwrap();
    assert!(read(&directory.0, b"target").is_err());
}
#[test]
fn fragmented_records_require_order_complete_payload_and_checked_crc() {
    fn fragment(kind: u8, bytes: &[u8]) -> Vec<u8> {
        let payload = [&[kind], bytes].concat();
        [
            masked_crc(&payload).to_le_bytes().as_slice(),
            &(bytes.len() as u16).to_le_bytes(),
            &payload,
        ]
        .concat()
    }
    let first = vec![b'x'; BLOCK_SIZE - 7];
    let bytes = [fragment(2, &first), fragment(4, b"tail")].concat();
    let mut seen = 0;
    log_records(&bytes, |record| {
        assert_eq!(record.len(), BLOCK_SIZE - 3);
        assert!(record.ends_with(b"tail"));
        seen += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, 1);
    assert!(log_records(&fragment(4, b"tail"), |_| Ok(())).is_err());
    assert!(log_records(&fragment(2, &first), |_| Ok(())).is_err());
}
