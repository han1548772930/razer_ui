//! Retained shared macro editor state and revision acknowledgements.
use gpui_kit::{Context, EventEmitter};
pub use razer_model::macro_library::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroLibraryChanged;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroLibraryProfileImported;

/// Shared `Entity<MacroLibrary>` content. The shell owns subscriptions and
/// persistence; a completed write acknowledges only the snapshot it wrote.
pub struct MacroLibrary {
    live: MacroLibraryFile,
    saved: MacroLibraryFile,
}

impl EventEmitter<MacroLibraryChanged> for MacroLibrary {}
impl EventEmitter<MacroLibraryProfileImported> for MacroLibrary {}

impl MacroLibrary {
    pub fn new(mut file: MacroLibraryFile) -> Self {
        debug_assert!(file.validate().is_ok());
        let saved = file.clone();
        file.ensure_guids();
        file.restore_recording_preferences();
        Self { saved, live: file }
    }

    pub fn snapshot(&self) -> MacroLibraryFile {
        self.live.clone()
    }

    pub fn saved_snapshot(&self) -> MacroLibraryFile {
        self.saved.clone()
    }

    pub fn pending(&self) -> bool {
        !self.live.same_document(&self.saved)
    }

    pub fn replace(&mut self, mut file: MacroLibraryFile, cx: &mut Context<Self>) {
        debug_assert!(file.validate().is_ok());
        file.ensure_guids();
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

    pub fn publish_profile_import(
        &mut self,
        import: &razer_model::profile_transfer::ProfileMacroImport,
        merged: MacroLibraryFile,
        cx: &mut Context<Self>,
    ) {
        debug_assert!(merged.validate().is_ok());
        self.live = merged;
        self.saved = import.candidate().clone();
        cx.emit(MacroLibraryProfileImported);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_uuid_migration_is_stable_and_needs_persistence() {
        let file = MacroLibraryFile {
            next_id: 2,
            entries: vec![Entry {
                id: 1,
                name: "Macro 1".into(),
                kind: EntryKind::Macro,
                parent: None,
                open: false,
                actions: vec![],
                macro_type: MacroType::Standard,
                active_phase: None,
                record_delay: 0,
                xml_guid: None,
                xml_mouse_mode: 0,
                source_document: None,
                execution: None,
            }],
            ..Default::default()
        };
        let library = MacroLibrary::new(file);
        assert!(library.pending());
        let mut snapshot = library.snapshot();
        let uuid = snapshot.entries[0].xml_guid.clone();
        snapshot.ensure_guids();
        assert_eq!(snapshot.entries[0].xml_guid, uuid);
        assert!(uuid::Uuid::parse_str(uuid.as_deref().unwrap()).is_ok());
    }
}
