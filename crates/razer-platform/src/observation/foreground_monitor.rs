//! Current SysUtilsNative foreground event ownership and delivery.
//! Windows hooks are isolated; other platforms do not fake these events.
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundWindow {
    pub name: String,
    pub path: String,
}

impl ForegroundWindow {
    /// Current 0x3c5a0 uses Windows filename()/parent_path(), preserving case.
    pub fn from_executable(executable: &str) -> Self {
        let separator = executable.rfind(['\\', '/']);
        match separator {
            Some(index) => Self {
                name: executable[index + 1..].into(),
                path: executable[..if index == 2 && executable.as_bytes().get(1) == Some(&b':') {
                    index + 1
                } else {
                    index
                }]
                    .into(),
            },
            None => Self {
                name: executable.into(),
                path: String::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_formatter_keeps_windows_filename_parent_and_case() {
        assert_eq!(
            ForegroundWindow::from_executable("C:\\Apps\\Browser.EXE"),
            ForegroundWindow {
                name: "Browser.EXE".into(),
                path: "C:\\Apps".into()
            }
        );
        assert_eq!(
            ForegroundWindow::from_executable("C:\\Browser.EXE"),
            ForegroundWindow {
                name: "Browser.EXE".into(),
                path: "C:\\".into()
            }
        );
        assert_eq!(
            ForegroundWindow::from_executable("\\\\server\\share\\Browser.EXE"),
            ForegroundWindow {
                name: "Browser.EXE".into(),
                path: "\\\\server\\share".into()
            }
        );
    }
}

#[derive(Default)]
pub struct ForegroundMonitor {
    subscribers: BTreeSet<String>,
    #[cfg(windows)]
    listener: Option<crate::platform::windows::foreground_monitor::Listener>,
}

impl ForegroundMonitor {
    /// Host sysutil keeps unique sender URLs; repeated starts are idempotent.
    pub fn start(&mut self, view_url: &str) -> anyhow::Result<bool> {
        anyhow::ensure!(
            !view_url.is_empty(),
            "foreground subscription requires a view URL"
        );
        #[cfg(not(windows))]
        anyhow::bail!(
            "Current SysUtilsNative Windows foreground hooks are unavailable on this platform"
        );
        #[cfg(windows)]
        {
            if self.listener.is_none() {
                self.listener =
                    Some(crate::platform::windows::foreground_monitor::Listener::start()?);
            }
            self.subscribers.insert(view_url.into());
            if let Some(listener) = &mut self.listener {
                listener.set_subscribers(&self.subscribers)?;
            }
            Ok(true)
        }
    }

    pub fn stop(&mut self, view_url: &str) -> anyhow::Result<bool> {
        #[cfg(not(windows))]
        {
            let _ = view_url;
            anyhow::bail!(
                "Current SysUtilsNative Windows foreground hooks are unavailable on this platform"
            );
        }
        #[cfg(windows)]
        {
            self.subscribers.remove(view_url);
            if let Some(listener) = &mut self.listener {
                listener.set_subscribers(&self.subscribers)?;
            }
            if self.subscribers.is_empty() {
                if let Some(listener) = &mut self.listener {
                    listener.stop()?;
                }
                self.listener = None;
            }
            Ok(true)
        }
    }

    /// The host broadcasts only to subscribed, live views. Queue one event
    /// per registered URL so one consumer cannot drain another view's event.
    pub fn drain(&mut self, view_url: &str) -> anyhow::Result<Vec<ForegroundWindow>> {
        #[cfg(not(windows))]
        {
            let _ = view_url;
            anyhow::bail!(
                "Current SysUtilsNative Windows foreground hooks are unavailable on this platform"
            );
        }
        #[cfg(windows)]
        {
            if !self.subscribers.contains(view_url) {
                return Ok(Vec::new());
            }
            match &mut self.listener {
                Some(listener) => listener.drain(view_url, &self.subscribers),
                None => Ok(Vec::new()),
            }
        }
    }
}
