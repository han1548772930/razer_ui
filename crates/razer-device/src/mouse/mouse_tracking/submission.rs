//! Typed source storage values and retained results at the worker boundary.
use super::*;
pub use crate::feature_submission::Target;
use crate::receiver_session::ReceiverSession;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "TrackingFields", into = "TrackingFields")]
pub struct SmartTracking {
    fields: TrackingFields,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackingFields {
    is_asymmetric: bool,
    tracking_distance: u8,
    lift_off_distance: u8,
    landing_distance: u8,
}

impl TryFrom<TrackingFields> for SmartTracking {
    type Error = anyhow::Error;
    fn try_from(fields: TrackingFields) -> Result<Self> {
        ensure!(
            (1..=3).contains(&fields.tracking_distance),
            "Tracking distance is outside 1..3"
        );
        ensure!(
            (2..=26).contains(&fields.lift_off_distance)
                && fields.landing_distance > 0
                && fields.landing_distance < fields.lift_off_distance,
            "Tracking cutoffs must satisfy 1 <= landing < lift <= 26"
        );
        Ok(Self { fields })
    }
}
impl From<SmartTracking> for TrackingFields {
    fn from(value: SmartTracking) -> Self {
        value.fields
    }
}
impl SmartTracking {
    pub fn setting(self) -> TrackingSetting {
        if self.fields.is_asymmetric {
            TrackingSetting::Asymmetric {
                lift_off_distance: self.fields.lift_off_distance,
                landing_distance: self.fields.landing_distance,
            }
        } else {
            TrackingSetting::Symmetric {
                tracking_distance: self.fields.tracking_distance,
            }
        }
    }

    /// The current local storage call merges only the supplied smartTracking
    /// fields into calibration; profiles, preset data and extensions survive.
    pub fn merge_local_settings(self, root: &mut Option<serde_json::Value>) -> Result<()> {
        self.merge_local_settings_from(root, None)
    }

    /// Migrate a previously stored profile projection without discarding
    /// extension fields. An existing device record remains authoritative.
    pub fn merge_local_settings_from(
        self,
        root: &mut Option<serde_json::Value>,
        projection: Option<&serde_json::Value>,
    ) -> Result<()> {
        use serde_json::{Value, json};
        let root = root.get_or_insert_with(|| json!({}));
        let root = root
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Tracking root storage is not an object"))?;
        let calibration = root.entry("calibration").or_insert_with(|| json!({}));
        let calibration = calibration
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Calibration storage is not an object"))?;
        if let Some(projection) = projection {
            let projection = projection
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Calibration projection is not an object"))?;
            for (key, value) in projection {
                calibration
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        let smart = calibration
            .entry("smartTracking")
            .or_insert_with(|| json!({}));
        let smart = smart
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Smart tracking storage is not an object"))?;
        if let Some(projected) = projection.and_then(|projection| projection.get("smartTracking")) {
            let projected = projected
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Smart tracking projection is not an object"))?;
            for (key, value) in projected {
                smart.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
        let Value::Object(fields) = serde_json::to_value(self)? else {
            unreachable!("typed smart tracking object");
        };
        smart.extend(fields);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SubmissionReport {
    pub requested: TrackingSetting,
    pub transport_source_class: String,
    pub outcome: Option<std::result::Result<TrackingWriteResult, TrackingWriteFailure>>,
    pub receiver_before: Option<Vec<(u16, u8)>>,
    pub receiver_after: Option<Vec<(u16, u8)>>,
    pub observation_error: Option<String>,
}
impl SubmissionReport {
    pub fn acknowledged(&self) -> bool {
        self.observation_error.is_none()
            && self.outcome.as_ref().is_some_and(|outcome| {
                outcome
                    .as_ref()
                    .is_ok_and(|result| result.source_chain_acknowledged && !result.verified)
            })
    }
}

pub type Progress = crate::feature_submission::Progress<TrackingSetting, SubmissionReport>;

/// Keep acknowledged setters even when a later peer observation or scope check
/// fails. No report projects getter values or successful rollback into the UI.
pub fn submit(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    setting: TrackingSetting,
    session: Option<&ReceiverSession>,
    mut next: impl FnMut() -> Result<u8>,
    validate: impl Fn() -> Result<()>,
) -> SubmissionReport {
    let mut report = SubmissionReport {
        requested: setting,
        transport_source_class: cap.source_class.clone(),
        outcome: None,
        receiver_before: None,
        receiver_after: None,
        observation_error: None,
    };
    if let Some(session) = session {
        let outcome = &mut report.outcome;
        let before = &mut report.receiver_before;
        let after = &mut report.receiver_after;
        let observation = session.observe_recording(
            device,
            &mut next,
            &validate,
            |next, guard| {
                *outcome = Some(apply(device, cap, setting, next, guard));
                Ok(())
            },
            |phase, peers| match phase {
                crate::receiver_session::PeerObservationPhase::Before => {
                    *before = Some(peers.to_vec())
                }
                crate::receiver_session::PeerObservationPhase::After => {
                    *after = Some(peers.to_vec())
                }
            },
        );
        match observation {
            Ok(observation) => {
                report.receiver_before = Some(observation.receiver_before);
                report.receiver_after = Some(observation.receiver_after);
            }
            Err(error) => report.observation_error = Some(format!("{error:#}")),
        }
    } else {
        report.outcome = Some(apply(device, cap, setting, &mut next, &validate));
    }
    report
}
