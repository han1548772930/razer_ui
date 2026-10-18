//! Read-only LevelDB recovery for one exact Chromium localStorage key. No lock,
//! log, manifest, repair or database writes are performed. Corruption and a
//! changing snapshot are errors, never evidence that the requested key is absent.
use anyhow::{Context, Result, bail, ensure};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

const FILE_LIMIT: usize = 64 * 1024 * 1024;
const SNAPSHOT_LIMIT: usize = 256 * 1024 * 1024;
const BLOCK_SIZE: usize = 32768;
const MAX_SEQUENCE: u64 = (1 << 56) - 1;

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        ensure!(count <= self.0.len(), "truncated LevelDB record");
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }
    fn varint(&mut self) -> Result<u64> {
        let mut value = 0;
        for shift in (0..=63).step_by(7) {
            let byte = self.take(1)?[0];
            ensure!(shift != 63 || byte <= 1, "overflowing LevelDB varint");
            value |= u64::from(byte & 127) << shift;
            if byte < 128 {
                return Ok(value);
            }
        }
        bail!("overflowing LevelDB varint")
    }
    fn size(&mut self) -> Result<usize> {
        Ok(usize::try_from(self.varint()?)?)
    }
    fn bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.size()?;
        self.take(n)
    }
    fn level(&mut self) -> Result<u64> {
        let level = self.varint()?;
        ensure!(level < 7, "invalid LevelDB table level");
        Ok(level)
    }
}
fn u32le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("four bytes"))
}
fn u64le(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("eight bytes"))
}
fn masked_crc(bytes: &[u8]) -> u32 {
    crc32c::crc32c(bytes)
        .rotate_right(15)
        .wrapping_add(0xa282ead8)
}

fn log_records(bytes: &[u8], mut visit: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
    let mut fragment: Option<Vec<u8>> = None;
    for block in bytes.chunks(BLOCK_SIZE) {
        let mut at = 0;
        while at < block.len() {
            let remaining = &block[at..];
            if remaining.len() < 7 {
                ensure!(
                    block.len() == BLOCK_SIZE && remaining.iter().all(|b| *b == 0),
                    "truncated LevelDB log header"
                );
                break;
            }
            let len = u16::from_le_bytes([remaining[4], remaining[5]]) as usize;
            let kind = remaining[6];
            if kind == 0 && len == 0 && remaining.iter().all(|b| *b == 0) {
                break;
            }
            ensure!(len <= remaining.len() - 7, "truncated LevelDB log payload");
            ensure!(
                masked_crc(&remaining[6..7 + len]) == u32le(&remaining[..4]),
                "LevelDB log checksum mismatch"
            );
            let value = &remaining[7..7 + len];
            match kind {
                1 => {
                    ensure!(fragment.is_none(), "unfinished LevelDB log fragment");
                    visit(value)?;
                }
                2 => {
                    ensure!(fragment.is_none(), "nested LevelDB log fragment");
                    fragment = Some(value.to_vec());
                }
                3 | 4 => {
                    let accumulated = fragment
                        .as_mut()
                        .context("LevelDB continuation without first fragment")?;
                    ensure!(
                        accumulated.len() + value.len() <= FILE_LIMIT,
                        "LevelDB record exceeds limit"
                    );
                    accumulated.extend_from_slice(value);
                    if kind == 4 {
                        visit(&fragment.take().expect("existing fragment"))?;
                    }
                }
                _ => bail!("unknown LevelDB log record type"),
            }
            at += 7 + len;
        }
    }
    ensure!(fragment.is_none(), "unfinished LevelDB log record");
    Ok(())
}

#[derive(Default)]
struct Manifest {
    tables: BTreeMap<(u64, u64), u64>,
    log: Option<u64>,
    previous_log: u64,
    comparator: bool,
}
impl Manifest {
    fn parse(bytes: &[u8]) -> Result<Self> {
        let mut state = Self::default();
        log_records(bytes, |record| {
            let mut cursor = Cursor(record);
            // VersionEdit applies deletions before additions, regardless of
            // the order in which tags were encoded in a record.
            let mut deleted = Vec::new();
            let mut added = Vec::new();
            while !cursor.0.is_empty() {
                match cursor.varint()? {
                    1 => {
                        ensure!(
                            cursor.bytes()? == b"leveldb.BytewiseComparator",
                            "unsupported LevelDB comparator"
                        );
                        state.comparator = true;
                    }
                    2 => state.log = Some(cursor.varint()?),
                    3 => {
                        cursor.varint()?;
                    }
                    4 => {
                        ensure!(cursor.varint()? <= MAX_SEQUENCE, "invalid LevelDB sequence");
                    }
                    5 => {
                        cursor.level()?;
                        internal_key(cursor.bytes()?)?;
                    }
                    6 => {
                        deleted.push((cursor.level()?, cursor.varint()?));
                    }
                    7 => {
                        let key = (cursor.level()?, cursor.varint()?);
                        let size = cursor.varint()?;
                        internal_key(cursor.bytes()?)?;
                        internal_key(cursor.bytes()?)?;
                        added.push((key, size));
                    }
                    9 => state.previous_log = cursor.varint()?,
                    _ => bail!("unsupported LevelDB manifest tag"),
                }
            }
            for key in deleted {
                state.tables.remove(&key);
            }
            for (key, size) in added {
                state.tables.insert(key, size);
            }
            Ok(())
        })?;
        ensure!(
            state.comparator && state.log.is_some(),
            "incomplete LevelDB manifest"
        );
        Ok(state)
    }
}

fn internal_key(key: &[u8]) -> Result<(&[u8], u64, bool)> {
    ensure!(key.len() >= 8, "short LevelDB internal key");
    let trailer = u64le(&key[key.len() - 8..]);
    let kind = trailer & 255;
    ensure!(kind <= 1, "unknown LevelDB internal value type");
    Ok((&key[..key.len() - 8], trailer >> 8, kind == 0))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Value {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}
#[derive(Default)]
struct Selected(Option<(u64, Option<Vec<u8>>)>);
impl Selected {
    fn accept(&mut self, sequence: u64, bytes: Option<&[u8]>) -> Result<()> {
        ensure!(sequence <= MAX_SEQUENCE, "invalid LevelDB sequence");
        if let Some((previous, value)) = &self.0 {
            if sequence == *previous {
                ensure!(value.as_deref() == bytes, "conflicting LevelDB sequence");
                return Ok(());
            }
            if sequence < *previous {
                return Ok(());
            }
        }
        self.0 = Some((sequence, bytes.map(<[u8]>::to_vec)));
        Ok(())
    }
    fn finish(self) -> Option<Value> {
        let (sequence, bytes) = self.0?;
        Some(Value {
            sequence,
            bytes: bytes?,
        })
    }
}

fn table_block(table: &[u8], handle: &[u8], exact: bool) -> Result<Vec<u8>> {
    let mut cursor = Cursor(handle);
    let offset = cursor.size()?;
    let size = cursor.size()?;
    ensure!(
        !exact || cursor.0.is_empty(),
        "trailing LevelDB block handle bytes"
    );
    let end = offset
        .checked_add(size)
        .context("LevelDB block offset overflow")?;
    ensure!(
        end.checked_add(5).is_some_and(|n| n <= table.len() - 48),
        "LevelDB block outside table"
    );
    let block = &table[offset..end];
    let compression = table[end];
    ensure!(
        masked_crc(&table[offset..=end]) == u32le(&table[end + 1..end + 5]),
        "LevelDB table checksum mismatch"
    );
    match compression {
        0 => Ok(block.to_vec()),
        1 => {
            ensure!(
                snap::raw::decompress_len(block)? <= FILE_LIMIT,
                "LevelDB expanded block exceeds limit"
            );
            Ok(snap::raw::Decoder::new().decompress_vec(block)?)
        }
        _ => bail!("unsupported LevelDB table compression"),
    }
}
fn entries(block: &[u8], mut visit: impl FnMut(&[u8], &[u8]) -> Result<()>) -> Result<()> {
    ensure!(block.len() >= 8, "short LevelDB table block");
    let count = u32le(&block[block.len() - 4..]) as usize;
    ensure!(
        count > 0 && count <= (block.len() - 4) / 4,
        "invalid LevelDB restarts"
    );
    let end = block.len() - (count + 1) * 4;
    let restarts: Vec<_> = block[end..block.len() - 4]
        .chunks_exact(4)
        .map(|b| u32le(b) as usize)
        .collect();
    ensure!(
        restarts[0] == 0
            && restarts.windows(2).all(|w| w[0] < w[1])
            && restarts.iter().all(|n| *n < end || end == 0 && *n == 0),
        "invalid LevelDB restart offsets"
    );
    let mut cursor = Cursor(&block[..end]);
    let mut key = Vec::new();
    let mut next_restart = 0;
    while !cursor.0.is_empty() {
        let offset = end - cursor.0.len();
        let shared = cursor.size()?;
        let extra = cursor.size()?;
        let length = cursor.size()?;
        ensure!(shared <= key.len(), "invalid LevelDB key prefix");
        if restarts.get(next_restart) == Some(&offset) {
            ensure!(shared == 0, "LevelDB restart has shared prefix");
            next_restart += 1;
        }
        key.truncate(shared);
        key.extend_from_slice(cursor.take(extra)?);
        visit(&key, cursor.take(length)?)?;
    }
    ensure!(
        next_restart == count || end == 0 && count == 1,
        "LevelDB restart does not point to an entry"
    );
    Ok(())
}
fn table(table: &[u8], wanted: &[u8], selected: &mut Selected) -> Result<()> {
    ensure!(
        table.len() >= 48 && u64le(&table[table.len() - 8..]) == 0xdb4775248b80fb57,
        "invalid LevelDB table footer"
    );
    let mut footer = Cursor(&table[table.len() - 48..table.len() - 8]);
    let meta_handle = footer.0;
    footer.varint()?;
    footer.varint()?;
    let index_handle = footer.0;
    footer.varint()?;
    footer.varint()?;
    ensure!(
        footer.0.iter().all(|b| *b == 0),
        "invalid LevelDB footer padding"
    );
    // Validate referenced metadata too; it is not used to skip target reads.
    entries(&table_block(table, meta_handle, false)?, |_, handle| {
        table_block(table, handle, true)?;
        Ok(())
    })?;
    entries(&table_block(table, index_handle, false)?, |key, handle| {
        internal_key(key)?;
        entries(&table_block(table, handle, true)?, |key, value| {
            let (key, sequence, deleted) = internal_key(key)?;
            if key == wanted {
                selected.accept(sequence, (!deleted).then_some(value))?;
            }
            Ok(())
        })
    })
}
fn wal(bytes: &[u8], wanted: &[u8], selected: &mut Selected) -> Result<()> {
    log_records(bytes, |record| {
        let mut cursor = Cursor(record);
        let sequence = u64le(cursor.take(8)?);
        let count = u32le(cursor.take(4)?);
        ensure!(
            count as usize <= cursor.0.len()
                && sequence <= MAX_SEQUENCE
                && u64::from(count).saturating_sub(1) <= MAX_SEQUENCE - sequence,
            "invalid LevelDB write batch"
        );
        for i in 0..count {
            let kind = cursor.take(1)?[0];
            ensure!(kind <= 1, "unknown LevelDB write batch tag");
            let key = cursor.bytes()?;
            let value = if kind == 1 { Some(cursor.bytes()?) } else { None };
            if key == wanted {
                selected.accept(sequence + u64::from(i), value)?;
            }
        }
        ensure!(cursor.0.is_empty(), "trailing LevelDB write batch bytes");
        Ok(())
    })
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path).with_context(|| {
        format!(
            "read local profile file {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    ensure!(
        file.metadata()?.len() <= FILE_LIMIT as u64,
        "local profile file exceeds size limit"
    );
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= FILE_LIMIT,
        "local profile file grew beyond size limit"
    );
    Ok(bytes)
}
fn logs(root: &Path, manifest: &Manifest) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root)? {
        let name = entry?.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(number) = name
            .strip_suffix(".log")
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u64>().ok())
            && (number >= manifest.log.expect("validated manifest")
                || number == manifest.previous_log)
        {
            result.push(name.to_owned());
        }
    }
    result.sort();
    Ok(result)
}
pub(super) fn read(root: &Path, wanted: &[u8]) -> Result<Option<Value>> {
    read_with(root, wanted, || {})
}
fn read_with(root: &Path, wanted: &[u8], after_capture: impl FnOnce()) -> Result<Option<Value>> {
    match fs::metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => {
            ensure!(
                result?.is_dir(),
                "local profile database is not a directory"
            );
        }
    }
    let current = read_file(&root.join("CURRENT"))?;
    let name = std::str::from_utf8(&current)?
        .strip_suffix('\n')
        .context("invalid LevelDB CURRENT terminator")?;
    ensure!(
        name.strip_prefix("MANIFEST-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        "invalid LevelDB manifest name"
    );
    let manifest_bytes = read_file(&root.join(name))?;
    let manifest = Manifest::parse(&manifest_bytes)?;
    let log_names = logs(root, &manifest)?;
    let mut files = BTreeMap::new();
    files.insert("CURRENT".to_owned(), current.clone());
    files.insert(name.to_owned(), manifest_bytes);
    let mut total = files.values().map(Vec::len).sum::<usize>();
    let mut selected = Selected::default();
    for (_, number) in manifest.tables.keys() {
        let mut filename = format!("{number:06}.ldb");
        if !root.join(&filename).try_exists()? {
            filename = format!("{number:06}.sst");
        }
        let bytes = read_file(&root.join(&filename))?;
        ensure!(
            manifest
                .tables
                .iter()
                .find(|((_, n), _)| n == number)
                .is_some_and(|(_, size)| *size == bytes.len() as u64),
            "LevelDB table size differs from manifest"
        );
        total += bytes.len();
        ensure!(
            total <= SNAPSHOT_LIMIT,
            "local profile snapshot exceeds size limit"
        );
        table(&bytes, wanted, &mut selected)?;
        files.insert(filename, bytes);
    }
    for filename in &log_names {
        let bytes = read_file(&root.join(filename))?;
        total += bytes.len();
        ensure!(
            total <= SNAPSHOT_LIMIT,
            "local profile snapshot exceeds size limit"
        );
        wal(&bytes, wanted, &mut selected)?;
        files.insert(filename.clone(), bytes);
    }
    after_capture();
    for (filename, bytes) in &files {
        ensure!(
            read_file(&root.join(filename))? == *bytes,
            "local profile database changed during read"
        );
    }
    ensure!(
        logs(root, &manifest)? == log_names && read_file(&root.join("CURRENT"))? == current,
        "local profile database changed during read"
    );
    Ok(selected.finish())
}

#[cfg(test)]
#[path = "leveldb_tests.rs"]
mod tests;
