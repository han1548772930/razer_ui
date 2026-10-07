//! Current Macro main _r/Nr/Rr and module 13139 G/H, statically audited.
//! time is microseconds; time_tick identifies callbacks and is not a clock.
use super::{ActionItem, ActionKind};
use crate::features::macro_library::{KeyboardEvent, MacroType, MouseEvent};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};

#[derive(Clone)]
pub(super) struct Options {
    pub(super) macro_type: MacroType,
    pub(super) phase: Option<u8>,
    pub(super) delay_mode: u8,
    pub(super) fixed: f64,
    pub(super) random: [f64; 2],
    pub(super) next_pair: u64,
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
fn keys() -> &'static [Key] {
    #[derive(Deserialize)]
    struct Data {
        keys: Vec<Key>,
    }
    static DATA: OnceLock<Data> = OnceLock::new();
    &DATA
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_data.json")).expect("audited keys")
        })
        .keys
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
fn key(value: &Value, preview: bool) -> Result<(&'static Key, u16), String> {
    let mut vk = word(value, "vkCode")?;
    let flags = word(value, "flags")?;
    let mut scan = word(value, "scanCode")?;
    if scan == 92 {
        scan = 91;
    }
    if matches!(flags, 0 | 128) {
        vk = match vk {
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
            _ => vk,
        };
    }
    let flag = match flags {
        32 | 0 | 128 => 0,
        33 | 1 | 129 => 2,
        _ => 0,
    };
    let matches_vk =
        |k: &&Key| k.virtual_key.as_deref().and_then(|v| v.parse::<u16>().ok()) == Some(vk);
    let k = keys()
        .iter()
        .find(|k| {
            if matches!(flags, 0 | 128) && scan == 69 && vk == 19 {
                matches_vk(k)
            } else {
                (preview || matches_vk(k))
                    && k.scancode == Some(scan)
                    && k.output_flag.filter(|v| *v != 0).unwrap_or(k.flag) == flag
            }
        })
        .or_else(|| keys().iter().find(matches_vk))
        .ok_or_else(|| format!("当前源键表无法识别录制按键：VK {vk}，Scan {scan}"))?;
    Ok((
        k,
        makecode(
            k.virtual_key
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(vk),
        ),
    ))
}

pub(super) struct Decoder {
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
}
impl Decoder {
    pub(super) fn new(options: &Options) -> Self {
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
        }
    }
    /// 58190 le uses a separate live converter and a 50-row DOM window.
    pub(super) fn for_preview(options: &Options) -> Self {
        let mut decoder = Self::new(options);
        decoder.preview = true;
        decoder
    }
    /// Stream source events into isolated temporary rows. No document mutation.
    pub(super) fn push(&mut self, callback: &Value) -> Result<(), String> {
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
        if callback["kind"] == "started" {
            self.started_at = callback["received_at_ms"].as_u64();
            return Ok(());
        }
        if callback["kind"] != "item" {
            return Ok(());
        }
        self.first_at = self.first_at.or(callback["received_at_ms"].as_u64());
        let batch = callback["event"]
            .as_array()
            .ok_or("录制事件不是原版数组格式")?;
        if batch.is_empty() {
            return Err("录制事件数组为空".into());
        }
        // _r sends preceding entries as mouse movement. This UI currently
        // exposes no tracking mode; retain source callbacks on recorded rows.
        if batch[..batch.len() - 1]
            .iter()
            .any(|v| v["action"] != "mousemove")
        {
            return Err("录制事件批次包含非鼠标轨迹前缀".into());
        }
        let tick = callback["time_tick"]
            .as_u64()
            .ok_or("录制事件缺少 time_tick")?
            .checked_add(u64::from(batch.len() > 1))
            .ok_or("录制事件 time_tick 溢出")?;
        if self.last_tick == Some(tick) {
            return Ok(());
        }
        self.last_tick = Some(tick);
        let raw = batch.last().unwrap();
        let action = raw["action"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| raw["action"].as_i64().map(|n| n.to_string()))
            .ok_or("录制事件缺少 action")?;
        if action == "mousemove" {
            return Ok(());
        }
        if let Some(vk) = raw["vkCode"].as_u64() {
            if matches!(vk, 173 | 176 | 177 | 179 | 175 | 174)
                || vk == 255 && !matches!(raw["scanCode"].as_u64(), Some(112 | 121))
            {
                return Ok(());
            }
        }
        let mut item;
        let mut repeat = false;
        if matches!(action.as_str(), "keydown" | "keyup") {
            let down = action == "keydown";
            if sequence && down {
                return Ok(());
            }
            let (key, makecode) = key(raw, self.preview)?;
            let flag = key.output_flag.filter(|v| *v != 0).unwrap_or(key.flag);
            if self.preview && !sequence {
                if down {
                    // le only folds consecutive state-0 keydowns in its live
                    // view. Post-processing H folds every still-held key.
                    if flag == 0 && self.previous_key == Some((makecode, 0)) {
                        self.pending_key_delay += number(raw, "time")? / 1_000_000.;
                        return Ok(());
                    }
                    self.previous_key = Some((makecode, flag));
                    self.pending_key_delay = 0.;
                } else {
                    self.previous_key = None;
                }
            }
            let (pair, state) = if sequence {
                (allocate()?, None)
            } else if down {
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
            };
            item = ActionItem::new(ActionKind::Keyboard);
            item.value = key.name.clone();
            item.keyboard = Some(KeyboardEvent {
                pair_id: Some(pair),
                makecode: Some(makecode),
                state,
                flag: state,
                key_type: Some(key.key_type.clone()),
            });
        } else {
            let data = raw["data"].as_i64().unwrap_or(0);
            let (button, state) = match action.as_str() {
                "lbuttondown" => (0, Some(0)),
                "lbuttonup" => (0, Some(1)),
                "rbuttondown" => (1, Some(0)),
                "rbuttonup" => (1, Some(1)),
                "mbuttondown" => (2, Some(0)),
                "mbuttonup" => (2, Some(1)),
                "523" => (if data == 131072 { 4 } else { 3 }, Some(0)),
                // Rr's live Sequence branch reverses these two source values.
                "524" => (
                    if (data == 131072) ^ (sequence && !self.preview) {
                        4
                    } else {
                        3
                    },
                    Some(1),
                ),
                "mousewheel" => (if data > 0 { 8 } else { 9 }, None),
                "mousehwheel" => (if data > 0 { 7 } else { 6 }, None),
                _ => return Err(format!("不支持的原生录制事件：{action}")),
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
            item.value = super::editors::MOUSE_ACTION_KEYS[button as usize].into();
            item.mouse = Some(MouseEvent {
                pair_id: pair,
                button: Some(button),
                state: if sequence && action == "524" {
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
            item.recorded_input = Some(callback.clone());
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
                    number(raw, "time")? / 1_000_000.
                        + if self.preview {
                            self.pending_key_delay
                        } else {
                            0.
                        }
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
            if self.preview && action != "keydown" {
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
    pub(super) fn len(&self) -> usize {
        self.output.len()
    }
    pub(super) fn dropped(&self) -> usize {
        self.dropped
    }
    /// Snapshot the bounded temporary window without persistence provenance.
    pub(super) fn preview_since(&self, index: usize) -> Vec<ActionItem> {
        self.output[index..]
            .iter()
            .map(|item| {
                let mut row = item.clone();
                row.recorded_input = None;
                row
            })
            .collect()
    }
    pub(super) fn finish(self, click_stop: bool) -> Result<Vec<ActionItem>, String> {
        let options = &self.options;
        let sequence = options.macro_type == MacroType::Sequence;
        let mut output = self.output;
        let first_at = self.first_at;
        let started_at = self.started_at;
        if click_stop && !sequence {
            if let Some(index) = output
                .iter()
                .rposition(|v| v.mouse.as_ref().is_some_and(|m| m.button == Some(0)))
            {
                output.remove(index); // Nr removes exactly its last left-button row.
            }
        }
        if output.iter().all(|v| v.kind == ActionKind::Delay) {
            output.clear();
        }
        // Nr removes the stop row before H merges any newly adjacent delays.
        let mut combined: Vec<ActionItem> = Vec::with_capacity(output.len());
        for item in output {
            if let Some(previous) = combined.last_mut().filter(|v| {
                v.kind == ActionKind::Delay
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
        if options.delay_mode == 0 {
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
