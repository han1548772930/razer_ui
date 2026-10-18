//! Current 94608/Te marks the row whose newly recorded chord conflicts.
//! Store59007 clears only warned inputs in its published and persisted copy.
use super::Shortcut;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct InputWarnings {
    ids: BTreeSet<String>,
}

impl InputWarnings {
    pub fn recorded(&mut self, items: &[Shortcut], id: &str) {
        match items.iter().find(|item| item.id() == id) {
            Some(item) if Self::conflicts(items, item) => {
                self.ids.insert(id.to_owned());
            }
            _ => {
                self.ids.remove(id);
            }
        }
    }

    /// Source rows already showing a warning recheck when mappings change.
    /// An existing valid owner is not marked merely because another row changes.
    pub fn refresh(&mut self, items: &[Shortcut]) {
        self.ids.retain(|id| {
            items
                .iter()
                .find(|item| item.id() == id)
                .is_some_and(|item| Self::conflicts(items, item))
        });
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub fn storage_snapshot(&self, items: &[Shortcut]) -> Vec<Shortcut> {
        items
            .iter()
            .cloned()
            .map(|mut item| {
                if self.contains(item.id()) {
                    item.input.clear();
                    item.modifiers.clear();
                }
                item
            })
            .collect()
    }

    fn conflicts(items: &[Shortcut], item: &Shortcut) -> bool {
        !item.input().is_empty()
            && items
                .iter()
                .any(|other| other.id() != item.id() && other.same_chord(item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macro_library::MacroLibraryFile;
    use crate::shortcuts::{Compiler, ShortcutOutput};

    fn shortcut(id: &str, input: &str) -> Shortcut {
        Shortcut::new(
            id.into(),
            input.into(),
            vec!["CTRL".into()],
            false,
            ShortcutOutput::Text { text: id.into() },
        )
    }

    #[test]
    fn later_recording_warns_only_that_row_and_does_not_disable_the_existing_owner() {
        let mut items = vec![shortcut("existing", "KEY_K"), shortcut("later", "KEY_A")];
        let mut warnings = InputWarnings::default();
        items[1].input = "KEY_K".into();
        warnings.recorded(&items, "later");
        assert!(!warnings.contains("existing"));
        assert!(warnings.contains("later"));
        let snapshot = warnings.storage_snapshot(&items);
        assert_eq!(items[1].input(), "KEY_K");
        assert_eq!(snapshot[0], items[0]);
        assert!(snapshot[1].input().is_empty());
        assert!(snapshot[1].modifiers().is_empty());
        let document = Compiler::new()
            .compile(&snapshot, &MacroLibraryFile::default())
            .unwrap();
        assert_eq!(
            document.app_engine()["mappings"].as_array().unwrap().len(),
            6
        );
        assert_eq!(
            document.app_engine()["mappings"][0]["output"]["text"],
            "existing"
        );
        items.remove(0);
        warnings.refresh(&items);
        assert!(!warnings.contains("later"));
        assert_eq!(warnings.storage_snapshot(&items), items);
    }

    #[test]
    fn warning_tracks_the_changed_identity_instead_of_list_order() {
        let mut items = vec![shortcut("first", "KEY_A"), shortcut("second", "KEY_K")];
        let mut warnings = InputWarnings::default();
        items[0].input = "KEY_K".into();
        warnings.recorded(&items, "first");
        assert!(warnings.contains("first"));
        assert!(!warnings.contains("second"));
        items[0].input = "KEY_A".into();
        warnings.recorded(&items, "first");
        assert!(!warnings.contains("first"));
    }
}
