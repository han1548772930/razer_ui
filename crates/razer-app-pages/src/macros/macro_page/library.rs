//! Current U -> 25572.B remoteUpdate consumer and 88152.S2 profile identities.
use super::*;
use razer_pages::features::macro_library::MacroLibraryFile;
use std::collections::HashMap;

fn same_macros(previous: &[Entry], next: &[Entry]) -> bool {
    let previous: Vec<_> = previous
        .iter()
        .filter(|entry| entry.kind == EntryKind::Macro)
        .collect();
    let next: Vec<_> = next
        .iter()
        .filter(|entry| entry.kind == EntryKind::Macro)
        .collect();
    previous.len() == next.len()
        && previous.iter().all(|a| {
            next.iter()
                .find(|b| a.xml_guid == b.xml_guid)
                .is_some_and(|b| {
                    a.name == b.name
                        && a.actions == b.actions
                        && a.macro_type == b.macro_type
                        && a.active_phase == b.active_phase
                        && a.record_delay == b.record_delay
                        && a.xml_mouse_mode == b.xml_mouse_mode
                        && a.source_document == b.source_document
                        && a.execution == b.execution
                })
        })
}

fn same_entry(previous: &Entry, next: &Entry) -> bool {
    previous.kind == next.kind
        && match previous.kind {
            EntryKind::Macro => previous.xml_guid == next.xml_guid,
            EntryKind::Folder => previous.id == next.id,
        }
}

impl MacroPage {
    pub(super) fn refresh_translated_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut candidate = self.library_snapshot.clone();
        candidate.entries = self.entries.clone();
        for entry in &mut candidate.entries {
            if self.actions_for == Some(entry.id) {
                entry.actions = self.actions.clone();
                entry.execution = self.execution.clone();
            } else if let Some(draft) = self.inactive_drafts.get(&entry.id) {
                entry.actions = draft.actions.clone();
                entry.execution = draft.execution.clone();
            }
        }
        match candidate.translate_text_actions(|key| Some(tr(key))) {
            Ok(false) => return,
            Err(error) => {
                self.recording.error = Some(error);
                return;
            }
            Ok(true) => {}
        }
        let previous_text = self.editing_action.and_then(|index| self.actions.get(index))
            .filter(|action| action.kind == ActionKind::Text).map(|action| action.value.clone());
        self.actions_dirty |= self.actions != self.saved_actions;
        for (id, draft) in &mut self.inactive_drafts {
            if let Some(previous) = self.entries.iter().find(|entry| entry.id == *id) {
                draft.actions_dirty |= draft.actions != previous.actions;
            }
            if let Some(entry) = candidate.entries.iter().find(|entry| entry.id == *id) {
                draft.actions = entry.actions.clone();
                draft.execution = entry.execution.clone();
            }
        }
        self.entries = candidate.entries;
        if let Some(entry) = self.entries.iter().find(|entry| Some(entry.id) == self.current) {
            self.actions = entry.actions.clone();
            self.saved_actions = entry.actions.clone();
            self.execution = entry.execution.clone();
        }
        if let Some(index) = self.editing_action
            && let Some(value) = self.actions.get(index).filter(|action| action.kind == ActionKind::Text)
                .map(|action| action.value.clone())
            && previous_text.as_ref() != Some(&value)
        {
            self.text_ui.previous = value.clone();
            self.text_row(index, cx).borrow_mut().armed |= !value.is_empty();
            self.text_editor.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.publish_library(cx);
        self.refresh_recording_preview();
        self.update_recording_preview_options(cx);
    }

    pub(super) fn load_current_execution(&mut self) {
        self.execution = None;
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| Some(entry.id) == self.current)
        {
            match entry.editor_execution() {
                Ok(execution) => self.execution = Some(execution),
                Err(error) => self.recording.error = Some(error),
            }
        }
    }

    pub(super) fn invalidate_execution(&mut self) {
        self.actions_dirty = true;
        if let Some(execution) = &mut self.execution {
            execution.invalidate();
        }
    }

    pub(super) fn checkpoint_actions(&mut self) {
        self.undo.push(self.actions.clone());
        self.invalidate_execution();
    }

    pub(super) fn rebuild_action_execution(&mut self) {
        let Some(entry) = self.entries.iter().find(|entry|Some(entry.id)==self.current) else { return; };
        match self.library_snapshot.execution_for_actions(
            entry,&self.actions,self.execution.as_ref(),self.recording_movement_setting(),
        ) {
            Ok(execution) => self.execution=Some(execution),
            Err(error) => self.recording.error=Some(error),
        }
    }

    pub(super) fn publish_library(&mut self, cx: &mut Context<Self>) {
        self.update_phased_mount();
        let mut entries = self.entries.clone();
        for entry in &mut entries {
            entry.open = false;
        }
        let file = MacroLibraryFile {
            next_id: self.next_id,
            entries,
            current: self.current,
            tutorial: self.tutorial,
            record_shortcut: self.record_ui.shortcut.selected.clone(),
            recording_preferences: Some(self.record_preferences()),
        };
        self.library_snapshot = self.library.update(cx, |library, cx| {
            library.replace(file, cx);
            library.snapshot()
        });
    }

    pub(super) fn receive_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.library.read(cx).snapshot();
        // GPUI delivers emitted events at the end of the owning update. A
        // page's own metadata/save notification must not discard its draft.
        if snapshot != self.library_snapshot {
            self.apply_library_snapshot(snapshot, false, window, cx);
        }
    }

    pub(super) fn reload_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_phased_mount();
        let mut snapshot = self.library.read(cx).snapshot();
        let preferences = snapshot.recording_preferences.clone();
        snapshot.restore_recording_preferences();
        let restored = snapshot.recording_preferences != preferences;
        self.apply_library_snapshot(snapshot, true, window, cx);
        if restored {
            self.publish_library(cx);
        }
    }

    fn apply_library_snapshot(
        &mut self,
        snapshot: MacroLibraryFile,
        reload: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let replace_actions =
            reload || !same_macros(&self.library_snapshot.entries, &snapshot.entries);
        let old_current = self
            .entries
            .iter()
            .find(|entry| Some(entry.id) == self.current);
        let new_current = snapshot
            .entries
            .iter()
            .find(|entry| Some(entry.id) == snapshot.current);
        let changed_current = self.current != snapshot.current
            || match (old_current, new_current) {
                (Some(previous), Some(next)) => !same_entry(previous, next),
                (None, None) => false,
                _ => true,
            };
        if replace_actions || changed_current {
            // New macroEvents invalidate row-index editors and delayed commits;
            // the independently mounted recording settings remain open.
            self.cancel_action_editors();
        }
        if let Some(id) = self.actions_for.filter(|id| Some(*id) == self.current) {
            self.inactive_drafts.insert(
                id,
                unsaved::ActionDraft {
                    actions: self.actions.clone(),
                    undo: self.undo.clone(),
                    redo: self.redo.clone(),
                    selected: self.selected_actions.clone(),
                    actions_dirty: self.actions_dirty,
                    execution: self.execution.clone(),
                },
            );
        }
        let mut drafts = std::mem::take(&mut self.inactive_drafts);
        let mut by_guid: HashMap<_, _> = self
            .entries
            .iter()
            .filter_map(|entry| Some((entry.xml_guid.clone()?, drafts.remove(&entry.id)?)))
            .collect();
        let previous = std::mem::replace(&mut self.entries, snapshot.entries.clone());
        for entry in &mut self.entries {
            if let Some(old) = previous.iter().find(|old| same_entry(old, entry)) {
                entry.open = old.open;
            }
            if entry.kind != EntryKind::Macro || reload {
                continue;
            }
            if let Some(mut draft) = entry
                .xml_guid
                .as_ref()
                .and_then(|guid| by_guid.remove(guid))
            {
                if replace_actions {
                    // B replaces events and savedMacros, retaining only the
                    // matching GUID's previousAction/nextAction history.
                    draft.actions = entry.actions.clone();
                    draft.selected.clear();
                    draft.actions_dirty = false;
                    draft.execution = match entry.editor_execution() {
                        Ok(execution) => Some(execution),
                        Err(error) => {
                            self.recording.error = Some(error);
                            None
                        }
                    };
                }
                self.inactive_drafts.insert(entry.id, draft);
            }
        }
        self.next_id = snapshot.next_id;
        self.current = snapshot.current;
        if reload {
            self.tutorial = snapshot.tutorial;
        }
        // B explicitly retains firstTime for an external editor update.
        self.saved_actions_for = self.current;
        self.saved_actions = new_current
            .map(|entry| entry.actions.clone())
            .unwrap_or_default();
        if let Some(draft) = self.current.and_then(|id| self.inactive_drafts.remove(&id)) {
            self.actions = draft.actions;
            self.undo = draft.undo;
            self.redo = draft.redo;
            self.selected_actions = draft.selected;
            self.actions_dirty = draft.actions_dirty;
            self.execution = draft.execution;
        } else {
            self.actions = self.saved_actions.clone();
            self.undo.clear();
            self.redo.clear();
            self.selected_actions.clear();
            self.actions_dirty = false;
            self.load_current_execution();
        }
        self.actions_for = self.current;
        if changed_current {
            self.tree_selection = self.current;
        }
        let still_present = |id: u64| {
            previous
                .iter()
                .find(|entry| entry.id == id)
                .is_some_and(|old| {
                    self.entries
                        .iter()
                        .any(|entry| entry.id == id && same_entry(old, entry))
                })
        };
        if !changed_current {
            self.tree_selection = self.tree_selection.filter(|id| still_present(*id));
        }
        self.tree_menu = self.tree_menu.filter(|id| still_present(*id));
        self.tree_hover = self.tree_hover.filter(|id| still_present(*id));
        self.rename = self.rename.filter(|id| still_present(*id));
        if self.rename.is_none() {
            self.rename_in_tree = false;
        }
        self.deletion = self.deletion.filter(|id| still_present(*id));
        if let Some(unsaved::PendingAction::Select(id)) = self.suspended_action
            && !still_present(id)
        {
            self.cancel_suspended_action(window, cx);
        }
        self.reconcile_library_bindings(&previous, window, cx);
        self.adopt_record_shortcut(snapshot.record_shortcut.clone(), window, cx);
        if reload || snapshot.recording_preferences != self.library_snapshot.recording_preferences {
            self.adopt_record_preferences(snapshot.recording_preferences.clone(), window, cx);
        }
        self.library_snapshot = snapshot;
        self.sync_phased_mount(cx);
        self.refresh_recording_preview();
        self.update_recording_preview_options(cx);
        cx.notify();
    }
}
