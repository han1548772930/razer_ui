//! Worker-owned Core Audio notification replacement; no vendor DLL loading.
//! OS callbacks only enqueue the original payload; never fabricate device data.
use razer_device::audio_notification::{
    AudioNotification, NotificationQueue, source_enable_response,
};
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub struct AudioNotifications {
    queue: Arc<Mutex<NotificationQueue>>,
    #[cfg(windows)]
    registration: Option<windows::Registration>,
}

impl Default for AudioNotifications {
    fn default() -> Self {
        Self {
            queue: Arc::new(Mutex::new(NotificationQueue::default())),
            #[cfg(windows)]
            registration: None,
        }
    }
}

impl AudioNotifications {
    pub fn enable(&mut self, enabled: bool) -> anyhow::Result<Value> {
        #[cfg(not(windows))]
        {
            let _ = enabled;
            anyhow::bail!("RzAudioUtil Core Audio notifications are unavailable on this platform")
        }
        #[cfg(windows)]
        {
            if enabled {
                // Preserve callback multiplicity, but register the OS listener
                // once. Source appends before registering, so callbacks that
                // arrive during registration already see the subscription.
                self.queue
                    .lock()
                    .map_err(|_| anyhow::anyhow!("audio notification queue poisoned"))?
                    .enabled()?;
                if self.registration.is_none() {
                    match windows::Registration::open(self.queue.clone()) {
                        Ok(registration) => self.registration = Some(registration),
                        Err(error) => {
                            let mut queue = self.queue.lock().map_err(|_| {
                                anyhow::anyhow!("audio notification queue poisoned")
                            })?;
                            queue.disabled();
                            queue.drain();
                            return Err(error);
                        }
                    }
                }
            } else {
                self.queue
                    .lock()
                    .map_err(|_| anyhow::anyhow!("audio notification queue poisoned"))?
                    .disabled();
                if let Some(registration) = &mut self.registration {
                    // Keep the owner alive on unregister failure so a later
                    // disable/drop can retry without a dangling callback.
                    registration.close()?;
                }
                self.registration = None;
            }
            Ok(source_enable_response(enabled))
        }
    }

    pub fn drain(&self) -> anyhow::Result<Vec<AudioNotification>> {
        #[cfg(not(windows))]
        anyhow::bail!("RzAudioUtil Core Audio notifications are unavailable on this platform");
        #[cfg(windows)]
        Ok(self
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("audio notification queue poisoned"))?
            .drain())
    }
}

#[cfg(windows)]
#[path = "platform/windows/audio_notification.rs"]
mod windows;
