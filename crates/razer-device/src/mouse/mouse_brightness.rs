//! Current 170/204/226/45066 mouse brightness over the retained Feature session.
//! Current source gates, byte layouts and task order are recorded separately in
//! `docs/re/mouse-brightness-current-source.json`. No local draft is hardware state.
use crate::{
    backend::FeatureTransport, device_reads::DeviceReadCapability, feature_exchange::exchange,
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;

const REGIONS: [u8; 3] = [80, 15, 128];
const GET: [u8; 3] = [3, 15, 132];
const SET: [u8; 3] = [3, 15, 4];
const ALL_REGION: u8 = 0;

#[derive(Clone, Copy, Debug)]
pub struct MouseBrightnessCapability {
    product_id: u32,
}

const PRODUCTS: &[MouseBrightnessCapability] = &[
    MouseBrightnessCapability { product_id: 170 },
    MouseBrightnessCapability { product_id: 204 },
    MouseBrightnessCapability { product_id: 226 },
    MouseBrightnessCapability { product_id: 45066 },
];

pub fn capability(product_id: u32) -> Option<&'static MouseBrightnessCapability> {
    PRODUCTS.iter().find(|cap| cap.product_id == product_id)
}

impl MouseBrightnessCapability {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
}

/// Each source region row contains five bytes. Unknown IDs/types stay observable.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BrightnessRegion {
    pub region_id: u8,
    pub update_rate: u8,
    pub data_type: u8,
    pub array_x: u8,
    pub array_y: u8,
}

fn parse_regions(data: &[u8]) -> Result<Vec<BrightnessRegion>> {
    ensure!(
        data.len().is_multiple_of(5),
        "Mouse brightness region table has a truncated five-byte row"
    );
    Ok(data
        .chunks_exact(5)
        .map(|row| BrightnessRegion {
            region_id: row[0],
            update_rate: row[1],
            data_type: row[2],
            array_x: row[3],
            array_y: row[4],
        })
        .collect())
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BrightnessValue {
    pub profile_id: u8,
    pub region_id: u8,
    pub raw_value: u8,
    pub percent: u8,
}

fn raw_percent(raw: u8) -> u8 {
    (u16::from(raw) * 100).div_ceil(255) as u8
}

fn percent_raw(percent: u8) -> u8 {
    (u16::from(percent) * 255 / 100) as u8
}

fn parse_brightness(data: &[u8], profile_id: u8, region_id: u8) -> Result<BrightnessValue> {
    ensure!(data.len() >= 3, "Mouse brightness response is truncated");
    ensure!(
        data[0] == profile_id,
        "Mouse brightness response profile does not match"
    );
    // All-region setter echoes may identify a concrete region. The source
    // getter resolves AllRegion before sending its command when a row exists.
    ensure!(
        region_id == ALL_REGION || data[1] == region_id,
        "Mouse brightness response region does not match"
    );
    Ok(BrightnessValue {
        profile_id: data[0],
        region_id: data[1],
        raw_value: data[2],
        percent: raw_percent(data[2]),
    })
}

#[derive(Clone, Debug, Serialize)]
pub enum BrightnessRegionResolution {
    Explicit,
    FirstAvailable {
        regions: Vec<BrightnessRegion>,
    },
    EmptyRegionList,
    /// Original getter catches a failed region query and uses region 1.
    /// The error is retained; this does not claim successful discovery.
    SourceFallback {
        error: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct BrightnessObservation {
    pub requested_region_id: u8,
    pub queried_region_id: u8,
    pub resolution: BrightnessRegionResolution,
    pub value: BrightnessValue,
}

#[derive(Clone, Debug)]
pub struct BrightnessProfileAssociation {
    pub guid: String,
    pub slot_id: u8,
}

/// Actual `obmData.profiles` observed by the caller, separate from draft intent.
#[derive(Clone, Debug, Default)]
pub struct BrightnessProfileObservation {
    pub active_guid: String,
    pub observed_profiles: Vec<BrightnessProfileAssociation>,
}

impl BrightnessProfileObservation {
    pub fn matching_profile_ids(&self) -> Result<Vec<u8>> {
        ensure!(
            self.observed_profiles.is_empty() || !self.active_guid.is_empty(),
            "Observed OBM associations require the actual active profile GUID"
        );
        let ids: Vec<_> = self
            .observed_profiles
            .iter()
            .filter(|profile| profile.guid == self.active_guid)
            .map(|profile| profile.slot_id)
            .collect();
        ensure!(
            ids.iter().all(|id| *id > 0),
            "Observed matching mouse profile IDs must be nonzero"
        );
        // The source reduce preserves every match, including slot 1 and order.
        Ok(ids)
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BrightnessChange {
    pub is_enabled: bool,
    /// Integral 0..100 API input. Disabling sends zero and retains draft value
    /// solely in the caller; this executor never persists local profile data.
    pub percent: u8,
    /// Zero follows the current product's default AllRegion task branch.
    pub region_id: u8,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum BrightnessScope {
    Active,
    Onboard,
}

#[derive(Clone, Debug, Serialize)]
pub struct BrightnessWriteStep {
    pub scope: BrightnessScope,
    pub profile_id: u8,
    pub requested_region_id: u8,
    pub requested_percent: u8,
    pub previous: Option<BrightnessObservation>,
    /// Entered the mutation exchange. A missing reply does not prove no write.
    pub write_attempted: bool,
    pub acknowledgment_received: bool,
    pub acknowledged: Option<BrightnessValue>,
    /// Separate post-mutation getter. Skips retain their pre-read in `previous`.
    pub observed: Option<BrightnessObservation>,
    /// Source conditional read/set chain completed, before Rust readback.
    pub source_completed: bool,
    pub error: Option<String>,
}

impl BrightnessWriteStep {
    fn verified(&self) -> bool {
        if !self.source_completed || self.error.is_some() {
            return false;
        }
        if self.write_attempted {
            self.acknowledgment_received
                && self.observed.as_ref().is_some_and(|observation| {
                    observation.value.raw_value == percent_raw(self.requested_percent)
                })
        } else {
            self.previous
                .as_ref()
                .is_some_and(|observation| observation.value.percent == self.requested_percent)
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BrightnessWriteReport {
    pub steps: Vec<BrightnessWriteStep>,
    /// Hardware part of the original task only; no local persistence is implied.
    pub source_completed: bool,
}

impl BrightnessWriteReport {
    pub fn verified(&self) -> bool {
        self.source_completed
            && !self.steps.is_empty()
            && self.steps.iter().all(BrightnessWriteStep::verified)
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum BrightnessFailurePhase {
    Prepare,
    Read,
    Write,
    Onboard,
    Readback,
    Cancelled,
}

#[derive(Debug)]
pub struct BrightnessWriteFailure {
    pub phase: BrightnessFailurePhase,
    pub progress: BrightnessWriteReport,
    pub failed_step: Option<usize>,
    error: anyhow::Error,
}

impl BrightnessWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.progress.steps.iter().any(|step| step.write_attempted)
    }
}

impl std::fmt::Display for BrightnessWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mouse brightness {:?} failed; hardware progress is retained: {:#}",
            self.phase, self.error
        )
    }
}

impl std::error::Error for BrightnessWriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

pub struct MouseBrightness<'a> {
    device: &'a dyn FeatureTransport,
    cap: &'static MouseBrightnessCapability,
    transport: &'a DeviceReadCapability,
}

impl<'a> MouseBrightness<'a> {
    pub fn new(
        device: &'a dyn FeatureTransport,
        transport: &'a DeviceReadCapability,
    ) -> Result<Self> {
        let cap = capability(transport.product_id)
            .context("Direct mouse brightness is not source-gated for this product")?;
        ensure!(
            transport.source_class == "rzDevice25DualLinkMouse",
            "Mouse brightness requires the audited rzDevice25 DualLink adapter"
        );
        Ok(Self {
            device,
            cap,
            transport,
        })
    }

    pub fn product_id(&self) -> u32 {
        self.cap.product_id
    }

    pub fn read_regions(
        &self,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<Vec<BrightnessRegion>> {
        validate()?;
        parse_regions(&exchange(
            self.device,
            self.transport,
            REGIONS,
            &[],
            next_transaction()?,
            &validate,
        )?)
    }

    pub fn read(
        &self,
        profile_id: u8,
        region_id: u8,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> Result<BrightnessObservation> {
        ensure!(profile_id > 0, "Mouse brightness profile ID cannot be zero");
        validate()?;
        let (queried_region_id, resolution) = if region_id == ALL_REGION {
            let region_data = exchange(
                self.device,
                self.transport,
                REGIONS,
                &[],
                next_transaction()?,
                &validate,
            );
            match region_data {
                Ok(data) => {
                    // Malformed rows fail explicitly instead of assuming a LED.
                    let regions = parse_regions(&data)?;
                    if let Some(first) = regions.first() {
                        (
                            first.region_id,
                            BrightnessRegionResolution::FirstAvailable { regions },
                        )
                    } else {
                        (ALL_REGION, BrightnessRegionResolution::EmptyRegionList)
                    }
                }
                Err(error) => {
                    // Cancellation/session loss must never become fallback I/O.
                    validate()?;
                    (
                        1,
                        BrightnessRegionResolution::SourceFallback {
                            error: format!("{error:#}"),
                        },
                    )
                }
            }
        } else {
            (region_id, BrightnessRegionResolution::Explicit)
        };
        validate()?;
        let data = exchange(
            self.device,
            self.transport,
            GET,
            &[profile_id, queried_region_id],
            next_transaction()?,
            &validate,
        )?;
        Ok(BrightnessObservation {
            requested_region_id: region_id,
            queried_region_id,
            resolution,
            value: parse_brightness(&data, profile_id, queried_region_id)?,
        })
    }

    fn run_source_step(
        &self,
        step: &mut BrightnessWriteStep,
        next_transaction: &mut impl FnMut() -> Result<u8>,
        validate: &impl Fn() -> Result<()>,
    ) -> std::result::Result<(), (BrightnessFailurePhase, anyhow::Error)> {
        let query_region = if step.scope == BrightnessScope::Active {
            step.requested_region_id
        } else {
            ALL_REGION
        };
        let previous = self
            .read(
                step.profile_id,
                query_region,
                &mut *next_transaction,
                validate,
            )
            .map_err(|error| (BrightnessFailurePhase::Read, error))?;
        let write = previous.value.percent != step.requested_percent
            || (step.scope == BrightnessScope::Active
                && step.requested_region_id == ALL_REGION
                && previous.value.region_id != ALL_REGION);
        step.previous = Some(previous);
        if write {
            validate().map_err(|error| (BrightnessFailurePhase::Cancelled, error))?;
            let transaction =
                next_transaction().map_err(|error| (BrightnessFailurePhase::Write, error))?;
            validate().map_err(|error| (BrightnessFailurePhase::Cancelled, error))?;
            step.write_attempted = true;
            let data = exchange(
                self.device,
                self.transport,
                SET,
                &[
                    step.profile_id,
                    step.requested_region_id,
                    percent_raw(step.requested_percent),
                ],
                transaction,
                validate,
            )
            .map_err(|error| (BrightnessFailurePhase::Write, error))?;
            step.acknowledgment_received = true;
            if !data.is_empty() {
                step.acknowledged = Some(
                    parse_brightness(&data, step.profile_id, step.requested_region_id)
                        .map_err(|error| (BrightnessFailurePhase::Write, error))?,
                );
            }
        }
        step.source_completed = true;
        Ok(())
    }

    pub fn apply(
        &self,
        change: BrightnessChange,
        profiles: &BrightnessProfileObservation,
        mut next_transaction: impl FnMut() -> Result<u8>,
        validate: impl Fn() -> Result<()>,
    ) -> std::result::Result<BrightnessWriteReport, BrightnessWriteFailure> {
        let mut progress = BrightnessWriteReport::default();
        let prepared = (|| -> Result<Vec<u8>> {
            ensure!(
                change.percent <= 100,
                "Mouse brightness percent must be 0..100"
            );
            let ids = profiles.matching_profile_ids()?;
            validate()?;
            Ok(ids)
        })();
        let ids = prepared.map_err(|error| BrightnessWriteFailure {
            phase: BrightnessFailurePhase::Prepare,
            progress: progress.clone(),
            failed_step: None,
            error,
        })?;
        let requested_percent = if change.is_enabled { change.percent } else { 0 };
        let mut onboard_errors = Vec::new();
        for (scope, profile_id) in std::iter::once((BrightnessScope::Active, 1))
            .chain(ids.into_iter().map(|id| (BrightnessScope::Onboard, id)))
        {
            if let Err(error) = validate() {
                return Err(BrightnessWriteFailure {
                    phase: BrightnessFailurePhase::Cancelled,
                    progress,
                    failed_step: None,
                    error,
                });
            }
            let index = progress.steps.len();
            progress.steps.push(BrightnessWriteStep {
                scope,
                profile_id,
                requested_region_id: if scope == BrightnessScope::Active {
                    change.region_id
                } else {
                    ALL_REGION
                },
                requested_percent,
                previous: None,
                write_attempted: false,
                acknowledgment_received: false,
                acknowledged: None,
                observed: None,
                source_completed: false,
                error: None,
            });
            if let Err((phase, error)) =
                self.run_source_step(&mut progress.steps[index], &mut next_transaction, &validate)
            {
                progress.steps[index].error = Some(format!("{error:#}"));
                if let Err(cancelled) = validate() {
                    return Err(BrightnessWriteFailure {
                        phase: BrightnessFailurePhase::Cancelled,
                        progress,
                        failed_step: Some(index),
                        error: cancelled.context(error),
                    });
                }
                if scope == BrightnessScope::Active || phase == BrightnessFailurePhase::Cancelled {
                    return Err(BrightnessWriteFailure {
                        phase,
                        progress,
                        failed_step: Some(index),
                        error,
                    });
                }
                // Source OBM helper catches each profile and continues, then
                // rejects its aggregate. Do not lose acknowledged earlier steps.
                onboard_errors.push((index, format!("Profile {profile_id}: {error:#}")));
            }
        }
        if !onboard_errors.is_empty() {
            return Err(BrightnessWriteFailure {
                phase: BrightnessFailurePhase::Onboard,
                failed_step: Some(onboard_errors[0].0),
                progress,
                error: anyhow::anyhow!(
                    onboard_errors
                        .iter()
                        .map(|(_, message)| message.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            });
        }
        progress.source_completed = true;
        // Confirmation is an additional Rust policy after the complete source
        // hardware task, so it cannot reorder its active/OBM writes.
        for index in 0..progress.steps.len() {
            if !progress.steps[index].write_attempted {
                continue;
            }
            let step = &mut progress.steps[index];
            let result = (|| -> Result<()> {
                let observed = self.read(
                    step.profile_id,
                    step.requested_region_id,
                    &mut next_transaction,
                    &validate,
                )?;
                let matches = observed.value.raw_value == percent_raw(step.requested_percent);
                step.observed = Some(observed);
                ensure!(
                    matches,
                    "Mouse brightness raw readback differs from the acknowledged write"
                );
                Ok(())
            })();
            if let Err(error) = result {
                progress.steps[index].error = Some(format!("{error:#}"));
                return Err(BrightnessWriteFailure {
                    phase: if validate().is_err() {
                        BrightnessFailurePhase::Cancelled
                    } else {
                        BrightnessFailurePhase::Readback
                    },
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
#[path = "mouse_brightness_tests.rs"]
mod tests;
