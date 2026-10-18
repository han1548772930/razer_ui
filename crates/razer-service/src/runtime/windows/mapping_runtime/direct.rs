//! Device-thread primitive output execution through the retained driver route.
use super::{
    Cancellation, State, audio,
    device::gamepad_observation::{Control, Observation, Sink},
    driver, input, launch, timing,
};
use anyhow::{Result, ensure};
use razer_device::{
    macro_runtime::{Event, Keyboard, KeyboardExecutor},
    mapping_native_analog::NativeAnalogOutput,
    mapping_native_digital::NativeDigitalOutput,
    mapping_native_input::NativeInput,
    mapping_native_lookup::InputIdentity,
    mapping_native_output::{CompletionPoll, DriverInputReport, MappingOutputTransport},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[path = "direct/output_state.rs"]
mod output_state;
use output_state::OutputState;

pub(super) struct Direct {
    input: input::Input,
    output: driver::Output,
    route: driver::Route,
    keyboard: KeyboardExecutor,
    outputs: OutputState,
    gamepad_observation: Observation,
    cancel: Arc<Cancellation>,
    manager: Arc<Mutex<State>>,
    epoch: Instant,
    cleanup: bool,
}
impl Direct {
    pub fn open(
        route: driver::Route,
        cancel: Arc<Cancellation>,
        manager: Arc<Mutex<State>>,
    ) -> Result<Self> {
        let acceleration = manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping manager poisoned"))?
            .acceleration
            .clone();
        Ok(Self {
            input: input::Input::open(Some(&route), cancel.clone(), acceleration)?,
            output: route.output(),
            route,
            keyboard: KeyboardExecutor::new(timing()),
            outputs: OutputState::default(),
            gamepad_observation: Observation::default(),
            cancel,
            manager,
            epoch: Instant::now(),
            cleanup: false,
        })
    }
    fn tick(&self) -> u64 {
        self.epoch
            .elapsed()
            .as_micros()
            .try_into()
            .unwrap_or(u64::MAX)
    }
    pub fn gamepad_observation(&mut self, control: Control, sink: Sink) -> Result<()> {
        self.gamepad_observation.control(control, sink)
    }
    pub fn poll(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.gamepad_observation.next_deadline().is_some() {
            match razer_platform::event_clock::tick() {
                Ok(tick) => self.gamepad_observation.poll(tick),
                Err(error) => errors.push(format!("Gamepad event clock: {error:#}")),
            }
        }
        let now = self.tick();
        errors.extend(self.keyboard.poll(now, &mut |key| self.input.keyboard(key)));
        errors
    }
    pub fn next_wait(&self) -> Duration {
        let keyboard =
            self.keyboard
                .next_deadline_us()
                .map_or(Duration::from_millis(4), |deadline| {
                    Duration::from_micros(deadline.saturating_sub(self.tick()))
                        .min(Duration::from_millis(4))
                });
        self.gamepad_observation
            .next_deadline()
            .and_then(|deadline| {
                razer_platform::event_clock::tick()
                    .ok()
                    .map(|now| Duration::from_micros(deadline.saturating_sub(now)))
            })
            .map_or(keyboard, |wait| wait.min(keyboard))
    }
    pub fn native(&mut self, input: NativeInput) -> Result<()> {
        self.input.send(input)
    }
    pub fn cleanup(&mut self, cleanup: bool) {
        self.cleanup = cleanup;
        self.input.cancellable = !cleanup;
    }
    pub fn action(&mut self, action: razer_device::macro_runtime::Action) -> Result<()> {
        use razer_device::macro_runtime::Action;
        match action {
            Action::Keyboard(key) => self.keyboard(key, false),
            Action::Mouse(mouse) => self.native(NativeInput::Mouse(mouse.input())),
            Action::MouseMove {
                mode,
                x,
                y,
                origin_epoch,
            } => self.input.movement(mode, x, y, origin_epoch),
            Action::Launch(action) => self.event(&Event::Launch(action), false),
            Action::Clipboard(text) => self.event(&Event::Clipboard(text), false),
            Action::Audio(action) => self.event(&Event::Audio(action), false),
        }
    }
    pub fn keyboard(&mut self, key: Keyboard, smart_stop: bool) -> Result<()> {
        self.keyboard
            .execute(key, smart_stop, self.tick(), &mut |key| {
                self.input.keyboard(key)
            })
            .map_err(anyhow::Error::msg)
    }
    pub fn event(&mut self, event: &Event, smart_stop: bool) -> Result<()> {
        match event {
            Event::Keyboard(key) => self.keyboard(key.clone(), smart_stop),
            Event::Mouse(mouse) => self.native(NativeInput::Mouse(mouse.input())),
            Event::Clipboard(text) => {
                let client = self
                    .manager
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping manager poisoned"))?
                    .clipboard_client()?;
                client.paste(text.clone(), Some(self.route.clone()))
            }
            Event::Launch(action) => launch::execute(action),
            Event::Audio(action) => {
                let report = audio::execute(*action)?;
                ensure!(report.errors.is_empty(), "{}", report.errors.join("; "));
                Ok(())
            }
            Event::Delay { milliseconds } => self.wait(*milliseconds),
            Event::RandomDelay { min_ms, max_ms } => {
                if min_ms > max_ms {
                    return Ok(());
                }
                self.wait(super::random(*min_ms, *max_ms)?)
            }
            Event::MouseMove { .. }
            | Event::LoopStart { .. }
            | Event::LoopEnd
            | Event::Macro { .. } => {
                anyhow::bail!("Macro-only event reached the direct primitive executor")
            }
        }
    }
    fn wait(&self, milliseconds: u32) -> Result<()> {
        self.cancel.wait(Duration::from_millis(milliseconds.into()));
        ensure!(!self.cancel.is_cancelled(), "Mapping sequence was canceled");
        Ok(())
    }
    pub fn digital(&mut self, key: &InputIdentity, output: NativeDigitalOutput) -> Result<()> {
        let reports = self.outputs.digital(key, output)?;
        self.reports(reports)
    }
    pub fn combination(&mut self, key: &str, output: NativeDigitalOutput) -> Result<()> {
        let reports = self.outputs.combination(key, output)?;
        self.reports(reports)
    }
    pub fn analog(
        &mut self,
        key: &InputIdentity,
        output: &NativeAnalogOutput,
        actuation: u16,
    ) -> Result<()> {
        let reports = self.outputs.analog(key, output, actuation);
        self.reports(reports)
    }
    pub fn begin_hardware_frame(&mut self) -> Result<()> {
        self.outputs.begin_frame()
    }
    pub fn finish_hardware_frame(&mut self) -> Result<()> {
        let reports = self.outputs.finish_frame();
        self.reports(reports)
    }
    pub fn stop_keyboard(&mut self) {
        self.keyboard.stop();
    }
    pub fn release_held(&mut self) -> Result<()> {
        self.cleanup(true);
        let mut errors = Vec::new();
        if let Err(error) = self.input.release_held() {
            errors.push(format!("{error:#}"));
        }
        for (key, output) in self.outputs.take_analog() {
            if let Err(error) = self.analog(&key, &output, 0) {
                errors.push(format!("Analog raw-zero release: {error:#}"));
            }
        }
        if let Err(error) = self.finish_hardware_frame() {
            errors.push(format!("Hardware frame cleanup: {error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn reports(&mut self, reports: Vec<DriverInputReport>) -> Result<()> {
        let mut errors = Vec::new();
        let transition = self.outputs.take_gamepad_transition();
        let last = reports.len().checked_sub(1);
        for (index, report) in reports.into_iter().enumerate() {
            // changed_driver_inputs places the complete gamepad bank last.
            let observed = if Some(index) == last { transition } else { None };
            if let Err(error) = self.report_observed(report, observed) {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn report_observed(
        &mut self,
        report: DriverInputReport,
        transition: Option<(
            razer_device::mapping_native_output::GamepadOutputState,
            razer_device::mapping_native_output::GamepadOutputState,
        )>,
    ) -> Result<()> {
        let mut observation_error = None;
        let id = submit_observed(&mut self.output, report, || {
            if let Some((previous, current)) = transition {
                match razer_platform::event_clock::tick() {
                    Ok(tick) => self.gamepad_observation.observe(previous, current, tick),
                    Err(error) => observation_error = Some(error),
                }
            }
        })?;
        let mut canceled = false;
        loop {
            if !canceled && !self.cleanup && self.cancel.is_cancelled() {
                self.output.cancel(id)?;
                canceled = true;
            }
            match self.output.poll_completion(id, Duration::from_millis(5))? {
                CompletionPoll::Pending => {}
                CompletionPoll::Completed(result) => {
                    result?;
                    if let Some(error) = observation_error {
                        return Err(error.into());
                    }
                    return Ok(());
                }
            }
        }
    }
    pub fn close(&mut self) -> Result<()> {
        self.keyboard.stop();
        let mut errors = Vec::new();
        if let Err(error) = self.release_held() {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.input.close() {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.output.close() {
            errors.push(error.to_string());
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
impl Drop for Direct {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("Mapping direct-output cleanup: {error:#}");
        }
    }
}

// AFB14 queues output then immediately notifies, without waiting for the
// driver operation. Submission or completion failure does not invent readback.
fn submit_observed<T: MappingOutputTransport>(
    output: &mut T,
    report: DriverInputReport,
    notify: impl FnOnce(),
) -> Result<
    razer_device::mapping_native_output::SubmissionId,
    razer_device::mapping_native_output::DriverOutputError,
> {
    let submitted = output.submit(report);
    notify();
    submitted
}

#[cfg(test)]
mod gamepad_observation_tests {
    use super::*;
    use razer_device::mapping_native_output::{
        DriverInput, DriverOutputError, GamepadOutputState, SubmissionId, encode_driver_input,
    };
    use std::{
        cell::RefCell,
        sync::{atomic::AtomicBool, mpsc},
    };
    struct Transport<'a> {
        trace: &'a RefCell<Vec<&'static str>>,
        fail_submit: bool,
    }
    impl MappingOutputTransport for Transport<'_> {
        fn submit(&mut self, _: DriverInputReport) -> Result<SubmissionId, DriverOutputError> {
            self.trace.borrow_mut().push("submit");
            if self.fail_submit {
                Err(DriverOutputError::Closed)
            } else {
                Ok(SubmissionId(1))
            }
        }
        fn poll_completion(
            &mut self,
            _: SubmissionId,
            _: Duration,
        ) -> Result<CompletionPoll, DriverOutputError> {
            self.trace.borrow_mut().push("completion");
            Ok(CompletionPoll::Completed(Err(DriverOutputError::Closed)))
        }
        fn cancel(&mut self, _: SubmissionId) -> Result<(), DriverOutputError> {
            self.trace.borrow_mut().push("cancel");
            Ok(())
        }
        fn close(&mut self) -> Result<(), DriverOutputError> {
            self.trace.borrow_mut().push("close");
            Ok(())
        }
    }
    #[test]
    fn computed_sample_is_not_gated_by_submit_or_completion_success() {
        for fail_submit in [true, false] {
            let trace = RefCell::new(Vec::new());
            let mut output = Transport {
                trace: &trace,
                fail_submit,
            };
            let mut observation = Observation::default();
            let (sender, receiver) = mpsc::sync_channel(2);
            let epoch = uuid::Uuid::new_v4();
            observation
                .control(
                    Control::Register {
                        epoch,
                        registration: razer_ipc::gamepad_observation::Registration::Events {
                            skip: 50,
                        },
                    },
                    Sink {
                        events: sender,
                        overflow: Arc::new(AtomicBool::new(false)),
                    },
                )
                .unwrap();
            let state = GamepadOutputState {
                left_trigger: 64,
                ..Default::default()
            };
            let submitted = submit_observed(
                &mut output,
                encode_driver_input(DriverInput::Gamepad(state)),
                || {
                    trace.borrow_mut().push("notify");
                    observation.observe(GamepadOutputState::default(), state, 17);
                },
            );
            assert_eq!(*trace.borrow(), ["submit", "notify"]);
            assert!(
                matches!(receiver.try_recv().unwrap(),super::super::device::Event::Gamepad { observer,delivery } if observer == epoch && delivery.events() == [state] && delivery.time_tick() == 17)
            );
            if fail_submit {
                assert_eq!(submitted, Err(DriverOutputError::Closed));
            } else {
                assert_eq!(
                    output
                        .poll_completion(submitted.unwrap(), Duration::ZERO)
                        .unwrap(),
                    CompletionPoll::Completed(Err(DriverOutputError::Closed))
                );
                assert_eq!(*trace.borrow(), ["submit", "notify", "completion"]);
            }
        }
    }
}
