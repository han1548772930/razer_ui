//! Current analog Snap Tap middleware K/Q and reducers PFu/iwq.
use super::*;
use razer_device::mapping_native_lookup::InputKind;
use razer_ipc::input_observation::InputEvent;
use razer_ipc::mapping_observation::{SnapTapEvent, Target};
use razer_service::mapping::input_observation;
use razer_service::mapping::observation::{Reply, Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    SnapTap,
    RawInput,
}

enum Stream {
    SnapTap(Session),
    RawInput(input_observation::Session),
}

enum Event {
    SnapTap(SnapTapEvent),
    RawInput(InputEvent),
}

impl Stream {
    fn start(target: Target, mode: Mode) -> anyhow::Result<Self> {
        match mode {
            Mode::SnapTap => Session::<SnapTapEvent>::start(target).map(Self::SnapTap),
            Mode::RawInput => input_observation::Session::start(target).map(Self::RawInput),
        }
    }
    fn drain(&self) -> Vec<Reply<Event>> {
        fn map<E>(reply: Reply<E>, event: impl FnOnce(E) -> Event) -> Reply<Event> {
            match reply {
                Reply::Ready => Reply::Ready,
                Reply::Event(value) => Reply::Event(event(value)),
                Reply::Failed(error) => Reply::Failed(error),
            }
        }
        match self {
            Self::SnapTap(stream) => stream
                .drain()
                .into_iter()
                .map(|reply| map(reply, Event::SnapTap))
                .collect(),
            Self::RawInput(stream) => stream
                .drain()
                .into_iter()
                .map(|reply| map(reply, Event::RawInput))
                .collect(),
        }
    }
    fn is_finished(&self) -> bool {
        match self {
            Self::SnapTap(stream) => stream.is_finished(),
            Self::RawInput(stream) => stream.is_finished(),
        }
    }
    fn request_stop(&self) {
        match self {
            Self::SnapTap(stream) => stream.request_stop(),
            Self::RawInput(stream) => stream.request_stop(),
        }
    }
    fn stop(self) -> Result<(), String> {
        match self {
            Self::SnapTap(stream) => stream.stop(),
            Self::RawInput(stream) => stream.stop(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct PressedKey {
    #[serde(rename = "inputID")]
    input_id: String,
    snaptap: bool,
}

#[derive(Default)]
pub(in crate::features::keyboard_products) struct Observer {
    target: Option<Target>,
    active: bool,
    epoch: u64,
    mode: Option<Mode>,
    stream: Option<Stream>,
    pump: Option<Task<()>>,
    cleanup: Option<Task<()>>,
    error: Option<String>,
    pressed: Vec<PressedKey>,
}

fn reduce(pressed: &mut Vec<PressedKey>, pair: &Pair, input_id: &str, flag: u16, replayed: bool) {
    if input_id != pair.key1 && input_id != pair.key2 {
        return;
    }
    let existing = pressed.iter().position(|key| key.input_id == input_id);
    let released = matches!(flag, 1 | 3);
    if replayed {
        // PFu marks a physically held key as snapped; a replay cannot invent
        // a physical press absent from the ordinary callback stream.
        if let Some(index) = existing {
            pressed[index].snaptap = released;
        }
    } else if released {
        if let Some(index) = existing {
            pressed.remove(index);
        }
    } else if let Some(index) = existing {
        pressed[index].snaptap = false;
    } else {
        pressed.push(PressedKey {
            input_id: input_id.into(),
            snaptap: false,
        });
    }
}

fn reduce_raw(
    pressed: &mut Vec<PressedKey>,
    pair: &Pair,
    enabled: bool,
    input_id: &str,
    flag: u16,
) -> bool {
    if enabled || (input_id != pair.key1 && input_id != pair.key2) {
        return false;
    }
    let existing = pressed.iter().position(|key| key.input_id == input_id);
    if flag % 2 == 0 {
        if existing.is_none() {
            pressed.push(PressedKey {
                input_id: input_id.into(),
                snaptap: false,
            });
        }
    } else if let Some(index) = existing {
        pressed.remove(index);
    }
    true
}

fn reduce_raw_event(
    pressed: &mut Vec<PressedKey>,
    pair: &Pair,
    enabled: bool,
    event: &InputEvent,
) -> bool {
    // Q forwards flag/scancode without a type gate. Only these typed
    // input variants carry those fields; all others are unobserved by iwq.
    let (InputKind::Keyboard { scancode, flag } | InputKind::AnalogKey { scancode, flag }) =
        event.input().kind
    else {
        return false;
    };
    let Some(input_id) = analog_input_id(scancode) else {
        return false;
    };
    reduce_raw(pressed, pair, enabled, input_id, flag)
}

impl KeyboardProductWorkspace {
    pub fn set_snap_observation_target(
        &mut self,
        target: Option<Target>,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if self.snap_observer.target != target || self.snap_observer.active != active {
            self.stop_snap_observation(cx);
            self.snap_observer.target = target;
            self.snap_observer.active = active;
            self.snap_observer.error = None;
        }
        self.sync_snap_observation(cx);
    }

    pub(in crate::features::keyboard_products) fn stop_snap_observation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.stop_snap_stream(cx);
        self.snap_observer.pressed.clear();
        if let Some(state) = &mut self.snap_tap {
            state.observed_pressed = json!([]);
            state.pressed = [0; 2];
        }
    }

    fn stop_snap_stream(&mut self, cx: &mut Context<Self>) {
        self.snap_observer.epoch = self.snap_observer.epoch.wrapping_add(1);
        self.snap_observer.pump = None;
        if let Some(stream) = self.snap_observer.stream.take() {
            stream.request_stop();
            let epoch = self.snap_observer.epoch;
            let target = self.snap_observer.target.clone();
            let cleanup = cx.background_executor().spawn(async move { stream.stop() });
            self.snap_observer.cleanup = Some(cx.spawn(async move |view, cx| {
                let result = cleanup.await;
                if let Err(error) = &result {
                    eprintln!("[Snap Tap observation cleanup] {error}");
                }
                let _ = view.update(cx, |this, cx| {
                    this.snap_observer.cleanup = None;
                    if this.snap_observer.epoch == epoch && this.snap_observer.target == target {
                        if let Err(error) = result {
                            this.snap_observation_failed(error, cx);
                        }
                    }
                    this.sync_snap_observation(cx);
                });
            }));
        }
    }

    pub(super) fn leave_snap_observation(&mut self, cx: &mut Context<Self>) {
        self.snap_observer.active = false;
        self.stop_snap_observation(cx);
    }

    pub(super) fn reset_snap_observation(&mut self, cx: &mut Context<Self>) {
        self.stop_snap_observation(cx);
        self.snap_observer.error = None;
    }

    pub(in crate::features::keyboard_products) fn sync_snap_observation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let observing = self.spec.product_id == 679
            && self.snap_observer.active
            && self.page == "ACTUATION"
            && self.snap_tap.as_ref().is_some_and(|state| state.analog);
        let mode = observing.then(|| {
            if self.snap_tap.as_ref().unwrap().config.enabled {
                Mode::SnapTap
            } else {
                Mode::RawInput
            }
        });
        if self.snap_observer.mode != mode {
            self.snap_observer.mode = mode;
            self.snap_observer.error = None;
            // jm's effect changes the subscription when isEnabled changes.
            // PFu/iwq share pressedKeys; this transition does not reset it.
            self.stop_snap_stream(cx);
        }
        let Some(mode) = mode else {
            self.stop_snap_observation(cx);
            return;
        };
        if self.snap_observer.target.is_none() {
            if self.snap_observer.stream.is_some() {
                self.stop_snap_observation(cx);
            }
            return;
        }
        if self.snap_observer.stream.is_some()
            || self.snap_observer.cleanup.is_some()
            || self.snap_observer.error.is_some()
        {
            return;
        }
        // PID679 owns its mapping registration asynchronously. The Ready
        // callback resumes this subscription; cached profile data cannot prove readiness.
        if self.spec.product_id == 679 && !self.snap_mapping_ready() {
            return;
        }
        let target = self.snap_observer.target.clone().unwrap();
        match Stream::start(target, mode) {
            Ok(stream) => self.snap_observer.stream = Some(stream),
            Err(error) => {
                self.snap_observation_failed(format!("{error:#}"), cx);
                return;
            }
        }
        let epoch = self.snap_observer.epoch;
        self.snap_observer.pump = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if !view
                    .update(cx, |this, cx| {
                        if this.snap_observer.epoch != epoch {
                            return false;
                        }
                        let Some(stream) = &this.snap_observer.stream else {
                            return false;
                        };
                        let replies = stream.drain();
                        let finished = stream.is_finished();
                        for reply in replies {
                            match reply {
                                Reply::Ready => {}
                                Reply::Event(Event::SnapTap(event)) => {
                                    this.consume_snap_observation(event, cx)
                                }
                                Reply::Event(Event::RawInput(event)) => {
                                    this.consume_raw_observation(event, cx)
                                }
                                Reply::Failed(error) => this.snap_observation_failed(error, cx),
                            }
                        }
                        if finished {
                            // Joining on the background executor preserves even a
                            // terminal overflow error that could not enter the queue.
                            this.stop_snap_observation(cx);
                            return false;
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

    fn snap_observation_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.snap_observer.error = Some(error.clone());
        cx.emit(super::super::legacy_mapping::KeyboardLegacyRuntimeStatus(
            format!("Snap Tap：{error}"),
        ));
        cx.notify();
    }

    fn consume_snap_observation(&mut self, event: SnapTapEvent, cx: &mut Context<Self>) {
        let InputKind::AnalogKey { scancode, flag } = event.input().kind else {
            return;
        };
        let Some(input_id) = analog_input_id(scancode) else {
            return;
        };
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        let Some(pair) = state.config.pairs.first() else {
            return;
        };
        reduce(
            &mut self.snap_observer.pressed,
            pair,
            input_id,
            flag,
            event.replayed(),
        );
        self.publish_snap_pressed(cx);
    }

    fn consume_raw_observation(&mut self, event: InputEvent, cx: &mut Context<Self>) {
        let Some(state) = &self.snap_tap else {
            return;
        };
        let Some(pair) = state.config.pairs.first() else {
            return;
        };
        if reduce_raw_event(
            &mut self.snap_observer.pressed,
            pair,
            state.config.enabled,
            &event,
        ) {
            self.publish_snap_pressed(cx);
        }
    }

    fn publish_snap_pressed(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        state.observed_pressed = json!(self.snap_observer.pressed);
        if let Some(pair) = state.staged.first() {
            for (index, input_id) in [&pair.key1, &pair.key2].into_iter().enumerate() {
                state.pressed[index] = self
                    .snap_observer
                    .pressed
                    .iter()
                    .find(|key| &key.input_id == input_id)
                    .map_or(0, |key| if key.snaptap { 2 } else { 1 });
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn observed_snap_release_retains_physical_key_until_ordinary_release() {
        let pair = Pair {
            key1: "KEY_A".into(),
            key2: "KEY_D".into(),
            id: 1,
            extra: Default::default(),
        };
        let mut pressed = Vec::new();
        reduce(&mut pressed, &pair, "KEY_A", 0, true);
        assert!(pressed.is_empty());
        reduce(&mut pressed, &pair, "KEY_A", 0, false);
        reduce(&mut pressed, &pair, "KEY_D", 0, false);
        reduce(&mut pressed, &pair, "KEY_A", 1, true);
        assert_eq!(
            pressed,
            vec![
                PressedKey {
                    input_id: "KEY_A".into(),
                    snaptap: true
                },
                PressedKey {
                    input_id: "KEY_D".into(),
                    snaptap: false
                }
            ]
        );
        reduce(&mut pressed, &pair, "KEY_A", 0, true);
        assert!(!pressed[0].snaptap);
        reduce(&mut pressed, &pair, "KEY_A", 1, false);
        assert_eq!(pressed.len(), 1);
        reduce(&mut pressed, &pair, "KEY_W", 0, false);
        assert_eq!(pressed[0].input_id, "KEY_D");
    }

    #[::core::prelude::v1::test]
    fn disabled_raw_input_tracks_both_keys_and_uses_flag_parity() {
        let pair = Pair {
            key1: "KEY_A".into(),
            key2: "KEY_D".into(),
            id: 1,
            extra: Default::default(),
        };
        let mut pressed = Vec::new();
        reduce_raw(&mut pressed, &pair, false, "KEY_A", 0);
        reduce_raw(&mut pressed, &pair, false, "KEY_A", 2);
        reduce_raw(&mut pressed, &pair, false, "KEY_D", 4);
        reduce_raw(&mut pressed, &pair, false, "KEY_W", 0);
        assert_eq!(
            pressed,
            vec![
                PressedKey {
                    input_id: "KEY_A".into(),
                    snaptap: false
                },
                PressedKey {
                    input_id: "KEY_D".into(),
                    snaptap: false
                },
            ]
        );
        reduce_raw(&mut pressed, &pair, false, "KEY_A", 3);
        assert_eq!(pressed[0].input_id, "KEY_D");
        reduce_raw(&mut pressed, &pair, false, "KEY_D", 5);
        assert!(pressed.is_empty());
    }

    #[::core::prelude::v1::test]
    fn enabled_and_raw_callbacks_share_observed_keys_across_toggle() {
        let pair = Pair {
            key1: "KEY_A".into(),
            key2: "KEY_D".into(),
            id: 1,
            extra: Default::default(),
        };
        let mut pressed = Vec::new();
        reduce_raw(&mut pressed, &pair, true, "KEY_A", 0);
        assert!(pressed.is_empty());
        reduce(&mut pressed, &pair, "KEY_A", 0, false);
        reduce(&mut pressed, &pair, "KEY_A", 1, true);
        assert!(pressed[0].snaptap);
        // iwq does not rewrite a previously held key on a duplicate press.
        reduce_raw(&mut pressed, &pair, false, "KEY_A", 2);
        assert!(pressed[0].snaptap);
        reduce_raw(&mut pressed, &pair, false, "KEY_A", 1);
        assert!(pressed.is_empty());
        reduce_raw(&mut pressed, &pair, false, "KEY_D", 0);
        reduce(&mut pressed, &pair, "KEY_D", 1, true);
        assert!(pressed[0].snaptap);
        reduce(&mut pressed, &pair, "KEY_D", 1, false);
        assert!(pressed.is_empty());
    }

    #[::core::prelude::v1::test]
    fn raw_input_lookup_keeps_the_source_first_scancode_match() {
        assert_eq!(analog_input_id(30), Some("KEY_A"));
        assert_eq!(analog_input_id(32), Some("KEY_D"));
        // UI 15032.nt delegates to 46114.Rk.find(scancode), without flag.
        assert_eq!(analog_input_id(72), Some("KEY_NUMPAD_8"));
        assert_eq!(analog_input_id(u16::MAX), None);
    }

    #[::core::prelude::v1::test]
    fn typed_raw_input_reaches_the_disabled_page_reducer_without_inferred_depth() {
        use razer_device::mapping_native_lookup::Input;
        let pair = Pair {
            key1: "KEY_A".into(),
            key2: "KEY_D".into(),
            id: 1,
            extra: Default::default(),
        };
        let mut pressed = Vec::new();
        let event = |kind| InputEvent::new(Input::new(kind), 91);
        assert!(reduce_raw_event(
            &mut pressed,
            &pair,
            false,
            &event(InputKind::Keyboard {
                scancode: 30,
                flag: 2
            })
        ));
        assert_eq!(pressed[0].input_id, "KEY_A");
        assert!(!pressed[0].snaptap);
        assert!(!reduce_raw_event(
            &mut pressed,
            &pair,
            false,
            &event(InputKind::Mouse { id: 1, data: 0 })
        ));
        assert!(!reduce_raw_event(
            &mut pressed,
            &pair,
            false,
            &event(InputKind::Keyboard {
                scancode: u16::MAX,
                flag: 1
            })
        ));
        assert!(reduce_raw_event(
            &mut pressed,
            &pair,
            false,
            &event(InputKind::AnalogKey {
                scancode: 30,
                flag: 3
            })
        ));
        assert!(pressed.is_empty());
    }
}
