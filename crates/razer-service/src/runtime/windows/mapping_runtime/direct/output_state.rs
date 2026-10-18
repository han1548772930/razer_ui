//! Device-owned contributions shared by driver and hardware input handlers.
use anyhow::{Result, ensure};
use razer_device::{
    mapping_native_analog::NativeAnalogOutput,
    mapping_native_digital::{NativeDigitalMappingState, NativeDigitalOutput, NativeMappingDelta},
    mapping_native_lookup::InputIdentity,
    mapping_native_output::{
        DriverInputReport, GamepadOutputState, MappingOutputState, changed_driver_inputs,
        encode_driver_input,
    },
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct OutputState {
    digital: NativeDigitalMappingState,
    identities: BTreeMap<InputIdentity, u64>,
    analog: BTreeMap<InputIdentity, NativeAnalogOutput>,
    next_identity: u64,
    frame_start: Option<MappingOutputState>,
    gamepad_transition: Option<(GamepadOutputState, GamepadOutputState)>,
}

impl OutputState {
    fn identity(&mut self, key: &InputIdentity) -> String {
        let id = self.identities.entry(key.clone()).or_insert_with(|| {
            self.next_identity += 1;
            self.next_identity
        });
        format!("input:{id}")
    }

    // 79E9E snapshots both output banks before processing the complete HID
    // frame and calls AF2AE/AFB14 afterward. Intermediate key contributions
    // must not become separate joystick/gamepad writes.
    pub fn begin_frame(&mut self) -> Result<()> {
        ensure!(
            self.frame_start.is_none(),
            "Hardware output frame is already active"
        );
        self.frame_start = Some(self.digital.output());
        Ok(())
    }

    pub fn finish_frame(&mut self) -> Vec<DriverInputReport> {
        self.frame_start.take().map_or_else(Vec::new, |previous| {
            self.record_gamepad(previous, self.digital.output());
            changed_driver_inputs(previous, self.digital.output())
                .into_iter()
                .map(encode_driver_input)
                .collect()
        })
    }

    pub fn take_gamepad_transition(&mut self) -> Option<(GamepadOutputState, GamepadOutputState)> {
        self.gamepad_transition.take()
    }
    fn record_gamepad(&mut self, previous: MappingOutputState, current: MappingOutputState) {
        if previous.gamepad != current.gamepad {
            self.gamepad_transition = Some((previous.gamepad, current.gamepad));
        }
    }
    fn reports(&mut self, delta: NativeMappingDelta) -> Vec<DriverInputReport> {
        if self.frame_start.is_some() {
            Vec::new()
        } else {
            self.record_gamepad(delta.previous, delta.desired);
            delta.encoded_reports()
        }
    }

    pub fn digital(
        &mut self,
        key: &InputIdentity,
        output: NativeDigitalOutput,
    ) -> Result<Vec<DriverInputReport>> {
        let identity = self.identity(key);
        let delta = self.digital.apply_digital(&identity, output)?;
        Ok(self.reports(delta))
    }

    pub fn combination(
        &mut self,
        key: &str,
        output: NativeDigitalOutput,
    ) -> Result<Vec<DriverInputReport>> {
        let delta = self
            .digital
            .apply_digital(&format!("combination:{key}"), output)?;
        Ok(self.reports(delta))
    }

    pub fn analog(
        &mut self,
        key: &InputIdentity,
        output: &NativeAnalogOutput,
        actuation: u16,
    ) -> Vec<DriverInputReport> {
        let identity = self.identity(key);
        if actuation != 0 {
            self.analog.insert(key.clone(), output.clone());
        } else {
            self.analog.remove(key);
        }
        let delta = self.digital.apply_analog(&identity, output, actuation);
        self.reports(delta)
    }

    pub fn take_analog(&mut self) -> BTreeMap<InputIdentity, NativeAnalogOutput> {
        std::mem::take(&mut self.analog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::{
        mapping_native_analog::{AnalogResolution, GamepadAxis, GamepadCurve, GamepadPoint},
        mapping_native_lookup::{Input, InputKind},
    };

    fn key(scancode: u16) -> InputIdentity {
        Input::new(InputKind::AnalogKey { scancode, flag: 0 }).identity()
    }

    fn trigger() -> NativeAnalogOutput {
        NativeAnalogOutput::Gamepad(
            GamepadCurve::compile(
                AnalogResolution::V1,
                GamepadAxis::LeftTrigger,
                &[
                    GamepadPoint {
                        actuation: 0,
                        value: 0,
                    },
                    GamepadPoint {
                        actuation: 255,
                        value: 255,
                    },
                ],
            )
            .unwrap(),
        )
    }

    #[test]
    fn gamepad_observation_uses_the_same_aggregate_as_the_submitted_frame() {
        let mut state = OutputState::default();
        state.begin_frame().unwrap();
        state.analog(&key(1), &trigger(), 64);
        state.analog(&key(2), &trigger(), 128);
        assert!(state.take_gamepad_transition().is_none());
        let reports = state.finish_frame();
        let (previous, current) = state.take_gamepad_transition().unwrap();
        assert_eq!(previous.left_trigger, 0);
        assert_eq!(current.left_trigger, 128);
        assert_eq!(
            reports,
            vec![encode_driver_input(
                razer_device::mapping_native_output::DriverInput::Gamepad(current)
            )]
        );
        assert!(state.take_gamepad_transition().is_none());
        state.begin_frame().unwrap();
        state.analog(&key(1), &trigger(), 90);
        assert!(state.finish_frame().is_empty());
        assert!(state.take_gamepad_transition().is_none());
        state.analog(&key(2), &trigger(), 0);
        let (previous, current) = state.take_gamepad_transition().unwrap();
        assert_eq!((previous.left_trigger, current.left_trigger), (128, 90));
    }

    #[test]
    fn hardware_frame_combines_analog_and_digital_before_one_driver_report() {
        let mut state = OutputState::default();
        state.begin_frame().unwrap();
        assert!(state.analog(&key(1), &trigger(), 64).is_empty());
        assert!(state.analog(&key(2), &trigger(), 128).is_empty());
        assert!(
            state
                .digital(
                    &key(3),
                    NativeDigitalOutput::GamepadButtons {
                        mask: 0x1000,
                        flag: 0
                    }
                )
                .unwrap()
                .is_empty()
        );
        let reports = state.finish_frame();
        let mut expected = [0; 32];
        expected[4] = 12;
        expected[11] = 0x10;
        // Trigger contributions select the maximum; they are not summed.
        expected[12] = 128;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].bytes(), &expected);

        state.begin_frame().unwrap();
        assert!(state.analog(&key(2), &trigger(), 0).is_empty());
        let reports = state.finish_frame();
        expected[12] = 64;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].bytes(), &expected);

        state.begin_frame().unwrap();
        assert!(state.analog(&key(1), &trigger(), 0).is_empty());
        let reports = state.finish_frame();
        expected[12] = 0;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].bytes(), &expected);
        assert!(state.take_analog().is_empty());
    }

    #[test]
    fn net_unchanged_frame_sends_nothing_and_digital_input_remains_immediate() {
        let mut state = OutputState::default();
        state.begin_frame().unwrap();
        assert!(state.begin_frame().is_err());
        state.analog(&key(1), &trigger(), 90);
        state.analog(&key(1), &trigger(), 0);
        assert!(state.finish_frame().is_empty());
        let reports = state
            .digital(
                &key(2),
                NativeDigitalOutput::GamepadButtons { mask: 1, flag: 0 },
            )
            .unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].bytes()[10], 1);
        assert!(state.finish_frame().is_empty());
    }
}
