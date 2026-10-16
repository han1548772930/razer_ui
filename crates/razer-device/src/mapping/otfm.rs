//! Current background otfm-d56d4f50 Ct/$t/Io/wo. Pure lifecycle and input
//! reduction; service completion, native observations and persistence remain
//! separate. No transport acceptance is treated as a saved macro.
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};

#[path = "otfm_document.rs"]
mod document;
pub use document::{assign_mapping, macro_document, update_profile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Querying,
    DisablingMapping,
    CheckingRunningMacros,
    MonitorBefore,
    StartingRecorder,
    MacroLedOn,
    ReactiveShort,
    Recording,
    MacroLedBlinking,
    ReactiveLong,
    AssigningKey,
    AcceptedLedOff,
    MouseMoveChoice,
    MonitorAfter,
    Processing,
    Stopping,
    RejectedRestore,
    Restore,
    EnablingMapping,
    ErrorLighting,
    AbortedLedOff,
    AbortedStop,
    CrashRetry,
    Recovered,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    QueryRecording,
    SuspendMappings,
    CheckRunningMacros,
    MonitorBefore,
    StartRecorder,
    MacroLedOn,
    ReactiveShort,
    MacroLedBlinking,
    ReactiveLong,
    AcceptedLedOff,
    MonitorAfter,
    PostRecording,
    StopRecorder,
    RestoreLighting,
    EnableMappings,
    ErrorLighting,
    MacroLedOff,
    CheckRecovered,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abort {
    Escape,
    ScreenLock,
    SwitchUser,
    Exit,
    Timeout,
    Unplug,
}
impl Abort {
    pub fn source_reason(self) -> &'static str {
        match self {
            Self::Escape => "esc",
            Self::ScreenLock => "screen_lock",
            Self::SwitchUser => "switch_user",
            Self::Exit => "exit",
            Self::Timeout | Self::Unplug => "timeout",
        }
    }
}
pub struct Machine {
    pub state: State,
    pub items: Vec<Value>,
    pub assigned: Vec<Value>,
    pub has_mouse_move: bool,
    pub mouse_move_type: &'static str,
    pub brightness: f64,
    pub keyboard_started: Value,
    pub keyboard_ended: Value,
    pub monitor_before: Value,
    pub screen_before: Value,
    pub monitor_after: Value,
    pub screen_after: Value,
    pub macro_started_tick: Option<u64>,
    pub abort: Option<Abort>,
    pub completed: bool,
    pub assigning: bool,
    deadline_ms: Option<u64>,
    effect: Option<Effect>,
}
impl Default for Machine {
    fn default() -> Self {
        Self {
            state: State::Idle,
            items: vec![],
            assigned: vec![],
            has_mouse_move: false,
            mouse_move_type: "none",
            brightness: 100.,
            keyboard_started: json!({"containerId":"","timerTick":0}),
            keyboard_ended: json!({"containerId":"","timerTick":0}),
            monitor_before: json!([]),
            screen_before: json!({}),
            monitor_after: json!([]),
            screen_after: json!({}),
            macro_started_tick: Some(0),
            abort: None,
            completed: false,
            assigning: false,
            deadline_ms: None,
            effect: None,
        }
    }
}
impl Machine {
    fn enter(&mut self, state: State, now: u64) {
        self.state = state;
        self.deadline_ms = match state {
            State::Recording | State::AssigningKey => Some(now.saturating_add(120_000)),
            State::MouseMoveChoice => Some(now.saturating_add(10_000)),
            State::Recovered => Some(now.saturating_add(13_000)),
            _ => None,
        };
        self.effect = match state {
            State::Querying => Some(Effect::QueryRecording),
            State::DisablingMapping => Some(Effect::SuspendMappings),
            State::CheckingRunningMacros => Some(Effect::CheckRunningMacros),
            State::MonitorBefore => Some(Effect::MonitorBefore),
            State::StartingRecorder => Some(Effect::StartRecorder),
            State::MacroLedOn => Some(Effect::MacroLedOn),
            State::ReactiveShort => Some(Effect::ReactiveShort),
            State::MacroLedBlinking => Some(Effect::MacroLedBlinking),
            State::ReactiveLong => Some(Effect::ReactiveLong),
            State::AcceptedLedOff => Some(Effect::AcceptedLedOff),
            State::MonitorAfter => Some(Effect::MonitorAfter),
            State::Processing => Some(Effect::PostRecording),
            State::Stopping | State::AbortedStop => Some(Effect::StopRecorder),
            State::RejectedRestore | State::Restore => Some(Effect::RestoreLighting),
            State::EnablingMapping => Some(Effect::EnableMappings),
            State::ErrorLighting => Some(Effect::ErrorLighting),
            State::AbortedLedOff => Some(Effect::MacroLedOff),
            State::CrashRetry => Some(Effect::CheckRecovered),
            _ => None,
        };
        if state == State::MacroLedBlinking {
            self.assigning = true;
        }
        if state == State::Processing {
            self.assigning = false;
        }
    }
    pub fn effect(&mut self) -> Option<Effect> {
        self.effect.take()
    }
    pub fn press(&mut self, container: &str, source_tick: Option<u64>, now: u64) {
        match self.state {
            State::Idle => {
                self.keyboard_started =
                    json!({"containerId":container,"timerTick":source_tick.unwrap_or(0)});
                self.enter(State::Querying, now);
            }
            State::MacroLedOn | State::ReactiveShort | State::Recording => {
                self.keyboard_ended = json!({"containerId":container});
                if let Some(tick) = source_tick {
                    self.keyboard_ended["timerTick"] = json!(tick);
                }
                self.enter(State::MacroLedBlinking, now);
            }
            State::MacroLedBlinking | State::ReactiveLong | State::AssigningKey => {
                self.abort(Abort::Timeout, now)
            }
            State::Recovered => {
                self.items.clear();
                self.assigned.clear();
                self.enter(State::Querying, now);
            }
            _ => {}
        }
    }
    pub fn started(&mut self, source_tick: Option<u64>) {
        self.macro_started_tick = source_tick;
    }
    pub fn native_items(&mut self, items: &[Value], native_tick: u64, now: u64) -> Result<()> {
        if self.assigning {
            return Ok(());
        }
        for item in items {
            if matches!(
                item["vkCode"].as_u64(),
                Some(173 | 176 | 177 | 179 | 175 | 174)
            ) {
                continue;
            }
            let mut item = item.clone();
            item["timerTick"] = json!(native_tick);
            self.item(item, now)?;
        }
        Ok(())
    }
    pub fn item(&mut self, mut item: Value, now: u64) -> Result<()> {
        let recording = matches!(
            self.state,
            State::MacroLedOn | State::ReactiveShort | State::Recording
        );
        if recording {
            if item["vkCode"] == 27 {
                if item["action"] == "keyup" {
                    self.abort(Abort::Escape, now);
                }
                return Ok(());
            }
            if item["action"] == "mousemove" {
                self.has_mouse_move = true;
            }
            normalize_key(&mut item);
            self.items.push(item);
            ensure!(
                self.items.len() <= 100_000,
                "OTFM input limit exceeded; incomplete macro is not saved"
            );
            if self.state == State::Recording {
                self.deadline_ms = Some(now.saturating_add(120_000));
            }
            return Ok(());
        }
        if matches!(
            self.state,
            State::MacroLedBlinking | State::ReactiveLong | State::AssigningKey
        ) {
            let key_up = item["action"] == "keyup";
            let assign = if self.state == State::AssigningKey {
                (matches!(item["type"].as_str(), Some("razerKey" | "analogKey"))
                    && item["scancode"] != 91)
                    || ((key_up || (item["action"] == "keydown" && item["scanCode"] == 69))
                        && item["vkCode"] != 91
                        && item["vkCode"] != 27)
            } else {
                key_up && item["vkCode"] != 91 && item["vkCode"] != 27
            };
            if assign {
                normalize_key(&mut item);
                self.assigned.push(item);
                self.enter(State::AcceptedLedOff, now);
            } else if matches!(
                item["vkCode"].as_u64(),
                Some(91 | 160 | 161 | 162 | 163 | 164 | 165)
            ) {
                if let Some(previous) = self.assigned.last() {
                    if previous["action"] == item["action"] && previous["vkCode"] == item["vkCode"]
                    {
                        return Ok(());
                    }
                    if key_up && previous["vkCode"] == item["vkCode"] {
                        self.assigned.pop();
                        return Ok(());
                    }
                }
                self.assigned.push(item);
                if self.state == State::AssigningKey {
                    self.deadline_ms = Some(now.saturating_add(120_000));
                }
            } else if key_up && item["vkCode"] == 27 {
                self.abort(Abort::Escape, now);
            }
        } else if self.state == State::MouseMoveChoice {
            let analog = item["type"] == "analogKey";
            let code = if analog {
                item["scancode"].as_u64()
            } else if item["action"] == "keyup" {
                item["vkCode"].as_u64()
            } else {
                None
            };
            self.mouse_move_type = match (analog, code) {
                (true, Some(2)) | (false, Some(49)) => "relative",
                (true, Some(3)) | (false, Some(50)) => "absolute",
                (true, Some(4)) | (false, Some(51)) => "foreground",
                _ => self.mouse_move_type,
            };
            if matches!(
                (analog, code),
                (true, Some(1..=4)) | (false, Some(27 | 49 | 50 | 51))
            ) {
                self.enter(State::MonitorAfter, now);
            }
        }
        Ok(())
    }
    pub fn abort(&mut self, reason: Abort, now: u64) {
        self.abort = Some(reason);
        self.assigning = false;
        self.enter(State::AbortedLedOff, now);
    }
    pub fn connection_error(&mut self, now: u64) {
        self.enter(State::CrashRetry, now);
    }
    pub fn tick(&mut self, now: u64) {
        if self.deadline_ms.is_some_and(|deadline| now >= deadline) {
            match self.state {
                State::Recording | State::AssigningKey => self.abort(Abort::Timeout, now),
                State::MouseMoveChoice => self.enter(State::MonitorAfter, now),
                State::Recovered => self.enter(State::AbortedLedOff, now),
                _ => {}
            }
        }
    }
    /// `answer` is used only by actual hasMacroRecordingStarted/start results.
    /// Side-effect failures follow the current state's own onError transition.
    pub fn resolved(&mut self, effect: Effect, success: bool, answer: Option<bool>, now: u64) {
        let next = match (self.state, effect) {
            (State::Querying, Effect::QueryRecording) if success => {
                if answer == Some(true) {
                    State::ErrorLighting
                } else {
                    State::DisablingMapping
                }
            }
            // Querying has no onError edge in Ct. Keep it awaiting a real event.
            (State::Querying, Effect::QueryRecording) => return,
            (State::DisablingMapping, Effect::SuspendMappings) => {
                if success {
                    State::MonitorBefore
                } else {
                    State::CheckingRunningMacros
                }
            }
            (State::CheckingRunningMacros, Effect::CheckRunningMacros) => {
                if success {
                    State::MonitorBefore
                } else {
                    State::ErrorLighting
                }
            }
            (State::MonitorBefore, Effect::MonitorBefore) => State::StartingRecorder,
            (State::StartingRecorder, Effect::StartRecorder) => {
                if success && answer == Some(true) {
                    State::MacroLedOn
                } else {
                    State::ErrorLighting
                }
            }
            (State::MacroLedOn, Effect::MacroLedOn) => State::ReactiveShort,
            (State::ReactiveShort, Effect::ReactiveShort) => State::Recording,
            (State::MacroLedBlinking, Effect::MacroLedBlinking) => State::ReactiveLong,
            (State::ReactiveLong, Effect::ReactiveLong) => State::AssigningKey,
            (State::AcceptedLedOff, Effect::AcceptedLedOff) => {
                if self.has_mouse_move {
                    State::MouseMoveChoice
                } else {
                    State::MonitorAfter
                }
            }
            (State::MonitorAfter, Effect::MonitorAfter) => State::Processing,
            (State::Processing, Effect::PostRecording) if success => State::Stopping,
            // Ro only emits ON_MACRO_REJECTED for an observed geometry change.
            // Persistence exceptions do not fabricate that machine event.
            (State::Processing, Effect::PostRecording) => return,
            (State::Stopping, Effect::StopRecorder)
            | (State::AbortedStop, Effect::StopRecorder) => State::Restore,
            (State::RejectedRestore, Effect::RestoreLighting) => State::ErrorLighting,
            (State::Restore, Effect::RestoreLighting)
            | (State::ErrorLighting, Effect::ErrorLighting) => State::EnablingMapping,
            (State::EnablingMapping, Effect::EnableMappings) => {
                self.completed = true;
                State::Idle
            }
            (State::AbortedLedOff, Effect::MacroLedOff) => {
                if self.abort.is_some() {
                    State::AbortedStop
                } else {
                    State::Restore
                }
            }
            (State::CrashRetry, Effect::CheckRecovered) => {
                if success {
                    State::Recovered
                } else {
                    State::AbortedLedOff
                }
            }
            _ => return,
        };
        self.enter(next, now);
    }
    pub fn reject_geometry(&mut self, now: u64) {
        self.enter(State::RejectedRestore, now);
    }
    pub fn geometry(&mut self, before: bool, monitors: Value, screen: Value) {
        if before {
            self.monitor_before = monitors;
            self.screen_before = screen;
        } else {
            self.monitor_after = monitors;
            self.screen_after = screen;
        }
    }
    pub fn geometry_changed(&self) -> bool {
        self.has_mouse_move
            && ["top", "left", "width", "height"]
                .iter()
                .any(|key| self.screen_before.get(key) != self.screen_after.get(key))
    }
}

/// Zr uses flags&1, followed by Ro's additional exact 0/128 numpad remap.
pub fn normalize_key(item: &mut Value) {
    if !matches!(item["action"].as_str(), Some("keyup" | "keydown")) {
        return;
    }
    if item["flags"].as_u64().unwrap_or(0) & 1 != 0 {
        return;
    }
    let vk = match item["vkCode"].as_u64() {
        Some(45) => 96,
        Some(35) => 97,
        Some(40) => 98,
        Some(34) => 99,
        Some(37) => 100,
        Some(12) => 101,
        Some(39) => 102,
        Some(36) => 103,
        Some(38) => 104,
        Some(33) => 105,
        Some(46) => 110,
        _ => return,
    };
    item["vkCode"] = json!(vk);
}

/// Full source input pair validator. Resetting assignkey clears the collection
/// and trigger grace period; the Fn-held flag survives that reset as in source.
#[derive(Default)]
pub struct AssignmentInput {
    items: Vec<Value>,
    trigger: u64,
    first_at: Option<u64>,
    fn_held: bool,
}
impl AssignmentInput {
    pub fn new(trigger: u64) -> Self {
        Self {
            trigger,
            ..Default::default()
        }
    }
    pub fn reset(&mut self) {
        self.items.clear();
        self.first_at = None;
    }
    pub fn set_trigger(&mut self, scan: u64) {
        self.trigger = scan;
    }
    pub fn push(
        &mut self,
        mut input: Value,
        now: u64,
        layout: u64,
        ignored_razer: &[u64],
    ) -> Result<Option<Value>> {
        let first = *self.first_at.get_or_insert(now);
        let kind = input["type"].as_str().unwrap_or("").to_owned();
        if now.saturating_sub(first) < 300
            && matches!(kind.as_str(), "keyboard" | "analogKey")
            && input["scancode"] == self.trigger
        {
            return Ok(None);
        }
        if !matches!(kind.as_str(), "keyboard" | "analogKey" | "razerKey") {
            return Ok(None);
        }
        ensure!(
            self.items.len() < 100_000,
            "OTFM assignment input limit exceeded"
        );
        self.items.push(input.clone());
        let flag = input["flag"]
            .as_u64()
            .context("OTFM input flag is absent")?;
        if kind == "razerKey" && input["key"] == 1 {
            self.fn_held = flag & 1 == 0;
            return Ok(None);
        }
        if self.fn_held || self.items.len() < 2 || flag & 1 == 0 {
            return Ok(None);
        }
        if kind == "razerKey" {
            let key = input["key"].as_u64().unwrap_or(u64::MAX);
            if ignored_razer.contains(&key)
                || ![32, 33, 34, 35, 36, 84, 85, 83, 82, 80, 81, 100, 101].contains(&key)
            {
                return Ok(None);
            }
        }
        let paired = self.items[..self.items.len() - 1].iter().any(|previous| {
            previous["type"] == kind
                && previous["flag"].as_u64() == flag.checked_sub(1)
                && if kind == "razerKey" {
                    previous["key"] == input["key"]
                } else {
                    previous["scancode"] == input["scancode"]
                }
        });
        if !paired {
            return Ok(None);
        }
        if kind == "analogKey" {
            input["flag"] = json!(flag - 1);
        }
        let keys =
            crate::mapping_engine::source_keys(70).context("Current macro key table is absent")?;
        let keys = keys.iter().filter(|key| {
            key.input_id
                != Some(if [3, 4, 6, 7, 10, 15, 16, 17, 18].contains(&layout) {
                    "KEY_BACKSLASH"
                } else {
                    "KEY_NON_US_POUND"
                })
        });
        if kind == "keyboard" {
            let Some(key) = keys.into_iter().find(|key| {
                u64::from(key.output_flag.unwrap_or(key.flag)) + 1 == flag
                    && Some(u64::from(key.scancode)) == input["scancode"].as_u64()
            }) else {
                return Ok(None);
            };
            return Ok(Some(
                json!({"action":"keyup","scanCode":input["scancode"],"flag":key.output_flag.unwrap_or(key.flag),"timerTick":now,"vkCode":key.virtual_key.parse::<u32>().ok()}),
            ));
        }
        if (kind == "razerKey" && input["key"] == 1)
            || (kind == "analogKey" && input["scancode"] == 0)
        {
            return Ok(None);
        }
        let key = keys.into_iter().find(|key| {
            kind == "analogKey" && Some(u64::from(key.scancode)) == input["scancode"].as_u64()
        });
        input["scanCode"] = if kind == "analogKey" {
            key.map(|key| json!(key.scancode))
                .unwrap_or_else(|| json!(""))
        } else {
            Value::Null
        };
        input["hypershift"] = json!(false);
        input["timerTick"] = json!(now);
        input["virtualKey"] = key
            .and_then(|key| key.virtual_key.parse::<u32>().ok())
            .map(|v| json!(v))
            .unwrap_or(Value::Null);
        Ok(Some(input))
    }
}

#[cfg(test)]
#[path = "otfm_tests.rs"]
mod tests;
