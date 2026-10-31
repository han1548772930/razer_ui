//! The editor's macroEvents and its already compiled appEngine are separate.
//! y.nu appends to existing lanes; Vs rebuilds lanes after an action edit.
use super::*;
use serde_json::{Number, Value, json};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "execution/tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MovementMode {
    #[default]
    None,
    Absolute,
    Foreground,
    Relative,
}

impl MovementMode {
    pub fn from_setting(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Absolute),
            2 => Ok(Self::Foreground),
            3 => Ok(Self::Relative),
            _ => Err("Unknown macro mouse movement setting".into()),
        }
    }

    pub fn setting(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Absolute => 1,
            Self::Foreground => 2,
            Self::Relative => 3,
        }
    }
}

/// A Sequence key can remain unresolved in the editor. Its native consumer
/// must still reject the missing scan code when publication is attempted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScanCode {
    Observed(Number),
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EngineEvent {
    Keyboard {
        scancode: ScanCode,
        flag: Number,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Mouse {
        id: Number,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<Number>,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    MouseMove {
        x: Number,
        y: Number,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Delay {
        ms: Number,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    RandomDelay {
        min_ms: Number,
        max_ms: Number,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Clipboard {
        id: String,
        text: String,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Launch {
        path: String,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Audio {
        id: String,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Loop {
        flag: Number,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        repeat: Option<Number>,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
    Macro {
        guid: String,
        flag: Number,
        #[serde(flatten)]
        extensions: BTreeMap<String, Value>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<EngineEvent>>,
    #[serde(rename = "mouseMoveType", default)]
    pub movement: MovementMode,
    #[serde(
        rename = "pressEvents",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub press: Option<Vec<EngineEvent>>,
    #[serde(
        rename = "holdEvents",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hold: Option<Vec<EngineEvent>>,
    #[serde(
        rename = "releaseEvents",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub release: Option<Vec<EngineEvent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

impl EngineState {
    pub fn ordinary_events(&self) -> Result<&[EngineEvent], String> {
        self.events
            .as_deref()
            .ok_or_else(|| "Macro appEngine ordinary events are absent".into())
    }

    fn validate_lanes(&self, kind: MacroType) -> Result<(), String> {
        if kind != MacroType::Phased {
            self.ordinary_events()?;
        }
        Ok(())
    }

    fn empty(movement: MovementMode) -> Self {
        Self {
            events: Some(Vec::new()),
            movement,
            press: None,
            hold: None,
            release: None,
            hash: None,
            extensions: BTreeMap::new(),
        }
    }

    fn hash(&mut self, kind: MacroType) -> Result<(), String> {
        let input = if kind == MacroType::Phased {
            json!({
                "pressEvents": self.press.as_deref().unwrap_or(&[]),
                "holdEvents": self.hold.as_deref().unwrap_or(&[]),
                "releaseEvents": self.release.as_deref().unwrap_or(&[]),
            })
        } else {
            json!({"mouseMoveType":self.movement,"events":self.ordinary_events()?})
        };
        self.hash = Some(
            razer_device::mapping_engine_hash::source_hash(&input)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    fn lane(&mut self, phase: Option<u8>) -> Result<&mut Vec<EngineEvent>, String> {
        match phase {
            Some(0) => Ok(self.press.get_or_insert_with(Vec::new)),
            Some(1) => Ok(self.hold.get_or_insert_with(Vec::new)),
            Some(2) => Ok(self.release.get_or_insert_with(Vec::new)),
            _ => Err("Current phased macro has no valid active phase".into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSnapshot {
    actions: Vec<ActionItem>,
    engine: EngineState,
    #[serde(default)]
    invalidated: bool,
}

impl ExecutionSnapshot {
    pub fn engine(&self) -> &EngineState {
        &self.engine
    }

    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }

    pub fn matches(&self, actions: &[ActionItem]) -> bool {
        !self.invalidated && self.actions == actions
    }

    pub(super) fn validate(&self, kind: MacroType) -> Result<(), String> {
        self.engine.validate_lanes(kind)
    }
}

pub struct RecordedAppend {
    pub actions: Vec<ActionItem>,
    pub execution: ExecutionSnapshot,
    pub appended: usize,
}

impl Entry {
    pub fn editor_execution(&self) -> Result<ExecutionSnapshot, String> {
        if let Some(execution) = &self.execution {
            execution.validate(self.macro_type)?;
            return Ok(execution.clone());
        }
        let (engine, invalidated) = if let Some(document) = &self.source_document {
            let engine: EngineState = serde_json::from_value(
                document
                    .get("appEngine")
                    .cloned()
                    .ok_or("Macro appEngine is missing")?,
            )
            .map_err(|error| format!("Invalid retained macro appEngine: {error}"))?;
            engine.validate_lanes(self.macro_type)?;
            (engine, self.retained_source_document()?.is_none())
        } else {
            let setting = self
                .actions
                .iter()
                .find_map(|item| item.mouse_movement.as_ref().map(|movement| movement.mode))
                .unwrap_or(self.xml_mouse_mode);
            (
                EngineState::empty(MovementMode::from_setting(setting)?),
                true,
            )
        };
        Ok(ExecutionSnapshot {
            actions: self.actions.clone(),
            engine,
            invalidated,
        })
    }
}

impl MacroLibraryFile {
    /// PV translates the live list, then rebuilds each macro from the first
    /// changed entry onward. LD also replaces savedMacros with that live list.
    pub fn translate_text_actions(
        &mut self,
        mut translate: impl FnMut(&str) -> Option<String>,
    ) -> Result<bool, String> {
        let mut candidate = self.clone();
        let mut changed = false;
        for index in 0..candidate.entries.len() {
            let mut entry = candidate.entries[index].clone();
            if entry.kind != EntryKind::Macro {
                continue;
            }
            for action in &mut entry.actions {
                if action.kind == ActionKind::Text
                    && let Some(key) = &action.translation_key
                    && let Some(text) = translate(key).filter(|text| !text.is_empty())
                    && action.value != text
                {
                    action.value = text;
                    changed = true;
                }
            }
            if changed {
                let execution = candidate.rebuild_translated_execution(&entry)?;
                let setting = execution.engine.movement.setting();
                // $ writes the selected global mode back to actionBar. Keep
                // its other external fields alongside the updated execution.
                if let Some(bar) = entry
                    .source_document
                    .as_mut()
                    .and_then(|document| document.get_mut("macroEvents"))
                    .and_then(Value::as_array_mut)
                    .and_then(|events| events.first_mut())
                {
                    let bar = bar
                        .as_object_mut()
                        .ok_or("Macro action bar is not an object")?;
                    let profile = bar.entry("recordProfile").or_insert_with(|| json!({}));
                    if profile.is_null() {
                        *profile = json!({});
                    }
                    profile
                        .as_object_mut()
                        .ok_or("Macro action bar profile is not an object")?
                        .insert("mmtSetting".into(), setting.into());
                }
                entry.execution = Some(execution);
            }
            candidate.entries[index] = entry;
        }
        if changed {
            candidate.validate()?;
            *self = candidate;
        }
        Ok(changed)
    }

    fn rebuild_translated_execution(&self, entry: &Entry) -> Result<ExecutionSnapshot, String> {
        let previous = entry.editor_execution()?;
        let stored_setting = entry
            .source_document
            .as_ref()
            .and_then(|document| document.get("macroEvents"))
            .and_then(Value::as_array)
            .and_then(|events| events.first())
            .and_then(|bar| bar.get("recordProfile"))
            .and_then(|profile| profile.get("mmtSetting"))
            .filter(|value| !value.is_null())
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| "Invalid macro action bar movement setting".to_string())
            })
            .transpose()?;
        let movement = stored_setting
            .map(MovementMode::from_setting)
            .transpose()?
            .unwrap_or(previous.engine.movement);
        let mut engine = EngineState::empty(movement);
        if entry.macro_type == MacroType::Phased {
            engine.events = None;
            engine.press = Some(Vec::new());
            engine.hold = Some(Vec::new());
            engine.release = Some(Vec::new());
        }
        for action in &entry.actions {
            if entry.macro_type == MacroType::Phased && action.phase.is_none() {
                continue;
            }
            let compiled = self.compile_action(action, stored_setting.unwrap_or(0))?;
            if entry.macro_type == MacroType::Phased {
                if action.phase.is_some() {
                    engine.lane(action.phase)?.extend(compiled);
                }
            } else {
                engine
                    .events
                    .as_mut()
                    .expect("ordinary translation lane")
                    .extend(compiled);
            }
        }
        engine.hash(entry.macro_type)?;
        Ok(ExecutionSnapshot {
            actions: entry.actions.clone(),
            engine,
            invalidated: false,
        })
    }

    pub fn execution_for_actions(
        &self,
        entry: &Entry,
        actions: &[ActionItem],
        draft: Option<&ExecutionSnapshot>,
        conversion_mode: u8,
    ) -> Result<ExecutionSnapshot, String> {
        let previous = draft.or(entry.execution.as_ref());
        if let Some(snapshot) = previous.filter(|snapshot| snapshot.matches(actions)) {
            snapshot.validate(entry.macro_type)?;
            return Ok(snapshot.clone());
        }
        let mode = MovementMode::from_setting(conversion_mode)?;
        let mut engine = if let Some(previous) = previous {
            previous.engine.clone()
        } else if let Some(document) = &entry.source_document {
            let engine: EngineState = serde_json::from_value(
                document
                    .get("appEngine")
                    .cloned()
                    .ok_or("Macro appEngine is missing")?,
            )
            .map_err(|error| format!("Invalid retained macro appEngine: {error}"))?;
            engine.validate_lanes(entry.macro_type)?;
            if actions == entry.actions && entry.retained_source_document()?.is_some() {
                return Ok(ExecutionSnapshot {
                    actions: actions.to_vec(),
                    engine,
                    invalidated: false,
                });
            }
            engine
        } else {
            EngineState::empty(mode)
        };
        if entry.macro_type == MacroType::Phased {
            engine.press = Some(Vec::new());
            engine.hold = Some(Vec::new());
            engine.release = Some(Vec::new());
        } else {
            engine.events = Some(Vec::new());
        }
        for action in actions {
            if entry.macro_type == MacroType::Phased && action.phase.is_none() {
                continue;
            }
            let compiled = self.compile_action(action, conversion_mode)?;
            if entry.macro_type == MacroType::Phased {
                if action.phase.is_some() {
                    engine.lane(action.phase)?.extend(compiled);
                }
            } else {
                engine
                    .events
                    .as_mut()
                    .expect("ordinary lane rebuilt above")
                    .extend(compiled);
            }
        }
        if entry.macro_type != MacroType::Phased && engine.ordinary_events()?.is_empty() {
            engine.movement = MovementMode::None;
        }
        engine.hash(entry.macro_type)?;
        Ok(ExecutionSnapshot {
            actions: actions.to_vec(),
            engine,
            invalidated: false,
        })
    }

    pub fn append_recorded_actions(
        &self,
        entry: &Entry,
        actions: &[ActionItem],
        draft: Option<&ExecutionSnapshot>,
        mut rows: Vec<ActionItem>,
        settings: &recording::Options,
    ) -> Result<RecordedAppend, String> {
        let mut execution =
            self.execution_for_actions(entry, actions, draft, settings.movement_mode)?;
        execution.engine.movement = MovementMode::from_setting(settings.movement_mode)?;
        if entry.macro_type == MacroType::Phased {
            execution.engine.press.get_or_insert_with(Vec::new);
            execution.engine.hold.get_or_insert_with(Vec::new);
            execution.engine.release.get_or_insert_with(Vec::new);
        }
        rows.retain(|row| settings.movement_mode != 0 || row.mouse_movement.is_none());
        for row in &mut rows {
            if row.kind == ActionKind::Delay {
                match entry.record_delay {
                    1 => {
                        row.state = "fixed".into();
                        row.value = settings.fixed.to_string();
                    }
                    2 => {
                        row.state = "randomized".into();
                        row.number_min = settings.random[0].to_string();
                        row.number_max = settings.random[1].to_string();
                    }
                    _ => {}
                }
            }
            if let Some(movement) = &mut row.mouse_movement {
                movement.mode = settings.movement_mode;
            }
            let compiled = self.compile_action(row, settings.movement_mode)?;
            if compiled.is_empty() {
                continue;
            }
            if entry.macro_type == MacroType::Phased {
                execution.engine.lane(entry.active_phase)?.extend(compiled);
            } else {
                execution
                    .engine
                    .events
                    .as_mut()
                    .ok_or("Macro appEngine ordinary events are absent")?
                    .extend(compiled);
            }
        }
        if execution.engine.ordinary_events()?.is_empty() {
            execution.engine.movement = MovementMode::None;
        }
        execution.engine.hash(entry.macro_type)?;
        let appended = rows.len();
        let mut actions = actions.to_vec();
        actions.extend(rows);
        execution.actions = actions.clone();
        execution.invalidated = false;
        Ok(RecordedAppend {
            actions,
            execution,
            appended,
        })
    }

    fn compile_action(
        &self,
        action: &ActionItem,
        movement_mode: u8,
    ) -> Result<Vec<EngineEvent>, String> {
        let event = self.source_action(action)?;
        let geometry = action
            .mouse_movement
            .as_ref()
            .and_then(|movement| movement.geometry.as_ref())
            .map(|geometry| {
                json!({
                    "monitorInforBefore":geometry.monitors_before,
                    "monitorInforAfter":geometry.monitors_after,
                    "virtualScreenInforBefore":geometry.screen_before,
                    "virtualScreenInforAfter":geometry.screen_after,
                })
            });
        let events =
            razer_device::macro_engine::compile_event(&event, movement_mode, geometry.as_ref())
                .map_err(|error| format!("Macro action compilation failed: {error:#}"))?;
        events
            .into_iter()
            .map(|event| {
                serde_json::from_value(event)
                    .map_err(|error| format!("Invalid compiled macro event: {error}"))
            })
            .collect()
    }
}
