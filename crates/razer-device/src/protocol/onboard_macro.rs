//! Current rzDevice25 onboard macro storage and content chain.
//!
//! The implementation follows the receipt-pinned mouse and magnetic-keyboard
//! OBM methods. Product adapters retain their independent transport gates.
//! It keeps transport acknowledgements,
//! independent readback and storage refresh separate: a successful Feature
//! reply is not treated as proof that the device committed the requested bytes.
//! Local profile drafts and macro-editor JSON are outside this executor.
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability,
    feature_exchange::exchange_with_report_data_size,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const AUDITED_MOUSE_PRODUCTS: &[u32] = &[170, 204, 226, 45066];
pub const AUDITED_KEYBOARD_PRODUCTS: &[u32] = &[740, 746];
const STORAGE: [u8; 3] = [14, 6, 142];
const COUNT: [u8; 3] = [2, 6, 128];
const ID_LIST: [u8; 3] = [80, 6, 139];
const CREATE: [u8; 3] = [2, 6, 2];
const DELETE: [u8; 3] = [2, 6, 3];
const GET_NAME: [u8; 3] = [80, 6, 140];
const SET_NAME: [u8; 3] = [80, 6, 12];
const ALLOCATE: [u8; 3] = [6, 6, 8];
const GET_MEMORY: [u8; 3] = [80, 6, 137];
const SET_MEMORY: [u8; 3] = [80, 6, 9];
const GET_DATA: [u8; 3] = [80, 6, 133];
const GET_SIZE: [u8; 3] = [6, 6, 136];

const METADATA_BYTES: usize = 250;
const METADATA_CHUNK: usize = 64;
const DATA_CHUNK: usize = 72;
const MAX_ID_PAGE: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroStorageInfo {
    pub max_macro_id: u16,
    pub max_memory: u32,
    pub available_memory: u32,
    pub recyclable_memory: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroInventory {
    pub storage: MacroStorageInfo,
    pub count: u16,
    pub ids: Vec<u16>,
    pub pages: Vec<MacroIdPage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroIdPage {
    pub offset: u16,
    /// Exposed by the source parser but not used to slice the requested page.
    pub returned_size: u16,
    pub ids: Vec<u16>,
    pub raw: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroReadBlock {
    pub offset: u32,
    pub returned_size: u16,
    pub data: Vec<u8>,
    pub raw_response: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroMetadata {
    pub macro_id: u16,
    pub guid: String,
    pub num_items: u32,
    pub name: String,
    pub extra_info: [u8; 16],
    pub timestamp: String,
    pub hash: String,
    pub raw: Vec<u8>,
    pub blocks: Vec<MacroReadBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroReading {
    pub metadata: MacroMetadata,
    pub data: Vec<u8>,
    pub data_size: u32,
    pub data_blocks: Vec<MacroReadBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroSummary {
    pub metadata: MacroMetadata,
    pub data_size: u32,
    pub allocated_size_with_name: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroCatalog {
    pub inventory: MacroInventory,
    /// Source loader reads metadata and size; detailed bytes are read on demand.
    pub macros: Vec<MacroSummary>,
}

impl MacroCatalog {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.inventory.ids.len() == usize::from(self.inventory.count)
                && self.macros.len() == self.inventory.ids.len(),
            "Macro catalog count differs from its observed inventory"
        );
        let mut ids = std::collections::BTreeSet::new();
        for (summary, id) in self.macros.iter().zip(&self.inventory.ids) {
            ensure!(
                ids.insert(*id) && summary.metadata.macro_id == *id,
                "Macro catalog contains a repeated or foreign ID"
            );
            let mut decoded = decode_metadata(*id, summary.metadata.raw.clone())?;
            decoded.blocks = summary.metadata.blocks.clone();
            ensure!(
                decoded == summary.metadata,
                "Macro metadata differs from its observed bytes"
            );
            ensure!(
                summary.data_size <= self.inventory.storage.max_memory
                    && summary.allocated_size_with_name
                        == required_memory(summary.data_size as usize)?,
                "Macro catalog size differs from its observed storage"
            );
        }
        Ok(())
    }
}

/// Current keyboard CalMaxMem loader starts from the observed available-memory
/// field, subtracts allocated macros and non-live profile metadata, and tracks
/// deletions separately. The keyboard branch reuses this allowance directly;
/// only the mouse branch performs destructive storage defragmentation.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct KeyboardMacroMemory {
    available: i64,
    recyclable: i64,
}

impl KeyboardMacroMemory {
    pub(crate) fn observe(catalog: &MacroCatalog, profile_count: usize) -> Result<Self> {
        ensure!(
            profile_count > 0,
            "Keyboard macro profile inventory is empty"
        );
        let occupied = catalog.macros.iter().try_fold(0u32, |sum, entry| {
            sum.checked_add(entry.allocated_size_with_name)
                .context("Keyboard macro allocation sum overflow")
        })?;
        let profiles = u32::try_from(profile_count - 1)?
            .checked_mul(256)
            .context("Keyboard profile metadata allocation overflow")?;
        let occupied = occupied
            .checked_add(profiles)
            .context("Keyboard storage allocation overflow")?;
        let available = i64::from(catalog.inventory.storage.available_memory) - i64::from(occupied);
        Ok(Self {
            available,
            recyclable: 0,
        })
    }

    pub(crate) fn released(&mut self, allocated_size: u32) -> Result<()> {
        self.recyclable = self
            .recyclable
            .checked_add(i64::from(allocated_size))
            .context("Keyboard recyclable macro allocation overflow")?;
        Ok(())
    }

    fn after_upload(self, required: u32) -> Result<Option<Self>> {
        let required = i64::from(required);
        let total = self
            .available
            .checked_add(self.recyclable)
            .context("Keyboard macro allowance overflow")?;
        if required > total {
            return Ok(None);
        }
        if required > self.available {
            Ok(Some(Self {
                available: total - required,
                recyclable: 0,
            }))
        } else {
            Ok(Some(Self {
                available: self.available - required,
                ..self
            }))
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroWriteRequest {
    pub guid: String,
    pub num_items: u32,
    pub name: String,
    pub timestamp: String,
    pub hash: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MacroBlock {
    pub operation: String,
    pub command: [u8; 3],
    pub report_data_size: u8,
    pub payload: Vec<u8>,
    /// Entered a mutation exchange; a lost reply may follow a device write.
    pub attempted: bool,
    pub acknowledgment_received: bool,
    pub acknowledgment_data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct MacroWriteReport {
    pub macro_id: u16,
    pub storage_before: Option<MacroStorageInfo>,
    pub storage_after: Option<MacroStorageInfo>,
    pub inventory_before: Option<MacroInventory>,
    pub inventory_after: Option<MacroInventory>,
    pub steps: Vec<MacroBlock>,
    pub metadata_readback: Option<MacroMetadata>,
    pub metadata_readback_blocks: Vec<MacroReadBlock>,
    pub data_size_readback: Option<u32>,
    pub data_readback: Option<Vec<u8>>,
    pub data_readback_blocks: Vec<MacroReadBlock>,
    verified: bool,
}

impl MacroWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MacroFailurePhase {
    Prepare,
    Observe,
    Allocate,
    Metadata,
    Data,
    Delete,
    StorageRefresh,
    Readback,
    Cancelled,
    CompactionRequired,
    InsufficientStorage,
}

#[derive(Debug)]
pub struct MacroWriteFailure {
    pub phase: MacroFailurePhase,
    pub progress: MacroWriteReport,
    pub failed_step: Option<usize>,
    error: anyhow::Error,
}

impl MacroWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.progress.steps.iter().any(|step| step.attempted)
    }

    pub fn error(&self) -> &anyhow::Error {
        &self.error
    }
}

impl fmt::Display for MacroWriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Onboard macro {:?} failed; progress retained: {:#}",
            self.phase, self.error
        )
    }
}

impl std::error::Error for MacroWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub fn mouse_capability(product_id: u32) -> bool {
    AUDITED_MOUSE_PRODUCTS.contains(&product_id)
}

fn be16(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

fn be32(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

fn guid_bytes(value: &str) -> Result<[u8; 16]> {
    let bytes = value.as_bytes();
    ensure!(
        bytes.len() == 36 && [8, 13, 18, 23].iter().all(|i| bytes[*i] == b'-'),
        "Macro metadata GUID must be a canonical 8-4-4-4-12 value"
    );
    let digits: Vec<_> = bytes
        .iter()
        .enumerate()
        .filter_map(|(i, b)| (!matches!(i, 8 | 13 | 18 | 23)).then_some(*b))
        .collect();
    let mut result = [0u8; 16];
    for (i, pair) in digits.chunks_exact(2).enumerate() {
        let high = char::from(pair[0])
            .to_digit(16)
            .context("invalid macro GUID high nibble")?;
        let low = char::from(pair[1])
            .to_digit(16)
            .context("invalid macro GUID low nibble")?;
        result[i] = (high * 16 + low) as u8;
    }
    result[..4].reverse();
    result[4..6].reverse();
    result[6..8].reverse();
    Ok(result)
}

fn guid_text(bytes: &[u8]) -> Result<String> {
    let mut guid: [u8; 16] = bytes.try_into().context("macro GUID is not 16 bytes")?;
    guid[..4].reverse();
    guid[4..6].reverse();
    guid[6..8].reverse();
    let mut text = String::with_capacity(36);
    for (index, byte) in guid.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        use std::fmt::Write as _;
        write!(text, "{byte:02x}").expect("String write cannot fail");
    }
    Ok(text)
}

fn source_text(field: &[u8], drop_last_without_nul: bool) -> String {
    let end = field.iter().position(|byte| *byte == 0).unwrap_or_else(|| {
        if drop_last_without_nul {
            field.len().saturating_sub(1)
        } else {
            field.len()
        }
    });
    let bytes = field[..end]
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(&field[..end]);
    String::from_utf8_lossy(bytes).into_owned()
}

fn encode_text(field: &str, capacity: usize) -> Vec<u8> {
    let bytes = field.as_bytes();
    let mut out = vec![0u8; capacity];
    out[..bytes.len().min(capacity)].copy_from_slice(&bytes[..bytes.len().min(capacity)]);
    out
}

pub fn encode_metadata(request: &MacroWriteRequest) -> Result<Vec<u8>> {
    let mut out = vec![0u8; METADATA_BYTES];
    out[..16].copy_from_slice(&guid_bytes(&request.guid)?);
    out[16..20].copy_from_slice(&request.num_items.to_le_bytes());
    out[20..120].copy_from_slice(&encode_text(&request.name, 100));
    // The current setter takes no extraInfo argument and always writes zeroes.
    out[136..152].copy_from_slice(&encode_text(&request.timestamp, 16));
    out[152..184].copy_from_slice(&encode_text(&request.hash, 32));
    Ok(out)
}

fn decode_metadata(macro_id: u16, raw: Vec<u8>) -> Result<MacroMetadata> {
    ensure!(
        raw.len() == METADATA_BYTES,
        "macro metadata must contain 250 bytes"
    );
    Ok(MacroMetadata {
        macro_id,
        guid: guid_text(&raw[..16])?,
        num_items: u32::from_le_bytes(raw[16..20].try_into().unwrap()),
        name: source_text(&raw[20..120], true),
        extra_info: raw[120..136].try_into().unwrap(),
        timestamp: source_text(&raw[136..152], false),
        hash: source_text(&raw[152..184], false),
        raw,
        blocks: Vec::new(),
    })
}

fn required_memory(data_size: usize) -> Result<u32> {
    let size = u32::try_from(data_size).context("macro content exceeds u32")?;
    let with_name = size
        .checked_add(256)
        .context("macro storage size overflow")?;
    let blocks = with_name
        .checked_add(255)
        .context("macro storage rounding overflow")?
        / 256;
    blocks
        .checked_mul(256)
        .context("macro storage size overflow")
}

/// The current source masks a crypto-random u16 with 0x7fff and retries IDs
/// already present in observed inventory. Entropy is supplied by the OS adapter;
/// zero is a possible source ID. A caller-specified bound avoids an endless loop.
pub fn allocate_macro_id(
    observed_ids: &[u16],
    mut random_u16: impl FnMut() -> Result<u16>,
    max_attempts: usize,
) -> Result<u16> {
    ensure!(max_attempts > 0, "macro ID attempt bound must be nonzero");
    let mut occupied = vec![false; 32768];
    for id in observed_ids.iter().filter(|id| **id <= 32767) {
        occupied[usize::from(*id)] = true;
    }
    ensure!(
        occupied.iter().any(|taken| !taken),
        "all 15-bit macro IDs are occupied"
    );
    for _ in 0..max_attempts {
        let id = random_u16()? & 0x7fff;
        if !occupied[usize::from(id)] {
            return Ok(id);
        }
    }
    anyhow::bail!("macro ID allocation exhausted its collision bound")
}

pub struct OnboardMacro<'a> {
    device: &'a dyn FeatureTransport,
    transport: DeviceReadCapability,
    metadata_transport: DeviceReadCapability,
    write_transport: DeviceReadCapability,
    metadata_write_transport: DeviceReadCapability,
}

/// The current 679 loader shares the captured macro read protocol. Its write
/// branch is deliberately outside this capability's public surface.
pub(crate) struct CurrentKeyboardMacroReader<'a> {
    inner: OnboardMacro<'a>,
}

impl<'a> CurrentKeyboardMacroReader<'a> {
    pub(crate) fn new(
        device: &'a dyn FeatureTransport,
        cap: &DeviceReadCapability,
    ) -> Result<Self> {
        let audited = crate::keyboard_metadata::capability(679)
            .context("Current keyboard macro factory is absent")?;
        ensure!(
            cap.product_id == 679
                && cap.source_class == audited.source_class
                && cap.vendor_id == audited.vendor_id
                && cap.claim_interface == audited.claim_interface
                && cap.claim_interfaces_by_pid == audited.claim_interfaces_by_pid
                && cap.direct_pids == audited.direct_pids,
            "Current keyboard macro reader differs from its captured HID factory"
        );
        Ok(Self {
            inner: OnboardMacro::from_transport(device, cap)?,
        })
    }

    pub(crate) fn load(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroCatalog> {
        self.inner.load_macros(transactions, validate)
    }

    /// The consumer has already read metadata in the catalog. Its detail phase
    /// rereads size and content only, retaining that same observed metadata.
    pub(crate) fn read_details(
        &self,
        summary: &MacroSummary,
        max_data_bytes: u32,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroReading> {
        let macro_id = summary.metadata.macro_id;
        let data_size = self
            .inner
            .read_data_size(macro_id, transactions, validate)?;
        ensure!(
            data_size <= max_data_bytes,
            "Macro content exceeds its read bound"
        );
        let mut data_blocks = Vec::new();
        let data = self.inner.read_data_bytes(
            macro_id,
            data_size,
            transactions,
            validate,
            &mut data_blocks,
        )?;
        Ok(MacroReading {
            metadata: summary.metadata.clone(),
            data_size,
            data,
            data_blocks,
        })
    }
}

impl<'a> OnboardMacro<'a> {
    pub fn for_mouse(device: &'a dyn FeatureTransport, cap: &DeviceReadCapability) -> Result<Self> {
        ensure!(
            mouse_capability(cap.product_id) && cap.source_class == "rzDevice25DualLinkMouse",
            "Mouse macro requires an audited rzDevice25 DualLink Feature transport"
        );
        Self::from_transport(device, cap)
    }

    pub(crate) fn for_keyboard(
        device: &'a dyn FeatureTransport,
        cap: &crate::keyboard_analog::KeyboardAnalogTransportCapability,
    ) -> Result<Self> {
        ensure!(
            AUDITED_KEYBOARD_PRODUCTS.contains(&cap.product_id)
                && crate::keyboard_analog::transport_capability(cap.product_id) == Some(*cap),
            "Keyboard macro requires the captured magnetic-keyboard transport"
        );
        Self::from_transport(device, &cap.device_read_capability())
    }

    fn from_transport(
        device: &'a dyn FeatureTransport,
        cap: &DeviceReadCapability,
    ) -> Result<Self> {
        ensure!(
            cap.report_bytes == 91
                && cap.report_id == 0
                && cap.transaction_prefix == 0
                && cap.transaction_modulus == 31
                && cap.max_retry_out > 0
                && cap.max_retry_in > 0,
            "Onboard macro Feature transport layout differs"
        );
        let mut metadata_transport = cap.clone();
        let extended = metadata_transport
            .sleep_between_out_in_ms
            .checked_mul(2)
            .context("macro extended sleep overflow")?;
        metadata_transport.sleep_between_out_in_ms =
            metadata_transport.sleep_between_out_in_ms.max(extended);
        metadata_transport.sleep_between_in_ms =
            metadata_transport.sleep_between_in_ms.max(extended);
        let mut write_transport = cap.clone();
        write_transport.sleep_between_out_in_ms = write_transport.sleep_between_out_in_ms.max(10);
        write_transport.sleep_between_in_ms = write_transport.sleep_between_in_ms.max(10);
        write_transport.max_retry_out = write_transport.max_retry_out.max(40);
        let mut metadata_write_transport = write_transport.clone();
        metadata_write_transport.sleep_between_out_in_ms = metadata_write_transport
            .sleep_between_out_in_ms
            .max(extended);
        metadata_write_transport.sleep_between_in_ms =
            metadata_write_transport.sleep_between_in_ms.max(extended);
        Ok(Self {
            device,
            transport: cap.clone(),
            metadata_transport,
            write_transport,
            metadata_write_transport,
        })
    }

    pub fn product_id(&self) -> u32 {
        self.transport.product_id
    }

    fn query(
        &self,
        transport: &DeviceReadCapability,
        command: [u8; 3],
        payload: &[u8],
        report_data_size: u8,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        validate()?;
        let transaction = transactions()?;
        exchange_with_report_data_size(
            self.device,
            transport,
            command,
            payload,
            transaction,
            report_data_size,
            validate,
        )
    }

    fn step(
        &self,
        operation: &str,
        command: [u8; 3],
        payload: Vec<u8>,
        transport: &DeviceReadCapability,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        report: &mut MacroWriteReport,
    ) -> Result<Vec<u8>> {
        self.sized_step(
            operation,
            command,
            payload,
            command[0],
            transport,
            transactions,
            validate,
            report,
        )
    }

    fn sized_step(
        &self,
        operation: &str,
        command: [u8; 3],
        payload: Vec<u8>,
        report_data_size: u8,
        transport: &DeviceReadCapability,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        report: &mut MacroWriteReport,
    ) -> Result<Vec<u8>> {
        validate()?;
        let transaction = transactions()?;
        validate()?;
        let index = report.steps.len();
        report.steps.push(MacroBlock {
            operation: operation.to_owned(),
            command,
            report_data_size,
            payload,
            attempted: true,
            acknowledgment_received: false,
            acknowledgment_data: Vec::new(),
        });
        let block = &mut report.steps[index];
        let result = exchange_with_report_data_size(
            self.device,
            transport,
            command,
            &block.payload,
            transaction,
            report_data_size,
            validate,
        );
        if let Ok(data) = &result {
            block.acknowledgment_received = true;
            block.acknowledgment_data = data.clone();
        }
        result
    }

    pub fn read_storage_info(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroStorageInfo> {
        let raw = self.query(
            &self.transport,
            STORAGE,
            &[],
            STORAGE[0],
            transactions,
            validate,
        )?;
        ensure!(raw.len() >= 14, "truncated macro storage information");
        Ok(MacroStorageInfo {
            max_macro_id: u16::from_be_bytes(raw[..2].try_into().unwrap()),
            max_memory: u32::from_be_bytes(raw[2..6].try_into().unwrap()),
            available_memory: u32::from_be_bytes(raw[6..10].try_into().unwrap()),
            recyclable_memory: u32::from_be_bytes(raw[10..14].try_into().unwrap()),
        })
    }

    fn read_count(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<u16> {
        let raw = self.query(
            &self.transport,
            COUNT,
            &[],
            COUNT[0],
            transactions,
            validate,
        )?;
        ensure!(raw.len() >= 2, "truncated macro count");
        Ok(u16::from_be_bytes(raw[..2].try_into().unwrap()))
    }

    fn read_id_page(
        &self,
        offset: u16,
        count: usize,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroIdPage> {
        ensure!(
            (1..=MAX_ID_PAGE).contains(&count),
            "invalid macro ID page size"
        );
        let mut payload = Vec::with_capacity(2);
        payload.extend_from_slice(&be16(offset));
        let report_size = u8::try_from(4 + count * 2).context("macro ID page report size")?;
        let raw = self.query(
            &self.transport,
            ID_LIST,
            &payload,
            report_size,
            transactions,
            validate,
        )?;
        ensure!(raw.len() >= 4 + count * 2, "truncated macro ID page");
        let returned_offset = u16::from_be_bytes(raw[..2].try_into().unwrap());
        let returned_size = u16::from_be_bytes(raw[2..4].try_into().unwrap());
        ensure!(returned_offset == offset, "macro ID page offset differs");
        let ids = raw[4..4 + count * 2]
            .chunks_exact(2)
            .map(|bytes| u16::from_be_bytes(bytes.try_into().unwrap()))
            .collect();
        Ok(MacroIdPage {
            offset: returned_offset,
            returned_size,
            ids,
            raw,
        })
    }

    pub fn read_macro_inventory(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroInventory> {
        let storage = self.read_storage_info(transactions, validate)?;
        self.read_ids_with_storage(storage, transactions, validate)
    }

    fn read_ids_with_storage(
        &self,
        storage: MacroStorageInfo,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroInventory> {
        let count = self.read_count(transactions, validate)?;
        let mut ids = Vec::with_capacity(usize::from(count));
        let mut pages = Vec::new();
        let mut offset = 0usize;
        while offset < usize::from(count) {
            let page = (usize::from(count) - offset).min(MAX_ID_PAGE);
            let page = self.read_id_page(
                u16::try_from(offset).context("macro ID page offset exceeds u16")?,
                page,
                transactions,
                validate,
            )?;
            offset += page.ids.len();
            ids.extend_from_slice(&page.ids);
            pages.push(page);
        }
        ensure!(
            ids.len() == usize::from(count),
            "macro ID count differs from pages"
        );
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        ensure!(
            unique.len() == ids.len(),
            "macro ID list contains duplicates"
        );
        Ok(MacroInventory {
            storage,
            count,
            ids,
            pages,
        })
    }

    fn read_metadata_raw(
        &self,
        macro_id: u16,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        blocks: &mut Vec<MacroReadBlock>,
    ) -> Result<(Vec<u8>, Vec<MacroReadBlock>)> {
        let mut raw = Vec::with_capacity(METADATA_BYTES);
        let start = blocks.len();
        for offset in (0..METADATA_BYTES).step_by(METADATA_CHUNK) {
            let count = (METADATA_BYTES - offset).min(METADATA_CHUNK);
            let mut payload = Vec::with_capacity(4);
            payload.extend_from_slice(&be16(macro_id));
            payload.extend_from_slice(&be16(u16::try_from(offset).unwrap()));
            let response = self.query(
                &self.metadata_transport,
                GET_NAME,
                &payload,
                u8::try_from(count + 6).unwrap(),
                transactions,
                validate,
            )?;
            ensure!(response.len() >= 6, "truncated macro metadata block header");
            let returned_id = u16::from_be_bytes(response[..2].try_into().unwrap());
            let returned_offset = u16::from_be_bytes(response[2..4].try_into().unwrap());
            let returned_size = u16::from_be_bytes(response[4..6].try_into().unwrap());
            blocks.push(MacroReadBlock {
                offset: u32::from(returned_offset),
                returned_size,
                data: response[6..].to_vec(),
                raw_response: response.clone(),
            });
            ensure!(returned_id == macro_id, "macro metadata reply ID differs");
            ensure!(
                returned_offset == offset as u16,
                "macro metadata reply offset differs"
            );
            ensure!(
                response.len() >= 6 + count,
                "truncated macro metadata block"
            );
            raw.extend_from_slice(&response[6..6 + count]);
        }
        Ok((raw, blocks[start..].to_vec()))
    }

    pub fn read_macro_metadata(
        &self,
        macro_id: u16,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroMetadata> {
        self.read_macro_metadata_with_progress(macro_id, transactions, validate, &mut Vec::new())
    }

    fn read_macro_metadata_with_progress(
        &self,
        macro_id: u16,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        progress: &mut Vec<MacroReadBlock>,
    ) -> Result<MacroMetadata> {
        let (raw, blocks) = self.read_metadata_raw(macro_id, transactions, validate, progress)?;
        let mut metadata = decode_metadata(macro_id, raw)?;
        metadata.blocks = blocks;
        Ok(metadata)
    }

    fn read_data_size(
        &self,
        macro_id: u16,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<u32> {
        let mut payload = Vec::with_capacity(2);
        payload.extend_from_slice(&be16(macro_id));
        let raw = self.query(
            &self.transport,
            GET_SIZE,
            &payload,
            GET_SIZE[0],
            transactions,
            validate,
        )?;
        ensure!(raw.len() >= 6, "truncated macro data size");
        ensure!(
            u16::from_be_bytes(raw[..2].try_into().unwrap()) == macro_id,
            "macro data size ID differs"
        );
        Ok(u32::from_be_bytes(raw[2..6].try_into().unwrap()))
    }

    fn read_data_bytes(
        &self,
        macro_id: u16,
        size: u32,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        blocks: &mut Vec<MacroReadBlock>,
    ) -> Result<Vec<u8>> {
        let size_usize = usize::try_from(size).context("macro data size cannot fit usize")?;
        let mut data = Vec::with_capacity(size_usize);
        let mut offset = 0usize;
        while offset < size_usize {
            let count = (size_usize - offset).min(DATA_CHUNK);
            let mut payload = Vec::with_capacity(7);
            payload.extend_from_slice(&be16(macro_id));
            payload.extend_from_slice(&be32(u32::try_from(offset).context("macro data offset")?));
            payload.push(u8::try_from(count).unwrap());
            let response = self.query(
                &self.transport,
                GET_MEMORY,
                &payload,
                u8::try_from(count + 7).unwrap(),
                transactions,
                validate,
            )?;
            ensure!(response.len() >= 7, "truncated macro memory block header");
            let returned_id = u16::from_be_bytes(response[..2].try_into().unwrap());
            let returned_offset = u32::from_be_bytes(response[2..6].try_into().unwrap());
            let returned_size = usize::from(response[6]);
            blocks.push(MacroReadBlock {
                offset: returned_offset,
                returned_size: u16::from(response[6]),
                data: response[7..].to_vec(),
                raw_response: response.clone(),
            });
            ensure!(returned_id == macro_id, "macro memory reply ID differs");
            ensure!(
                returned_offset == offset as u32,
                "macro memory reply offset differs"
            );
            ensure!(returned_size == count, "macro memory block size differs");
            ensure!(response.len() >= 7 + count, "truncated macro memory block");
            data.extend_from_slice(&response[7..7 + count]);
            offset += count;
        }
        Ok(data)
    }

    pub fn read_macro(
        &self,
        macro_id: u16,
        max_data_bytes: u32,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroReading> {
        let metadata = self.read_macro_metadata(macro_id, transactions, validate)?;
        let data_size = self.read_data_size(macro_id, transactions, validate)?;
        ensure!(
            data_size <= max_data_bytes,
            "macro data exceeds caller read bound"
        );
        let mut data_blocks = Vec::new();
        let data = self.read_data_bytes(
            macro_id,
            data_size,
            transactions,
            validate,
            &mut data_blocks,
        )?;
        Ok(MacroReading {
            metadata,
            data,
            data_size,
            data_blocks,
        })
    }

    /// Complete source startup macro-loader sequence. It reads no content blocks;
    /// `read_macro` retrieves the detailed firmware byte stream separately.
    pub fn load_macros(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<MacroCatalog> {
        let inventory = self.read_macro_inventory(transactions, validate)?;
        let mut macros = Vec::with_capacity(inventory.ids.len());
        for id in &inventory.ids {
            let metadata = self.read_macro_metadata(*id, transactions, validate)?;
            let data_size = self.read_data_size(*id, transactions, validate)?;
            let allocated_size_with_name = required_memory(data_size as usize)?;
            macros.push(MacroSummary {
                metadata,
                data_size,
                allocated_size_with_name,
            });
        }
        Ok(MacroCatalog { inventory, macros })
    }

    fn allocate(
        &self,
        macro_id: u16,
        data_size: u32,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        report: &mut MacroWriteReport,
    ) -> Result<Vec<u8>> {
        let mut payload = Vec::with_capacity(6);
        payload.extend_from_slice(&be16(macro_id));
        payload.extend_from_slice(&be32(data_size));
        self.step(
            "allocate_macro_data_size",
            ALLOCATE,
            payload,
            &self.transport,
            transactions,
            validate,
            report,
        )
    }

    fn write_metadata(
        &self,
        macro_id: u16,
        metadata: &[u8],
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        report: &mut MacroWriteReport,
    ) -> Result<()> {
        ensure!(
            metadata.len() == METADATA_BYTES,
            "metadata encoder returned wrong size"
        );
        for offset in (0..METADATA_BYTES).step_by(METADATA_CHUNK) {
            let count = (METADATA_BYTES - offset).min(METADATA_CHUNK);
            let mut payload = Vec::with_capacity(count + 6);
            payload.extend_from_slice(&be16(macro_id));
            payload.extend_from_slice(&be16(u16::try_from(offset).unwrap()));
            payload.extend_from_slice(&be16(METADATA_BYTES as u16));
            payload.extend_from_slice(&metadata[offset..offset + count]);
            self.sized_step(
                "set_macro_name",
                SET_NAME,
                payload,
                u8::try_from(count + 6).unwrap(),
                &self.metadata_write_transport,
                transactions,
                validate,
                report,
            )?;
        }
        Ok(())
    }

    fn write_data(
        &self,
        macro_id: u16,
        data: &[u8],
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        report: &mut MacroWriteReport,
    ) -> Result<()> {
        let mut offset = 0usize;
        while offset < data.len() {
            let count = (data.len() - offset).min(DATA_CHUNK);
            let mut payload = Vec::with_capacity(count + 7);
            payload.extend_from_slice(&be16(macro_id));
            payload.extend_from_slice(&be32(u32::try_from(offset).context("macro data offset")?));
            payload.push(u8::try_from(count).unwrap());
            payload.extend_from_slice(&data[offset..offset + count]);
            self.sized_step(
                "set_macro_memory_data",
                SET_MEMORY,
                payload,
                u8::try_from(count + 7).unwrap(),
                &self.write_transport,
                transactions,
                validate,
                report,
            )?;
            offset += count;
        }
        Ok(())
    }

    fn failure(
        phase: MacroFailurePhase,
        progress: MacroWriteReport,
        failed_step: Option<usize>,
        error: anyhow::Error,
    ) -> MacroWriteFailure {
        MacroWriteFailure {
            phase,
            progress,
            failed_step,
            error,
        }
    }

    pub fn write_macro(
        &self,
        macro_id: u16,
        request: &MacroWriteRequest,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<MacroWriteReport, MacroWriteFailure> {
        let report = MacroWriteReport {
            macro_id,
            ..Default::default()
        };
        let metadata = match encode_metadata(request) {
            Ok(value) => value,
            Err(error) => {
                return Err(Self::failure(
                    MacroFailurePhase::Prepare,
                    report,
                    None,
                    error,
                ));
            }
        };
        if AUDITED_KEYBOARD_PRODUCTS.contains(&self.product_id()) {
            return Err(Self::failure(
                MacroFailurePhase::Prepare,
                report,
                None,
                anyhow::anyhow!("Keyboard macro upload requires its calculated storage owner"),
            ));
        }
        self.write_raw(
            macro_id,
            metadata,
            &request.data,
            None,
            transactions,
            validate,
        )
    }

    pub(crate) fn write_keyboard_macro(
        &self,
        macro_id: u16,
        request: &MacroWriteRequest,
        memory: &mut KeyboardMacroMemory,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<MacroWriteReport, MacroWriteFailure> {
        let prepared = (|| -> Result<_> {
            ensure!(
                AUDITED_KEYBOARD_PRODUCTS.contains(&self.product_id()),
                "Calculated macro storage requires the keyboard adapter"
            );
            Ok(encode_metadata(request)?)
        })();
        let metadata = prepared.map_err(|error| {
            Self::failure(
                MacroFailurePhase::Prepare,
                MacroWriteReport {
                    macro_id,
                    ..Default::default()
                },
                None,
                error,
            )
        })?;
        self.write_raw(
            macro_id,
            metadata,
            &request.data,
            Some(memory),
            transactions,
            validate,
        )
    }

    /// Restore an observed macro without the source editor codec's lossy name,
    /// timestamp, hash or event conversion. The same observed-inventory guards
    /// and independent byte readbacks as an ordinary upload still apply.
    pub fn restore_macro(
        &self,
        observed: &MacroReading,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<MacroWriteReport, MacroWriteFailure> {
        let macro_id = observed.metadata.macro_id;
        let prepared = (|| {
            ensure!(
                observed.data_size as usize == observed.data.len(),
                "Observed macro size disagrees with its bytes"
            );
            let decoded = decode_metadata(macro_id, observed.metadata.raw.clone())?;
            ensure!(
                decoded.guid == observed.metadata.guid
                    && decoded.num_items == observed.metadata.num_items,
                "Observed macro metadata disagrees with its raw bytes"
            );
            Ok(())
        })();
        if let Err(error) = prepared {
            return Err(Self::failure(
                MacroFailurePhase::Prepare,
                MacroWriteReport {
                    macro_id,
                    ..Default::default()
                },
                None,
                error,
            ));
        }
        self.write_raw(
            macro_id,
            observed.metadata.raw.clone(),
            &observed.data,
            None,
            transactions,
            validate,
        )
    }

    fn write_raw(
        &self,
        macro_id: u16,
        metadata: Vec<u8>,
        data: &[u8],
        mut keyboard_memory: Option<&mut KeyboardMacroMemory>,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<MacroWriteReport, MacroWriteFailure> {
        let mut report = MacroWriteReport {
            macro_id,
            ..Default::default()
        };
        if macro_id > 32767 {
            return Err(Self::failure(
                MacroFailurePhase::Prepare,
                report,
                None,
                anyhow::anyhow!("source macro allocator uses 15-bit IDs"),
            ));
        }
        let required = match required_memory(data.len()) {
            Ok(value) => value,
            Err(error) => {
                return Err(Self::failure(
                    MacroFailurePhase::Prepare,
                    report,
                    None,
                    error,
                ));
            }
        };
        if let Err(error) = validate() {
            return Err(Self::failure(
                MacroFailurePhase::Cancelled,
                report,
                None,
                error,
            ));
        }
        let inventory = match self.read_macro_inventory(transactions, validate) {
            Ok(value) => value,
            Err(error) => {
                return Err(Self::failure(
                    MacroFailurePhase::Observe,
                    report,
                    None,
                    error,
                ));
            }
        };
        report.storage_before = Some(inventory.storage.clone());
        report.inventory_before = Some(inventory.clone());
        if inventory.ids.contains(&macro_id) {
            return Err(Self::failure(
                MacroFailurePhase::Prepare,
                report,
                None,
                anyhow::anyhow!("macro ID already exists in observed device inventory"),
            ));
        }
        if inventory.count == u16::MAX {
            return Err(Self::failure(
                MacroFailurePhase::Prepare,
                report,
                None,
                anyhow::anyhow!("observed macro count cannot represent a new entry"),
            ));
        }
        let updated_keyboard_memory = if let Some(memory) = keyboard_memory.as_ref() {
            match memory.after_upload(required) {
                Ok(Some(updated)) => Some(updated),
                Ok(None) => {
                    return Err(Self::failure(
                        MacroFailurePhase::InsufficientStorage,
                        report,
                        None,
                        anyhow::anyhow!("insufficient calculated keyboard macro storage space"),
                    ));
                }
                Err(error) => {
                    return Err(Self::failure(
                        MacroFailurePhase::Prepare,
                        report,
                        None,
                        error,
                    ));
                }
            }
        } else {
            None
        };
        let total = inventory
            .storage
            .available_memory
            .checked_add(inventory.storage.recyclable_memory)
            .context("macro storage total overflow")
            .map_err(|error| {
                Self::failure(MacroFailurePhase::Prepare, report.clone(), None, error)
            })?;
        if updated_keyboard_memory.is_none() && required > total {
            return Err(Self::failure(
                MacroFailurePhase::Prepare,
                report,
                None,
                anyhow::anyhow!("insufficient macro storage space"),
            ));
        }
        if updated_keyboard_memory.is_none() && required > inventory.storage.available_memory {
            return Err(Self::failure(
                MacroFailurePhase::CompactionRequired,
                report,
                None,
                anyhow::anyhow!("macro storage requires source defragmentation before upload"),
            ));
        }
        if let Err(error) = self.allocate(
            macro_id,
            data.len() as u32,
            transactions,
            validate,
            &mut report,
        ) {
            let failed_step = report
                .steps
                .iter()
                .rposition(|step| !step.acknowledgment_received);
            return Err(Self::failure(
                if validate().is_err() {
                    MacroFailurePhase::Cancelled
                } else {
                    MacroFailurePhase::Allocate
                },
                report,
                failed_step,
                error,
            ));
        }
        if let Err(error) =
            self.write_metadata(macro_id, &metadata, transactions, validate, &mut report)
        {
            let failed_step = report
                .steps
                .iter()
                .rposition(|step| !step.acknowledgment_received);
            return Err(Self::failure(
                if validate().is_err() {
                    MacroFailurePhase::Cancelled
                } else {
                    MacroFailurePhase::Metadata
                },
                report,
                failed_step,
                error,
            ));
        }
        if let Err(error) = self.write_data(macro_id, data, transactions, validate, &mut report) {
            let failed_step = report
                .steps
                .iter()
                .rposition(|step| !step.acknowledgment_received);
            return Err(Self::failure(
                if validate().is_err() {
                    MacroFailurePhase::Cancelled
                } else {
                    MacroFailurePhase::Data
                },
                report,
                failed_step,
                error,
            ));
        }
        report.storage_after = match self.read_storage_info(transactions, validate) {
            Ok(value) => Some(value),
            Err(error) => {
                return Err(Self::failure(
                    if validate().is_err() {
                        MacroFailurePhase::Cancelled
                    } else {
                        MacroFailurePhase::StorageRefresh
                    },
                    report,
                    None,
                    error,
                ));
            }
        };
        let readback = (|| -> Result<()> {
            let metadata_readback = self.read_macro_metadata_with_progress(
                macro_id,
                transactions,
                validate,
                &mut report.metadata_readback_blocks,
            )?;
            let metadata_matches = metadata_readback.raw == metadata;
            report.metadata_readback = Some(metadata_readback);
            ensure!(metadata_matches, "macro metadata readback differs");
            let data_size = self.read_data_size(macro_id, transactions, validate)?;
            report.data_size_readback = Some(data_size);
            ensure!(
                data_size == data.len() as u32,
                "macro data size readback differs"
            );
            let data_readback = self.read_data_bytes(
                macro_id,
                data_size,
                transactions,
                validate,
                &mut report.data_readback_blocks,
            )?;
            let data_matches = data_readback == data;
            report.data_readback = Some(data_readback);
            ensure!(data_matches, "macro data readback differs");
            let inventory_after = self.read_ids_with_storage(
                report
                    .storage_after
                    .clone()
                    .expect("Source storage refresh succeeded"),
                transactions,
                validate,
            )?;
            let mut expected_ids = inventory.ids.clone();
            expected_ids.push(macro_id);
            expected_ids.sort_unstable();
            let mut actual_ids = inventory_after.ids.clone();
            actual_ids.sort_unstable();
            report.inventory_after = Some(inventory_after);
            ensure!(
                expected_ids == actual_ids,
                "macro inventory differs after upload"
            );
            Ok(())
        })();
        if let Err(error) = readback {
            return Err(Self::failure(
                if validate().is_err() {
                    MacroFailurePhase::Cancelled
                } else {
                    MacroFailurePhase::Readback
                },
                report,
                None,
                error,
            ));
        }
        report.verified = true;
        if let (Some(memory), Some(updated)) = (keyboard_memory.as_mut(), updated_keyboard_memory) {
            **memory = updated;
        }
        Ok(report)
    }

    pub fn delete_macro(
        &self,
        macro_id: u16,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<MacroWriteReport, MacroWriteFailure> {
        let mut report = MacroWriteReport {
            macro_id,
            ..Default::default()
        };
        let before = match self.read_macro_inventory(transactions, validate) {
            Ok(value) => value,
            Err(error) => {
                return Err(Self::failure(
                    MacroFailurePhase::Observe,
                    report,
                    None,
                    error,
                ));
            }
        };
        report.storage_before = Some(before.storage.clone());
        report.inventory_before = Some(before.clone());
        if !before.ids.contains(&macro_id) {
            return Err(Self::failure(
                MacroFailurePhase::Observe,
                report,
                None,
                anyhow::anyhow!("macro ID is absent from observed device inventory"),
            ));
        }
        let mut payload = Vec::with_capacity(2);
        payload.extend_from_slice(&be16(macro_id));
        if let Err(error) = self.step(
            "delete_macro",
            DELETE,
            payload,
            &self.transport,
            transactions,
            validate,
            &mut report,
        ) {
            let failed_step = report
                .steps
                .iter()
                .rposition(|step| !step.acknowledgment_received);
            return Err(Self::failure(
                if validate().is_err() {
                    MacroFailurePhase::Cancelled
                } else {
                    MacroFailurePhase::Delete
                },
                report,
                failed_step,
                error,
            ));
        }
        report.storage_after = match self.read_storage_info(transactions, validate) {
            Ok(value) => Some(value),
            Err(error) => {
                return Err(Self::failure(
                    if validate().is_err() {
                        MacroFailurePhase::Cancelled
                    } else {
                        MacroFailurePhase::StorageRefresh
                    },
                    report,
                    None,
                    error,
                ));
            }
        };
        let after = (|| -> Result<()> {
            let inventory_after = self.read_ids_with_storage(
                report
                    .storage_after
                    .clone()
                    .expect("Source storage refresh succeeded"),
                transactions,
                validate,
            )?;
            let mut expected_ids: Vec<_> = before
                .ids
                .iter()
                .copied()
                .filter(|id| *id != macro_id)
                .collect();
            expected_ids.sort_unstable();
            let mut actual_ids = inventory_after.ids.clone();
            actual_ids.sort_unstable();
            report.inventory_after = Some(inventory_after);
            ensure!(
                expected_ids == actual_ids,
                "macro inventory does not confirm deletion"
            );
            Ok(())
        })();
        if let Err(error) = after {
            return Err(Self::failure(
                if validate().is_err() {
                    MacroFailurePhase::Cancelled
                } else {
                    MacroFailurePhase::Readback
                },
                report,
                None,
                error,
            ));
        }
        report.verified = true;
        Ok(report)
    }

    /// Expose the audited protocol method constants to source-aware callers.
    pub fn protocol_commands() -> &'static [([u8; 3], &'static str)] {
        &[
            (STORAGE, "getMacroStorageInfo"),
            (COUNT, "getNumberOfMacros"),
            (ID_LIST, "getMacroIDList"),
            (CREATE, "createNewMacro"),
            (DELETE, "deleteMacro"),
            (GET_NAME, "getMacroName"),
            (SET_NAME, "setMacroName"),
            (ALLOCATE, "allocateMacroDataSize"),
            (GET_MEMORY, "getMacroMemoryData"),
            (SET_MEMORY, "setMacroMemoryData"),
            (GET_DATA, "getMacroData"),
            (GET_SIZE, "getMacroDataSize"),
        ]
    }
}

#[cfg(test)]
#[path = "onboard_macro_tests.rs"]
mod tests;
