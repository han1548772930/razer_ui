//! Local executable selection metadata. Does not load or start the file.
use razer_storage::file_transfer::Operation;
use std::path::{Path, PathBuf};

pub struct SelectedExecutable {
    name: String,
    path: PathBuf,
}

impl SelectedExecutable {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub enum SelectionError {
    Read(String),
    NotExecutable,
    MissingName,
    Cancelled(String),
}

/// The caller accepts the operation before scheduling this filesystem work.
pub fn inspect(path: &Path, operation: &Operation) -> Result<SelectedExecutable, SelectionError> {
    operation.check().map_err(SelectionError::Cancelled)?;
    let canonical = path
        .canonicalize()
        .map_err(|error| SelectionError::Read(error.to_string()))?;
    operation.check().map_err(SelectionError::Cancelled)?;
    let metadata =
        std::fs::metadata(&canonical).map_err(|error| SelectionError::Read(error.to_string()))?;
    if !metadata.is_file()
        || !canonical
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err(SelectionError::NotExecutable);
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or(SelectionError::MissingName)?
        .to_owned();
    operation.check().map_err(SelectionError::Cancelled)?;
    Ok(SelectedExecutable {
        name,
        path: canonical,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_storage::file_transfer::Transfer;

    #[test]
    fn selection_observes_a_regular_executable_without_executing_it() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "razer-executable-selection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let executable = root.join("fixture.EXE");
        let document = root.join("fixture.txt");
        std::fs::write(&executable, b"static test bytes").unwrap();
        std::fs::write(&document, b"static test bytes").unwrap();
        let operation = Transfer::new().accept().unwrap();
        let selected = inspect(&executable, &operation).ok().unwrap();
        assert_eq!(selected.name(), "fixture");
        assert_eq!(selected.path(), executable.canonicalize().unwrap());
        assert!(matches!(
            inspect(&document, &operation),
            Err(SelectionError::NotExecutable)
        ));
        assert!(matches!(
            inspect(&root, &operation),
            Err(SelectionError::NotExecutable)
        ));
        assert!(matches!(
            inspect(&root.join("absent.exe"), &operation),
            Err(SelectionError::Read(_))
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancelled_selection_stops_before_missing_path_lookup() {
        let request = Transfer::new();
        let operation = request.accept().unwrap();
        request.cancel();
        assert!(matches!(
            inspect(Path::new("absent.exe"), &operation),
            Err(SelectionError::Cancelled(_))
        ));
    }
}
