//! Current rzDevice25 mouse OBM identity/name and slot lifecycle execution.
//! This is the 250-byte profile identity chain, not a complete content save.
//! Caller owns the observed collection, retained session and transaction source.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange::{
        exchange_with_report_data_size, exchange_with_report_data_size_observing_send,
    },
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

pub const AUDITED_PRODUCTS: &[u32] = &[170, 182, 204, 226, 45066];
const METADATA_BYTES: usize = 250;
const BLOCK_BYTES: usize = 64;

fn extend_metadata_sleeps(out_in: &mut u64, input: &mut u64) -> Result<()> {
    let extended = out_in
        .checked_mul(2)
        .context("OBM extended sleep overflow")?;
    *out_in = extended;
    *input = (*input).max(extended);
    Ok(())
}

/// An identity read from the device's ordered profile inventory. The source
/// engine leaves its first inventory entry unnamed, regardless of its ID.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OnboardProfileAssociation {
    pub profile_id: u8,
    pub guid: String,
}

/// Current mouse engine inventory/name reads shared by scroll and DPI tasks.
/// This entry exposes no mutation handle and does not expand the write gate.
pub fn read_profile_associations(
    device: &dyn FeatureTransport,
    capability: &DeviceReadCapability,
    transactions: &mut impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<Vec<OnboardProfileAssociation>> {
    ensure!(
        matches!(
            capability.product_id,
            153 | 170 | 182 | 190 | 203 | 204 | 214 | 226 | 45066
        ) && matches!(
            capability.source_class.as_str(),
            "rzDevice25" | "rzDevice25DualLinkMouse"
        ),
        "No audited mouse profile identity reader for this product"
    );
    let onboard = MouseOnboard::with_metadata_transport(device, capability)?;
    let inventory = onboard.read_inventory(transactions, validate)?;
    let mut profiles = Vec::new();
    for profile_id in inventory.profile_ids.into_iter().skip(1) {
        validate()?;
        let identity = onboard.read_identity(profile_id, transactions, validate)?;
        profiles.push(OnboardProfileAssociation {
            profile_id,
            guid: identity.guid,
        });
    }
    validate()?;
    Ok(profiles)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OnboardInventory {
    pub max_profiles: u8,
    pub profile_count: u8,
    pub profile_ids: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct OnboardIdentityRequest {
    /// Canonical 8-4-4-4-12 hex GUID, converted to the source firmware ordering.
    pub guid: String,
    pub name: String,
    pub owner: String,
    /// Actual cached/observed hash for 170/204. None reproduces six zero bytes.
    /// 226/45066 have no source hash argument and always write six zero bytes.
    pub mapping_hash: Option<[u8; 6]>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardIdentityBlock {
    pub attempt: u8,
    pub offset: u16,
    /// Source getter exposes this raw field but does not use it to slice data.
    pub returned_size: u16,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OnboardIdentityObservation {
    pub profile_id: u8,
    pub guid: String,
    /// Source display semantics, including the no-NUL slice(0,-1) behavior.
    pub name: String,
    /// Source getter reads only 64 of the 128 owner bytes and trims edge NULs.
    pub owner: String,
    pub mapping_hash: Option<[u8; 6]>,
    /// All actual hardware bytes are retained even when display parsing omits them.
    pub raw_metadata: Vec<u8>,
    pub source_guid_accepted: bool,
    pub attempts: u8,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OnboardStorageInfo {
    pub max_macro_id: u16,
    pub max_memory: u32,
    pub available_memory: u32,
    pub recyclable_memory: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardWriteStep {
    pub command: [u8; 3],
    pub report_data_size: u8,
    pub payload: Vec<u8>,
    pub attempted: bool,
    pub acknowledgment_received: bool,
    /// Empty SUCCESS and arbitrary setter data remain ACKs, not readback.
    pub acknowledgment_data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct OnboardWriteReport {
    pub inventory_before: Option<OnboardInventory>,
    pub inventory_after: Option<OnboardInventory>,
    pub steps: Vec<OnboardWriteStep>,
    pub identity_readback_blocks: Vec<OnboardIdentityBlock>,
    pub identity_readback: Option<OnboardIdentityObservation>,
    pub storage_refresh: Option<OnboardStorageInfo>,
    verified: bool,
}
impl OnboardWriteReport {
    pub fn verified(&self) -> bool {
        self.verified
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum OnboardFailurePhase {
    Prepare,
    Observe,
    Write,
    StorageRefresh,
    Readback,
}

#[derive(Debug)]
pub struct OnboardWriteFailure {
    pub phase: OnboardFailurePhase,
    pub progress: OnboardWriteReport,
    pub failed_step: Option<usize>,
    error: anyhow::Error,
}
impl OnboardWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.progress.steps.iter().any(|step| step.attempted)
    }
}
impl std::fmt::Display for OnboardWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse OBM {:?} failed; partial progress retained: {:#}",
            self.phase, self.error
        )
    }
}
impl std::error::Error for OnboardWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

fn guid_bytes(value: &str) -> Result<[u8; 16]> {
    let bytes = value.as_bytes();
    ensure!(
        bytes.len() == 36 && [8, 13, 18, 23].iter().all(|i| bytes[*i] == b'-'),
        "OBM identity requires a canonical GUID"
    );
    let digits: Vec<_> = bytes
        .iter()
        .enumerate()
        .filter_map(|(i, b)| (!matches!(i, 8 | 13 | 18 | 23)).then_some(*b))
        .collect();
    let mut result = [0; 16];
    for (i, pair) in digits.chunks_exact(2).enumerate() {
        let high = char::from(pair[0])
            .to_digit(16)
            .context("Invalid GUID hex digit")?;
        let low = char::from(pair[1])
            .to_digit(16)
            .context("Invalid GUID hex digit")?;
        result[i] = (high * 16 + low) as u8;
    }
    result[..4].reverse();
    result[4..6].reverse();
    result[6..8].reverse();
    Ok(result)
}

pub(crate) fn guid_text(bytes: &[u8]) -> String {
    let mut guid: [u8; 16] = bytes.try_into().expect("16 source GUID bytes");
    guid[..4].reverse();
    guid[4..6].reverse();
    guid[6..8].reverse();
    let mut text = String::with_capacity(36);
    for (index, byte) in guid.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        use std::fmt::Write as _;
        write!(text, "{byte:02x}").expect("String write");
    }
    text
}

/// Exact source data preparation; no JSON parsing or local device defaults.
pub fn encode_identity(product_id: u32, request: &OnboardIdentityRequest) -> Result<Vec<u8>> {
    ensure!(
        AUDITED_PRODUCTS.contains(&product_id),
        "No audited OBM identity for this product"
    );
    let owner: Vec<_> = request.owner.encode_utf16().collect();
    ensure!(
        owner.len() <= 128,
        "Source owner buffer exceeds 128 UTF-16 code units"
    );
    let hash = if matches!(product_id, 170 | 204) {
        request.mapping_hash.unwrap_or([0; 6])
    } else {
        ensure!(
            request.mapping_hash.is_none_or(|hash| hash == [0; 6]),
            "This source branch always writes a zero mapping hash"
        );
        [0; 6]
    };
    let mut result = vec![0; METADATA_BYTES];
    result[..16].copy_from_slice(&guid_bytes(&request.guid)?);
    let name = request.name.as_bytes();
    let count = name.len().min(100);
    result[16..16 + count].copy_from_slice(&name[..count]);
    for (i, unit) in owner.into_iter().enumerate() {
        result[116 + i] = unit as u8;
    }
    result[244..250].copy_from_slice(&hash);
    Ok(result)
}

pub(crate) fn decode_identity(
    product_id: u32,
    profile: u8,
    raw: Vec<u8>,
    attempts: u8,
) -> OnboardIdentityObservation {
    let guid = guid_text(&raw[..16]);
    let name_bytes = &raw[16..116];
    let name_end = name_bytes.iter().position(|b| *b == 0).unwrap_or(99);
    let name_bytes = &name_bytes[..name_end];
    // TextDecoder strips a leading UTF-8 BOM and replaces malformed byte runs.
    let name_bytes = name_bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(name_bytes);
    let owner: String = raw[116..180].iter().map(|b| char::from(*b)).collect();
    let mapping_hash = matches!(product_id, 170 | 204)
        .then(|| raw[244..250].try_into().expect("Six source hash bytes"));
    OnboardIdentityObservation {
        profile_id: profile,
        source_guid_accepted: !guid.contains("0000") && !guid.contains("ffff"),
        guid,
        name: String::from_utf8_lossy(name_bytes).into_owned(),
        owner: owner.trim_matches('\0').to_owned(),
        mapping_hash,
        raw_metadata: raw,
        attempts,
    }
}

pub struct MouseOnboard<'a> {
    device: &'a dyn FeatureTransport,
    transport: DeviceReadCapability,
    metadata_transport: DeviceReadCapability,
}
impl<'a> MouseOnboard<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        capability: &DeviceReadCapability,
    ) -> Result<Self> {
        ensure!(
            AUDITED_PRODUCTS.contains(&capability.product_id)
                && capability.source_class == "rzDevice25DualLinkMouse",
            "No audited direct mouse OBM adapter for this product"
        );
        Self::with_metadata_transport(device, capability)
    }

    fn with_metadata_transport(
        device: &'a dyn FeatureTransport,
        capability: &DeviceReadCapability,
    ) -> Result<Self> {
        // The source temporarily raises both sleeps to extendedSleepTime, which
        // is twice the base out/in delay. This owned copy avoids caller mutation.
        let mut metadata_transport = capability.clone();
        extend_metadata_sleeps(
            &mut metadata_transport.sleep_between_out_in_ms,
            &mut metadata_transport.sleep_between_in_ms,
        )?;
        // for_exchange selects the live sender after this adapter is created.
        // Preserve its metadata timing together with its namespace/interface.
        for branch in metadata_transport.physical_transports_by_pid.values_mut() {
            extend_metadata_sleeps(
                &mut branch.sleep_between_out_in_ms,
                &mut branch.sleep_between_in_ms,
            )?;
        }
        Ok(Self {
            device,
            transport: capability.clone(),
            metadata_transport,
        })
    }

    fn query(
        &self,
        command: [u8; 3],
        payload: &[u8],
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        validate()?;
        let transaction = transactions()?;
        exchange_with_report_data_size(
            self.device,
            &self.transport,
            command,
            payload,
            transaction,
            command[0],
            validate,
        )
    }

    fn query_sized(
        &self,
        command: [u8; 3],
        payload: &[u8],
        data_size: u8,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        validate()?;
        let transaction = transactions()?;
        exchange_with_report_data_size(
            self.device,
            &self.metadata_transport,
            command,
            payload,
            transaction,
            data_size,
            validate,
        )
    }

    /// Source profile-inventory subsequence: maximum, count, then actual ID list.
    pub fn read_inventory(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<OnboardInventory> {
        let maximum = self.query([1, 5, 138], &[], transactions, validate)?;
        let max_profiles = *maximum.first().context("Missing OBM profile capacity")?;
        let count = self.query([1, 5, 128], &[], transactions, validate)?;
        let profile_count = *count.first().context("Missing OBM profile count")?;
        let ids = self.query([80, 5, 129], &[], transactions, validate)?;
        let total = usize::from(*ids.first().context("Missing OBM profile ID count")?);
        ensure!(ids.len() > total, "Truncated OBM profile ID list");
        ensure!(
            total == usize::from(profile_count) && profile_count <= max_profiles,
            "OBM capacity, count and ID list disagree"
        );
        let profile_ids = ids[1..1 + total].to_vec();
        for (i, id) in profile_ids.iter().enumerate() {
            ensure!(
                *id != 0 && !profile_ids[..i].contains(id),
                "Invalid or duplicate OBM profile ID"
            );
        }
        Ok(OnboardInventory {
            max_profiles,
            profile_count,
            profile_ids,
        })
    }

    pub fn read_storage_info(
        &self,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<OnboardStorageInfo> {
        let raw = self.query([14, 6, 142], &[], transactions, validate)?;
        ensure!(raw.len() >= 14, "Truncated OBM storage information");
        Ok(OnboardStorageInfo {
            max_macro_id: u16::from_be_bytes(raw[..2].try_into().unwrap()),
            max_memory: u32::from_be_bytes(raw[2..6].try_into().unwrap()),
            available_memory: u32::from_be_bytes(raw[6..10].try_into().unwrap()),
            recyclable_memory: u32::from_be_bytes(raw[10..14].try_into().unwrap()),
        })
    }

    fn read_identity_with_progress(
        &self,
        profile: u8,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        progress: &mut Vec<OnboardIdentityBlock>,
    ) -> Result<OnboardIdentityObservation> {
        ensure!(
            profile != 0,
            "Source mouse identity loader requires an observed nonzero profile"
        );
        for attempt in 1..=10 {
            let mut assembled = Vec::with_capacity(METADATA_BYTES);
            for offset in (0..METADATA_BYTES).step_by(BLOCK_BYTES) {
                let count = (METADATA_BYTES - offset).min(BLOCK_BYTES);
                let offset_bytes = (offset as u16).to_be_bytes();
                let request = [profile, offset_bytes[0], offset_bytes[1], 0, 0];
                // Actual packetSizeSet replaces the method's nominal 80 bytes.
                let raw = self.query_sized(
                    [80, 5, 136],
                    &request,
                    (count + 5) as u8,
                    transactions,
                    validate,
                )?;
                ensure!(raw.len() >= count + 5, "Truncated OBM identity block");
                let returned_offset = u16::from_be_bytes([raw[1], raw[2]]);
                let block = OnboardIdentityBlock {
                    attempt,
                    offset: returned_offset,
                    returned_size: u16::from_be_bytes([raw[3], raw[4]]),
                    data: raw[5..5 + count].to_vec(),
                };
                assembled.extend_from_slice(&block.data);
                progress.push(block);
                ensure!(
                    raw[0] == profile && returned_offset == offset as u16,
                    "OBM identity reply profile or offset differs"
                );
            }
            let observed = decode_identity(self.transport.product_id, profile, assembled, attempt);
            if observed.source_guid_accepted || attempt == 10 {
                return Ok(observed);
            }
        }
        unreachable!("Finite source identity reads")
    }

    pub fn read_identity(
        &self,
        profile: u8,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<OnboardIdentityObservation> {
        self.read_identity_with_progress(profile, transactions, validate, &mut Vec::new())
    }

    fn mutate(
        &self,
        command: [u8; 3],
        payload: Vec<u8>,
        report: &mut OnboardWriteReport,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<()> {
        validate()?;
        let transaction = transactions()?;
        let data_size = if command[2] == 8 {
            payload.len() as u8
        } else {
            command[0]
        };
        report.steps.push(OnboardWriteStep {
            command,
            report_data_size: data_size,
            payload,
            attempted: false,
            acknowledgment_received: false,
            acknowledgment_data: Vec::new(),
        });
        let step = report.steps.last_mut().unwrap();
        let transport = if matches!(command[2], 2 | 8) {
            &self.metadata_transport
        } else {
            &self.transport
        };
        let response = exchange_with_report_data_size_observing_send(
            self.device,
            transport,
            command,
            &step.payload,
            transaction,
            data_size,
            validate,
            &mut || step.attempted = true,
        );
        if let Some(data) = crate::feature_exchange::acknowledged_reply(&response) {
            step.acknowledgment_received = true;
            step.acknowledgment_data = data.to_vec();
        }
        response?;
        Ok(())
    }

    fn failure(
        phase: OnboardFailurePhase,
        report: OnboardWriteReport,
        error: anyhow::Error,
    ) -> OnboardWriteFailure {
        let failed_step = (phase == OnboardFailurePhase::Write)
            .then(|| {
                report
                    .steps
                    .iter()
                    .rposition(|step| !step.acknowledgment_received)
            })
            .flatten();
        OnboardWriteFailure {
            phase,
            progress: report,
            failed_step,
            error,
        }
    }

    /// Current setProfileNameStructure chain: create, then 64/64/64/58 bytes.
    /// Every block ACK is retained; independent full raw readback proves identity.
    pub fn write_identity(
        &self,
        profile: u8,
        request: &OnboardIdentityRequest,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<OnboardWriteReport, OnboardWriteFailure> {
        let report = OnboardWriteReport::default();
        let prepared = (|| {
            ensure!(
                profile > 1,
                "Source mouse identity writer reserves profile 1"
            );
            encode_identity(self.transport.product_id, request)
        })();
        let expected = match prepared {
            Ok(bytes) => bytes,
            Err(error) => return Err(Self::failure(OnboardFailurePhase::Prepare, report, error)),
        };
        self.write_raw_identity(profile, expected, transactions, validate)
    }

    /// Preserve all 250 observed bytes during storage restoration, including
    /// fields which the ordinary display/parser or metadata setter omits.
    pub fn restore_identity(
        &self,
        observed: &OnboardIdentityObservation,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<OnboardWriteReport, OnboardWriteFailure> {
        let prepared = (|| {
            ensure!(
                observed.profile_id > 1,
                "Source mouse identity writer reserves profile 1"
            );
            ensure!(
                observed.raw_metadata.len() == METADATA_BYTES,
                "Observed profile metadata must contain 250 bytes"
            );
            ensure!(
                guid_text(&observed.raw_metadata[..16]) == observed.guid,
                "Observed profile GUID disagrees with its raw bytes"
            );
            Ok(())
        })();
        if let Err(error) = prepared {
            return Err(Self::failure(
                OnboardFailurePhase::Prepare,
                OnboardWriteReport::default(),
                error,
            ));
        }
        self.write_raw_identity(
            observed.profile_id,
            observed.raw_metadata.clone(),
            transactions,
            validate,
        )
    }

    fn write_raw_identity(
        &self,
        profile: u8,
        expected: Vec<u8>,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<OnboardWriteReport, OnboardWriteFailure> {
        let mut report = OnboardWriteReport::default();
        let before = match self.read_inventory(transactions, validate) {
            Ok(value) => value,
            Err(error) => return Err(Self::failure(OnboardFailurePhase::Observe, report, error)),
        };
        let existing = before.profile_ids.contains(&profile);
        report.inventory_before = Some(before.clone());
        if !existing && before.profile_count >= before.max_profiles {
            return Err(Self::failure(
                OnboardFailurePhase::Observe,
                report,
                anyhow::anyhow!("Observed OBM profile capacity is full"),
            ));
        }
        let written = (|| {
            self.mutate(
                [1, 5, 2],
                vec![profile],
                &mut report,
                transactions,
                validate,
            )?;
            for offset in (0..METADATA_BYTES).step_by(BLOCK_BYTES) {
                let count = (METADATA_BYTES - offset).min(BLOCK_BYTES);
                let mut payload = vec![profile];
                payload.extend_from_slice(&(offset as u16).to_be_bytes());
                payload.extend_from_slice(&(METADATA_BYTES as u16).to_be_bytes());
                payload.extend_from_slice(&expected[offset..offset + count]);
                self.mutate([80, 5, 8], payload, &mut report, transactions, validate)?;
            }
            Ok(())
        })();
        if let Err(error) = written {
            return Err(Self::failure(OnboardFailurePhase::Write, report, error));
        }
        let checked = (|| {
            let observed = self.read_identity_with_progress(
                profile,
                transactions,
                validate,
                &mut report.identity_readback_blocks,
            )?;
            let same = observed.raw_metadata == expected;
            report.identity_readback = Some(observed);
            ensure!(
                same,
                "Independent OBM identity readback differs from requested bytes"
            );
            let after = self.read_inventory(transactions, validate)?;
            let mut expected_ids = before.profile_ids.clone();
            if !existing {
                expected_ids.push(profile);
            }
            expected_ids.sort_unstable();
            let mut actual_ids = after.profile_ids.clone();
            actual_ids.sort_unstable();
            let same = expected_ids == actual_ids;
            report.inventory_after = Some(after);
            ensure!(
                same,
                "Independent OBM slot inventory differs after identity write"
            );
            Ok(())
        })();
        if let Err(error) = checked {
            return Err(Self::failure(OnboardFailurePhase::Readback, report, error));
        }
        report.verified = true;
        Ok(report)
    }

    /// Source deleteProfile refreshes macro storage after ACK. Independent count/
    /// list confirmation is an additional Rust policy, never a local-cache save.
    pub fn delete_profile(
        &self,
        profile: u8,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<OnboardWriteReport, OnboardWriteFailure> {
        let mut report = OnboardWriteReport::default();
        if profile <= 1 {
            return Err(Self::failure(
                OnboardFailurePhase::Prepare,
                report,
                anyhow::anyhow!("Source mouse OBM lifecycle reserves profile 1"),
            ));
        }
        let before = match self.read_inventory(transactions, validate) {
            Ok(value) => value,
            Err(error) => return Err(Self::failure(OnboardFailurePhase::Observe, report, error)),
        };
        report.inventory_before = Some(before.clone());
        if !before.profile_ids.contains(&profile) {
            return Err(Self::failure(
                OnboardFailurePhase::Observe,
                report,
                anyhow::anyhow!("OBM profile is absent from the actual device inventory"),
            ));
        }
        if let Err(error) = self.mutate(
            [1, 5, 3],
            vec![profile],
            &mut report,
            transactions,
            validate,
        ) {
            return Err(Self::failure(OnboardFailurePhase::Write, report, error));
        }
        report.storage_refresh = match self.read_storage_info(transactions, validate) {
            Ok(value) => Some(value),
            Err(error) => {
                return Err(Self::failure(
                    OnboardFailurePhase::StorageRefresh,
                    report,
                    error,
                ));
            }
        };
        let after = match self.read_inventory(transactions, validate) {
            Ok(value) => value,
            Err(error) => return Err(Self::failure(OnboardFailurePhase::Readback, report, error)),
        };
        let mut expected_ids: Vec<_> = before
            .profile_ids
            .into_iter()
            .filter(|id| *id != profile)
            .collect();
        expected_ids.sort_unstable();
        let mut actual_ids = after.profile_ids.clone();
        actual_ids.sort_unstable();
        report.inventory_after = Some(after);
        if expected_ids != actual_ids {
            return Err(Self::failure(
                OnboardFailurePhase::Readback,
                report,
                anyhow::anyhow!("Independent OBM profile inventory does not confirm deletion"),
            ));
        }
        report.verified = true;
        Ok(report)
    }
}

#[cfg(test)]
#[path = "mouse_onboard_tests.rs"]
mod tests;
