//! Retained output callback, registered and revoked on the device thread.
use super::*;
use razer_device::{
    mapping_native_gamepad_observation::State, mapping_native_output::GamepadOutputState,
};
use razer_ipc::gamepad_observation::{Delivery, Registration};

pub(in super::super) enum Control {
    Register {
        epoch: Uuid,
        registration: Registration,
    },
    Stop {
        epoch: Uuid,
    },
    Clear,
}
pub(in super::super) struct Sink {
    pub events: mpsc::SyncSender<Event>,
    pub overflow: Arc<AtomicBool>,
}
#[derive(Default)]
pub(in super::super) struct Observation {
    owner: Option<(Uuid, Sink)>,
    stream: State,
}
impl Observation {
    pub fn control(&mut self, control: Control, sink: Sink) -> Result<()> {
        match control {
            Control::Register {
                epoch,
                registration,
            } => {
                ensure!(
                    self.owner.is_none(),
                    "Gamepad observer is already registered"
                );
                let registered = match registration {
                    Registration::Events { skip } => self.stream.register_events(skip),
                    Registration::Interval { milliseconds } => {
                        self.stream.register_interval(milliseconds)
                    }
                };
                ensure!(registered, "Gamepad observer is already registered");
                self.owner = Some((epoch, sink));
            }
            Control::Stop { epoch } => {
                ensure!(
                    self.owner
                        .as_ref()
                        .is_some_and(|(active, _)| *active == epoch),
                    "Gamepad observer generation changed"
                );
                *self = Self::default();
            }
            Control::Clear => *self = Self::default(),
        }
        Ok(())
    }
    pub fn observe(
        &mut self,
        previous: GamepadOutputState,
        current: GamepadOutputState,
        tick: u64,
    ) {
        if let Some(delivery) = self.stream.observe(previous, current, tick) {
            self.emit(Delivery::new(delivery.events, delivery.time_tick));
        }
    }
    pub fn poll(&mut self, tick: u64) {
        for delivery in self.stream.poll(tick) {
            self.emit(Delivery::new(delivery.events, delivery.time_tick));
        }
    }
    pub fn next_deadline(&self) -> Option<u64> {
        self.stream.next_deadline()
    }
    fn emit(&self, delivery: Delivery) {
        if let Some((observer, sink)) = &self.owner {
            if sink
                .events
                .try_send(Event::Gamepad {
                    observer: *observer,
                    delivery,
                })
                .is_err()
            {
                sink.overflow.store(true, Ordering::Release);
            }
        }
    }
}
impl<E: Executor> Worker<E> {
    pub(super) fn control_gamepad_observation(&mut self, control: Control) -> Result<()> {
        self.executor.gamepad_observation(
            control,
            Sink {
                events: self.events.clone(),
                overflow: self.overflow.clone(),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sink(sender: &mpsc::SyncSender<Event>) -> Sink {
        Sink {
            events: sender.clone(),
            overflow: Arc::new(AtomicBool::new(false)),
        }
    }
    fn state(value: u8) -> GamepadOutputState {
        GamepadOutputState {
            left_trigger: value,
            ..Default::default()
        }
    }
    #[test]
    fn stop_discards_pending_batch_and_stale_stop_cannot_cancel_new_generation() {
        let (tx, rx) = mpsc::sync_channel(16);
        let mut observer = Observation::default();
        let first = Uuid::new_v4();
        let next = Uuid::new_v4();
        observer
            .control(
                Control::Register {
                    epoch: first,
                    registration: Registration::Interval { milliseconds: 5 },
                },
                sink(&tx),
            )
            .unwrap();
        observer.observe(state(0), state(1), 1_000);
        assert!(rx.try_recv().is_err());
        assert!(
            observer
                .control(Control::Stop { epoch: next }, sink(&tx))
                .is_err()
        );
        observer
            .control(Control::Stop { epoch: first }, sink(&tx))
            .unwrap();
        observer
            .control(
                Control::Register {
                    epoch: next,
                    registration: Registration::Interval { milliseconds: 5 },
                },
                sink(&tx),
            )
            .unwrap();
        assert!(
            observer
                .control(Control::Stop { epoch: first }, sink(&tx))
                .is_err()
        );
        observer.poll(5_000);
        assert!(rx.try_recv().is_err());
        observer.observe(state(1), state(2), 6_000);
        assert!(
            matches!(rx.try_recv().unwrap(),Event::Gamepad { observer,delivery } if observer == next && delivery.events() == [state(2)])
        );
    }
    #[test]
    fn clear_prevents_cleanup_outputs_and_overflow_is_explicit() {
        let (tx, rx) = mpsc::sync_channel(1);
        let epoch = Uuid::new_v4();
        let overflow = Arc::new(AtomicBool::new(false));
        let mut observer = Observation::default();
        observer
            .control(
                Control::Register {
                    epoch,
                    registration: Registration::Events { skip: 50 },
                },
                Sink {
                    events: tx.clone(),
                    overflow: overflow.clone(),
                },
            )
            .unwrap();
        observer.observe(state(0), state(1), 1);
        observer.observe(state(1), state(2), 2);
        assert!(overflow.load(Ordering::Acquire));
        assert!(
            matches!(rx.try_recv().unwrap(),Event::Gamepad { observer,.. } if observer == epoch)
        );
        observer.control(Control::Clear, sink(&tx)).unwrap();
        observer.observe(state(2), state(0), 3);
        observer.poll(u64::MAX);
        assert!(rx.try_recv().is_err());
    }
}
