//! Local file selection with a retained cancellation owner. No file is executed.
use anyhow::Result;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

static ACTIVE_DIALOGS: AtomicUsize = AtomicUsize::new(0);
pub fn active_dialogs() -> usize {
    ACTIVE_DIALOGS.load(Ordering::Acquire)
}
struct DialogActivity;
impl DialogActivity {
    fn new() -> Self {
        ACTIVE_DIALOGS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for DialogActivity {
    fn drop(&mut self) {
        ACTIVE_DIALOGS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct OpenFileDialog {
    cancelled: Arc<AtomicBool>,
    result: async_channel::Receiver<Result<Option<Vec<PathBuf>>>>,
}
impl OpenFileDialog {
    /// `owner` is the calling window's native handle; the dialog is modal to it.
    pub fn start(owner: isize, title: &str, extensions: &[&str], multiple: bool) -> Result<Self> {
        let filters = extension_filter(extensions)?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, result) = async_channel::bounded(1);
        let cancel = cancelled.clone();
        let title = title.to_owned();
        let activity = DialogActivity::new();
        std::thread::Builder::new()
            .name("local-file-dialog".into())
            .spawn(move || {
                let _activity = activity;
                let outcome = platform::show(owner, &title, &filters, multiple, cancel);
                let _ = sender.send_blocking(outcome);
            })?;
        Ok(Self { cancelled, result })
    }

    pub async fn result(&self) -> Result<Option<Vec<PathBuf>>> {
        self.result
            .recv()
            .await
            .map_err(|_| anyhow::anyhow!("Local file dialog ended without a result"))?
    }
}

#[derive(Clone, Debug)]
pub struct SaveFileOptions {
    pub title: String,
    pub file_name: String,
    pub button_label: String,
    pub filter_name: String,
    pub extension: String,
    pub directory: Option<PathBuf>,
}
pub struct SaveFileDialog {
    cancelled: Arc<AtomicBool>,
    finished: AtomicBool,
    result: async_channel::Receiver<Result<Option<PathBuf>>>,
}
impl SaveFileDialog {
    /// The request owner may cancel while the modal COM loop is running.
    pub fn start(
        owner: isize,
        options: SaveFileOptions,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        extension_filter(&[&options.extension])?;
        anyhow::ensure!(
            [
                &options.title,
                &options.file_name,
                &options.button_label,
                &options.filter_name
            ]
            .into_iter()
            .all(|text| !text.contains('\0')),
            "Native save-dialog text contains NUL"
        );
        let (sender, result) = async_channel::bounded(1);
        let cancel = cancelled.clone();
        let activity = DialogActivity::new();
        std::thread::Builder::new()
            .name("local-save-dialog".into())
            .spawn(move || {
                let _activity = activity;
                let outcome = platform::save(owner, &options, cancel);
                let _ = sender.send_blocking(outcome);
            })?;
        Ok(Self {
            cancelled,
            finished: AtomicBool::new(false),
            result,
        })
    }
    pub async fn result(&self) -> Result<Option<PathBuf>> {
        let result = self
            .result
            .recv()
            .await
            .map_err(|_| anyhow::anyhow!("Local save dialog ended without a result"));
        self.finished.store(true, Ordering::Release);
        result?
    }
    /// Wait only from a retained background worker, never the GUI executor.
    pub fn blocking_result(&self) -> Result<Option<PathBuf>> {
        let result = self
            .result
            .recv_blocking()
            .map_err(|_| anyhow::anyhow!("Local save dialog ended without a result"));
        self.finished.store(true, Ordering::Release);
        result?
    }
}
impl Drop for SaveFileDialog {
    fn drop(&mut self) {
        if !self.finished.load(Ordering::Acquire) {
            self.cancelled.store(true, Ordering::Release);
        }
    }
}
impl Drop for OpenFileDialog {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
fn extension_filter(extensions: &[&str]) -> Result<String> {
    anyhow::ensure!(!extensions.is_empty(), "A local file filter is required");
    extensions
        .iter()
        .map(|extension| {
            let extension = extension.trim_start_matches('.');
            anyhow::ensure!(
                !extension.is_empty() && extension.bytes().all(|c| c.is_ascii_alphanumeric()),
                "Invalid local file extension"
            );
            Ok(format!("*.{extension}"))
        })
        .collect::<Result<Vec<_>>>()
        .map(|filters| filters.join(";"))
}

#[cfg(windows)]
#[path = "platform/windows/file_dialog.rs"]
mod platform;
#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn show(
        _: isize,
        _: &str,
        _: &str,
        _: bool,
        _: Arc<AtomicBool>,
    ) -> Result<Option<Vec<PathBuf>>> {
        anyhow::bail!("Native filtered file selection is unavailable on this platform")
    }
    pub fn save(_: isize, _: &SaveFileOptions, _: Arc<AtomicBool>) -> Result<Option<PathBuf>> {
        anyhow::bail!("Native filtered file saving is unavailable on this platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_extensions_become_native_filters() {
        assert_eq!(extension_filter(&[".exe"]).unwrap(), "*.exe");
        assert_eq!(extension_filter(&[".png", ".jpg"]).unwrap(), "*.png;*.jpg");
        for input in [vec![], vec!["."], vec!["exe;*"], vec!["../exe"]] {
            assert!(extension_filter(&input).is_err());
        }
    }
    #[test]
    fn dropping_owner_requests_cancellation_and_releases_receiver() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, result) = async_channel::bounded(1);
        drop(OpenFileDialog {
            cancelled: cancelled.clone(),
            result,
        });
        assert!(cancelled.load(Ordering::Acquire));
        assert!(sender.is_closed());
    }
    #[test]
    fn save_dialog_cancels_only_an_unfinished_operation() {
        for finished in [false, true] {
            let cancelled = Arc::new(AtomicBool::new(false));
            let (sender, result) = async_channel::bounded(1);
            drop(SaveFileDialog {
                cancelled: cancelled.clone(),
                finished: AtomicBool::new(finished),
                result,
            });
            assert_eq!(cancelled.load(Ordering::Acquire), !finished);
            assert!(sender.is_closed());
        }
    }
}
