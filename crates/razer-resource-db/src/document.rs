//! Bounded typed snapshots. Constructors are generated in their domain owners.
use anyhow::{Context as _, ensure};
use std::{collections::BTreeMap, ops::Range, sync::OnceLock};

const MAX_DOCUMENT: usize = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 128;

/// Stable identity of a packaged domain catalog; unrelated user state is separate.
#[derive(Clone, Copy)]
pub struct CatalogId(pub(crate) &'static str);

struct Node {
    scalar: Range<usize>,
    children: Range<usize>,
}

struct Document {
    codec: String,
    bytes: Vec<u8>,
    roots: BTreeMap<u32, usize>,
    nodes: Vec<Node>,
}

impl Document {
    fn parse(codec: String, bytes: Vec<u8>) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() <= MAX_DOCUMENT,
            "Typed catalog exceeds size limit"
        );
        ensure!(
            bytes.get(..8) == Some(b"RZTDAG01"),
            "Invalid typed catalog format"
        );
        let mut cursor = 8;
        let count = take_u32(&bytes, &mut cursor)? as usize;
        let root_count = take_u32(&bytes, &mut cursor)? as usize;
        ensure!(
            count > 0 && count <= bytes.len() / 10,
            "Invalid typed catalog node count"
        );
        ensure!(
            root_count > 0 && root_count <= count,
            "Invalid typed catalog roots"
        );
        let mut roots = BTreeMap::new();
        for _ in 0..root_count {
            let id = take_u32(&bytes, &mut cursor)?;
            let node = take_u32(&bytes, &mut cursor)? as usize;
            ensure!(
                node < count && roots.insert(id, node).is_none(),
                "Invalid or duplicate catalog root"
            );
        }
        let mut nodes = Vec::with_capacity(count);
        for id in 0..count {
            let scalar_length = take_u32(&bytes, &mut cursor)? as usize;
            let child_count = take_u32(&bytes, &mut cursor)? as usize;
            ensure!(scalar_length >= 2, "Missing typed catalog alternative");
            let scalar = take_range(&bytes, &mut cursor, scalar_length)?;
            let children = take_range(
                &bytes,
                &mut cursor,
                child_count
                    .checked_mul(4)
                    .context("Catalog child length overflow")?,
            )?;
            for offset in (children.start..children.end).step_by(4) {
                let child = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?) as usize;
                ensure!(child < id, "Catalog child is cyclic or out of bounds");
            }
            nodes.push(Node { scalar, children });
        }
        ensure!(cursor == bytes.len(), "Typed catalog has trailing bytes");
        Ok(Self {
            codec,
            bytes,
            roots,
            nodes,
        })
    }
}

fn take_range(bytes: &[u8], cursor: &mut usize, length: usize) -> anyhow::Result<Range<usize>> {
    let end = cursor
        .checked_add(length)
        .context("Catalog length overflow")?;
    ensure!(end <= bytes.len(), "Typed catalog is truncated");
    let range = *cursor..end;
    *cursor = end;
    Ok(range)
}

fn take_u32(bytes: &[u8], cursor: &mut usize) -> anyhow::Result<u32> {
    Ok(u32::from_le_bytes(
        bytes[take_range(bytes, cursor, 4)?].try_into()?,
    ))
}

static DOCUMENTS: OnceLock<Result<BTreeMap<String, Document>, String>> = OnceLock::new();

/// Prepare immutable encoded catalogs before constructing views. Domain objects
/// remain lazy in their existing OnceLock or per-product owners.
pub fn initialize() -> anyhow::Result<()> {
    documents().map(|_| ())
}

fn documents() -> anyhow::Result<&'static BTreeMap<String, Document>> {
    DOCUMENTS
        .get_or_init(|| {
            super::read_catalog(|connection| {
                use sha2::{Digest as _, Sha256};
                let mut statement = connection
                    .prepare("SELECT id, codec, sha256, payload FROM typed_catalog ORDER BY id")?;
                let mut rows = statement.query([])?;
                let mut documents = BTreeMap::new();
                let mut total = 0usize;
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let codec: String = row.get(1)?;
                    let digest: String = row.get(2)?;
                    let bytes: Vec<u8> = row.get(3)?;
                    total = total
                        .checked_add(bytes.len())
                        .context("Catalog total length overflow")?;
                    ensure!(
                        total <= 128 * 1024 * 1024,
                        "Packaged catalogs exceed total size limit"
                    );
                    ensure!(
                        format!("{:x}", Sha256::digest(&bytes)) == digest,
                        "Typed catalog checksum failed: {id}"
                    );
                    let document = Document::parse(codec, bytes)
                        .with_context(|| format!("Invalid typed catalog {id}"))?;
                    ensure!(
                        documents.insert(id, document).is_none(),
                        "Duplicate typed catalog identity"
                    );
                }
                ensure!(!documents.is_empty(), "Packaged typed catalogs are missing");
                Ok(documents)
            })
            .map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// Decode one requested typed root, without loading another product's state.
pub fn read_document<T>(
    id: CatalogId,
    root: u32,
    codec: &str,
    decode: fn(&mut NodeReader<'_>) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let document = documents()?
        .get(id.0)
        .with_context(|| format!("Missing typed catalog {}", id.0))?;
    ensure!(
        document.codec == codec,
        "Typed catalog schema mismatch: {}",
        id.0
    );
    let node = *document
        .roots
        .get(&root)
        .with_context(|| format!("Missing typed catalog root {}/{root}", id.0))?;
    let mut reader = NodeReader::new(document, node, 0)?;
    let result = decode(&mut reader)
        .with_context(|| format!("Cannot decode typed catalog {}/{root}", id.0))?;
    reader.finish()?;
    Ok(result)
}

/// A single typed constructor node. Generated domain decoders own field meaning.
pub struct NodeReader<'a> {
    document: &'a Document,
    scalar: &'a [u8],
    children: &'a [u8],
    depth: usize,
}

macro_rules! numbers {
    ($($name:ident: $size:expr),* $(,)?) => {$ (
        pub fn $name(&mut self) -> anyhow::Result<$name> {
            Ok($name::from_le_bytes(self.take($size)?.try_into()?))
        }
    )*};
}

impl<'a> NodeReader<'a> {
    fn new(document: &'a Document, node: usize, depth: usize) -> anyhow::Result<Self> {
        ensure!(depth <= MAX_DEPTH, "Typed catalog nesting exceeds limit");
        let node = document
            .nodes
            .get(node)
            .context("Invalid typed catalog child")?;
        Ok(Self {
            document,
            scalar: &document.bytes[node.scalar.clone()],
            children: &document.bytes[node.children.clone()],
            depth,
        })
    }

    fn take(&mut self, length: usize) -> anyhow::Result<&'a [u8]> {
        ensure!(
            length <= self.scalar.len(),
            "Truncated typed catalog scalar"
        );
        let (value, remainder) = self.scalar.split_at(length);
        self.scalar = remainder;
        Ok(value)
    }

    numbers!(u8: 1, u16: 2, u32: 4, u64: 8, i8: 1, i16: 2, i32: 4, i64: 8, f32: 4, f64: 8);

    pub fn usize(&mut self) -> anyhow::Result<usize> {
        Ok(usize::try_from(self.u64()?)?)
    }
    pub fn isize(&mut self) -> anyhow::Result<isize> {
        Ok(isize::try_from(self.i64()?)?)
    }
    pub fn tag(&mut self) -> anyhow::Result<u16> {
        self.u16()
    }
    pub fn boolean(&mut self) -> anyhow::Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => anyhow::bail!("Invalid catalog boolean"),
        }
    }
    pub fn string(&mut self) -> anyhow::Result<String> {
        let length = self.scalar.len();
        Ok(std::str::from_utf8(self.take(length)?)?.to_owned())
    }
    pub fn finish(&self) -> anyhow::Result<()> {
        ensure!(
            self.scalar.is_empty() && self.children.is_empty(),
            "Typed catalog fields were not consumed"
        );
        Ok(())
    }
    pub fn child<T>(
        &mut self,
        decode: fn(&mut NodeReader<'_>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        ensure!(self.children.len() >= 4, "Typed catalog child is missing");
        let child = u32::from_le_bytes(self.children[..4].try_into()?) as usize;
        self.children = &self.children[4..];
        let mut reader = Self::new(self.document, child, self.depth + 1)?;
        let result = decode(&mut reader)?;
        reader.finish()?;
        Ok(result)
    }
    pub fn sequence<T>(
        &mut self,
        decode: fn(&mut NodeReader<'_>) -> anyhow::Result<T>,
    ) -> anyhow::Result<Vec<T>> {
        let mut values = Vec::with_capacity(self.children.len() / 4);
        while !self.children.is_empty() {
            values.push(self.child(decode)?);
        }
        Ok(values)
    }
    pub fn array<T, const N: usize>(
        &mut self,
        decode: fn(&mut NodeReader<'_>) -> anyhow::Result<T>,
    ) -> anyhow::Result<[T; N]> {
        ensure!(
            self.children.len() == N * 4,
            "Typed catalog array length differs"
        );
        self.sequence(decode)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid catalog array"))
    }
    pub fn map<K, V, M: FromIterator<(K, V)>>(
        &mut self,
        key: fn(&mut NodeReader<'_>) -> anyhow::Result<K>,
        value: fn(&mut NodeReader<'_>) -> anyhow::Result<V>,
    ) -> anyhow::Result<M> {
        ensure!(
            self.children.len().is_multiple_of(8),
            "Typed catalog map entry is incomplete"
        );
        let mut values = Vec::with_capacity(self.children.len() / 8);
        while !self.children.is_empty() {
            values.push((self.child(key)?, self.child(value)?));
        }
        Ok(values.into_iter().collect())
    }
    pub fn external_value(&mut self) -> anyhow::Result<serde_json::Value> {
        self.external_value_at(0)
    }
    fn external_value_at(&mut self, depth: usize) -> anyhow::Result<serde_json::Value> {
        use serde_json::Value;
        ensure!(
            depth <= MAX_DEPTH,
            "External catalog value nesting exceeds limit"
        );
        Ok(match self.u8()? {
            0 => Value::Null,
            1 => Value::Bool(self.boolean()?),
            2 => Value::Number(self.i64()?.into()),
            3 => Value::Number(self.u64()?.into()),
            4 => Value::Number(
                serde_json::Number::from_f64(self.f64()?).context("Non-finite catalog value")?,
            ),
            5 => {
                let length = self.u32()? as usize;
                Value::String(std::str::from_utf8(self.take(length)?)?.to_owned())
            }
            tag @ (6 | 7) => {
                let length = self.u32()? as usize;
                ensure!(
                    length <= self.scalar.len(),
                    "External catalog length exceeds remaining bytes"
                );
                if tag == 6 {
                    let mut values = Vec::with_capacity(length);
                    for _ in 0..length {
                        values.push(self.external_value_at(depth + 1)?);
                    }
                    Value::Array(values)
                } else {
                    let mut values = serde_json::Map::new();
                    for _ in 0..length {
                        let Value::String(key) = self.external_value_at(depth + 1)? else {
                            anyhow::bail!("Catalog object key is not text")
                        };
                        ensure!(!values.contains_key(&key), "Duplicate catalog object key");
                        values.insert(key, self.external_value_at(depth + 1)?);
                    }
                    Value::Object(values)
                }
            }
            _ => anyhow::bail!("Invalid external catalog value tag"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let mut bytes = b"RZTDAG01".to_vec();
        for value in [2u32, 1, 70, 1, 3, 0] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([0u8, 0, 1]);
        for value in [2u32, 2] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([0u8, 0]);
        for value in [0u32, 0] {
            bytes.extend(value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn shared_children_decode_in_order_and_field_consumption_is_checked() {
        let document = Document::parse(String::new(), fixture()).unwrap();
        fn boolean(r: &mut NodeReader<'_>) -> anyhow::Result<bool> {
            assert_eq!(r.tag()?, 0);
            r.boolean()
        }
        let mut reader = NodeReader::new(&document, document.roots[&70], 0).unwrap();
        assert_eq!(reader.tag().unwrap(), 0);
        assert!(reader.finish().is_err());
        assert_eq!(reader.sequence(boolean).unwrap(), [true, true]);
        reader.finish().unwrap();
    }

    #[test]
    fn truncation_cycles_forward_references_and_trailing_bytes_fail() {
        let original = fixture();
        for end in 0..original.len() {
            assert!(Document::parse(String::new(), original[..end].to_vec()).is_err());
        }
        let mut cyclic = original.clone();
        let end = cyclic.len();
        cyclic[end - 4..].copy_from_slice(&1u32.to_le_bytes());
        assert!(Document::parse(String::new(), cyclic).is_err());
        let mut trailing = original;
        trailing.push(0);
        assert!(Document::parse(String::new(), trailing).is_err());
    }
}
