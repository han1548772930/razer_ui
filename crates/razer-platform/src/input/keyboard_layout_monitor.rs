//! SysUtilsNative calling-thread keyboard-layout timer and host URL routing.
//! The owner must be the message-pumped UI thread. A foreground-thread query
//! or a newly spawned timer thread changes the original HKL semantics.
use std::collections::BTreeSet;

#[derive(Default)]
pub struct KeyboardLayoutMonitor {
    subscribers: BTreeSet<String>,
    #[cfg(windows)]
    timer: Option<crate::platform::windows::keyboard_layout_monitor::Timer>,
}

impl KeyboardLayoutMonitor {
    pub fn start_on_message_thread(&mut self, view_url: &str) -> anyhow::Result<()> {
        #[cfg(not(windows))]
        anyhow::bail!(
            "SysUtilsNative calling-thread keyboard-layout timer is unavailable on this platform"
        );
        #[cfg(windows)]
        {
            if let Some(timer) = &self.timer {
                timer.check_owner()?;
            } else {
                self.timer =
                    Some(crate::platform::windows::keyboard_layout_monitor::Timer::start()?);
            }
            // Host still starts the native timer when sender/getURL is absent;
            // only a present URL joins the notification audience.
            if !view_url.is_empty() {
                self.subscribers.insert(view_url.into());
            }
            if let Some(timer) = &self.timer {
                timer.set_subscribers(&self.subscribers)?;
            }
            Ok(())
        }
    }

    /// Current host bug is observable: keyboard subscribers become the
    /// foreground URL list minus this URL, and foreground count gates stop.
    /// Do not silently replace that branch with a keyboard subscriber count.
    pub fn source_stop_on_message_thread(
        &mut self,
        view_url: &str,
        foreground_urls: &BTreeSet<String>,
    ) -> anyhow::Result<()> {
        #[cfg(not(windows))]
        {
            let _ = (view_url, foreground_urls);
            anyhow::bail!(
                "SysUtilsNative calling-thread keyboard-layout timer is unavailable on this platform"
            );
        }
        #[cfg(windows)]
        {
            if let Some(timer) = &self.timer {
                timer.check_owner()?;
            }
            if !view_url.is_empty() {
                self.subscribers = foreground_urls
                    .iter()
                    .filter(|url| url.as_str() != view_url)
                    .cloned()
                    .collect();
            }
            if let Some(timer) = &self.timer {
                timer.set_subscribers(&self.subscribers)?;
            }
            if foreground_urls.is_empty() {
                if let Some(mut timer) = self.timer.take() {
                    timer.stop()?;
                }
            }
            Ok(())
        }
    }

    pub fn drain_on_message_thread(&mut self, view_url: &str) -> anyhow::Result<Vec<i32>> {
        #[cfg(not(windows))]
        {
            let _ = view_url;
            anyhow::bail!(
                "SysUtilsNative calling-thread keyboard-layout timer is unavailable on this platform"
            );
        }
        #[cfg(windows)]
        {
            match &mut self.timer {
                Some(timer) => timer.drain(view_url, &self.subscribers),
                None => Ok(Vec::new()),
            }
        }
    }
}
