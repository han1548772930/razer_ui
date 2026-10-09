//! Shared local Macro documents and their persisted baseline.
//!
//! Actions retain local editor drafts and optional source-shaped key fields.
//! They are not a native macro file, device GUIDs or hardware service data.
//! IDs are local persistent identities; allocation must retain `next_id` as a
//! high-water mark even when entries are deleted. Nothing here sends them to
//! a device.
use gpui_kit::{Context, EventEmitter};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionKind {
    Delay,
    Keyboard,
    Mouse,
    Macro,
    Launch,
    Command,
    Text,
    Loop,
}

/// Existing local event-row data, including both launch targets, both random
/// delay bounds and the source-facing local state string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionItem {
    pub kind: ActionKind,
    /// Source phase 0/1/2. Missing legacy values are never guessed from order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<u8>,
    /// Persistent local nested-macro identity, never a native event GUID.
    /// Older display-only rows remain unassigned; names are not identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_id: Option<u64>,
    /// An unresolved reference read from XML, never a local ID or device read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml_macro_guid: Option<String>,
    /// Source-shaped key data. Missing on legacy text-only rows; text must not
    /// be interpreted as a recorded physical key or an event identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<KeyboardEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse: Option<MouseEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouse_movement: Option<MouseMovement>,
    /// Complete original callback, including movement prefix and timestamps.
    /// Local provenance only; this does not submit events to a device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_input: Option<serde_json::Value>,
    /// Only explicitly created paired Loop rows have this local identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_pair_id: Option<u64>,
    pub value: String,
    pub secondary_value: String,
    pub number_min: String,
    pub number_max: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardEvent {
    /// Local document-scoped pairing identity, never a device GUID.
    pub pair_id: Option<u64>,
    pub makecode: Option<u16>,
    pub state: Option<u8>,
    pub flag: Option<u8>,
    pub key_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MouseEvent {
    pub pair_id: Option<u64>,
    pub button: Option<u8>,
    pub state: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MouseMovement {
    /// File coordinates carry no live screen/monitor observation.
    #[serde(default)]
    pub imported_xml: bool,
    /// Current source mmtSetting: 1 absolute, 2 foreground, 3 start point.
    pub mode: u8,
    /// Source Nr Buffer entries; x/y are untransformed recorder coordinates,
    /// time is milliseconds. Device coordinate encoding is deferred.
    pub buffer: Vec<serde_json::Value>,
    pub geometry: Option<RecordingGeometry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingGeometry {
    pub monitors_before: serde_json::Value,
    pub monitors_after: serde_json::Value,
    pub screen_before: serde_json::Value,
    pub screen_after: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EntryKind {
    Macro,
    Folder,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroType {
    #[default]
    Standard,
    Sequence,
    Phased,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: u64,
    pub name: String,
    pub kind: EntryKind,
    pub parent: Option<u64>,
    /// Expansion belongs to this UI session and defaults closed when loaded.
    #[serde(skip)]
    pub open: bool,
    /// Explicitly saved local editor actions, independent of its undo draft.
    #[serde(default)]
    pub actions: Vec<ActionItem>,
    #[serde(default)]
    pub macro_type: MacroType,
    /// 25572.G saves the currently chosen phase as metadata, not an action edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_phase: Option<u8>,
    /// 25572.G commits the selected recording delay mode to live and saved
    /// macro metadata immediately, independently from the action draft.
    #[serde(default)]
    pub record_delay: u8,
    /// Stable file interchange identity, separate from local IDs and devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml_guid: Option<String>,
    #[serde(default)]
    pub xml_mouse_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Tutorial {
    Initial,
    Record,
    Add,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroLibraryFile {
    pub next_id: u64,
    pub entries: Vec<Entry>,
    pub current: Option<u64>,
    pub tutorial: Tutorial,
}

impl Default for MacroLibraryFile {
    fn default() -> Self {
        Self {
            next_id: 1,
            entries: Vec::new(),
            current: None,
            tutorial: Tutorial::Initial,
        }
    }
}

impl MacroLibraryFile {
    /// Validate the entire forest before allowing it into the local workspace.
    /// Walk ancestors iteratively so malformed deep files cannot recurse into
    /// the UI's tree traversal before missing parents and cycles are rejected.
    pub fn validate(&self) -> Result<(), String> {
        if self.next_id == 0 || self.next_id == u64::MAX {
            return Err("Macro next_id must be nonzero and below u64::MAX".into());
        }
        let mut entries = HashMap::with_capacity(self.entries.len());
        let mut xml_guids = HashSet::new();
        for entry in &self.entries {
            if entry.active_phase.is_some_and(|phase| phase > 2) {
                return Err(format!("Invalid active phase in {}", entry.id));
            }
            if entry.record_delay > 3 {
                return Err(format!("Invalid recording delay mode in {}", entry.id));
            }
            if entry.xml_mouse_mode > 3 {
                return Err(format!("Invalid XML mouse mode in {}", entry.id));
            }
            if let Some(id) = &entry.xml_guid {
                let guid = uuid::Uuid::parse_str(id)
                    .map_err(|_| format!("Invalid XML Macro GUID in {}", entry.id))?;
                if entry.kind != EntryKind::Macro || !xml_guids.insert(guid) {
                    return Err(format!("Non-unique XML Macro GUID in {}", entry.id));
                }
            }
            if entry.id == 0 {
                return Err("Macro entry IDs must be nonzero".into());
            }
            if entries.insert(entry.id, entry).is_some() {
                return Err(format!("Duplicate Macro entry ID {}", entry.id));
            }
            if entry.id >= self.next_id {
                return Err(format!(
                    "Macro next_id {} must exceed entry ID {}",
                    self.next_id, entry.id
                ));
            }
            if entry.kind == EntryKind::Folder && !entry.actions.is_empty() {
                return Err(format!("Macro folder {} cannot contain actions", entry.id));
            }
        }
        for entry in &self.entries {
            let mut key_pairs = HashMap::<u64, usize>::new();
            let mut mouse_pairs = HashMap::<u64, usize>::new();
            let mut loop_pairs = HashMap::<u64, usize>::new();
            for action in &entry.actions {
                if action.xml_macro_guid.as_ref().is_some_and(|guid| {
                    action.kind != ActionKind::Macro || uuid::Uuid::parse_str(guid).is_err()
                }) {
                    return Err(format!("Invalid nested XML Macro GUID in {}", entry.id));
                }
                if let Some(movement) = &action.mouse_movement {
                    if action.kind != ActionKind::Mouse
                        || action.mouse.is_some()
                        || !(1..=3).contains(&movement.mode)
                        || movement.buffer.is_empty()
                        || (movement.geometry.is_none() && !movement.imported_xml)
                        || movement.buffer.iter().any(|point| {
                            ["x", "y", "time"].iter().any(|field| {
                                point
                                    .get(field)
                                    .and_then(serde_json::Value::as_f64)
                                    .is_none_or(|n| !n.is_finite())
                            }) || point["time"].as_f64().is_some_and(|time| time < 0.)
                        })
                    {
                        return Err(format!("Invalid recorded mouse movement in {}", entry.id));
                    }
                }
                if action.phase.is_some_and(|phase| phase > 2) {
                    return Err(format!("Invalid action phase in {}", entry.id));
                }
                if let Some(mouse) = &action.mouse {
                    if action.kind != ActionKind::Mouse
                        || mouse.pair_id == Some(0)
                        || mouse.button.is_some_and(|button| button > 9)
                        || mouse.state.is_some_and(|state| state > 1)
                    {
                        return Err(format!("Invalid mouse metadata in {}", entry.id));
                    }
                    if let Some(id) = mouse.pair_id {
                        let count = mouse_pairs.entry(id).or_default();
                        *count += 1;
                        if *count > 2 {
                            return Err(format!("Ambiguous mouse pair {id} in {}", entry.id));
                        }
                    }
                }
                if let Some(id) = action.loop_pair_id {
                    if action.kind != ActionKind::Loop
                        || id == 0
                        || !matches!(action.state.as_str(), "start" | "end")
                    {
                        return Err(format!("Invalid Loop metadata in {}", entry.id));
                    }
                    let count = loop_pairs.entry(id).or_default();
                    *count += 1;
                    if *count > 2 {
                        return Err(format!("Ambiguous Loop pair {id} in {}", entry.id));
                    }
                }
                if let Some(key) = &action.keyboard {
                    if action.kind != ActionKind::Keyboard || key.pair_id == Some(0) {
                        return Err(format!("Invalid keyboard metadata in {}", entry.id));
                    }
                    if let Some(id) = key.pair_id {
                        let count = key_pairs.entry(id).or_default();
                        *count += 1;
                        if *count > 2 {
                            return Err(format!("Ambiguous keyboard pair {id} in {}", entry.id));
                        }
                    }
                }
                if let Some(id) = action.macro_id {
                    if action.kind != ActionKind::Macro || id == 0 || id >= self.next_id {
                        return Err(format!(
                            "Invalid nested Macro reference {id} in {}",
                            entry.id
                        ));
                    }
                    // Deleted targets stay dangling below the high-water mark;
                    // never allow a future allocation to reuse their identity.
                    if entries
                        .get(&id)
                        .is_some_and(|target| target.kind != EntryKind::Macro)
                    {
                        return Err(format!("Nested Macro reference {id} points to a folder"));
                    }
                }
            }
            if let Some(parent) = entry.parent {
                let Some(parent_entry) = entries.get(&parent) else {
                    return Err(format!(
                        "Macro entry {} refers to missing parent {parent}",
                        entry.id
                    ));
                };
                if parent_entry.kind != EntryKind::Folder {
                    return Err(format!(
                        "Macro entry {} parent {parent} must be a folder",
                        entry.id
                    ));
                }
            }
        }
        let mut complete = HashSet::with_capacity(entries.len());
        for entry in &self.entries {
            let mut ancestors = HashSet::new();
            let mut cursor = Some(entry.id);
            while let Some(id) = cursor {
                if complete.contains(&id) {
                    break;
                }
                if !ancestors.insert(id) {
                    return Err(format!("Macro parent cycle contains entry {id}"));
                }
                // Parent existence was checked above, before any traversal.
                cursor = entries[&id].parent;
            }
            complete.extend(ancestors);
        }
        if let Some(current) = self.current {
            if entries
                .get(&current)
                .is_none_or(|entry| entry.kind != EntryKind::Macro)
            {
                return Err(format!(
                    "Current Macro entry {current} must refer to an existing macro"
                ));
            }
        }
        Ok(())
    }

    /// Match exactly the serializable fields. Folder expansion must not make
    /// a successfully saved document look dirty after deserialization.
    fn same_document(&self, other: &Self) -> bool {
        self.next_id == other.next_id
            && self.current == other.current
            && self.tutorial == other.tutorial
            && self.entries.len() == other.entries.len()
            && self.entries.iter().zip(&other.entries).all(|(a, b)| {
                a.id == b.id
                    && a.name == b.name
                    && a.kind == b.kind
                    && a.parent == b.parent
                    && a.actions == b.actions
                    && a.macro_type == b.macro_type
                    && a.active_phase == b.active_phase
                    && a.record_delay == b.record_delay
                    && a.xml_guid == b.xml_guid
                    && a.xml_mouse_mode == b.xml_mouse_mode
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroLibraryChanged;

/// Shared `Entity<MacroLibrary>` content. The shell owns subscriptions and
/// persistence; a completed write acknowledges only the snapshot it wrote.
pub struct MacroLibrary {
    live: MacroLibraryFile,
    saved: MacroLibraryFile,
}

impl EventEmitter<MacroLibraryChanged> for MacroLibrary {}

impl MacroLibrary {
    pub fn new(file: MacroLibraryFile) -> Self {
        debug_assert!(file.validate().is_ok());
        Self {
            saved: file.clone(),
            live: file,
        }
    }

    pub fn snapshot(&self) -> MacroLibraryFile {
        self.live.clone()
    }

    pub fn pending(&self) -> bool {
        !self.live.same_document(&self.saved)
    }

    pub fn replace(&mut self, file: MacroLibraryFile, cx: &mut Context<Self>) {
        debug_assert!(file.validate().is_ok());
        if self.live == file {
            return;
        }
        self.live = file;
        cx.emit(MacroLibraryChanged);
        cx.notify();
    }

    pub fn mark_saved(&mut self, file: MacroLibraryFile, cx: &mut Context<Self>) {
        debug_assert!(file.validate().is_ok());
        if self.saved == file {
            return;
        }
        self.saved = file;
        cx.notify();
    }
}

impl ActionItem {
    pub fn new(kind: ActionKind) -> Self {
        let value = match kind {
            ActionKind::Delay => "0".to_string(),
            ActionKind::Keyboard
            | ActionKind::Mouse
            | ActionKind::Macro
            | ActionKind::Launch
            | ActionKind::Command => String::new(),
            ActionKind::Text => String::new(),
            ActionKind::Loop => "1".to_string(),
        };
        let state = match kind {
            ActionKind::Loop => "start".to_string(),
            // Current source template starts with RadioIndex:null.
            ActionKind::Launch => String::new(),
            ActionKind::Delay => "fixed".to_string(),
            _ => String::new(),
        };
        Self {
            kind,
            phase: None,
            macro_id: None,
            xml_macro_guid: None,
            keyboard: None,
            mouse: None,
            mouse_movement: None,
            recorded_input: None,
            loop_pair_id: None,
            value,
            secondary_value: String::new(),
            number_min: if kind == ActionKind::Delay {
                "0.000".to_string()
            } else {
                String::new()
            },
            number_max: if kind == ActionKind::Delay {
                "0.000".to_string()
            } else {
                String::new()
            },
            state,
        }
    }
}

impl ActionKind {
    pub fn from_palette(kind: &str) -> Option<Self> {
        Some(match kind {
            "delay" => Self::Delay,
            "keyboard" => Self::Keyboard,
            "mouse" => Self::Mouse,
            "macro" => Self::Macro,
            "launch" => Self::Launch,
            "command" => Self::Command,
            "text" => Self::Text,
            "loop" => Self::Loop,
            _ => return None,
        })
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Delay => "delay",
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Macro => "macro",
            Self::Launch => "launch",
            Self::Command => "command",
            Self::Text => "text",
            Self::Loop => "loop",
        }
    }
}
