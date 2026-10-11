//! Current 2676/2684 tp/Rc + controllerCalibrationReducer.
//! START changes step locally; visual timers never acknowledge calibration.
use super::calibration::state::CalibrationAction;
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct TriggerCalibrationIntent {
    generation: u64,
    part: u8,
    action: CalibrationAction,
}
impl TriggerCalibrationIntent {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn part(&self) -> u8 {
        self.part
    }
    pub fn action(&self) -> CalibrationAction {
        self.action
    }
    pub fn payload(&self) -> Value {
        json!({"action": if self.action == CalibrationAction::Start { "start" } else { "stop" }, "partId": self.part})
    }
}

#[derive(Clone, Debug)]
enum ObservationValue {
    Progress {
        step: i8,
        valid: bool,
        part: Option<u8>,
    },
    Movement(f64),
    Unavailable,
}

/// Only fresh service progress or CalibrationUserMovement storage events.
#[derive(Clone, Debug)]
pub struct TriggerCalibrationObservation {
    generation: u64,
    sequence: u64,
    value: ObservationValue,
}
impl TriggerCalibrationObservation {
    pub fn progress(
        generation: u64,
        sequence: u64,
        step: i8,
        valid: bool,
        part: Option<u8>,
    ) -> Option<Self> {
        // The mounted popup observes the shared reducer, not an invented
        // trigger-only monotonic state machine. The source accepts these enums.
        if !matches!(step, -1..=6 | 10..=12) {
            return None;
        }
        Some(Self {
            generation,
            sequence,
            value: ObservationValue::Progress { step, valid, part },
        })
    }
    pub fn user_movement(generation: u64, sequence: u64, t: f64) -> Option<Self> {
        t.is_finite().then(|| Self {
            generation,
            sequence,
            value: ObservationValue::Movement(t.clamp(0., 100.)),
        })
    }
    pub fn unavailable(generation: u64, sequence: u64) -> Self {
        Self {
            generation,
            sequence,
            value: ObservationValue::Unavailable,
        }
    }
}

#[derive(Default)]
pub(super) struct TriggerCalibrationState {
    generation: u64,
    sequence: Option<u64>,
    intent: Option<TriggerCalibrationIntent>,
    submission_error: Option<String>,
    pub(super) step: i8,
    pub(super) valid: bool,
    reducer_part: Option<u8>,
    pub(super) selected_part: u8,
    pub(super) raw_trigger_percent: Option<f64>,
    timer_key: Option<(u8, bool)>,
    timer_started: Option<Instant>,
    marker_started: Option<Instant>,
    marker_from: f64,
    marker_target: f64,
    pub(super) marker_progress: f64,
    prompt_started: Option<Instant>,
    pub(super) prompt_progress: f64,
    pub(super) timer_progress: f64,
}
impl TriggerCalibrationState {
    pub(super) fn set_reducer_valid(&mut self, valid: bool) {
        self.valid = valid;
    }
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
    pub(super) fn intent(&self) -> Option<&TriggerCalibrationIntent> {
        self.intent.as_ref()
    }
    pub(super) fn part(&self) -> u8 {
        self.reducer_part
            .filter(|p| matches!(p, 3 | 4))
            .unwrap_or(self.selected_part)
    }
    pub(super) fn open(&mut self, part: u8, now: Instant) -> Option<TriggerCalibrationIntent> {
        if !matches!(part, 3 | 4) {
            return None;
        }
        self.selected_part = part;
        self.raw_trigger_percent = Some(100.);
        self.marker_started = None;
        self.marker_from = 0.;
        self.marker_target = 0.;
        self.marker_progress = 0.;
        self.request(CalibrationAction::Start, part, now)
    }
    pub(super) fn request(
        &mut self,
        action: CalibrationAction,
        part: u8,
        now: Instant,
    ) -> Option<TriggerCalibrationIntent> {
        if !matches!(part, 3 | 4) || action == CalibrationAction::RotateComplete {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.sequence = None;
        self.submission_error = None;
        self.reducer_part = Some(part);
        self.step = if action == CalibrationAction::Start {
            10
        } else {
            0
        };
        // Source START/STOP spread the reducer. isStepValid is retained.
        self.sync_visual_phase(now);
        let intent = TriggerCalibrationIntent {
            generation: self.generation,
            part,
            action,
        };
        self.intent = Some(intent.clone());
        Some(intent)
    }
    pub(super) fn observe(
        &mut self,
        observation: TriggerCalibrationObservation,
        now: Instant,
    ) -> bool {
        if observation.generation != self.generation
            || self.sequence.is_some_and(|s| observation.sequence <= s)
        {
            return false;
        }
        match observation.value {
            ObservationValue::Progress { step, valid, part } => {
                self.step = step;
                self.valid = valid;
                // MW_UPDATE_CONTROLLER_CALIBRATION_PROGRESS doesn't change
                // partId. Optional part is a genuine full reducer observation.
                if let Some(part) = part {
                    self.reducer_part = Some(part);
                }
            }
            ObservationValue::Movement(t) => {
                self.tick(now);
                self.raw_trigger_percent = Some(t);
                self.marker_from = self.marker_progress;
                self.marker_target = 100. - t;
                self.marker_started = Some(now);
            }
            ObservationValue::Unavailable => {
                self.generation = self.generation.wrapping_add(1);
                self.sequence = None;
                self.intent = None;
                self.timer_started = None;
                self.marker_started = None;
                self.prompt_started = None;
                return true;
            }
        }
        self.sequence = Some(observation.sequence);
        self.sync_visual_phase(now);
        true
    }
    fn phase(&self) -> Option<bool> {
        if matches!(self.step, -1 | 12) {
            None
        } else {
            Some(self.step >= 11)
        }
    }
    fn show_prompt(&self) -> bool {
        self.phase() == Some(false) && self.raw_trigger_percent.unwrap_or(100.) >= 95.
    }
    fn sync_visual_phase(&mut self, now: Instant) {
        let key = self.phase().map(|release| (self.part(), release));
        if key != self.timer_key {
            self.timer_key = key;
            self.timer_started = None;
            self.timer_progress = 0.;
        }
        if key.is_none() || !self.valid {
            self.timer_started = None;
            self.timer_progress = 0.;
        } else if self.timer_started.is_none() {
            self.timer_started = Some(now);
        }
        if self.show_prompt() {
            self.prompt_started.get_or_insert(now);
        } else {
            self.prompt_started = None;
            self.prompt_progress = 0.;
        }
    }
    pub(super) fn tick(&mut self, now: Instant) {
        if let Some(start) = self.timer_started {
            let seconds = if self.phase() == Some(true) { 3. } else { 2. };
            self.timer_progress =
                (now.saturating_duration_since(start).as_secs_f64() / seconds * 100.).min(100.);
        }
        if let Some(start) = self.marker_started {
            let fraction = (now.saturating_duration_since(start).as_secs_f64() / 0.05).min(1.);
            let ease = 1. - (1. - fraction).powi(3);
            self.marker_progress =
                self.marker_from + (self.marker_target - self.marker_from) * ease;
            if fraction == 1. {
                self.marker_started = None;
            }
        }
        if let Some(start) = self.prompt_started {
            self.prompt_progress = (now.saturating_duration_since(start).as_secs_f64() % 2.) / 2.;
        }
    }
    pub(super) fn needs_frame(&self) -> bool {
        self.marker_started.is_some()
            || self.prompt_started.is_some()
            || self.timer_started.is_some() && self.timer_progress < 100.
    }
    pub(super) fn stop_visuals(&mut self) {
        self.timer_key = None;
        self.timer_started = None;
        self.marker_started = None;
        self.prompt_started = None;
        self.timer_progress = 0.;
        self.prompt_progress = 0.;
    }
    pub(super) fn submission_error(&self) -> Option<&str> {
        self.submission_error.as_deref()
    }
    pub(super) fn finish_submission(
        &mut self,
        generation: u64,
        result: Result<(), String>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        // Transport failure is separate from the service's Step_Error. A
        // completed send must not fabricate Step_3_Trigger_Complete either.
        self.submission_error = result.err();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn visual_timer_never_completes_the_service_step() {
        let now = Instant::now();
        let mut s = TriggerCalibrationState::default();
        let intent = s.open(3, now).unwrap();
        assert_eq!(intent.payload(), json!({"action":"start","partId":3}));
        assert_eq!(s.step, 10);
        assert!(!s.valid);
        assert!(s.observe(
            TriggerCalibrationObservation::progress(s.generation(), 1, 10, true, None).unwrap(),
            now
        ));
        s.tick(now + Duration::from_secs(4));
        assert_eq!(s.timer_progress, 100.);
        assert_eq!(s.step, 10);
        assert!(s.observe(
            TriggerCalibrationObservation::progress(s.generation(), 2, 11, true, None).unwrap(),
            now + Duration::from_secs(4)
        ));
        assert_eq!(s.timer_progress, 0.);
        s.tick(now + Duration::from_secs(8));
        assert_eq!(s.step, 11);
        assert!(s.observe(
            TriggerCalibrationObservation::progress(s.generation(), 3, 12, false, None).unwrap(),
            now + Duration::from_secs(8)
        ));
        assert_eq!(s.step, 12);
        assert_eq!(s.timer_progress, 0.);
    }
    #[test]
    fn start_preserves_valid_and_cleanup_stops_the_selected_part() {
        let now = Instant::now();
        let mut s = TriggerCalibrationState::default();
        s.open(3, now);
        let stale = s.generation();
        s.observe(
            TriggerCalibrationObservation::progress(stale, 1, -1, true, Some(4)).unwrap(),
            now,
        );
        assert_eq!(s.part(), 4);
        s.request(CalibrationAction::Start, s.part(), now).unwrap();
        assert!(s.valid);
        assert!(!s.observe(
            TriggerCalibrationObservation::progress(stale, 2, 12, true, None).unwrap(),
            now
        ));
        let stop = s
            .request(CalibrationAction::Stop, s.selected_part, now)
            .unwrap();
        assert_eq!(stop.part(), 3);
        assert!(s.valid);
        assert_eq!(s.step, 0);
    }
    #[test]
    fn movement_is_finite_clamped_and_interpolated_without_progress_mutation() {
        let now = Instant::now();
        let mut s = TriggerCalibrationState::default();
        s.open(4, now);
        assert!(
            TriggerCalibrationObservation::user_movement(s.generation(), 1, f64::NAN).is_none()
        );
        let movement =
            TriggerCalibrationObservation::user_movement(s.generation(), 1, -20.).unwrap();
        assert!(s.observe(movement, now));
        s.tick(now + Duration::from_millis(25));
        assert_eq!(s.raw_trigger_percent, Some(0.));
        assert!((s.marker_progress - 87.5).abs() < 1e-8);
        s.tick(now + Duration::from_millis(50));
        assert_eq!(s.marker_progress, 100.);
        assert_eq!(s.step, 10);
    }
}
