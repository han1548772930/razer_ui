//! Pure synthetic Feature transport tests for the source-derived mouse macro chain.
use super::*;
use crate::backend::FeatureTransport;
use anyhow::Result;
use std::{cell::RefCell, collections::VecDeque};

enum Step {
    Send(Vec<u8>),
    Get(Vec<u8>),
    SendError(Vec<u8>),
    ReadError,
}

struct Transport {
    steps: RefCell<VecDeque<Step>>,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
        }
    }
    fn finish(&self) {
        assert!(self.steps.borrow().is_empty(), "synthetic I/O remains");
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        match self.steps.borrow_mut().pop_front().expect("unexpected OUT") {
            Step::Send(expected) => {
                assert_eq!(report, expected);
                Ok(())
            }
            Step::SendError(expected) => {
                assert_eq!(report, expected);
                anyhow::bail!("synthetic send failure")
            }
            _ => panic!("expected IN, received OUT"),
        }
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        assert_eq!(report, [0; 91]);
        match self.steps.borrow_mut().pop_front().expect("unexpected IN") {
            Step::Get(response) => {
                report[..response.len()].copy_from_slice(&response);
                Ok(response.len())
            }
            Step::ReadError => anyhow::bail!("synthetic read failure"),
            _ => panic!("expected OUT, received IN"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn cap() -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(170).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}

fn packet(header: [u8; 3], payload: &[u8], tx: u8) -> Vec<u8> {
    let mut bytes = vec![0u8; 91];
    bytes[2] = tx;
    bytes[6..9].copy_from_slice(&header);
    bytes[9..9 + payload.len()].copy_from_slice(payload);
    bytes[89] = bytes[3..89].iter().fold(0, |crc, byte| crc ^ byte);
    bytes
}

fn send(command: [u8; 3], payload: &[u8], tx: u8, report_size: u8) -> Step {
    Step::Send(packet([report_size, command[1], command[2]], payload, tx))
}

fn reply(command: [u8; 3], payload: &[u8], tx: u8) -> Step {
    let mut bytes = packet(command, payload, tx);
    bytes[1] = 2;
    bytes[6] = payload.len() as u8;
    bytes[89] = bytes[3..89].iter().fold(0, |crc, byte| crc ^ byte);
    Step::Get(bytes)
}

fn txs() -> impl FnMut() -> Result<u8> {
    let mut tx = 0;
    move || {
        tx += 1;
        Ok(tx)
    }
}

fn request(size: usize) -> MacroWriteRequest {
    MacroWriteRequest {
        guid: "00112233-4455-6677-8899-aabbccddeeff".into(),
        num_items: 3,
        name: "macro".into(),
        timestamp: "20261012".into(),
        hash: "0123456789abcdef".into(),
        data: (0..size).map(|n| (n % 256) as u8).collect(),
    }
}

fn mouse_test<'a>(device: &'a Transport) -> MouseMacro<'a> {
    let mut mouse = MouseMacro::new(device, &cap()).unwrap();
    // Synthetic transport has no scheduling delay. Keep source timing verified
    // separately, and use a single failed attempt in error-stage tests.
    mouse.metadata_write_transport = cap();
    mouse.write_transport = cap();
    mouse
}

fn advance(tx: &mut u8) -> u8 {
    let value = *tx;
    *tx = (*tx + 1) % 31;
    value
}

fn storage_bytes(available: u32, recyclable: u32) -> Vec<u8> {
    let mut bytes = vec![0x7f, 0xff];
    bytes.extend_from_slice(&0x0100_0000u32.to_be_bytes());
    bytes.extend_from_slice(&available.to_be_bytes());
    bytes.extend_from_slice(&recyclable.to_be_bytes());
    bytes
}

fn storage_io(steps: &mut Vec<Step>, tx: &mut u8, available: u32, recyclable: u32) {
    let t = advance(tx);
    steps.push(send(STORAGE, &[], t, 14));
    steps.push(reply(STORAGE, &storage_bytes(available, recyclable), t));
}

fn ids_io(steps: &mut Vec<Step>, tx: &mut u8, ids: &[u16]) {
    let t = advance(tx);
    steps.push(send(COUNT, &[], t, 2));
    steps.push(reply(COUNT, &(ids.len() as u16).to_be_bytes(), t));
    for (page, chunk) in ids.chunks(32).enumerate() {
        let offset = (page * 32) as u16;
        let t = advance(tx);
        let mut data = offset.to_be_bytes().to_vec();
        data.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
        for id in chunk {
            data.extend_from_slice(&id.to_be_bytes());
        }
        steps.push(send(
            ID_LIST,
            &offset.to_be_bytes(),
            t,
            (chunk.len() * 2 + 4) as u8,
        ));
        steps.push(reply(ID_LIST, &data, t));
    }
}

fn inventory_io(steps: &mut Vec<Step>, tx: &mut u8, ids: &[u16], available: u32, recyclable: u32) {
    storage_io(steps, tx, available, recyclable);
    ids_io(steps, tx, ids);
}

fn metadata_io(steps: &mut Vec<Step>, tx: &mut u8, macro_id: u16, metadata: &[u8]) {
    for (page, chunk) in metadata.chunks(64).enumerate() {
        let offset = (page * 64) as u16;
        let t = advance(tx);
        let mut payload = macro_id.to_be_bytes().to_vec();
        payload.extend_from_slice(&offset.to_be_bytes());
        let mut data = payload.clone();
        // Raw returned size remains observable; its meaning is not guessed.
        data.extend_from_slice(&250u16.to_be_bytes());
        data.extend_from_slice(chunk);
        steps.push(send(GET_NAME, &payload, t, (chunk.len() + 6) as u8));
        steps.push(reply(GET_NAME, &data, t));
    }
}

fn size_io(steps: &mut Vec<Step>, tx: &mut u8, macro_id: u16, size: u32) {
    let t = advance(tx);
    let mut data = macro_id.to_be_bytes().to_vec();
    data.extend_from_slice(&size.to_be_bytes());
    steps.push(send(GET_SIZE, &macro_id.to_be_bytes(), t, 6));
    steps.push(reply(GET_SIZE, &data, t));
}

fn data_io(steps: &mut Vec<Step>, tx: &mut u8, macro_id: u16, data: &[u8]) {
    for (page, chunk) in data.chunks(72).enumerate() {
        let t = advance(tx);
        let mut payload = macro_id.to_be_bytes().to_vec();
        payload.extend_from_slice(&((page * 72) as u32).to_be_bytes());
        payload.push(chunk.len() as u8);
        let mut returned = payload.clone();
        returned.extend_from_slice(chunk);
        steps.push(send(GET_MEMORY, &payload, t, (chunk.len() + 7) as u8));
        steps.push(reply(GET_MEMORY, &returned, t));
    }
}

fn write_io(steps: &mut Vec<Step>, tx: &mut u8, macro_id: u16, req: &MacroWriteRequest) {
    let t = advance(tx);
    let mut allocation = macro_id.to_be_bytes().to_vec();
    allocation.extend_from_slice(&(req.data.len() as u32).to_be_bytes());
    steps.push(send(ALLOCATE, &allocation, t, 6));
    steps.push(reply(ALLOCATE, &[], t));
    for (page, chunk) in encode_metadata(req).unwrap().chunks(64).enumerate() {
        let t = advance(tx);
        let mut payload = macro_id.to_be_bytes().to_vec();
        payload.extend_from_slice(&((page * 64) as u16).to_be_bytes());
        payload.extend_from_slice(&250u16.to_be_bytes());
        payload.extend_from_slice(chunk);
        steps.push(send(SET_NAME, &payload, t, (chunk.len() + 6) as u8));
        steps.push(reply(SET_NAME, &[], t));
    }
    for (page, chunk) in req.data.chunks(72).enumerate() {
        let t = advance(tx);
        let mut payload = macro_id.to_be_bytes().to_vec();
        payload.extend_from_slice(&((page * 72) as u32).to_be_bytes());
        payload.push(chunk.len() as u8);
        payload.extend_from_slice(chunk);
        steps.push(send(SET_MEMORY, &payload, t, (chunk.len() + 7) as u8));
        steps.push(reply(SET_MEMORY, &[], t));
    }
}

fn wrapping_txs() -> impl FnMut() -> Result<u8> {
    let mut next = 1;
    move || Ok(advance(&mut next))
}

#[test]
fn source_constants_and_metadata_layout_are_exact() {
    assert!(AUDITED_PRODUCTS.iter().all(|id| capability(*id)));
    assert!(!capability(175));
    assert_eq!(MouseMacro::protocol_commands().len(), 12);
    let request = MacroWriteRequest {
        guid: "00112233-4455-6677-8899-aabbccddeeff".into(),
        num_items: 0x1122_3344,
        name: "macro".into(),
        timestamp: "2026-10-12".into(),
        hash: "abcd".into(),
        data: vec![],
    };
    let raw = encode_metadata(&request).unwrap();
    assert_eq!(
        &raw[..16],
        &[
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff
        ]
    );
    assert_eq!(&raw[16..20], &[0x44, 0x33, 0x22, 0x11]);
    assert_eq!(decode_metadata(7, raw).unwrap().guid, request.guid);
    assert_eq!(required_memory(0).unwrap(), 256);
    assert_eq!(required_memory(1).unwrap(), 512);
    assert_eq!(required_memory(256).unwrap(), 512);
}

#[test]
fn inventory_reads_storage_count_and_two_id_pages_in_source_order() {
    let ids1: Vec<u16> = (1..=32).collect();
    let ids2 = [99u16];
    let mut page1 = vec![0, 0, 0, 64];
    for id in &ids1 {
        page1.extend_from_slice(&id.to_be_bytes());
    }
    let mut page2 = vec![0, 32, 0, 2];
    page2.extend_from_slice(&ids2[0].to_be_bytes());
    let storage = [0, 200, 0, 0, 16, 0, 0, 0, 8, 0, 0, 0, 4, 0];
    let transport = Transport::new([
        send(STORAGE, &[], 1, 14),
        reply(STORAGE, &storage, 1),
        send(COUNT, &[], 2, 2),
        reply(COUNT, &[0, 33], 2),
        send(ID_LIST, &[0, 0], 3, 68),
        reply(ID_LIST, &page1, 3),
        send(ID_LIST, &[0, 32], 4, 6),
        reply(ID_LIST, &page2, 4),
    ]);
    let mouse = MouseMacro::new(&transport, &cap()).unwrap();
    let inventory = mouse.read_macro_inventory(&mut txs(), &|| Ok(())).unwrap();
    assert_eq!(inventory.count, 33);
    assert_eq!(&inventory.ids[..32], ids1.as_slice());
    assert_eq!(inventory.ids[32], 99);
    assert_eq!(inventory.storage.max_macro_id, 200);
    assert_eq!(inventory.storage.available_memory, 8 << 8);
    transport.finish();
}

#[test]
fn read_macro_preserves_metadata_endianness_and_72_byte_content_blocks() {
    let macro_id: u16 = 0x1234;
    let mut metadata = vec![0u8; 250];
    metadata[..16].copy_from_slice(&[
        0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ]);
    metadata[16..20].copy_from_slice(&0x0102_0304u32.to_le_bytes());
    metadata[20..25].copy_from_slice(b"hello");
    metadata[136..140].copy_from_slice(b"time");
    metadata[152..156].copy_from_slice(b"hash");
    let mut steps = Vec::new();
    for (offset, chunk) in metadata.chunks(64).enumerate() {
        let offset = offset * 64;
        let mut payload = vec![];
        payload.extend_from_slice(&macro_id.to_be_bytes());
        payload.extend_from_slice(&(offset as u16).to_be_bytes());
        payload.extend_from_slice(&(chunk.len() as u16).to_be_bytes());
        payload.extend_from_slice(chunk);
        steps.push(send(
            GET_NAME,
            &macro_id
                .to_be_bytes()
                .into_iter()
                .chain((offset as u16).to_be_bytes())
                .collect::<Vec<_>>(),
            (offset / 64 + 1) as u8,
            (chunk.len() + 6) as u8,
        ));
        steps.push(reply(GET_NAME, &payload, (offset / 64 + 1) as u8));
    }
    let data: Vec<u8> = (0..73).map(|n| n as u8).collect();
    steps.push(send(GET_SIZE, &macro_id.to_be_bytes(), 5, 6));
    steps.push(reply(GET_SIZE, &[0x12, 0x34, 0, 0, 0, 73], 5));
    let mut content = vec![
        macro_id.to_be_bytes()[0],
        macro_id.to_be_bytes()[1],
        0,
        0,
        0,
        0,
        72,
    ];
    content[2..6].copy_from_slice(&0u32.to_be_bytes());
    content.extend_from_slice(&data[..72]);
    steps.push(send(GET_MEMORY, &[0x12, 0x34, 0, 0, 0, 0, 72], 6, 79));
    steps.push(reply(GET_MEMORY, &content, 6));
    let mut tail = vec![0x12, 0x34, 0, 0, 0, 72, 1];
    tail.extend_from_slice(&data[72..]);
    steps.push(send(GET_MEMORY, &[0x12, 0x34, 0, 0, 0, 72, 1], 7, 8));
    steps.push(reply(GET_MEMORY, &tail, 7));
    let transport = Transport::new(steps);
    let mouse = MouseMacro::new(&transport, &cap()).unwrap();
    let reading = mouse
        .read_macro(macro_id, 73, &mut txs(), &|| Ok(()))
        .unwrap();
    assert_eq!(
        reading.metadata.guid,
        "00112233-4455-6677-8899-aabbccddeeff"
    );
    assert_eq!(reading.metadata.num_items, 0x0102_0304);
    assert_eq!(reading.metadata.name, "hello");
    assert_eq!(reading.data, data);
    transport.finish();
}

#[test]
fn cancellation_and_bad_metadata_do_not_fabricate_success() {
    let transport = Transport::new([]);
    let mouse = MouseMacro::new(&transport, &cap()).unwrap();
    let mut tx = txs();
    let error = mouse
        .read_storage_info(&mut tx, &|| anyhow::bail!("cancelled"))
        .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert!(decode_metadata(1, vec![0; 249]).is_err());
    transport.finish();
}

#[test]
fn receipts_pin_each_product_gate_commands_and_block_semantics() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-macro-current-source.json"
    ))
    .unwrap();
    assert_eq!(
        evidence["implemented_product_ids"],
        serde_json::json!([170, 204, 226, 45066])
    );
    for product in evidence["products"].as_array().unwrap() {
        assert_eq!(product["methods"].as_array().unwrap().len(), 12);
        assert_eq!(product["genericMethods"].as_array().unwrap().len(), 19);
        assert_eq!(product["source_class"], "rzDevice25DualLinkMouse");
        assert_eq!(product["semantics"]["data_offset"], "BE32");
        assert_eq!(
            product["semantics"]["metadata_report_sizes"],
            serde_json::json!([70, 70, 70, 64])
        );
        for (command, name) in MouseMacro::protocol_commands() {
            let receipt = product["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|method| method["name"] == *name)
                .unwrap();
            assert_eq!(receipt["command"], serde_json::to_value(command).unwrap());
            assert_eq!(receipt["receipt"]["sha256"].as_str().unwrap().len(), 64);
        }
    }
}

#[test]
fn constructor_gates_transport_and_preserves_source_owned_overrides() {
    let device = Transport::new([]);
    for product in AUDITED_PRODUCTS {
        let source = crate::device_reads::capability(*product).unwrap().clone();
        let mouse = MouseMacro::new(&device, &source).unwrap();
        assert_eq!(mouse.product_id(), *product);
        assert_eq!(mouse.transport.max_retry_out, 20);
        assert_eq!(mouse.metadata_transport.sleep_between_out_in_ms, 50);
        assert_eq!(mouse.metadata_transport.sleep_between_in_ms, 50);
        assert_eq!(mouse.write_transport.max_retry_out, 40);
        assert_eq!(mouse.metadata_write_transport.max_retry_out, 40);
        assert_eq!(mouse.metadata_write_transport.sleep_between_out_in_ms, 50);
        assert_eq!(source.sleep_between_out_in_ms, 25);
    }
    let mut wrong = cap();
    wrong.product_id = 175;
    assert!(MouseMacro::new(&device, &wrong).is_err());
    wrong = cap();
    wrong.source_class = "rzDevice30".into();
    assert!(MouseMacro::new(&device, &wrong).is_err());
    wrong = cap();
    wrong.transaction_modulus = 32;
    assert!(MouseMacro::new(&device, &wrong).is_err());
    wrong = cap();
    wrong.report_bytes = 90;
    assert!(MouseMacro::new(&device, &wrong).is_err());
    wrong = cap();
    wrong.max_retry_in = 0;
    assert!(MouseMacro::new(&device, &wrong).is_err());
    device.finish();
}

#[test]
fn utf8_metadata_truncation_and_source_no_nul_display_rules_are_retained() {
    let mut req = request(0);
    req.name = "a".repeat(99) + "界";
    req.timestamp = "b".repeat(16);
    req.hash = "c".repeat(32);
    let encoded = encode_metadata(&req).unwrap();
    assert_eq!(encoded[119], 0xe7);
    assert_eq!(&encoded[120..136], &[0; 16]);
    assert_eq!(&encoded[184..250], &[0; 66]);
    let parsed = decode_metadata(0, encoded).unwrap();
    assert_eq!(parsed.name, "a".repeat(99));
    assert_eq!(parsed.timestamp, "b".repeat(16));
    assert_eq!(parsed.hash, "c".repeat(32));
    req.name = "\u{feff}好".into();
    assert_eq!(
        decode_metadata(0, encode_metadata(&req).unwrap())
            .unwrap()
            .name,
        "好"
    );
}

#[test]
fn metadata_observation_retains_unknown_extra_info_and_invalid_utf8_bytes() {
    let mut raw = encode_metadata(&request(0)).unwrap();
    raw[20..24].copy_from_slice(&[0xef, 0x80, 0, 0]);
    raw[120..136].fill(0xaa);
    raw[200] = 0x55;
    let observation = decode_metadata(0, raw.clone()).unwrap();
    assert_eq!(observation.extra_info, [0xaa; 16]);
    assert!(observation.name.contains('\u{fffd}'));
    assert_eq!(observation.raw, raw);
}

#[test]
fn source_id_allocator_masks_entropy_retries_collisions_and_allows_zero() {
    let mut entropy = VecDeque::from([0xffff, 0x8000, 0x8123]);
    assert_eq!(
        allocate_macro_id(&[32767, 0], || Ok(entropy.pop_front().unwrap()), 3).unwrap(),
        0x123
    );
    assert_eq!(allocate_macro_id(&[], || Ok(0x8000), 1).unwrap(), 0);
    assert!(allocate_macro_id(&[], || Ok(1), 0).is_err());
    assert!(allocate_macro_id(&[1], || Ok(1), 2).is_err());
    assert!(
        allocate_macro_id(&[], || anyhow::bail!("entropy failed"), 1)
            .unwrap_err()
            .to_string()
            .contains("entropy failed")
    );
    let full: Vec<_> = (0..=32767).collect();
    assert!(allocate_macro_id(&full, || panic!("no ID can be allocated"), 1).is_err());
}

#[test]
fn storage_rounding_matches_source_boundaries_and_checks_overflow() {
    for (size, expected) in [
        (0, 256),
        (1, 512),
        (255, 512),
        (256, 512),
        (257, 768),
        (512, 768),
        (513, 1024),
    ] {
        assert_eq!(required_memory(size).unwrap(), expected);
    }
    assert!(required_memory(u32::MAX as usize).is_err());
}

#[test]
fn storage_fields_use_unsigned_big_endian_and_truncation_fails() {
    let mut raw = vec![0xff; 14];
    raw[6..10].copy_from_slice(&0x8123_4567u32.to_be_bytes());
    let device = Transport::new([send(STORAGE, &[], 1, 14), reply(STORAGE, &raw, 1)]);
    let storage = mouse_test(&device)
        .read_storage_info(&mut txs(), &|| Ok(()))
        .unwrap();
    assert_eq!(storage.max_memory, u32::MAX);
    assert_eq!(storage.available_memory, 0x8123_4567);
    assert_eq!(storage.recyclable_memory, u32::MAX);
    device.finish();
    for length in 0..14 {
        let device = Transport::new([
            send(STORAGE, &[], 1, 14),
            reply(STORAGE, &vec![0; length], 1),
        ]);
        assert!(
            mouse_test(&device)
                .read_storage_info(&mut txs(), &|| Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn empty_inventory_issues_no_id_page_and_preserves_zero_count() {
    let mut steps = Vec::new();
    inventory_io(&mut steps, &mut 1, &[], 4096, 0);
    let device = Transport::new(steps);
    let inventory = mouse_test(&device)
        .read_macro_inventory(&mut txs(), &|| Ok(()))
        .unwrap();
    assert_eq!(inventory.count, 0);
    assert!(inventory.ids.is_empty());
    assert!(inventory.pages.is_empty());
    device.finish();
}

#[test]
fn id_pages_retain_uninterpreted_size_and_fail_wrong_offsets_duplicates_and_short_data() {
    for page in [
        vec![0, 1, 0, 0, 0, 7],
        vec![0, 0, 0, 0, 0],
        vec![0, 0, 0, 0, 0, 7, 0, 7],
    ] {
        let count = if page.len() == 8 { 2 } else { 1 };
        let mut steps = Vec::new();
        storage_io(&mut steps, &mut 1, 4096, 0);
        steps.extend([
            send(COUNT, &[], 2, 2),
            reply(COUNT, &[0, count], 2),
            send(ID_LIST, &[0, 0], 3, count * 2 + 4),
            reply(ID_LIST, &page, 3),
        ]);
        let device = Transport::new(steps);
        assert!(
            mouse_test(&device)
                .read_macro_inventory(&mut txs(), &|| Ok(()))
                .is_err()
        );
        device.finish();
    }
    let mut steps = Vec::new();
    inventory_io(&mut steps, &mut 1, &[0, 0x1234], 4096, 0);
    let device = Transport::new(steps);
    let inventory = mouse_test(&device)
        .read_macro_inventory(&mut txs(), &|| Ok(()))
        .unwrap();
    assert_eq!(inventory.ids, [0, 0x1234]);
    assert_eq!(inventory.pages[0].returned_size, 2);
    device.finish();
}

#[test]
fn metadata_blocks_accept_raw_size_and_fail_wrong_id_offset_or_truncation() {
    let encoded = encode_metadata(&request(0)).unwrap();
    let mut steps = Vec::new();
    metadata_io(&mut steps, &mut 1, 7, &encoded);
    let device = Transport::new(steps);
    let metadata = mouse_test(&device)
        .read_macro_metadata(7, &mut txs(), &|| Ok(()))
        .unwrap();
    assert_eq!(metadata.blocks.len(), 4);
    assert_eq!(metadata.blocks[3].returned_size, 250);
    assert_eq!(metadata.raw, encoded);
    device.finish();
    for bad in [
        vec![0, 8, 0, 0, 0, 250],
        vec![0, 7, 0, 1, 0, 250],
        vec![0, 7, 0, 0, 0, 250],
    ] {
        let device = Transport::new([
            send(GET_NAME, &[0, 7, 0, 0], 1, 70),
            reply(GET_NAME, &bad, 1),
        ]);
        assert!(
            mouse_test(&device)
                .read_macro_metadata(7, &mut txs(), &|| Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn content_response_checks_u32_offsets_ids_sizes_and_missing_bytes() {
    for data in [
        vec![0, 8, 0, 0, 0, 0, 1, 77],
        vec![0, 7, 0, 1, 0, 0, 1, 77],
        vec![0, 7, 0, 0, 0, 0, 0],
        vec![0, 7, 0, 0, 0, 0, 1],
    ] {
        let device = Transport::new([
            send(GET_MEMORY, &[0, 7, 0, 0, 0, 0, 1], 1, 8),
            reply(GET_MEMORY, &data, 1),
        ]);
        let mut blocks = Vec::new();
        assert!(
            mouse_test(&device)
                .read_data_bytes(7, 1, &mut txs(), &|| Ok(()), &mut blocks)
                .is_err()
        );
        assert_eq!(blocks.len(), 1);
        device.finish();
    }
}

#[test]
fn read_bound_stops_before_content_allocation_and_empty_content_sends_no_blocks() {
    for (size, bound, success) in [(0, 0, true), (73, 72, false), (u32::MAX, 4096, false)] {
        let mut steps = Vec::new();
        metadata_io(
            &mut steps,
            &mut 1,
            7,
            &encode_metadata(&request(0)).unwrap(),
        );
        size_io(&mut steps, &mut 5, 7, size);
        let device = Transport::new(steps);
        let read = mouse_test(&device).read_macro(7, bound, &mut txs(), &|| Ok(()));
        assert_eq!(read.is_ok(), success);
        if let Ok(read) = read {
            assert!(read.data.is_empty());
            assert!(read.data_blocks.is_empty())
        }
        device.finish();
    }
}

#[test]
fn startup_loader_reads_each_metadata_then_size_and_does_not_load_content() {
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[7, 9], 8192, 0);
    for id in [7, 9] {
        metadata_io(
            &mut steps,
            &mut tx,
            id,
            &encode_metadata(&request(0)).unwrap(),
        );
        size_io(&mut steps, &mut tx, id, 73);
    }
    let device = Transport::new(steps);
    let catalog = mouse_test(&device)
        .load_macros(&mut txs(), &|| Ok(()))
        .unwrap();
    assert_eq!(catalog.macros.len(), 2);
    assert_eq!(catalog.macros[0].metadata.macro_id, 7);
    assert_eq!(catalog.macros[1].metadata.macro_id, 9);
    assert_eq!(catalog.macros[0].allocated_size_with_name, 512);
    device.finish();
}

#[test]
fn upload_allocates_then_writes_metadata_and_content_then_refreshes_and_reads_back() {
    for size in [0, 1, 72, 73, 144, 145] {
        let req = request(size);
        let id = 0x1234;
        let mut steps = Vec::new();
        let mut tx = 1;
        inventory_io(&mut steps, &mut tx, &[8], 8192, 0);
        write_io(&mut steps, &mut tx, id, &req);
        storage_io(
            &mut steps,
            &mut tx,
            8192 - required_memory(size).unwrap(),
            0,
        );
        metadata_io(&mut steps, &mut tx, id, &encode_metadata(&req).unwrap());
        size_io(&mut steps, &mut tx, id, size as u32);
        data_io(&mut steps, &mut tx, id, &req.data);
        ids_io(&mut steps, &mut tx, &[id, 8]);
        let device = Transport::new(steps);
        let report = mouse_test(&device)
            .write_macro(id, &req, &mut wrapping_txs(), &|| Ok(()))
            .unwrap();
        assert!(report.verified());
        assert_eq!(report.steps.len(), 5 + size.div_ceil(72));
        assert!(report.steps.iter().all(|step| step.acknowledgment_received));
        assert!(
            report
                .steps
                .iter()
                .all(|step| step.acknowledgment_data.is_empty())
        );
        assert_eq!(report.metadata_readback_blocks.len(), 4);
        assert_eq!(report.data_readback_blocks.len(), size.div_ceil(72));
        assert_eq!(report.data_readback.as_ref().unwrap(), &req.data);
        assert_eq!(report.steps[4].payload[4..6], [0, 250]);
        assert_eq!(report.steps[4].report_data_size, 64);
        if size > 0 {
            assert_eq!(report.steps[5].report_data_size, (size.min(72) + 7) as u8);
        }
        assert_eq!(report.inventory_after.as_ref().unwrap().ids, [id, 8]);
        device.finish();
    }
}

#[test]
fn invalid_guid_id_and_cancelled_preparation_issue_no_io() {
    for bad in [
        "",
        "00112233445566778899aabbccddeeff",
        "gg112233-4455-6677-8899-aabbccddeeff",
    ] {
        let device = Transport::new([]);
        let mut req = request(1);
        req.guid = bad.into();
        let failure = mouse_test(&device)
            .write_macro(
                1,
                &req,
                &mut || panic!("invalid GUID minted transaction"),
                &|| Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, MacroFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
        device.finish();
    }
    let device = Transport::new([]);
    let mouse = mouse_test(&device);
    assert_eq!(
        mouse
            .write_macro(
                0x8000,
                &request(1),
                &mut || panic!("out-of-source ID minted transaction"),
                &|| Ok(())
            )
            .unwrap_err()
            .phase,
        MacroFailurePhase::Prepare
    );
    let failure = mouse
        .write_macro(
            1,
            &request(1),
            &mut || panic!("cancelled preparation minted transaction"),
            &|| anyhow::bail!("cancelled"),
        )
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Cancelled);
    device.finish();
}

#[test]
fn duplicate_ids_insufficient_storage_and_required_compaction_fail_before_mutation() {
    for (ids, available, recyclable, phase) in [
        (vec![7], 4096, 0, MacroFailurePhase::Prepare),
        (vec![], 511, 0, MacroFailurePhase::Prepare),
        (vec![], 256, 256, MacroFailurePhase::CompactionRequired),
    ] {
        let mut steps = Vec::new();
        inventory_io(&mut steps, &mut 1, &ids, available, recyclable);
        let device = Transport::new(steps);
        let failure = mouse_test(&device)
            .write_macro(7, &request(1), &mut txs(), &|| Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, phase);
        assert!(failure.progress.steps.is_empty());
        assert!(!failure.device_state_may_have_changed());
        assert!(failure.progress.inventory_before.is_some());
        device.finish();
    }
}

#[test]
fn upload_failure_retains_all_acknowledged_prefixes_and_stops_subsequent_blocks() {
    for (index, phase) in [
        (0, MacroFailurePhase::Allocate),
        (1, MacroFailurePhase::Metadata),
        (4, MacroFailurePhase::Metadata),
        (5, MacroFailurePhase::Data),
        (6, MacroFailurePhase::Data),
    ] {
        let req = request(73);
        let mut steps = Vec::new();
        let mut tx = 1;
        inventory_io(&mut steps, &mut tx, &[], 8192, 0);
        let mut writes = Vec::new();
        write_io(&mut writes, &mut tx, 7, &req);
        writes.truncate(index * 2 + 1);
        writes.push(Step::ReadError);
        steps.extend(writes);
        let device = Transport::new(steps);
        let failure = mouse_test(&device)
            .write_macro(7, &req, &mut txs(), &|| Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, phase);
        assert_eq!(failure.failed_step, Some(index));
        assert_eq!(failure.progress.steps.len(), index + 1);
        assert!(
            failure.progress.steps[..index]
                .iter()
                .all(|step| step.acknowledgment_received)
        );
        assert!(!failure.progress.steps[index].acknowledgment_received);
        assert!(failure.device_state_may_have_changed());
        assert!(!failure.progress.verified());
        assert!(failure.progress.storage_after.is_none());
        device.finish();
    }
}

#[test]
fn lost_allocation_send_retains_uncertain_device_change_without_claiming_ack() {
    let mut steps = Vec::new();
    inventory_io(&mut steps, &mut 1, &[], 8192, 0);
    steps.push(Step::SendError(packet(ALLOCATE, &[0, 7, 0, 0, 0, 1], 3)));
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &request(1), &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Allocate);
    assert!(failure.device_state_may_have_changed());
    assert!(!failure.progress.steps[0].acknowledgment_received);
    device.finish();
}

#[test]
fn source_storage_refresh_failure_retains_all_upload_acks_but_no_readback() {
    let req = request(73);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    write_io(&mut steps, &mut tx, 7, &req);
    let t = advance(&mut tx);
    steps.extend([send(STORAGE, &[], t, 14), Step::ReadError]);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::StorageRefresh);
    assert_eq!(failure.progress.steps.len(), 7);
    assert!(
        failure
            .progress
            .steps
            .iter()
            .all(|step| step.acknowledgment_received)
    );
    assert!(failure.progress.metadata_readback.is_none());
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn metadata_mismatch_is_retained_as_independent_observation_after_all_acks() {
    let req = request(1);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    write_io(&mut steps, &mut tx, 7, &req);
    storage_io(&mut steps, &mut tx, 7680, 0);
    let mut raw = encode_metadata(&req).unwrap();
    raw[249] = 99;
    metadata_io(&mut steps, &mut tx, 7, &raw);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Readback);
    assert_eq!(
        failure.progress.metadata_readback.as_ref().unwrap().raw,
        raw
    );
    assert_eq!(failure.progress.metadata_readback_blocks.len(), 4);
    assert!(failure.progress.storage_after.is_some());
    assert!(
        failure
            .progress
            .steps
            .iter()
            .all(|step| step.acknowledgment_received)
    );
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn partial_metadata_readback_failure_retains_completed_raw_blocks() {
    let req = request(1);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    write_io(&mut steps, &mut tx, 7, &req);
    storage_io(&mut steps, &mut tx, 7680, 0);
    let mut queries = Vec::new();
    metadata_io(&mut queries, &mut tx, 7, &encode_metadata(&req).unwrap());
    queries.truncate(5);
    queries.push(Step::ReadError);
    steps.extend(queries);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Readback);
    assert_eq!(failure.progress.metadata_readback_blocks.len(), 2);
    assert_eq!(failure.progress.metadata_readback_blocks[1].offset, 64);
    assert!(failure.progress.metadata_readback.is_none());
    device.finish();
}

#[test]
fn data_size_mismatch_stops_before_memory_query_and_retains_size() {
    let req = request(1);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    write_io(&mut steps, &mut tx, 7, &req);
    storage_io(&mut steps, &mut tx, 7680, 0);
    metadata_io(&mut steps, &mut tx, 7, &encode_metadata(&req).unwrap());
    size_io(&mut steps, &mut tx, 7, 2);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Readback);
    assert_eq!(failure.progress.data_size_readback, Some(2));
    assert!(failure.progress.data_readback.is_none());
    device.finish();
}

#[test]
fn content_mismatch_retains_bytes_after_all_block_acks() {
    let req = request(73);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    write_io(&mut steps, &mut tx, 7, &req);
    storage_io(&mut steps, &mut tx, 7680, 0);
    metadata_io(&mut steps, &mut tx, 7, &encode_metadata(&req).unwrap());
    size_io(&mut steps, &mut tx, 7, 73);
    let mut wrong = req.data.clone();
    wrong[72] = 0xff;
    data_io(&mut steps, &mut tx, 7, &wrong);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Readback);
    assert_eq!(failure.progress.data_readback, Some(wrong));
    assert_eq!(failure.progress.data_readback_blocks.len(), 2);
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn inventory_readback_requires_preserving_other_ids() {
    let req = request(0);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[8], 8192, 0);
    write_io(&mut steps, &mut tx, 7, &req);
    storage_io(&mut steps, &mut tx, 7936, 0);
    metadata_io(&mut steps, &mut tx, 7, &encode_metadata(&req).unwrap());
    size_io(&mut steps, &mut tx, 7, 0);
    ids_io(&mut steps, &mut tx, &[7, 9]);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Readback);
    assert_eq!(
        failure.progress.inventory_after.as_ref().unwrap().ids,
        [7, 9]
    );
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn delete_keeps_source_storage_refresh_and_independently_confirms_other_ids() {
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[7, 8], 8192, 0);
    let t = advance(&mut tx);
    steps.extend([send(DELETE, &[0, 7], t, 2), reply(DELETE, &[], t)]);
    storage_io(&mut steps, &mut tx, 8192, 512);
    ids_io(&mut steps, &mut tx, &[8]);
    let device = Transport::new(steps);
    let report = mouse_test(&device)
        .delete_macro(7, &mut txs(), &|| Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].command, DELETE);
    assert_eq!(
        report.storage_after.as_ref().unwrap().recyclable_memory,
        512
    );
    assert_eq!(report.inventory_after.unwrap().ids, [8]);
    device.finish();
}

#[test]
fn delete_absent_id_has_no_mutation() {
    let mut steps = Vec::new();
    inventory_io(&mut steps, &mut 1, &[8], 8192, 0);
    let device = Transport::new(steps);
    let failure = mouse_test(&device)
        .delete_macro(7, &mut txs(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Observe);
    assert!(failure.progress.steps.is_empty());
    assert!(!failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn delete_lost_ack_refresh_failure_and_unchanged_inventory_are_distinct() {
    for phase in [
        MacroFailurePhase::Delete,
        MacroFailurePhase::StorageRefresh,
        MacroFailurePhase::Readback,
    ] {
        let mut steps = Vec::new();
        let mut tx = 1;
        inventory_io(&mut steps, &mut tx, &[7, 8], 8192, 0);
        let t = advance(&mut tx);
        steps.push(send(DELETE, &[0, 7], t, 2));
        if phase == MacroFailurePhase::Delete {
            steps.push(Step::ReadError)
        } else {
            steps.push(reply(DELETE, &[], t));
            if phase == MacroFailurePhase::StorageRefresh {
                let t = advance(&mut tx);
                steps.extend([send(STORAGE, &[], t, 14), Step::ReadError]);
            } else {
                storage_io(&mut steps, &mut tx, 8192, 512);
                ids_io(&mut steps, &mut tx, &[7, 8]);
            }
        }
        let device = Transport::new(steps);
        let failure = mouse_test(&device)
            .delete_macro(7, &mut txs(), &|| Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, phase);
        assert!(failure.device_state_may_have_changed());
        assert_eq!(
            failure.progress.steps[0].acknowledgment_received,
            phase != MacroFailurePhase::Delete
        );
        assert!(!failure.progress.verified());
        if phase == MacroFailurePhase::Readback {
            assert!(failure.progress.inventory_after.is_some());
        }
        device.finish();
    }
}

#[test]
fn transaction_failure_between_blocks_retains_prefix_without_marking_new_attempt() {
    let req = request(1);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    let mut writes = Vec::new();
    write_io(&mut writes, &mut tx, 7, &req);
    writes.truncate(4);
    steps.extend(writes);
    let device = Transport::new(steps);
    let mut next = 1;
    let mut transactions = || {
        if next == 5 {
            anyhow::bail!("counter failed")
        }
        let t = next;
        next += 1;
        Ok(t)
    };
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut transactions, &|| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Metadata);
    assert_eq!(failure.progress.steps.len(), 2);
    assert!(
        failure
            .progress
            .steps
            .iter()
            .all(|step| step.acknowledgment_received)
    );
    assert_eq!(failure.failed_step, None);
    device.finish();
}

#[test]
fn cancellation_before_next_metadata_block_stops_io_and_retains_prior_acks() {
    let req = request(1);
    let mut steps = Vec::new();
    let mut tx = 1;
    inventory_io(&mut steps, &mut tx, &[], 8192, 0);
    let mut writes = Vec::new();
    write_io(&mut writes, &mut tx, 7, &req);
    writes.truncate(4);
    steps.extend(writes);
    let device = Transport::new(steps);
    let cancelled = std::cell::Cell::new(false);
    let mut next = 1;
    let mut transactions = || {
        let t = next;
        next += 1;
        if t == 5 {
            cancelled.set(true)
        }
        Ok(t)
    };
    let validate = || {
        anyhow::ensure!(!cancelled.get(), "cancelled");
        Ok(())
    };
    let failure = mouse_test(&device)
        .write_macro(7, &req, &mut transactions, &validate)
        .unwrap_err();
    assert_eq!(failure.phase, MacroFailurePhase::Cancelled);
    assert_eq!(failure.progress.steps.len(), 2);
    assert!(
        failure
            .progress
            .steps
            .iter()
            .all(|step| step.acknowledgment_received)
    );
    assert!(failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn busy_reply_retries_in_without_resending_and_success_is_still_not_readback() {
    let mut steps = Vec::new();
    inventory_io(&mut steps, &mut 1, &[7], 8192, 0);
    steps.push(send(DELETE, &[0, 7], 4, 2));
    let Step::Get(mut busy) = reply(DELETE, &[], 4) else {
        unreachable!()
    };
    busy[1] = 1;
    steps.push(Step::Get(busy));
    steps.push(reply(DELETE, &[], 4));
    storage_io(&mut steps, &mut 5, 8192, 256);
    ids_io(&mut steps, &mut 6, &[]);
    let device = Transport::new(steps);
    let mut mouse = mouse_test(&device);
    mouse.transport.max_retry_in = 2;
    let report = mouse.delete_macro(7, &mut txs(), &|| Ok(())).unwrap();
    assert!(report.verified());
    assert_eq!(report.steps.len(), 1);
    assert!(report.steps[0].acknowledgment_received);
    device.finish();
}

#[test]
fn malformed_report_count_and_unsupported_status_fail_without_ack() {
    for kind in 0..3 {
        let mut steps = Vec::new();
        inventory_io(&mut steps, &mut 1, &[7], 8192, 0);
        steps.push(send(DELETE, &[0, 7], 4, 2));
        let Step::Get(mut raw) = reply(DELETE, &[], 4) else {
            unreachable!()
        };
        match kind {
            0 => {
                raw.truncate(90);
            }
            1 => {
                raw[1] = 5;
            }
            _ => {
                raw[2] = 9;
            }
        }
        steps.push(Step::Get(raw));
        let device = Transport::new(steps);
        let failure = mouse_test(&device)
            .delete_macro(7, &mut txs(), &|| Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, MacroFailurePhase::Delete);
        assert!(!failure.progress.steps[0].acknowledgment_received);
        assert!(failure.device_state_may_have_changed());
        device.finish();
    }
}
