//! Current four-model OBM format/compaction with a complete bounded backup.
//! The caller retains the selected Feature session and owns cancellation/time.
//! Raw restoration preserves fields omitted by the source's editor conversion.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange::exchange_with_report_data_size_observing_send,
    mouse_button_inventory::{ButtonInventoryReport, MouseButtonInventory},
    mouse_button_mapping::{ButtonWriteReport, MouseButtonMapping},
    mouse_onboard::{
        MouseOnboard, OnboardIdentityObservation, OnboardInventory, OnboardWriteReport,
    },
    mouse_profile_polling::{
        MouseProfilePolling, ProfilePollingObservation, ProfilePollingWriteReport,
    },
    onboard_macro::{
        MacroFailurePhase, MacroInventory, MacroReading, MacroWriteReport, MacroWriteRequest,
        OnboardMacro,
    },
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;
use std::time::Duration;

const SET_FORMAT: [u8; 3] = [6, 6, 10];
const GET_FORMAT: [u8; 3] = [6, 6, 138];
const FORMAT_SELECTOR: [u8; 4] = [0, 0, 0, 2];

#[derive(Clone, Copy, Debug)]
pub struct StorageReadBounds {
    pub max_macro_bytes: u32,
    pub max_total_macro_bytes: usize,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum StoredSettingKind {
    DpiLevel,
    DpiStages,
    Brightness { region_id: u8 },
    ScrollEnabledModes,
    ScrollMode,
    ScrollAcceleration,
    SmartReel,
    ScrollAccelerationLevel,
    SmartReelThreshold,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct StoredSetting {
    pub kind: StoredSettingKind,
    pub get_command: [u8; 3],
    pub set_command: [u8; 3],
    pub read_payload: Vec<u8>,
    /// Bytes used by the source getter, with actual firmware values.
    pub value: Vec<u8>,
    /// Includes source padding/unknown response bytes; never a local default.
    pub raw_response: Vec<u8>,
    /// Exact source setter data. DPI indices are normalized to zero-based form.
    pub restore_payload: Vec<u8>,
    pub restore_report_size: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct StoredProfile {
    pub profile_id: u8,
    /// Source reserves slot 1; its identity is never deleted or recreated.
    pub identity: Option<OnboardIdentityObservation>,
    pub settings: Vec<StoredSetting>,
    pub polling: Option<ProfilePollingObservation>,
    pub buttons: Option<ButtonInventoryReport>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorageIoStep {
    pub operation: String,
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    pub report_data_size: u8,
    pub mutation: bool,
    pub attempted: bool,
    pub reply_received: bool,
    pub response: Vec<u8>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StorageBackup {
    pub product_id: u32,
    pub macro_inventory: Option<MacroInventory>,
    pub profile_inventory: Option<OnboardInventory>,
    pub brightness_regions: Vec<u8>,
    pub macros: Vec<MacroReading>,
    pub profiles: Vec<StoredProfile>,
    pub io: Vec<StorageIoStep>,
    complete: bool,
}

impl StorageBackup {
    fn empty(product_id: u32) -> Self {
        Self {
            product_id,
            macro_inventory: None,
            profile_inventory: None,
            brightness_regions: Vec::new(),
            macros: Vec::new(),
            profiles: Vec::new(),
            io: Vec::new(),
            complete: false,
        }
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
}

#[derive(Debug)]
pub struct StorageBackupFailure {
    pub progress: StorageBackup,
    error: anyhow::Error,
}
impl std::fmt::Display for StorageBackupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse storage backup failed; partial observations retained: {:#}",
            self.error
        )
    }
}
impl std::error::Error for StorageBackupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum StorageRestoreEntry {
    Identity {
        profile_id: u8,
        report: OnboardWriteReport,
    },
    Button {
        profile_id: u8,
        report: ButtonWriteReport,
    },
    Macro {
        macro_id: u16,
        report: MacroWriteReport,
    },
    Polling {
        profile_id: u8,
        report: ProfilePollingWriteReport,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct StorageCompactionReport {
    pub backup: StorageBackup,
    pub io: Vec<StorageIoStep>,
    pub restoration: Vec<StorageRestoreEntry>,
    pub format_statuses: Vec<u8>,
    pub after: Option<StorageBackup>,
    verified: bool,
}

impl StorageCompactionReport {
    fn new(product_id: u32) -> Self {
        Self {
            backup: StorageBackup::empty(product_id),
            io: Vec::new(),
            restoration: Vec::new(),
            format_statuses: Vec::new(),
            after: None,
            verified: false,
        }
    }
    pub fn verified(&self) -> bool {
        self.verified
    }
    pub fn device_state_may_have_changed(&self) -> bool {
        self.io.iter().any(|step| step.mutation && step.attempted)
            || self.restoration.iter().any(|entry| match entry {
                StorageRestoreEntry::Identity { report, .. } => {
                    report.steps.iter().any(|step| step.attempted)
                }
                StorageRestoreEntry::Button { report, .. } => {
                    report.steps.iter().any(|step| step.attempted)
                }
                StorageRestoreEntry::Macro { report, .. } => {
                    report.steps.iter().any(|step| step.attempted)
                }
                StorageRestoreEntry::Polling { report, .. } => report.attempted,
            })
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum StorageFailurePhase {
    Prepare,
    Backup,
    DeleteProfiles,
    FormatStart,
    FormatWait,
    FormatPoll,
    VerifyEmpty,
    RestoreIdentity,
    RestoreSettings,
    RestoreButtons,
    RestorePolling,
    RestoreMacros,
    Readback,
    Cancelled,
}

#[derive(Debug)]
pub struct StorageCompactionFailure {
    pub phase: StorageFailurePhase,
    pub progress: StorageCompactionReport,
    pub profile_id: Option<u8>,
    pub macro_id: Option<u16>,
    error: anyhow::Error,
}
impl StorageCompactionFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.progress.device_state_may_have_changed()
    }
}
impl std::fmt::Display for StorageCompactionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse storage {:?} failed; backup and partial restoration retained: {:#}",
            self.phase, self.error
        )
    }
}
impl std::error::Error for StorageCompactionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MacroStorageWriteReport {
    /// Initial ordinary upload, including a read-only CompactionRequired result.
    pub initial: Option<MacroWriteReport>,
    pub compaction: Option<StorageCompactionReport>,
    pub upload: Option<MacroWriteReport>,
}
impl MacroStorageWriteReport {
    pub fn verified(&self) -> bool {
        self.upload.as_ref().is_some_and(MacroWriteReport::verified)
            && self
                .compaction
                .as_ref()
                .is_none_or(StorageCompactionReport::verified)
    }
    pub fn device_state_may_have_changed(&self) -> bool {
        [&self.initial, &self.upload].into_iter().any(|value| {
            value
                .as_ref()
                .is_some_and(|report| report.steps.iter().any(|step| step.attempted))
        }) || self
            .compaction
            .as_ref()
            .is_some_and(StorageCompactionReport::device_state_may_have_changed)
    }
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum MacroStorageFailurePhase {
    Upload,
    Compaction,
}
#[derive(Debug)]
pub struct MacroStorageWriteFailure {
    pub phase: MacroStorageFailurePhase,
    pub progress: MacroStorageWriteReport,
    pub macro_phase: Option<MacroFailurePhase>,
    pub storage_phase: Option<StorageFailurePhase>,
    error: anyhow::Error,
}
impl std::fmt::Display for MacroStorageWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse macro upload {:?} failed; storage progress retained: {:#}",
            self.phase, self.error
        )
    }
}
impl std::error::Error for MacroStorageWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub struct MouseStorage<'a> {
    device: &'a dyn FeatureTransport,
    transport: &'a DeviceReadCapability,
}

impl<'a> MouseStorage<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        // Reuse the strict collection/channel gate used by the actual mapping
        // startup selector; class names alone do not authorize an adapter.
        MouseButtonMapping::new(device, transport)?;
        OnboardMacro::for_mouse(device, transport)?;
        Ok(Self { device, transport })
    }

    /// Actual source add behavior: ordinary upload first; only its observed
    /// recyclable-space branch compacts and retries the requested new macro.
    /// ID collisions, insufficient total space and other errors never format.
    pub fn write_macro_with_compaction(
        &self,
        macro_id: u16,
        request: &MacroWriteRequest,
        bounds: StorageReadBounds,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        wait: &mut impl FnMut(Duration) -> Result<()>,
    ) -> std::result::Result<MacroStorageWriteReport, MacroStorageWriteFailure> {
        let mut report = MacroStorageWriteReport::default();
        let macros = OnboardMacro::for_mouse(self.device, self.transport)
            .expect("Constructor gated current macro transport");
        match macros.write_macro(macro_id, request, transactions, validate) {
            Ok(value) => {
                report.upload = Some(value);
                return Ok(report);
            }
            Err(error) => {
                let phase = error.phase;
                let message = format!("{error}");
                report.initial = Some(error.progress);
                if phase != MacroFailurePhase::CompactionRequired {
                    return Err(MacroStorageWriteFailure {
                        phase: MacroStorageFailurePhase::Upload,
                        progress: report,
                        macro_phase: Some(phase),
                        storage_phase: None,
                        error: anyhow::anyhow!(message),
                    });
                }
            }
        }
        match self.compact(bounds, transactions, validate, wait) {
            Ok(value) => report.compaction = Some(value),
            Err(error) => {
                let phase = error.phase;
                let message = format!("{error}");
                report.compaction = Some(error.progress);
                return Err(MacroStorageWriteFailure {
                    phase: MacroStorageFailurePhase::Compaction,
                    progress: report,
                    macro_phase: None,
                    storage_phase: Some(phase),
                    error: anyhow::anyhow!(message),
                });
            }
        }
        match macros.write_macro(macro_id, request, transactions, validate) {
            Ok(value) => {
                report.upload = Some(value);
                Ok(report)
            }
            Err(error) => {
                let phase = error.phase;
                let message = format!("{error}");
                report.upload = Some(error.progress);
                Err(MacroStorageWriteFailure {
                    phase: MacroStorageFailurePhase::Upload,
                    progress: report,
                    macro_phase: Some(phase),
                    storage_phase: None,
                    error: anyhow::anyhow!(message),
                })
            }
        }
    }

    fn io(
        &self,
        operation: &str,
        command: [u8; 3],
        payload: &[u8],
        size: u8,
        mutation: bool,
        steps: &mut Vec<StorageIoStep>,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        validate()?;
        let transaction = transactions()?;
        steps.push(StorageIoStep {
            operation: operation.into(),
            command,
            payload: payload.to_vec(),
            report_data_size: size,
            mutation,
            attempted: false,
            reply_received: false,
            response: Vec::new(),
            error: None,
        });
        let step = steps.last_mut().unwrap();
        let value = exchange_with_report_data_size_observing_send(
            self.device,
            self.transport,
            command,
            payload,
            transaction,
            size,
            validate,
            &mut || step.attempted = true,
        );
        match value {
            Ok(value) => {
                step.reply_received = true;
                step.response = value.clone();
                Ok(value)
            }
            Err(error) => {
                step.error = Some(format!("{error:#}"));
                Err(error)
            }
        }
    }

    fn capture_setting(
        &self,
        profile: u8,
        kind: StoredSettingKind,
        get: [u8; 3],
        read_payload: Vec<u8>,
        values: usize,
        steps: &mut Vec<StorageIoStep>,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<StoredSetting> {
        let raw = self.io(
            "backup setting",
            get,
            &read_payload,
            get[0],
            false,
            steps,
            transactions,
            validate,
        )?;
        ensure!(
            raw.len() >= values && raw.first() == Some(&profile),
            "Stored setting response has missing bytes or a wrong profile"
        );
        if let StoredSettingKind::Brightness { region_id } = kind {
            ensure!(
                raw[1] == region_id,
                "Stored brightness response has a wrong region"
            );
        }
        let mut value = raw[..values].to_vec();
        let mut payload = value.clone();
        let mut size = get[0];
        if kind == StoredSettingKind::DpiStages {
            let count = usize::from(raw[2]);
            ensure!(
                (1..=11).contains(&count)
                    && raw.len() >= 3 + 7 * count
                    && raw[1] > 0
                    && usize::from(raw[1]) <= count,
                "Stored DPI table is truncated or has inconsistent active/count fields"
            );
            value = raw[..3 + 7 * count].to_vec();
            payload = value.clone();
            size = payload.len() as u8;
            for index in 0..count {
                ensure!(
                    value[3 + 7 * index] == index as u8 + 1,
                    "Stored DPI table has unsupported nonsequential indices"
                );
                payload[3 + 7 * index] = index as u8;
            }
        }
        let set = [get[0], get[1], get[2] & 0x7f];
        Ok(StoredSetting {
            kind,
            get_command: get,
            set_command: set,
            read_payload,
            value,
            raw_response: raw,
            restore_payload: payload,
            restore_report_size: size,
        })
    }

    fn capture_profile(
        &self,
        profile: u8,
        progress: &mut StorageBackup,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> Result<()> {
        progress.profiles.push(StoredProfile {
            profile_id: profile,
            identity: None,
            settings: Vec::new(),
            polling: None,
            buttons: None,
        });
        let index = progress.profiles.len() - 1;
        if profile != 1 {
            progress.profiles[index].identity = Some(
                MouseOnboard::new(self.device, self.transport)?.read_identity(
                    profile,
                    transactions,
                    validate,
                )?,
            );
        }
        let mut reads = vec![
            (StoredSettingKind::DpiLevel, [7, 4, 133], vec![profile], 7),
            (StoredSettingKind::DpiStages, [80, 4, 134], vec![profile], 3),
        ];
        // The source defrag omits these values. Their current product-specific
        // command gates and layouts are verified in the brightness/scroll audits.
        for &region_id in &progress.brightness_regions {
            reads.push((
                StoredSettingKind::Brightness { region_id },
                [3, 15, 132],
                vec![profile, region_id],
                3,
            ));
        }
        if self.transport.product_id == 226 {
            reads.push((
                StoredSettingKind::ScrollEnabledModes,
                [2, 21, 139],
                vec![profile],
                2,
            ));
            reads.push((StoredSettingKind::ScrollMode, [2, 2, 176], vec![profile], 2));
        } else {
            reads.push((StoredSettingKind::ScrollMode, [2, 2, 148], vec![profile], 2));
        }
        reads.push((
            StoredSettingKind::ScrollAcceleration,
            [2, 2, 150],
            vec![profile],
            2,
        ));
        reads.push((StoredSettingKind::SmartReel, [80, 2, 151], vec![profile], 2));
        if self.transport.product_id != 170 {
            reads.push((
                StoredSettingKind::ScrollAccelerationLevel,
                [2, 2, 162],
                vec![profile],
                2,
            ));
            reads.push((
                StoredSettingKind::SmartReelThreshold,
                [2, 2, 163],
                vec![profile],
                2,
            ));
        }
        for (kind, command, payload, count) in reads {
            let setting = self.capture_setting(
                profile,
                kind,
                command,
                payload,
                count,
                &mut progress.io,
                transactions,
                validate,
            )?;
            progress.profiles[index].settings.push(setting);
        }
        progress.profiles[index].polling =
            Some(MouseProfilePolling::new(self.device, self.transport)?.read(
                profile,
                &mut *transactions,
                validate,
            )?);
        let buttons = MouseButtonInventory::new(self.device, self.transport)?.read_profile(
            profile,
            &mut *transactions,
            validate,
        );
        match buttons {
            Ok(value) => {
                let complete = value.complete();
                let restorable = value.entries.iter().all(|entry| {
                    entry
                        .observed
                        .as_ref()
                        .is_some_and(|observed| observed.assignment.data_size <= 5)
                });
                progress.profiles[index].buttons = Some(value);
                ensure!(
                    complete && restorable,
                    "A complete button backup is required before formatting"
                );
            }
            Err(error) => {
                let message = format!("{error}");
                progress.profiles[index].buttons = Some(error.progress);
                return Err(anyhow::anyhow!(message));
            }
        }
        Ok(())
    }

    /// Bounded reads only. A missing field or incomplete button scan prevents
    /// compaction; partial observations remain available to the caller.
    pub fn backup(
        &self,
        bounds: StorageReadBounds,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<StorageBackup, StorageBackupFailure> {
        let mut progress = StorageBackup::empty(self.transport.product_id);
        let captured = (|| -> Result<()> {
            validate()?;
            let macros = OnboardMacro::for_mouse(self.device, self.transport)?;
            let inventory = macros.read_macro_inventory(transactions, validate)?;
            let ids = inventory.ids.clone();
            progress.macro_inventory = Some(inventory);
            let mut total = 0usize;
            for id in ids {
                ensure!(
                    id <= 32767,
                    "Observed macro ID is outside the source restore allocator range"
                );
                let remaining = bounds.max_total_macro_bytes.saturating_sub(total);
                let maximum = bounds
                    .max_macro_bytes
                    .min(u32::try_from(remaining).unwrap_or(u32::MAX))
                    .min(
                        progress
                            .macro_inventory
                            .as_ref()
                            .unwrap()
                            .storage
                            .max_memory
                            .saturating_sub(256),
                    );
                let macro_data = macros.read_macro(id, maximum, transactions, validate)?;
                total = total
                    .checked_add(macro_data.data.len())
                    .context("Stored macro byte count overflow")?;
                ensure!(
                    total <= bounds.max_total_macro_bytes,
                    "Stored macro data exceeds the total backup bound"
                );
                progress.macros.push(macro_data);
            }
            let required = progress.macros.iter().try_fold(0u32, |total, value| {
                let rounded = value
                    .data_size
                    .checked_add(256)
                    .and_then(|value| value.checked_add(255))
                    .map(|value| value / 256 * 256)
                    .context("Macro restoration memory bound overflow")?;
                total
                    .checked_add(rounded)
                    .context("Total macro restoration memory bound overflow")
            })?;
            ensure!(
                required
                    <= progress
                        .macro_inventory
                        .as_ref()
                        .unwrap()
                        .storage
                        .max_memory,
                "Observed macro inventory cannot be restored inside the reported memory capacity"
            );
            let inventory = MouseOnboard::new(self.device, self.transport)?
                .read_inventory(transactions, validate)?;
            // Source deletion skips the first list entry while restoration skips
            // ID 1. Reject disagreement before any destructive exchange.
            ensure!(
                inventory.profile_ids.first() == Some(&1),
                "Storage formatting requires reserved profile 1 as the first observed slot"
            );
            let profiles = inventory.profile_ids.clone();
            progress.profile_inventory = Some(inventory);
            let regions = self.io(
                "backup brightness regions",
                [80, 15, 128],
                &[],
                80,
                false,
                &mut progress.io,
                transactions,
                validate,
            )?;
            ensure!(
                regions.len().is_multiple_of(5),
                "Brightness region table contains a truncated row"
            );
            for row in regions.chunks_exact(5) {
                if row.iter().all(|value| *value == 0) {
                    continue;
                }
                if !progress.brightness_regions.contains(&row[0]) {
                    progress.brightness_regions.push(row[0]);
                }
            }
            if progress.brightness_regions.is_empty() {
                progress.brightness_regions.push(0);
            }
            for profile in profiles {
                self.capture_profile(profile, &mut progress, transactions, validate)?;
            }
            let macros_after = macros.read_macro_inventory(transactions, validate)?;
            let profiles_after = MouseOnboard::new(self.device, self.transport)?
                .read_inventory(transactions, validate)?;
            let before = progress.macro_inventory.as_ref().unwrap();
            ensure!(
                same_ids(&before.ids, &macros_after.ids)
                    && before.storage.max_memory == macros_after.storage.max_memory,
                "Macro inventory changed while taking its backup"
            );
            ensure!(
                same_ids(
                    &progress.profile_inventory.as_ref().unwrap().profile_ids,
                    &profiles_after.profile_ids
                ),
                "Profile inventory changed while taking its backup"
            );
            progress.complete = true;
            Ok(())
        })();
        match captured {
            Ok(()) => Ok(progress),
            Err(error) => Err(StorageBackupFailure { progress, error }),
        }
    }

    fn fail(
        phase: StorageFailurePhase,
        progress: StorageCompactionReport,
        profile_id: Option<u8>,
        macro_id: Option<u16>,
        error: anyhow::Error,
        validate: &impl Fn() -> Result<()>,
    ) -> StorageCompactionFailure {
        StorageCompactionFailure {
            phase: if validate().is_err() {
                StorageFailurePhase::Cancelled
            } else {
                phase
            },
            progress,
            profile_id,
            macro_id,
            error,
        }
    }

    /// Source delete→format action→100 ms wait→at most three status polls,
    /// then full observed-byte restoration and an independent final backup.
    /// Callers can inject a virtual clock for Rust-only transport simulations.
    pub fn compact(
        &self,
        bounds: StorageReadBounds,
        transactions: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
        wait: &mut impl FnMut(Duration) -> Result<()>,
    ) -> std::result::Result<StorageCompactionReport, StorageCompactionFailure> {
        let mut report = StorageCompactionReport::new(self.transport.product_id);
        report.backup = match self.backup(bounds, transactions, validate) {
            Ok(value) => value,
            Err(error) => {
                report.backup = error.progress;
                return Err(Self::fail(
                    StorageFailurePhase::Backup,
                    report,
                    None,
                    None,
                    error.error,
                    validate,
                ));
            }
        };
        let profile_ids = report
            .backup
            .profile_inventory
            .as_ref()
            .unwrap()
            .profile_ids
            .clone();
        for &profile in profile_ids.iter().skip(1) {
            if let Err(error) = self.io(
                "delete backed-up profile",
                [1, 5, 3],
                &[profile],
                1,
                true,
                &mut report.io,
                transactions,
                validate,
            ) {
                return Err(Self::fail(
                    StorageFailurePhase::DeleteProfiles,
                    report,
                    Some(profile),
                    None,
                    error,
                    validate,
                ));
            }
        }
        if let Err(error) = self.io(
            "start storage format",
            SET_FORMAT,
            &FORMAT_SELECTOR,
            6,
            true,
            &mut report.io,
            transactions,
            validate,
        ) {
            return Err(Self::fail(
                StorageFailurePhase::FormatStart,
                report,
                None,
                None,
                error,
                validate,
            ));
        }
        let waited = (|| {
            validate()?;
            wait(Duration::from_millis(100))?;
            validate()
        })();
        if let Err(error) = waited {
            return Err(Self::fail(
                StorageFailurePhase::FormatWait,
                report,
                None,
                None,
                error,
                validate,
            ));
        }
        let mut formatted = false;
        for attempt in 0..3 {
            let polled = self.io(
                "poll storage format",
                GET_FORMAT,
                &FORMAT_SELECTOR,
                6,
                false,
                &mut report.io,
                transactions,
                validate,
            );
            match polled {
                Ok(data) => {
                    if data.len() < 4 || data[0..2] != [0, 0] || data[3] != 2 {
                        return Err(Self::fail(
                            StorageFailurePhase::FormatPoll,
                            report,
                            None,
                            None,
                            anyhow::anyhow!(
                                "Format status has missing bytes or a wrong macro/action selector"
                            ),
                            validate,
                        ));
                    }
                    report.format_statuses.push(data[2]);
                    if data[2] == 2 {
                        formatted = true;
                        break;
                    }
                }
                Err(error) => {
                    if attempt == 2 || validate().is_err() {
                        return Err(Self::fail(
                            StorageFailurePhase::FormatPoll,
                            report,
                            None,
                            None,
                            error,
                            validate,
                        ));
                    }
                }
            }
        }
        if !formatted {
            return Err(Self::fail(
                StorageFailurePhase::FormatPoll,
                report,
                None,
                None,
                anyhow::anyhow!(
                    "Format action did not reach source completion status 2 within three polls"
                ),
                validate,
            ));
        }
        let empty = (|| -> Result<()> {
            let macros = OnboardMacro::for_mouse(self.device, self.transport)?
                .read_macro_inventory(transactions, validate)?;
            ensure!(
                macros.ids.is_empty(),
                "Independent macro inventory is not empty after format completion"
            );
            let profiles = MouseOnboard::new(self.device, self.transport)?
                .read_inventory(transactions, validate)?;
            ensure!(
                profiles.profile_ids == [1],
                "Independent profile inventory differs after format completion"
            );
            Ok(())
        })();
        if let Err(error) = empty {
            return Err(Self::fail(
                StorageFailurePhase::VerifyEmpty,
                report,
                None,
                None,
                error,
                validate,
            ));
        }
        for index in 0..report.backup.profiles.len() {
            let profile = report.backup.profiles[index].clone();
            if let Some(identity) = &profile.identity {
                let result = MouseOnboard::new(self.device, self.transport)
                    .unwrap()
                    .restore_identity(identity, transactions, validate);
                match result {
                    Ok(value) => report.restoration.push(StorageRestoreEntry::Identity {
                        profile_id: profile.profile_id,
                        report: value,
                    }),
                    Err(error) => {
                        let message = format!("{error}");
                        report.restoration.push(StorageRestoreEntry::Identity {
                            profile_id: profile.profile_id,
                            report: error.progress,
                        });
                        return Err(Self::fail(
                            StorageFailurePhase::RestoreIdentity,
                            report,
                            Some(profile.profile_id),
                            None,
                            anyhow::anyhow!(message),
                            validate,
                        ));
                    }
                }
            }
            for setting in &profile.settings {
                let restored = (|| -> Result<()> {
                    self.io(
                        "restore backed-up setting",
                        setting.set_command,
                        &setting.restore_payload,
                        setting.restore_report_size,
                        true,
                        &mut report.io,
                        transactions,
                        validate,
                    )?;
                    let read = self.io(
                        "confirm restored setting",
                        setting.get_command,
                        &setting.read_payload,
                        setting.get_command[0],
                        false,
                        &mut report.io,
                        transactions,
                        validate,
                    )?;
                    ensure!(
                        read.starts_with(&setting.value),
                        "Independent setting readback differs after restoration"
                    );
                    Ok(())
                })();
                if let Err(error) = restored {
                    return Err(Self::fail(
                        StorageFailurePhase::RestoreSettings,
                        report,
                        Some(profile.profile_id),
                        None,
                        error,
                        validate,
                    ));
                }
            }
            let polling = MouseProfilePolling::new(self.device, self.transport)
                .unwrap()
                .restore(
                    profile.polling.as_ref().unwrap(),
                    true,
                    &mut *transactions,
                    validate,
                );
            match polling {
                Ok(value) => report.restoration.push(StorageRestoreEntry::Polling {
                    profile_id: profile.profile_id,
                    report: value,
                }),
                Err(error) => {
                    let message = format!("{error}");
                    report.restoration.push(StorageRestoreEntry::Polling {
                        profile_id: profile.profile_id,
                        report: error.progress,
                    });
                    return Err(Self::fail(
                        StorageFailurePhase::RestorePolling,
                        report,
                        Some(profile.profile_id),
                        None,
                        anyhow::anyhow!(message),
                        validate,
                    ));
                }
            }
            // The saved scan uses source NormalMode-all then HypershiftMode-all.
            for entry in &profile.buttons.as_ref().unwrap().entries {
                let result = MouseButtonMapping::new(self.device, self.transport)
                    .unwrap()
                    .restore_assignment(
                        entry.observed.as_ref().unwrap(),
                        true,
                        &mut *transactions,
                        validate,
                    );
                match result {
                    Ok(value) => report.restoration.push(StorageRestoreEntry::Button {
                        profile_id: profile.profile_id,
                        report: value,
                    }),
                    Err(error) => {
                        let text = format!("{error}");
                        report.restoration.push(StorageRestoreEntry::Button {
                            profile_id: profile.profile_id,
                            report: error.progress,
                        });
                        return Err(Self::fail(
                            StorageFailurePhase::RestoreButtons,
                            report,
                            Some(profile.profile_id),
                            None,
                            anyhow::anyhow!(text),
                            validate,
                        ));
                    }
                }
            }
        }
        for index in 0..report.backup.macros.len() {
            let saved = &report.backup.macros[index];
            let macro_id = saved.metadata.macro_id;
            let result = OnboardMacro::for_mouse(self.device, self.transport)
                .unwrap()
                .restore_macro(saved, transactions, validate);
            match result {
                Ok(value) => report.restoration.push(StorageRestoreEntry::Macro {
                    macro_id,
                    report: value,
                }),
                Err(error) => {
                    let text = format!("{error}");
                    report.restoration.push(StorageRestoreEntry::Macro {
                        macro_id,
                        report: error.progress,
                    });
                    return Err(Self::fail(
                        StorageFailurePhase::RestoreMacros,
                        report,
                        None,
                        Some(macro_id),
                        anyhow::anyhow!(text),
                        validate,
                    ));
                }
            }
        }
        match self.backup(bounds, transactions, validate) {
            Ok(value) => {
                let same = backups_match(&report.backup, &value);
                report.after = Some(value);
                if let Err(error) = same {
                    return Err(Self::fail(
                        StorageFailurePhase::Readback,
                        report,
                        None,
                        None,
                        error,
                        validate,
                    ));
                }
            }
            Err(error) => {
                report.after = Some(error.progress);
                return Err(Self::fail(
                    StorageFailurePhase::Readback,
                    report,
                    None,
                    None,
                    error.error,
                    validate,
                ));
            }
        }
        report.verified = true;
        Ok(report)
    }
}

fn same_ids<T: Ord + Copy>(left: &[T], right: &[T]) -> bool {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

fn backups_match(before: &StorageBackup, after: &StorageBackup) -> Result<()> {
    ensure!(
        before.complete && after.complete && before.product_id == after.product_id,
        "Final storage backup is incomplete or has a wrong product"
    );
    ensure!(
        same_ids(
            &before.macro_inventory.as_ref().unwrap().ids,
            &after.macro_inventory.as_ref().unwrap().ids
        ) && same_ids(
            &before.profile_inventory.as_ref().unwrap().profile_ids,
            &after.profile_inventory.as_ref().unwrap().profile_ids
        ),
        "Final profile or macro inventory differs from the complete backup"
    );
    ensure!(
        same_ids(&before.brightness_regions, &after.brightness_regions),
        "Brightness region inventory changed during restoration"
    );
    for saved in &before.macros {
        let current = after
            .macros
            .iter()
            .find(|value| value.metadata.macro_id == saved.metadata.macro_id)
            .context("Restored macro is missing")?;
        ensure!(
            current.metadata.raw == saved.metadata.raw
                && current.data_size == saved.data_size
                && current.data == saved.data,
            "Final macro byte readback differs from its complete backup"
        );
    }
    for saved in &before.profiles {
        let current = after
            .profiles
            .iter()
            .find(|value| value.profile_id == saved.profile_id)
            .context("Restored profile is missing")?;
        ensure!(
            current.identity.as_ref().map(|value| &value.raw_metadata)
                == saved.identity.as_ref().map(|value| &value.raw_metadata),
            "Final profile identity differs from its complete backup"
        );
        ensure!(
            current.settings.len() == saved.settings.len()
                && current
                    .settings
                    .iter()
                    .zip(&saved.settings)
                    .all(|(got, want)| got.kind == want.kind && got.value == want.value),
            "Final profile setting readback differs"
        );
        ensure!(
            current
                .polling
                .as_ref()
                .map(|value| (value.profile_id, value.raw_code))
                == saved
                    .polling
                    .as_ref()
                    .map(|value| (value.profile_id, value.raw_code)),
            "Final profile polling readback differs"
        );
        let got = current.buttons.as_ref().unwrap();
        let want = saved.buttons.as_ref().unwrap();
        ensure!(
            got.complete()
                && got.entries.len() == want.entries.len()
                && got
                    .entries
                    .iter()
                    .zip(&want.entries)
                    .all(|(got, want)| got.target == want.target
                        && got.observed.as_ref().map(|value| &value.assignment)
                            == want.observed.as_ref().map(|value| &value.assignment)),
            "Final button assignment readback differs"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "mouse_storage_tests.rs"]
mod tests;
