//! Source-ordered analog continuations and Snap Tap replay on the device actor.
use super::*;
use razer_device::{
    macro_runtime::Event as MacroEvent,
    mapping_native_composite::{AnalogStep, AnalogTransition},
    mapping_native_lookup::ResolvedOutput,
    mapping_native_snaptap::{AnalogEffect, AnalogTransition as Arbitration, Policy},
};

impl<E: Executor> Worker<E> {
    pub(super) fn execute_analog_operations(
        &mut self,
        operations: Vec<AnalogOperation>,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        let mut errors = Vec::new();
        for operation in operations {
            let result = match self.composites.advance_analog(operation) {
                Ok(Some(AnalogStep::Continuous {
                    key_id,
                    resolved,
                    actuation,
                    notify,
                })) => {
                    if notify {
                        if let Err(error) = self.notify_analog_mapping(key_id, actuation, &resolved)
                        {
                            errors.push(format!("Analog callback: {error:#}"));
                        }
                        if let Err(error) = self.notify_analog_input(key_id, &resolved.input, tick)
                        {
                            errors.push(format!("Analog input notification: {error:#}"));
                        }
                    }
                    self.executor.analog(resolved, actuation)
                }
                Ok(Some(AnalogStep::Transition(current))) => {
                    self.analog_transition(current, tick, cleanup)
                }
                Ok(Some(AnalogStep::Depth { key_id, resolved })) => {
                    if let Some(group) = resolved.selectors.snap_tap {
                        let policy = self.analog_policy(group);
                        if let Some(transition) = self.hardware.snaptap.begin_depth(
                            key_id,
                            group,
                            policy,
                            resolved.input.phase(),
                        ) {
                            self.analog_arbitration(transition, None, tick, cleanup)
                        } else {
                            Ok(())
                        }
                    } else {
                        Ok(())
                    }
                }
                Ok(None) => Ok(()),
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                errors.push(format!("{error:#}"));
            }
        }
        errors.extend(self.composites.take_diagnostics());
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn analog_policy(&self, group: u32) -> Policy {
        self.hardware
            .config
            .as_ref()
            .and_then(|config| config.profile.snaptaps.get(&group).copied())
            .unwrap_or(Policy::LastInput)
    }

    fn analog_transition(
        &mut self,
        current: AnalogTransition,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        let resolved = &current.resolved;
        let excluded = matches!(
            resolved.output,
            ResolvedOutput::Passthrough(_)
                | ResolvedOutput::Mapped(
                    Output::Cascade(_)
                        | Output::ModTap { .. }
                        | Output::Dpad(_)
                        | Output::Hypershift { .. }
                        | Output::Event(MacroEvent::Clipboard(_))
                )
        );
        if !excluded && let Some(group) = resolved.selectors.snap_tap {
            let transition = self.hardware.snaptap.begin(
                current.key_id,
                group,
                resolved.selectors.snap_tap_prioritize,
                self.analog_policy(group),
                resolved.input.phase(),
            );
            match transition {
                Ok(Some(transition)) => {
                    return self.analog_arbitration(transition, Some(current), tick, cleanup);
                }
                Ok(None) => {}
                Err(error) => self.emit(Event::Failure(format!("{error:#}"))),
            }
        }
        self.analog_current(current, tick, cleanup)
    }

    fn analog_arbitration(
        &mut self,
        mut transition: Arbitration,
        current: Option<AnalogTransition>,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        let mut errors = Vec::new();
        loop {
            let ingress = &self.hardware.ingress;
            let effect = self
                .hardware
                .snaptap
                .next(&mut transition, |key| ingress.tracked_level(key));
            let Some(effect) = effect else { break };
            let result = match effect {
                AnalogEffect::Current { execute } => {
                    if let Some(current) = &current {
                        if execute {
                            if let Err(error) = self.analog_current(current.clone(), tick, cleanup)
                            {
                                errors.push(format!("Analog Snap Tap output: {error:#}"));
                            }
                        }
                        // 99d6c also notifies an inactive retained release whose
                        // callback is empty; 96450 never emits Current.
                        self.notify_snaptap(&current.resolved.input, false)
                    } else {
                        Ok(())
                    }
                }
                AnalogEffect::Pause => self.executor.pause_mapping_replay(),
                AnalogEffect::Replay { key_id, phase } => {
                    self.replay_analog(key_id, phase, tick, cleanup)
                }
            };
            if let Err(error) = result {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn replay_analog(&mut self, key: u8, phase: Phase, tick: u64, cleanup: bool) -> Result<()> {
        // 9ac00 first requires +1576 activity and 92792's retained lookup pair.
        // A missing mapping does not change the arbitration active/latch bytes.
        let Some(identity) = self.composites.active_analog_identity(key) else {
            return Ok(());
        };
        let Some(pair) = self.lookup.analog_held_by_stateless(identity) else {
            return Ok(());
        };
        if !self.hardware.snaptap.replayed(key, phase) {
            return Ok(());
        }
        let resolved = if phase == Phase::Press {
            pair.make
        } else {
            pair.release
        };
        let input = resolved.input.clone();
        let mut errors = Vec::new();
        // Replay uses 8059c rather than 940d8's physical-key deduplication.
        if let Err(error) = self.notify_input(&input, tick, true) {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.execute_resolved(resolved, 0, cleanup) {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.notify_snaptap(&input, true) {
            errors.push(format!("{error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn analog_current(
        &mut self,
        current: AnalogTransition,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(error) =
            self.notify_analog_mapping(current.key_id, current.actuation, &current.resolved)
        {
            errors.push(format!("Analog callback: {error:#}"));
        }
        if let Err(error) = self.notify_analog_input(current.key_id, &current.resolved.input, tick)
        {
            errors.push(format!("Analog input notification: {error:#}"));
        }
        let lookup = &self.lookup;
        match self.composites.dispatch_analog(
            current,
            |id| lookup.held_by_stateless(id),
            self.began.elapsed(),
        ) {
            Ok(actions) => {
                if let Err(error) = self.execute_dispatches(actions, 0, cleanup) {
                    errors.push(format!("Analog output: {error:#}"));
                }
            }
            Err(error) => errors.push(format!("Analog continuation: {error:#}")),
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn notify_analog_mapping(
        &mut self,
        key: u8,
        level: u16,
        resolved: &Resolved<Output>,
    ) -> Result<()> {
        match &resolved.output {
            ResolvedOutput::Mapped(output) => {
                self.notify_analog(key, level, &resolved.input, Some(output), true)
            }
            ResolvedOutput::Passthrough(input) => {
                let output = Output::from_passthrough(input)?;
                self.notify_analog(key, level, &resolved.input, Some(&output), true)
            }
        }
    }

    fn notify_analog_input(&mut self, key: u8, input: &Input, tick: u64) -> Result<()> {
        if self.input_observation_enabled() {
            let changed = if input.phase() == Phase::Press {
                self.hardware.notified.insert(key)
            } else {
                self.hardware.notified.remove(&key)
            };
            if changed {
                self.emit(Event::Input {
                    input: input.clone(),
                    tick: razer_platform::event_clock::tick()?,
                    redirected: false,
                    observer: self.input_observer,
                });
            }
        }
        self.interrupt_input(input, tick, true)
    }
}
