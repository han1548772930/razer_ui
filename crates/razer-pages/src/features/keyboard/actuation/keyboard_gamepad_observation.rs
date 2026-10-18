//! PID679 MW14486 output callbacks and the Customize gamepad tester.
use super::*;
use razer_device::mapping_native_output::GamepadOutputState;
use razer_ipc::gamepad_observation::{Delivery, Registration, Target};
use razer_service::mapping::gamepad_observation::{Reply, Session};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// The source replaces an older array playback when a newer callback arrives.
#[derive(Default)]
struct Playback {
    pending: VecDeque<GamepadOutputState>,
    next: Option<Duration>,
    observed: Option<KeyboardGamepadTesterObservation>,
}
impl Playback {
    fn replace(&mut self, events: &[GamepadOutputState], now: Duration) -> bool {
        self.pending = events.iter().cloned().collect();
        self.next = (!self.pending.is_empty()).then_some(now);
        self.advance(now)
    }

    fn advance(&mut self, now: Duration) -> bool {
        if !self.next.is_some_and(|next| next <= now) {
            return false;
        }
        let Some(event) = self.pending.pop_front() else {
            self.next = None;
            return false;
        };
        let sample = KeyboardGamepadTesterObservation {
            left_thumb_x: event.left_thumb[0],
            left_thumb_y: event.left_thumb[1],
            right_thumb_x: event.right_thumb[0],
            right_thumb_y: event.right_thumb[1],
            left_trigger: event.left_trigger,
            right_trigger: event.right_trigger,
        };
        let changed = self.observed != Some(sample);
        self.observed = Some(sample);
        self.next = (!self.pending.is_empty()).then_some(now + Duration::from_millis(5));
        changed
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    active_profile: String,
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    guid: String,
    mappings: Vec<Mapping>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mapping {
    input_type: String,
    mapping: Option<Vec<Assignment>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Assignment {
    controller_group: Option<Controller>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Controller {
    controller_assignment: String,
}

fn source_mapping_enabled(document: &Value) -> Result<bool, String> {
    let document: Document = serde_json::from_value(document.clone())
        .map_err(|error| format!("Gamepad observation profile: {error}"))?;
    let mut selected = document
        .profiles
        .iter()
        .filter(|profile| profile.guid == document.active_profile);
    let profile = selected
        .next()
        .ok_or_else(|| "Gamepad observation active profile is absent".to_owned())?;
    if selected.next().is_some() {
        return Err("Gamepad observation active profile is repeated".into());
    }
    let mut enabled = false;
    for mapping in &profile.mappings {
        if mapping.input_type != "AnalogInput" {
            continue;
        }
        let assignments = mapping
            .mapping
            .as_ref()
            .ok_or_else(|| "Gamepad observation AnalogInput mapping is absent".to_owned())?;
        // MW52418.$T is the ten analog controller assignments. Digital
        // buttons and joystickGroup mappings do not register this consumer.
        enabled |= assignments
            .iter()
            .filter_map(|entry| entry.controller_group.as_ref())
            .any(|controller| {
                matches!(
                    controller.controller_assignment.as_str(),
                    "LEFTTRIGGER"
                        | "RIGHTTRIGGER"
                        | "LEFT_JS_UP"
                        | "LEFT_JS_DOWN"
                        | "LEFT_JS_LEFT"
                        | "LEFT_JS_RIGHT"
                        | "RIGHT_JS_UP"
                        | "RIGHT_JS_DOWN"
                        | "RIGHT_JS_LEFT"
                        | "RIGHT_JS_RIGHT"
                )
            });
    }
    Ok(enabled)
}

#[derive(Default)]
struct Lifecycle {
    target: Option<Target>,
    active: bool,
    mapping_enabled: bool,
    epoch: u64,
    error: Option<String>,
    cleanup_error: Option<String>,
    playback: Playback,
}
impl Lifecycle {
    fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.playback = Playback::default();
    }
    fn retarget(&mut self, target: Option<Target>, active: bool) -> bool {
        if self.target == target && self.active == active {
            return false;
        }
        self.invalidate();
        if self.target != target {
            self.mapping_enabled = false;
            // An actual connection change can retry a completed failed stop.
            // A still-pending stop records its own result before admission.
            self.cleanup_error = None;
        }
        self.target = target;
        self.active = active;
        self.error = None;
        true
    }
    fn suspend_mapping(&mut self) {
        self.mapping_enabled = false;
        self.invalidate();
    }
    fn published(&mut self, enabled: bool) {
        self.mapping_enabled = enabled;
        self.error = None;
    }
    fn failed(&mut self, error: String) {
        self.error = Some(error);
        self.playback = Playback::default();
    }
    fn cleanup_finished(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.cleanup_error = Some(error.clone());
            self.failed(error);
        }
    }
    fn can_start(&self, product_id: u32, page: &str, mapping_ready: bool) -> bool {
        product_id == 679
            && self.active
            && page == "TAB_CUSTOMIZE"
            && self.mapping_enabled
            && mapping_ready
            && self.target.is_some()
            && self.error.is_none()
            && self.cleanup_error.is_none()
    }
    fn accepts(&self, epoch: u64) -> bool {
        self.epoch == epoch
            && self.active
            && self.mapping_enabled
            && self.error.is_none()
            && self.cleanup_error.is_none()
    }
    fn consume(&mut self, epoch: u64, events: &[GamepadOutputState], now: Duration) -> bool {
        self.accepts(epoch) && self.playback.replace(events, now)
    }
}

/// The cleanup wait is retained until completion, even across multiple stops.
/// Admission never constructs a replacement stream while that wait exists.
struct StreamTasks<S, C> {
    stream: Option<S>,
    cleanup: Option<C>,
}
impl<S, C> Default for StreamTasks<S, C> {
    fn default() -> Self {
        Self {
            stream: None,
            cleanup: None,
        }
    }
}
impl<S, C> StreamTasks<S, C> {
    fn occupied(&self) -> bool {
        self.stream.is_some() || self.cleanup.is_some()
    }
    fn begin_cleanup(&mut self, wait: impl FnOnce(S) -> C) {
        if self.cleanup.is_none() {
            self.cleanup = self.stream.take().map(wait);
        }
    }
    fn start<E>(&mut self, start: impl FnOnce() -> Result<S, E>) -> Result<bool, E> {
        if self.occupied() {
            return Ok(false);
        }
        self.stream = Some(start()?);
        Ok(true)
    }
    fn finish_cleanup(&mut self) {
        self.cleanup = None;
    }
}

#[derive(Default)]
pub(super) struct Observer {
    lifecycle: Lifecycle,
    tasks: StreamTasks<Session, Task<()>>,
    pump: Option<Task<()>>,
}

impl KeyboardProductWorkspace {
    pub fn set_gamepad_observation_target(
        &mut self,
        target: Option<Target>,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if self.gamepad_observer.lifecycle.retarget(target, active) {
            self.stop_gamepad_observation(cx);
        }
        self.sync_gamepad_observation(cx);
    }

    pub(super) fn suspend_gamepad_mapping(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != 679 {
            return;
        }
        self.gamepad_observer.lifecycle.suspend_mapping();
        self.stop_gamepad_observation(cx);
    }

    pub(super) fn gamepad_mapping_published(&mut self, document: &Value, cx: &mut Context<Self>) {
        if self.spec.product_id != 679 {
            return;
        }
        match source_mapping_enabled(document) {
            Ok(enabled) => {
                self.gamepad_observer.lifecycle.published(enabled);
                self.sync_gamepad_observation(cx);
            }
            Err(error) => self.gamepad_observation_failed(error, cx),
        }
    }

    pub(in crate::features) fn leave_gamepad_observation(&mut self, cx: &mut Context<Self>) {
        self.gamepad_observer.lifecycle.active = false;
        self.stop_gamepad_observation(cx);
    }

    pub(super) fn stop_gamepad_observation(&mut self, cx: &mut Context<Self>) {
        self.gamepad_observer.lifecycle.invalidate();
        self.gamepad_observer.pump = None;
        self.analog_gamepad.gamepad = None;
        self.gamepad_observer.tasks.begin_cleanup(|stream| {
            stream.request_stop();
            let cleanup = cx.background_executor().spawn(async move { stream.stop() });
            cx.spawn(async move |view, cx| {
                let result = cleanup.await;
                if let Err(error) = &result {
                    eprintln!("[gamepad-observation-cleanup] {error}");
                }
                let _ = view.update(cx, |this, cx| {
                    this.gamepad_observer.tasks.finish_cleanup();
                    // A stop acknowledgement is owned by the cleanup task,
                    // independently of how many presentation epochs elapsed.
                    this.gamepad_observer
                        .lifecycle
                        .cleanup_finished(result.clone());
                    if let Err(error) = result {
                        this.gamepad_observation_failed(error, cx);
                    }
                    this.sync_gamepad_observation(cx);
                });
            })
        });
        cx.notify();
    }

    pub(super) fn sync_gamepad_observation(&mut self, cx: &mut Context<Self>) {
        if !self.gamepad_observer.lifecycle.can_start(
            self.spec.product_id,
            &self.page,
            self.snap_mapping_ready(),
        ) || self.gamepad_observer.tasks.occupied()
        {
            return;
        }
        // PID679 is analogV3 with neither firmware analog mapping nor the
        // 8 kHz skip map. MW14486.V passes numOfEventsToSkip=50; the
        // native observation owner applies the original counter semantics.
        let target = self.gamepad_observer.lifecycle.target.clone().unwrap();
        match self
            .gamepad_observer
            .tasks
            .start(|| Session::start(target, Registration::Events { skip: 50 }))
        {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                self.gamepad_observation_failed(format!("{error:#}"), cx);
                return;
            }
        }
        let epoch = self.gamepad_observer.lifecycle.epoch;
        let began = Instant::now();
        self.gamepad_observer.pump = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1))
                    .await;
                if !view
                    .update(cx, |this, cx| {
                        if !this.gamepad_observer.lifecycle.accepts(epoch) {
                            return false;
                        }
                        let Some(stream) = &this.gamepad_observer.tasks.stream else {
                            return false;
                        };
                        let replies = stream.drain();
                        let finished = stream.is_finished();
                        for reply in replies {
                            match reply {
                                Reply::Ready => {}
                                Reply::Event(delivery) => this.consume_gamepad_output(
                                    epoch,
                                    delivery,
                                    began.elapsed(),
                                    cx,
                                ),
                                Reply::Failed(error) => this.gamepad_observation_failed(error, cx),
                            }
                        }
                        if finished || this.gamepad_observer.lifecycle.error.is_some() {
                            this.stop_gamepad_observation(cx);
                            return false;
                        }
                        if this
                            .gamepad_observer
                            .lifecycle
                            .playback
                            .advance(began.elapsed())
                        {
                            this.analog_gamepad.gamepad =
                                this.gamepad_observer.lifecycle.playback.observed;
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

    fn consume_gamepad_output(
        &mut self,
        epoch: u64,
        delivery: Delivery,
        now: Duration,
        cx: &mut Context<Self>,
    ) {
        if self
            .gamepad_observer
            .lifecycle
            .consume(epoch, delivery.events(), now)
        {
            self.analog_gamepad.gamepad = self.gamepad_observer.lifecycle.playback.observed;
            cx.notify();
        }
    }

    fn gamepad_observation_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.gamepad_observer.lifecycle.failed(error.clone());
        self.analog_gamepad.gamepad = None;
        cx.emit(KeyboardLegacyRuntimeStatus(format!(
            "Gamepad output observation: {error}"
        )));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(value: i16) -> GamepadOutputState {
        GamepadOutputState {
            buttons: 0x1000,
            left_thumb: [value, -123],
            right_thumb: [456, -789],
            left_trigger: 31,
            right_trigger: 229,
        }
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_playback_keeps_source_spacing_and_replaces_stale_array() {
        let mut playback = Playback::default();
        assert!(playback.replace(&[sample(12), sample(23), sample(34)], Duration::ZERO));
        assert_eq!(playback.observed.unwrap().left_thumb_x, 12);
        assert!(!playback.advance(Duration::from_millis(4)));
        assert!(playback.advance(Duration::from_millis(5)));
        assert_eq!(playback.observed.unwrap().left_thumb_x, 23);
        assert!(playback.replace(&[sample(900)], Duration::from_millis(6)));
        assert!(!playback.advance(Duration::from_millis(50)));
        assert_eq!(playback.observed.unwrap().left_thumb_x, 900);
        assert_eq!(playback.observed.unwrap().right_trigger, 229);
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_empty_batch_does_not_manufacture_a_center_sample() {
        let mut playback = Playback::default();
        assert!(!playback.replace(&[], Duration::ZERO));
        assert!(playback.observed.is_none());
        playback.replace(&[sample(41), sample(51)], Duration::ZERO);
        assert!(!playback.replace(&[], Duration::from_millis(1)));
        assert!(!playback.advance(Duration::from_millis(20)));
        assert_eq!(playback.observed.unwrap().left_thumb_x, 41);
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_registration_uses_active_saved_analog_controller_assignments() {
        let mut document = json!({"activeProfile":"current", "profiles":[
            {"guid":"other", "mappings":[{"inputType":"AnalogInput","mapping":[{"controllerGroup":{"controllerAssignment":"LEFTTRIGGER"}}]}]},
            {"guid":"current", "mappings":[{"inputType":"AnalogInput","mapping":[{"controllerGroup":{"controllerAssignment":"A_BUTTON"}}]}]}
        ]});
        assert_eq!(source_mapping_enabled(&document), Ok(false));
        document["profiles"][1]["mappings"][0]["mapping"][0]["controllerGroup"]["controllerAssignment"] =
            json!("RIGHT_JS_LEFT");
        assert_eq!(source_mapping_enabled(&document), Ok(true));
        document["profiles"][1]["mappings"][0]["inputType"] = json!("KeyInput");
        assert_eq!(source_mapping_enabled(&document), Ok(false));
        document["activeProfile"] = json!("missing");
        assert!(source_mapping_enabled(&document).is_err());
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_registration_rejects_missing_or_duplicate_active_profile_data() {
        assert!(source_mapping_enabled(&json!({})).is_err());
        assert!(source_mapping_enabled(&json!({"activeProfile":"a", "profiles":[{"guid":"a","mappings":[{"inputType":"AnalogInput"}]}]})).is_err());
        assert!(source_mapping_enabled(&json!({"activeProfile":"a", "profiles":[{"guid":"a","mappings":[]},{"guid":"a","mappings":[]}]})).is_err());
    }

    fn observing(container: &str) -> Lifecycle {
        let mut lifecycle = Lifecycle::default();
        lifecycle.retarget(Some(Target::new(679, container.into())), true);
        lifecycle.published(true);
        lifecycle
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_target_change_rejects_old_batches_after_new_observation() {
        let mut lifecycle = observing("first-connection");
        let old = lifecycle.epoch;
        assert!(lifecycle.consume(old, &[sample(10), sample(20)], Duration::ZERO));
        lifecycle.retarget(Some(Target::new(679, "second-connection".into())), true);
        assert!(lifecycle.playback.observed.is_none());
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(!lifecycle.consume(old, &[sample(30)], Duration::from_millis(5)));
        lifecycle.published(true);
        assert!(lifecycle.consume(lifecycle.epoch, &[sample(700)], Duration::from_millis(7)));
        assert!(!lifecycle.consume(old, &[sample(40)], Duration::from_millis(8)));
        assert_eq!(lifecycle.playback.observed.unwrap().left_thumb_x, 700);
        assert!(!lifecycle.playback.advance(Duration::from_millis(50)));
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_same_connection_page_return_keeps_eligibility_and_clears_samples() {
        let mut lifecycle = observing("connection");
        let old = lifecycle.epoch;
        lifecycle.consume(old, &[sample(12), sample(24)], Duration::ZERO);
        lifecycle.invalidate();
        assert!(!lifecycle.can_start(679, "ACTUATION", true));
        assert!(!lifecycle.retarget(Some(Target::new(679, "connection".into())), true));
        assert!(lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(lifecycle.playback.observed.is_none());
        assert!(!lifecycle.consume(old, &[sample(24)], Duration::from_millis(5)));
        // Switching to Help/dashboard additionally deactivates the owner.
        lifecycle.retarget(Some(Target::new(679, "connection".into())), false);
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        lifecycle.retarget(Some(Target::new(679, "connection".into())), true);
        assert!(lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(!lifecycle.can_start(678, "TAB_CUSTOMIZE", true));
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", false));
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_failed_mapping_republication_cannot_resume_previous_output() {
        let mut lifecycle = observing("connection");
        let old = lifecycle.epoch;
        lifecycle.consume(old, &[sample(900), sample(901)], Duration::ZERO);
        lifecycle.suspend_mapping();
        lifecycle.cleanup_finished(Ok(()));
        // A failed publication has no Published-success transition, even
        // though the native mapping connection itself remains registered.
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(!lifecycle.consume(old, &[sample(902)], Duration::from_millis(5)));
        assert!(lifecycle.playback.observed.is_none());
        lifecycle.retarget(Some(Target::new(679, "connection".into())), false);
        lifecycle.retarget(Some(Target::new(679, "connection".into())), true);
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        lifecycle.published(true);
        assert!(lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(lifecycle.playback.observed.is_none());
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_cleanup_failure_survives_new_scope_and_mapping_acknowledgement() {
        let mut lifecycle = observing("connection");
        lifecycle.invalidate();
        // The view can switch connection while the old stop is still pending.
        lifecycle.retarget(Some(Target::new(679, "replacement".into())), true);
        lifecycle.published(true);
        lifecycle.cleanup_finished(Err("unregister acknowledgement failed".into()));
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert_eq!(
            lifecycle.cleanup_error.as_deref(),
            Some("unregister acknowledgement failed")
        );
        lifecycle.published(true);
        lifecycle.retarget(Some(Target::new(679, "replacement".into())), false);
        lifecycle.retarget(Some(Target::new(679, "replacement".into())), true);
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(!lifecycle.consume(lifecycle.epoch, &[sample(80)], Duration::ZERO));
        // A subsequent actual disconnect/reconnect may retry, with no cached
        // mapping eligibility or observed value promoted to the new owner.
        lifecycle.retarget(None, false);
        lifecycle.retarget(Some(Target::new(679, "replacement".into())), true);
        assert!(!lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        lifecycle.published(true);
        assert!(lifecycle.can_start(679, "TAB_CUSTOMIZE", true));
        assert!(lifecycle.playback.observed.is_none());
    }

    #[::core::prelude::v1::test]
    fn gamepad_output_repeated_stop_retains_cleanup_wait_and_defers_stream_construction() {
        use std::{cell::Cell, rc::Rc};
        struct Stream(Rc<Cell<usize>>);
        impl Drop for Stream {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        struct StopWait {
            _stream: Stream,
        }
        let dropped = Rc::new(Cell::new(0));
        let starts = Cell::new(0);
        let waits = Cell::new(0);
        let mut tasks: StreamTasks<Stream, StopWait> = StreamTasks::default();
        let create = || {
            starts.set(starts.get() + 1);
            Ok::<_, ()>(Stream(dropped.clone()))
        };
        assert_eq!(tasks.start(create), Ok(true));
        tasks.begin_cleanup(|stream| {
            waits.set(waits.get() + 1);
            StopWait { _stream: stream }
        });
        // Repeated navigation/stop must keep the exact old join wait alive.
        tasks.begin_cleanup(|stream| {
            waits.set(waits.get() + 1);
            StopWait { _stream: stream }
        });
        assert_eq!(waits.get(), 1);
        assert_eq!(dropped.get(), 0);
        assert_eq!(tasks.start(create), Ok(false));
        assert_eq!(starts.get(), 1);
        tasks.finish_cleanup();
        assert_eq!(dropped.get(), 1);
        assert_eq!(tasks.start(create), Ok(true));
        assert_eq!(starts.get(), 2);
    }
}
