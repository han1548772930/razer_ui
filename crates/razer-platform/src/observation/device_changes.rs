//! Observe Windows USB/HID topology without loading the vendor Node addons.
//! The current host starts USB add/remove monitoring in UsbRzDeviceAction.initUSB.
//! Events only invalidate discovery; they never supply a device or a profile.
use async_channel::{Receiver, Sender};
use std::{sync::mpsc, thread::JoinHandle};

pub enum DeviceChange {
    Ready,
    Changed,
    Failed(String),
}

/// Dropping the owner only signals the thread. Native unregistration may wait
/// for callbacks and therefore must never run in a view's destructor.
pub struct DeviceChangeMonitor {
    stop: Option<mpsc::Sender<()>>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl DeviceChangeMonitor {
    pub fn start() -> Result<(Self, Receiver<DeviceChange>), String> {
        let (events, changes) = async_channel::bounded(1);
        let monitor = Self::start_worker(events, |stopped, events| {
            if !matches!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty)) {
                return Ok(());
            }
            let notifications = native::Notifications::register(events.clone())?;
            wait_for_stop(stopped, &events);
            notifications.shutdown()
        })?;
        Ok((monitor, changes))
    }

    fn start_worker(
        events: Sender<DeviceChange>,
        run: impl FnOnce(mpsc::Receiver<()>, Sender<DeviceChange>) -> Result<(), String>
        + Send
        + 'static,
    ) -> Result<Self, String> {
        let (stop, stopped) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("razer-device-changes".into())
            .spawn(move || {
                let delivery = EventDelivery(events);
                let result = run(stopped, delivery.0.clone());
                if let Err(error) = &result {
                    // A terminal error replaces a coalesced topology change;
                    // cleanup never waits for the UI to drain its event queue.
                    let _ = delivery.0.force_send(DeviceChange::Failed(error.clone()));
                }
                result
            })
            .map_err(|error| format!("无法创建设备变化监听线程：{error}"))?;
        Ok(Self {
            stop: Some(stop),
            worker: Some(worker),
        })
    }

    /// Consume on a background executor and wait for native unregistration
    /// and callback-context release. Failed unregistration retains its context
    /// for safety and returns an error instead of claiming complete cleanup.
    pub fn shutdown(mut self) -> Result<(), String> {
        self.stop.take();
        self.worker
            .take()
            .expect("retained device-change worker")
            .join()
            .map_err(|_| "设备变化监听线程异常结束。".to_string())?
    }
}

impl Drop for DeviceChangeMonitor {
    fn drop(&mut self) {
        // Native unregistration can wait for callbacks. Only shutdown joins;
        // an entity destructor merely closes the dedicated stop channel.
        self.stop.take();
    }
}

struct EventDelivery(Sender<DeviceChange>);

impl Drop for EventDelivery {
    fn drop(&mut self) {
        // Even a retained callback context after failed unregistration cannot
        // keep delivery open after its observation worker exits or unwinds.
        self.0.close();
    }
}

fn wait_for_stop(stopped: mpsc::Receiver<()>, events: &Sender<DeviceChange>) {
    // Registration precedes Ready, closing the enumerate/subscribe race. A
    // pending Changed has the same rescan meaning and must not block stopping.
    let _ = events.force_send(DeviceChange::Ready);
    let _ = stopped.recv();
}

#[cfg(windows)]
use crate::platform::windows::device_changes as native;

#[cfg(not(windows))]
mod native {
    use super::*;
    pub(super) struct Notifications;
    impl Notifications {
        pub(super) fn register(_: Sender<DeviceChange>) -> Result<Self, String> {
            Err("此平台尚未接入设备插拔监听；可手动刷新设备信息。".into())
        }

        pub(super) fn shutdown(self) -> Result<(), String> {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "device_changes/tests.rs"]
mod tests;
