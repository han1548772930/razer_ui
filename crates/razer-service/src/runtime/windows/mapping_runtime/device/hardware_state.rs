//! Hardware frame dispatch in the same device actor as filter-driver inputs.
use super::*;
use razer_device::{
    mapping_native_analog::AnalogResolution,
    mapping_native_composite::{AnalogDispatch, AnalogOperation, AnalogSample},
    mapping_native_hid::{
        NativeHidAnalogEvent, NativeHidAnalogPhase, NativeHidFrameStep, NativeHidFraming,
        NativeHidPayload, NativeHidProcessingContext, NativeHidRuntime, decode_native_hid_packet,
    },
    mapping_native_lookup::{AnalogInterrupts, Phase},
};
use std::collections::{BTreeMap, BTreeSet};

#[path = "analog_execution.rs"]
mod analog_execution;

#[derive(Default)]
pub(super) struct Hardware {
    pub config: Option<NativeHidDeviceConfig>,
    ingress: NativeHidRuntime,
    endpoints: BTreeMap<String, hardware::EndpointIdentity>,
    inputs: BTreeMap<u8, AnalogInput>,
    interrupts: AnalogInterrupts,
    interrupt_mappings: BTreeMap<u8, Resolved<Output>>,
    snaptap: razer_device::mapping_native_snaptap::AnalogState,
    notified: BTreeSet<u8>,
}

#[derive(Clone)]
struct AnalogInput {
    input: Input,
    resolution: AnalogResolution,
}

impl Hardware {
    pub(super) fn replace_config(&mut self, config: NativeHidDeviceConfig) {
        // AA588 clears registrations on publication, preserving physical held
        // state and interrupted-key suppression until their real releases.
        self.interrupts.clear_registrations();
        self.interrupt_mappings.clear();
        self.config = Some(config);
    }
}

impl<E: Executor> Worker<E> {
    pub(super) fn hardware_input(&mut self, packet: hardware::Packet) -> Result<()> {
        if let Some(previous) = self.hardware.endpoints.get(&packet.endpoint.path) {
            ensure!(
                previous.instance == packet.endpoint.instance,
                "Hardware endpoint changed physical instance within its registration"
            );
            if previous.generation > packet.endpoint.generation {
                return Ok(());
            }
        }
        self.hardware
            .endpoints
            .insert(packet.endpoint.path.clone(), packet.endpoint);
        // HardwareEventEndpoint is the USB registration path. A descriptor's
        // bluetoothLE flag still controls its inner framing (A8DE0).
        let descriptor = self
            .hardware
            .config
            .as_ref()
            .map(|config| &config.descriptor);
        let decoded =
            decode_native_hid_packet(NativeHidFraming::Usb, descriptor, &packet.input.bytes)?;
        let context = NativeHidProcessingContext {
            analog_enabled: self.mapping_enabled,
            blocked_keys: self.hardware.interrupts.blocked_keys().clone(),
            // FA90E case3 allocates the native stick input unconditionally.
            stick_available: true,
            stick_threshold: descriptor.map_or_else(Default::default, |d| d.stick_threshold),
            actuation_ceiling: descriptor.and_then(|d| d.analog_data.max_actuation_level),
        };
        let mut prepared =
            self.hardware
                .ingress
                .prepare_frame(decoded, &context, packet.input.modifiers);
        let frame = prepared.dispatch().clone();
        self.executor.begin_hardware_frame(false)?;
        let mut errors = Vec::new();
        for &step in &frame.steps {
            match step {
                NativeHidFrameStep::Analog(event) => {
                    if matches!(event.phase, NativeHidAnalogPhase::Actuation { .. })
                        && self
                            .hardware
                            .interrupts
                            .blocked_keys()
                            .contains(&event.key_id)
                    {
                        // An earlier key's callback can interrupt this key after
                        // the frame was decoded. Do not publish its stale candidate.
                        prepared.restore_analog_key(&self.hardware.ingress, event.key_id);
                        continue;
                    }
                    match self.prepare_analog_event(event) {
                        Ok(dispatches) => {
                            prepared.commit_analog_key(&mut self.hardware.ingress, event.key_id);
                            if let Err(error) = self.execute_analog_operations(
                                dispatches,
                                packet.input.time_tick,
                                false,
                            ) {
                                errors
                                    .push(format!("Analog key {} output: {error:#}", event.key_id));
                            }
                            self.finish_analog_event(
                                event,
                                event.phase == NativeHidAnalogPhase::Released,
                            );
                        }
                        Err(error) => {
                            if event.phase == NativeHidAnalogPhase::Released {
                                prepared
                                    .commit_analog_key(&mut self.hardware.ingress, event.key_id);
                                self.finish_analog_event(event, true);
                            } else {
                                prepared.restore_analog_key(&self.hardware.ingress, event.key_id);
                            }
                            errors.push(format!("Analog key {}: {error:#}", event.key_id));
                        }
                    }
                }
                NativeHidFrameStep::AnalogCleanup(key) => {
                    prepared.commit_analog_key(&mut self.hardware.ingress, key);
                    self.hardware.snaptap.physical_missing(key);
                    self.hardware.inputs.remove(&key);
                }
                NativeHidFrameStep::Signed(transition) => {
                    prepared.commit_stick_key(&mut self.hardware.ingress, transition.key_id);
                    if let Err(error) = self.hardware_action(
                        transition.input(),
                        packet.input.time_tick,
                        true,
                        false,
                        1,
                    ) {
                        errors.push(format!("Analog stick: {error:#}"));
                    }
                }
            }
        }
        for event in &frame.input_events {
            if let Err(error) = self.hardware_action(
                event.input.clone(),
                packet.input.time_tick,
                event.mapping_allowed,
                event.notify_input,
                event.repetitions,
            ) {
                errors.push(format!("Hardware input: {error:#}"));
            }
        }
        // A failed transport is reported separately from the observed physical
        // frame. Replaying a successfully handled press would restart actions.
        prepared.commit(&mut self.hardware.ingress);
        if let NativeHidPayload::Analog(report) = &frame.packet.payload {
            let present: BTreeSet<_> = report.levels.keys().copied().collect();
            self.hardware.interrupts.remove_missing(&present);
            self.hardware.inputs.retain(|key, _| present.contains(key));
        }
        if let Err(error) = self.executor.finish_hardware_frame() {
            errors.push(format!("Hardware output completion: {error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn hardware_action(
        &mut self,
        input: Input,
        tick: u64,
        mapping: bool,
        notify: bool,
        repetitions: u8,
    ) -> Result<()> {
        let mut errors = Vec::new();
        if notify {
            // 7f29a continues to lookup/output after 8059c notification;
            // failure of a previous mapping's release is not this input's result.
            if let Err(error) = self.notify_input(&input, tick, true) {
                errors.push(format!("Input interruption: {error:#}"));
            }
        }
        if mapping {
            match self.current_modes() {
                Ok(modes) => {
                    if let Err(error) = self.resolve_digital(input, modes, tick, repetitions, true)
                    {
                        errors.push(format!("{error:#}"));
                    }
                }
                Err(error) => errors.push(format!("Input mapping modes: {error:#}")),
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn prepare_analog_event(
        &mut self,
        event: NativeHidAnalogEvent,
    ) -> Result<Vec<AnalogOperation>> {
        let config = self
            .hardware
            .config
            .as_ref()
            .context("Analog report has no published device descriptor")?;
        let input = self
            .hardware
            .inputs
            .get(&event.key_id)
            .map(|observed| observed.input.clone())
            .or_else(|| config.analog_input(event.key_id))
            .context("Analog report key has no source-defined input identity")?;
        self.hardware
            .inputs
            .entry(event.key_id)
            .or_insert_with(|| AnalogInput {
                input: input.clone(),
                resolution: event.resolution,
            });
        let mut input = input;
        let modes = self.current_modes()?;
        let phase = if event.level > event.previous_level {
            Phase::Press
        } else {
            Phase::Release
        };
        let parameters = config.analog_parameters(event.key_id, modes.hypershift, phase);
        let maximum =
            config
                .descriptor
                .analog_data
                .max_actuation_level
                .unwrap_or(match event.resolution {
                    AnalogResolution::V1 => 255,
                    AnalogResolution::V2 => u16::MAX,
                });
        let allow_passthrough = config.descriptor.analog_passthrough();
        // 7E05C retains the selected mapping and interrupt metadata before its
        // initial callback, even when the first physical sample is zero.
        if matches!(event.phase, NativeHidAnalogPhase::Actuation { first: true }) {
            input = input.pressed();
            input.modifiers = event.modifiers;
            if let Some(pair) = self.lookup.analog_mapping(&input, modes, allow_passthrough) {
                input = pair.make.input.pressed();
                self.hardware.interrupts.register(event.key_id, &input);
                if input.interruptible {
                    self.hardware
                        .interrupt_mappings
                        .insert(event.key_id, pair.make);
                }
            }
            if let Some(observed) = self.hardware.inputs.get_mut(&event.key_id) {
                observed.input = input.clone();
            }
            if let Err(error) = self.notify_analog(event.key_id, event.level, &input, None, true) {
                self.emit(Event::Failure(format!(
                    "Analog initial callback: {error:#}"
                )));
            }
            if event.level == event.previous_level {
                return Ok(Vec::new());
            }
        }
        if event.phase == NativeHidAnalogPhase::Released {
            // 7A916 assigns current modifiers before both the raw release
            // callback and mapping lookup, and always forces level 0.
            let mut released = self
                .hardware
                .config
                .as_ref()
                .and_then(|config| config.analog_input(event.key_id))
                .context("Released analog key has no descriptor input")?
                .released();
            released.modifiers = event.modifiers;
            if let Err(error) = self.notify_analog(event.key_id, 0, &released, None, true) {
                self.emit(Event::Failure(format!(
                    "Analog release callback: {error:#}"
                )));
            }
            input = released;
        } else {
            input = if phase == Phase::Press {
                input.pressed()
            } else {
                input.released()
            };
            input.modifiers = event.modifiers;
            if event.level != event.previous_level {
                if let Err(error) = self.notify_analog(
                    event.key_id,
                    event.level,
                    &input,
                    None,
                    event.level == maximum,
                ) {
                    self.emit(Event::Failure(format!("Analog level callback: {error:#}")));
                }
            }
        }
        input.modifiers = event.modifiers;
        if let Some(observed) = self.hardware.inputs.get_mut(&event.key_id) {
            observed.input = input.clone();
        }
        let pair = self.lookup.analog_mapping(&input, modes, allow_passthrough);
        let dispatches = if let Some(pair) = pair {
            CompositeState::prepare_analog(
                AnalogSample {
                    key_id: event.key_id,
                    phase,
                    previous: event.previous_level,
                    value: event.level,
                    maximum,
                },
                parameters,
                &pair,
            )
        } else {
            Vec::new()
        };
        Ok(dispatches)
    }

    fn finish_analog_event(&mut self, event: NativeHidAnalogEvent, physical_missing: bool) {
        if event.phase == NativeHidAnalogPhase::Released {
            if let Some(observed) = self.hardware.inputs.get(&event.key_id) {
                self.lookup
                    .release_analog_mapping(&observed.input.identity());
            }
            if physical_missing {
                self.hardware.snaptap.physical_missing(event.key_id);
                self.hardware.inputs.remove(&event.key_id);
            }
        }
    }

    fn execute_analog_dispatches(
        &mut self,
        dispatches: Vec<AnalogDispatch>,
        tick: u64,
        cleanup: bool,
    ) -> Result<()> {
        let mut errors = Vec::new();
        for dispatch in dispatches {
            let result = match dispatch {
                AnalogDispatch::Action(action) => {
                    self.execute_dispatches(vec![action], tick, cleanup)
                }
                AnalogDispatch::Analog {
                    resolved,
                    actuation,
                } => self.executor.analog(resolved, actuation),
            };
            if let Err(error) = result {
                errors.push(format!("{error:#}"));
            }
        }
        errors.extend(self.composites.take_diagnostics());
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub(super) fn release_hardware(&mut self) -> Result<()> {
        let tick = razer_platform::event_clock::tick()?;
        self.executor.begin_hardware_frame(true)?;
        let lookup = &self.lookup;
        let dispatches = self
            .composites
            .drain_analog_dispatches(|id| lookup.held_by_stateless(id), self.began.elapsed());
        let mut errors = Vec::new();
        match dispatches {
            Ok(dispatches) => {
                if let Err(error) = self.execute_analog_dispatches(dispatches, tick, true) {
                    errors.push(format!("{error:#}"));
                }
            }
            Err(error) => errors.push(format!("{error:#}")),
        }
        if let Err(error) = self.executor.finish_hardware_frame() {
            errors.push(format!("{error:#}"));
        }
        self.lookup.clear_analog_mappings();
        self.hardware.ingress.reset_after_disconnect();
        self.hardware.inputs.clear();
        self.hardware.interrupts.clear();
        self.hardware.interrupt_mappings.clear();
        self.hardware.snaptap.clear();
        self.hardware.notified.clear();
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub(super) fn interrupt_hardware(
        &mut self,
        input: &Input,
        tick: u64,
        batched: bool,
    ) -> Result<()> {
        let keys = self.hardware.interrupts.notify(input);
        self.release_analog_keys(keys, input.modifiers, tick, batched, true)
    }

    fn release_analog_keys(
        &mut self,
        keys: Vec<u8>,
        modifiers: u32,
        tick: u64,
        batched: bool,
        stop_macro: bool,
    ) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        if !batched {
            self.executor.begin_hardware_frame(false)?;
        }
        let mut errors = Vec::new();
        for key in keys {
            if let Some((observed, previous)) = self
                .hardware
                .inputs
                .get(&key)
                .cloned()
                .zip(self.hardware.ingress.tracked_level(key))
            {
                self.hardware.interrupts.block(key);
                let event = NativeHidAnalogEvent {
                    key_id: key,
                    resolution: observed.resolution,
                    raw_level: 0,
                    level: 0,
                    previous_level: previous,
                    modifiers,
                    phase: NativeHidAnalogPhase::Released,
                };
                match self.prepare_analog_event(event) {
                    Ok(dispatches) => {
                        if let Err(error) = self.execute_analog_operations(dispatches, tick, false)
                        {
                            errors.push(format!("Analog interrupt output: {error:#}"));
                        }
                        self.finish_analog_event(event, false);
                    }
                    Err(error) => errors.push(format!("Analog interrupt: {error:#}")),
                }
            }
            if stop_macro {
                // 8e394 retains the interrupt registration after physical release.
                // A running macro can still be canceled without a held analog key.
                if let Some(mapping) =
                    self.hardware
                        .interrupt_mappings
                        .get(&key)
                        .filter(|mapping| {
                            matches!(
                                mapping.output,
                                razer_device::mapping_native_lookup::ResolvedOutput::Mapped(
                                    Output::Macro { .. }
                                )
                            )
                        })
                {
                    if let Err(error) = self.executor.interrupt_macro(mapping) {
                        errors.push(format!("Interrupted macro cancellation: {error:#}"));
                    }
                }
            }
        }
        if !batched {
            if let Err(error) = self.executor.finish_hardware_frame() {
                errors.push(format!("{error:#}"));
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    pub(super) fn hypershift_hardware_released(&mut self) -> Result<()> {
        if self
            .hardware
            .config
            .as_ref()
            .is_none_or(|config| config.descriptor.analog_format.is_none())
        {
            return Ok(());
        }
        let keys = self
            .composites
            .active_analog_keys()
            .into_iter()
            .filter_map(|(key, identity)| {
                self.lookup
                    .analog_held_by_stateless(&identity)
                    .is_some_and(|pair| !pair.make.is_passthrough())
                    .then_some(key)
            })
            .collect::<Vec<_>>();
        for &key in &keys {
            self.hardware.interrupts.block(key);
        }
        let modifiers = driver::capture_modifiers().source_bits();
        self.release_analog_keys(
            keys,
            modifiers,
            razer_platform::event_clock::tick()?,
            false,
            false,
        )
    }
}

#[cfg(test)]
#[path = "hardware_state_tests.rs"]
mod tests;
