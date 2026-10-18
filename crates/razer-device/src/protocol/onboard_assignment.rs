//! Current onboard assignment wire fields and common output encoders.
use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonAssignment {
    pub key_id: u8,
    pub mode: u8,
    pub function_id: u8,
    pub data_size: u8,
    pub data: [u8; 5],
}
impl ButtonAssignment {
    pub fn payload(self, slot: u8) -> anyhow::Result<Vec<u8>> {
        payload(
            slot,
            self.key_id,
            self.mode,
            self.function_id,
            self.data_size,
            &self.data,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroPlayback {
    Once,
    NTimes(u8),
    Held,
    Toggle,
    Sequence,
}

/// The macro ID must come from firmware metadata, including newly uploaded
/// macros confirmed by the storage owner. A local GUID is never an ID.
pub(crate) fn macro_function(id: u16, playback: MacroPlayback) -> (u8, u8, [u8; 5]) {
    let [high, low] = id.to_be_bytes();
    let mut data = [high, low, 0, 0, 0];
    let (function, size) = match playback {
        MacroPlayback::Once => {
            data[2] = 1;
            (3, 3)
        }
        MacroPlayback::NTimes(count) => {
            data[2] = count;
            (3, 3)
        }
        MacroPlayback::Held => (4, 2),
        MacroPlayback::Toggle => (5, 2),
        MacroPlayback::Sequence => (15, 2),
    };
    (function, size, data)
}

pub(crate) const SET_SINGLE_KEY_SNAP_TAP: [u8; 3] = [11, 2, 44];

pub(crate) fn interrupt_keys_payload(
    slot: u8,
    dynamic_keystroke: bool,
    enabled: bool,
    mode: u8,
    keys: &[u8],
) -> anyhow::Result<([u8; 3], Vec<u8>)> {
    ensure!(
        slot > 0 && mode <= 1 && keys.len() <= 4,
        "Interrupt key slot/mode/count is invalid"
    );
    let mut payload = vec![slot, u8::from(enabled), mode];
    payload.extend_from_slice(keys);
    payload.resize(7, 0);
    Ok(([7, 2, if dynamic_keystroke { 46 } else { 45 }], payload))
}

pub(crate) fn single_key_snap_tap_payload(
    class_id: u8,
    enabled: bool,
    keys: &[[u8; 2]],
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        keys.len() <= 4,
        "Single-key Snap Tap exceeds current source bounds"
    );
    let mut payload = vec![class_id, keys.len() as u8, u8::from(enabled)];
    for key in keys {
        payload.extend_from_slice(key);
    }
    payload.resize(11, 0);
    Ok(payload)
}

pub(crate) fn payload(
    slot: u8,
    key_id: u8,
    mode: u8,
    function_id: u8,
    data_size: u8,
    data: &[u8; 5],
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        slot > 0 && mode <= 1 && data_size <= 5,
        "Keyboard assignment slot/mode/data size is invalid"
    );
    let mut payload = vec![slot, key_id, mode, function_id, data_size];
    payload.extend_from_slice(data);
    Ok(payload)
}

/// Current26803's keyboard output encoder. NKRO modifier keys use their
/// modifier bit with HID zero; mouse callers retain the ordinary HID form.
pub(crate) fn keyboard_function(
    hid: u8,
    modifiers: u8,
    turbo: bool,
    rate: Option<u16>,
    nkey_rollover: bool,
) -> anyhow::Result<(u8, u8, [u8; 5])> {
    let mut data = [0; 5];
    if nkey_rollover && (224..=231).contains(&hid) {
        data[0] = 1 << (hid - 224);
    } else {
        data[0] = modifiers;
        data[1] = hid;
    }
    if turbo {
        let rate = rate.unwrap_or(7);
        ensure!(rate > 0, "Turbo keys-per-second must be greater than zero");
        data[2..4].copy_from_slice(&(1000 / rate).to_be_bytes());
        Ok((13, 4, data))
    } else {
        Ok((2, 2, data))
    }
}
