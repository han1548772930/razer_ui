//! Current ordinary rzDevice25 DPI tasks. Stage profile 1 and transient DPI
//! profile 0 are separate source tasks; independent X/Y is a local flag.
//! Physical receiver/BLE branches are admitted only by their source factory.
use crate::mouse_dpi_stages::{DpiStagesDraft, DpiStagesReading, PackedDpiStage};
use crate::{backend::FeatureTransport, device_reads::DeviceReadCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{sync::OnceLock, time::Duration};

pub const PRODUCT_ID: u32 = 190;
#[cfg(test)]
#[path = "mouse_current_dpi_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize)]
pub struct Capability {
    product_id: u32,
    minimum: u16,
    maximum: u16,
    cycle_sensitivity: bool,
    stage_level_only: bool,
    wired_pid: u32,
    dongle_pid: Option<u32>,
    transport: DeviceReadCapability,
}

impl Capability {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn minimum(&self) -> u16 {
        self.minimum
    }
    pub fn maximum(&self) -> u16 {
        self.maximum
    }
    pub fn wired_pid(&self) -> u32 {
        self.wired_pid
    }
    pub fn dongle_pid(&self) -> Option<u32> {
        self.dongle_pid
    }
    pub fn transport(&self) -> &DeviceReadCapability {
        &self.transport
    }
    pub fn is_stage_level_only(&self) -> bool {
        self.stage_level_only
    }
    pub fn is_physical_branch(&self, physical_pid: u32, is_dongle: bool) -> bool {
        if is_dongle {
            self.dongle_pid == Some(physical_pid)
                && !self
                    .transport
                    .physical_transports_by_pid
                    .get(&physical_pid.to_string())
                    .is_some_and(|branch| branch.is_duallink)
        } else {
            self.wired_pid == physical_pid
        }
    }

    /// The command owner is inherited by the current DualLink mouse sender.
    /// Binding a master still requires the source peer and two live queries.
    pub fn receiver_session(
        &self,
        physical_pid: u32,
        peer_pid: u32,
    ) -> anyhow::Result<crate::receiver_session::ReceiverSession> {
        crate::receiver_session::ReceiverSession::mouse_with_source(
            &self.transport,
            physical_pid,
            peer_pid,
        )
    }
}

pub fn capability(product_id: u32) -> Option<&'static Capability> {
    static CAPS: OnceLock<Vec<Capability>> = OnceLock::new();
    CAPS.get_or_init(|| {
        let mut capabilities: Vec<Capability> =
            serde_json::from_str(include_str!("mouse_current_dpi_data.json"))
                .expect("current DPI source factory and protocol capabilities");
        // Current182 factory87887 selects ordinary wired and DualLink radio
        // senders. Retain those distinct physical branches for each exchange.
        if let Some(cap) = capabilities.iter_mut().find(|cap| cap.product_id == 182) {
            cap.transport = crate::device_reads::capability(182)
                .expect("current182 sender")
                .clone();
        }
        capabilities
    })
    .iter()
    .find(|cap| cap.product_id == product_id)
}

fn required(product_id: u32) -> anyhow::Result<&'static Capability> {
    capability(product_id).with_context(|| {
        format!("PID{product_id} DPI factory/protocol branch has no current source capability")
    })
}

pub fn transport_capability() -> &'static DeviceReadCapability {
    required(PRODUCT_ID)
        .expect("PID190 current DPI capability")
        .transport()
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiPair {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiChange {
    pub stages: Option<DpiStagesDraft>,
    pub active: Option<DpiPair>,
    /// Active local profile identity used by 30525.Ye to select actual OBM slots.
    #[serde(default)]
    pub profile_guid: Option<String>,
    /// ON_STAGES_INIT has device access, without a local save/version increment.
    #[serde(default)]
    pub initialize: bool,
}

impl DpiChange {
    /// The first 90012.gn identity query precedes source profile selection.
    /// This request carries no profile data and cannot change device settings.
    pub fn is_identity_read(&self) -> bool {
        self.initialize
            && self.stages.is_none()
            && self.active.is_none()
            && self.profile_guid.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PackedStages {
    pub active_stage: u8,
    pub number_of_active_stages: u8,
    pub records: Vec<PackedDpiStage>,
}

fn validate_pair(cap: &Capability, value: DpiPair) -> anyhow::Result<()> {
    ensure!(
        (cap.minimum..=cap.maximum).contains(&value.x)
            && (cap.minimum..=cap.maximum).contains(&value.y),
        "DPI is outside PID{}'s source range {}..={}",
        cap.product_id,
        cap.minimum,
        cap.maximum
    );
    Ok(())
}

/// He preserves findIndex=-1 -> activeStage=0 for the hidden-row unlink branch.
/// Disabled staging still declares one record, whose missing JS values become
/// zero bytes. Keep this source edge case observable instead of selecting a row.
pub fn pack(draft: &DpiStagesDraft) -> anyhow::Result<PackedStages> {
    pack_for(PRODUCT_ID, draft)
}

pub fn pack_for(product_id: u32, draft: &DpiStagesDraft) -> anyhow::Result<PackedStages> {
    let cap = required(product_id)?;
    ensure!(
        draft.stages.len() == 5,
        "Current source retains exactly five local DPI stages"
    );
    let selected = draft
        .active_stage
        .checked_sub(1)
        .filter(|index| *index < draft.stages.len())
        .context("DPI selection is outside the local profile")?;
    for row in &draft.stages {
        validate_pair(cap, DpiPair { x: row.x, y: row.y })?;
    }
    let rows: Vec<_> = draft
        .stages
        .iter()
        .enumerate()
        .filter(|(index, row)| {
            row.visible && (cap.cycle_sensitivity || draft.enabled || *index == selected)
        })
        .collect();
    let active_stage = rows
        .iter()
        .position(|(index, _)| *index == selected)
        .map_or(0, |index| index as u8 + 1);
    let number_of_active_stages = if cap.cycle_sensitivity || draft.enabled {
        rows.len() as u8
    } else {
        1
    };
    let records = rows
        .into_iter()
        .enumerate()
        .map(|(index, (_, row))| PackedDpiStage {
            index: index as u8,
            x: row.x,
            y: row.y,
            z: 0,
        })
        .collect();
    Ok(PackedStages {
        active_stage,
        number_of_active_stages,
        records,
    })
}

pub fn validate_change(change: &DpiChange) -> anyhow::Result<()> {
    validate_change_for(PRODUCT_ID, change)
}

pub fn validate_change_for(product_id: u32, change: &DpiChange) -> anyhow::Result<()> {
    let cap = required(product_id)?;
    if change.is_identity_read() {
        ensure!(
            product_id == 182,
            "This source has no audited initialization identity task"
        );
        return Ok(());
    }
    ensure!(
        change.stages.is_some() || change.active.is_some(),
        "Local XY flags need no device operation"
    );
    if let Some(stages) = &change.stages {
        pack_for(product_id, stages)?;
    }
    if let Some(active) = change.active {
        ensure!(
            product_id != 182,
            "PID182 stage edits do not dispatch the separate transient DPI task"
        );
        validate_pair(cap, active)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Acknowledgement {
    pub command: [u8; 3],
    pub transaction: u8,
    pub response_bytes: usize,
    pub data: Vec<u8>,
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &Capability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    let (transport, transaction) = cap.transport().for_exchange(device, transaction)?;
    ensure!(
        transaction & 31 < transport.transaction_modulus
            && transaction & !31 == transport.transaction_prefix
            && transport.report_bytes == 91
            && data.len() <= usize::from(command[0])
            && command[0] <= 80,
        "DPI packet or transaction is outside its source layout"
    );
    let mut outgoing = [0; 91];
    outgoing[0] = transport.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + data.len()].copy_from_slice(data);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let cap = transport.as_ref();
    let mut last = "No DPI response".to_owned();
    for _ in 0..cap.max_retry_out {
        validate()?;
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        validate()?;
        let sent = device.send_feature_at_collection_max(&outgoing);
        validate()?;
        if let Err(error) = sent {
            last = format!("{error:#}");
            continue;
        }
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = [0; 91];
            // for_exchange already selected the exact physical factory (or
            // retained the explicitly bound master) before framing this call.
            incoming[0] = cap.get_report_id();
            let received = device.get_feature_at_collection_max(&mut incoming);
            validate()?;
            let received = match received {
                Ok(value) => value,
                Err(error) => {
                    last = format!("{error:#}");
                    break;
                }
            };
            ensure!(
                received == incoming.len(),
                "DPI response report length differs"
            );
            if incoming[2] != transaction || incoming[7..9] != command[1..] {
                last = "DPI response transaction or command differs".into();
                break;
            }
            match incoming[1] {
                1 => {
                    last = "DPI device remains busy".into();
                }
                2 => {
                    let count = usize::from(incoming[6]);
                    ensure!(count <= 80, "DPI response payload is truncated");
                    return Ok(Acknowledgement {
                        command,
                        transaction,
                        response_bytes: count,
                        data: incoming[9..9 + count].to_vec(),
                    });
                }
                0 | 3 | 4 => {
                    last = format!("DPI response status {}", incoming[1]);
                    break;
                }
                status => anyhow::bail!("DPI command rejected with device status {status}"),
            }
        }
    }
    anyhow::bail!("DPI command exhausted source retries: {last}")
}

/// PID191 is the source's ordinary rzDevice25 dongle, not a dual-link relay.
/// ue() requires the real wireless-connected byte to be one before operations.
pub fn verify_wireless_online(
    device: &dyn FeatureTransport,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    verify_wireless_online_for(PRODUCT_ID, device, transaction, validate)
}

pub fn verify_wireless_online_for(
    product_id: u32,
    device: &dyn FeatureTransport,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let cap = required(product_id)?;
    ensure!(
        cap.dongle_pid.is_some(),
        "DPI source has no ordinary dongle branch"
    );
    let reply = exchange(device, cap, [1, 0, 184], &[0], transaction, &validate)?;
    ensure!(
        reply.data.first() == Some(&1),
        "DPI receiver has not confirmed its wireless mouse online"
    );
    Ok(())
}

fn stage_reading(ack: &Acknowledgement) -> anyhow::Result<DpiStagesReading> {
    let data = &ack.data;
    ensure!(
        data.len() >= 3 && (data.len() - 3).is_multiple_of(7) && data[0] == 1,
        "DPI stage response is truncated or belongs to another profile"
    );
    Ok(DpiStagesReading {
        profile_id: data[0],
        active_stage: data[1],
        number_of_active_stages: data[2],
        records: data[3..]
            .chunks_exact(7)
            .map(|row| PackedDpiStage {
                index: row[0],
                x: u16::from_be_bytes([row[1], row[2]]),
                y: u16::from_be_bytes([row[3], row[4]]),
                z: u16::from_be_bytes([row[5], row[6]]),
            })
            .collect(),
        response_bytes: data.len(),
    })
}

fn level_reading(
    cap: &Capability,
    profile_id: u8,
    ack: &Acknowledgement,
) -> anyhow::Result<DpiPair> {
    ensure!(
        ack.data.len() >= 7 && ack.data[0] == profile_id,
        "Active DPI response is truncated or belongs to another profile"
    );
    let value = DpiPair {
        x: u16::from_be_bytes([ack.data[1], ack.data[2]]),
        y: u16::from_be_bytes([ack.data[3], ack.data[4]]),
    };
    if cap.product_id != 182 {
        validate_pair(cap, value)?;
    }
    Ok(value)
}

fn stage_payload(packed: &PackedStages) -> Vec<u8> {
    let mut bytes = vec![0; 3 + 7 * usize::from(packed.number_of_active_stages)];
    bytes[..3].copy_from_slice(&[1, packed.active_stage, packed.number_of_active_stages]);
    for (index, row) in packed.records.iter().enumerate() {
        let start = 3 + index * 7;
        bytes[start] = row.index;
        bytes[start + 1..start + 3].copy_from_slice(&row.x.to_be_bytes());
        bytes[start + 3..start + 5].copy_from_slice(&row.y.to_be_bytes());
        bytes[start + 5..start + 7].copy_from_slice(&row.z.to_be_bytes());
    }
    bytes
}

pub fn same_stages(sent: &PackedStages, got: &DpiStagesReading) -> bool {
    let records: Vec<_> = got
        .records
        .iter()
        .take_while(|row| row.index != 0)
        .collect();
    got.profile_id == 1
        && got.active_stage == sent.active_stage
        && got.number_of_active_stages == sent.number_of_active_stages
        && records.len() == sent.records.len()
        && records.iter().zip(&sent.records).all(|(got, sent)| {
            got.index == sent.index + 1 && (got.x, got.y, got.z) == (sent.x, sent.y, sent.z)
        })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct DpiReport {
    pub requested: Option<DpiChange>,
    pub stages: Option<DpiStagesReading>,
    /// Present only for an actual profile-1 getter/returned setter value.
    /// A source level-only stage setter is confirmed by its protocol ACK.
    #[serde(default)]
    pub stage_level: Option<DpiPair>,
    pub active: Option<DpiPair>,
    pub acknowledgements: Vec<Acknowledgement>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub initialization_mode_error: Option<String>,
    #[serde(default)]
    pub profile_slots: Vec<crate::mouse_dpi_stages::ObservedProfileSlot>,
    /// Source Mt only pre-reads and conditionally sets; ACK is not readback.
    #[serde(default)]
    pub profile_slot_writes: Vec<crate::mouse_dpi_stages::DpiSlotWriteResult>,
    pub verified: bool,
}

/// Cancellation and identity/deadline checks surround every feature transfer.
/// A failed confirmation leaves the optimistic local profile to its UI owner.
pub fn execute(
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    execute_for(PRODUCT_ID, device, change, next, validate)
}

pub fn execute_for(
    product_id: u32,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    let cap = required(product_id)?;
    execute_with_capability(cap, device, change, next, validate)
}

/// The current182 receiver factory selects its physical master's interface;
/// the mouse retains its own namespace and source tasks on the same handle.
pub(crate) fn execute_receiver(
    transport: &DeviceReadCapability,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    let mut cap = required(transport.product_id)?.clone();
    ensure!(
        transport.source_class == "rzDevice25DualLinkMouse"
            && cap
                .transport
                .physical_transports_by_pid
                .values()
                .any(|branch| branch.is_duallink && branch.source_class == transport.source_class),
        "Receiver DPI source sender differs"
    );
    cap.transport = transport.clone();
    execute_with_capability(&cap, device, change, next, validate)
}

/// Preserve the complete current DPI task (stage 1 / transient 0) between the
/// receiver's before/after peer observations. This is not the stage-only task.
pub fn execute_in_session(
    session: &crate::receiver_session::ReceiverSession,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<crate::receiver_session::Observation<DpiReport>> {
    session.observe(device, next, validate, |next, validate| {
        execute_receiver(session.transport(), device, change, next, validate)
    })
}

fn execute_with_capability(
    cap: &Capability,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    let product_id = cap.product_id;
    if let Some(change) = change {
        validate_change_for(product_id, change)?;
    }
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
        verified: false,
    };
    if product_id == 182 && change.is_some_and(|change| change.initialize) {
        let serial = crate::feature_exchange::exchange(
            device,
            cap.transport(),
            [22, 0, 130],
            &[],
            next()?,
            &validate,
        )?;
        // Current12844.u retains every printable ASCII byte, including spaces.
        let serial: String = serial
            .iter()
            .copied()
            .filter(|byte| (32..127).contains(byte))
            .map(char::from)
            .collect();
        ensure!(
            !serial.is_empty(),
            "PID182 initialization has no queried device serial number"
        );
        report.serial_number = Some(serial);
        if change.is_some_and(DpiChange::is_identity_read) {
            validate()?;
            report.verified = true;
            return Ok(report);
        }
        // 90012.gn -> 87887.Y ->70035.xZ=[3,0]. Y catches a mode
        // failure; it does not pretend success or prevent later profile tasks.
        let mode = (|| -> anyhow::Result<()> {
            let current = crate::feature_exchange::exchange(
                device,
                cap.transport(),
                [2, 0, 132],
                &[],
                next()?,
                &validate,
            )?;
            ensure!(current.len() >= 2, "PID182 mode getter is truncated");
            if current[..2] != [3, 0] {
                crate::feature_exchange::exchange(
                    device,
                    cap.transport(),
                    [2, 0, 4],
                    &[3, 0],
                    next()?,
                    &validate,
                )?;
            }
            Ok(())
        })();
        if let Err(error) = mode {
            report.initialization_mode_error = Some(format!("{error:#}"));
        }
        validate()?;
    }
    let mut errors = Vec::new();
    if change.is_none() || change.is_some_and(|change| change.stages.is_some()) {
        let stage_result = (|| -> anyhow::Result<()> {
            let packed = change
                .and_then(|change| change.stages.as_ref())
                .map(|draft| pack_for(product_id, draft))
                .transpose()?;
            if cap.stage_level_only {
                if let Some(packed) = &packed {
                    let stage = packed
                        .active_stage
                        .checked_sub(1)
                        .and_then(|index| packed.records.get(usize::from(index)))
                        .context("Source selected DPI stage has no numeric X/Y values")?;
                    let mut payload = vec![1];
                    payload.extend_from_slice(&stage.x.to_be_bytes());
                    payload.extend_from_slice(&stage.y.to_be_bytes());
                    payload.extend_from_slice(&[0, 0]);
                    let ack = exchange(device, cap, [7, 4, 5], &payload, next()?, &validate)?;
                    if ack.data.len() >= 7 {
                        report.stage_level = Some(level_reading(cap, 1, &ack)?);
                    }
                    report.acknowledgements.push(ack);
                } else {
                    report.stage_level = Some(level_reading(
                        cap,
                        1,
                        &exchange(device, cap, [7, 4, 133], &[1], next()?, &validate)?,
                    )?);
                }
                return Ok(());
            }
            if let Some(packed) = &packed {
                let payload = stage_payload(packed);
                report.acknowledgements.push(exchange(
                    device,
                    cap,
                    [payload.len() as u8, 4, 6],
                    &payload,
                    next()?,
                    &validate,
                )?);
            }
            let observed = stage_reading(&exchange(
                device,
                cap,
                [80, 4, 134],
                &[1],
                next()?,
                &validate,
            )?)?;
            if let Some(packed) = &packed {
                ensure!(
                    same_stages(packed, &observed),
                    "DPI stage setter acknowledged but readback differs"
                );
            }
            report.stages = Some(observed);
            if product_id == 182
                && let Some(change) = change
                && let (Some(guid), Some(draft)) = (&change.profile_guid, &change.stages)
            {
                ensure!(!guid.is_empty(), "PID182 active profile GUID is empty");
                let onboard = crate::mouse_onboard::MouseOnboard::new(device, cap.transport())?;
                let inventory = onboard.read_inventory(&mut next, &validate)?;
                // Current77303 skips the first profile's identity by index.
                for slot in inventory.profile_ids.iter().skip(1) {
                    let identity = onboard.read_identity(*slot, &mut next, &validate)?;
                    report
                        .profile_slots
                        .push(crate::mouse_dpi_stages::ObservedProfileSlot {
                            slot_id: *slot,
                            guid: identity.guid,
                        });
                }
                let mut stages_cap = crate::mouse_dpi_stages::capability(182)
                    .context("PID182 stage capability absent")?
                    .clone();
                stages_cap.transport = cap.transport().clone();
                report.profile_slot_writes = crate::mouse_dpi_stages::synchronize_profile_slots(
                    device,
                    &stages_cap,
                    guid,
                    &report.profile_slots,
                    draft,
                    &mut next,
                    &validate,
                )?;
                let failures: Vec<_> = report
                    .profile_slot_writes
                    .iter()
                    .filter_map(|slot| slot.error.as_deref())
                    .collect();
                ensure!(
                    failures.is_empty(),
                    "PID182 OBM stage synchronization failed: {}",
                    failures.join("; ")
                );
            }
            Ok(())
        })();
        if let Err(error) = stage_result {
            errors.push(format!("stage table: {error:#}"));
        }
        // Stage and current X/Y are independent source tasks. Device rejection
        // does not discard the other task; scope cancellation stops both.
        validate()?;
    }
    if change.is_none() || change.is_some_and(|change| change.active.is_some()) {
        let active_result = (|| -> anyhow::Result<()> {
            let mut observed = level_reading(
                cap,
                0,
                &exchange(device, cap, [7, 4, 133], &[0], next()?, &validate)?,
            )?;
            if let Some(wanted) = change.and_then(|change| change.active) {
                if observed != wanted {
                    let mut payload = vec![0];
                    payload.extend_from_slice(&wanted.x.to_be_bytes());
                    payload.extend_from_slice(&wanted.y.to_be_bytes());
                    payload.extend_from_slice(&[0, 0]);
                    report.acknowledgements.push(exchange(
                        device,
                        cap,
                        [7, 4, 5],
                        &payload,
                        next()?,
                        &validate,
                    )?);
                    observed = level_reading(
                        cap,
                        0,
                        &exchange(device, cap, [7, 4, 133], &[0], next()?, &validate)?,
                    )?;
                }
                ensure!(
                    observed == wanted,
                    "Active DPI setter acknowledged but readback differs"
                );
            }
            report.active = Some(observed);
            Ok(())
        })();
        if let Err(error) = active_result {
            errors.push(format!("active DPI: {error:#}"));
        }
    }
    validate()?;
    ensure!(
        errors.is_empty(),
        "Current mouse DPI operation is unconfirmed: {}",
        errors.join("; ")
    );
    report.verified = true;
    Ok(report)
}
