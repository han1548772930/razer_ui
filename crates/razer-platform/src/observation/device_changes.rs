//! Observe Windows USB/HID topology without loading the vendor Node addons.
//! The current host starts USB add/remove monitoring in UsbRzDeviceAction.initUSB.
//! Events only invalidate discovery; they never supply a device or a profile.
use async_channel::Receiver;
#[cfg(not(windows))]
use async_channel::Sender;
use std::sync::mpsc;

pub enum DeviceChange {
    Ready,
    Changed,
    Failed(String),
}

/// Dropping the owner only signals the thread. Native unregistration may wait
/// for callbacks and therefore must never run in a view's destructor.
pub struct DeviceChangeMonitor {
    _stop: mpsc::Sender<()>,
}

impl DeviceChangeMonitor {
    pub fn start() -> Result<(Self, Receiver<DeviceChange>), String> {
        let (stop, stopped) = mpsc::channel::<()>();
        let (events, changes) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("razer-device-changes".into())
            .spawn(
                move || match native::Notifications::register(events.clone()) {
                    Ok(_notifications) => {
                        // Registration precedes Ready. The consumer rescans after
                        // Ready too, closing the initial enumerate/subscribe race.
                        let _ = events.send_blocking(DeviceChange::Ready);
                        let _ = stopped.recv();
                    }
                    Err(error) => {
                        let _ = events.send_blocking(DeviceChange::Failed(error));
                    }
                },
            )
            .map_err(|error| format!("无法创建设备变化监听线程：{error}"))?;
        Ok((Self { _stop: stop }, changes))
    }
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
    }
}
