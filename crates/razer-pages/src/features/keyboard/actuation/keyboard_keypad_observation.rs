//! PID747 raw controller samples, distinct from computed mapping output.
use super::observation_tasks::{SubscriptionTasks, join_stream};
use super::*;
use razer_device::keyboard_keypad_observation::Sample;
use razer_ipc::keypad_observation::Target;
use razer_service::mapping::{
    keypad_observation::{Reply, Session},
    observation::Completion,
};
use std::time::Duration;

#[derive(Default)]
struct Lifecycle {
    target: Option<Target>,
    forwarding: bool,
    epoch: u64,
    error: Option<String>,
    last_sample: Option<Sample>,
}

impl Lifecycle {
    fn retarget(&mut self, target: Option<Target>) -> bool {
        if self.target == target {
            return false;
        }
        self.target = target;
        self.last_sample = None;
        self.error = None;
        true
    }

    fn set_forwarding(&mut self, enabled: bool) -> bool {
        if self.forwarding == enabled {
            return false;
        }
        self.forwarding = enabled;
        // MW41243 changes only its forwarding gate; it does not reset the
        // customizeReducer's last gamepadTester sample.
        if enabled {
            self.error = None;
        }
        true
    }

    fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn can_start(&self, product_id: u32) -> bool {
        product_id == 747 && self.forwarding && self.target.is_some() && self.error.is_none()
    }

    fn accepts(&self, epoch: u64) -> bool {
        self.epoch == epoch && self.can_start(747)
    }

    fn consume(&mut self, epoch: u64, sample: Sample) -> bool {
        if !self.accepts(epoch) {
            return false;
        }
        let changed = self.last_sample.as_ref() != Some(&sample);
        self.last_sample = Some(sample);
        changed
    }
}

#[derive(Default)]
pub(super) struct Observer {
    lifecycle: Lifecycle,
    tasks: SubscriptionTasks<Session, Task<()>>,
    pump: Option<Task<()>>,
}

impl Observer {
    pub(super) fn sample(&self) -> Option<&Sample> {
        self.lifecycle.last_sample.as_ref()
    }

    pub(super) fn release(&mut self, executor: &BackgroundExecutor) {
        self.lifecycle.forwarding = false;
        self.lifecycle.invalidate();
        self.pump = None;
        self.tasks.release(|stream| {
            stream.request_stop();
            join_stream(executor, stream, Session::stop_with_receipt);
        });
    }
}

impl KeyboardProductWorkspace {
    pub(in crate::features) fn set_keypad_observation_target(
        &mut self,
        target: Option<Target>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 747 {
            return;
        }
        if self.keypad_observer.lifecycle.retarget(target) {
            self.stop_keypad_observation(cx);
        }
        self.sync_keypad_observation(cx);
    }

    pub(super) fn set_keypad_sample_forwarding(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.keypad_observer.lifecycle.set_forwarding(enabled) {
            self.stop_keypad_observation(cx);
        }
        self.sync_keypad_observation(cx);
    }

    fn stop_keypad_observation(&mut self, cx: &mut Context<Self>) {
        self.keypad_observer.lifecycle.invalidate();
        self.keypad_observer.pump = None;
        let epoch = self.keypad_observer.lifecycle.epoch;
        let target = self.keypad_observer.lifecycle.target.clone();
        self.keypad_observer
            .tasks
            .begin_cleanup(|identity, stream| {
                stream.request_stop();
                let cleanup =
                    join_stream(cx.background_executor(), stream, Session::stop_with_receipt);
                cx.spawn(async move |view, cx| {
                    let result = cleanup.recv().await.unwrap_or_else(|_| {
                        Completion::cleanup_failed("Keypad observation cleanup result disconnected")
                    });
                    let _ = view.update(cx, |this, cx| {
                        if !this
                            .keypad_observer
                            .tasks
                            .finish_cleanup(&identity, result.cleanup_result())
                        {
                            return;
                        }
                        let error = result.cleanup_error().or_else(|| {
                            (this.keypad_observer.lifecycle.epoch == epoch
                                && this.keypad_observer.lifecycle.target == target
                                && identity.matches_target(target.as_ref()))
                            .then(|| result.operation_error())
                            .flatten()
                        });
                        if let Some(error) = error {
                            this.keypad_observation_failed(error.into(), cx);
                        }
                        this.sync_keypad_observation(cx);
                    });
                })
            });
        self.notify_keypad_views(cx);
    }

    fn sync_keypad_observation(&mut self, cx: &mut Context<Self>) {
        if !self
            .keypad_observer
            .lifecycle
            .can_start(self.spec.product_id)
            || self.keypad_observer.tasks.occupied()
        {
            return;
        }
        let target = self.keypad_observer.lifecycle.target.clone().unwrap();
        let identity = match self
            .keypad_observer
            .tasks
            .start(target.clone(), || Session::start(target))
        {
            Ok(Some(identity)) => identity,
            Ok(None) => return,
            Err(error) => {
                self.keypad_observation_failed(format!("{error:#}"), cx);
                return;
            }
        };
        let epoch = self.keypad_observer.lifecycle.epoch;
        self.keypad_observer.pump = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1))
                    .await;
                if !view
                    .update(cx, |this, cx| {
                        if !this.keypad_observer.lifecycle.accepts(epoch)
                            || !this.keypad_observer.tasks.accepts(&identity)
                        {
                            return false;
                        }
                        let Some(stream) = this.keypad_observer.tasks.stream() else {
                            return false;
                        };
                        let replies = stream.drain();
                        let finished = stream.is_finished();
                        for reply in replies {
                            match reply {
                                Reply::Ready => {}
                                Reply::Event(delivery) => {
                                    if this
                                        .keypad_observer
                                        .lifecycle
                                        .consume(epoch, *delivery.sample())
                                    {
                                        this.notify_keypad_views(cx);
                                    }
                                }
                                Reply::Failed(error) => this.keypad_observation_failed(error, cx),
                            }
                        }
                        if finished || this.keypad_observer.lifecycle.error.is_some() {
                            this.stop_keypad_observation(cx);
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

    fn keypad_observation_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.keypad_observer.lifecycle.error = Some(error.clone());
        cx.emit(KeyboardLegacyRuntimeStatus(format!(
            "Keypad observation: {error}"
        )));
        self.notify_keypad_views(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Sample {
        razer_device::keyboard_keypad_observation::decode_report(&[
            0, 0, 128, 255, 255, 0, 0, 0, 192, 0, 128, 0, 0, 0, 0,
        ])
        .unwrap()
        .sample
    }

    #[::core::prelude::v1::test]
    fn keypad_forwarding_preserves_source_sample_and_does_not_register_a_device_setting() {
        let mut state = Lifecycle::default();
        assert!(state.retarget(Some(Target::new(747, "device-a".into()))));
        assert!(!state.can_start(747));
        assert!(state.set_forwarding(true));
        assert!(state.can_start(747));
        assert!(!state.can_start(679));
        assert!(state.consume(state.epoch, sample()));
        let sample = state.last_sample;
        assert!(state.set_forwarding(false));
        state.invalidate();
        assert_eq!(state.last_sample, sample);
        assert!(!state.can_start(747));
        assert!(state.set_forwarding(true));
        assert_eq!(state.last_sample, sample);
        assert!(state.can_start(747));
    }

    #[::core::prelude::v1::test]
    fn keypad_reconnect_rejects_stale_samples_without_replacing_device_values_with_defaults() {
        let mut state = Lifecycle::default();
        state.retarget(Some(Target::new(747, "device-a".into())));
        state.set_forwarding(true);
        let old_epoch = state.epoch;
        assert!(state.consume(old_epoch, sample()));
        assert!(state.retarget(None));
        state.invalidate();
        assert!(state.last_sample.is_none());
        assert!(state.retarget(Some(Target::new(747, "device-b".into()))));
        state.invalidate();
        assert!(!state.consume(old_epoch, sample()));
        assert!(state.last_sample.is_none());
        assert!(state.consume(state.epoch, sample()));
        state.error = Some("raw HID read failed".into());
        assert!(!state.consume(state.epoch, sample()));
        assert!(!state.can_start(747));
    }
}
