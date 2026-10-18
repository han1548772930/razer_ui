//! Digital input notification, interruption and Snap Tap on the device actor.
use super::*;
use razer_device::{
    mapping_native_dynamic_key::Effect as DynamicEffect,
    mapping_native_lookup::{Phase, ResolvedOutput},
    mapping_native_snaptap::{Effect, Policy},
};

impl<E: Executor> Worker<E> {
    pub(super) fn notify_input(&mut self, input: &Input, tick: u64, batched: bool) -> Result<()> {
        if self.input_observation_enabled() {
            self.emit(Event::Input {
                input: input.clone(),
                tick: razer_platform::event_clock::tick()?,
                redirected: false,
                observer: self.input_observer,
            });
        }
        self.interrupt_input(input, tick, batched)
    }

    pub(super) fn interrupt_input(
        &mut self,
        input: &Input,
        tick: u64,
        batched: bool,
    ) -> Result<()> {
        let mut errors = Vec::new();
        // 8059c invokes digital 8dd0e before analog 8e394. The selected make
        // snapshots survive nested notifications; held lookup occurs per item.
        for make in self.lookup.select_digital_interrupts(input) {
            let interrupted = self.lookup.prepare_digital_interrupt(make);
            if let Some(release) = interrupted.release {
                if let Err(error) = self.execute_digital(release, 0, batched, true) {
                    errors.push(format!("Digital interrupt output: {error:#}"));
                }
            }
            if matches!(
                interrupted.make.output,
                ResolvedOutput::Mapped(Output::Macro { .. })
            ) {
                if let Err(error) = self.executor.interrupt_macro(&interrupted.make) {
                    errors.push(format!("Digital interrupted macro: {error:#}"));
                }
            }
        }
        if let Err(error) = self.interrupt_hardware(input, tick, batched) {
            errors.push(format!("Analog interruption: {error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub(super) fn resolve_digital(
        &mut self,
        input: Input,
        modes: Modes,
        tick: u64,
        repetitions: u8,
        batched: bool,
    ) -> Result<()> {
        let resolved = self.lookup.resolve(input, modes);
        let mut errors = Vec::new();
        if let Some(resolved) = resolved {
            // 677ba consumes interruption suppression before checking Snap Tap.
            // Suppressed real breaks must not re-enter arbitration.
            let inactive = resolved.input.phase() == Phase::Release
                && self
                    .snaptap
                    .physical_release(&resolved.input.stateless_id());
            for _ in 0..repetitions {
                if let Err(error) =
                    self.execute_tracked_mapping(resolved.clone(), tick, batched, !inactive)
                {
                    errors.push(format!("{error:#}"));
                }
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn execute_tracked_mapping(
        &mut self,
        resolved: Resolved<Output>,
        tick: u64,
        batched: bool,
        emit: bool,
    ) -> Result<()> {
        let mut errors = Vec::new();
        if let Some(mut transition) = self.dynamic_keys.begin(&resolved) {
            while let Some(effect) = self.dynamic_keys.next(&mut transition) {
                let result = match effect {
                    DynamicEffect::Current if emit => {
                        self.execute_resolved(resolved.clone(), tick, false)
                    }
                    DynamicEffect::Current => Ok(()),
                    DynamicEffect::Pause => self.executor.pause_mapping_replay(),
                    // 69728 directly replays the retained make with tick 0;
                    // it neither re-enters lookup nor sends input observations.
                    DynamicEffect::Replay(mapping) => self.execute_resolved(mapping, 0, false),
                };
                if let Err(error) = result {
                    errors.push(format!("Dynamic key output: {error:#}"));
                }
            }
            if transition.handled() {
                ensure!(errors.is_empty(), "{}", errors.join("; "));
                return Ok(());
            }
        }
        // 69530 tries Snap Tap only when dynamic-key did not consume the
        // current mapping. 8DD0E interruption calls execute_digital directly.
        if let Err(error) = self.execute_digital(resolved, tick, batched, emit) {
            errors.push(format!("{error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn execute_digital(
        &mut self,
        resolved: Resolved<Output>,
        tick: u64,
        batched: bool,
        emit: bool,
    ) -> Result<()> {
        let policy = resolved
            .selectors
            .snap_tap
            .and_then(|group| {
                self.hardware
                    .config
                    .as_ref()?
                    .profile
                    .snaptaps
                    .get(&group)
                    .copied()
            })
            .unwrap_or(Policy::LastInput);
        let transition = match self.snaptap.begin(&resolved, policy) {
            Ok(transition) => transition,
            Err(error) => {
                // 68d0c logs an unsupported digital policy and returns false,
                // allowing the ordinary output branch to consume this input.
                self.emit(Event::Failure(format!("{error:#}")));
                None
            }
        };
        let Some(mut transition) = transition else {
            return if emit {
                self.execute_resolved(resolved, tick, false)
            } else {
                Ok(())
            };
        };
        let mut errors = Vec::new();
        while let Some(effect) = self.snaptap.next(&mut transition) {
            match effect {
                Effect::Current => {
                    if emit {
                        if let Err(error) = self.execute_resolved(resolved.clone(), tick, false) {
                            errors.push(format!("Snap Tap output: {error:#}"));
                        }
                    }
                    // 82482/828C8/82F94 observe even an empty current callback.
                    if let Err(error) = self.notify_snaptap(&resolved.input, false) {
                        errors.push(format!("Snap Tap observation: {error:#}"));
                    }
                }
                Effect::Pause => {
                    if let Err(error) = self.executor.pause_mapping_replay() {
                        errors.push(format!("Snap Tap wait: {error:#}"));
                    }
                }
                Effect::Replay { identity, phase } => {
                    let Some(pair) = self.lookup.held_by_stateless(&identity) else {
                        continue;
                    };
                    if !self.snaptap.replayed(&identity, phase) {
                        continue;
                    }
                    let replay = if phase == Phase::Press {
                        pair.make
                    } else {
                        pair.release
                    };
                    if let Err(error) = self.notify_input(&replay.input, tick, batched) {
                        errors.push(format!("Snap Tap input notification: {error:#}"));
                    }
                    // 832e4 dispatches the retained mapping directly, without
                    // re-entering Snap Tap or replacing the held lookup pair.
                    let input = replay.input.clone();
                    if let Err(error) = self.execute_resolved(replay, 0, false) {
                        errors.push(format!("Snap Tap replay: {error:#}"));
                    }
                    if let Err(error) = self.notify_snaptap(&input, true) {
                        errors.push(format!("Snap Tap replay observation: {error:#}"));
                    }
                }
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
