//! Device-thread owner of analog callback registrations and scheduled batches.
use super::*;
use razer_device::mapping_native_observation::State;
use razer_ipc::analog_observation::{AnalogEvent, Delivery, Registration};

#[derive(Default)]
pub(super) struct Observation {
    epoch: Option<Uuid>,
    stream: State<AnalogEvent>,
}
pub(in super::super) enum Control {
    Register {
        epoch: Uuid,
        registration: Registration,
    },
    RemoveKey {
        epoch: Uuid,
        input: Input,
    },
    Stop {
        epoch: Uuid,
    },
}
pub(in super::super) fn same_key(left: &Input, right: &Input) -> bool {
    match (&left.kind, &right.kind) {
        (
            InputKind::Keyboard {
                scancode: a,
                flag: af,
            }
            | InputKind::AnalogKey {
                scancode: a,
                flag: af,
            },
            InputKind::Keyboard {
                scancode: b,
                flag: bf,
            }
            | InputKind::AnalogKey {
                scancode: b,
                flag: bf,
            },
        ) => a == b && af & !1 == bf & !1,
        _ => left.stateless_id() == right.stateless_id(),
    }
}
impl<E: Executor> Worker<E> {
    fn analog_key(&self, input: &Input) -> Result<u8> {
        let config = self
            .hardware
            .config
            .as_ref()
            .context("Analog observer requires a device configuration")?;
        ensure!(
            config.descriptor.analog_format.is_some(),
            "Device has no analog key report"
        );
        (0..=u8::MAX)
            .find(|key| {
                config
                    .analog_input(*key)
                    .is_some_and(|candidate| same_key(&candidate, input))
            })
            .context("Analog observer input is absent from the device configuration")
    }
    pub(super) fn control_analog_observation(&mut self, control: Control) -> Result<()> {
        match control {
            Control::Register {
                epoch,
                registration,
            } => {
                let config = self
                    .hardware
                    .config
                    .as_ref()
                    .context("Analog observer requires a device configuration")?;
                ensure!(
                    config.descriptor.analog_format.is_some(),
                    "Device has no analog key report"
                );
                ensure!(
                    self.analog_observation
                        .epoch
                        .is_none_or(|active| active == epoch),
                    "Analog observer generation changed"
                );
                match registration {
                    Registration::All { skip } => self.analog_observation.stream.register_all(skip),
                    Registration::Interval { milliseconds } => self
                        .analog_observation
                        .stream
                        .register_interval(milliseconds),
                    Registration::Key { input, skip } => {
                        let key = self.analog_key(&input)?;
                        self.analog_observation.stream.register_key(key, skip);
                    }
                }
                self.analog_observation.epoch = Some(epoch);
            }
            Control::RemoveKey { epoch, input } => {
                ensure!(
                    self.analog_observation.epoch == Some(epoch),
                    "Analog observer generation changed"
                );
                let key = self.analog_key(&input)?;
                ensure!(
                    self.analog_observation.stream.unregister_key(key),
                    "Analog key observer is not registered"
                );
            }
            Control::Stop { epoch } => {
                ensure!(
                    self.analog_observation.epoch == Some(epoch),
                    "Analog observer generation changed"
                );
                // No subscriber remains. Retired queued batches must not enter
                // the next owner generation, including a rapid page reopen.
                self.analog_observation = Observation::default();
            }
        }
        Ok(())
    }
    pub(super) fn notify_analog(
        &mut self,
        key_id: u8,
        level: u16,
        input: &Input,
        output: Option<&Output>,
        force: bool,
    ) -> Result<()> {
        let Some(observer) = self.analog_observation.epoch else {
            return Ok(());
        };
        let tick = razer_platform::event_clock::tick()?;
        if let Some(delivery) = self
            .analog_observation
            .stream
            .observe(key_id, force, tick, || {
                AnalogEvent::new(input.clone(), key_id, level, output.cloned(), tick)
            })
        {
            self.emit(Event::Analog {
                observer,
                delivery: Delivery::new(delivery.events, delivery.time_tick),
            });
        }
        Ok(())
    }
    pub(super) fn poll_analog_observation(&mut self) -> Result<()> {
        let Some(observer) = self.analog_observation.epoch else {
            return Ok(());
        };
        if self.analog_observation.stream.next_deadline().is_none() {
            return Ok(());
        }
        let tick = razer_platform::event_clock::tick()?;
        for delivery in self.analog_observation.stream.poll(tick) {
            self.emit(Event::Analog {
                observer,
                delivery: Delivery::new(delivery.events, delivery.time_tick),
            });
        }
        Ok(())
    }
    pub(super) fn observation_wait(&self) -> Duration {
        self.analog_observation
            .stream
            .next_deadline()
            .and_then(|deadline| {
                razer_platform::event_clock::tick()
                    .ok()
                    .map(|now| Duration::from_micros(deadline.saturating_sub(now)))
            })
            .map_or(Duration::from_millis(4), |wait| {
                wait.min(Duration::from_millis(4))
            })
    }
}
