//! Current 182 DPI stage-table protocol, independent of the operating system.
//! Profile/version persistence and the original task queue belong to the caller.
//! A current-table readback does not prove that any onboard profile was saved.
use super::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Deserialize)]
pub struct MouseDpiStagesCapability {
    pub product_id: u32,
    pub source_class: String,
    pub get_command: [u8; 3],
    pub set_command: [u8; 3],
    pub active_profile: u8,
    pub editor_stage_counts: Vec<usize>,
    pub protocol_record_capacity: usize,
    pub min_dpi: u16,
    pub max_dpi: u16,
    pub dpi_step: u16,
    pub use_cycle_sensitivity: bool,
    pub transport: DeviceReadCapability,
}

pub fn capability(product_id: u32) -> Option<&'static MouseDpiStagesCapability> {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        products: Vec<MouseDpiStagesCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let catalog: Catalog = serde_json::from_str(include_str!(
                "../../../assets/data/mouse-dpi-stages-capabilities.json"
            ))
            .expect("current source mouse DPI stage capabilities");
            assert_eq!(catalog.schema_version, 1);
            catalog
        })
        .products
        .iter()
        .find(|cap| cap.product_id == product_id)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiStage {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
    /// Local profile/UI flag; the device stage table has no independent flag.
    pub independent: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiStagesDraft {
    pub enabled: bool,
    /// One-based position in the complete local list, including hidden rows.
    pub active_stage: usize,
    pub stages: Vec<DpiStage>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PackedDpiStage {
    /// Original setter uses zero-based indices, getter returns one-based ones.
    pub index: u8,
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PackedDpiStages {
    pub active_stage: u8,
    pub stages: Vec<PackedDpiStage>,
}

/// Reproduce Ke's visibility filtering and remapping, including disabled mode.
/// Reject a hidden/missing selection rather than fabricate the source's index 0.
/// Min/max and editor bounds are application validation of source-defined limits.
pub fn pack(
    cap: &MouseDpiStagesCapability,
    draft: &DpiStagesDraft,
) -> anyhow::Result<PackedDpiStages> {
    ensure!(
        !draft.stages.is_empty()
            && draft.stages.len() <= cap.editor_stage_counts.iter().copied().max().unwrap_or(0),
        "DPI stage draft exceeds the current editor capacity"
    );
    let selected = draft
        .active_stage
        .checked_sub(1)
        .filter(|index| *index < draft.stages.len())
        .context("DPI stage selection is outside the local profile")?;
    ensure!(
        draft.stages[selected].visible,
        "Selected DPI stage is hidden"
    );
    let enabled = cap.use_cycle_sensitivity || draft.enabled;
    let visible: Vec<_> = draft
        .stages
        .iter()
        .enumerate()
        .filter(|(index, row)| row.visible && (enabled || *index == selected))
        .collect();
    ensure!(
        !enabled || cap.editor_stage_counts.contains(&visible.len()),
        "DPI stage count is outside the current editor choices"
    );
    ensure!(
        !visible.is_empty() && visible.len() <= cap.protocol_record_capacity,
        "DPI stage count exceeds the proven protocol capacity"
    );
    let active_stage = visible
        .iter()
        .position(|(index, _)| *index == selected)
        .context("Selected DPI stage was not packed")? as u8
        + 1;
    let mut stages = Vec::with_capacity(visible.len());
    for (ordinal, (_, row)) in visible.into_iter().enumerate() {
        ensure!(
            (cap.min_dpi..=cap.max_dpi).contains(&row.x)
                && (cap.min_dpi..=cap.max_dpi).contains(&row.y),
            "DPI value is outside the current product range"
        );
        stages.push(PackedDpiStage {
            index: ordinal as u8,
            x: row.x,
            y: row.y,
            z: 0,
        });
    }
    Ok(PackedDpiStages {
        active_stage,
        stages,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiStagesReading {
    pub profile_id: u8,
    pub active_stage: u8,
    pub number_of_active_stages: u8,
    /// Preserve every returned record, including index-zero padding/terminators.
    /// These indices are one-based; do not feed them straight into the setter.
    pub records: Vec<PackedDpiStage>,
    pub response_bytes: usize,
}

fn parse(data: &[u8], profile: u8) -> anyhow::Result<DpiStagesReading> {
    ensure!(
        data.len() >= 3 && data.len() <= 80 && (data.len() - 3).is_multiple_of(7),
        "DPI stage response contains truncated records"
    );
    ensure!(
        data[0] == profile,
        "DPI stage response profile does not match"
    );
    let records = data[3..]
        .chunks_exact(7)
        .map(|row| PackedDpiStage {
            index: row[0],
            x: u16::from_be_bytes([row[1], row[2]]),
            y: u16::from_be_bytes([row[3], row[4]]),
            z: u16::from_be_bytes([row[5], row[6]]),
        })
        .collect();
    Ok(DpiStagesReading {
        profile_id: data[0],
        active_stage: data[1],
        number_of_active_stages: data[2],
        records,
        response_bytes: data.len(),
    })
}

pub fn same_table(table: &PackedDpiStages, reading: &DpiStagesReading) -> bool {
    let records: Vec<_> = reading
        .records
        .iter()
        .take_while(|row| row.index != 0)
        .collect();
    reading.active_stage == table.active_stage
        && usize::from(reading.number_of_active_stages) == table.stages.len()
        && records.len() == table.stages.len()
        && table.stages.iter().zip(records).all(|(sent, got)| {
            got.index == sent.index + 1 && (got.x, got.y, got.z) == (sent.x, sent.y, sent.z)
        })
}

/// Validate typed IPC observations without discarding the source padding.
/// Consistency/range rejection is application policy, not invented device data.
pub fn validate_current_reading(
    cap: &MouseDpiStagesCapability,
    reading: &DpiStagesReading,
) -> anyhow::Result<()> {
    let rows: Vec<_> = reading
        .records
        .iter()
        .take_while(|row| row.index != 0)
        .collect();
    ensure!(
        reading.profile_id == cap.active_profile
            && reading.response_bytes >= 3
            && reading.response_bytes <= 80
            && (reading.response_bytes - 3).is_multiple_of(7)
            && reading.records.len() == (reading.response_bytes - 3) / 7
            && rows.len() == usize::from(reading.number_of_active_stages)
            && !rows.is_empty()
            && rows.len() <= cap.protocol_record_capacity
            && reading.active_stage > 0
            && usize::from(reading.active_stage) <= rows.len(),
        "DPI stage observation has inconsistent profile, active selection or record count"
    );
    ensure!(
        rows.iter()
            .enumerate()
            .all(|(index, row)| row.index as usize == index + 1
                && (cap.min_dpi..=cap.max_dpi).contains(&row.x)
                && (cap.min_dpi..=cap.max_dpi).contains(&row.y)),
        "DPI stage observation contains invalid indices or values"
    );
    ensure!(
        reading
            .records
            .iter()
            .skip(rows.len())
            .all(|row| row.index == 0),
        "DPI stage observation contains nonzero records after its terminator"
    );
    Ok(())
}

/// Caller retains the uniquely observed transport, process lock and transaction
/// generator; validation checks identity, cancellation and deadline around I/O.
struct WireResponse {
    data: Vec<u8>,
    response_bytes: usize,
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    command: [u8; 3],
    payload: &[u8],
    packet_size: u8,
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<WireResponse> {
    let transport = &cap.transport;
    ensure!(
        transport.source_class == cap.source_class
            && transport.product_id == cap.product_id
            && transport.report_bytes == 91
            && transport.report_id == 0
            && command[0] == 80
            && packet_size <= 80
            && payload.len() <= 80,
        "DPI stage source transport layout does not match"
    );
    ensure!(
        transaction & 0xE0 == transport.transaction_prefix
            && transaction & 0x1F < transport.transaction_modulus,
        "DPI stage transaction is outside the source namespace"
    );
    let mut outgoing = vec![0; transport.report_bytes];
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    // Original inline setter passes 7*count+3 to sendCommand, overriding 80.
    outgoing[6] = packet_size;
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last_error = "No DPI stage response received".to_owned();
    for _ in 0..transport.max_retry_out {
        std::thread::sleep(Duration::from_millis(transport.sleep_between_out_ms));
        validate()?;
        let sent = device.send_feature(&outgoing);
        validate()?;
        if let Err(error) = sent {
            last_error = format!("{error:#}");
            continue;
        }
        std::thread::sleep(Duration::from_millis(transport.sleep_between_out_in_ms));
        for attempt in 0..transport.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(transport.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = vec![0; transport.report_bytes];
            let received = device.get_feature(&mut incoming);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(error) => {
                    last_error = format!("{error:#}");
                    break;
                }
            };
            ensure!(
                count == 91 && incoming[0] == 0,
                "DPI stage response report is truncated"
            );
            let packet = &incoming[1..];
            if packet[1] != transaction || packet[6..8] != command[1..] {
                last_error = "DPI stage response transaction or command does not match".into();
                break;
            }
            match packet[0] {
                1 => {
                    last_error = "DPI stage device is busy".into();
                    continue;
                }
                2 => {}
                0 | 3 | 4 => {
                    last_error = format!("DPI stage response status {}", packet[0]);
                    break;
                }
                state => anyhow::bail!(
                    "DPI stage command is unsupported or returned unknown status {state}"
                ),
            }
            let len = usize::from(packet[5]);
            ensure!(
                len <= 80,
                "DPI stage response data exceeds the protocol buffer"
            );
            return Ok(WireResponse {
                data: packet[8..88].to_vec(),
                response_bytes: len,
            });
        }
    }
    anyhow::bail!("DPI stage command retries failed: {last_error}")
}

fn read_profile(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    profile: u8,
    next_transaction: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiStagesReading> {
    let data = exchange(
        device,
        cap,
        cap.get_command,
        &[profile],
        80,
        next_transaction()?,
        validate,
    )?;
    parse(&data.data[..data.response_bytes], profile)
}

fn set_profile(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    profile: u8,
    table: &PackedDpiStages,
    next_transaction: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiStagesAcknowledgement> {
    ensure!(
        !table.stages.is_empty()
            && table.stages.len() <= cap.protocol_record_capacity
            && table.active_stage > 0
            && usize::from(table.active_stage) <= table.stages.len(),
        "DPI stage packed table is invalid"
    );
    let mut data = vec![profile, table.active_stage, table.stages.len() as u8];
    for (index, row) in table.stages.iter().enumerate() {
        ensure!(
            row.index as usize == index && row.z == 0,
            "DPI stage packed indices are invalid"
        );
        data.push(row.index);
        data.extend_from_slice(&row.x.to_be_bytes());
        data.extend_from_slice(&row.y.to_be_bytes());
        data.extend_from_slice(&row.z.to_be_bytes());
    }
    let response = exchange(
        device,
        cap,
        cap.set_command,
        &data,
        data.len() as u8,
        next_transaction()?,
        validate,
    )?;
    // Original Xe/Mt ignore setter jsonData; status/transaction/command prove
    // acknowledgement. Only the subsequent getter confirms the stage table.
    Ok(DpiStagesAcknowledgement {
        response_bytes: response.response_bytes,
        raw_data: response.data,
    })
}

pub fn read_current(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiStagesReading> {
    validate()?;
    let reading = read_profile(
        device,
        cap,
        cap.active_profile,
        &mut next_transaction,
        &validate,
    )?;
    validate_current_reading(cap, &reading)?;
    Ok(reading)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DpiStagesAcknowledgement {
    pub response_bytes: usize,
    /// Keep the entire zero-padded source response buffer; ack is not a read.
    pub raw_data: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DpiStagesWriteResult {
    pub requested: DpiStagesDraft,
    pub packed: PackedDpiStages,
    pub acknowledged: DpiStagesAcknowledgement,
    pub observed: DpiStagesReading,
    pub verified: bool,
}

/// Source Xe writes current profile then reads it back, with no read-before skip.
/// Full active/count/record-count confirmation is stricter application policy:
/// the source current task compares records only; Mt also compares active/count.
pub fn apply_current(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    draft: &DpiStagesDraft,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiStagesWriteResult> {
    let packed = pack(cap, draft)?;
    validate()?;
    let acknowledged = set_profile(
        device,
        cap,
        cap.active_profile,
        &packed,
        &mut next_transaction,
        &validate,
    )
    .context("DPI stage write is unconfirmed; the device may have accepted it, read it again")?;
    validate().context("DPI stage session changed after write; final state is unconfirmed")?;
    let observed = read_profile(
        device,
        cap,
        cap.active_profile,
        &mut next_transaction,
        &validate,
    )
    .context("DPI stage write was acknowledged but readback failed; final state is unconfirmed")?;
    ensure!(
        same_table(&packed, &observed),
        "DPI stage readback does not match the requested table"
    );
    validate()?;
    Ok(DpiStagesWriteResult {
        requested: draft.clone(),
        packed,
        acknowledged,
        observed,
        verified: true,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ObservedProfileSlot {
    /// Actual source OBMSlotID; no fabricated numeric slot offset/range.
    pub slot_id: u8,
    pub guid: String,
}

/// Original Ye preserves metadata order and all entries matching active GUID.
pub fn select_profile_slots<'a>(
    active_guid: &str,
    observed: &'a [ObservedProfileSlot],
) -> Vec<&'a ObservedProfileSlot> {
    observed
        .iter()
        .filter(|slot| slot.guid == active_guid)
        .collect()
}

#[derive(Debug, Serialize)]
pub struct DpiSlotWriteResult {
    pub slot_id: u8,
    pub was_equal: bool,
    pub before: Option<DpiStagesReading>,
    pub acknowledged: Option<DpiStagesAcknowledgement>,
    /// Original Mt continues other slots and reports collected errors.
    pub error: Option<String>,
}

/// Source Mt queries every actual matching slot and conditionally writes it.
/// The source has no post-set readback here. An ack is not verified persistence.
/// Caller must handle every returned error and retain partially written state.
pub fn synchronize_profile_slots(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    active_guid: &str,
    observed_slots: &[ObservedProfileSlot],
    draft: &DpiStagesDraft,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<DpiSlotWriteResult>> {
    let table = pack(cap, draft)?;
    let mut results = Vec::new();
    for slot in select_profile_slots(active_guid, observed_slots) {
        validate()?;
        let mut result = DpiSlotWriteResult {
            slot_id: slot.slot_id,
            was_equal: false,
            before: None,
            acknowledged: None,
            error: None,
        };
        match read_profile(device, cap, slot.slot_id, &mut next_transaction, &validate) {
            Err(error) => result.error = Some(format!("{error:#}")),
            Ok(before) => {
                result.was_equal = same_table(&table, &before);
                result.before = Some(before);
                if !result.was_equal {
                    validate()?;
                    match set_profile(
                        device,
                        cap,
                        slot.slot_id,
                        &table,
                        &mut next_transaction,
                        &validate,
                    ) {
                        Ok(ack) => result.acknowledged = Some(ack),
                        Err(error) => result.error = Some(format!("{error:#}")),
                    }
                }
            }
        }
        // Cancellation/identity loss must prevent subsequent slot writes.
        validate()?;
        results.push(result);
    }
    Ok(results)
}
