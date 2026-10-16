//! Local file selection with a retained cancellation owner. No file is executed.
use anyhow::Result;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

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
        std::thread::Builder::new()
            .name("local-file-dialog".into())
            .spawn(move || {
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
}
