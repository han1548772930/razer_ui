//! Source calibration configuration at the local persistence boundary.
use super::*;
use anyhow::Context as _;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct LiftStage(u8);
impl TryFrom<u8> for LiftStage {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self> {
        ensure!(
            (1..=10).contains(&value),
            "Philips lift stage is outside 1..10"
        );
        Ok(Self(value))
    }
}
impl From<LiftStage> for u8 {
    fn from(value: LiftStage) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProfileId(String);
impl TryFrom<String> for ProfileId {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        ensure!(
            !value.is_empty() && value.len() <= 128 && !value.contains('\0'),
            "Calibration profile GUID is invalid"
        );
        Ok(Self(value))
    }
}
impl From<ProfileId> for String {
    fn from(value: ProfileId) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Surface {
    #[serde(rename = "Goliathus_Speed")]
    GoliathusSpeed,
    #[serde(rename = "Goliathus_Control")]
    GoliathusControl,
    Sphex,
    #[serde(rename = "Vespula_Speed")]
    VespulaSpeed,
    #[serde(rename = "Vespula_Control")]
    VespulaControl,
    Kabuto,
    Destructor2,
    Manticor,
    #[serde(rename = "Invicta_Speed")]
    InvictaSpeed,
    #[serde(rename = "Invicta_Control")]
    InvictaControl,
    Megasoma2,
    #[serde(rename = "Firefly_Hard")]
    FireflyHard,
    #[serde(rename = "Firefly_Cloth")]
    FireflyCloth,
    Gigantus,
    #[serde(rename = "Goliathus_Mobile")]
    GoliathusMobile,
    SphexV2,
}
impl Surface {
    pub fn parameters(self) -> (u16, [u16; 10]) {
        match self {
            Self::GoliathusSpeed => (2310, [349, 353, 357, 365, 382, 415, 480, 612, 875, 1400]),
            Self::GoliathusControl => (2365, [366, 370, 373, 381, 395, 424, 483, 600, 833, 1300]),
            Self::Sphex => (2520, [295, 297, 298, 301, 308, 320, 346, 396, 498, 700]),
            Self::VespulaSpeed => (2790, [285, 287, 288, 291, 298, 311, 337, 389, 493, 700]),
            Self::VespulaControl => (2815, [350, 351, 353, 355, 361, 372, 394, 438, 525, 700]),
            Self::Kabuto => (2260, [244, 249, 254, 264, 283, 323, 401, 558, 872, 1500]),
            Self::Destructor2 => (2490, [234, 236, 238, 243, 252, 269, 305, 376, 517, 800]),
            Self::Manticor => (2265, [167, 171, 175, 183, 199, 232, 296, 425, 684, 1200]),
            Self::InvictaSpeed => (2530, [264, 266, 267, 271, 278, 291, 319, 373, 482, 700]),
            Self::InvictaControl => (2505, [255, 257, 258, 262, 269, 283, 311, 366, 478, 700]),
            Self::Megasoma2 => (2370, [228, 230, 232, 235, 243, 258, 287, 346, 464, 700]),
            Self::FireflyHard => (2370, [320, 321, 323, 326, 332, 344, 368, 415, 510, 700]),
            Self::FireflyCloth => (2439, [440, 457, 474, 481, 489, 496, 504, 511, 519, 526]),
            Self::Gigantus => (2276, [452, 469, 486, 488, 490, 492, 494, 496, 498, 500]),
            Self::GoliathusMobile => (2248, [315, 331, 348, 361, 374, 387, 399, 412, 425, 438]),
            Self::SphexV2 => (2555, [274, 290, 307, 338, 368, 399, 430, 461, 491, 522]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCalibration {
    x_offset: f64,
    x_swing: f64,
    #[serde(rename = "xTIAGain")]
    x_tia_gain: f64,
    x_reference_height_a: f64,
    x_reference_height_b: f64,
    temperature_var_a: f64,
    temperature_var_b: f64,
    y_offset: f64,
    y_swing: f64,
    #[serde(rename = "yTIAGain")]
    y_tia_gain: f64,
    y_reference_height_a: f64,
    y_reference_height_b: f64,
    mean: f64,
    standard_deviation: f64,
    #[serde(rename = "VPower")]
    v_power: f64,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
/// The source's four bitwise helpers use ToInt32 followed by masks. Numeric
/// truncation and wrapping happen here, rather than reinterpreting float bits.
fn word(value: f64) -> Result<u32> {
    ensure!(value.is_finite(), "Philips calibration value is not finite");
    Ok(value.trunc().rem_euclid(4_294_967_296.0) as u32)
}
impl RawCalibration {
    pub(super) fn matches_measurement(&self, sensor: &SensorCalibration) -> bool {
        [
            (self.x_offset, sensor.x_offset),
            (self.x_swing, sensor.x_swing),
            (self.x_tia_gain, sensor.x_tia_gain),
            (self.x_reference_height_a, sensor.x_reference_height_a),
            (self.x_reference_height_b, sensor.x_reference_height_b),
            (self.temperature_var_a, sensor.temperature_var_a),
            (self.temperature_var_b, sensor.temperature_var_b),
            (self.y_offset, sensor.y_offset),
            (self.y_swing, sensor.y_swing),
            (self.y_tia_gain, sensor.y_tia_gain),
            (self.y_reference_height_a, sensor.y_reference_height_a),
            (self.y_reference_height_b, sensor.y_reference_height_b),
        ]
        .iter()
        .all(|(measured, observed)| *measured == f64::from(*observed))
    }
    pub(super) fn measured(sensor: SensorCalibration, statistics: measurement::Statistics) -> Self {
        Self {
            x_offset: f64::from(sensor.x_offset),
            x_swing: f64::from(sensor.x_swing),
            x_tia_gain: f64::from(sensor.x_tia_gain),
            x_reference_height_a: f64::from(sensor.x_reference_height_a),
            x_reference_height_b: f64::from(sensor.x_reference_height_b),
            temperature_var_a: f64::from(sensor.temperature_var_a),
            temperature_var_b: f64::from(sensor.temperature_var_b),
            y_offset: f64::from(sensor.y_offset),
            y_swing: f64::from(sensor.y_swing),
            y_tia_gain: f64::from(sensor.y_tia_gain),
            y_reference_height_a: f64::from(sensor.y_reference_height_a),
            y_reference_height_b: f64::from(sensor.y_reference_height_b),
            mean: statistics.mean,
            standard_deviation: statistics.standard_deviation,
            v_power: statistics.v_power,
            extensions: BTreeMap::from([
                ("classId".into(), Value::from(0)),
                ("sensorId".into(), Value::from(5)),
                (
                    "sensorIdEnum".into(),
                    Value::from("Philips_Proximity_sensor"),
                ),
            ]),
        }
    }
    fn calibration(&self, surface_distance: f64) -> Result<SensorCalibration> {
        Ok(SensorCalibration {
            x_offset: word(self.x_offset)?,
            x_swing: word(self.x_swing)?,
            x_tia_gain: word(self.x_tia_gain)?,
            x_reference_height_a: word(
                self.x_reference_height_a + self.mean - 5.0 * surface_distance,
            )?,
            x_reference_height_b: word(
                self.x_reference_height_b + self.mean - 5.0 * surface_distance,
            )?,
            temperature_var_a: word(self.temperature_var_a)?,
            temperature_var_b: word(self.temperature_var_b)?,
            y_offset: word(self.y_offset)?,
            y_swing: word(self.y_swing)?,
            y_tia_gain: word(self.y_tia_gain)?,
            y_reference_height_a: word(
                self.y_reference_height_a + self.mean - 5.0 * surface_distance,
            )?,
            y_reference_height_b: word(
                self.y_reference_height_b + self.mean - 5.0 * surface_distance,
            )?,
        })
    }
    pub fn preset(&self, surface: Surface, lift: LiftStage) -> Result<Setting> {
        let (distance, thresholds) = surface.parameters();
        Ok(Setting::Apply {
            calibration: self.calibration(f64::from(distance))?,
            lift_off_threshold: thresholds[usize::from(lift.0 - 1)],
        })
    }
    fn custom(&self, lift: LiftStage) -> Result<Setting> {
        ensure!(
            self.mean.is_finite()
                && self.standard_deviation.is_finite()
                && self.v_power.is_finite(),
            "Philips custom statistics are not finite"
        );
        let mut distance = 4.3;
        while distance >= 2.8 && !(179.65 * self.v_power * (-2.06_f64 * distance).exp() > 800.0) {
            distance -= 0.1;
        }
        let mut upper = 1000.0 * (distance - 2.3);
        let lower = (3.0 * self.standard_deviation / 5.0).clamp(100.0, 350.0);
        if upper < lower + 50.0 {
            upper = lower + 50.0;
        } else if upper > 2000.0 {
            upper = 2000.0;
        }
        let increment = (upper - (lower + 50.0)) / 7.0;
        let threshold = match lift.0 {
            1 => lower,
            2 => lower + 25.0,
            3 => lower + 50.0,
            10 => upper,
            stage => lower + 50.0 + f64::from(stage - 3) * increment,
        };
        Ok(Setting::Apply {
            calibration: self.calibration(0.0)?,
            lift_off_threshold: word(threshold)? as u16,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
enum ProfileKind {
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "preset")]
    Preset {
        #[serde(rename = "surfaceId")]
        surface: Surface,
    },
    #[serde(rename = "custom")]
    Custom {
        #[serde(
            rename = "sensorData",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        sensor_data: Option<RawCalibration>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<RawCalibration>,
    },
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
struct Profile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guid: Option<ProfileId>,
    #[serde(flatten)]
    kind: ProfileKind,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChange {
    profiles: Vec<Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_profile_guid: Option<ProfileId>,
    lift_off_range_value: LiftStage,
}
#[derive(Clone, Default, Deserialize, Serialize)]
struct Selection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guid: Option<ProfileId>,
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredCalibration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profiles: Option<Vec<Profile>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_profile: Option<Selection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lift_off_range_value: Option<LiftStage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preset_sensor_data: Option<RawCalibration>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
#[derive(Clone, Default, Deserialize, Serialize)]
struct Document {
    #[serde(default)]
    calibration: StoredCalibration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<u64>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
fn document(root: Option<&Value>) -> Result<Document> {
    root.map(|root| serde_json::from_value(root.clone()))
        .transpose()
        .map_err(Into::into)
        .map(|document| document.unwrap_or_default())
}
impl ConfigChange {
    pub fn parse(payload: Value) -> Result<Self> {
        ensure!(
            !payload
                .get("selectedProfileGuid")
                .is_some_and(Value::is_null),
            "Selected calibration GUID is null"
        );
        let value: Self = serde_json::from_value(payload)?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.profiles.is_empty(),
            "Calibration profile list is empty"
        );
        let mut identities = BTreeSet::new();
        for profile in &self.profiles {
            ensure!(
                matches!(profile.kind, ProfileKind::Default) || profile.guid.is_some(),
                "Saved calibration profile has no GUID"
            );
            ensure!(
                identities.insert(profile.guid.as_ref()),
                "Calibration profile GUID is repeated"
            );
        }
        self.selected()?;
        Ok(())
    }
    fn selected(&self) -> Result<&Profile> {
        self.profiles
            .iter()
            .find(|profile| profile.guid == self.selected_profile_guid)
            .context("Selected calibration profile does not exist")
    }
    pub fn setting(&self, root: Option<&Value>) -> Result<Setting> {
        self.validate()?;
        match &self.selected()?.kind {
            ProfileKind::Default => Ok(Setting::Disabled),
            ProfileKind::Preset { surface } => document(root)?.calibration.preset_sensor_data
                .context("Philips preset requires locally stored sensor data from a completed mini calibration")?
                .preset(*surface, self.lift_off_range_value),
            ProfileKind::Custom { sensor_data, data } => sensor_data.as_ref().or(data.as_ref())
                .context("Philips custom profile has no measured sensor data")?.custom(self.lift_off_range_value),
        }
    }
    /// Persist the source-configured profiles before queuing its hardware task.
    pub fn merge_local_settings(&self, root: &mut Option<Value>) -> Result<()> {
        self.validate()?;
        let mut stored = document(root.as_ref())?;
        stored.calibration.profiles = Some(self.profiles.clone());
        stored.calibration.selected_profile = Some(Selection {
            guid: self.selected_profile_guid.clone(),
        });
        stored.calibration.lift_off_range_value = Some(self.lift_off_range_value);
        stored.version = Some(crate::mapping_submission::next_source_version(
            stored.version,
        )?);
        let mut published = serde_json::to_value(stored)?;
        // Retained source measurements are immutable input to this operation.
        // Preserve their numeric representation as well as all extension data.
        if let Some(baseline) = root
            .as_ref()
            .and_then(|root| root.pointer("/calibration/presetSensorData"))
        {
            published["calibration"]["presetSensorData"] = baseline.clone();
        }
        *root = Some(published);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum MeasurementDestination {
    Custom,
    Preset { surface: Surface, name: String },
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum MeasurementMode {
    Add,
    Recalibrate { profile: ProfileId },
}
/// A completed physical measurement is immutable input to the save queue.
/// Preparing against the durable owner keeps its extensions and version,
/// while the source UI projection and immediate hardware apply remain distinct.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct MeasurementSave {
    destination: MeasurementDestination,
    mode: MeasurementMode,
    guid: ProfileId,
    calibration_date: String,
    measured: RawCalibration,
}
pub struct PreparedMeasurement {
    root: Value,
    projection: Value,
    setting: Setting,
}
impl PreparedMeasurement {
    pub fn root(&self) -> &Value {
        &self.root
    }
    pub fn projection(&self) -> &Value {
        &self.projection
    }
    pub fn setting(&self) -> Setting {
        self.setting
    }
}
impl MeasurementSave {
    pub fn kind(&self) -> measurement::Kind {
        match self.destination {
            MeasurementDestination::Custom => measurement::Kind::Manual,
            MeasurementDestination::Preset { .. } => measurement::Kind::Mini,
        }
    }
    pub fn new(
        destination: MeasurementDestination,
        mode: MeasurementMode,
        guid: ProfileId,
        calibration_date: String,
        measured: RawCalibration,
    ) -> Result<Self> {
        ensure!(
            !calibration_date.is_empty() && calibration_date.len() <= 64,
            "Invalid calibration date"
        );
        if let MeasurementDestination::Preset { name, .. } = &destination {
            ensure!(
                !name.is_empty() && name.len() <= 256,
                "Invalid calibration surface name"
            );
        }
        Ok(Self {
            destination,
            mode,
            guid,
            calibration_date,
            measured,
        })
    }
    pub fn prepare(&self, root: Option<&Value>) -> Result<PreparedMeasurement> {
        let mut stored = document(root)?;
        let lift = stored
            .calibration
            .lift_off_range_value
            .unwrap_or(LiftStage(1));
        let profiles = stored.calibration.profiles.get_or_insert_with(Vec::new);
        let name = match &self.destination {
            MeasurementDestination::Custom => {
                crate::source_string::unique_profile_name("MOUSE MAT", |candidate| {
                    profiles.iter().any(|profile| {
                        profile.extensions.get("name").and_then(Value::as_str) == Some(candidate)
                    })
                })
            }
            MeasurementDestination::Preset { name, .. } => name.clone(),
        };
        let (kind, setting, surface_id) = match &self.destination {
            MeasurementDestination::Custom => {
                let setting = self.measured.custom(lift)?;
                // calculatePhilipsCustomData mutates the four references in l
                // before getPhilipsManualCalibration copies l into sensorData.
                let mut saved = self.measured.clone();
                saved.x_reference_height_a += saved.mean;
                saved.x_reference_height_b += saved.mean;
                saved.y_reference_height_a += saved.mean;
                saved.y_reference_height_b += saved.mean;
                (
                    ProfileKind::Custom {
                        sensor_data: Some(saved),
                        data: None,
                    },
                    setting,
                    "Custom".to_owned(),
                )
            }
            MeasurementDestination::Preset { surface, .. } => {
                let Setting::Apply { calibration, .. } = self.measured.preset(*surface, lift)?
                else {
                    unreachable!()
                };
                let surface_id = serde_json::to_value(surface)?
                    .as_str()
                    .context("Surface ID is not a string")?
                    .to_owned();
                (
                    ProfileKind::Preset { surface: *surface },
                    Setting::Mini { calibration },
                    surface_id,
                )
            }
        };
        let mut profile = Profile {
            guid: Some(self.guid.clone()),
            kind,
            extensions: BTreeMap::from([
                ("name".into(), Value::String(name)),
                ("surfaceId".into(), Value::String(surface_id)),
                (
                    "calibrationDate".into(),
                    Value::String(self.calibration_date.clone()),
                ),
            ]),
        };
        // surfaceId is a known field of Preset; do not serialize a second copy
        // from its extension map.
        if matches!(profile.kind, ProfileKind::Preset { .. }) {
            profile.extensions.remove("surfaceId");
        }
        match &self.mode {
            MeasurementMode::Add => {
                ensure!(
                    !profiles.iter().any(|old| old.guid == profile.guid),
                    "New calibration GUID already exists"
                );
                profiles.push(profile.clone());
            }
            MeasurementMode::Recalibrate { profile: guid } => {
                let previous = profiles
                    .iter_mut()
                    .find(|profile| profile.guid.as_ref() == Some(guid))
                    .context("Recalibration target no longer exists")?;
                profile.guid = previous.guid.clone();
                if let Some(name) = previous.extensions.get("name") {
                    profile.extensions.insert("name".into(), name.clone());
                }
                // taskMakerEndCalibrationStateMachine changes the durable
                // recalibration date, then separately emits its new UI profile.
                previous.extensions.insert(
                    "calibrationDate".into(),
                    Value::String(self.calibration_date.clone()),
                );
            }
        }
        if matches!(self.destination, MeasurementDestination::Preset { .. })
            && stored.calibration.preset_sensor_data.is_none()
        {
            stored.calibration.preset_sensor_data = Some(self.measured.clone());
        }
        stored.calibration.selected_profile = Some(Selection {
            guid: profile.guid.clone(),
        });
        stored.version = Some(crate::mapping_submission::next_source_version(
            stored.version,
        )?);
        let recalibrated_projection =
            if let MeasurementMode::Recalibrate { profile: guid } = &self.mode {
                let projected: Vec<_> = profiles
                    .iter()
                    .map(|previous| {
                        if previous.guid.as_ref() == Some(guid) {
                            &profile
                        } else {
                            previous
                        }
                    })
                    .collect();
                Some(serde_json::to_value(projected)?)
            } else {
                None
            };
        let mut projection = serde_json::to_value(&stored.calibration)?;
        if let Some(profiles) = recalibrated_projection {
            projection["profiles"] = profiles;
        }
        let mut root_value = serde_json::to_value(stored)?;
        if let Some(baseline) = root
            .and_then(|root| root.pointer("/calibration/presetSensorData"))
            .filter(|baseline| !baseline.is_null())
        {
            root_value["calibration"]["presetSensorData"] = baseline.clone();
            projection["presetSensorData"] = baseline.clone();
        }
        Ok(PreparedMeasurement {
            root: root_value,
            projection,
            setting,
        })
    }
}

/// Move the view's calibration projection into its device owner. Existing
/// sensor measurements and extension fields remain authoritative at that owner.
pub fn capture_projection(root: &mut Option<Value>, projection: &Value) -> Result<()> {
    let incoming: StoredCalibration = serde_json::from_value(projection.clone())?;
    let mut stored = document(root.as_ref())?;
    if let Some(profiles) = incoming.profiles {
        stored.calibration.profiles = Some(profiles);
    }
    if let Some(selected) = incoming.selected_profile {
        stored.calibration.selected_profile = Some(selected);
    }
    if let Some(lift) = incoming.lift_off_range_value {
        stored.calibration.lift_off_range_value = Some(lift);
    }
    if stored.calibration.preset_sensor_data.is_none() {
        stored.calibration.preset_sensor_data = incoming.preset_sensor_data;
    }
    for (key, value) in incoming.extensions {
        stored.calibration.extensions.entry(key).or_insert(value);
    }
    let mut published = serde_json::to_value(stored)?;
    if let Some(baseline) = root
        .as_ref()
        .and_then(|root| root.pointer("/calibration/presetSensorData"))
        .or_else(|| projection.get("presetSensorData"))
    {
        published["calibration"]["presetSensorData"] = baseline.clone();
    }
    *root = Some(published);
    Ok(())
}
