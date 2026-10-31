//! Current Macro main _r/Nr/Rr and module 13139 G/H, statically audited.
//! time is microseconds; time_tick identifies callbacks and is not a clock.
use super::{
    ActionItem, ActionKind, KeyboardEvent, MacroType, MouseEvent, MouseMovement, RecordingGeometry,
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};

#[path = "recording/capture.rs"]
mod capture;
#[path = "recording/preferences.rs"]
mod preferences;
pub use capture::{Callback, Capture};
use capture::{CallbackKind, Input, InputKind, MouseAction, NativeKey};
pub use preferences::{DelayMode, Preferences, Seconds};

#[derive(Clone)]
pub struct Options {
    pub macro_type: MacroType,
    pub phase: Option<u8>,
    pub delay_mode: u8,
    pub fixed: f64,
    pub random: [f64; 2],
    pub next_pair: u64,
    pub movement_mode: u8,
    pub shortcut: Vec<ShortcutKey>,
    /// Nr uses recordProfile again for stop trimming and final delay handling;
    /// Rr's per-row conversion uses the current document fields above.
    pub finalization_macro_type: MacroType,
    pub finalization_delay_mode: u8,
}

/// Source _r uses native VKs for initial releases; Nr uses the recorder
/// table's keyCode for the trailing stop chord. These are different domains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutKey {
    pub virtual_key: u16,
    pub key_code: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopCause {
    Button,
    Shortcut,
}

#[cfg(test)]
mod movement_tests {
    use super::*;
    use serde_json::json;
    fn options(mode: u8) -> Options {
        Options {
            macro_type: MacroType::Standard,
            phase: None,
            delay_mode: 3,
            fixed: 0.,
            random: [0., 0.],
            next_pair: 1,
            movement_mode: mode,
            shortcut: Vec::new(),
            finalization_macro_type: MacroType::Standard,
            finalization_delay_mode: 3,
        }
    }
    fn geometry() -> RecordingGeometry {
        RecordingGeometry {
            monitors_before: json!([{"isPrimaryMonitor":true,"MonitorRect":{"left":0,"top":0,"right":1920,"bottom":1080},"dpiScaleX":1.,"dpiScaleY":1.}]),
            monitors_after: json!([]),
            screen_before: json!({"MonitorRect":{"left":0,"top":0,"width":1920,"height":1080}}),
            screen_after: json!({}),
        }
    }
    #[test]
    fn movement_prefix_is_retained_before_final_input_with_millisecond_times() {
        let mut decoder = Decoder::new(&options(1));
        decoder.set_geometry(geometry());
        decoder.push(&json!({"kind":"item","time_tick":4,"received_at_ms":100,"event":[{"action":"mousemove","x":10,"y":20,"time":1500},{"action":"mousemove","x":11,"y":21,"time":2500},{"action":"lbuttondown","time":1000}]})).unwrap();
        let rows = decoder.finish(None).unwrap();
        let movement = rows[0].mouse_movement.as_ref().unwrap();
        assert_eq!(
            movement.buffer,
            json!([{"x":10.,"y":20.,"time":1.5},{"x":11.,"y":21.,"time":2.5}])
                .as_array()
                .unwrap()
                .clone()
        );
        assert_eq!(rows[0].value, "0.004");
        assert_eq!(rows[1].mouse.as_ref().unwrap().button, Some(0));
    }
    #[test]
    fn trailing_movement_flushes_only_with_observed_geometry() {
        let callback = json!({"kind":"item","time_tick":5,"received_at_ms":100,"event":[{"action":"mousemove","x":10,"y":20,"time":1500}]});
        let mut decoder = Decoder::new(&options(3));
        decoder.push(&callback).unwrap();
        assert!(decoder.finish(None).is_err());
        let mut decoder = Decoder::new(&options(3));
        decoder.set_geometry(geometry());
        decoder.push(&callback).unwrap();
        assert_eq!(
            decoder.finish(None).unwrap()[0]
                .mouse_movement
                .as_ref()
                .unwrap()
                .mode,
            3
        );
        let mut disabled = Decoder::new(&options(0));
        disabled.push(&callback).unwrap();
        assert!(disabled.finish(None).unwrap().is_empty());
    }
}

#[derive(Deserialize)]
struct Key {
    name: String,
    scancode: Option<u16>,
    #[serde(rename = "virtualKey")]
    virtual_key: Option<String>,
    #[serde(rename = "type")]
    key_type: String,
    flag: u8,
    #[serde(rename = "outputFlag")]
    output_flag: Option<u8>,
}
#[derive(Deserialize)]
struct Data {
    keys: Vec<Key>,
}

fn keys() -> &'static [Key] {
    static DATA: OnceLock<Data> = OnceLock::new();
    &DATA.get_or_init(|| read_recording_keyboard_catalog()).keys
}
fn makecode(vk: u16) -> u16 {
    match vk {
        162 => 17,
        163 => 285,
        160 => 16,
        161 => 310,
        164 => 18,
        165 => 312,
        _ => vk,
    }
}
fn number(value: &Value, field: &str) -> Result<f64, String> {
    value
        .get(field)
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("录制事件缺少有效的 {field}"))
}
fn word(value: &Value, field: &str) -> Result<u16, String> {
    let n = number(value, field)?;
    if n.fract() != 0. || !(0. ..=u16::MAX as f64).contains(&n) {
        return Err(format!("录制事件的 {field} 不是有效整数"));
    }
    Ok(n as u16)
}
struct RecordedKeyInput {
    virtual_key: u16,
    scan_code: u16,
    flags: u16,
}
impl RecordedKeyInput {
    fn from_native(native: NativeKey) -> Self {
        let mut virtual_key = native.virtual_key;
        let flags = native.flags;
        let mut scan_code = native.scan_code;
        if scan_code == 92 {
            scan_code = 91;
        }
        // Current 62141.C/M normalizes numpad navigation before Rr consumes it.
        if matches!(flags, 0 | 128) {
            virtual_key = match virtual_key {
                45 => 96,
                36 => 103,
                38 => 104,
                33 => 105,
                37 => 100,
                12 => 101,
                39 => 102,
                35 => 97,
                40 => 98,
                34 => 99,
                46 => 110,
                _ => virtual_key,
            };
        }
        Self {
            virtual_key,
            scan_code,
            flags,
        }
    }
}
fn key(input: &RecordedKeyInput, preview: bool) -> Option<&'static Key> {
    let flag = match input.flags {
        32 | 0 | 128 => 0,
        33 | 1 | 129 => 2,
        _ => 0,
    };
    let matches_vk = |k: &&Key| {
        k.virtual_key.as_deref().and_then(|v| v.parse::<u16>().ok()) == Some(input.virtual_key)
    };
    keys()
        .iter()
        .find(|k| {
            if matches!(input.flags, 0 | 128) && input.scan_code == 69 && input.virtual_key == 19 {
                matches_vk(k)
            } else {
                (preview || matches_vk(k))
                    && k.scancode == Some(input.scan_code)
                    && k.output_flag.filter(|v| *v != 0).unwrap_or(k.flag) == flag
            }
        })
        .or_else(|| keys().iter().find(matches_vk))
}

pub struct Decoder {
    options: Options,
    output: Vec<ActionItem>,
    next: u64,
    held_keys: HashMap<u16, (u64, u8)>,
    held_mouse: HashMap<u8, u64>,
    last_tick: Option<u64>,
    started_at: Option<u64>,
    first_at: Option<u64>,
    preview: bool,
    previous_key: Option<(u16, u8)>,
    pending_key_delay: f64,
    dropped: usize,
    movement: Vec<Input>,
    geometry: Option<RecordingGeometry>,
    initial_release: bool,
    shortcut_preview: Vec<Callback>,
}
impl Decoder {
    pub fn new(options: &Options) -> Self {
        Self {
            options: options.clone(),
            output: Vec::new(),
            next: options.next_pair,
            held_keys: HashMap::new(),
            held_mouse: HashMap::new(),
            last_tick: None,
            started_at: None,
            first_at: None,
            preview: false,
            previous_key: None,
            pending_key_delay: 0.,
            dropped: 0,
            movement: Vec::new(),
            geometry: None,
            initial_release: true,
            shortcut_preview: Vec::new(),
        }
    }
    /// 58190 le uses a separate live converter and a 50-row DOM window.
    pub fn for_preview(options: &Options) -> Self {
        let mut decoder = Self::new(options);
        decoder.preview = true;
        decoder
    }
    pub fn set_geometry(&mut self, geometry: RecordingGeometry) {
        for item in &mut self.output {
            if let Some(movement) = &mut item.mouse_movement {
                movement.geometry = Some(geometry.clone());
            }
        }
        self.geometry = Some(geometry);
    }
    fn flush_movement(&mut self) -> Result<(), String> {
        if self.movement.is_empty() {
            return Ok(());
        }
        let geometry = self
            .geometry
            .clone()
            .ok_or("Mouse recording has no observed monitor geometry")?;
        let raw = std::mem::take(&mut self.movement);
        let mut buffer = Vec::with_capacity(raw.len());
        let mut duration = 0.;
        for point in &raw {
            let time = point.time / 1000.;
            if time < 0. {
                return Err("Mouse movement delay is negative".into());
            }
            duration += time;
            let InputKind::Movement { x, y } = point.kind else {
                return Err("录制鼠标轨迹包含非移动输入".into());
            };
            buffer.push(serde_json::json!({"x":x,"y":y,"time":time}));
        }
        let options = &self.options;
        if options.macro_type == MacroType::Standard
            && options.delay_mode != 3
            && !(options.delay_mode == 1 && options.fixed == 0.)
        {
            let mut delay = ActionItem::new(ActionKind::Delay);
            if options.delay_mode == 2 {
                delay.state = "randomized".into();
                delay.number_min = options.random[0].to_string();
                delay.number_max = options.random[1].to_string();
            } else {
                let seconds = if options.delay_mode == 1 {
                    options.fixed
                } else {
                    raw.last().unwrap().time / 1_000_000.
                };
                delay.value = format!("{seconds:.3}");
            }
            self.output.push(delay);
        }
        let mut item = ActionItem::new(ActionKind::Mouse);
        item.value = format!("{:.3}", duration / 1000.);
        item.mouse_movement = Some(MouseMovement {
            imported_xml: false,
            mode: options.movement_mode,
            buffer,
            geometry: Some(geometry),
        });
        if !self.preview {
            item.recorded_input = Some(Value::Array(
                raw.into_iter().map(|point| point.source).collect(),
            ));
        }
        self.output.push(item);
        Ok(())
    }
    /// Stream source events into isolated temporary rows. No document mutation.
    pub fn push(&mut self, callback: &Value) -> Result<(), String> {
        if let Some(callback) = Callback::parse(callback)? {
            self.push_callback(&callback)?;
        }
        Ok(())
    }

    pub fn update_preview_options(&mut self, options: &Options) {
        debug_assert!(self.preview);
        self.options = options.clone();
        self.next = self.next.max(options.next_pair);
    }

    pub fn push_callback(&mut self, callback: &Callback) -> Result<(), String> {
        if matches!(callback.kind, CallbackKind::Started) {
            self.started_at = callback.received_at_ms;
            return Ok(());
        }
        let CallbackKind::Items { tick, inputs } = &callback.kind else {
            return Ok(());
        };
        self.first_at = self.first_at.or(callback.received_at_ms);
        if self.last_tick == Some(*tick) {
            return Ok(());
        }
        self.last_tick = Some(*tick);
        let raw = inputs.last().expect("validated nonempty native callback");
        if self.options.movement_mode != 0 && self.options.macro_type != MacroType::Sequence {
            self.movement
                .extend(inputs[..inputs.len() - 1].iter().cloned());
            if raw.is_movement() {
                self.movement.push(raw.clone());
            } else {
                self.flush_movement()?;
            }
        }
        if raw.is_movement() {
            // _r clears its initial-release gate on the first accepted item,
            // even when mouse tracking is disabled for the document.
            self.initial_release = false;
            if self.preview && self.shortcut_preview.len() != self.options.shortcut.len() {
                for held in std::mem::take(&mut self.shortcut_preview) {
                    self.push_input(&held, held.final_input().unwrap())?;
                }
            }
            return Ok(());
        }
        if let Some(key) = raw.key() {
            if matches!(key.virtual_key, 173 | 176 | 177 | 179 | 175 | 174)
                || key.virtual_key == 255 && !matches!(key.scan_code, 112 | 121)
            {
                return Ok(());
            }
        }
        let shortcut_key = self.options.shortcut.iter().any(|key| {
            raw.key()
                .is_some_and(|native| native.virtual_key == key.virtual_key)
        });
        let key_down = matches!(raw.kind, InputKind::Key { down: true, .. });
        let key_up = matches!(raw.kind, InputKind::Key { down: false, .. });
        if self.initial_release && shortcut_key && key_up {
            return Ok(());
        }
        self.initial_release = false;
        if self.preview && !self.options.shortcut.is_empty() {
            if shortcut_key
                && key_down
                && self.shortcut_preview.len() != self.options.shortcut.len()
                && self.shortcut_preview.iter().all(|held| {
                    held.final_input()
                        .and_then(Input::key)
                        .map(|key| key.virtual_key)
                        != raw.key().map(|key| key.virtual_key)
                })
            {
                self.shortcut_preview.push(callback.only_final_input());
                return Ok(());
            }
            if self.shortcut_preview.len() != self.options.shortcut.len() {
                for held in std::mem::take(&mut self.shortcut_preview) {
                    self.push_input(&held, held.final_input().unwrap())?;
                }
            }
        }
        self.push_input(callback, raw)
    }
    fn push_input(&mut self, callback: &Callback, raw: &Input) -> Result<(), String> {
        let options = &self.options;
        let sequence = options.macro_type == MacroType::Sequence;
        let output = &mut self.output;
        let held_keys = &mut self.held_keys;
        let held_mouse = &mut self.held_mouse;
        let next = &mut self.next;
        let mut allocate = || {
            let id = *next;
            *next = next
                .checked_add(1)
                .ok_or_else(|| "Local macro event identity exhausted".to_string())?;
            Ok::<_, String>(id)
        };
        let mut item;
        let mut repeat = false;
        let key_down = matches!(raw.kind, InputKind::Key { down: true, .. });
        if let InputKind::Key { down, key: native } = raw.kind {
            if sequence && down {
                return Ok(());
            }
            let input = RecordedKeyInput::from_native(native);
            // Rr's Sequence branch retains the observed VK without a key-table
            // lookup. le resolves a live label when possible and otherwise
            // keeps that same VK; an unknown name must not reject the recording.
            let key = if sequence && !self.preview {
                None
            } else {
                key(&input, self.preview)
            };
            let makecode = makecode(
                key.and_then(|key| key.virtual_key.as_deref()?.parse().ok())
                    .unwrap_or(input.virtual_key),
            );
            let (pair, state) = if sequence {
                (allocate()?, None)
            } else {
                let key = key.ok_or_else(|| {
                    format!(
                        "当前源键表无法识别录制按键：VK {}，Scan {}",
                        input.virtual_key, input.scan_code
                    )
                })?;
                let flag = key.output_flag.filter(|v| *v != 0).unwrap_or(key.flag);
                if self.preview {
                    if down {
                        // le only folds consecutive state-0 keydowns in its live
                        // view. Post-processing H folds every still-held key.
                        if flag == 0 && self.previous_key == Some((makecode, 0)) {
                            self.pending_key_delay += raw.time / 1_000_000.;
                            return Ok(());
                        }
                        self.previous_key = Some((makecode, flag));
                        self.pending_key_delay = 0.;
                    } else {
                        self.previous_key = None;
                    }
                }
                if down {
                    if let Some(&(id, state)) = held_keys.get(&makecode).filter(|_| !self.preview) {
                        repeat = true;
                        (id, Some(state))
                    } else {
                        let id = allocate()?;
                        held_keys.insert(makecode, (id, flag));
                        (id, Some(flag))
                    }
                } else {
                    let (id, flag) = held_keys
                        .remove(&makecode)
                        .map(Ok)
                        .unwrap_or_else(|| allocate().map(|id| (id, flag)))?;
                    (id, Some(flag + 1))
                }
            };
            item = ActionItem::new(ActionKind::Keyboard);
            if let Some(key) = key {
                item.value = key.name.clone();
            }
            item.keyboard = Some(KeyboardEvent {
                pair_id: Some(pair),
                makecode: Some(makecode),
                state,
                flag: state,
                key_type: key.map(|key| key.key_type.clone()),
            });
        } else {
            let InputKind::Mouse { action, data } = raw.kind else {
                return Err("录制输入转换收到未归并的鼠标轨迹".into());
            };
            let (button, state) = match action {
                MouseAction::LeftDown => (0, Some(0)),
                MouseAction::LeftUp => (0, Some(1)),
                MouseAction::RightDown => (1, Some(0)),
                MouseAction::RightUp => (1, Some(1)),
                MouseAction::MiddleDown => (2, Some(0)),
                MouseAction::MiddleUp => (2, Some(1)),
                MouseAction::ExtraDown => (if data == 131072 { 4 } else { 3 }, Some(0)),
                // Rr's live Sequence branch reverses these two source values.
                MouseAction::ExtraUp => (
                    if (data == 131072) ^ (sequence && !self.preview) {
                        4
                    } else {
                        3
                    },
                    Some(1),
                ),
                MouseAction::Wheel => (if data > 0 { 8 } else { 9 }, None),
                MouseAction::HorizontalWheel => (if data > 0 { 7 } else { 6 }, None),
            };
            if sequence && state == Some(0) {
                return Ok(());
            }
            let pair = if sequence {
                Some(allocate()?)
            } else {
                match state {
                    Some(0) => {
                        let id = allocate()?;
                        held_mouse.insert(button, id);
                        Some(id)
                    }
                    Some(1) => Some(
                        held_mouse
                            .remove(&button)
                            .map(Ok)
                            .unwrap_or_else(&mut allocate)?,
                    ),
                    _ => None,
                }
            };
            item = ActionItem::new(ActionKind::Mouse);
            item.value = super::MOUSE_ACTION_KEYS[button as usize].into();
            item.mouse = Some(MouseEvent {
                pair_id: pair,
                button: Some(button),
                state: if sequence && matches!(action, MouseAction::ExtraUp) {
                    Some(0)
                } else if sequence {
                    None
                } else {
                    state
                },
            });
        }
        item.phase = options.phase;
        if !self.preview {
            item.recorded_input = Some(callback.source.clone());
        }
        if options.macro_type == MacroType::Standard
            && options.delay_mode != 3
            && !(options.delay_mode == 1 && options.fixed == 0.)
        {
            let mut delay = ActionItem::new(ActionKind::Delay);
            if options.delay_mode == 2 {
                delay.state = "randomized".into();
                delay.number_min = options.random[0].to_string();
                delay.number_max = options.random[1].to_string();
            } else {
                let seconds = if options.delay_mode == 1 && options.fixed != 0. {
                    options.fixed
                } else {
                    raw.time / 1_000_000. + if self.preview { self.pending_key_delay } else { 0. }
                };
                if seconds < 0. {
                    return Err("录制延迟为负数".into());
                }
                delay.value = format!("{seconds:.3}");
            }
            // H combines adjacent fixed delays after repeated keydown removal.
            if let Some(previous) = output.last_mut().filter(|v| {
                !self.preview
                    && v.kind == ActionKind::Delay
                    && v.state == "fixed"
                    && delay.state == "fixed"
            }) {
                previous.value = format!(
                    "{:.3}",
                    super::parse_delay(&previous.value) + super::parse_delay(&delay.value)
                );
            } else {
                output.push(delay);
            }
            if self.preview && !key_down {
                self.pending_key_delay = 0.;
                self.previous_key = None;
            }
        }
        if !repeat {
            output.push(item);
        }
        if self.preview && output.len() > 50 {
            let excess = output.len() - 50;
            output.drain(..excess);
            self.dropped += excess;
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.output.len()
    }
    pub fn dropped(&self) -> usize {
        self.dropped
    }
    /// Snapshot the bounded temporary window without persistence provenance.
    pub fn preview_since(&self, index: usize) -> Vec<ActionItem> {
        self.output[index..]
            .iter()
            .map(|item| {
                let mut row = item.clone();
                row.recorded_input = None;
                row
            })
            .collect()
    }
    pub fn finish(mut self, stop: Option<StopCause>) -> Result<Vec<ActionItem>, String> {
        self.flush_movement()?;
        let options = &self.options;
        let sequence = options.finalization_macro_type == MacroType::Sequence;
        let mut output = self.output;
        let first_at = self.first_at;
        let started_at = self.started_at;
        if stop == Some(StopCause::Button) && !sequence {
            if let Some(index) = output
                .iter()
                .rposition(|v| v.mouse.as_ref().is_some_and(|m| m.button == Some(0)))
            {
                output.remove(index); // Nr removes exactly its last left-button row.
            }
        }
        if stop == Some(StopCause::Shortcut) {
            if options.shortcut.is_empty() {
                return Err("Shortcut stop has no configured chord".into());
            }
            trim_stop_shortcut(&mut output, &options.shortcut);
        }
        if !sequence && output.iter().all(|v| v.kind == ActionKind::Delay) {
            output.clear();
        }
        // Nr removes the stop row before H merges any newly adjacent delays.
        let mut combined: Vec<ActionItem> = Vec::with_capacity(output.len());
        for item in output {
            if let Some(previous) = combined.last_mut().filter(|v| {
                !sequence
                    && v.kind == ActionKind::Delay
                    && v.state == "fixed"
                    && item.kind == ActionKind::Delay
                    && item.state == "fixed"
            }) {
                previous.value = format!(
                    "{:.3}",
                    super::parse_delay(&previous.value) + super::parse_delay(&item.value)
                );
            } else {
                combined.push(item);
            }
        }
        let mut output = combined;
        if options.finalization_delay_mode == 0 {
            if let Some(delay) = output.first_mut().filter(|v| v.kind == ActionKind::Delay) {
                let first = first_at.ok_or("录制缺少首事件接收时间")?;
                let start = started_at.ok_or("录制缺少 started 接收时间")?;
                let elapsed = first
                    .checked_sub(start)
                    .ok_or("录制期间系统时间倒退，无法确认首事件延迟")?;
                delay.value = format!("{:.3}", elapsed as f64 / 1000.);
            }
        }
        Ok(output)
    }
}

fn trim_stop_shortcut(output: &mut Vec<ActionItem>, shortcut: &[ShortcutKey]) {
    let mut remaining: Vec<_> = shortcut.iter().map(|key| key.key_code).collect();
    let mut remove = vec![false; output.len()];
    let mut cursor = output.len();
    while cursor > 0 && !remaining.is_empty() {
        cursor -= 1;
        let row = &output[cursor];
        if row.kind == ActionKind::Delay {
            continue;
        }
        let Some(key) = row
            .keyboard
            .as_ref()
            .filter(|key| key.makecode.is_some_and(|code| remaining.contains(&code)))
        else {
            break;
        };
        if key.state == Some(0) {
            remaining.retain(|code| Some(*code) != key.makecode);
        }
        remove[cursor] = true;
        if cursor > 0 && output[cursor - 1].kind == ActionKind::Delay {
            cursor -= 1;
            remove[cursor] = true;
        }
    }
    let mut index = 0;
    output.retain(|_| {
        let keep = !remove[index];
        index += 1;
        keep
    });
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    use serde_json::json;
    fn options() -> Options {
        Options {
            macro_type: MacroType::Standard,
            phase: None,
            delay_mode: 3,
            fixed: 0.,
            random: [0., 0.],
            next_pair: 1,
            movement_mode: 0,
            shortcut: vec![
                ShortcutKey {
                    virtual_key: 162,
                    key_code: 17,
                },
                ShortcutKey {
                    virtual_key: 65,
                    key_code: 65,
                },
            ],
            finalization_macro_type: MacroType::Standard,
            finalization_delay_mode: 3,
        }
    }
    fn input(tick: u64, vk: u16, scan: u16, down: bool) -> Value {
        json!({"kind":"item","time_tick":tick,"received_at_ms":100+tick,
            "event":[{"action":if down {"keydown"} else {"keyup"},"vkCode":vk,"scanCode":scan,"flags":if down {0} else {128},"time":1000}]})
    }
    #[test]
    fn leading_trigger_releases_are_ignored_but_later_releases_are_retained() {
        let mut decoder = Decoder::new(&options());
        decoder.push(&input(1, 65, 30, false)).unwrap();
        decoder.push(&input(2, 162, 29, false)).unwrap();
        assert_eq!(decoder.len(), 0);
        decoder.push(&input(3, 66, 48, true)).unwrap();
        decoder.push(&input(4, 65, 30, false)).unwrap();
        assert_eq!(decoder.finish(None).unwrap().len(), 2);
        let mut decoder = Decoder::new(&options());
        decoder.push(&json!({"kind":"item","time_tick":1,"received_at_ms":101,"event":[{"action":"mousemove","x":1,"y":2,"time":1000}]})).unwrap();
        decoder.push(&input(2, 65, 30, false)).unwrap();
        assert_eq!(decoder.finish(None).unwrap().len(), 1);
    }
    #[test]
    fn shortcut_stop_removes_only_the_trailing_chord_and_its_delays() {
        let mut options = options();
        options.delay_mode = 1;
        options.fixed = 0.1;
        let mut decoder = Decoder::new(&options);
        for (tick, vk, scan, down) in [
            (1, 66, 48, true),
            (2, 66, 48, false),
            (3, 162, 29, true),
            (4, 65, 30, true),
            (5, 65, 30, false),
            (6, 162, 29, false),
        ] {
            decoder.push(&input(tick, vk, scan, down)).unwrap();
        }
        let rows = decoder.finish(Some(StopCause::Shortcut)).unwrap();
        assert_eq!(rows.len(), 4);
        assert!(
            rows.iter()
                .filter_map(|row| row.keyboard.as_ref())
                .all(|key| key.makecode == Some(66))
        );
    }
    #[test]
    fn partial_preview_flushes_before_unrelated_input_and_stopping_preserves_mouse() {
        let options = options();
        let mut preview = Decoder::for_preview(&options);
        preview.push(&input(1, 162, 29, true)).unwrap();
        assert_eq!(preview.len(), 0);
        preview.push(&input(2, 66, 48, true)).unwrap();
        assert_eq!(preview.len(), 2);
        let mut decoder = Decoder::new(&options);
        decoder.push(&input(1, 162, 29, true)).unwrap();
        decoder.push(&json!({"kind":"item","time_tick":2,"received_at_ms":102,"event":[{"action":"lbuttondown","time":1000}]})).unwrap();
        let rows = decoder.finish(Some(StopCause::Shortcut)).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[1].mouse.is_some());
    }
}

#[cfg(test)]
mod sequence_tests {
    use super::*;
    use serde_json::json;

    fn options() -> Options {
        Options {
            macro_type: MacroType::Sequence,
            phase: None,
            delay_mode: 3,
            fixed: 0.,
            random: [0., 0.],
            next_pair: 1,
            movement_mode: 0,
            shortcut: Vec::new(),
            finalization_macro_type: MacroType::Sequence,
            finalization_delay_mode: 3,
        }
    }

    fn release(tick: u64, virtual_key: u16, scan_code: u16, flags: u16) -> Value {
        json!({"kind":"item","time_tick":tick,"received_at_ms":100+tick,
            "event":[{"action":"keyup","vkCode":virtual_key,"scanCode":scan_code,"flags":flags,"time":1000}]})
    }

    #[test]
    fn sequence_retains_unlisted_virtual_keys_in_the_draft_and_preview() {
        // Current Rr converts the native VK directly; le keeps it when its
        // scan/VK name lookup misses. These VKs are absent from that key table.
        for preview in [false, true] {
            let options = options();
            let mut decoder = if preview {
                Decoder::for_preview(&options)
            } else {
                Decoder::new(&options)
            };
            for (index, virtual_key) in [21, 25, 231, 250].into_iter().enumerate() {
                decoder
                    .push(&release(index as u64 + 1, virtual_key, 0, 128))
                    .unwrap();
            }
            let rows = decoder.finish(None).unwrap();
            let keys: Vec<_> = rows
                .iter()
                .map(|row| row.keyboard.as_ref().unwrap())
                .collect();
            assert_eq!(
                keys.iter().map(|key| key.makecode).collect::<Vec<_>>(),
                [Some(21), Some(25), Some(231), Some(250)]
            );
            assert!(
                keys.iter()
                    .all(|key| key.state.is_none() && key.flag.is_none() && key.key_type.is_none())
            );
            assert!(
                keys.windows(2)
                    .all(|pair| pair[0].pair_id != pair[1].pair_id)
            );
            assert!(
                rows.iter()
                    .all(|row| row.recorded_input.is_some() != preview)
            );
        }
    }

    #[test]
    fn sequence_applies_native_modifier_and_numpad_conversion_without_a_key_type() {
        // 13139.y and 62141.C/M define these conversions before Rr stores the
        // Sequence Makecode. The Sequence template leaves State and Type unset.
        let mut decoder = Decoder::new(&options());
        for (tick, virtual_key, scan_code, flags) in [
            (1, 162, 29, 128),
            (2, 163, 29, 129),
            (3, 160, 42, 128),
            (4, 161, 54, 128),
            (5, 164, 56, 128),
            (6, 165, 56, 129),
            (7, 45, 82, 128),
            (8, 45, 82, 129),
        ] {
            decoder
                .push(&release(tick, virtual_key, scan_code, flags))
                .unwrap();
        }
        let rows = decoder.finish(None).unwrap();
        let keys: Vec<_> = rows
            .iter()
            .map(|row| row.keyboard.as_ref().unwrap())
            .collect();
        assert_eq!(
            keys.iter().map(|key| key.makecode).collect::<Vec<_>>(),
            [
                Some(17),
                Some(285),
                Some(16),
                Some(310),
                Some(18),
                Some(312),
                Some(96),
                Some(45)
            ]
        );
        assert!(
            keys.iter()
                .all(|key| key.state.is_none() && key.flag.is_none() && key.key_type.is_none())
        );
    }

    #[test]
    fn sequence_still_rejects_malformed_native_key_fields() {
        for preview in [false, true] {
            for field in ["vkCode", "scanCode", "flags"] {
                for value in [None, Some(json!(-1)), Some(json!(1.5))] {
                    let mut event = release(1, 231, 0, 128);
                    let raw = event["event"][0].as_object_mut().unwrap();
                    if let Some(value) = value {
                        raw.insert(field.into(), value);
                    } else {
                        raw.remove(field);
                    }
                    let options = options();
                    let mut decoder = if preview {
                        Decoder::for_preview(&options)
                    } else {
                        Decoder::new(&options)
                    };
                    assert!(decoder.push(&event).is_err(), "{field}: {event}");
                }
            }
        }
    }

    #[test]
    fn standard_recording_still_requires_a_resolved_key_and_pairs_known_keys() {
        let mut options = options();
        options.macro_type = MacroType::Standard;
        let mut decoder = Decoder::new(&options);
        assert!(decoder.push(&release(1, 231, 0, 128)).is_err());

        let mut decoder = Decoder::new(&options);
        decoder
            .push(&json!({"kind":"item","time_tick":1,"received_at_ms":101,
                "event":[{"action":"keydown","vkCode":65,"scanCode":30,"flags":0,"time":1000}]}))
            .unwrap();
        decoder.push(&release(2, 65, 30, 128)).unwrap();
        let rows = decoder.finish(None).unwrap();
        let down = rows[0].keyboard.as_ref().unwrap();
        let up = rows[1].keyboard.as_ref().unwrap();
        assert_eq!(down.makecode, Some(65));
        assert_eq!(up.makecode, Some(65));
        assert_eq!(down.state, Some(0));
        assert_eq!(up.state, Some(1));
        assert_eq!(down.pair_id, up.pair_id);
        assert!(down.key_type.is_some());
    }
}

// Generated catalog reader: razer-app-pages/recording_keyboard_data.
fn read_recording_keyboard_catalog() -> Data {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                keys: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Key>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Key> {
        let value = match r.tag()? {
            0 => Key {
                name: r.child(decode_3)?,
                scancode: r.child(decode_4)?,
                virtual_key: r.child(decode_6)?,
                key_type: r.child(decode_3)?,
                flag: r.child(decode_7)?,
                output_flag: None,
            },
            1 => Key {
                name: r.child(decode_3)?,
                scancode: r.child(decode_4)?,
                virtual_key: r.child(decode_6)?,
                key_type: r.child(decode_3)?,
                flag: r.child(decode_7)?,
                output_flag: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u16>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_5)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u16> {
        let value = match r.tag()? {
            0 => r.u16()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_3)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u8>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_7)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_RECORDING_KEYBOARD_DATA,
        0,
        "8db186d279a5a98ae37b27020cb1a0b9ea92ae32aada2556f1477f7adfa7ade9",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod recording_keyboard_data_catalog_tests {
    #[test]
    fn packaged_catalog_recording_keyboard_data_decodes() {
        let _ = super::read_recording_keyboard_catalog();
    }
}
// End generated catalog reader: razer-app-pages/recording_keyboard_data.
