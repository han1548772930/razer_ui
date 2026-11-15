//! Profile-scoped settings in the mounted OBM loader's phase order.
use super::{Reading, retain};
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange,
    keyboard_knob::{self, CurrentKnobMode, KnobConfiguration},
    keyboard_onboard_inventory::Features,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollingRate {
    pub raw_code: u8,
    pub hertz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Debounce {
    pub make_debounce_ms: u8,
    pub make_mask_ms: u8,
    pub break_debounce_ms: u8,
    pub break_mask_ms: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModTap {
    pub key_id: u8,
    pub raw_enabled: u8,
    pub key_code: u8,
    pub hold_time: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnapTap {
    pub raw_enabled: u8,
    pub number_of_pairs: u8,
    pub pairs: [[u8; 3]; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SingleKeySnapTap {
    pub raw_enabled: u8,
    pub number_of_keys: u8,
    pub keys: Vec<[u8; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InterruptKeys {
    pub raw_enabled: u8,
    pub raw_mode: u8,
    pub key_ids: [u8; 4],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DualKeypressPriority {
    pub raw_enabled: u8,
    pub number_of_keys: u8,
    pub key_ids: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Observation {
    pub polling_rate: Reading<PollingRate>,
    pub bottom_deadzone: Reading<u8>,
    pub debounce: Reading<Debounce>,
    pub knob_configuration: Reading<KnobConfiguration>,
    pub knob_current: Reading<CurrentKnobMode>,
    pub mod_tap: Reading<Vec<ModTap>>,
    pub snap_tap: Reading<SnapTap>,
    pub single_key_snap_tap: Reading<SingleKeySnapTap>,
    pub interrupt_dynamic_keystroke: Reading<InterruptKeys>,
    pub interrupt_macro: Reading<InterruptKeys>,
    pub dual_keypress_priority: Reading<DualKeypressPriority>,
}

impl Observation {
    pub(super) fn unobserved() -> Self {
        Self {
            polling_rate: Reading::Unobserved,
            bottom_deadzone: Reading::Unobserved,
            debounce: Reading::Unobserved,
            knob_configuration: Reading::Unobserved,
            knob_current: Reading::Unobserved,
            mod_tap: Reading::Unobserved,
            snap_tap: Reading::Unobserved,
            single_key_snap_tap: Reading::Unobserved,
            interrupt_dynamic_keystroke: Reading::Unobserved,
            interrupt_macro: Reading::Unobserved,
            dual_keypress_priority: Reading::Unobserved,
        }
    }

    pub(super) fn validate(&self, features: Features) -> Result<()> {
        fn field<T>(value: &Reading<T>, mounted: bool) -> Result<()> {
            if mounted {
                value.validate_attempt()
            } else {
                ensure!(
                    matches!(value, Reading::Unobserved),
                    "Unmounted OBM setting was read"
                );
                Ok(())
            }
        }
        field(&self.polling_rate, features.high_speed_polling_rate)?;
        if let Some(rate) = self.polling_rate.observed() {
            ensure!(
                rate.hertz == polling_hertz(rate.raw_code),
                "OBM polling code differs"
            );
        }
        field(&self.bottom_deadzone, features.bottom_dead_zone)?;
        field(&self.debounce, features.debounce_time)?;
        field(&self.knob_configuration, features.knob_id.is_some())?;
        field(
            &self.knob_current,
            self.knob_configuration.observed().is_some(),
        )?;
        field(&self.mod_tap, features.mod_tap)?;
        if let Some(rows) = self.mod_tap.observed() {
            super::validate_returned_keys(&MOD_TAP_KEYS, rows.iter().map(|row| row.key_id))?;
        }
        field(&self.snap_tap, features.snap_tap)?;
        field(&self.single_key_snap_tap, features.single_key_snap_tap)?;
        if let Some(value) = self.single_key_snap_tap.observed() {
            ensure!(
                value.number_of_keys <= 4 && value.keys.len() == usize::from(value.number_of_keys),
                "OBM single-key Snap Tap count differs"
            );
        }
        field(
            &self.interrupt_dynamic_keystroke,
            features.interruptible_dynamic_keystroke,
        )?;
        field(&self.interrupt_macro, features.interruptible_macro)?;
        field(
            &self.dual_keypress_priority,
            features.dual_keypress_priority,
        )?;
        if let Some(value) = self.dual_keypress_priority.observed() {
            ensure!(
                value.key_ids.len() == usize::from(value.number_of_keys.min(4)),
                "OBM dual-keypress count differs"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) enum Phase {
    PollingRate,
    BottomDeadzone,
    Debounce,
    Knob,
    ModTap,
    SnapTap,
    SingleKeySnapTap,
    InterruptDynamicKeystroke,
    InterruptMacro,
    DualKeypressPriority,
}

pub(super) const BEFORE_ASSIGNMENTS: [Phase; 3] =
    [Phase::PollingRate, Phase::BottomDeadzone, Phase::Debounce];
pub(super) const BEFORE_KILL_SWITCH: [Phase; 2] = [Phase::Knob, Phase::ModTap];
pub(super) const AFTER_KILL_SWITCH: [Phase; 5] = [
    Phase::SnapTap,
    Phase::SingleKeySnapTap,
    Phase::InterruptDynamicKeystroke,
    Phase::InterruptMacro,
    Phase::DualKeypressPriority,
];
const MOD_TAP_KEYS: [u8; 4] = [62, 64, 129, 57];

fn polling_hertz(code: u8) -> Option<u32> {
    match code {
        1 => Some(8000),
        2 => Some(4000),
        4 => Some(2000),
        8 => Some(1000),
        16 => Some(500),
        32 => Some(250),
        64 => Some(125),
        _ => None,
    }
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    payload: &[u8],
    slot: u8,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    guard()?;
    let bytes = feature_exchange::exchange(device, cap, command, payload, next()?, guard)?;
    ensure!(
        bytes.len() == usize::from(command[0]) && bytes.first() == Some(&slot),
        "OBM setting response size or profile differs"
    );
    Ok(bytes)
}

pub(super) fn read_phase(
    phase: Phase,
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    features: Features,
    slot: u8,
    observation: &mut Observation,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<()> {
    match phase {
        Phase::PollingRate if features.high_speed_polling_rate => {
            observation.polling_rate = retain(
                exchange(device, cap, [2, 0, 192], &[slot, 0], slot, next, guard).map(|bytes| {
                    PollingRate {
                        raw_code: bytes[1],
                        hertz: polling_hertz(bytes[1]),
                    }
                }),
                guard,
            )?;
        }
        Phase::BottomDeadzone if features.bottom_dead_zone => {
            observation.bottom_deadzone = retain(
                exchange(device, cap, [2, 2, 168], &[slot, 0], slot, next, guard)
                    .map(|bytes| bytes[1]),
                guard,
            )?;
        }
        Phase::Debounce if features.debounce_time => {
            observation.debounce = retain(
                exchange(
                    device,
                    cap,
                    [5, 2, 149],
                    &[slot, 0, 0, 0, 0],
                    slot,
                    next,
                    guard,
                )
                .map(|bytes| Debounce {
                    make_debounce_ms: bytes[1],
                    make_mask_ms: bytes[2],
                    break_debounce_ms: bytes[3],
                    break_mask_ms: bytes[4],
                }),
                guard,
            )?;
        }
        Phase::Knob if features.knob_id.is_some() => {
            let knob_id = features.knob_id.unwrap();
            observation.knob_configuration = retain(
                exchange(
                    device,
                    cap,
                    [50, 3, 146],
                    &[slot, knob_id],
                    slot,
                    next,
                    guard,
                )
                .and_then(|bytes| keyboard_knob::parse_configuration(&bytes, slot, knob_id)),
                guard,
            )?;
            if observation.knob_configuration.observed().is_some() {
                observation.knob_current = retain(
                    exchange(
                        device,
                        cap,
                        [8, 3, 147],
                        &[slot, knob_id],
                        slot,
                        next,
                        guard,
                    )
                    .and_then(|bytes| keyboard_knob::parse_current(&bytes, slot, knob_id)),
                    guard,
                )?;
            }
        }
        Phase::ModTap if features.mod_tap => {
            let result = (|| {
                let mut payload = [0; 17];
                payload[0] = slot;
                for (index, key) in MOD_TAP_KEYS.iter().enumerate() {
                    payload[1 + 4 * index] = *key;
                }
                guard()?;
                let bytes = feature_exchange::exchange_with_report_data_size(
                    device,
                    cap,
                    [80, 2, 155],
                    &payload,
                    next()?,
                    17,
                    guard,
                )?;
                ensure!(
                    bytes.len() == 17 && bytes[0] == slot,
                    "OBM Mod Tap response identity differs"
                );
                let rows = bytes[1..]
                    .chunks_exact(4)
                    .map(|row| ModTap {
                        key_id: row[0],
                        raw_enabled: row[1],
                        key_code: row[2],
                        hold_time: row[3],
                    })
                    .collect::<Vec<_>>();
                super::validate_returned_keys(&MOD_TAP_KEYS, rows.iter().map(|row| row.key_id))?;
                Ok(rows)
            })();
            observation.mod_tap = retain(result, guard)?;
        }
        Phase::SnapTap if features.snap_tap => {
            observation.snap_tap = retain(
                exchange(device, cap, [15, 2, 167], &[slot], slot, next, guard).map(|bytes| {
                    SnapTap {
                        raw_enabled: bytes[2],
                        number_of_pairs: bytes[1],
                        pairs: std::array::from_fn(|i| {
                            bytes[3 + i * 3..6 + i * 3].try_into().unwrap()
                        }),
                    }
                }),
                guard,
            )?;
        }
        Phase::SingleKeySnapTap if features.single_key_snap_tap => {
            observation.single_key_snap_tap = retain(
                exchange(device, cap, [11, 2, 172], &[slot], slot, next, guard).and_then(|bytes| {
                    ensure!(
                        bytes[1] <= 4,
                        "OBM single-key Snap Tap count exceeds payload"
                    );
                    Ok(SingleKeySnapTap {
                        raw_enabled: bytes[2],
                        number_of_keys: bytes[1],
                        keys: bytes[3..3 + 2 * usize::from(bytes[1])]
                            .chunks_exact(2)
                            .map(|row| [row[0], row[1]])
                            .collect(),
                    })
                }),
                guard,
            )?;
        }
        Phase::InterruptDynamicKeystroke if features.interruptible_dynamic_keystroke => {
            observation.interrupt_dynamic_keystroke =
                read_interrupt(device, cap, slot, 174, next, guard)?;
        }
        Phase::InterruptMacro if features.interruptible_macro => {
            observation.interrupt_macro = read_interrupt(device, cap, slot, 173, next, guard)?;
        }
        Phase::DualKeypressPriority if features.dual_keypress_priority => {
            observation.dual_keypress_priority = retain(
                exchange(device, cap, [7, 2, 177], &[slot], slot, next, guard).map(|bytes| {
                    DualKeypressPriority {
                        raw_enabled: bytes[2],
                        number_of_keys: bytes[1],
                        key_ids: bytes[3..3 + usize::from(bytes[1].min(4))].to_vec(),
                    }
                }),
                guard,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn read_interrupt(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    slot: u8,
    command_id: u8,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<Reading<InterruptKeys>> {
    retain(
        exchange(device, cap, [7, 2, command_id], &[slot], slot, next, guard).map(|bytes| {
            InterruptKeys {
                raw_enabled: bytes[1],
                raw_mode: bytes[2],
                key_ids: bytes[3..7].try_into().unwrap(),
            }
        }),
        guard,
    )
}
