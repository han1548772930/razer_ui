//! PID226's physical BLE228 branch uses Protocol30 namespace 0x0b.
//! Profiles are request header fields; getter payloads contain no profile echo.
use super::{
    Acknowledgement, DpiChange, DpiPair, DpiReport, DpiTaskErrors, PackedStages, SourceNumber,
    pack_for, required, same_stages, validate_change_for, validate_pair,
};
use crate::{
    mouse_dpi_stages::{
        DpiSlotSynchronization, DpiStagesAcknowledgement, DpiStagesReading, ObservedProfileSlot,
        PackedDpiStage,
    },
    mouse_runtime_artwork::WindowsBleTarget,
};
use anyhow::{Context as _, ensure};

#[path = "mouse_current_dpi_ble_initialization.rs"]
pub mod initialization;

pub const EDITION: [u8; 3] = [0, 1, 134];
pub const GET_SERIAL: [u8; 3] = [0, 1, 131];
pub const GET_LEVEL: [u8; 3] = [0, 11, 129];
pub const SET_LEVEL: [u8; 3] = [6, 11, 1];
pub const GET_STAGES: [u8; 3] = [0, 11, 132];
pub const SET_STAGES: [u8; 3] = [255, 11, 4];
pub const GET_MAX_PROFILES: [u8; 3] = [0, 3, 137];
pub const GET_PROFILE_IDS: [u8; 3] = [0, 3, 128];
pub const GET_PROFILE_NAME: [u8; 3] = [4, 3, 132];
pub const STAGES_PROFILE: u8 = 1;
pub const ACTIVE_PROFILE: u8 = 0;

pub fn accepts(product_id: u32, physical_product_id: u32) -> bool {
    product_id == 226 && physical_product_id == 228
}

pub fn target_from_hid(
    product_id: u32,
    physical_product_id: u32,
    container: &str,
    observation: &serde_json::Value,
) -> anyhow::Result<WindowsBleTarget> {
    ensure!(
        accepts(product_id, physical_product_id),
        "Current DPI BLE physical factory differs"
    );
    WindowsBleTarget::from_hid(product_id, physical_product_id, container, observation)
}

pub fn frame(
    transaction: u8,
    command: [u8; 3],
    profile: u8,
    payload: &[u8],
) -> anyhow::Result<Vec<u8>> {
    ensure!(transaction < 31, "Current DPI BLE transaction differs");
    let valid = match command {
        EDITION | GET_SERIAL => profile == 0 && payload.is_empty(),
        GET_LEVEL => payload.is_empty(),
        GET_STAGES => profile != 0 && payload.is_empty(),
        SET_LEVEL => profile == ACTIVE_PROFILE && payload.len() == 6,
        SET_STAGES => {
            profile != 0
                && payload.len() >= 3
                && payload.len() == 3 + 7 * usize::from(payload[1])
                && payload.len() <= usize::from(command[0])
                && payload.last() == Some(&0)
        }
        GET_MAX_PROFILES | GET_PROFILE_IDS => profile == 0 && payload.is_empty(),
        GET_PROFILE_NAME => profile != 0 && payload.len() == 4,
        _ => false,
    };
    ensure!(valid, "Current DPI BLE command or payload differs");
    // 20063.setDPIStages retains command[0]=255, while _packetSizeSet selects
    // the actual data length. The profile remains in byte6, outside the data.
    let mut bytes = vec![
        transaction,
        payload.len() as u8,
        0,
        0,
        command[1],
        command[2],
        profile,
        0,
    ];
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

pub trait Transport {
    /// Complete the real transaction/status response before returning its data.
    /// Acknowledgement.profile identifies this request, not a device echo.
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        payload: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement>;
}

fn exchange(
    transport: &mut dyn Transport,
    command: [u8; 3],
    profile: u8,
    payload: &[u8],
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    frame(0, command, profile, payload)?;
    validate()?;
    let reply = transport.exchange(command, profile, payload, validate)?;
    ensure!(
        reply.command == command
            && reply.profile == Some(profile)
            && reply.transaction < 31
            && reply.response_bytes == reply.data.len()
            && reply.response_bytes <= 65535,
        "Current DPI BLE response belongs to another request or is truncated"
    );
    // Preserve an actual setter ACK as diagnostic progress. Its caller records
    // it before the next operation checks the connection/cancellation scope.
    if !matches!(command, SET_STAGES | SET_LEVEL) {
        validate()?;
    }
    Ok(reply)
}

/// Current37857.cA keeps every printable ASCII byte, including spaces and
/// bytes following NUL. Protocol30 has no fixed 22-byte serial payload.
pub fn read_serial(
    transport: &mut dyn Transport,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<String> {
    let reply = exchange(transport, GET_SERIAL, 0, &[], validate)?;
    let serial: String = reply
        .data
        .into_iter()
        .filter(|byte| (32..127).contains(byte))
        .map(char::from)
        .collect();
    ensure!(
        !serial.is_empty(),
        "BLE initialization has no queried device serial number"
    );
    Ok(serial)
}

fn stage_payload(packed: &PackedStages) -> Vec<u8> {
    let mut data = vec![0; 3 + 7 * usize::from(packed.number_of_active_stages)];
    data[0] = packed.active_stage;
    data[1] = packed.number_of_active_stages;
    for (index, row) in packed.records.iter().enumerate() {
        let start = 2 + index * 7;
        data[start] = row.index;
        data[start + 1..start + 3]
            .copy_from_slice(&row.x.map_or(0, SourceNumber::word).to_le_bytes());
        data[start + 3..start + 5]
            .copy_from_slice(&row.y.map_or(0, SourceNumber::word).to_le_bytes());
        data[start + 5..start + 7].copy_from_slice(&row.z.word().to_le_bytes());
    }
    data
}

fn stage_reading(ack: &Acknowledgement, profile: u8) -> anyhow::Result<DpiStagesReading> {
    ensure!(
        profile != 0 && ack.profile == Some(profile) && ack.data.len() >= 2,
        "Current DPI BLE stage response is truncated or has another request profile"
    );
    let mut records = Vec::new();
    for row in ack.data[2..].chunks(7) {
        if row.len() != 7 {
            // The source consumer stops at an explicit index zero. Its absent
            // axes stay unobserved instead of becoming a fabricated zero row.
            ensure!(row[0] == 0, "Current DPI BLE stage record is truncated");
            break;
        }
        records.push(PackedDpiStage {
            index: row[0],
            x: u16::from_le_bytes([row[1], row[2]]),
            y: u16::from_le_bytes([row[3], row[4]]),
            z: u16::from_le_bytes([row[5], row[6]]),
        });
    }
    let reading = DpiStagesReading {
        profile_id: profile,
        active_stage: ack.data[0],
        number_of_active_stages: ack.data[1],
        records,
        response_bytes: ack.response_bytes,
    };
    validate_stage_reading(&reading, profile)?;
    Ok(reading)
}

fn validate_stage_reading(reading: &DpiStagesReading, profile: u8) -> anyhow::Result<()> {
    let parsed_bytes = reading
        .records
        .len()
        .checked_mul(7)
        .and_then(|bytes| bytes.checked_add(2))
        .context("Current DPI BLE stage record length overflow")?;
    ensure!(
        profile != 0
            && reading.profile_id == profile
            && (2..=65535).contains(&reading.response_bytes)
            && parsed_bytes <= reading.response_bytes
            && reading.response_bytes - parsed_bytes < 7,
        "Current DPI BLE stage reading has another request profile or invalid byte boundaries"
    );
    Ok(())
}

/// Validate the common observations at the typed protocol/IPC boundary.
/// A source terminal marker can leave up to six non-record bytes unobserved.
pub fn validate_readings(report: &DpiReport) -> anyhow::Result<()> {
    if let Some(active) = report.active {
        validate_pair(required(226)?, active)?;
    }
    if let Some(reading) = &report.stages {
        validate_stage_reading(reading, STAGES_PROFILE)?;
    }
    for ack in &report.acknowledgements {
        ensure!(
            ack.transaction < 31
                && ack.response_bytes == ack.data.len()
                && ack.response_bytes <= 65535
                && match (ack.command, ack.profile) {
                    (SET_STAGES, Some(STAGES_PROFILE)) => report
                        .requested
                        .as_ref()
                        .is_some_and(|change| change.stages.is_some()),
                    (SET_LEVEL, Some(ACTIVE_PROFILE)) => report
                        .requested
                        .as_ref()
                        .is_some_and(|change| change.active.is_some()),
                    _ => false,
                },
            "Current DPI BLE setter acknowledgement belongs to another request or has invalid byte boundaries"
        );
    }
    for (index, slot) in report.profile_slots.iter().enumerate() {
        ensure!(
            slot.slot_id > 1
                && !report.profile_slots[..index]
                    .iter()
                    .any(|previous| previous.slot_id == slot.slot_id),
            "Current DPI BLE projected profile slots are reserved or duplicated"
        );
    }
    let source = report
        .requested
        .as_ref()
        .and_then(|change| Some((change.stages.as_ref()?, change.profile_guid.as_deref()?)));
    let expected_slots = source.map_or_else(Vec::new, |(_, guid)| {
        crate::mouse_dpi_stages::select_profile_slots(guid, &report.profile_slots)
            .into_iter()
            .map(|slot| slot.slot_id)
            .collect()
    });
    ensure!(
        report.profile_slots.is_empty() && report.profile_slot_writes.is_empty()
            || source.is_some(),
        "Current DPI BLE profile synchronization has no requested stage or profile identity"
    );
    ensure!(
        report.profile_slot_writes.len() <= expected_slots.len()
            && (report.write_errors.stages().is_some()
                || report.profile_slot_writes.len() == expected_slots.len()),
        "Current DPI BLE profile synchronization omitted or added source slot tasks"
    );
    let packed = source
        .map(|(stages, _)| pack_for(226, stages))
        .transpose()?;
    for (index, slot) in report.profile_slot_writes.iter().enumerate() {
        ensure!(
            expected_slots.get(index) == Some(&slot.slot_id),
            "Current DPI BLE profile synchronization order differs"
        );
        if let Some(before) = &slot.before {
            validate_stage_reading(before, slot.slot_id)?;
            ensure!(
                slot.was_equal
                    == same_slot(packed.as_ref().expect("requested BLE slot task"), before),
                "Current DPI BLE profile equality claim differs from the actual getter"
            );
        } else {
            ensure!(
                !slot.was_equal && slot.acknowledged.is_none() && slot.error.is_some(),
                "Current DPI BLE profile mutation has no preceding getter"
            );
        }
        if let Some(ack) = &slot.acknowledged {
            ensure!(
                slot.before.is_some()
                    && !slot.was_equal
                    && ack.response_bytes == ack.raw_data.len()
                    && ack.response_bytes <= 65535,
                "Current DPI BLE profile setter acknowledgement has invalid byte boundaries or no differing getter"
            );
        }
        ensure!(
            !slot.was_equal || slot.acknowledged.is_none(),
            "Current DPI BLE wrote a slot already equal to its source request"
        );
        if let Some(error) = &slot.error {
            ensure!(
                !error.is_empty() && report.write_errors.stages().is_some(),
                "Current DPI BLE profile failure omits its stage task error"
            );
        } else {
            ensure!(
                slot.was_equal || slot.acknowledged.is_some(),
                "Current DPI BLE profile slot claims success without source confirmation"
            );
        }
    }
    Ok(())
}

fn read_inventory(
    transport: &mut dyn Transport,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<crate::mouse_onboard::OnboardInventory> {
    let maximum = exchange(transport, GET_MAX_PROFILES, 0, &[], validate)?;
    let maximum = usize::from(
        *maximum
            .data
            .first()
            .context("BLE OBM capacity is unobserved")?,
    );
    // Current getNumberOfProfiles and getProfileIDList invoke the same getter
    // independently. The number is the returned data length, not its first byte.
    let count = exchange(transport, GET_PROFILE_IDS, 0, &[], validate)?;
    ensure!(
        !count.data.is_empty(),
        "BLE OBM profile count is unobserved"
    );
    let ids = exchange(transport, GET_PROFILE_IDS, 0, &[], validate)?.data;
    ensure!(
        !ids.is_empty() && count.data.len() == ids.len() && ids.len() <= maximum,
        "BLE OBM capacity, count and actual profile IDs disagree"
    );
    for (index, profile) in ids.iter().enumerate() {
        ensure!(
            *profile != 0 && !ids[..index].contains(profile),
            "BLE OBM profile IDs are zero or duplicated"
        );
    }
    validate()?;
    Ok(crate::mouse_onboard::OnboardInventory {
        max_profiles: u8::try_from(maximum)?,
        profile_count: u8::try_from(ids.len())?,
        profile_ids: ids,
    })
}

fn read_profile_identity(
    transport: &mut dyn Transport,
    profile: u8,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<crate::mouse_onboard::OnboardIdentityObservation> {
    let mut last_error = None;
    for attempt in 0..10 {
        let result = (|| -> anyhow::Result<crate::mouse_onboard::OnboardIdentityObservation> {
            let mut metadata = Vec::with_capacity(250);
            for offset in (0..250u16).step_by(78) {
                let count = (250 - offset).min(78);
                let mut payload = Vec::from(offset.to_le_bytes());
                payload.extend_from_slice(&count.to_le_bytes());
                let block = exchange(transport, GET_PROFILE_NAME, profile, &payload, validate)?;
                ensure!(
                    block.data.len() >= 2,
                    "BLE OBM metadata header is truncated"
                );
                metadata.extend_from_slice(&block.data[2..]);
            }
            ensure!(
                metadata.len() == 250,
                "BLE OBM metadata is incomplete or oversized"
            );
            Ok(crate::mouse_onboard::decode_identity(
                226,
                profile,
                metadata,
                attempt + 1,
            ))
        })();
        validate()?;
        match result {
            Ok(identity) if identity.source_guid_accepted || attempt == 9 => {
                return Ok(identity);
            }
            Ok(_) => {}
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.context("BLE OBM profile identity was never observed")?)
}

fn same_slot(packed: &PackedStages, reading: &DpiStagesReading) -> bool {
    let mut records_equal = false;
    for (index, observed) in reading.records.iter().enumerate() {
        if observed.index == 0 {
            break;
        }
        if !packed
            .records
            .get(index)
            .is_some_and(|expected| super::same_record(expected, observed))
        {
            return false;
        }
        records_equal = true;
    }
    records_equal
        && packed.active_stage == reading.active_stage
        && packed.number_of_active_stages == reading.number_of_active_stages
}

fn synchronize_profile_slots(
    transport: &mut dyn Transport,
    active_guid: &str,
    observed: &[ObservedProfileSlot],
    packed: &PackedStages,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> DpiSlotSynchronization {
    let ids: Vec<_> = crate::mouse_dpi_stages::select_profile_slots(active_guid, observed)
        .into_iter()
        .map(|slot| slot.slot_id)
        .collect();
    synchronize_slot_ids(transport, &ids, packed, validate)
}

fn synchronize_slot_ids(
    transport: &mut dyn Transport,
    ids: &[u8],
    packed: &PackedStages,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> DpiSlotSynchronization {
    let mut context = transport;
    crate::mouse_dpi_stages::synchronize_profile_slot_ids_with(
        ids,
        &mut context,
        &|| validate(),
        |transport, profile| {
            let before = stage_reading(
                &exchange(*transport, GET_STAGES, profile, &[], validate)?,
                profile,
            )?;
            Ok(before)
        },
        |before| same_slot(packed, before),
        |transport, profile| {
            let ack = exchange(
                *transport,
                SET_STAGES,
                profile,
                &stage_payload(packed),
                validate,
            )?;
            Ok(DpiStagesAcknowledgement {
                response_bytes: ack.response_bytes,
                raw_data: ack.data,
            })
        },
    )
}

fn level_reading(ack: &Acknowledgement) -> anyhow::Result<DpiPair> {
    ensure!(
        ack.profile == Some(ACTIVE_PROFILE) && ack.data.len() >= 6,
        "Current DPI BLE active response is truncated or has another request profile"
    );
    let value = DpiPair {
        x: u16::from_le_bytes([ack.data[0], ack.data[1]]),
        y: u16::from_le_bytes([ack.data[2], ack.data[3]]),
    };
    validate_pair(required(226)?, value)?;
    Ok(value)
}

pub fn execute(
    product_id: u32,
    physical_product_id: u32,
    transport: &mut dyn Transport,
    change: Option<&DpiChange>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    execute_with_slots(
        product_id,
        physical_product_id,
        transport,
        change,
        None,
        validate,
    )
}

/// The retained, projected OBM associations come from this connection's init.
/// Its lifecycle owner validates the complete target/owner/generation identity.
pub fn execute_with_slots(
    product_id: u32,
    physical_product_id: u32,
    transport: &mut dyn Transport,
    change: Option<&DpiChange>,
    slots: Option<&initialization::ObservedSlots>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    ensure!(
        accepts(product_id, physical_product_id),
        "Current DPI BLE product identity differs"
    );
    if let Some(change) = change {
        validate_change_for(product_id, change)?;
        if change.stages.is_some() && change.profile_guid.is_some() {
            ensure!(
                change
                    .profile_guid
                    .as_ref()
                    .is_some_and(|guid| !guid.is_empty()),
                "BLE active profile GUID is empty"
            );
            let observed = slots
                .context("BLE DPI edits require this connection's initialized OBM associations")?;
            observed.validate()?;
            ensure!(
                observed.target.product_id == product_id
                    && observed.target.physical_product_id == physical_product_id,
                "BLE DPI initialization belongs to another product"
            );
        }
    }
    validate()?;
    let mut report = DpiReport {
        requested: change.cloned(),
        stages: None,
        stage_level: None,
        active: None,
        acknowledgements: vec![],
        serial_number: None,
        initialization_mode_error: None,
        profile_slots: vec![],
        profile_slot_writes: vec![],
        read_errors: DpiTaskErrors::default(),
        write_errors: DpiTaskErrors::default(),
        verified: false,
    };
    if change.is_none() || change.is_some_and(|change| change.stages.is_some()) {
        let mut interruption = None;
        let result = (|| -> anyhow::Result<()> {
            let packed = change
                .and_then(|change| change.stages.as_ref())
                .map(|stages| pack_for(product_id, stages))
                .transpose()?;
            if let Some(packed) = &packed {
                report.acknowledgements.push(exchange(
                    transport,
                    SET_STAGES,
                    STAGES_PROFILE,
                    &stage_payload(packed),
                    validate,
                )?);
            }
            let observed = stage_reading(
                &exchange(transport, GET_STAGES, STAGES_PROFILE, &[], validate)?,
                STAGES_PROFILE,
            )?;
            report.stages = Some(observed);
            if let Some(packed) = &packed {
                ensure!(
                    same_stages(packed, report.stages.as_ref().expect("actual BLE getter")),
                    "Current DPI BLE stage setter acknowledged but readback differs"
                );
                if let Some(guid) = change.and_then(|change| change.profile_guid.as_deref()) {
                    report.profile_slots = slots
                        .expect("prevalidated BLE initialization")
                        .profiles
                        .clone();
                    let synchronization = synchronize_profile_slots(
                        transport,
                        guid,
                        &report.profile_slots,
                        packed,
                        validate,
                    );
                    report.profile_slot_writes = synchronization.writes;
                    if let Some(error) = synchronization.interrupted {
                        interruption = Some(error.clone());
                        anyhow::bail!(error);
                    }
                    let failures: Vec<_> = report
                        .profile_slot_writes
                        .iter()
                        .filter_map(|slot| slot.error.as_deref())
                        .collect();
                    ensure!(
                        failures.is_empty(),
                        "BLE OBM DPI synchronization failed: {}",
                        failures.join("; ")
                    );
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            if change.is_none() {
                report.read_errors.stages = Some(format!("{error:#}"));
            } else {
                report.write_errors.stages = Some(format!("{error:#}"));
            }
        }
        if let Some(error) = interruption {
            return interrupted_report(report, anyhow::Error::msg(error));
        }
        if let Err(error) = validate() {
            return interrupted_report(report, error);
        }
    }
    if change.is_none() || change.is_some_and(|change| change.active.is_some()) {
        let result = (|| -> anyhow::Result<()> {
            let mut observed = level_reading(&exchange(
                transport,
                GET_LEVEL,
                ACTIVE_PROFILE,
                &[],
                validate,
            )?)?;
            if let Some(wanted) = change.and_then(|change| change.active) {
                if observed != wanted {
                    let mut payload = Vec::from(wanted.x.to_le_bytes());
                    payload.extend_from_slice(&wanted.y.to_le_bytes());
                    payload.extend_from_slice(&[0, 0]);
                    report.acknowledgements.push(exchange(
                        transport,
                        SET_LEVEL,
                        ACTIVE_PROFILE,
                        &payload,
                        validate,
                    )?);
                    observed = level_reading(&exchange(
                        transport,
                        GET_LEVEL,
                        ACTIVE_PROFILE,
                        &[],
                        validate,
                    )?)?;
                }
                report.active = Some(observed);
                ensure!(
                    observed == wanted,
                    "Current DPI BLE active setter acknowledged but readback differs"
                );
            }
            report.active = Some(observed);
            Ok(())
        })();
        if let Err(error) = result {
            if change.is_none() {
                report.read_errors.active = Some(format!("{error:#}"));
            } else {
                report.write_errors.active = Some(format!("{error:#}"));
            }
        }
    }
    if let Err(error) = validate() {
        return interrupted_report(report, error);
    }
    report.verified = report.read_errors.is_empty() && report.write_errors.is_empty();
    validate_readings(&report)?;
    Ok(report)
}

fn interrupted_report(mut report: DpiReport, error: anyhow::Error) -> anyhow::Result<DpiReport> {
    let message = format!("{error:#}");
    match report.requested.as_ref() {
        Some(change) => {
            if change.stages.is_some() {
                report.write_errors.stages.get_or_insert(message.clone());
            }
            if change.active.is_some() {
                report.write_errors.active.get_or_insert(message);
            }
        }
        None => {
            if report.stages.is_none() {
                report.read_errors.stages.get_or_insert(message.clone());
            }
            if report.active.is_none() {
                report.read_errors.active.get_or_insert(message);
            }
        }
    }
    report.verified = report.read_errors.is_empty() && report.write_errors.is_empty();
    validate_readings(&report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mouse_current_dpi::{SourceStage, SourceStages};
    use std::{cell::Cell, collections::VecDeque, rc::Rc};

    struct Step {
        command: [u8; 3],
        profile: u8,
        payload: Vec<u8>,
        response: anyhow::Result<Vec<u8>>,
    }

    impl Step {
        fn read(command: [u8; 3], profile: u8, response: &[u8]) -> Self {
            Self {
                command,
                profile,
                payload: vec![],
                response: Ok(response.to_vec()),
            }
        }

        fn write(command: [u8; 3], profile: u8, payload: &[u8]) -> Self {
            Self {
                command,
                profile,
                payload: payload.to_vec(),
                response: Ok(vec![]),
            }
        }

        fn fail(command: [u8; 3], profile: u8) -> Self {
            Self {
                command,
                profile,
                payload: vec![],
                response: Err(anyhow::anyhow!("scripted device rejection")),
            }
        }
    }

    struct Script {
        steps: VecDeque<Step>,
        calls: usize,
        cancelled: Rc<Cell<bool>>,
        cancel_after: Option<usize>,
        cancel_before: Option<usize>,
        wrong_profile: bool,
    }

    impl Script {
        fn new(steps: impl IntoIterator<Item = Step>) -> Self {
            Self {
                steps: steps.into_iter().collect(),
                calls: 0,
                cancelled: Rc::new(Cell::new(false)),
                cancel_after: None,
                cancel_before: None,
                wrong_profile: false,
            }
        }

        fn done(&self) {
            assert!(self.steps.is_empty(), "unconsumed source command");
        }
    }

    impl Transport for Script {
        fn exchange(
            &mut self,
            command: [u8; 3],
            profile: u8,
            payload: &[u8],
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<Acknowledgement> {
            if self.cancel_before == Some(self.calls) {
                self.cancelled.set(true);
            }
            validate()?;
            let step = self.steps.pop_front().expect("unexpected source command");
            assert_eq!(
                (command, profile, payload),
                (step.command, step.profile, step.payload.as_slice())
            );
            let transaction = (self.calls % 31) as u8;
            self.calls += 1;
            let data = step.response?;
            if self.cancel_after == Some(self.calls) {
                self.cancelled.set(true);
            }
            Ok(Acknowledgement {
                command,
                transaction,
                profile: Some(if self.wrong_profile { profile ^ 1 } else { profile }),
                response_bytes: data.len(),
                data,
            })
        }
    }

    fn stages() -> SourceStages {
        SourceStages::new(
            true,
            2,
            vec![
                SourceStage::new(
                    Some(400u16.into()),
                    Some(500u16.into()),
                    Some(true),
                    Some(true),
                ),
                SourceStage::new(
                    Some(600u16.into()),
                    Some(700u16.into()),
                    Some(true),
                    Some(false),
                ),
            ],
        )
    }

    fn change(stages: Option<SourceStages>, active: Option<DpiPair>) -> DpiChange {
        DpiChange {
            stages,
            active,
            profile_guid: None,
            initialize: false,
        }
    }

    // Current 20063 encodes profile in header byte6, X/Y/Z little-endian,
    // and setDPIStages includes one final padding byte after its records.
    const TABLE_WRITE: &[u8] = &[2, 2, 0, 144, 1, 244, 1, 0, 0, 1, 88, 2, 188, 2, 0, 0, 0];
    const TABLE_READ: &[u8] = &[9, 7, 1, 144, 1, 244, 1, 0, 0, 2, 88, 2, 188, 2, 0, 0];
    const ACTIVE_1000: &[u8] = &[232, 3, 232, 3, 0, 0];
    const ACTIVE_2200_2300: &[u8] = &[152, 8, 252, 8, 0, 0];

    #[test]
    fn literal_protocol30_frames_keep_namespace_profile_and_stage_padding() {
        assert_eq!(
            frame(0, EDITION, 0, &[]).unwrap(),
            [0, 0, 0, 0, 1, 134, 0, 0]
        );
        assert_eq!(
            frame(4, GET_STAGES, 1, &[]).unwrap(),
            [4, 0, 0, 0, 11, 132, 1, 0]
        );
        assert_eq!(
            frame(5, SET_LEVEL, 0, ACTIVE_2200_2300).unwrap(),
            [5, 6, 0, 0, 11, 1, 0, 0, 152, 8, 252, 8, 0, 0]
        );
        let packed = pack_for(226, &stages()).unwrap();
        assert_eq!(stage_payload(&packed), TABLE_WRITE);
        assert_eq!(
            frame(30, SET_STAGES, 1, TABLE_WRITE).unwrap(),
            [
                30, 17, 0, 0, 11, 4, 1, 0, 2, 2, 0, 144, 1, 244, 1, 0, 0, 1, 88, 2, 188, 2, 0, 0, 0
            ]
        );
    }

    #[test]
    fn commands_cannot_cross_usb_and_ble_or_the_two_current_profile_tasks() {
        for (tx, command, profile, data) in [
            (31, GET_LEVEL, 0, &[][..]),
            (0, [7, 4, 133], 0, &[][..]),
            (0, GET_MAX_PROFILES, 1, &[][..]),
            (0, GET_STAGES, 0, &[][..]),
            (0, EDITION, 1, &[][..]),
            (0, SET_LEVEL, 0, &[0, 0][..]),
            (0, SET_STAGES, 1, &[1, 1, 0][..]),
        ] {
            assert!(frame(tx, command, profile, data).is_err());
        }
        for (product, physical) in [(190, 191), (226, 226), (226, 227), (228, 228)] {
            let mut script = Script::new([]);
            assert!(execute(product, physical, &mut script, None, &|| Ok(())).is_err());
            assert_eq!(script.calls, 0);
        }
        assert!(accepts(226, 228));
    }

    #[test]
    fn reads_stage_then_active_without_default_initialization_or_mutation() {
        let mut script = Script::new([
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_LEVEL, 0, ACTIVE_2200_2300),
        ]);
        let report = execute(226, 228, &mut script, None, &|| Ok(())).unwrap();
        script.done();
        assert!(report.verified);
        let table = report.stages.unwrap();
        assert_eq!(
            (
                table.profile_id,
                table.active_stage,
                table.number_of_active_stages
            ),
            (1, 9, 7)
        );
        assert_eq!((table.records[0].x, table.records[0].y), (400, 500));
        assert_eq!(report.active, Some(DpiPair { x: 2200, y: 2300 }));
        assert!(report.acknowledgements.is_empty());
        assert!(report.profile_slots.is_empty() && report.profile_slot_writes.is_empty());
    }

    #[test]
    fn writes_follow_source_set_get_and_active_get_conditional_set_get_order() {
        let change = change(Some(stages()), Some(DpiPair { x: 2200, y: 2300 }));
        let mut script = Script::new([
            Step::write(SET_STAGES, 1, TABLE_WRITE),
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_LEVEL, 0, ACTIVE_1000),
            Step::write(SET_LEVEL, 0, ACTIVE_2200_2300),
            Step::read(GET_LEVEL, 0, ACTIVE_2200_2300),
        ]);
        let report = execute(226, 228, &mut script, Some(&change), &|| Ok(())).unwrap();
        script.done();
        assert!(report.verified);
        assert_eq!(report.requested, Some(change));
        assert_eq!(
            report
                .acknowledgements
                .iter()
                .map(|ack| (ack.command, ack.profile))
                .collect::<Vec<_>>(),
            [(SET_STAGES, Some(1)), (SET_LEVEL, Some(0))]
        );
        assert_eq!(report.active, Some(DpiPair { x: 2200, y: 2300 }));
    }

    #[test]
    fn an_equal_active_getter_does_not_generate_a_setter_acknowledgement() {
        let change = change(None, Some(DpiPair { x: 1000, y: 1000 }));
        let mut script = Script::new([Step::read(GET_LEVEL, 0, ACTIVE_1000)]);
        let report = execute(226, 228, &mut script, Some(&change), &|| Ok(())).unwrap();
        script.done();
        assert!(report.verified && report.acknowledgements.is_empty());
        assert!(report.stages.is_none());
    }

    #[test]
    fn independent_getter_failures_keep_only_the_values_actually_observed() {
        for stage_failed in [false, true] {
            let mut script = Script::new([
                if stage_failed {
                    Step::fail(GET_STAGES, 1)
                } else {
                    Step::read(GET_STAGES, 1, TABLE_READ)
                },
                if stage_failed {
                    Step::read(GET_LEVEL, 0, ACTIVE_2200_2300)
                } else {
                    Step::fail(GET_LEVEL, 0)
                },
            ]);
            let report = execute(226, 228, &mut script, None, &|| Ok(())).unwrap();
            script.done();
            assert!(!report.verified);
            assert_eq!(report.stages.is_none(), stage_failed);
            assert_eq!(report.read_errors.stages().is_some(), stage_failed);
            assert_eq!(report.active.is_some(), stage_failed);
            assert_eq!(report.read_errors.active().is_none(), stage_failed);
            assert!(report.write_errors.is_empty() && report.acknowledgements.is_empty());
        }
    }

    #[test]
    fn failed_stage_confirmation_does_not_discard_the_active_task_or_actual_table() {
        let change = change(Some(stages()), Some(DpiPair { x: 1000, y: 1000 }));
        let mut different = TABLE_READ.to_vec();
        different[3] = 145;
        let mut script = Script::new([
            Step::write(SET_STAGES, 1, TABLE_WRITE),
            Step::read(GET_STAGES, 1, &different),
            Step::read(GET_LEVEL, 0, ACTIVE_1000),
        ]);
        let report = execute(226, 228, &mut script, Some(&change), &|| Ok(())).unwrap();
        script.done();
        assert!(!report.verified && report.write_errors.stages().is_some());
        assert!(report.write_errors.active().is_none());
        assert_eq!(report.stages.unwrap().records[0].x, 401);
        assert_eq!(report.active, Some(DpiPair { x: 1000, y: 1000 }));
        assert_eq!(report.acknowledgements.len(), 1);
    }

    #[test]
    fn failed_active_readback_keeps_the_real_getter_and_setter_acknowledgement() {
        let change = change(None, Some(DpiPair { x: 2200, y: 2300 }));
        let mut script = Script::new([
            Step::read(GET_LEVEL, 0, ACTIVE_1000),
            Step::write(SET_LEVEL, 0, ACTIVE_2200_2300),
            Step::read(GET_LEVEL, 0, &[53, 8, 153, 8, 0, 0]),
        ]);
        let report = execute(226, 228, &mut script, Some(&change), &|| Ok(())).unwrap();
        script.done();
        assert!(!report.verified && report.write_errors.active().is_some());
        assert_eq!(report.active, Some(DpiPair { x: 2101, y: 2201 }));
        assert_eq!(report.acknowledgements[0].profile, Some(0));
    }

    #[test]
    fn explicit_zero_terminal_keeps_absent_axes_unobserved_and_nonzero_truncation_fails() {
        for suffix in [
            &[][..],
            &[0][..],
            &[0, 99, 77][..],
            &[0, 0, 0, 0, 0, 0, 0][..],
        ] {
            let mut data = TABLE_READ.to_vec();
            data.extend_from_slice(suffix);
            let mut script = Script::new([
                Step::write(SET_STAGES, 1, TABLE_WRITE),
                Step::read(GET_STAGES, 1, &data),
            ]);
            let report = execute(
                226,
                228,
                &mut script,
                Some(&change(Some(stages()), None)),
                &|| Ok(()),
            )
            .unwrap();
            script.done();
            assert!(report.verified);
            let table = report.stages.unwrap();
            assert_eq!(table.response_bytes, data.len());
            assert_eq!(table.records.len(), if suffix.len() == 7 { 3 } else { 2 });
        }
        for data in [&[1][..], &[1, 2, 1][..], &[1, 2, 1, 144, 1][..]] {
            let mut script = Script::new([
                Step::read(GET_STAGES, 1, data),
                Step::read(GET_LEVEL, 0, ACTIVE_1000),
            ]);
            let report = execute(226, 228, &mut script, None, &|| Ok(())).unwrap();
            script.done();
            assert!(!report.verified && report.stages.is_none());
            assert!(report.read_errors.stages().is_some());
        }
    }

    #[test]
    fn cancellation_after_a_setter_prevents_getter_and_following_independent_task() {
        let mut script = Script::new([Step::write(SET_STAGES, 1, TABLE_WRITE)]);
        let cancelled = script.cancelled.clone();
        script.cancel_after = Some(1);
        let report = execute(
            226,
            228,
            &mut script,
            Some(&change(Some(stages()), Some(DpiPair { x: 1000, y: 1000 }))),
            &|| {
                ensure!(!cancelled.get(), "connection generation changed");
                Ok(())
            },
        )
        .unwrap();
        assert!(!report.verified);
        assert!(report.write_errors.stages().is_some() && report.write_errors.active().is_some());
        assert!(report.stages.is_none() && report.active.is_none());
        assert_eq!(report.acknowledgements.len(), 1);
        assert_eq!(report.acknowledgements[0].command, SET_STAGES);
        assert_eq!(report.acknowledgements[0].profile, Some(1));
        assert_eq!(report.acknowledgements[0].response_bytes, 0);
        script.done();
        assert_eq!(script.calls, 1);
    }

    #[test]
    fn canceled_read_keeps_confirmed_stages_and_stops_before_the_active_getter() {
        let mut script = Script::new([Step::read(GET_STAGES, 1, TABLE_READ)]);
        let cancelled = script.cancelled.clone();
        script.cancel_before = Some(1);
        let report = execute(226, 228, &mut script, None, &|| {
            ensure!(!cancelled.get(), "read owner canceled");
            Ok(())
        })
        .unwrap();
        script.done();
        assert_eq!(script.calls, 1);
        assert!(report.stages.is_some());
        assert!(report.read_errors.stages().is_none());
        assert!(report.active.is_none());
        assert!(report.read_errors.active().is_some());
        assert!(report.acknowledgements.is_empty());
        assert!(!report.verified);
        validate_readings(&report).unwrap();
    }

    #[test]
    fn interruption_after_both_getters_keeps_their_confirmed_values_and_task_results() {
        let mut script = Script::new([
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_LEVEL, 0, ACTIVE_1000),
        ]);
        let actual = execute(226, 228, &mut script, None, &|| Ok(())).unwrap();
        let actual_stages = actual.stages.clone();
        let report = interrupted_report(actual, anyhow::anyhow!("read owner canceled")).unwrap();
        script.done();
        assert_eq!(script.calls, 2);
        assert_eq!(report.stages, actual_stages);
        assert_eq!(report.active, Some(DpiPair { x: 1000, y: 1000 }));
        assert!(report.read_errors.is_empty() && report.write_errors.is_empty());
        assert!(report.verified);
        assert!(report.acknowledgements.is_empty());
        validate_readings(&report).unwrap();
    }

    #[test]
    fn a_response_from_another_request_profile_cannot_confirm_either_task() {
        let mut script = Script::new([
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_LEVEL, 0, ACTIVE_1000),
        ]);
        script.wrong_profile = true;
        let report = execute(226, 228, &mut script, None, &|| Ok(())).unwrap();
        script.done();
        assert!(!report.verified && report.stages.is_none() && report.active.is_none());
        assert!(report.read_errors.stages().is_some() && report.read_errors.active().is_some());
    }

    #[test]
    fn usb_acknowledgements_keep_the_legacy_wire_shape_and_no_ble_profile_claim() {
        let legacy =
            serde_json::json!({"command":[7,4,5],"transaction":2,"response_bytes":0,"data":[]});
        let ack: Acknowledgement = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(ack.profile, None);
        assert_eq!(serde_json::to_value(ack).unwrap(), legacy);
    }

    fn metadata_steps(profile: u8, guid: [u8; 16]) -> Vec<Step> {
        let mut metadata = [0; 250];
        metadata[..16].copy_from_slice(&guid);
        (0..250u16)
            .step_by(78)
            .map(|offset| {
                let count = (250 - offset).min(78);
                let mut payload = Vec::from(offset.to_le_bytes());
                payload.extend_from_slice(&count.to_le_bytes());
                let mut response = vec![0, 0];
                response
                    .extend_from_slice(&metadata[usize::from(offset)..usize::from(offset + count)]);
                Step {
                    command: GET_PROFILE_NAME,
                    profile,
                    payload,
                    response: Ok(response),
                }
            })
            .collect()
    }

    const GUID_BYTES: [u8; 16] = [
        0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0xf0, 0xde, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde,
        0xf0,
    ];
    const GUID_TEXT: &str = "12345678-9abc-def0-1234-56789abcdef0";

    fn read_profile_associations(
        transport: &mut dyn Transport,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<ObservedProfileSlot>> {
        let inventory = read_inventory(transport, validate)?;
        inventory
            .profile_ids
            .into_iter()
            .skip(1)
            .map(|slot_id| {
                Ok(ObservedProfileSlot {
                    slot_id,
                    guid: read_profile_identity(transport, slot_id, validate)?.guid,
                })
            })
            .collect()
    }

    #[test]
    fn inventory_preserves_max_count_ids_and_source_78_byte_metadata_projection() {
        assert_eq!(
            frame(6, GET_MAX_PROFILES, 0, &[]).unwrap(),
            [6, 0, 0, 0, 3, 137, 0, 0]
        );
        assert_eq!(
            frame(7, GET_PROFILE_IDS, 0, &[]).unwrap(),
            [7, 0, 0, 0, 3, 128, 0, 0]
        );
        assert_eq!(
            frame(8, GET_PROFILE_NAME, 9, &[234, 0, 16, 0]).unwrap(),
            [8, 4, 0, 0, 3, 132, 9, 0, 234, 0, 16, 0]
        );
        let mut steps = vec![
            Step::read(GET_MAX_PROFILES, 0, &[4]),
            Step::read(GET_PROFILE_IDS, 0, &[1, 9]),
            Step::read(GET_PROFILE_IDS, 0, &[1, 9]),
        ];
        steps.extend(metadata_steps(9, GUID_BYTES));
        let mut script = Script::new(steps);
        let observed = read_profile_associations(&mut script, &|| Ok(())).unwrap();
        script.done();
        assert_eq!(
            observed,
            [ObservedProfileSlot {
                slot_id: 9,
                guid: GUID_TEXT.into()
            }]
        );
        assert_eq!(script.calls, 7);
    }

    #[test]
    fn profile_slot_synchronization_preserves_source_prefix_and_header_comparison() {
        let packed = pack_for(226, &stages()).unwrap();
        let observed = [
            ObservedProfileSlot {
                slot_id: 9,
                guid: GUID_TEXT.into(),
            },
            ObservedProfileSlot {
                slot_id: 4,
                guid: "another local profile".into(),
            },
            ObservedProfileSlot {
                slot_id: 7,
                guid: GUID_TEXT.into(),
            },
        ];
        // Tt accepts a nonempty matching returned prefix, while independently
        // requiring active/count. A following index zero stops its comparison.
        let equal_prefix = [2, 2, 1, 144, 1, 244, 1, 0, 0, 0];
        let different_count = [2, 1, 1, 144, 1, 244, 1, 0, 0, 0];
        let mut script = Script::new([
            Step::read(GET_STAGES, 9, &equal_prefix),
            Step::read(GET_STAGES, 7, &different_count),
            Step::write(SET_STAGES, 7, TABLE_WRITE),
        ]);
        let progress =
            synchronize_profile_slots(&mut script, GUID_TEXT, &observed, &packed, &|| Ok(()));
        assert!(progress.interrupted.is_none());
        let actual = progress.writes;
        script.done();
        assert_eq!(actual.len(), 2);
        assert!(actual[0].was_equal && actual[0].acknowledged.is_none());
        assert_eq!(actual[0].slot_id, 9);
        assert_eq!(actual[0].before.as_ref().unwrap().profile_id, 9);
        assert!(!actual[1].was_equal && actual[1].acknowledged.is_some());
        assert_eq!(actual[1].slot_id, 7);
        assert!(actual.iter().all(|slot| slot.error.is_none()));
    }

    #[test]
    fn empty_slot_records_require_a_setter_and_failed_slots_do_not_skip_later_slots() {
        let packed = pack_for(226, &stages()).unwrap();
        let observed = [
            ObservedProfileSlot {
                slot_id: 3,
                guid: GUID_TEXT.into(),
            },
            ObservedProfileSlot {
                slot_id: 4,
                guid: GUID_TEXT.into(),
            },
        ];
        let mut script = Script::new([
            Step::fail(GET_STAGES, 3),
            Step::read(GET_STAGES, 4, &[2, 2]),
            Step::write(SET_STAGES, 4, TABLE_WRITE),
        ]);
        let progress =
            synchronize_profile_slots(&mut script, GUID_TEXT, &observed, &packed, &|| Ok(()));
        assert!(progress.interrupted.is_none());
        let actual = progress.writes;
        script.done();
        assert_eq!(actual.len(), 2);
        assert!(actual[0].before.is_none() && actual[0].error.is_some());
        assert!(!actual[1].was_equal && actual[1].acknowledged.is_some());
        assert!(actual[1].error.is_none());
    }

    #[test]
    fn stage_write_keeps_slot_get_before_conditional_set_without_invented_readback() {
        let mut change = change(Some(stages()), None);
        change.profile_guid = Some(GUID_TEXT.into());
        let steps = vec![
            Step::write(SET_STAGES, 1, TABLE_WRITE),
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_STAGES, 9, TABLE_READ),
            Step::write(SET_STAGES, 9, TABLE_WRITE),
        ];
        let mut script = Script::new(steps);
        let slots = initialization::ObservedSlots {
            target: WindowsBleTarget {
                product_id: 226,
                physical_product_id: 228,
                device_container_id: "container".into(),
                hid_path: "hid".into(),
                device_instance_id: "device".into(),
                parent_instance_id: "parent".into(),
                mac_address: 1,
            },
            connection_owner: "source-connection".into(),
            generation: 7,
            profiles: vec![ObservedProfileSlot {
                slot_id: 9,
                guid: GUID_TEXT.into(),
            }],
        };
        let report =
            execute_with_slots(226, 228, &mut script, Some(&change), Some(&slots), &|| {
                Ok(())
            })
            .unwrap();
        script.done();
        assert!(report.verified);
        assert_eq!(report.profile_slots[0].slot_id, 9);
        assert_eq!(report.profile_slot_writes.len(), 1);
        assert_eq!(
            report.profile_slot_writes[0]
                .before
                .as_ref()
                .unwrap()
                .active_stage,
            9
        );
        assert!(report.profile_slot_writes[0].acknowledged.is_some());
        assert_eq!(report.acknowledgements.len(), 1);
        assert_eq!(report.acknowledgements[0].profile, Some(1));
    }

    #[test]
    fn cancellation_retains_completed_slot_ack_and_stops_later_slots_and_active_task() {
        let mut change = change(Some(stages()), Some(DpiPair { x: 1000, y: 1000 }));
        change.profile_guid = Some(GUID_TEXT.into());
        let mut script = Script::new([
            Step::write(SET_STAGES, 1, TABLE_WRITE),
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_STAGES, 9, TABLE_READ),
            Step::write(SET_STAGES, 9, TABLE_WRITE),
        ]);
        let cancelled = script.cancelled.clone();
        script.cancel_after = Some(4);
        let slots = initialization::ObservedSlots {
            target: WindowsBleTarget {
                product_id: 226,
                physical_product_id: 228,
                device_container_id: "container".into(),
                hid_path: "hid".into(),
                device_instance_id: "device".into(),
                parent_instance_id: "parent".into(),
                mac_address: 1,
            },
            connection_owner: "source-connection".into(),
            generation: 7,
            profiles: [9, 7]
                .map(|slot_id| ObservedProfileSlot {
                    slot_id,
                    guid: GUID_TEXT.into(),
                })
                .to_vec(),
        };
        let report =
            execute_with_slots(226, 228, &mut script, Some(&change), Some(&slots), &|| {
                ensure!(!cancelled.get(), "source connection canceled");
                Ok(())
            })
            .unwrap();
        script.done();
        assert_eq!(script.calls, 4);
        assert!(!report.verified);
        assert!(report.write_errors.stages().is_some() && report.write_errors.active().is_some());
        assert!(report.stages.is_some() && report.active.is_none());
        assert_eq!(report.acknowledgements.len(), 1);
        assert_eq!(report.profile_slots, slots.profiles);
        assert_eq!(report.profile_slot_writes.len(), 1);
        let completed = &report.profile_slot_writes[0];
        assert_eq!(completed.slot_id, 9);
        assert_eq!(completed.before.as_ref().unwrap().active_stage, 9);
        assert!(completed.acknowledged.is_some());
        assert!(!completed.was_equal && completed.error.is_none());
        assert_eq!(completed.acknowledged.as_ref().unwrap().response_bytes, 0);
        validate_readings(&report).unwrap();
    }

    #[test]
    fn profile_stage_edit_without_initialized_associations_stops_before_any_write() {
        let mut change = change(Some(stages()), None);
        change.profile_guid = Some(GUID_TEXT.into());
        let mut script = Script::new([]);
        assert!(execute(226, 228, &mut script, Some(&change), &|| Ok(())).is_err());
        assert_eq!(script.calls, 0);
    }

    fn confirmed_slot_report() -> DpiReport {
        let mut change = change(Some(stages()), None);
        change.profile_guid = Some(GUID_TEXT.into());
        let mut script = Script::new([
            Step::write(SET_STAGES, 1, TABLE_WRITE),
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_STAGES, 9, TABLE_READ),
            Step::write(SET_STAGES, 9, TABLE_WRITE),
        ]);
        let slots = initialization::ObservedSlots {
            target: WindowsBleTarget {
                product_id: 226,
                physical_product_id: 228,
                device_container_id: "container".into(),
                hid_path: "hid".into(),
                device_instance_id: "device".into(),
                parent_instance_id: "parent".into(),
                mac_address: 1,
            },
            connection_owner: "source-connection".into(),
            generation: 7,
            profiles: vec![ObservedProfileSlot {
                slot_id: 9,
                guid: GUID_TEXT.into(),
            }],
        };
        let report =
            execute_with_slots(226, 228, &mut script, Some(&change), Some(&slots), &|| {
                Ok(())
            })
            .unwrap();
        script.done();
        report
    }

    #[test]
    fn slot_delivery_validation_recomputes_equality_from_the_actual_before_getter() {
        let valid = confirmed_slot_report();
        validate_readings(&valid).unwrap();
        let mut forged = valid.clone();
        forged.profile_slot_writes[0].was_equal = true;
        forged.profile_slot_writes[0].acknowledged = None;
        assert!(validate_readings(&forged).is_err());
        let mut forged = valid.clone();
        forged.profile_slot_writes[0].before = None;
        assert!(validate_readings(&forged).is_err());
        let mut forged = valid.clone();
        forged.profile_slot_writes.clear();
        assert!(validate_readings(&forged).is_err());
        let mut forged = valid.clone();
        forged.profile_slots[0].slot_id = 1;
        assert!(validate_readings(&forged).is_err());
        let mut forged = valid;
        forged.profile_slots.push(forged.profile_slots[0].clone());
        assert!(validate_readings(&forged).is_err());
    }

    #[test]
    fn failed_slot_still_validates_retained_acknowledgement_bytes_and_identity() {
        let mut failed = confirmed_slot_report();
        failed.verified = false;
        failed.write_errors.stages = Some("slot operation later failed".into());
        failed.profile_slot_writes[0].error =
            Some("setter error retained with actual delivery".into());
        validate_readings(&failed).unwrap();
        let mut forged = failed.clone();
        forged.profile_slot_writes[0]
            .acknowledged
            .as_mut()
            .unwrap()
            .response_bytes = 1;
        assert!(validate_readings(&forged).is_err());
        let mut forged = failed.clone();
        forged.profile_slot_writes[0]
            .before
            .as_mut()
            .unwrap()
            .profile_id = 7;
        assert!(validate_readings(&forged).is_err());
        let mut forged = failed.clone();
        forged.profile_slot_writes[0].slot_id = 7;
        assert!(validate_readings(&forged).is_err());
        let mut forged = failed;
        forged.profile_slot_writes[0].error = Some(String::new());
        assert!(validate_readings(&forged).is_err());
    }

    #[test]
    fn top_level_setter_delivery_requires_the_exact_requested_task_profile_and_bytes() {
        let valid = confirmed_slot_report();
        for (command, profile, bytes) in [
            (SET_STAGES, Some(0), 0),
            (SET_STAGES, None, 0),
            (SET_LEVEL, Some(0), 0),
            (SET_STAGES, Some(1), 1),
        ] {
            let mut forged = valid.clone();
            let ack = &mut forged.acknowledgements[0];
            ack.command = command;
            ack.profile = profile;
            ack.response_bytes = bytes;
            assert!(validate_readings(&forged).is_err());
        }
    }

    #[test]
    fn malformed_inventory_fails_without_using_local_slots_or_default_values() {
        for (maximum, count, ids) in [
            (&[][..], &[1][..], &[1][..]),
            (&[1][..], &[1, 2][..], &[1, 2][..]),
            (&[4][..], &[][..], &[1][..]),
            (&[4][..], &[1][..], &[1, 2][..]),
            (&[4][..], &[1, 1][..], &[1, 1][..]),
            (&[4][..], &[1, 0][..], &[1, 0][..]),
        ] {
            let mut steps = vec![Step::read(GET_MAX_PROFILES, 0, maximum)];
            if !maximum.is_empty() {
                steps.push(Step::read(GET_PROFILE_IDS, 0, count));
                if !count.is_empty() {
                    steps.push(Step::read(GET_PROFILE_IDS, 0, ids));
                }
            }
            let mut script = Script::new(steps);
            assert!(read_profile_associations(&mut script, &|| Ok(())).is_err());
            script.done();
        }
    }

    #[test]
    fn typed_observation_validation_rejects_forged_axes_and_record_byte_boundaries() {
        let mut script = Script::new([
            Step::read(GET_STAGES, 1, TABLE_READ),
            Step::read(GET_LEVEL, 0, ACTIVE_1000),
        ]);
        let mut report = execute(226, 228, &mut script, None, &|| Ok(())).unwrap();
        let valid = report.clone();
        for axis in [0, 50001] {
            report.active = Some(DpiPair { x: axis, y: 1000 });
            assert!(validate_readings(&report).is_err());
        }
        for bytes in [0, 1, 15, 23, 65536] {
            report = valid.clone();
            report.stages.as_mut().unwrap().response_bytes = bytes;
            assert!(
                validate_readings(&report).is_err(),
                "invalid response bytes {bytes}"
            );
        }
        report = valid.clone();
        report.stages.as_mut().unwrap().profile_id = 9;
        assert!(validate_readings(&report).is_err());
        assert!(validate_readings(&valid).is_ok());
    }

    #[test]
    fn serial_query_uses_protocol30_zero_length_command_and_profile_zero() {
        assert_eq!(
            frame(5, GET_SERIAL, 0, &[]).unwrap(),
            [5, 0, 0, 0, 1, 131, 0, 0]
        );
        assert!(frame(5, GET_SERIAL, 1, &[]).is_err());
        assert!(frame(5, GET_SERIAL, 0, &[0]).is_err());
        assert!(frame(5, [22, 0, 130], 0, &[]).is_err());
    }

    #[test]
    fn serial_query_preserves_spaces_and_ascii_after_nul_without_trimming() {
        let mut script = Script::new([Step::read(GET_SERIAL, 0, b" \0A\x1fB\x7f\x80Z ~\r")]);
        assert_eq!(read_serial(&mut script, &|| Ok(())).unwrap(), " ABZ ~");
        script.done();
        assert_eq!(script.calls, 1);
    }

    #[test]
    fn serial_response_uses_actual_variable_data_boundaries_instead_of_usb_width() {
        let mut script = Script::new([Step::read(
            GET_SERIAL,
            0,
            b"1234567890123456789012\0AFTER22 ",
        )]);
        assert_eq!(
            read_serial(&mut script, &|| Ok(())).unwrap(),
            "1234567890123456789012AFTER22 "
        );
        script.done();
        let mut script = Script::new([Step::read(GET_SERIAL, 0, b" ")]);
        assert_eq!(read_serial(&mut script, &|| Ok(())).unwrap(), " ");
        script.done();
    }

    #[test]
    fn empty_or_nonprintable_serial_is_unobserved_without_runtime_sentinel() {
        for data in [&[][..], &[0, 1, 31, 127, 128, 255][..]] {
            let mut script = Script::new([Step::read(GET_SERIAL, 0, data)]);
            assert!(read_serial(&mut script, &|| Ok(())).is_err());
            script.done();
        }
    }

    #[test]
    fn rejected_or_misassociated_serial_reply_never_becomes_a_device_identity() {
        let mut script = Script::new([Step::fail(GET_SERIAL, 0)]);
        assert!(read_serial(&mut script, &|| Ok(())).is_err());
        script.done();
        let mut script = Script::new([Step::read(GET_SERIAL, 0, b"REAL-SERIAL")]);
        script.wrong_profile = true;
        assert!(read_serial(&mut script, &|| Ok(())).is_err());
        script.done();
    }

    #[test]
    fn canceled_serial_query_cannot_select_a_profile_or_continue_to_device_writes() {
        let mut script = Script::new([]);
        assert!(read_serial(&mut script, &|| anyhow::bail!("owner canceled")).is_err());
        assert_eq!(script.calls, 0);
        let mut script = Script::new([Step::read(GET_SERIAL, 0, b"OLD-CONNECTION")]);
        let signal = Rc::clone(&script.cancelled);
        script.cancel_after = Some(1);
        assert!(
            read_serial(&mut script, &|| {
                ensure!(!signal.get(), "connection replaced");
                Ok(())
            })
            .is_err()
        );
        script.done();
        assert_eq!(script.calls, 1);
    }
}
