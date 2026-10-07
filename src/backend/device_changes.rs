//! Observe Windows USB/HID topology without loading the vendor Node addons.
//! The current host starts USB add/remove monitoring in UsbRzDeviceAction.initUSB.
//! Events only invalidate discovery; they never supply a device or a profile.
use async_channel::{Receiver, Sender};
use std::sync::mpsc;

pub(crate) enum DeviceChange {
    Ready,
    Changed,
    Failed(String),
}

/// Dropping the owner only signals the thread. Native unregistration may wait
/// for callbacks and therefore must never run in a view's destructor.
pub(crate) struct DeviceChangeMonitor {
    _stop: mpsc::Sender<()>,
}

impl DeviceChangeMonitor {
    pub(crate) fn start() -> Result<(Self, Receiver<DeviceChange>), String> {
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
mod native {
    use super::*;
    use std::{ffi::c_void, mem::size_of, ptr};
    use windows_sys::{Win32::Devices::DeviceAndDriverInstallation::*, core::GUID};

    // USB_DEVICE matches the current detection.node's statically verified GUID.
    // HID supplements USB for collections appearing after the physical device.
    const CLASSES: [GUID; 2] = [
        GUID::from_u128(0xa5dcbf10_6530_11d2_901f_00c04fb951ed),
        GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030),
    ];

    pub(super) struct Notifications {
        handles: Vec<HCMNOTIFICATION>,
        context: Option<Box<Sender<DeviceChange>>>,
    }

    impl Notifications {
        pub(super) fn register(events: Sender<DeviceChange>) -> Result<Self, String> {
            let mut notifications = Self {
                handles: Vec::new(),
                context: Some(Box::new(events)),
            };
            for class in CLASSES {
                let filter = CM_NOTIFY_FILTER {
                    cbSize: size_of::<CM_NOTIFY_FILTER>() as u32,
                    FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
                    u: CM_NOTIFY_FILTER_0 {
                        DeviceInterface: CM_NOTIFY_FILTER_0_0 { ClassGuid: class },
                    },
                    ..Default::default()
                };
                let mut handle = ptr::null_mut();
                // Box keeps the callback context at a stable address until all
                // registrations have been removed and in-flight callbacks end.
                let context = notifications.context.as_deref().unwrap();
                let result = unsafe {
                    CM_Register_Notification(
                        &filter,
                        (context as *const Sender<DeviceChange>).cast(),
                        Some(on_change),
                        &mut handle,
                    )
                };
                if result != CR_SUCCESS {
                    return Err(format!(
                        "设备插拔监听注册失败（CONFIGRET {result:#x}）；可手动刷新设备信息。"
                    ));
                }
                notifications.handles.push(handle);
            }
            Ok(notifications)
        }
    }

    unsafe extern "system" fn on_change(
        _: HCMNOTIFICATION,
        context: *const c_void,
        action: CM_NOTIFY_ACTION,
        _: *const CM_NOTIFY_EVENT_DATA,
        _: u32,
    ) -> u32 {
        if matches!(
            action,
            CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL | CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL
        ) && !context.is_null()
        {
            // A capacity-one channel coalesces bursts without blocking Windows
            // callbacks. Neither native device reads nor UI code run here.
            let sender = unsafe { &*context.cast::<Sender<DeviceChange>>() };
            let _ = sender.try_send(DeviceChange::Changed);
        }
        0
    }

    impl Drop for Notifications {
        fn drop(&mut self) {
            let mut failed = false;
            for handle in self.handles.drain(..) {
                // This destructor runs on the dedicated owner thread, never
                // inside on_change or on the UI thread.
                failed |= unsafe { CM_Unregister_Notification(handle) } != CR_SUCCESS;
            }
            if failed {
                // A failed unregister cannot prove callback quiescence. Retain
                // the tiny context rather than risk a callback use-after-free.
                if let Some(context) = self.context.take() {
                    let _ = Box::leak(context);
                }
            }
        }
    }
}

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
