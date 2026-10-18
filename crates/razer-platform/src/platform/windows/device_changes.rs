//! Windows topology notification registrations and callback ownership.
use crate::device_changes::DeviceChange;
use async_channel::Sender;
use std::{ffi::c_void, mem::size_of, ptr};
use windows_sys::{Win32::Devices::DeviceAndDriverInstallation::*, core::GUID};

// USB_DEVICE matches the current detection.node's statically verified GUID.
// HID supplements USB for collections appearing after the physical device.
const CLASSES: [GUID; 2] = [
    GUID::from_u128(0xa5dcbf10_6530_11d2_901f_00c04fb951ed),
    GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030),
];

pub(crate) struct Notifications {
    handles: Vec<HCMNOTIFICATION>,
    context: Option<Box<Sender<DeviceChange>>>,
}

impl Notifications {
    pub(crate) fn register(events: Sender<DeviceChange>) -> Result<Self, String> {
        Self::register_with(
            events,
            |filter, context, handle| {
                // The boxed context remains valid until every registration is
                // removed and its in-flight callbacks have returned.
                unsafe { CM_Register_Notification(filter, context, Some(on_change), handle) }
            },
            unregister,
        )
    }

    fn register_with(
        events: Sender<DeviceChange>,
        mut register: impl FnMut(&CM_NOTIFY_FILTER, *const c_void, &mut HCMNOTIFICATION) -> CONFIGRET,
        mut unregister: impl FnMut(HCMNOTIFICATION) -> CONFIGRET,
    ) -> Result<Self, String> {
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
            let result = register(
                &filter,
                (context as *const Sender<DeviceChange>).cast(),
                &mut handle,
            );
            if result != CR_SUCCESS {
                let error =
                    format!("设备插拔监听注册失败（CONFIGRET {result:#x}）；可手动刷新设备信息。");
                return match notifications.release_with(&mut unregister) {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(format!("{error}；{cleanup}")),
                };
            }
            notifications.handles.push(handle);
        }
        Ok(notifications)
    }

    pub(crate) fn shutdown(mut self) -> Result<(), String> {
        self.release_with(unregister)
    }

    fn release_with(
        &mut self,
        mut unregister: impl FnMut(HCMNOTIFICATION) -> CONFIGRET,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        for (ix, handle) in self.handles.drain(..).enumerate() {
            let result = unregister(handle);
            if result != CR_SUCCESS {
                errors.push(format!(
                    "CM_Unregister_Notification[{ix}] 失败（CONFIGRET {result:#x}）"
                ));
            }
        }
        if errors.is_empty() {
            self.context.take();
            Ok(())
        } else {
            // Failed unregistration does not establish callback quiescence.
            // Its stable context must remain reachable until process teardown.
            if let Some(context) = self.context.take() {
                let _ = Box::leak(context);
            }
            Err(format!(
                "{}；回调上下文已保留，原生通知的完全释放尚未确认。",
                errors.join("；")
            ))
        }
    }
}

fn unregister(handle: HCMNOTIFICATION) -> CONFIGRET {
    // Only the dedicated observation worker owns registrations. This may wait
    // for on_change to return and must never run in that callback or on the UI.
    unsafe { CM_Unregister_Notification(handle) }
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
        let _ = self.release_with(unregister);
    }
}

#[cfg(test)]
#[path = "device_changes/tests.rs"]
mod tests;
