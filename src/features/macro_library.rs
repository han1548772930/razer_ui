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
pub(crate) enum ActionKind {
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
pub(crate) struct ActionItem {
    pub(crate) kind: ActionKind,
    /// Persistent local nested-macro identity, never a native event GUID.
    /// Older display-only rows remain unassigned; names are not identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) macro_id: Option<u64>,
    /// Source-shaped key data. Missing on legacy text-only rows; text must not
    /// be interpreted as a recorded physical key or an event identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) keyboard: Option<KeyboardEvent>,
    pub(crate) value: String,
    pub(crate) secondary_value: String,
    pub(crate) number_min: String,
    pub(crate) number_max: String,
    pub(crate) state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeyboardEvent {
    /// Local document-scoped pairing identity, never a device GUID.
    pub(crate) pair_id: Option<u64>,
    pub(crate) makecode: Option<u16>,
    pub(crate) state: Option<u8>,
    pub(crate) flag: Option<u8>,
    pub(crate) key_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EntryKind {
    Macro,
    Folder,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MacroType {
    #[default]
    Standard,
    Sequence,
    Phased,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) kind: EntryKind,
    pub(crate) parent: Option<u64>,
    /// Expansion belongs to this UI session and defaults closed when loaded.
    #[serde(skip)]
    pub(crate) open: bool,
    /// Explicitly saved local editor actions, independent of its undo draft.
    #[serde(default)]
    pub(crate) actions: Vec<ActionItem>,
    #[serde(default)]
    pub(crate) macro_type: MacroType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Tutorial {
    Initial,
    Record,
    Add,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MacroLibraryFile {
    pub(crate) next_id: u64,
    pub(crate) entries: Vec<Entry>,
    pub(crate) current: Option<u64>,
    pub(crate) tutorial: Tutorial,
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
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.next_id == 0 || self.next_id == u64::MAX {
            return Err("Macro next_id must be nonzero and below u64::MAX".into());
        }
        let mut entries = HashMap::with_capacity(self.entries.len());
        for entry in &self.entries {
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
            for action in &entry.actions {
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
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MacroLibraryChanged;

/// Shared `Entity<MacroLibrary>` content. The shell owns subscriptions and
/// persistence; a completed write acknowledges only the snapshot it wrote.
pub(crate) struct MacroLibrary {
    live: MacroLibraryFile,
    saved: MacroLibraryFile,
}

impl EventEmitter<MacroLibraryChanged> for MacroLibrary {}

impl MacroLibrary {
    pub(crate) fn new(file: MacroLibraryFile) -> Self {
        debug_assert!(file.validate().is_ok());
        Self {
            saved: file.clone(),
            live: file,
        }
    }

    pub(crate) fn snapshot(&self) -> MacroLibraryFile {
        self.live.clone()
    }

    pub(crate) fn pending(&self) -> bool {
        !self.live.same_document(&self.saved)
    }

    pub(crate) fn replace(&mut self, file: MacroLibraryFile, cx: &mut Context<Self>) {
        debug_assert!(file.validate().is_ok());
        if self.live == file {
            return;
        }
        self.live = file;
        cx.emit(MacroLibraryChanged);
        cx.notify();
    }

    pub(crate) fn mark_saved(&mut self, file: MacroLibraryFile, cx: &mut Context<Self>) {
        debug_assert!(file.validate().is_ok());
        if self.saved == file {
            return;
        }
        self.saved = file;
        cx.notify();
    }
}
