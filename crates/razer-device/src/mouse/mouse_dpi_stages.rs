//! Current source-gated direct mouse DPI stage tables, independent of the OS.
//! Profile/version persistence and the original task queue belong to the caller.
//! A current-table readback does not prove that any onboard profile was saved.
use super::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

#[derive(Clone, Debug)]
pub struct MouseDpiStagesCapability {
    pub product_id: u32,
    pub source_class: &'static str,
    pub get_command: [u8; 3],
    pub set_command: [u8; 3],
    pub active_profile: u8,
    pub editor_stage_counts: &'static [usize],
    pub protocol_record_capacity: usize,
    pub min_dpi: u16,
    pub max_dpi: u16,
    pub dpi_step: u16,
    pub use_cycle_sensitivity: bool,
    pub transport: DeviceReadCapability,
}

impl MouseDpiStagesCapability {
    /// Keep the product's DPI command owner while selecting its actual sender.
    pub fn for_physical_product(&self, product_id: u32) -> anyhow::Result<Self> {
        let mut bound = self.clone();
        bound.transport = self.transport.for_physical_product(product_id)?;
        Ok(bound)
    }
}

struct DpiStagesDescriptor {
    product_id: u32,
    min_dpi: u16,
    max_dpi: u16,
    dpi_step: u16,
}

include!("mouse_dpi_stages_data_generated.rs");

pub fn capability(product_id: u32) -> Option<&'static MouseDpiStagesCapability> {
    static CATALOG: OnceLock<Vec<MouseDpiStagesCapability>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            AUDITED_CAPABILITIES
                .iter()
                .map(|source| MouseDpiStagesCapability {
                    product_id: source.product_id,
                    source_class: "rzDevice25DualLinkMouse",
                    get_command: [80, 4, 134],
                    set_command: [80, 4, 6],
                    active_profile: 1,
                    editor_stage_counts: &[2, 3, 4, 5],
                    protocol_record_capacity: 11,
                    min_dpi: source.min_dpi,
                    max_dpi: source.max_dpi,
                    dpi_step: source.dpi_step,
                    use_cycle_sensitivity: false,
                    transport: super::device_reads::capability(source.product_id)
                        .expect("audited source mouse DPI transport")
                        .clone(),
                })
                .collect()
        })
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
    matches_packed_stage_records(
        table.active_stage,
        table.stages.len(),
        &table.stages,
        reading,
    )
}

/// Setter indices are zero-based; the source getter returns one-based records.
/// Declared count is independent of retained rows in ordinary mouse edge cases.
pub(crate) fn matches_packed_stage_records(
    active_stage: u8,
    number_of_active_stages: usize,
    packed: &[PackedDpiStage],
    reading: &DpiStagesReading,
) -> bool {
    let records: Vec<_> = reading
        .records
        .iter()
        .take_while(|row| row.index != 0)
        .collect();
    reading.active_stage == active_stage
        && usize::from(reading.number_of_active_stages) == number_of_active_stages
        && records.len() == packed.len()
        && packed.iter().zip(records).all(|(sent, got)| {
            got.index == sent.index + 1 && (got.x, got.y, got.z) == (sent.x, sent.y, sent.z)
        })
}

/// Validate typed IPC observations without discarding the source padding.
/// Consistency/range rejection is application policy, not invented device data.
pub fn validate_current_reading(
    cap: &MouseDpiStagesCapability,
    reading: &DpiStagesReading,
) -> anyhow::Result<()> {
    validate_profile_reading(cap, reading, cap.active_profile)
}

fn validate_profile_reading(
    cap: &MouseDpiStagesCapability,
    reading: &DpiStagesReading,
    profile: u8,
) -> anyhow::Result<()> {
    let rows: Vec<_> = reading
        .records
        .iter()
        .take_while(|row| row.index != 0)
        .collect();
    ensure!(
        reading.profile_id == profile
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
    exchange_observing(
        device,
        cap,
        command,
        payload,
        packet_size,
        transaction,
        validate,
        &mut |_| {},
    )
}

fn exchange_observing(
    device: &dyn FeatureTransport,
    cap: &MouseDpiStagesCapability,
    command: [u8; 3],
    payload: &[u8],
    packet_size: u8,
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    acknowledged: &mut dyn FnMut(&WireResponse),
) -> anyhow::Result<WireResponse> {
    let source = super::device_reads::capability(cap.product_id)
        .context("DPI stage product has no current source transport")?;
    let transport = &cap.transport;
    let logical = transport.source_class == source.source_class
        && transport.claim_interface == source.claim_interface
        && transport.report_id == source.report_id
        && transport.transaction_prefix == source.transaction_prefix;
    let physical = source.physical_transports_by_pid.values().any(|branch| {
        transport.source_class == branch.source_class
            && transport.claim_interface == branch.claim_interface
            && transport.report_id == branch.report_id
            && transport.transaction_prefix == branch.transaction_prefix
            && transport.transaction_modulus == branch.transaction_modulus
    });
    ensure!(
        cap.source_class == source.source_class
            && (logical || physical || crate::receiver_session::is_bound_master(transport, false))
            && transport.vendor_id == source.vendor_id
            && transport.product_id == cap.product_id
            && transport.report_bytes == 91
            && command[0] == 80
            && packet_size <= 80
            && payload.len() <= 80,
        "DPI stage source transport layout does not match"
    );
    let (transport, transaction) = transport.for_exchange(device, transaction)?;
    ensure!(
        transaction & 0xE0 == transport.transaction_prefix
            && transaction & 0x1F < transport.transaction_modulus,
        "DPI stage transaction is outside the source namespace"
    );
    let mut outgoing = vec![0; transport.report_bytes];
    outgoing[0] = transport.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    // Original inline setter passes 7*count+3 to sendCommand, overriding 80.
    outgoing[6] = packet_size;
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    crate::feature_exchange::with_validated_exchange(
        device,
        crate::feature_exchange::with_protocol25_scope(
            crate::backend::ExchangeOptions::mutex_out(Duration::from_millis(
                transport.sleep_between_out_ms,
            )),
            &transport.source_class,
        ),
        validate,
        |device, lease| {
            let mut last_error = "No DPI stage response received".to_owned();
            for out_attempt in 0..transport.max_retry_out {
                validate()?;
                let sent = device.send_feature_at_collection_max(&outgoing);
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
                    incoming[0] = transport.get_report_id_for_transport(device)?;
                    let received = device.get_feature_at_collection_max(&mut incoming);
                    validate()?;
                    let count = match received {
                        Ok(count) => count,
                        Err(error) => {
                            last_error = format!("{error:#}");
                            break;
                        }
                    };
                    ensure!(count == 91, "DPI stage response report is truncated");
                    let packet = &incoming[1..];
                    if packet[1] != transaction || packet[6..8] != command[1..] {
                        last_error =
                            "DPI stage response transaction or command does not match".into();
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
                    let response = WireResponse {
                        data: packet[8..88].to_vec(),
                        response_bytes: len,
                    };
                    acknowledged(&response);
                    return Ok(response);
                }
                if out_attempt + 1 < transport.max_retry_out {
                    lease.retry_release()?;
                }
            }
            anyhow::bail!("DPI stage command retries failed: {last_error}")
        },
    )
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
    let reading = parse(&data.data[..data.response_bytes], profile)?;
    validate_profile_reading(cap, &reading, profile)?;
    Ok(reading)
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
    let mut completed = None;
    let response = exchange_observing(
        device,
        cap,
        cap.set_command,
        &data,
        data.len() as u8,
        next_transaction()?,
        validate,
        &mut |response| {
            completed = Some(DpiStagesAcknowledgement {
                response_bytes: response.response_bytes,
                raw_data: response.data.clone(),
            });
        },
    );
    let response = response.map_err(|error| match completed {
        Some(acknowledged) => error.context(acknowledged),
        None => error,
    })?;
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiStagesAcknowledgement {
    pub response_bytes: usize,
    /// Keep the entire zero-padded source response buffer; ack is not a read.
    pub raw_data: Vec<u8>,
}

impl std::fmt::Display for DpiStagesAcknowledgement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DPI stage response was acknowledged before the task failed")
    }
}

pub(crate) fn completed_acknowledgement(
    result: &anyhow::Result<DpiStagesAcknowledgement>,
) -> Option<&DpiStagesAcknowledgement> {
    match result {
        Ok(acknowledged) => Some(acknowledged),
        Err(error) => error.downcast_ref::<DpiStagesAcknowledgement>(),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DpiStagesWriteFailure {
    pub requested: DpiStagesDraft,
    pub packed: PackedDpiStages,
    pub acknowledged: Option<DpiStagesAcknowledgement>,
    pub observed: Option<DpiStagesReading>,
}

impl std::fmt::Display for DpiStagesWriteFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .write_str("DPI stage write did not complete; partial responses are diagnostic only")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    let mut progress = DpiStagesWriteFailure {
        requested: draft.clone(),
        packed: pack(cap, draft)?,
        acknowledged: None,
        observed: None,
    };
    let result = (|| -> anyhow::Result<DpiStagesWriteResult> {
        validate()?;
        let response = set_profile(
            device,
            cap,
            cap.active_profile,
            &progress.packed,
            &mut next_transaction,
            &validate,
        );
        progress.acknowledged = completed_acknowledgement(&response).cloned();
        let acknowledged = response.context(
            "DPI stage write is unconfirmed; the device may have accepted it, read it again",
        )?;
        validate().context("DPI stage session changed after write; final state is unconfirmed")?;
        let observed = read_profile(
            device,
            cap,
            cap.active_profile,
            &mut next_transaction,
            &validate,
        )
        .context(
            "DPI stage write was acknowledged but readback failed; final state is unconfirmed",
        )?;
        progress.observed = Some(observed.clone());
        ensure!(
            same_table(&progress.packed, &observed),
            "DPI stage readback does not match the requested table"
        );
        validate()?;
        Ok(DpiStagesWriteResult {
            requested: progress.requested.clone(),
            packed: progress.packed.clone(),
            acknowledged,
            observed,
            verified: true,
        })
    })();
    result.map_err(|error| error.context(progress))
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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
    synchronize_profile_slots_with(
        active_guid,
        observed_slots,
        &mut next_transaction,
        &validate,
        |next, slot| read_profile(device, cap, slot, next, &validate),
        |before| same_table(&table, before),
        |next, slot| set_profile(device, cap, slot, &table, next, &validate),
    )
}

/// Shared Mt task order with product-owned packing and transport validation.
/// A failed slot is retained and later matching slots still run; cancellation
/// stops before any later write. Setter acknowledgements have no readback.
pub(crate) fn synchronize_profile_slots_with<C>(
    active_guid: &str,
    observed_slots: &[ObservedProfileSlot],
    context: &mut C,
    validate: &impl Fn() -> anyhow::Result<()>,
    read: impl FnMut(&mut C, u8) -> anyhow::Result<DpiStagesReading>,
    same: impl Fn(&DpiStagesReading) -> bool,
    write: impl FnMut(&mut C, u8) -> anyhow::Result<DpiStagesAcknowledgement>,
) -> anyhow::Result<Vec<DpiSlotWriteResult>> {
    let ids: Vec<_> = select_profile_slots(active_guid, observed_slots)
        .into_iter()
        .map(|slot| slot.slot_id)
        .collect();
    let progress = synchronize_profile_slot_ids_with(&ids, context, validate, read, same, write);
    if let Some(error) = progress.finalization {
        return Err(error.context(DpiSlotSynchronizationFailure {
            writes: progress.writes,
        }));
    }
    if let Some(error) = progress.interrupted {
        return Err(
            anyhow::anyhow!(error).context(DpiSlotSynchronizationFailure {
                writes: progress.writes,
            }),
        );
    }
    Ok(progress.writes)
}

pub(crate) struct DpiSlotSynchronization {
    pub writes: Vec<DpiSlotWriteResult>,
    pub interrupted: Option<String>,
    finalization: Option<anyhow::Error>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DpiSlotSynchronizationFailure {
    pub writes: Vec<DpiSlotWriteResult>,
}

impl std::fmt::Display for DpiSlotSynchronizationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DPI profile synchronization stopped before completion")
    }
}

/// Retain completed getter/setter outcomes when cancellation stops the next
/// slot. The owner decides whether to publish the partial diagnostic report.
pub(crate) fn synchronize_profile_slot_ids_with<C>(
    ids: &[u8],
    context: &mut C,
    validate: &impl Fn() -> anyhow::Result<()>,
    mut read: impl FnMut(&mut C, u8) -> anyhow::Result<DpiStagesReading>,
    same: impl Fn(&DpiStagesReading) -> bool,
    mut write: impl FnMut(&mut C, u8) -> anyhow::Result<DpiStagesAcknowledgement>,
) -> DpiSlotSynchronization {
    let mut progress = DpiSlotSynchronization {
        writes: vec![],
        interrupted: None,
        finalization: None,
    };
    for &slot_id in ids {
        if let Err(error) = validate() {
            progress.interrupted = Some(format!("{error:#}"));
            break;
        }
        let mut result = DpiSlotWriteResult {
            slot_id,
            was_equal: false,
            before: None,
            acknowledged: None,
            error: None,
        };
        match read(context, slot_id) {
            Err(error) => {
                result.error = Some(format!("{error:#}"));
                if crate::backend::exchange_finalization_failed(&error) {
                    progress.interrupted = Some(format!("{error:#}"));
                    progress.finalization = Some(error);
                }
            }
            Ok(before) => {
                result.was_equal = same(&before);
                result.before = Some(before);
                if !result.was_equal {
                    match validate() {
                        Ok(()) => match write(context, slot_id) {
                            Ok(ack) => result.acknowledged = Some(ack),
                            Err(error) => {
                                result.acknowledged =
                                    error.downcast_ref::<DpiStagesAcknowledgement>().cloned();
                                result.error = Some(format!("{error:#}"));
                                if crate::backend::exchange_finalization_failed(&error) {
                                    progress.interrupted = Some(format!("{error:#}"));
                                    progress.finalization = Some(error);
                                }
                            }
                        },
                        Err(error) => {
                            let error = format!("{error:#}");
                            result.error = Some(error.clone());
                            progress.interrupted = Some(error);
                        }
                    }
                }
            }
        }
        // Cancellation/identity loss must prevent subsequent slot writes.
        progress.writes.push(result);
        if progress.interrupted.is_some() {
            break;
        }
        if let Err(error) = validate() {
            progress.interrupted = Some(format!("{error:#}"));
            break;
        }
    }
    progress
}

#[cfg(test)]
mod tests {
    // Source expectations cover visibility remapping, setDPIStages'
    // 3+7*count packet-size override and the getter's one-based records.
    use super::*;
    use serde_json::json;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct Reply {
        status: u8,
        data: Vec<u8>,
        declared_len: Option<u8>,
    }

    struct MockTransport {
        sent: Mutex<Vec<Vec<u8>>>,
        replies: Mutex<VecDeque<Reply>>,
        get_ids: Mutex<Vec<u8>>,
        node: Option<crate::backend::HidNode>,
        returned_prefix: u8,
        collection_sends: Cell<usize>,
        collection_gets: Cell<usize>,
    }

    impl MockTransport {
        fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.into_iter().collect()),
                get_ids: Mutex::new(Vec::new()),
                node: None,
                returned_prefix: 0,
                collection_sends: Cell::new(0),
                collection_gets: Cell::new(0),
            }
        }

        fn sent(&self) -> Vec<Vec<u8>> {
            self.sent.lock().unwrap().clone()
        }
    }

    impl FeatureTransport for MockTransport {
        fn hid_node(&self) -> Option<&crate::backend::HidNode> {
            self.node.as_ref()
        }

        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(report.to_vec());
            Ok(())
        }

        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.get_ids.lock().unwrap().push(report[0]);
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .context("mock reply exhausted")?;
            let sent = self
                .sent
                .lock()
                .unwrap()
                .last()
                .context("mock has no sent report")?
                .clone();
            report.fill(0);
            report[0] = self.returned_prefix;
            report[1] = reply.status;
            report[2] = sent[2];
            let len = reply.declared_len.unwrap_or(reply.data.len() as u8);
            report[6] = len;
            report[7] = sent[7];
            report[8] = sent[8];
            report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
            Ok(91)
        }

        fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
            self.collection_sends.set(self.collection_sends.get() + 1);
            self.send_feature(report)
        }

        fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.collection_gets.set(self.collection_gets.get() + 1);
            self.get_feature(report)
        }

        fn metadata(&self) -> serde_json::Value {
            json!({"mock":true})
        }
    }

    fn cap() -> &'static MouseDpiStagesCapability {
        capability(182).unwrap()
    }

    fn transaction_sequence() -> impl FnMut() -> anyhow::Result<u8> {
        let mut next = 0;
        move || {
            let current = next;
            next = (next + 1) % 31;
            Ok(current)
        }
    }

    fn draft(
        enabled: bool,
        active_stage: usize,
        rows: &[(u16, u16, bool, bool)],
    ) -> DpiStagesDraft {
        DpiStagesDraft {
            enabled,
            active_stage,
            stages: rows
                .iter()
                .map(|&(x, y, visible, independent)| DpiStage {
                    x,
                    y,
                    visible,
                    independent,
                })
                .collect(),
        }
    }

    fn getter_data(active: u8, rows: &[(u16, u16, u16)]) -> Vec<u8> {
        getter_data_for_profile(1, active, rows)
    }

    fn getter_data_for_profile(profile: u8, active: u8, rows: &[(u16, u16, u16)]) -> Vec<u8> {
        let mut data = vec![profile, active, rows.len() as u8];
        for (index, &(x, y, z)) in rows.iter().enumerate() {
            data.push(index as u8 + 1);
            data.extend_from_slice(&x.to_be_bytes());
            data.extend_from_slice(&y.to_be_bytes());
            data.extend_from_slice(&z.to_be_bytes());
        }
        data
    }

    fn reply(data: Vec<u8>) -> Reply {
        Reply {
            status: 2,
            data,
            declared_len: None,
        }
    }

    #[test]
    fn finish_failure_keeps_the_dpi_stage_ack_without_readback_or_later_slots() {
        let transport = MockTransport::new([reply(vec![7, 8])]);
        let leased = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&transport);
        let mut cap = cap().clone();
        cap.transport.sleep_between_out_ms = 0;
        cap.transport.sleep_between_out_in_ms = 0;
        cap.transport.sleep_between_in_ms = 0;
        let requested = draft(true, 1, &[(800, 800, true, false)]);
        let error = apply_current(&leased, &cap, &requested, transaction_sequence(), || Ok(()))
            .unwrap_err();
        let progress = error.downcast_ref::<DpiStagesWriteFailure>().unwrap();
        let acknowledged = progress.acknowledged.as_ref().unwrap();
        assert_eq!(acknowledged.response_bytes, 2);
        assert_eq!(&acknowledged.raw_data[..2], &[7, 8]);
        assert!(progress.observed.is_none());
        assert_eq!(transport.sent().len(), 1);
        assert!(crate::backend::exchange_finalization_failed(&error));
    }

    #[test]
    fn dpi_factory_binds_wired_and_radio_selectors_before_independent_exchange() {
        for physical in [226, 227] {
            for bind_before_exchange in [false, true] {
                let mut cap = capability(226).unwrap().clone();
                for branch in cap.transport.physical_transports_by_pid.values_mut() {
                    branch.sleep_between_out_ms = 0;
                    branch.sleep_between_out_in_ms = 0;
                    branch.sleep_between_in_ms = 0;
                }
                if bind_before_exchange {
                    cap = cap.for_physical_product(physical).unwrap();
                    assert_eq!(cap.transport.transaction_prefix, 0);
                    assert_eq!(
                        cap.transport.source_class,
                        if physical == 226 {
                            "rzDevice25"
                        } else {
                            "rzDevice25DualLinkMouse"
                        }
                    );
                }
                let mut device = MockTransport::new([reply(getter_data(1, &[(800, 1600, 0)]))]);
                device.returned_prefix = 27;
                device.node = Some(crate::backend::HidNode {
                    path: b"simulated-dpi-collection".to_vec(),
                    vendor_id: 0x1532,
                    product_id: physical as u16,
                    usage_page: 1,
                    usage: 2,
                    // The Windows service can already have validated interface
                    // 3 with SetupAPI when hidapi itself reports -1.
                    interface_number: -1,
                    release_number: 0,
                    serial_number: None,
                    manufacturer: None,
                    product: None,
                });
                let observed =
                    read_current(&device, &cap, transaction_sequence(), || Ok(())).unwrap();
                assert_eq!(observed.records[0].x, 800);
                assert_eq!(device.sent()[0][0], if physical == 226 { 0 } else { 3 });
                assert_eq!(device.sent()[0][2], 0);
                assert_eq!(*device.get_ids.lock().unwrap(), [3]);
                assert_eq!(device.collection_sends.get(), 1);
                assert_eq!(device.collection_gets.get(), 1);
            }
        }
        assert!(capability(226).unwrap().for_physical_product(9999).is_err());
    }

    #[test]
    fn source_ke_filters_visible_rows_and_remaps_active_stage() {
        let packed = pack(
            cap(),
            &draft(
                true,
                3,
                &[
                    (400, 400, true, false),
                    (800, 800, false, false),
                    (1600, 3200, true, true),
                    (6400, 6400, true, false),
                    (12800, 12800, true, false),
                ],
            ),
        )
        .unwrap();
        assert_eq!(packed.active_stage, 2);
        assert_eq!(
            packed
                .stages
                .iter()
                .map(|row| (row.index, row.x, row.y, row.z))
                .collect::<Vec<_>>(),
            vec![
                (0, 400, 400, 0),
                (1, 1600, 3200, 0),
                (2, 6400, 6400, 0),
                (3, 12800, 12800, 0),
            ]
        );
    }

    #[test]
    fn source_ke_disabled_mode_sends_only_selected_stage() {
        let packed = pack(
            cap(),
            &draft(
                false,
                3,
                &[
                    (400, 400, true, false),
                    (800, 800, true, false),
                    (1600, 3200, true, true),
                    (6400, 6400, true, false),
                    (12800, 12800, true, false),
                ],
            ),
        )
        .unwrap();
        assert_eq!(packed.active_stage, 1);
        assert_eq!(
            packed.stages,
            vec![PackedDpiStage {
                index: 0,
                x: 1600,
                y: 3200,
                z: 0
            }]
        );
    }

    #[test]
    fn setter_and_getter_use_source_report_bytes_and_successful_readback() {
        let device = MockTransport::new([
            reply(vec![1, 1, 2]),
            reply(getter_data(2, &[(400, 400, 0), (1600, 3200, 0)])),
        ]);
        let requested = draft(
            true,
            3,
            &[
                (400, 400, true, false),
                (800, 800, false, false),
                (1600, 3200, true, true),
            ],
        );
        let result = apply_current(
            &device,
            cap(),
            &requested,
            {
                let mut transaction = 0u8;
                move || {
                    let current = transaction;
                    transaction += 1;
                    Ok(current)
                }
            },
            || Ok(()),
        )
        .unwrap();
        assert!(result.verified);
        let sent = device.sent();
        assert_eq!(sent.len(), 2);
        let setter = &sent[0];
        assert_eq!(setter.len(), 91);
        assert_eq!(&setter[6..9], &[17, 4, 6]); // 3 + 7*2 overrides 80
        assert_eq!(
            &setter[9..26],
            &[1, 2, 2, 0, 1, 144, 1, 144, 0, 0, 1, 6, 64, 12, 128, 0, 0]
        );
        assert_eq!(
            setter[89],
            setter[3..89].iter().fold(0, |sum, byte| sum ^ byte)
        );
        let getter = &sent[1];
        assert_eq!(&getter[6..9], &[80, 4, 134]);
        assert_eq!(getter[9], 1);
    }

    #[test]
    fn malformed_or_mismatched_getter_is_rejected() {
        let malformed = MockTransport::new([Reply {
            status: 2,
            data: vec![1, 1, 1, 1],
            declared_len: Some(4),
        }]);
        assert!(read_current(&malformed, cap(), || Ok(0), || Ok(())).is_err());

        let mismatched = MockTransport::new([reply(vec![2, 1, 1, 1, 1, 144, 1, 144, 0, 0])]);
        assert!(read_current(&mismatched, cap(), || Ok(0), || Ok(())).is_err());
    }

    #[test]
    fn write_returns_error_when_follow_up_readback_fails() {
        let device = MockTransport::new([
            reply(vec![1, 1, 1]),
            Reply {
                status: 2,
                data: vec![1, 1, 1, 1],
                declared_len: Some(4),
            },
        ]);
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let error = apply_current(
            &device,
            cap(),
            &requested,
            {
                let mut transaction = 0u8;
                move || {
                    let current = transaction;
                    transaction += 1;
                    Ok(current)
                }
            },
            || Ok(()),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("readback"));
        assert_eq!(device.sent().len(), 2);
    }

    #[test]
    fn invalid_drafts_are_rejected_before_any_device_send() {
        let mut cases = vec![
            draft(true, 0, &[(400, 400, true, false), (800, 800, true, false)]),
            draft(
                true,
                2,
                &[(400, 400, true, false), (800, 800, false, false)],
            ),
            draft(true, 1, &[(99, 400, true, false), (800, 800, true, false)]),
            draft(
                true,
                1,
                &[(400, 30001, true, true), (800, 800, true, false)],
            ),
            draft(true, 1, &[(400, 400, true, false)]),
        ];
        cases.push(draft(true, 1, &[(400, 400, true, false); 6]));
        for requested in cases {
            let device = MockTransport::new([]);
            assert!(apply_current(&device, cap(), &requested, || Ok(0), || Ok(())).is_err());
            assert!(device.sent().is_empty());
        }
    }

    #[test]
    fn a_complete_but_different_readback_never_returns_verified() {
        let device = MockTransport::new([
            reply(vec![1, 1, 2]),
            reply(getter_data(1, &[(400, 400, 0), (800, 1600, 0)])),
        ]);
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let mut transaction = 0u8;
        let error = apply_current(
            &device,
            cap(),
            &requested,
            || {
                let current = transaction;
                transaction += 1;
                Ok(current)
            },
            || Ok(()),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("does not match the requested table"));
        assert_eq!(device.sent().len(), 2);
    }

    #[test]
    fn capabilities_enable_only_reaudited_products_with_their_actual_limits() {
        for (pid, max, step, interface) in [
            (182, 30000, 50, 0),
            (170, 30000, 50, 0),
            (204, 35000, 50, 0),
            (226, 50000, 1, 3),
            (45066, 35000, 50, 0),
        ] {
            let cap = capability(pid).unwrap();
            assert_eq!((cap.min_dpi, cap.max_dpi, cap.dpi_step), (100, max, step));
            assert_eq!(cap.editor_stage_counts, &[2, 3, 4, 5]);
            assert_eq!(cap.transport.claim_interface_for(pid).unwrap(), interface);
            assert_eq!(cap.transport.product_id, pid);
            assert_eq!(cap.protocol_record_capacity, 11);
        }
        // Receiver, BLE and other direct products are not enabled by common
        // base methods or by having an ordinary single-level DPI capability.
        for pid in [
            0, 124, 167, 171, 172, 183, 205, 206, 222, 227, 228, 45067, 45068,
        ] {
            assert!(capability(pid).is_none(), "unexpected DPI stage gate {pid}");
        }
    }

    #[test]
    fn every_new_product_writes_variable_size_big_endian_table_then_reads_profile_one() {
        for pid in [170, 204, 226, 45066] {
            let cap = capability(pid).unwrap();
            let max = cap.max_dpi;
            let device = MockTransport::new([
                reply(Vec::new()), // A status-only success is an ack, not a table.
                reply(getter_data(2, &[(100, 100, 0), (max, max - 1, 0)])),
            ]);
            let requested = draft(
                true,
                3,
                &[
                    (100, 100, true, false),
                    (800, 800, false, false),
                    (max, max - 1, true, true),
                ],
            );
            let result =
                apply_current(&device, cap, &requested, transaction_sequence(), || Ok(())).unwrap();
            assert!(result.verified);
            assert_eq!(result.observed.profile_id, 1);
            assert_eq!(result.acknowledged.response_bytes, 0);
            let sent = device.sent();
            assert_eq!(sent.len(), 2);
            assert_eq!(&sent[0][6..9], &[17, 4, 6]);
            assert_eq!(&sent[0][9..13], &[1, 2, 2, 0]);
            assert_eq!(&sent[0][20..22], &max.to_be_bytes());
            assert_eq!(&sent[0][22..24], &(max - 1).to_be_bytes());
            assert_eq!(&sent[1][6..10], &[80, 4, 134, 1]);
        }
    }

    #[test]
    fn product_specific_ranges_reject_invalid_xy_before_any_io() {
        for pid in [170, 204, 226, 45066] {
            let cap = capability(pid).unwrap();
            for (x, y) in [(99, 400), (400, cap.max_dpi + 1)] {
                let device = MockTransport::new([]);
                let requested = draft(true, 1, &[(x, y, true, true), (800, 800, true, false)]);
                assert!(
                    apply_current(&device, cap, &requested, transaction_sequence(), || Ok(()))
                        .is_err()
                );
                assert!(device.sent().is_empty());
            }
        }
        let fine = pack(
            capability(226).unwrap(),
            &draft(false, 1, &[(49999, 10001, true, true)]),
        )
        .unwrap();
        assert_eq!((fine.stages[0].x, fine.stages[0].y), (49999, 10001));
        // dpiStep is source editor metadata. The wire protocol does not silently
        // quantize values; unsupported rounding must not change an explicit input.
        let exact = pack(
            capability(204).unwrap(),
            &draft(false, 1, &[(34999, 101, true, true)]),
        )
        .unwrap();
        assert_eq!((exact.stages[0].x, exact.stages[0].y), (34999, 101));
    }

    #[test]
    fn invalid_tail_count_and_active_selection_do_not_confirm_an_acknowledged_write() {
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let mut count = getter_data(1, &[(400, 400, 0), (800, 800, 0)]);
        count[2] = 1;
        let mut active = getter_data(1, &[(400, 400, 0), (800, 800, 0)]);
        active[1] = 3;
        let mut tail = getter_data(1, &[(400, 400, 0), (800, 800, 0)]);
        tail.extend_from_slice(&[0; 7]);
        tail.extend_from_slice(&[3, 1, 144, 1, 144, 0, 0]);
        for data in [count, active, tail] {
            let device = MockTransport::new([reply(Vec::new()), reply(data)]);
            let error = apply_current(
                &device,
                capability(226).unwrap(),
                &requested,
                transaction_sequence(),
                || Ok(()),
            )
            .unwrap_err();
            assert!(format!("{error:#}").contains("unconfirmed"));
            assert_eq!(device.sent().len(), 2);
        }
    }

    #[test]
    fn all_guid_matching_slots_preserve_metadata_order_and_skip_equal_tables() {
        let slots = [
            ObservedProfileSlot {
                slot_id: 4,
                guid: "active".into(),
            },
            ObservedProfileSlot {
                slot_id: 2,
                guid: "other".into(),
            },
            ObservedProfileSlot {
                slot_id: 3,
                guid: "active".into(),
            },
        ];
        assert_eq!(
            select_profile_slots("active", &slots)
                .iter()
                .map(|s| s.slot_id)
                .collect::<Vec<_>>(),
            vec![4, 3]
        );
        assert!(select_profile_slots("missing", &slots).is_empty());
        for pid in [170, 204, 226, 45066] {
            let device = MockTransport::new([
                reply(getter_data_for_profile(
                    4,
                    1,
                    &[(400, 400, 0), (800, 800, 0)],
                )),
                reply(getter_data_for_profile(
                    3,
                    2,
                    &[(400, 400, 0), (800, 800, 0)],
                )),
                reply(Vec::new()),
            ]);
            let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
            let results = synchronize_profile_slots(
                &device,
                capability(pid).unwrap(),
                "active",
                &slots,
                &requested,
                transaction_sequence(),
                || Ok(()),
            )
            .unwrap();
            assert_eq!(
                results.iter().map(|r| r.slot_id).collect::<Vec<_>>(),
                vec![4, 3]
            );
            assert!(results[0].was_equal && results[0].acknowledged.is_none());
            assert!(!results[1].was_equal && results[1].acknowledged.is_some());
            assert!(results.iter().all(|r| r.error.is_none()));
            let sent = device.sent();
            assert_eq!(
                sent.iter().map(|r| (r[8], r[9])).collect::<Vec<_>>(),
                vec![(134, 4), (134, 3), (6, 3)]
            );
            // Original slot chain has no post-set getter. An acknowledgement
            // remains separate from confirmed current-table verification.
            assert_eq!(sent.len(), 3);
        }
    }

    #[test]
    fn slot_errors_preserve_prior_progress_and_continue_other_observed_slots() {
        let slots = [
            ObservedProfileSlot {
                slot_id: 4,
                guid: "active".into(),
            },
            ObservedProfileSlot {
                slot_id: 3,
                guid: "active".into(),
            },
        ];
        let device = MockTransport::new([
            reply(getter_data_for_profile(
                4,
                2,
                &[(400, 400, 0), (800, 800, 0)],
            )),
            reply(Vec::new()),
            reply(vec![3, 1, 1, 1]), // Malformed second-slot reading.
        ]);
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let results = synchronize_profile_slots(
            &device,
            capability(45066).unwrap(),
            "active",
            &slots,
            &requested,
            transaction_sequence(),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].acknowledged.is_some() && results[0].error.is_none());
        assert!(
            results[1].before.is_none()
                && results[1].acknowledged.is_none()
                && results[1].error.is_some()
        );
        assert_eq!(
            device
                .sent()
                .iter()
                .map(|r| (r[8], r[9]))
                .collect::<Vec<_>>(),
            vec![(134, 4), (6, 4), (134, 3)]
        );
    }

    #[test]
    fn malformed_slot_does_not_write_it_and_later_valid_slot_still_completes() {
        let slots = [
            ObservedProfileSlot {
                slot_id: 2,
                guid: "active".into(),
            },
            ObservedProfileSlot {
                slot_id: 4,
                guid: "active".into(),
            },
        ];
        let device = MockTransport::new([
            reply(vec![2, 1, 1, 1]),
            reply(getter_data_for_profile(
                4,
                2,
                &[(400, 400, 0), (800, 800, 0)],
            )),
            reply(Vec::new()),
        ]);
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let results = synchronize_profile_slots(
            &device,
            capability(226).unwrap(),
            "active",
            &slots,
            &requested,
            transaction_sequence(),
            || Ok(()),
        )
        .unwrap();
        assert!(results[0].error.is_some() && results[0].acknowledged.is_none());
        assert!(results[1].error.is_none() && results[1].acknowledged.is_some());
        assert_eq!(
            device
                .sent()
                .iter()
                .map(|r| (r[8], r[9]))
                .collect::<Vec<_>>(),
            vec![(134, 2), (134, 4), (6, 4)]
        );
    }

    #[test]
    fn cancellation_before_send_has_no_write_and_cancellation_after_ack_stops_readback() {
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let device = MockTransport::new([]);
        assert!(
            apply_current(
                &device,
                capability(204).unwrap(),
                &requested,
                transaction_sequence(),
                || anyhow::bail!("canceled")
            )
            .is_err()
        );
        assert!(device.sent().is_empty());
        let device = MockTransport::new([reply(Vec::new())]);
        let calls = Cell::new(0);
        let error = apply_current(
            &device,
            capability(204).unwrap(),
            &requested,
            transaction_sequence(),
            || {
                let call = calls.get();
                calls.set(call + 1);
                ensure!(call < 5, "canceled after ack");
                Ok(())
            },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("unconfirmed"));
        assert_eq!(device.sent().len(), 1);
    }

    #[test]
    fn unknown_transaction_and_unsupported_device_status_never_return_success() {
        let requested = draft(true, 1, &[(400, 400, true, false), (800, 800, true, false)]);
        let device = MockTransport::new([]);
        assert!(
            apply_current(
                &device,
                capability(170).unwrap(),
                &requested,
                || Ok(31),
                || Ok(())
            )
            .is_err()
        );
        assert!(device.sent().is_empty());
        let device = MockTransport::new([Reply {
            status: 5,
            data: Vec::new(),
            declared_len: None,
        }]);
        assert!(
            apply_current(
                &device,
                capability(170).unwrap(),
                &requested,
                transaction_sequence(),
                || Ok(())
            )
            .is_err()
        );
        assert_eq!(device.sent().len(), 1);
    }
}
