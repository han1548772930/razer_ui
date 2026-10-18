//! PID679 middleware H/G and the mounted actuation slider's analog callback.
use super::*;
use razer_device::{
    macro_runtime::Event as OutputEvent,
    mapping_native_action::Output,
    mapping_native_digital::NativeDigitalOutput,
    mapping_native_lookup::{Input, InputKind},
};
use razer_ipc::analog_observation::{AnalogEvent, Delivery, Registration, Target};
use razer_service::mapping::analog_observation::{Reply, Session};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Color {
    #[default]
    Grey,
    Green,
    Red,
    Undefined,
}
impl Color {
    fn paint(self) -> Hsla {
        use razer_widgets::theme::KeyboardActuationColors as Colors;
        match self {
            Self::Grey | Self::Undefined => Colors::sensor_idle(),
            Self::Green => Colors::sensor_active(),
            Self::Red => Colors::sensor_release(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sample {
    level: u16,
    color: Color,
}
#[derive(Default)]
struct Feedback {
    held: Option<u8>,
    color: Color,
    pending: Option<Sample>,
    last_call: Option<Duration>,
    last_invoke: Duration,
    deadline: Option<Duration>,
    dirty: bool,
    observed: Option<Sample>,
}
impl Feedback {
    fn consume(&mut self, event: &AnalogEvent, input_id: &str, now: Duration) {
        let InputKind::AnalogKey { scancode, .. } = event.input().kind else {
            return;
        };
        if event.key_id() == 59 {
            return;
        }
        let held = self.held.get_or_insert(event.key_id());
        if *held != event.key_id() {
            return;
        }
        if let Some((output_scancode, flag)) = event.output().and_then(output_flag) {
            let extended =
                special_key(input_id) == Some(event.key_id()) && output_scancode == Some(scancode);
            let (press, release) = if extended {
                (2, 3)
            } else if event.key_id() == 126 {
                (4, 5)
            } else {
                (0, 1)
            };
            self.color = if flag == press {
                Color::Green
            } else if flag == release {
                Color::Red
            } else if flag == 7 {
                Color::Grey
            } else {
                Color::Undefined
            };
        }
        self.schedule(
            Sample {
                level: event.level(),
                color: self.color,
            },
            now,
        );
        if event.level() == 0 {
            self.held = None;
            self.color = Color::Grey;
            self.schedule(
                Sample {
                    level: 0,
                    color: Color::Grey,
                },
                now,
            );
        }
    }

    fn due(&self, now: Duration) -> bool {
        self.last_call
            .is_none_or(|last| now.saturating_sub(last) >= Duration::from_millis(16))
            || now.saturating_sub(self.last_invoke) >= Duration::from_millis(16)
    }
    fn publish(&mut self, now: Duration) {
        self.last_invoke = now;
        if let Some(sample) = self.pending.take() {
            self.observed = Some(sample);
            self.dirty = true;
        }
    }
    fn schedule(&mut self, sample: Sample, now: Duration) {
        // MW module78901 is lodash throttle: leading and trailing, maxWait16.
        let due = self.due(now);
        self.last_call = Some(now);
        self.pending = Some(sample);
        if due {
            self.deadline = Some(now + Duration::from_millis(16));
            self.publish(now);
        } else if self.deadline.is_none() {
            self.deadline = Some(now + Duration::from_millis(16));
        }
    }
    fn flush(&mut self, now: Duration) -> bool {
        if self.deadline.is_some_and(|deadline| deadline <= now) {
            if self.due(now) {
                self.deadline = None;
                if self.pending.is_some() {
                    self.publish(now);
                }
            } else {
                let wait = Duration::from_millis(16);
                let since_call = now.saturating_sub(self.last_call.unwrap());
                let since_invoke = now.saturating_sub(self.last_invoke);
                self.deadline = Some(now + (wait - since_call).min(wait - since_invoke));
            }
        }
        std::mem::take(&mut self.dirty)
    }
}
fn special_key(input: &str) -> Option<u8> {
    Some(match input {
        "KEY_LEFT_ALT" => 62,
        "KEY_APPLICATION" => 129,
        "KEY_LEFT_GUI" => 127,
        "KEY_PAUSE" => 64,
        "KEY_NUMPAD_0" => 75,
        "KEY_NUMPAD_PERIOD" => 76,
        "KEY_NUMPAD_1" => 81,
        "KEY_NUMPAD_7" => 80,
        "KEY_NUMPAD_9" => 85,
        "KEY_NUMPAD_3" => 86,
        "KEY_NUMPAD_4" => 79,
        "KEY_NUMPAD_8" => 83,
        "KEY_NUMPAD_6" => 89,
        "KEY_NUMPAD_2" => 84,
        "KEY_NUMPAD_ASTERISK" => 124,
        "KEY_NUMPAD_SLASH" => 95,
        "KEY_NUMPAD_ENTER" => 108,
        _ => return None,
    })
}
fn output_flag(output: &Output) -> Option<(Option<u16>, u32)> {
    match output {
        Output::Event(OutputEvent::Keyboard(key)) => Some((Some(key.scancode), key.flag.into())),
        Output::Macro { flag, .. }
        | Output::PhasedMacro { flag, .. }
        | Output::Turbo { flag, .. }
        | Output::Hypershift { flag }
        | Output::Otfs { flag } => Some((None, *flag)),
        Output::Mode { flag, .. } => Some((None, (*flag).into())),
        Output::Digital(
            NativeDigitalOutput::JoystickButton { flag, .. }
            | NativeDigitalOutput::GamepadButtons { flag, .. },
        ) => Some((None, (*flag).into())),
        Output::Forwarded { payload, .. } | Output::CopyText(payload) => {
            #[derive(Deserialize)]
            struct Fields {
                flag: Option<u32>,
                scancode: Option<u16>,
            }
            let fields: Fields = serde_json::from_str(payload.as_json()).ok()?;
            fields.flag.map(|flag| (fields.scancode, flag))
        }
        _ => None,
    }
}
#[derive(Default)]
pub(super) struct Observer {
    target: Option<Target>,
    active: bool,
    epoch: u64,
    stream: Option<Session>,
    pump: Option<Task<()>>,
    cleanup: Option<Task<()>>,
    error: Option<String>,
    feedback: Feedback,
    mode: Option<Mode>,
    level: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    All,
    Key(Input),
}
impl Mode {
    fn registration(&self) -> Registration {
        match self {
            Self::All => Registration::All { skip: 25 },
            Self::Key(input) => Registration::Key {
                input: input.clone(),
                skip: 50,
            },
        }
    }
    fn selected_level(&self, event: &AnalogEvent) -> Option<u16> {
        let Self::Key(input) = self else { return None };
        match (&input.kind, &event.input().kind) {
            (
                InputKind::AnalogKey {
                    scancode: a,
                    flag: af,
                },
                InputKind::AnalogKey {
                    scancode: b,
                    flag: bf,
                },
            ) if a == b && af & !1 == bf & !1 => Some(event.level()),
            _ => None,
        }
    }
}
impl KeyboardProductWorkspace {
    pub fn set_analog_observation_target(
        &mut self,
        target: Option<Target>,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if self.analog_observer.target != target || self.analog_observer.active != active {
            self.stop_analog_observation(cx);
            self.analog_observer.target = target;
            self.analog_observer.active = active;
            self.analog_observer.error = None;
        }
        self.sync_analog_observation(cx);
    }
    pub(in crate::features::keyboard_products) fn stop_analog_observation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.analog_observer.epoch = self.analog_observer.epoch.wrapping_add(1);
        self.analog_observer.pump = None;
        self.analog_observer.feedback = Feedback::default();
        self.analog_observer.level = None;
        if let Some(stream) = self.analog_observer.stream.take() {
            stream.request_stop();
            let epoch = self.analog_observer.epoch;
            let target = self.analog_observer.target.clone();
            let cleanup = cx.background_executor().spawn(async move { stream.stop() });
            self.analog_observer.cleanup = Some(cx.spawn(async move |view, cx| {
                let result = cleanup.await;
                if let Err(error) = &result {
                    eprintln!("[Analog observation cleanup] {error}");
                }
                let _ = view.update(cx, |this, cx| {
                    this.analog_observer.cleanup = None;
                    if this.analog_observer.epoch == epoch && this.analog_observer.target == target
                    {
                        if let Err(error) = result {
                            this.analog_observation_failed(error, cx);
                        }
                    }
                    this.sync_analog_observation(cx);
                });
            }));
        }
        cx.notify();
    }
    pub fn leave_analog_observation(&mut self, cx: &mut Context<Self>) {
        self.analog_observer.active = false;
        self.stop_analog_observation(cx);
    }
    pub(super) fn sync_analog_observation(&mut self, cx: &mut Context<Self>) {
        let mode = if self.spec.product_id != 679
            || !self.analog_observer.active
            || self.analog_observer.target.is_none()
        {
            None
        } else if self.page == "ACTUATION" {
            Some(Mode::All)
        } else if let Some(input_id) = self.mapping_analog_input_id() {
            match snap_tap::analog_input(input_id) {
                Ok(input) => Some(Mode::Key(input)),
                Err(error) => {
                    self.stop_analog_observation(cx);
                    self.analog_observation_failed(format!("{error:#}"), cx);
                    return;
                }
            }
        } else {
            None
        };
        if self.analog_observer.mode != mode {
            self.stop_analog_observation(cx);
            self.analog_observer.mode = mode;
            self.analog_observer.error = None;
        }
        let Some(mode) = self.analog_observer.mode.clone() else {
            return;
        };
        if self.analog_observer.stream.is_some()
            || self.analog_observer.cleanup.is_some()
            || self.analog_observer.error.is_some()
            || !self.snap_mapping_ready()
        {
            return;
        }
        // MW14486 K/Q: PID679 has no 8 kHz flag or interval option.
        // The actuation page observes all keys; a mounted controller graph
        // observes its selected input only, after old registration cleanup.
        let target = self.analog_observer.target.clone().unwrap();
        match Session::start(target, mode.registration()) {
            Ok(stream) => self.analog_observer.stream = Some(stream),
            Err(error) => {
                self.analog_observation_failed(format!("{error:#}"), cx);
                return;
            }
        }
        let epoch = self.analog_observer.epoch;
        let began = Instant::now();
        self.analog_observer.pump = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(4))
                    .await;
                if !view
                    .update(cx, |this, cx| {
                        if this.analog_observer.epoch != epoch {
                            return false;
                        }
                        let Some(stream) = &this.analog_observer.stream else {
                            return false;
                        };
                        let replies = stream.drain();
                        let finished = stream.is_finished();
                        for reply in replies {
                            match reply {
                                Reply::Ready => {}
                                Reply::Event(delivery) => {
                                    this.consume_analog_observation(delivery, began.elapsed(), cx)
                                }
                                Reply::Failed(error) => this.analog_observation_failed(error, cx),
                            }
                        }
                        if finished || this.analog_observer.error.is_some() {
                            this.stop_analog_observation(cx);
                            return false;
                        }
                        if this.analog_observer.feedback.flush(began.elapsed()) {
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }
    fn analog_observation_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.analog_observer.error = Some(error.clone());
        self.analog_observer.feedback = Feedback::default();
        self.analog_observer.level = None;
        cx.emit(KeyboardLegacyRuntimeStatus(format!(
            "Analog observation: {error}"
        )));
        cx.notify();
    }
    fn consume_analog_observation(
        &mut self,
        delivery: Delivery,
        now: Duration,
        cx: &mut Context<Self>,
    ) {
        if let Some(mode @ Mode::Key(_)) = &self.analog_observer.mode {
            let previous = self.analog_observer.level;
            for event in delivery.events() {
                if let Some(level) = mode.selected_level(event) {
                    self.analog_observer.level = Some(level);
                }
            }
            if previous != self.analog_observer.level {
                cx.notify();
            }
            return;
        }
        for event in delivery.events() {
            let InputKind::AnalogKey { scancode, .. } = event.input().kind else {
                continue;
            };
            let Some(input_id) = snap_tap::analog_input_id(scancode) else {
                continue;
            };
            self.analog_observer.feedback.consume(event, input_id, now);
        }
    }
    pub(in crate::features::keyboard_products) fn mapping_analog_level(&self) -> Option<u16> {
        self.analog_observer.level
    }
    pub(super) fn analog_feedback(&self) -> Option<(f32, Hsla)> {
        self.analog_observer
            .feedback
            .observed
            .as_ref()
            .map(|sample| (f32::from(sample.level) / 65535., sample.color.paint()))
    }
    pub(super) fn analog_actuation_slider(
        &self,
        slider: &Entity<SliderState>,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
        use razer_widgets::theme::KeyboardActuationColors as Colors;
        let observed = self.analog_feedback();
        let color = observed.map_or_else(Colors::sensor_idle, |(_, color)| color);
        let position = slider.read(cx).percentage().end;
        // The source's 90-degree rotation puts zero at the top. The editor's
        // GPUI slider value is mirrored; observed physical depth is independent.
        BaseSlider::new(slider)
            .vertical()
            .disabled(disabled)
            .flex()
            .items_center()
            .justify_center()
            .w(surface::css(70.))
            .h(surface::css(204.))
            .opacity(if disabled { 0.3 } else { 1. })
            .child(
                SliderTrack::new(slider)
                    .axis(Axis::Vertical)
                    .disabled(disabled)
                    .flex()
                    .justify_center()
                    .w(surface::css(24.))
                    .h_full()
                    .child(
                        SliderIndicator::new(slider)
                            .relative()
                            .h_full()
                            .w(surface::css(6.))
                            .rounded(surface::css(3.))
                            .bg(Colors::sensor_track())
                            .when_some(observed, |track, (level, color)| {
                                track.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .w_full()
                                        .h(relative(level))
                                        .rounded(surface::css(3.))
                                        .bg(color),
                                )
                            })
                            .child(
                                SliderThumb::new(slider)
                                    .axis(Axis::Vertical)
                                    .disabled(disabled)
                                    .absolute()
                                    .bottom(relative(position))
                                    .left(surface::css(-5.))
                                    .mb(surface::css(-8.))
                                    .size(surface::css(16.))
                                    .rounded_full()
                                    .bg(color)
                                    .when(!disabled, |thumb| {
                                        thumb.hover(|thumb| {
                                            thumb
                                                .bg(Colors::sync_border())
                                                .border_2()
                                                .border_color(Colors::sensor_active())
                                        })
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::mapping_native_lookup::Input;
    fn event(key: u8, scancode: u16, level: u16, output: Option<Output>) -> AnalogEvent {
        AnalogEvent::new(
            Input::new(InputKind::AnalogKey { scancode, flag: 0 }),
            key,
            level,
            output,
            0,
        )
    }
    fn keyboard(scancode: u16, flag: u16) -> Output {
        Output::Event(OutputEvent::Keyboard(
            razer_device::macro_runtime::Keyboard {
                scancode,
                flag,
                modifiers: 0,
                unit: None,
                delay_ms: None,
                release_after_ms: None,
            },
        ))
    }
    #[::core::prelude::v1::test]
    fn analog_mapping_observation_uses_selected_input_and_source_registration() {
        let up = snap_tap::analog_input("KEY_UP_ARROW").unwrap();
        assert_eq!(
            up.kind,
            InputKind::AnalogKey {
                scancode: 72,
                flag: 2
            }
        );
        let mode = Mode::Key(up.clone());
        assert_eq!(
            mode.registration(),
            Registration::Key {
                input: up,
                skip: 50
            }
        );
        assert_eq!(Mode::All.registration(), Registration::All { skip: 25 });
        assert!(snap_tap::analog_input("UNKNOWN_KEY").is_err());
        // NumPad8 shares the scan code but is a different physical input.
        assert_eq!(mode.selected_level(&event(83, 72, 40000, None)), None);
        let observed = AnalogEvent::new(
            Input::new(InputKind::AnalogKey {
                scancode: 72,
                flag: 3,
            }),
            83,
            12345,
            None,
            1,
        );
        assert_eq!(mode.selected_level(&observed), Some(12345));
        assert_eq!(Mode::All.selected_level(&observed), None);
        let zero = AnalogEvent::new(
            Input::new(InputKind::AnalogKey {
                scancode: 72,
                flag: 2,
            }),
            83,
            0,
            None,
            2,
        );
        assert_eq!(mode.selected_level(&zero), Some(0));
    }
    #[::core::prelude::v1::test]
    fn analog_feedback_uses_the_real_source_input_table() {
        let product = source_product(679).unwrap();
        assert!(product.keys.iter().any(|key| key["inputID"] == "KEY_A"));
        assert_eq!(snap_tap::analog_input_id(30), Some("KEY_A"));
        let mut state = Feedback::default();
        let callback = event(31, 30, 32000, Some(keyboard(30, 0)));
        state.consume(
            &callback,
            snap_tap::analog_input_id(30).unwrap(),
            Duration::ZERO,
        );
        assert_eq!(
            state.observed,
            Some(Sample {
                level: 32000,
                color: Color::Green
            })
        );
    }
    #[::core::prelude::v1::test]
    fn analog_feedback_throttle_has_leading_trailing_and_bounded_continuous_updates() {
        let mut state = Feedback::default();
        state.consume(&event(31, 30, 1, None), "KEY_A", Duration::ZERO);
        assert_eq!(state.observed.as_ref().unwrap().level, 1);
        assert!(state.flush(Duration::ZERO));
        for tick in 1..16 {
            state.consume(
                &event(31, 30, tick, None),
                "KEY_A",
                Duration::from_millis(tick.into()),
            );
            assert!(!state.flush(Duration::from_millis(tick.into())));
        }
        assert!(state.flush(Duration::from_millis(16)));
        assert_eq!(state.observed.as_ref().unwrap().level, 15);
        state.consume(&event(31, 30, 32, None), "KEY_A", Duration::from_millis(32));
        assert_eq!(state.observed.as_ref().unwrap().level, 32);
        assert!(state.flush(Duration::from_millis(32)));
        assert!(!state.flush(Duration::from_millis(48)));
    }
    #[::core::prelude::v1::test]
    fn analog_feedback_holds_first_key_until_zero_and_preserves_color_on_raw_samples() {
        let mut state = Feedback::default();
        state.consume(
            &event(31, 30, 100, Some(keyboard(30, 0))),
            "KEY_A",
            Duration::ZERO,
        );
        assert_eq!(
            state.observed,
            Some(Sample {
                level: 100,
                color: Color::Green
            })
        );
        state.consume(
            &event(32, 32, 500, Some(keyboard(32, 1))),
            "KEY_D",
            Duration::from_millis(20),
        );
        assert_eq!(state.observed.as_ref().unwrap().level, 100);
        state.consume(
            &event(31, 30, 200, None),
            "KEY_A",
            Duration::from_millis(20),
        );
        assert_eq!(
            state.observed,
            Some(Sample {
                level: 200,
                color: Color::Green
            })
        );
        state.consume(
            &event(31, 30, 0, Some(keyboard(30, 1))),
            "KEY_A",
            Duration::from_millis(40),
        );
        assert_eq!(state.observed.as_ref().unwrap().color, Color::Red);
        state.flush(Duration::from_millis(56));
        assert_eq!(
            state.observed,
            Some(Sample {
                level: 0,
                color: Color::Grey
            })
        );
        assert_eq!(state.held, None);
        state.consume(
            &event(32, 32, 500, Some(keyboard(32, 1))),
            "KEY_D",
            Duration::from_millis(60),
        );
        assert_eq!(
            state.observed,
            Some(Sample {
                level: 500,
                color: Color::Red
            })
        );
    }
    #[::core::prelude::v1::test]
    fn analog_feedback_extended_and_special_flags_match_source() {
        let mut state = Feedback::default();
        state.consume(
            &event(62, 56, 100, Some(keyboard(56, 2))),
            "KEY_LEFT_ALT",
            Duration::ZERO,
        );
        assert_eq!(state.observed.as_ref().unwrap().color, Color::Green);
        state.consume(
            &event(62, 56, 200, Some(keyboard(56, 3))),
            "KEY_LEFT_ALT",
            Duration::from_millis(20),
        );
        assert_eq!(state.observed.as_ref().unwrap().color, Color::Red);
        state.consume(
            &event(62, 56, 0, None),
            "KEY_LEFT_ALT",
            Duration::from_millis(40),
        );
        state.consume(
            &event(126, 99, 100, Some(keyboard(99, 4))),
            "KEY_TEST",
            Duration::from_millis(60),
        );
        assert_eq!(state.observed.as_ref().unwrap().color, Color::Green);
        state.consume(
            &event(126, 99, 100, Some(keyboard(99, 9))),
            "KEY_TEST",
            Duration::from_millis(80),
        );
        assert_eq!(state.observed.as_ref().unwrap().color, Color::Undefined);
        let mut ignored = Feedback::default();
        ignored.consume(&event(59, 1, 100, None), "KEY_ESCAPE", Duration::ZERO);
        assert!(ignored.observed.is_none());
    }
}
