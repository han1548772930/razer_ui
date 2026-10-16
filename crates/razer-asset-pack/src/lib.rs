#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{self, Read, Seek, SeekFrom, Write},
};

pub const FORMAT_VERSION: u32 = 1;
pub const HEADER_BYTES: usize = 96;
const MAGIC: &[u8; 8] = b"RZASPACK";
const BLOB_RECORD_BYTES: u64 = 64;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_entries: u64,
    pub max_blobs: u64,
    pub max_key_bytes: u32,
    pub max_index_bytes: u64,
    pub max_entry_bytes: u64,
    pub max_file_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 1_000_000,
            max_blobs: 1_000_000,
            max_key_bytes: 4096,
            max_index_bytes: 64 * 1024 * 1024,
            max_entry_bytes: 256 * 1024 * 1024,
            max_file_bytes: 16 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PackInput<'a> {
    pub key: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackSummary {
    pub entry_count: u64,
    pub blob_count: u64,
    pub index_bytes: u64,
    pub payload_bytes: u64,
    pub file_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Store,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackEntry<'a> {
    pub key: &'a str,
    pub blob_index: u32,
    pub codec: Codec,
    pub offset: u64,
    pub stored_bytes: u64,
    pub decoded_bytes: u64,
    pub sha256: [u8; 32],
}

#[derive(Debug)]
struct KeyEntry {
    key: String,
    blob_index: u32,
}

#[derive(Clone, Copy, Debug)]
struct Blob {
    offset: u64,
    length: u64,
    sha256: [u8; 32],
}

pub struct PackReader<Source> {
    source: Source,
    entries: Vec<KeyEntry>,
    blobs: Vec<Blob>,
    summary: PackSummary,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn checked_add(left: u64, right: u64) -> io::Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| invalid("Pack length or offset overflow"))
}

fn checked_multiply(left: u64, right: u64) -> io::Result<u64> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("Pack index length overflow"))
}

fn bounded(value: u64, maximum: u64, name: &str) -> io::Result<()> {
    if value > maximum {
        return Err(invalid(format!(
            "Pack {name} exceeds limit: {value} > {maximum}"
        )));
    }
    Ok(())
}

fn vector_with_capacity<Value>(count: u64) -> io::Result<Vec<Value>> {
    let count = usize::try_from(count).map_err(|_| invalid("Pack count exceeds address space"))?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|error| invalid(format!("Pack allocation failed: {error}")))?;
    Ok(values)
}

fn zeroed_bytes(count: u64) -> io::Result<Vec<u8>> {
    let mut bytes = vector_with_capacity(count)?;
    bytes.resize(
        usize::try_from(count).map_err(|_| invalid("Pack length exceeds address space"))?,
        0,
    );
    Ok(bytes)
}

fn validate_key(key: &str, limits: Limits) -> io::Result<()> {
    bounded(
        key.len() as u64,
        u64::from(limits.max_key_bytes),
        "key length",
    )?;
    if key.is_empty()
        || key
            .chars()
            .any(|character| character.is_control() || matches!(character, '\\' | ':'))
        || key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid(format!(
            "Pack key is not a canonical relative asset path: {key:?}"
        )));
    }
    Ok(())
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> io::Result<&'a [u8]> {
    if count > bytes.len() {
        return Err(invalid("Truncated pack index record"));
    }
    let (value, remaining) = bytes.split_at(count);
    *bytes = remaining;
    Ok(value)
}

fn read_u32(bytes: &mut &[u8]) -> io::Result<u32> {
    Ok(u32::from_le_bytes(
        take(bytes, 4)?
            .try_into()
            .map_err(|_| invalid("Invalid u32"))?,
    ))
}

fn read_u64(bytes: &mut &[u8]) -> io::Result<u64> {
    Ok(u64::from_le_bytes(
        take(bytes, 8)?
            .try_into()
            .map_err(|_| invalid("Invalid u64"))?,
    ))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

impl<Source: Read + Seek> PackReader<Source> {
    pub fn open(source: Source) -> io::Result<Self> {
        Self::with_limits(source, Limits::default())
    }

    pub fn with_limits(mut source: Source, limits: Limits) -> io::Result<Self> {
        let file_bytes = source.seek(SeekFrom::End(0))?;
        bounded(file_bytes, limits.max_file_bytes, "file length")?;
        if file_bytes < HEADER_BYTES as u64 {
            return Err(invalid("Truncated pack header"));
        }
        source.seek(SeekFrom::Start(0))?;
        let mut header = [0; HEADER_BYTES];
        source.read_exact(&mut header)?;
        let mut fields = header.as_slice();
        if take(&mut fields, 8)? != MAGIC {
            return Err(invalid("Unknown pack magic"));
        }
        let version = read_u32(&mut fields)?;
        if version != FORMAT_VERSION {
            return Err(invalid(format!("Unsupported pack version {version}")));
        }
        if read_u32(&mut fields)? != 0 {
            return Err(invalid("Unsupported pack header flags"));
        }
        let entry_count = read_u64(&mut fields)?;
        let blob_count = read_u64(&mut fields)?;
        let index_bytes = read_u64(&mut fields)?;
        let data_offset = read_u64(&mut fields)?;
        let declared_file_bytes = read_u64(&mut fields)?;
        let index_sha256 = take(&mut fields, 32)?;
        if read_u64(&mut fields)? != 0 {
            return Err(invalid("Unsupported pack reserved header fields"));
        }
        bounded(entry_count, limits.max_entries, "entry count")?;
        bounded(
            blob_count,
            limits.max_blobs.min(u64::from(u32::MAX)),
            "blob count",
        )?;
        bounded(index_bytes, limits.max_index_bytes, "index length")?;
        if (entry_count == 0) != (blob_count == 0) || blob_count > entry_count {
            return Err(invalid("Pack entry and blob counts are inconsistent"));
        }
        if data_offset != checked_add(HEADER_BYTES as u64, index_bytes)?
            || data_offset > file_bytes
            || declared_file_bytes != file_bytes
        {
            return Err(invalid(
                "Pack file length or data offset differs from header",
            ));
        }
        let minimum_index = checked_add(
            checked_multiply(entry_count, 9)?,
            checked_multiply(blob_count, BLOB_RECORD_BYTES)?,
        )?;
        if index_bytes < minimum_index {
            return Err(invalid("Pack index is too short for declared counts"));
        }
        let mut index = zeroed_bytes(index_bytes)?;
        source.read_exact(&mut index)?;
        if digest(&index).as_slice() != index_sha256 {
            return Err(invalid("Pack index SHA-256 mismatch"));
        }
        let mut remaining = index.as_slice();
        let mut entries: Vec<KeyEntry> = vector_with_capacity(entry_count)?;
        let mut used_blobs = zeroed_bytes(blob_count)?;
        for _ in 0..entry_count {
            let key_length = read_u32(&mut remaining)?;
            bounded(
                u64::from(key_length),
                u64::from(limits.max_key_bytes),
                "key length",
            )?;
            let key = std::str::from_utf8(take(&mut remaining, key_length as usize)?)
                .map_err(|error| invalid(format!("Pack key is not UTF-8: {error}")))?;
            validate_key(key, limits)?;
            if let Some(previous) = entries.last() {
                if previous.key == key {
                    return Err(invalid(format!("Duplicate pack key: {key}")));
                }
                if previous.key.as_str() > key {
                    return Err(invalid("Pack keys are not in canonical UTF-8 byte order"));
                }
            }
            let blob_index = read_u32(&mut remaining)?;
            let used = used_blobs
                .get_mut(blob_index as usize)
                .ok_or_else(|| invalid("Pack key refers to an absent blob"))?;
            *used = 1;
            entries.push(KeyEntry {
                key: key.to_owned(),
                blob_index,
            });
        }
        let mut blobs: Vec<Blob> = vector_with_capacity(blob_count)?;
        let mut next_offset = data_offset;
        for _ in 0..blob_count {
            let codec = read_u32(&mut remaining)?;
            if codec != 0 {
                return Err(invalid(format!("Unsupported pack codec {codec}")));
            }
            if read_u32(&mut remaining)? != 0 {
                return Err(invalid("Unsupported pack blob flags"));
            }
            let offset = read_u64(&mut remaining)?;
            let length = read_u64(&mut remaining)?;
            let decoded_length = read_u64(&mut remaining)?;
            let sha256: [u8; 32] = take(&mut remaining, 32)?
                .try_into()
                .map_err(|_| invalid("Invalid pack blob hash length"))?;
            bounded(length, limits.max_entry_bytes, "entry length")?;
            let end = checked_add(offset, length)?;
            if offset != next_offset || end > file_bytes || decoded_length != length {
                return Err(invalid(
                    "Pack blob length, offset, overlap or store size is invalid",
                ));
            }
            if blobs
                .last()
                .is_some_and(|previous| previous.sha256 >= sha256)
            {
                return Err(invalid(
                    "Pack blob hashes are duplicate or not in canonical byte order",
                ));
            }
            blobs.push(Blob {
                offset,
                length,
                sha256,
            });
            next_offset = end;
        }
        if !remaining.is_empty() || next_offset != file_bytes || used_blobs.contains(&0) {
            return Err(invalid(
                "Pack contains trailing index/data bytes or unreferenced blobs",
            ));
        }
        Ok(Self {
            source,
            entries,
            blobs,
            summary: PackSummary {
                entry_count,
                blob_count,
                index_bytes,
                payload_bytes: file_bytes - data_offset,
                file_bytes,
            },
        })
    }

    pub fn summary(&self) -> PackSummary {
        self.summary
    }

    pub fn keys(&self) -> impl ExactSizeIterator<Item = &str> {
        self.entries.iter().map(|entry| entry.key.as_str())
    }

    pub fn list<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.keys().filter(move |key| key.starts_with(prefix))
    }

    pub fn entry(&self, key: &str) -> Option<PackEntry<'_>> {
        let position = self
            .entries
            .binary_search_by(|entry| entry.key.as_str().cmp(key))
            .ok()?;
        let entry = &self.entries[position];
        let blob = self.blobs[entry.blob_index as usize];
        Some(PackEntry {
            key: &entry.key,
            blob_index: entry.blob_index,
            codec: Codec::Store,
            offset: blob.offset,
            stored_bytes: blob.length,
            decoded_bytes: blob.length,
            sha256: blob.sha256,
        })
    }

    pub fn read(&mut self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let Some(entry) = self.entry(key) else {
            return Ok(None);
        };
        let blob_index = entry.blob_index;
        self.read_blob(blob_index as usize).map(Some)
    }

    fn read_blob(&mut self, blob_index: usize) -> io::Result<Vec<u8>> {
        let blob = self.blobs[blob_index];
        self.source.seek(SeekFrom::Start(blob.offset))?;
        let mut bytes = zeroed_bytes(blob.length)?;
        self.source.read_exact(&mut bytes)?;
        if digest(&bytes) != blob.sha256 {
            return Err(invalid(format!(
                "Pack payload SHA-256 mismatch for blob {blob_index}"
            )));
        }
        Ok(bytes)
    }

    pub fn verify_all(&mut self) -> io::Result<PackSummary> {
        for blob_index in 0..self.blobs.len() {
            self.read_blob(blob_index)?;
        }
        Ok(self.summary)
    }

    pub fn into_inner(self) -> Source {
        self.source
    }
}

pub fn write_pack(writer: &mut impl Write, inputs: &[PackInput<'_>]) -> io::Result<PackSummary> {
    write_pack_with_limits(writer, inputs, Limits::default())
}

pub fn write_pack_with_limits(
    writer: &mut impl Write,
    inputs: &[PackInput<'_>],
    limits: Limits,
) -> io::Result<PackSummary> {
    bounded(inputs.len() as u64, limits.max_entries, "entry count")?;
    let keys = inputs.iter().map(|entry| entry.key).collect::<Vec<_>>();
    let source = inputs
        .iter()
        .map(|entry| (entry.key, entry.bytes))
        .collect::<BTreeMap<_, _>>();
    write_pack_from_loader_with_limits(
        writer,
        &keys,
        |key| {
            source
                .get(key)
                .copied()
                .ok_or_else(|| invalid("Pack input key disappeared"))
        },
        limits,
    )
}

pub fn write_pack_from_loader<Bytes: AsRef<[u8]>>(
    writer: &mut impl Write,
    keys: &[&str],
    load: impl FnMut(&str) -> io::Result<Bytes>,
) -> io::Result<PackSummary> {
    write_pack_from_loader_with_limits(writer, keys, load, Limits::default())
}

pub fn write_pack_from_loader_with_limits<Bytes: AsRef<[u8]>>(
    writer: &mut impl Write,
    keys: &[&str],
    mut load: impl FnMut(&str) -> io::Result<Bytes>,
    limits: Limits,
) -> io::Result<PackSummary> {
    let entry_count = keys.len() as u64;
    bounded(entry_count, limits.max_entries, "entry count")?;
    let mut ordered: Vec<&str> = vector_with_capacity(entry_count)?;
    ordered.extend(keys.iter().copied());
    ordered.sort_unstable();
    let mut unique: BTreeMap<[u8; 32], (u64, &str)> = BTreeMap::new();
    let mut references = vector_with_capacity(entry_count)?;
    let mut key_index_bytes = 0;
    let mut previous_key = None;
    for key in ordered {
        validate_key(key, limits)?;
        if previous_key == Some(key) {
            return Err(invalid(format!("Duplicate pack key: {key}")));
        }
        previous_key = Some(key);
        let loaded = load(key)?;
        let bytes = loaded.as_ref();
        bounded(bytes.len() as u64, limits.max_entry_bytes, "entry length")?;
        let sha256 = digest(bytes);
        if let Some((length, previous)) = unique.get(&sha256) {
            if *length != bytes.len() as u64 || load(previous)?.as_ref() != bytes {
                return Err(invalid("Pack content hash collision or changed source"));
            }
        } else {
            unique.insert(sha256, (bytes.len() as u64, key));
        }
        key_index_bytes = checked_add(key_index_bytes, checked_add(8, key.len() as u64)?)?;
        references.push((key, sha256));
    }
    let blob_count = unique.len() as u64;
    bounded(
        blob_count,
        limits.max_blobs.min(u64::from(u32::MAX)),
        "blob count",
    )?;
    let index_bytes = checked_add(
        key_index_bytes,
        checked_multiply(blob_count, BLOB_RECORD_BYTES)?,
    )?;
    bounded(index_bytes, limits.max_index_bytes, "index length")?;
    let data_offset = checked_add(HEADER_BYTES as u64, index_bytes)?;
    let mut next_offset = data_offset;
    let mut blobs = vector_with_capacity(blob_count)?;
    let mut identifiers = BTreeMap::new();
    for (sha256, (length, _)) in &unique {
        let blob_index =
            u32::try_from(blobs.len()).map_err(|_| invalid("Pack blob count overflow"))?;
        identifiers.insert(*sha256, blob_index);
        blobs.push(Blob {
            offset: next_offset,
            length: *length,
            sha256: *sha256,
        });
        next_offset = checked_add(next_offset, *length)?;
    }
    bounded(next_offset, limits.max_file_bytes, "file length")?;
    let mut index = vector_with_capacity(index_bytes)?;
    for (key, sha256) in references {
        let key_length =
            u32::try_from(key.len()).map_err(|_| invalid("Pack key length overflow"))?;
        index.extend_from_slice(&key_length.to_le_bytes());
        index.extend_from_slice(key.as_bytes());
        index.extend_from_slice(&identifiers[&sha256].to_le_bytes());
    }
    for blob in blobs {
        index.extend_from_slice(&0u32.to_le_bytes());
        index.extend_from_slice(&0u32.to_le_bytes());
        index.extend_from_slice(&blob.offset.to_le_bytes());
        index.extend_from_slice(&blob.length.to_le_bytes());
        index.extend_from_slice(&blob.length.to_le_bytes());
        index.extend_from_slice(&blob.sha256);
    }
    let mut header = Vec::with_capacity(HEADER_BYTES);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&entry_count.to_le_bytes());
    header.extend_from_slice(&blob_count.to_le_bytes());
    header.extend_from_slice(&index_bytes.to_le_bytes());
    header.extend_from_slice(&data_offset.to_le_bytes());
    header.extend_from_slice(&next_offset.to_le_bytes());
    header.extend_from_slice(&digest(&index));
    header.extend_from_slice(&0u64.to_le_bytes());
    writer.write_all(&header)?;
    writer.write_all(&index)?;
    for (sha256, (length, key)) in &unique {
        let loaded = load(key)?;
        let bytes = loaded.as_ref();
        if bytes.len() as u64 != *length || digest(bytes) != *sha256 {
            return Err(invalid(format!("Pack source changed while writing: {key}")));
        }
        writer.write_all(bytes)?;
    }
    writer.flush()?;
    Ok(PackSummary {
        entry_count,
        blob_count,
        index_bytes,
        payload_bytes: next_offset - data_offset,
        file_bytes: next_offset,
    })
}

#[cfg(test)]
mod tests;
