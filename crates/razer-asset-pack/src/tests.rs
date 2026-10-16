use super::*;
use std::{cell::RefCell, io::Cursor, rc::Rc};

fn sample() -> Vec<u8> {
    let mut bytes = Vec::new();
    write_pack(
        &mut bytes,
        &[
            PackInput {
                key: "b.png",
                bytes: b"xyz",
            },
            PackInput {
                key: "a.svg",
                bytes: b"abc",
            },
            PackInput {
                key: "alias.svg",
                bytes: b"abc",
            },
        ],
    )
    .unwrap();
    bytes
}

fn set_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn index_length(bytes: &[u8]) -> usize {
    u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize
}

fn refresh_index_hash(bytes: &mut [u8]) {
    let index_end = HEADER_BYTES + index_length(bytes);
    let hash = digest(&bytes[HEADER_BYTES..index_end]);
    bytes[56..88].copy_from_slice(&hash);
}

fn blob_table(bytes: &[u8]) -> usize {
    let count = u64::from_le_bytes(bytes[24..32].try_into().unwrap()) as usize;
    HEADER_BYTES + index_length(bytes) - count * BLOB_RECORD_BYTES as usize
}

fn error(bytes: Vec<u8>) -> String {
    match PackReader::open(Cursor::new(bytes)) {
        Ok(_) => panic!("Malformed archive unexpectedly accepted"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn deterministic_index_aliases_and_content_deduplication() {
    let bytes = sample();
    let mut reordered = Vec::new();
    write_pack(
        &mut reordered,
        &[
            PackInput {
                key: "alias.svg",
                bytes: b"abc",
            },
            PackInput {
                key: "b.png",
                bytes: b"xyz",
            },
            PackInput {
                key: "a.svg",
                bytes: b"abc",
            },
        ],
    )
    .unwrap();
    assert_eq!(bytes, reordered);
    let mut reader = PackReader::open(Cursor::new(bytes)).unwrap();
    assert_eq!(
        reader.keys().collect::<Vec<_>>(),
        ["a.svg", "alias.svg", "b.png"]
    );
    assert_eq!(reader.list("a").collect::<Vec<_>>(), ["a.svg", "alias.svg"]);
    assert_eq!(
        reader.entry("a.svg").unwrap().blob_index,
        reader.entry("alias.svg").unwrap().blob_index
    );
    assert_eq!(
        reader.entry("a.svg").unwrap().offset,
        reader.entry("alias.svg").unwrap().offset
    );
    assert_eq!(reader.read("a.svg").unwrap().unwrap(), b"abc");
    assert_eq!(reader.read("b.png").unwrap().unwrap(), b"xyz");
    assert_eq!(reader.read("alias.svg").unwrap().unwrap(), b"abc");
    assert_eq!(reader.read("missing").unwrap(), None);
    assert_eq!(reader.summary().entry_count, 3);
    assert_eq!(reader.summary().blob_count, 2);
    assert_eq!(reader.summary().payload_bytes, 6);
    assert_eq!(reader.verify_all().unwrap(), reader.summary());
}

#[test]
fn empty_archive_and_empty_content_round_trip() {
    let mut empty = Vec::new();
    write_pack(&mut empty, &[]).unwrap();
    assert_eq!(empty.len(), HEADER_BYTES);
    let mut reader = PackReader::open(Cursor::new(empty)).unwrap();
    assert_eq!(reader.verify_all().unwrap().blob_count, 0);
    assert_eq!(reader.keys().len(), 0);
    let mut bytes = Vec::new();
    write_pack(
        &mut bytes,
        &[
            PackInput {
                key: "empty-a",
                bytes: b"",
            },
            PackInput {
                key: "empty-b",
                bytes: b"",
            },
        ],
    )
    .unwrap();
    let mut reader = PackReader::open(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.summary().blob_count, 1);
    assert_eq!(reader.read("empty-b").unwrap(), Some(vec![]));
}

#[test]
fn duplicate_keys_fail_before_writing_even_for_identical_content() {
    for second in [b"same".as_slice(), b"different".as_slice()] {
        let mut output = Vec::new();
        let error = write_pack(
            &mut output,
            &[
                PackInput {
                    key: "same-key",
                    bytes: b"same",
                },
                PackInput {
                    key: "same-key",
                    bytes: second,
                },
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("Duplicate"));
        assert!(output.is_empty());
    }
}

#[test]
fn noncanonical_and_absolute_paths_fail_without_normalization() {
    for key in [
        "",
        "/absolute",
        "a//b",
        "a/",
        ".",
        "..",
        "a/../b",
        "a/./b",
        "a\\b",
        "C:/b",
        "a\0b",
        "a\nb",
    ] {
        let mut output = Vec::new();
        assert!(
            write_pack(&mut output, &[PackInput { key, bytes: b"a" }]).is_err(),
            "{key:?}"
        );
        assert!(output.is_empty());
    }
    let mut output = Vec::new();
    write_pack(
        &mut output,
        &[PackInput {
            key: "字体/é font.ttf",
            bytes: b"a",
        }],
    )
    .unwrap();
    let mut reader = PackReader::open(Cursor::new(output)).unwrap();
    assert_eq!(reader.read("字体/é font.ttf").unwrap(), Some(vec![b'a']));
    assert_eq!(reader.read("字体/e\u{301} font.ttf").unwrap(), None);
}

#[test]
fn every_truncation_and_single_byte_mutation_is_detected() {
    let bytes = sample();
    for end in 0..bytes.len() {
        assert!(
            PackReader::open(Cursor::new(bytes[..end].to_vec())).is_err(),
            "truncation {end}"
        );
    }
    for position in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[position] ^= 1;
        if let Ok(mut reader) = PackReader::open(Cursor::new(mutated)) {
            assert!(reader.verify_all().is_err(), "mutation {position}");
        }
    }
    let mut appended = bytes;
    appended.push(0);
    assert!(error(appended).contains("length"));
}

#[test]
fn unsupported_version_flags_and_codecs_are_explicit() {
    let mut bytes = sample();
    bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
    assert!(error(bytes).contains("version 2"));
    for offset in [12, 88] {
        let mut bytes = sample();
        bytes[offset] = 1;
        assert!(error(bytes).contains("Unsupported"));
    }
    let mut bytes = sample();
    let start = blob_table(&bytes);
    bytes[start..start + 4].copy_from_slice(&7u32.to_le_bytes());
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("codec 7"));
    let mut bytes = sample();
    let start = blob_table(&bytes);
    bytes[start + 4] = 1;
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("blob flags"));
}

#[test]
fn duplicate_keys_bad_utf8_and_unsorted_keys_fail_after_hash_validation() {
    let mut bytes = Vec::new();
    write_pack(
        &mut bytes,
        &[
            PackInput {
                key: "a",
                bytes: b"a",
            },
            PackInput {
                key: "b",
                bytes: b"b",
            },
        ],
    )
    .unwrap();
    let mut duplicate = bytes.clone();
    duplicate[HEADER_BYTES + 9 + 4] = b'a';
    refresh_index_hash(&mut duplicate);
    assert!(error(duplicate).contains("Duplicate"));
    let mut unsorted = bytes.clone();
    unsorted[HEADER_BYTES + 4] = b'z';
    refresh_index_hash(&mut unsorted);
    assert!(error(unsorted).contains("canonical"));
    bytes[HEADER_BYTES + 4] = 255;
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("UTF-8"));
}

#[test]
fn invalid_blob_references_and_unreferenced_content_are_rejected() {
    let mut bytes = sample();
    bytes[HEADER_BYTES + 4 + 5..HEADER_BYTES + 4 + 5 + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("absent blob"));
    let mut bytes = sample();
    for position in [
        HEADER_BYTES + 9,
        HEADER_BYTES + 13 + 13,
        HEADER_BYTES + 13 + 17 + 9,
    ] {
        bytes[position..position + 4].copy_from_slice(&0u32.to_le_bytes());
    }
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("unreferenced"));
}

#[test]
fn overlapping_gapped_mismatched_and_overflowing_blobs_fail() {
    for (field, value) in [(8, 0), (8, u64::MAX), (16, u64::MAX), (24, 0)] {
        let mut bytes = sample();
        let start = blob_table(&bytes);
        set_u64(&mut bytes, start + field, value);
        refresh_index_hash(&mut bytes);
        assert!(PackReader::open(Cursor::new(bytes)).is_err());
    }
    let mut bytes = sample();
    let start = blob_table(&bytes);
    let first_offset = u64::from_le_bytes(bytes[start + 8..start + 16].try_into().unwrap());
    set_u64(
        &mut bytes,
        start + BLOB_RECORD_BYTES as usize + 8,
        first_offset,
    );
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("overlap"));
    let mut bytes = sample();
    let start = blob_table(&bytes);
    let hash = bytes[start + 32..start + 64].to_vec();
    bytes[start + 96..start + 128].copy_from_slice(&hash);
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("hashes are duplicate"));
}

#[test]
fn untrusted_header_lengths_do_not_overflow_or_allocate() {
    let limits = Limits {
        max_entries: u64::MAX,
        max_blobs: u64::MAX,
        max_index_bytes: u64::MAX,
        max_file_bytes: u64::MAX,
        ..Limits::default()
    };
    for (field, value) in [
        (16, u64::MAX),
        (24, u64::MAX),
        (32, u64::MAX),
        (40, u64::MAX),
        (48, u64::MAX),
    ] {
        let mut bytes = sample();
        set_u64(&mut bytes, field, value);
        assert!(PackReader::with_limits(Cursor::new(bytes), limits).is_err());
    }
    assert!(checked_add(u64::MAX, 1).is_err());
    assert!(checked_multiply(u64::MAX, 64).is_err());
}

#[test]
fn configurable_resource_limits_apply_to_reader_and_writer() {
    let cases = [
        Limits {
            max_entries: 0,
            ..Limits::default()
        },
        Limits {
            max_blobs: 0,
            ..Limits::default()
        },
        Limits {
            max_key_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_index_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_entry_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_file_bytes: 0,
            ..Limits::default()
        },
    ];
    for limits in cases {
        assert!(PackReader::with_limits(Cursor::new(sample()), limits).is_err());
        let mut output = Vec::new();
        assert!(
            write_pack_with_limits(
                &mut output,
                &[PackInput {
                    key: "key",
                    bytes: b"data"
                }],
                limits
            )
            .is_err()
        );
        assert!(output.is_empty());
    }
}

struct Tracked {
    cursor: Cursor<Vec<u8>>,
    reads: Rc<RefCell<Vec<(u64, usize)>>>,
}

impl Read for Tracked {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let start = self.cursor.position();
        let count = self.cursor.read(buffer)?;
        self.reads.borrow_mut().push((start, count));
        Ok(count)
    }
}

impl Seek for Tracked {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(position)
    }
}

#[test]
fn opening_does_not_read_payload_and_loading_reads_only_requested_blob() {
    let bytes = sample();
    let payload_offset = (HEADER_BYTES + index_length(&bytes)) as u64;
    let reads = Rc::new(RefCell::new(Vec::new()));
    let mut reader = PackReader::open(Tracked {
        cursor: Cursor::new(bytes),
        reads: reads.clone(),
    })
    .unwrap();
    assert!(
        reads
            .borrow()
            .iter()
            .all(|(start, count)| start + *count as u64 <= payload_offset)
    );
    reads.borrow_mut().clear();
    let entry = reader.entry("a.svg").unwrap();
    let expected = (entry.offset, entry.stored_bytes as usize);
    reader.read("a.svg").unwrap();
    assert_eq!(*reads.borrow(), [expected]);
    reads.borrow_mut().clear();
    assert_eq!(reader.read("missing").unwrap(), None);
    assert!(reads.borrow().is_empty());
    reader.verify_all().unwrap();
    assert_eq!(
        reads.borrow().iter().map(|(_, count)| count).sum::<usize>(),
        6
    );
}

#[test]
fn payload_hash_validation_is_lazy_and_does_not_return_corrupt_bytes() {
    let mut bytes = sample();
    let position = PackReader::open(Cursor::new(bytes.clone()))
        .unwrap()
        .entry("a.svg")
        .unwrap()
        .offset as usize;
    bytes[position] ^= 32;
    let mut reader = PackReader::open(Cursor::new(bytes)).unwrap();
    assert_eq!(reader.read("b.png").unwrap(), Some(b"xyz".to_vec()));
    assert!(
        reader
            .read("a.svg")
            .unwrap_err()
            .to_string()
            .contains("SHA-256")
    );
    assert!(reader.read("alias.svg").is_err());
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "simulated write failure",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn writer_propagates_output_failure() {
    assert!(write_pack(&mut FailingWriter, &[]).is_err());
}

#[test]
fn payload_truncated_after_index_open_is_an_error_not_an_empty_asset() {
    let mut reader = PackReader::open(Cursor::new(sample())).unwrap();
    let offset = reader.entry("a.svg").unwrap().offset as usize;
    reader.source.get_mut().truncate(offset + 1);
    assert_eq!(
        reader.read("a.svg").unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn repaired_index_hash_does_not_allow_path_traversal_or_trailing_records() {
    let mut bytes = sample();
    bytes[HEADER_BYTES + 4..HEADER_BYTES + 9].copy_from_slice(b"../aa");
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("canonical relative"));
    let mut bytes = sample();
    let index_bytes = index_length(&bytes);
    let start = blob_table(&bytes);
    let insertion = HEADER_BYTES + index_bytes;
    bytes.insert(insertion, 0);
    for blob in 0..2 {
        let offset = start + blob * BLOB_RECORD_BYTES as usize + 8;
        let old = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        set_u64(&mut bytes, offset, old + 1);
    }
    set_u64(&mut bytes, 32, index_bytes as u64 + 1);
    set_u64(&mut bytes, 40, insertion as u64 + 1);
    let file_bytes = bytes.len() as u64;
    set_u64(&mut bytes, 48, file_bytes);
    refresh_index_hash(&mut bytes);
    assert!(error(bytes).contains("trailing"));
}

#[test]
fn header_wire_fields_and_entry_hash_match_the_versioned_format() {
    let mut bytes = Vec::new();
    let summary = write_pack(
        &mut bytes,
        &[PackInput {
            key: "a",
            bytes: b"abc",
        }],
    )
    .unwrap();
    assert_eq!(&bytes[..8], b"RZASPACK");
    assert_eq!(&bytes[8..12], &1u32.to_le_bytes());
    assert_eq!(summary.index_bytes, 9 + 64);
    assert_eq!(summary.file_bytes, 96 + 73 + 3);
    assert_eq!(&bytes[40..48], &169u64.to_le_bytes());
    let reader = PackReader::open(Cursor::new(bytes)).unwrap();
    let hash = reader
        .entry("a")
        .unwrap()
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        hash,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn independent_preparation_wire_fixture_matches_rust_reader_and_writer() {
    let encoded = include_str!("../testdata/store-v1.hex").trim();
    let fixture = (0..encoded.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&encoded[offset..offset + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let mut reader = PackReader::open(Cursor::new(fixture.clone())).unwrap();
    assert_eq!(reader.keys().collect::<Vec<_>>(), ["a", "alias", "zero"]);
    assert_eq!(reader.read("a").unwrap().unwrap(), b"abc");
    assert_eq!(reader.read("alias").unwrap().unwrap(), b"abc");
    assert_eq!(reader.read("zero").unwrap().unwrap(), b"");
    reader.verify_all().unwrap();
    let mut output = Vec::new();
    write_pack(
        &mut output,
        &[
            PackInput {
                key: "zero",
                bytes: b"",
            },
            PackInput {
                key: "alias",
                bytes: b"abc",
            },
            PackInput {
                key: "a",
                bytes: b"abc",
            },
        ],
    )
    .unwrap();
    assert_eq!(output, fixture);
    let mut streamed = Vec::new();
    write_pack_from_loader(&mut streamed, &["zero", "a", "alias"], |key| {
        Ok(if key == "zero" { Vec::new() } else { b"abc".to_vec() })
    })
    .unwrap();
    assert_eq!(streamed, fixture);
}

#[test]
fn streaming_writer_rejects_source_changes_between_index_and_payload() {
    let mut calls = 0;
    let mut bytes = Vec::new();
    let error = write_pack_from_loader(&mut bytes, &["key"], |_| {
        calls += 1;
        Ok(if calls == 1 {
            b"abc".to_vec()
        } else {
            b"xyz".to_vec()
        })
    })
    .unwrap_err();
    assert!(error.to_string().contains("source changed"));
    assert!(PackReader::open(Cursor::new(bytes)).is_err());
    let mut output = Vec::new();
    assert!(
        write_pack_from_loader::<Vec<u8>>(&mut output, &["key"], |_| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "source unavailable",
            ))
        })
        .is_err()
    );
    assert!(output.is_empty());
}

struct CountedBytes {
    bytes: Vec<u8>,
    live: Rc<std::cell::Cell<usize>>,
}

impl AsRef<[u8]> for CountedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for CountedBytes {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}

#[test]
fn streaming_writer_retains_at_most_two_entries_during_deduplication() {
    let keys = (0..100)
        .map(|index| format!("alias-{index:03}"))
        .collect::<Vec<_>>();
    let keys = keys.iter().map(String::as_str).collect::<Vec<_>>();
    let live = Rc::new(std::cell::Cell::new(0));
    let mut output = Vec::new();
    let summary = write_pack_from_loader(&mut output, &keys, |_| {
        live.set(live.get() + 1);
        assert!(live.get() <= 2);
        Ok(CountedBytes {
            bytes: vec![7; 1024],
            live: live.clone(),
        })
    })
    .unwrap();
    assert_eq!(live.get(), 0);
    assert_eq!(summary.entry_count, 100);
    assert_eq!(summary.blob_count, 1);
    PackReader::open(Cursor::new(output))
        .unwrap()
        .verify_all()
        .unwrap();
}
