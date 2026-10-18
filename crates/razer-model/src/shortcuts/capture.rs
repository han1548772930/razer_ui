//! Current Dashboard 94608/be inputredirect, browser and device-mode transitions.
use super::{
    Shortcut,
    capture_keys::{self, CaptureKey},
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

const EXCLUDED: &[&str] = &[
    "KEY_PAUSE",
    "KEY_CANCEL",
    "KEY_APPLICATION",
    "KEY_PRINT_SCREEN",
    "KEY_SCROLL_LOCK",
    "KEY_NUMPAD_NUM_LOCK",
];

pub fn default_input_modifiers() -> Vec<String> {
    vec!["KEY_LEFT_CTRL".into(), "KEY_LEFT_SHIFT".into()]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedInput {
    input: String,
    modifiers: Vec<String>,
    hypershift: bool,
}

impl RecordedInput {
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn modifiers(&self) -> &[String] {
        &self.modifiers
    }
    pub fn hypershift(&self) -> bool {
        self.hypershift
    }

    pub fn apply_to(&self, shortcut: &mut Shortcut) {
        shortcut.input.clone_from(&self.input);
        shortcut.modifiers.clone_from(&self.modifiers);
        shortcut.hypershift = self.hypershift;
    }
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "deviceInfo")]
    device: Device,
    #[serde(rename = "eventType")]
    event_type: u32,
    input: RedirectInput,
}

#[derive(Deserialize)]
struct Device {
    #[serde(rename = "productId")]
    product_id: u32,
}

#[derive(Deserialize)]
struct RedirectInput {
    #[serde(rename = "type")]
    kind: String,
    scancode: Option<u32>,
    flag: Option<u32>,
    key: Option<u32>,
}

#[derive(Default)]
pub struct CaptureState {
    hypershift: bool,
    modifiers: Vec<String>,
    pressed: Option<(&'static str, &'static str)>,
    pressed_scan_codes: BTreeMap<u32, Option<u32>>,
}

impl CaptureState {
    pub fn set_mode(&mut self, enabled: bool) {
        self.hypershift = enabled;
    }

    pub fn browser_key_down(&mut self, input: &str, modifiers: &[String]) -> Option<RecordedInput> {
        let key = key_by_id(input)?;
        self.key_down(key, modifiers)
    }

    pub fn browser_key_up(&mut self, input: &str) {
        if let Some(key) = key_by_id(input) {
            self.key_up(key);
        }
    }

    pub fn mouse(&self, input: &str, modifiers: &[String]) -> Option<RecordedInput> {
        if !matches!(input, "ScrollButton" | "RightClick" | "Button4" | "Button5") {
            return None;
        }
        Some(self.recorded(input, modifiers))
    }

    pub fn redirected(&mut self, raw: Value) -> Result<Option<RecordedInput>, String> {
        let event: Envelope = serde_json::from_value(raw).map_err(|error| error.to_string())?;
        if event.event_type != 3 {
            return Ok(None);
        }
        let mut input = event.input;
        if matches!(event.device.product_id, 706 | 707 | 708) {
            if let Some(scan_code) = input.key.and_then(capture_keys::dkm_scan_code) {
                input.kind = "keyboard".into();
                input.scancode = Some(scan_code);
            }
        }
        if input.kind == "razerKey" {
            let key = input.key.ok_or("Razer shortcut input has no key")?;
            if matches!(key, 1 | 10 | 11) {
                let flag = input.flag.ok_or("Razer shortcut input has no flag")?;
                self.hypershift = flag % 2 == 0;
            }
            return Ok(None);
        }
        if !matches!(input.kind.as_str(), "keyboard" | "analogKey") {
            return Ok(None);
        }
        let scan_code = input.scancode.ok_or("Shortcut input has no scan code")?;
        let flag = input.flag.ok_or("Shortcut input has no flag")?;
        let Some(mut key) = capture_keys::keys().iter().find(|key| {
            let down = key.output_flag.unwrap_or(key.flag);
            key.scancode == scan_code && (flag == down || down.checked_add(1) == Some(flag))
        }) else {
            return Ok(None);
        };
        if !allowed(key) {
            return Ok(None);
        }
        if flag % 2 == 0 {
            self.pressed_scan_codes.insert(scan_code, key.output_flag);
            if self.pressed_scan_codes.get(&29) == Some(&Some(4)) {
                if self.pressed_scan_codes.contains_key(&69) {
                    self.pressed_scan_codes.clear();
                    return Ok(None);
                }
                if self.modifiers.is_empty()
                    || self.modifiers.iter().any(|id| id == "KEY_LEFT_CTRL")
                {
                    key = key_by_id("KEY_CANCEL")
                        .ok_or("Source shortcut key table has no KEY_CANCEL")?;
                }
            }
            Ok(self.key_down(key, &[]))
        } else {
            self.pressed_scan_codes.clear();
            self.key_up(key);
            Ok(None)
        }
    }

    fn key_down(&mut self, key: &'static CaptureKey, fallback: &[String]) -> Option<RecordedInput> {
        if !allowed(key) || key.key_code == "91" {
            return None;
        }
        let identity = (key.key_code.as_str(), key.name.as_str());
        if self.pressed == Some(identity) {
            return None;
        }
        self.pressed = Some(identity);
        let input = key.input_id.as_deref()?;
        if is_modifier(key) {
            if !self.modifiers.iter().any(|id| id == input) {
                self.modifiers.push(input.into());
            }
            None
        } else {
            Some(self.recorded(input, fallback))
        }
    }

    fn key_up(&mut self, key: &CaptureKey) {
        if !allowed(key) {
            return;
        }
        if is_modifier(key) {
            self.modifiers
                .retain(|id| key_by_id(id).is_none_or(|held| held.key_code != key.key_code));
        } else {
            self.pressed = None;
        }
    }

    fn recorded(&self, input: &str, fallback: &[String]) -> RecordedInput {
        let mut modifiers = if self.modifiers.is_empty() {
            fallback.to_vec()
        } else {
            self.modifiers.clone()
        };
        if modifiers.is_empty() {
            modifiers = default_input_modifiers();
        }
        if modifiers.len() >= 2
            && matches!(
                (modifiers[0].as_str(), modifiers[1].as_str()),
                ("KEY_LEFT_SHIFT", "KEY_LEFT_CTRL") | ("KEY_RIGHT_SHIFT", "KEY_RIGHT_CTRL")
            )
        {
            modifiers.swap(0, 1);
        }
        RecordedInput {
            input: input.into(),
            modifiers,
            hypershift: self.hypershift,
        }
    }
}

fn key_by_id(input: &str) -> Option<&'static CaptureKey> {
    capture_keys::keys()
        .iter()
        .find(|key| key.input_id.as_deref() == Some(input))
}

fn allowed(key: &CaptureKey) -> bool {
    key.input_id
        .as_deref()
        .is_some_and(|id| !EXCLUDED.contains(&id))
}

fn is_modifier(key: &CaptureKey) -> bool {
    matches!(key.key_code.as_str(), "16" | "17" | "18")
}

#[cfg(test)]
mod tests;
