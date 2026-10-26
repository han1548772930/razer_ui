//! Current source-gated rzDevice25 mouse scrolling commands.
//! Caller retains the real collection and supplies observed profile associations.
//! V2 writes preserve the source mask-before-mode order. Source 45066 mode tasks
//! start in parallel; this retained Feature session serializes hardware exchanges.
//! Independent getter readback is an additional Rust confirmation policy.
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange::exchange,
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct MouseScrollCapability {
    product_id: u32,
    levels: bool,
    mode_v2: bool,
}

const PRODUCTS: &[MouseScrollCapability] = &[
    MouseScrollCapability {
        product_id: 153,
        levels: false,
        mode_v2: false,
    },
    MouseScrollCapability {
        product_id: 170,
        levels: false,
        mode_v2: false,
    },
    MouseScrollCapability {
        product_id: 203,
        levels: true,
        mode_v2: false,
    },
    MouseScrollCapability {
        product_id: 204,
        levels: true,
        mode_v2: false,
    },
    MouseScrollCapability {
        product_id: 214,
        levels: true,
        mode_v2: false,
    },
    MouseScrollCapability {
        product_id: 226,
        levels: true,
        mode_v2: true,
    },
    MouseScrollCapability {
        product_id: 45066,
        levels: true,
        mode_v2: false,
    },
];

pub fn capability(product_id: u32) -> Option<&'static MouseScrollCapability> {
    PRODUCTS.iter().find(|cap| cap.product_id == product_id)
}

impl MouseScrollCapability {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn supports_levels(&self) -> bool {
        self.levels
    }
    pub fn supports_mode_v2(&self) -> bool {
        self.mode_v2
    }

    pub fn read_order(&self) -> &'static [ScrollReadKind] {
        use ScrollReadKind::*;
        match (self.mode_v2, self.levels) {
            (true, _) => &[
                EnabledModes,
                ThreeMode,
                SmartReel,
                SmartReelThreshold,
                VirtualAcceleration,
                VirtualAccelerationLevel,
            ],
            (false, true) => &[
                LegacyMode,
                SmartReel,
                SmartReelThreshold,
                VirtualAcceleration,
                VirtualAccelerationLevel,
            ],
            (false, false) => &[LegacyMode, SmartReel, VirtualAcceleration],
        }
    }

    fn command(&self, kind: ScrollReadKind, write: bool) -> Result<[u8; 3]> {
        let (bytes, class, id) = match kind {
            ScrollReadKind::LegacyMode => {
                ensure!(
                    !self.mode_v2,
                    "This product uses the complete three-mode configuration chain"
                );
                (2, 2, 0x14)
            }
            ScrollReadKind::ThreeMode => {
                ensure!(
                    self.mode_v2,
                    "Three-mode scrolling is not source-gated for this product"
                );
                (2, 2, 0x30)
            }
            ScrollReadKind::EnabledModes => {
                ensure!(
                    self.mode_v2,
                    "Scroll enabled-mode information is not source-gated for this product"
                );
                (2, 21, 0x0B)
            }
            ScrollReadKind::VirtualAcceleration => (2, 2, 0x16),
            // The original command allocates all 80 payload bytes, not two.
            ScrollReadKind::SmartReel => (80, 2, 0x17),
            ScrollReadKind::VirtualAccelerationLevel => {
                ensure!(
                    self.levels,
                    "Acceleration level is not source-gated for this product"
                );
                (2, 2, 0x22)
            }
            ScrollReadKind::SmartReelThreshold => {
                ensure!(
                    self.levels,
                    "Smart Reel threshold is not source-gated for this product"
                );
                (2, 2, 0x23)
            }
        };
        Ok([bytes, class, id | if write { 0 } else { 0x80 }])
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrollMode {
    Tactile,
    FreeSpin,
    MicroTactile,
}

impl ScrollMode {
    fn raw(self) -> u8 {
        match self {
            Self::Tactile => 0,
            Self::FreeSpin => 1,
            Self::MicroTactile => 2,
        }
    }
    fn decode(raw: u8, three_mode: bool) -> Result<Self> {
        match raw {
            0 => Ok(Self::Tactile),
            1 => Ok(Self::FreeSpin),
            2 if three_mode => Ok(Self::MicroTactile),
            _ => anyhow::bail!("Unknown or unsupported scroll mode {raw}"),
        }
    }
}

/// Source enabled-mode bitmask: Tactile=1, FreeSpin=2, MicroTactile=4.
/// At least one mode must remain enabled; unsupported bits cannot be stored.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnabledScrollModes(u8);

impl EnabledScrollModes {
    pub fn new(mask: u8) -> Result<Self> {
        ensure!(
            (1..=7).contains(&mask),
            "Scroll enabled-mode mask must be 1..7"
        );
        Ok(Self(mask))
    }
    pub fn from_disabled(disabled: &[ScrollMode]) -> Result<Self> {
        let mut mask = 7;
        for mode in disabled {
            mask &= !(1 << mode.raw());
        }
        Self::new(mask)
    }
    pub fn mask(self) -> u8 {
        self.0
    }
    pub fn contains(self, mode: ScrollMode) -> bool {
        self.0 & (1 << mode.raw()) != 0
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrollReadKind {
    LegacyMode,
    ThreeMode,
    EnabledModes,
    VirtualAcceleration,
    SmartReel,
    VirtualAccelerationLevel,
    SmartReelThreshold,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrollValue {
    LegacyMode(ScrollMode),
    ThreeMode(ScrollMode),
    EnabledModes(EnabledScrollModes),
    VirtualAcceleration(bool),
    SmartReel(bool),
    VirtualAccelerationLevel(u8),
    SmartReelThreshold(u8),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrollReadOutcome {
    Observed(ScrollValue),
    Unknown { raw: u8 },
    Failed { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScrollReading {
    pub kind: ScrollReadKind,
    pub outcome: ScrollReadOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScrollSnapshot {
    pub readings: Vec<ScrollReading>,
}

impl ScrollSnapshot {
    pub fn validate_for(&self, product_id: u32) -> Result<()> {
        let cap = capability(product_id).context("Scroll observation product is absent")?;
        ensure!(
            self.readings.len() == cap.read_order().len(),
            "Scroll observation count differs"
        );
        for (reading, kind) in self.readings.iter().zip(cap.read_order()) {
            ensure!(reading.kind == *kind, "Scroll observation order differs");
            match &reading.outcome {
                ScrollReadOutcome::Observed(value) => {
                    ensure!(value.kind() == *kind, "Scroll observation kind differs");
                    ensure!(
                        parse(&[1, value.raw()], 1, *kind)? == *value,
                        "Scroll observation value differs"
                    );
                }
                ScrollReadOutcome::Unknown { raw } => {
                    ensure!(
                        parse(&[1, *raw], 1, *kind).is_err(),
                        "A known scroll value was classified as unknown"
                    );
                }
                ScrollReadOutcome::Failed { message } => {
                    ensure!(!message.is_empty(), "Scroll read failure has no cause");
                }
            }
        }
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        !self.readings.is_empty()
            && self
                .readings
                .iter()
                .all(|reading| matches!(reading.outcome, ScrollReadOutcome::Observed(_)))
    }
}

impl ScrollValue {
    fn kind(self) -> ScrollReadKind {
        match self {
            Self::LegacyMode(_) => ScrollReadKind::LegacyMode,
            Self::ThreeMode(_) => ScrollReadKind::ThreeMode,
            Self::EnabledModes(_) => ScrollReadKind::EnabledModes,
            Self::VirtualAcceleration(_) => ScrollReadKind::VirtualAcceleration,
            Self::SmartReel(_) => ScrollReadKind::SmartReel,
            Self::VirtualAccelerationLevel(_) => ScrollReadKind::VirtualAccelerationLevel,
            Self::SmartReelThreshold(_) => ScrollReadKind::SmartReelThreshold,
        }
    }
    fn raw(self) -> u8 {
        match self {
            Self::LegacyMode(mode) | Self::ThreeMode(mode) => mode.raw(),
            Self::EnabledModes(modes) => modes.mask(),
            Self::VirtualAcceleration(enabled) | Self::SmartReel(enabled) => u8::from(enabled),
            Self::VirtualAccelerationLevel(level) | Self::SmartReelThreshold(level) => level,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrollChange {
    LegacyMode(ScrollMode),
    /// Source V2 writes enabled-mode information before the selected mode.
    ModeV2 {
        selected: ScrollMode,
        enabled: EnabledScrollModes,
    },
    VirtualAcceleration(bool),
    SmartReel(bool),
    /// Exact source byte values; no unproved UI slider range is invented.
    VirtualAccelerationLevel(u8),
    SmartReelThreshold(u8),
}

impl ScrollChange {
    pub fn values(self) -> Result<Vec<ScrollValue>> {
        Ok(match self {
            Self::LegacyMode(mode) => {
                ensure!(
                    mode != ScrollMode::MicroTactile,
                    "Legacy scrolling has no MicroTactile mode"
                );
                vec![ScrollValue::LegacyMode(mode)]
            }
            Self::ModeV2 { selected, enabled } => {
                ensure!(
                    enabled.contains(selected),
                    "Selected scroll mode must remain enabled"
                );
                vec![
                    ScrollValue::EnabledModes(enabled),
                    ScrollValue::ThreeMode(selected),
                ]
            }
            Self::VirtualAcceleration(v) => vec![ScrollValue::VirtualAcceleration(v)],
            Self::SmartReel(v) => vec![ScrollValue::SmartReel(v)],
            Self::VirtualAccelerationLevel(v) => vec![ScrollValue::VirtualAccelerationLevel(v)],
            Self::SmartReelThreshold(v) => vec![ScrollValue::SmartReelThreshold(v)],
        })
    }
}

/// Source data observed by the caller, distinct from local draft intent.
/// `selected_obm_profile_id` is the first slot ID from the selected profile.
/// `matching_obm_profile_ids` comes from actual OBM profiles with the same GUID.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScrollProfileObservation {
    /// Source first selected-profile slot. Zero/None means no associated slot.
    pub selected_obm_profile_id: Option<u8>,
    /// Source `obmData.profiles.find(GUID).slotId`, used by 45066 legacy mode.
    pub first_matching_obm_slot_id: Option<u8>,
    /// Source `obmEngine.obmJson.profiles` GUID matches, used by Smart Reel.
    pub matching_obm_profile_ids: Vec<u8>,
}

/// One source profile update. IDs retain their source meaning: the selected
/// profile's first `obmSlotId` and independently observed GUID associations.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Submission {
    pub changes: Vec<ScrollChange>,
    pub profiles: ScrollProfileObservation,
    /// When present, GUID associations are read through the retained device
    /// session. A profile's draft slot list cannot stand in for that inventory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile_guid: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    observe_current: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl Submission {
    pub fn new(changes: Vec<ScrollChange>, profiles: ScrollProfileObservation) -> Self {
        Self {
            changes,
            profiles,
            profile_guid: None,
            observe_current: false,
        }
    }

    /// Observe the active hardware profile without saving or restoring local settings.
    pub fn read_current() -> Self {
        Self {
            changes: Vec::new(),
            profiles: ScrollProfileObservation::default(),
            profile_guid: None,
            observe_current: true,
        }
    }

    pub fn is_read(&self) -> bool {
        self.observe_current && self.changes.is_empty()
    }

    pub fn with_current_observation(mut self) -> Self {
        self.observe_current = true;
        self
    }

    pub fn for_profile(
        changes: Vec<ScrollChange>,
        guid: String,
        selected_obm_profile_id: Option<u8>,
    ) -> Self {
        Self {
            changes,
            profiles: ScrollProfileObservation {
                selected_obm_profile_id,
                ..Default::default()
            },
            profile_guid: Some(guid),
            observe_current: false,
        }
    }

    fn needs_observation(&self, product_id: u32, change: ScrollChange) -> bool {
        self.profile_guid.is_some()
            && (matches!(change, ScrollChange::SmartReel(_))
                || product_id == 45066 && matches!(change, ScrollChange::LegacyMode(_)))
    }

    fn resolve_profiles(
        &self,
        product_id: u32,
        onboard: Option<&[crate::mouse_onboard::OnboardProfileAssociation]>,
    ) -> Result<ScrollProfileObservation> {
        let Some(guid) = &self.profile_guid else {
            ensure!(onboard.is_none(), "Unrequested scroll profile inventory");
            return Ok(self.profiles.clone());
        };
        if !self
            .changes
            .iter()
            .any(|change| self.needs_observation(product_id, *change))
        {
            ensure!(onboard.is_none(), "Unrequested scroll profile inventory");
            return Ok(self.profiles.clone());
        }
        let onboard = onboard.context("Scroll GUID associations have not been observed")?;
        for (index, profile) in onboard.iter().enumerate() {
            ensure!(
                profile.profile_id > 0
                    && !onboard[..index]
                        .iter()
                        .any(|previous| previous.profile_id == profile.profile_id),
                "Observed scroll profile inventory has a zero or duplicate ID"
            );
        }
        let matching_obm_profile_ids: Vec<_> = onboard
            .iter()
            .filter(|profile| profile.guid == *guid)
            .map(|profile| profile.profile_id)
            .collect();
        Ok(ScrollProfileObservation {
            selected_obm_profile_id: self.profiles.selected_obm_profile_id,
            first_matching_obm_slot_id: matching_obm_profile_ids.first().copied(),
            matching_obm_profile_ids,
        })
    }

    pub fn validate(&self, product_id: u32) -> Result<()> {
        let cap = capability(product_id).context("Current mouse scroll capability is absent")?;
        ensure!(
            (!self.changes.is_empty() || self.observe_current) && self.changes.len() <= 5,
            "Scroll task list is empty or oversized"
        );
        if self.is_read() {
            ensure!(
                self.profile_guid.is_none() && self.profiles == ScrollProfileObservation::default(),
                "Current scroll observation cannot carry a local profile association"
            );
        }
        if let Some(guid) = &self.profile_guid {
            ensure!(!guid.is_empty(), "Scroll active profile GUID is empty");
            ensure!(
                self.profiles.first_matching_obm_slot_id.is_none()
                    && self.profiles.matching_obm_profile_ids.is_empty(),
                "Scroll GUID associations must come from the device session"
            );
        }
        for change in &self.changes {
            targets(cap, *change, &self.profiles)?;
            for value in change.values()? {
                cap.command(value.kind(), true)?;
                if let ScrollValue::EnabledModes(modes) = value {
                    EnabledScrollModes::new(modes.mask())?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SubmissionReport {
    pub requested: Submission,
    /// Actual inventory order and decoded GUIDs, separate from source intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboard_profiles: Option<Vec<crate::mouse_onboard::OnboardProfileAssociation>>,
    pub reports: Vec<ScrollWriteReport>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<ScrollSnapshot>,
}

impl SubmissionReport {
    /// Validate task association and typed partial progress independently from
    /// whether every requested write was acknowledged and read back.
    pub fn validate_for(&self, product_id: u32) -> Result<()> {
        self.requested.validate(product_id)?;
        if let Some(current) = &self.current {
            ensure!(
                self.requested.observe_current,
                "Unrequested current scroll observation"
            );
            current.validate_for(product_id)?;
        }
        ensure!(
            !self.requested.is_read() || self.current.is_some(),
            "Current scroll observation is absent"
        );
        ensure!(
            self.reports.len() == self.requested.changes.len(),
            "Scroll report task count differs from its submission"
        );
        let cap = capability(product_id).context("Scroll report product capability is absent")?;
        let resolved = self
            .requested
            .resolve_profiles(product_id, self.onboard_profiles.as_deref());
        if self.onboard_profiles.is_some() {
            resolved
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        }
        for (change, report) in self.requested.changes.iter().zip(&self.reports) {
            let profiles = match &resolved {
                Ok(profiles) => profiles,
                Err(_) if self.requested.needs_observation(product_id, *change) => {
                    ensure!(
                        report.steps.is_empty(),
                        "Scroll task has no observed GUID associations"
                    );
                    continue;
                }
                Err(_) => &self.requested.profiles,
            };
            let values = change.values()?;
            let expected: Vec<_> = targets(cap, *change, profiles)?
                .into_iter()
                .flat_map(|profile| values.iter().map(move |value| (profile, *value)))
                .collect();
            ensure!(
                report.steps.len() <= expected.len(),
                "Scroll report contains extra mutations"
            );
            for (step, (profile, requested)) in report.steps.iter().zip(expected) {
                ensure!(
                    step.profile_id == profile && step.requested == requested,
                    "Scroll report mutation identity or order differs"
                );
                ensure!(
                    step.acknowledgment_received
                        || step.acknowledged.is_none() && step.observed.is_none(),
                    "Scroll report contains values without a setter acknowledgement"
                );
                for value in step.acknowledged.iter().chain(step.observed.iter()) {
                    ensure!(
                        value.kind() == requested.kind(),
                        "Scroll report value kind differs"
                    );
                    cap.command(value.kind(), false)?;
                    match value {
                        ScrollValue::LegacyMode(ScrollMode::MicroTactile) => {
                            anyhow::bail!("Legacy scroll observation contains MicroTactile")
                        }
                        ScrollValue::EnabledModes(modes) => {
                            EnabledScrollModes::new(modes.mask())?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Confirm every value, hardware profile and source mutation order. A valid
    /// report for a different source task cannot confirm this submission.
    pub fn verified_for(&self, product_id: u32) -> bool {
        if self.requested.is_read()
            || self.validate_for(product_id).is_err()
            || !self.errors.is_empty()
            || self.requested.observe_current
                && !self
                    .current
                    .as_ref()
                    .is_some_and(ScrollSnapshot::is_complete)
        {
            return false;
        }
        if self.requested.observe_current
            && !self.current.as_ref().is_some_and(|current| {
                self.requested.changes.iter().all(|change| {
                    change.values().is_ok_and(|values| {
                        values.into_iter().all(|value| {
                            current.readings.iter().any(|reading| {
                                reading.kind == value.kind()
                                    && reading.outcome == ScrollReadOutcome::Observed(value)
                            })
                        })
                    })
                })
            })
        {
            return false;
        }
        let Some(cap) = capability(product_id) else {
            return false;
        };
        let Ok(observed) = self
            .requested
            .resolve_profiles(product_id, self.onboard_profiles.as_deref())
        else {
            return false;
        };
        self.requested
            .changes
            .iter()
            .zip(&self.reports)
            .all(|(change, report)| {
                let Ok(profiles) = targets(cap, *change, &observed) else {
                    return false;
                };
                let Ok(values) = change.values() else {
                    return false;
                };
                let expected: Vec<_> = profiles
                    .into_iter()
                    .flat_map(|profile| values.iter().map(move |value| (profile, *value)))
                    .collect();
                report.verified()
                    && report.steps.len() == expected.len()
                    && report
                        .steps
                        .iter()
                        .zip(expected)
                        .all(|(step, (profile, value))| {
                            step.profile_id == profile && step.requested == value
                        })
            })
    }
}

fn targets(
    cap: &MouseScrollCapability,
    change: ScrollChange,
    observed: &ScrollProfileObservation,
) -> Result<Vec<u8>> {
    ensure!(
        observed.first_matching_obm_slot_id != Some(0)
            && observed.matching_obm_profile_ids.iter().all(|id| *id > 0),
        "Observed actual hardware profile IDs must be nonzero"
    );
    let mut result = vec![1];
    // Smart Reel visits all actual engine GUID matches. The older 45066 mode
    // maker instead uses the first matching obmData slot. Its levels use only 1.
    if matches!(change, ScrollChange::SmartReel(_)) {
        for id in &observed.matching_obm_profile_ids {
            if *id != 1 {
                ensure!(
                    !result.contains(id),
                    "Duplicate observed OBM profile ID {id}"
                );
                result.push(*id);
            }
        }
    } else if cap.product_id == 45066 && matches!(change, ScrollChange::LegacyMode(_)) {
        if let Some(id) = observed.first_matching_obm_slot_id.filter(|id| *id != 1) {
            result.push(id);
        }
    } else if cap.product_id != 45066 {
        if let Some(id) = observed.selected_obm_profile_id.filter(|id| *id > 1) {
            result.push(id);
        }
    }
    Ok(result)
}

fn parse(data: &[u8], profile_id: u8, kind: ScrollReadKind) -> Result<ScrollValue> {
    ensure!(
        data.len() >= 2,
        "Mouse scroll response payload is truncated"
    );
    ensure!(
        data[0] == profile_id,
        "Mouse scroll response profile does not match"
    );
    let raw = data[1];
    Ok(match kind {
        ScrollReadKind::LegacyMode => ScrollValue::LegacyMode(ScrollMode::decode(raw, false)?),
        ScrollReadKind::ThreeMode => ScrollValue::ThreeMode(ScrollMode::decode(raw, true)?),
        ScrollReadKind::EnabledModes => ScrollValue::EnabledModes(EnabledScrollModes::new(raw)?),
        ScrollReadKind::VirtualAcceleration | ScrollReadKind::SmartReel => {
            ensure!(
                raw <= 1,
                "Mouse scroll enabled response is not a source boolean"
            );
            if kind == ScrollReadKind::VirtualAcceleration {
                ScrollValue::VirtualAcceleration(raw == 1)
            } else {
                ScrollValue::SmartReel(raw == 1)
            }
        }
        ScrollReadKind::VirtualAccelerationLevel => ScrollValue::VirtualAccelerationLevel(raw),
        ScrollReadKind::SmartReelThreshold => ScrollValue::SmartReelThreshold(raw),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrollWriteStep {
    pub profile_id: u8,
    pub requested: ScrollValue,
    /// SUCCESS was received; an empty source setter ACK is permitted.
    pub acknowledgment_received: bool,
    /// Setter echo, when supplied. It does not stand in for independent readback.
    pub acknowledged: Option<ScrollValue>,
    pub observed: Option<ScrollValue>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScrollWriteReport {
    pub steps: Vec<ScrollWriteStep>,
}

impl ScrollWriteReport {
    pub fn verified(&self) -> bool {
        !self.steps.is_empty()
            && self
                .steps
                .iter()
                .all(|step| step.acknowledgment_received && step.observed == Some(step.requested))
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum ScrollFailurePhase {
    Prepare,
    Write,
    Readback,
}

#[derive(Debug)]
pub struct ScrollWriteFailure {
    pub phase: ScrollFailurePhase,
    pub progress: ScrollWriteReport,
    pub failed_step: Option<usize>,
    error: anyhow::Error,
}

impl ScrollWriteFailure {
    /// Any attempted write can have reached the device even if its reply failed.
    pub fn device_state_may_have_changed(&self) -> bool {
        !self.progress.steps.is_empty()
    }
}
impl std::fmt::Display for ScrollWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse scroll {:?} failed; confirmed steps are retained: {:#}",
            self.phase, self.error
        )
    }
}
impl std::error::Error for ScrollWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub struct MouseScroll<'a> {
    device: &'a dyn FeatureTransport,
    cap: &'static MouseScrollCapability,
    transport: &'a DeviceReadCapability,
}

impl<'a> MouseScroll<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        let cap = capability(transport.product_id)
            .context("Direct mouse scrolling is not source-gated for this product")?;
        ensure!(
            matches!(
                transport.source_class.as_str(),
                "rzDevice25" | "rzDevice25DualLinkMouse"
            ),
            "Mouse scroll requires the current source's wired or DualLink adapter"
        );
        Ok(Self {
            device,
            cap,
            transport,
        })
    }

    pub fn submit(
        &self,
        submission: &Submission,
        mut next: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<SubmissionReport> {
        submission.validate(self.transport.product_id)?;
        let mut reports = Vec::new();
        let mut errors = Vec::new();
        let mut onboard_profiles = None;
        if submission
            .changes
            .iter()
            .any(|change| submission.needs_observation(self.transport.product_id, *change))
        {
            match crate::mouse_onboard::read_profile_associations(
                self.device,
                self.transport,
                &mut next,
                &validate,
            ) {
                Ok(profiles) => onboard_profiles = Some(profiles),
                Err(error) => {
                    errors.push(format!("OBM profile observation: {error:#}"));
                    validate()?;
                }
            }
        }
        let resolved =
            submission.resolve_profiles(self.transport.product_id, onboard_profiles.as_deref());
        for (index, change) in submission.changes.iter().enumerate() {
            validate()?;
            let profiles = match &resolved {
                Ok(profiles) => profiles,
                Err(_) if submission.needs_observation(self.transport.product_id, *change) => {
                    reports.push(ScrollWriteReport::default());
                    continue;
                }
                Err(_) => &submission.profiles,
            };
            match self.apply(*change, profiles, &mut next, &validate) {
                Ok(report) => reports.push(report),
                Err(failure) => {
                    errors.push(format!("task {index} ({change:?}): {failure:#}"));
                    reports.push(failure.progress);
                    // Source feature tasks are independent. Device rejection
                    // does not discard later edits; cancellation stops them.
                    validate()?;
                }
            }
        }
        let current = if submission.observe_current {
            Some(self.read_current(&mut next, &validate)?)
        } else {
            None
        };
        Ok(SubmissionReport {
            requested: submission.clone(),
            onboard_profiles,
            reports,
            errors,
            current,
        })
    }

    pub fn read_current(
        &self,
        mut next: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<ScrollSnapshot> {
        let mut readings = Vec::new();
        // Observe the same active profile targeted by the source restore tasks.
        // These GETs never copy local profile fields into the hardware.
        for kind in self.cap.read_order() {
            validate()?;
            let raw = (|| -> Result<_> {
                let command = self.cap.command(*kind, false)?;
                let data = exchange(
                    self.device,
                    self.transport,
                    command,
                    &[1],
                    next()?,
                    &validate,
                )?;
                ensure!(
                    data.len() >= 2 && data[0] == 1,
                    "Current mouse scroll response is truncated or belongs to another profile"
                );
                Ok(data[1])
            })();
            let outcome = match raw {
                Ok(raw) => match parse(&[1, raw], 1, *kind) {
                    Ok(value) => ScrollReadOutcome::Observed(value),
                    Err(_) => ScrollReadOutcome::Unknown { raw },
                },
                Err(error) => {
                    validate()?;
                    ScrollReadOutcome::Failed {
                        message: format!("{error:#}"),
                    }
                }
            };
            readings.push(ScrollReading {
                kind: *kind,
                outcome,
            });
        }
        validate()?;
        Ok(ScrollSnapshot { readings })
    }

    pub fn read(
        &self,
        profile_id: u8,
        kind: ScrollReadKind,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<ScrollValue> {
        ensure!(profile_id > 0, "Mouse scroll profile ID cannot be zero");
        let command = self.cap.command(kind, false)?;
        validate()?;
        let data = exchange(
            self.device,
            self.transport,
            command,
            &[profile_id],
            next_transaction()?,
            &validate,
        )?;
        parse(&data, profile_id, kind)
    }

    pub fn apply(
        &self,
        change: ScrollChange,
        profiles: &ScrollProfileObservation,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<ScrollWriteReport, ScrollWriteFailure> {
        let mut progress = ScrollWriteReport::default();
        let prepared = (|| -> Result<_> {
            let values = change.values()?;
            for value in &values {
                self.cap.command(value.kind(), true)?;
            }
            let targets = targets(self.cap, change, profiles)?;
            validate()?;
            Ok((values, targets))
        })();
        let (values, profiles) = prepared.map_err(|error| ScrollWriteFailure {
            phase: ScrollFailurePhase::Prepare,
            progress: progress.clone(),
            failed_step: None,
            error,
        })?;
        for profile_id in profiles {
            for requested in &values {
                // Validate before recording an attempted mutation or minting an ID.
                if let Err(error) = validate() {
                    return Err(ScrollWriteFailure {
                        phase: ScrollFailurePhase::Write,
                        progress,
                        failed_step: None,
                        error,
                    });
                }
                let index = progress.steps.len();
                progress.steps.push(ScrollWriteStep {
                    profile_id,
                    requested: *requested,
                    acknowledgment_received: false,
                    acknowledged: None,
                    observed: None,
                });
                let result = (|| -> Result<_> {
                    let command = self.cap.command(requested.kind(), true)?;
                    let data = exchange(
                        self.device,
                        self.transport,
                        command,
                        &[profile_id, requested.raw()],
                        next_transaction()?,
                        &validate,
                    )?;
                    progress.steps[index].acknowledgment_received = true;
                    // Source setters report SUCCESS without requiring an echo;
                    // level parsers explicitly allow packetSizeReturned == 0.
                    // Preserve an available echo separately from the later read.
                    if !data.is_empty() {
                        progress.steps[index].acknowledged =
                            Some(parse(&data, profile_id, requested.kind())?);
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    return Err(ScrollWriteFailure {
                        phase: ScrollFailurePhase::Write,
                        progress,
                        failed_step: Some(index),
                        error,
                    });
                }
            }
        }
        // Preserve the original complete mutation order before confirmation.
        for index in 0..progress.steps.len() {
            let step = progress.steps[index].clone();
            let result = (|| -> Result<()> {
                let observed = self.read(
                    step.profile_id,
                    step.requested.kind(),
                    &mut next_transaction,
                    &validate,
                )?;
                progress.steps[index].observed = Some(observed);
                ensure!(
                    observed == step.requested,
                    "Mouse scroll readback differs from the acknowledged write"
                );
                Ok(())
            })();
            if let Err(error) = result {
                return Err(ScrollWriteFailure {
                    phase: ScrollFailurePhase::Readback,
                    progress,
                    failed_step: Some(index),
                    error,
                });
            }
        }
        Ok(progress)
    }
}

#[cfg(test)]
#[path = "mouse_scroll_tests.rs"]
mod tests;
