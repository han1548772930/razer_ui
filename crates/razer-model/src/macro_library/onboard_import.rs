//! Two local commits in the current OBM import consumer: append source
//! documents, then update the macro index and its initial selection.
use super::MacroLibraryFile;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Documents,
    Index,
}

#[derive(Clone)]
pub struct Import {
    baseline: MacroLibraryFile,
    candidate: MacroLibraryFile,
    guids: Vec<String>,
    phase: Phase,
}

impl Import {
    /// This boundary accepts decoded source documents from the device worker.
    pub fn prepare(library: &MacroLibraryFile, documents: &[Value]) -> Result<Self, String> {
        if documents.is_empty() {
            return Err("Onboard macro import has no documents".into());
        }
        library.validate()?;
        let mut candidate = library.clone();
        candidate.append_source_documents(documents)?;
        let guids = candidate.entries[library.entries.len()..]
            .iter()
            .map(|entry| {
                entry
                    .xml_guid
                    .clone()
                    .ok_or("Imported macro has no GUID".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            baseline: library.clone(),
            candidate,
            guids,
            phase: Phase::Documents,
        })
    }
    pub fn candidate(&self) -> &MacroLibraryFile {
        &self.candidate
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn matches(&self, current: &MacroLibraryFile) -> bool {
        self.baseline.same_document(current)
    }

    /// Called after the document commit and its live mirror publication. The
    /// index timestamp is taken at this execution point, not when queued.
    pub fn prepare_index(
        &self,
        current: &MacroLibraryFile,
        now: impl Fn() -> Result<u64, String>,
    ) -> Result<Self, String> {
        if self.phase != Phase::Documents || !self.candidate.same_document(current) {
            return Err("Onboard macro document owner changed before index publication".into());
        }
        let mut candidate = current.clone();
        for guid in &self.guids {
            let entry = candidate
                .entries
                .iter_mut()
                .find(|entry| entry.xml_guid.as_ref() == Some(guid))
                .ok_or("Imported macro disappeared before index publication")?;
            entry.imported_at_millis = Some(now()?);
        }
        if candidate.current.is_none() {
            candidate.current = candidate
                .entries
                .iter()
                .find(|entry| entry.xml_guid.as_ref() == self.guids.first())
                .map(|entry| entry.id);
        }
        candidate.validate()?;
        Ok(Self {
            baseline: current.clone(),
            candidate,
            guids: self.guids.clone(),
            phase: Phase::Index,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn document_commit_precedes_index_and_index_failure_preserves_documents() {
        let library = MacroLibraryFile::default();
        let source = json!({"guid":"3067b9df-40a4-4bc3-a9ea-2380195fe948","name":"hardware","macroEvents":[],
            "appEngine":{"events":[],"hash":"","mouseMoveType":"none"}});
        let import = Import::prepare(&library, &[source]).unwrap();
        assert_eq!(import.phase(), Phase::Documents);
        let committed = import.candidate().clone();
        assert_eq!(committed.current, None);
        assert_eq!(committed.entries[0].imported_at_millis, None);
        assert!(
            import
                .prepare_index(&committed, || Err("clock failed".into()))
                .is_err()
        );
        assert_eq!(committed.entries.len(), 1);
        let index = import.prepare_index(&committed, || Ok(100)).unwrap();
        assert_eq!(index.candidate().current, Some(committed.entries[0].id));
        assert_eq!(index.candidate().entries[0].imported_at_millis, Some(100));
        assert!(!index.candidate().same_document(&committed));
    }
}
