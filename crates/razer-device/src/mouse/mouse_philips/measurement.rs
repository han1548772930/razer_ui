//! Current PID 70 manual/Mini engines. Logical time and device events drive
//! this state machine; the service owns I/O, callback leases and restoration.
use super::*;
use anyhow::Context as _;
use std::collections::VecDeque;

pub const INITIATING_TIMEOUT_MS: u64 = 35_000;
pub const GENERAL_TIMEOUT_MS: u64 = 60_000;
pub const MINI_COLLECTION_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Kind {
    Manual,
    Mini,
}
impl Kind {
    fn control(self) -> ControlType {
        match self {
            Self::Manual => ControlType::Manual,
            Self::Mini => ControlType::Preset,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum CalibrationEvent {
    Start,
    End,
    Halt,
    Reset,
    NormalSpeed,
    TooFast,
    Cancelled,
}
impl TryFrom<u8> for CalibrationEvent {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Start),
            1 => Ok(Self::End),
            2 => Ok(Self::Halt),
            3 => Ok(Self::Reset),
            4 => Ok(Self::NormalSpeed),
            5 => Ok(Self::TooFast),
            6 => Ok(Self::Cancelled),
            _ => anyhow::bail!("Unknown Philips calibration event {value}"),
        }
    }
}
impl From<CalibrationEvent> for u8 {
    fn from(value: CalibrationEvent) -> Self {
        match value {
            CalibrationEvent::Start => 0,
            CalibrationEvent::End => 1,
            CalibrationEvent::Halt => 2,
            CalibrationEvent::Reset => 3,
            CalibrationEvent::NormalSpeed => 4,
            CalibrationEvent::TooFast => 5,
            CalibrationEvent::Cancelled => 6,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    LeftDown,
    Sample { z: u8, q: u8 },
    Calibration(CalibrationEvent),
}
/// Protocol25Parser uses report 5/9 for notifications, 77 for buttons and 90
/// for Z/Q. Other hardware reports belong to other device features.
pub fn parse_report(bytes: &[u8]) -> Result<Option<Input>> {
    let Some(&record) = bytes.first() else {
        anyhow::bail!("Empty Philips hardware report");
    };
    if !matches!(record, 5 | 9 | 77 | 90) {
        return Ok(None);
    }
    ensure!(
        bytes.len() >= 3,
        "Truncated Philips hardware report {record}"
    );
    Ok(match record {
        5 | 9 if bytes[1] == 10 => Some(Input::Calibration(bytes[2].try_into()?)),
        77 if bytes[1] == 0 && bytes[2] == 1 => Some(Input::LeftDown),
        90 => Some(Input::Sample {
            z: bytes[1],
            q: bytes[2],
        }),
        _ => None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ControlType {
    Manual,
    Preset,
}
impl From<ControlType> for u8 {
    fn from(value: ControlType) -> Self {
        match value {
            ControlType::Manual => 0,
            ControlType::Preset => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ControlStage {
    Start,
    Stop,
}
impl From<ControlStage> for u8 {
    fn from(value: ControlStage) -> Self {
        match value {
            ControlStage::Start => 0,
            ControlStage::Stop => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum DeviceCommand {
    SensorState(bool),
    Configure,
    Control {
        calibration_type: ControlType,
        stage: ControlStage,
    },
    ReadCalibration,
}
impl DeviceCommand {
    pub fn packet(self) -> ([u8; 3], Vec<u8>) {
        match self {
            Self::SensorState(enabled) => (SET_STATE, vec![0, 5, u8::from(enabled)]),
            Self::Configure => ([8, 11, 1], vec![0, 5, 0, 10, 0, 0, 0, 0]),
            Self::Control {
                calibration_type,
                stage,
            } => (
                [4, 11, 9],
                vec![0, 5, calibration_type.into(), stage.into()],
            ),
            Self::ReadCalibration => ([50, 11, 136], {
                let mut payload = vec![0; 50];
                payload[1] = 5;
                payload
            }),
        }
    }
}
pub fn read_calibration(payload: &[u8]) -> Result<SensorCalibration> {
    ensure!(
        payload.len() == 50,
        "Philips calibration data needs 50 bytes"
    );
    ensure!(
        payload[..2] == [0, 5],
        "Philips calibration class/sensor changed"
    );
    let mut values = payload[2..]
        .chunks_exact(4)
        .map(|word| u32::from_be_bytes(word.try_into().expect("four-byte calibration word")));
    let mut next = || values.next().expect("twelve calibration words");
    Ok(SensorCalibration {
        x_offset: next(),
        x_swing: next(),
        x_tia_gain: next(),
        x_reference_height_a: next(),
        x_reference_height_b: next(),
        temperature_var_a: next(),
        temperature_var_b: next(),
        y_offset: next(),
        y_swing: next(),
        y_tia_gain: next(),
        y_reference_height_a: next(),
        y_reference_height_b: next(),
    })
}
pub fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: DeviceCommand,
    transaction: u8,
    validate: impl Fn() -> Result<()>,
    sent: &mut impl FnMut(),
) -> Result<Vec<u8>> {
    validate_transport(cap)?;
    let (header, payload) = command.packet();
    let result = feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        cap,
        header,
        &payload,
        transaction,
        header[0],
        &validate,
        sent,
    )?;
    if command == DeviceCommand::ReadCalibration {
        read_calibration(&result)?;
    } else if !result.is_empty() {
        ensure!(
            result.len() >= 2 && result[..2] == [0, 5],
            "Philips response class/sensor changed"
        );
    }
    validate()?;
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Statistics {
    pub sample_count: u64,
    pub mean: f64,
    pub standard_deviation: f64,
    pub v_power: f64,
}
#[derive(Default)]
struct Samples {
    count: u64,
    mean: f64,
    sum_squares: f64,
    q_counts: [u64; 2],
    q_sums: [f64; 2],
}
impl Samples {
    fn push(&mut self, z: u8, q: u8) {
        let z = f64::from(z) * 32.0;
        self.count += 1;
        if self.count == 1 {
            self.mean = z;
        } else {
            let next = self.mean + (z - self.mean) / self.count as f64;
            self.sum_squares += (z - self.mean) * (z - next);
            self.mean = next;
        }
        let group = usize::from(q & 128 != 0);
        self.q_counts[group] += 1;
        self.q_sums[group] += 2_f64.powf(f64::from(q & 127) / 4.0);
    }
    fn statistics(&self) -> Statistics {
        let average = |group: usize| {
            if self.q_counts[group] == 0 {
                0.0
            } else {
                self.q_sums[group] / self.q_counts[group] as f64
            }
        };
        Statistics {
            sample_count: self.count,
            mean: self.mean,
            standard_deviation: if self.count > 1 {
                (self.sum_squares / (self.count - 1) as f64).sqrt()
            } else {
                0.0
            },
            v_power: if self.count > 31_744 {
                10_000.0
            } else {
                average(0).min(average(1))
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Phase {
    Initiating,
    AwaitingClick,
    AwaitingStart,
    Collecting,
    AwaitingEnd,
    Reading,
    Releasing,
    Finished,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Effect {
    Device(DeviceCommand),
    Escape(bool),
    Redirect(bool),
    Release,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CommandReport {
    pub command: DeviceCommand,
    pub acknowledged: Option<Vec<u8>>,
    pub command_may_have_reached_device: bool,
    pub error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Notification {
    pub event: CalibrationEvent,
    pub succeeded: bool,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Progress {
    pub operation_id: String,
    pub target: crate::device_reads::DeviceReadTarget,
    pub kind: Kind,
    pub phase: Phase,
    pub terminal: bool,
    pub canceled: bool,
    pub cleanup_confirmed: bool,
    pub notifications: Vec<Notification>,
    pub commands: Vec<CommandReport>,
    pub initialization_error: Option<String>,
    pub driver_ready: bool,
    pub redirect_callbacks: u64,
    pub mouse_move_callbacks: u64,
    pub measured: Option<RawCalibration>,
    pub errors: Vec<String>,
}
impl Progress {
    pub fn validate(
        &self,
        id: &str,
        target: &crate::device_reads::DeviceReadTarget,
        kind: Kind,
    ) -> Result<()> {
        ensure!(
            self.operation_id == id && self.target == *target && self.kind == kind,
            "Philips measurement response owner, target or kind changed"
        );
        ensure!(
            self.terminal == (self.phase == Phase::Finished),
            "Philips terminal phase is inconsistent"
        );
        ensure!(
            !self.cleanup_confirmed || self.terminal,
            "Philips cleanup was claimed before release"
        );
        if let Some(measured) = &self.measured {
            ensure!(
                self.driver_ready,
                "Philips measured data has no correlated driver-ready observation"
            );
            ensure!(
                self.notifications
                    .iter()
                    .any(|event| event.event == CalibrationEvent::End && event.succeeded),
                "Philips measured data has no successful source End event"
            );
            let getter = self
                .commands
                .iter()
                .rev()
                .find(|command| command.command == DeviceCommand::ReadCalibration)
                .context("Philips measured data has no calibration getter report")?;
            ensure!(
                getter.command_may_have_reached_device && getter.error.is_none(),
                "Philips measurement getter was not acknowledged"
            );
            let sensor = read_calibration(
                getter
                    .acknowledged
                    .as_deref()
                    .context("Philips measured data has no getter acknowledgement")?,
            )?;
            ensure!(
                measured.matches_measurement(&sensor),
                "Philips measured calibration differs from its getter acknowledgement"
            );
        }
        Ok(())
    }
}
pub struct Machine {
    kind: Kind,
    phase: Phase,
    deadline: Option<u64>,
    pending: VecDeque<Effect>,
    samples: Samples,
    sensor_attempted: bool,
    redirect_attempted: bool,
    canceled: bool,
    cleanup_confirmed: bool,
    notifications: Vec<Notification>,
    measured: Option<RawCalibration>,
    errors: Vec<String>,
}
impl Machine {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            phase: Phase::Initiating,
            deadline: None,
            pending: VecDeque::from([
                Effect::Device(DeviceCommand::SensorState(true)),
                Effect::Device(DeviceCommand::Configure),
                Effect::Escape(true),
            ]),
            samples: Samples::default(),
            sensor_attempted: false,
            redirect_attempted: false,
            canceled: false,
            cleanup_confirmed: false,
            notifications: Vec::new(),
            measured: None,
            errors: Vec::new(),
        }
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn cleanup_confirmed(&self) -> bool {
        self.phase == Phase::Finished && self.cleanup_confirmed
    }
    pub fn next_effect(&mut self) -> Option<Effect> {
        let effect = self.pending.front().copied()?;
        match effect {
            Effect::Device(DeviceCommand::SensorState(true)) => self.sensor_attempted = true,
            Effect::Redirect(true) => self.redirect_attempted = true,
            _ => {}
        }
        Some(effect)
    }
    pub fn complete(
        &mut self,
        effect: Effect,
        result: Result<Option<Vec<u8>>>,
        now: u64,
    ) -> Result<()> {
        ensure!(
            self.pending.front() == Some(&effect),
            "Philips effect owner changed"
        );
        self.pending.pop_front();
        match result {
            Err(error) => {
                let error = format!("{effect:?}: {error:#}");
                if self.phase == Phase::Releasing
                    || matches!(effect, Effect::Redirect(_) | Effect::Escape(_))
                {
                    // The source invokes redirect/listener promises without
                    // awaiting them. Adapter diagnostics do not synthesize a
                    // source Halt notification or change its phase deadline.
                    self.errors.push(error);
                } else {
                    self.fail(error);
                }
            }
            Ok(payload) => match effect {
                Effect::Device(DeviceCommand::ReadCalibration) => {
                    match payload
                        .as_deref()
                        .context("Philips getter returned no data")
                        .and_then(read_calibration)
                    {
                        Ok(sensor) => {
                            self.measured =
                                Some(RawCalibration::measured(sensor, self.samples.statistics()));
                            self.notify(CalibrationEvent::End, true);
                            self.release(false, false);
                        }
                        Err(error) => self.fail(format!("Philips getter data: {error:#}")),
                    }
                }
                Effect::Release => self.cleanup_confirmed = true,
                _ => {}
            },
        }
        if self.pending.is_empty() {
            match self.phase {
                Phase::Initiating => {
                    self.phase = Phase::AwaitingClick;
                    self.deadline = Some(now.saturating_add(INITIATING_TIMEOUT_MS));
                }
                Phase::AwaitingStart | Phase::AwaitingEnd => {
                    self.deadline = Some(now.saturating_add(GENERAL_TIMEOUT_MS));
                }
                Phase::Releasing => self.phase = Phase::Finished,
                _ => {}
            }
        }
        Ok(())
    }
    fn notify(&mut self, event: CalibrationEvent, succeeded: bool) {
        self.notifications.push(Notification { event, succeeded });
    }
    fn release(&mut self, stop: bool, disable: bool) {
        self.pending.clear();
        self.deadline = None;
        if self.redirect_attempted {
            self.pending.push_back(Effect::Redirect(false));
        }
        self.pending.push_back(Effect::Escape(false));
        if stop {
            // The Mini reset path also uses Manual/Stop in the current source.
            self.pending
                .push_back(Effect::Device(DeviceCommand::Control {
                    calibration_type: ControlType::Manual,
                    stage: ControlStage::Stop,
                }));
        }
        if disable && self.sensor_attempted {
            self.pending
                .push_back(Effect::Device(DeviceCommand::SensorState(false)));
        }
        self.pending.push_back(Effect::Release);
        self.phase = Phase::Releasing;
    }
    pub fn fail(&mut self, error: String) {
        if matches!(self.phase, Phase::Finished | Phase::Releasing) {
            self.errors.push(error);
            return;
        }
        self.errors.push(error);
        self.notify(CalibrationEvent::Halt, false);
        self.release(
            matches!(self.phase, Phase::AwaitingStart | Phase::Collecting),
            true,
        );
    }
    /// Adapter observation failures require cleanup, but are not vendor events.
    pub fn abort(&mut self, error: String) {
        self.errors.push(error);
        if !matches!(self.phase, Phase::Finished | Phase::Releasing) {
            self.release(
                matches!(self.phase, Phase::AwaitingStart | Phase::Collecting),
                true,
            );
        }
    }
    pub fn cancel(&mut self) {
        if matches!(self.phase, Phase::Finished | Phase::Releasing) {
            return;
        }
        self.canceled = true;
        self.notify(CalibrationEvent::Cancelled, false);
        self.release(
            matches!(self.phase, Phase::AwaitingStart | Phase::Collecting),
            true,
        );
    }
    pub fn retry_cleanup(&mut self) {
        if self.phase == Phase::Finished && !self.cleanup_confirmed {
            self.pending.push_back(Effect::Release);
            self.phase = Phase::Releasing;
        }
    }
    fn stop_collection(&mut self) {
        self.deadline = None;
        self.phase = Phase::AwaitingEnd;
        self.pending.extend([
            Effect::Redirect(false),
            Effect::Escape(false),
            Effect::Device(DeviceCommand::Control {
                calibration_type: self.kind.control(),
                stage: ControlStage::Stop,
            }),
        ]);
    }
    pub fn tick(&mut self, now: u64) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            if self.phase == Phase::Collecting {
                self.stop_collection();
            } else {
                self.fail(format!("Philips {:?} timed out", self.phase));
            }
        }
    }
    pub fn observe(&mut self, input: Input, now: u64) {
        if matches!(self.phase, Phase::Finished | Phase::Releasing) {
            return;
        }
        match input {
            Input::Sample { z, q } => self.samples.push(z, q),
            Input::LeftDown if self.phase == Phase::AwaitingClick => {
                self.deadline = None;
                self.phase = Phase::AwaitingStart;
                self.pending
                    .push_back(Effect::Device(DeviceCommand::Control {
                        calibration_type: self.kind.control(),
                        stage: ControlStage::Start,
                    }));
            }
            Input::LeftDown if self.phase == Phase::Collecting && self.kind == Kind::Manual => {
                self.stop_collection()
            }
            Input::Calibration(event) => match self.phase {
                Phase::AwaitingStart if event == CalibrationEvent::Start => {
                    self.phase = Phase::Collecting;
                    self.deadline = Some(now.saturating_add(if self.kind == Kind::Mini {
                        MINI_COLLECTION_MS
                    } else {
                        GENERAL_TIMEOUT_MS
                    }));
                    self.pending.push_back(Effect::Redirect(true));
                    self.notify(event, true);
                }
                Phase::AwaitingEnd => {
                    self.deadline = None;
                    if event == CalibrationEvent::End {
                        self.phase = Phase::Reading;
                        self.pending
                            .push_back(Effect::Device(DeviceCommand::ReadCalibration));
                    } else {
                        if matches!(event, CalibrationEvent::Reset | CalibrationEvent::Halt) {
                            self.notify(event, true);
                        }
                        self.release(false, false);
                    }
                }
                Phase::Initiating | Phase::AwaitingClick | Phase::Collecting => {
                    self.notify(event, true);
                    if event == CalibrationEvent::Reset {
                        self.release(false, false);
                    }
                }
                Phase::AwaitingStart if event == CalibrationEvent::Reset => {
                    self.notify(event, true);
                    self.release(false, false);
                }
                _ => {}
            },
            _ => {}
        }
    }
    pub fn progress(
        &self,
        id: &str,
        target: &crate::device_reads::DeviceReadTarget,
        commands: Vec<CommandReport>,
    ) -> Progress {
        Progress {
            operation_id: id.into(),
            target: target.clone(),
            kind: self.kind,
            phase: self.phase,
            terminal: self.phase == Phase::Finished,
            canceled: self.canceled,
            cleanup_confirmed: self.cleanup_confirmed,
            notifications: self.notifications.clone(),
            commands,
            initialization_error: None,
            driver_ready: false,
            redirect_callbacks: 0,
            mouse_move_callbacks: 0,
            measured: self.measured.clone(),
            errors: self.errors.clone(),
        }
    }
}

#[cfg(test)]
#[path = "measurement_tests.rs"]
mod tests;
