//! Windows topology notification registrations and callback ownership.
use crate::device_changes::{
    DeviceChangeLoss, DeviceInterfaceAction, DeviceNotificationError, EventPublisher,
};
use std::{
    ffi::c_void,
    mem::{offset_of, size_of},
    ptr, slice,
};
use windows_sys::{Win32::Devices::DeviceAndDriverInstallation::*, core::GUID};

// USB_DEVICE matches the current detection.node's statically verified GUID.
// HID supplements USB for collections appearing after the physical device.
const CLASSES: [GUID; 2] = [
    GUID::from_u128(0xa5dcbf10_6530_11d2_901f_00c04fb951ed),
    GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030),
];

pub(crate) struct Notifications {
    handles: Vec<HCMNOTIFICATION>,
    context: Option<Box<EventPublisher>>,
}

impl Notifications {
    pub(crate) fn register(events: EventPublisher) -> Result<Self, String> {
        Self::register_with(
            events,
            |filter, context, handle| {
                // The boxed context remains valid until every registration is
                // removed and its in-flight callbacks have returned.
                unsafe { CM_Register_Notification(filter, context, Some(on_change), handle) }
            },
            unregister,
            wait_for_cleanup_retry,
        )
    }

    fn register_with(
        events: EventPublisher,
        mut register: impl FnMut(&CM_NOTIFY_FILTER, *const c_void, &mut HCMNOTIFICATION) -> CONFIGRET,
        mut unregister: impl FnMut(HCMNOTIFICATION) -> CONFIGRET,
        mut retry_wait: impl FnMut(),
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
                (context as *const EventPublisher).cast(),
                &mut handle,
            );
            if result != CR_SUCCESS {
                let error =
                    format!("设备插拔监听注册失败（CONFIGRET {result:#x}）；可手动刷新设备信息。");
                notifications.fail_delivery(&error);
                return match notifications
                    .release_until_complete_with(&mut unregister, &mut retry_wait)
                {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(format!("{error}；{cleanup}")),
                };
            }
            notifications.handles.push(handle);
        }
        Ok(notifications)
    }

    pub(crate) fn shutdown(mut self) -> Result<(), String> {
        self.release_until_complete_with(unregister, wait_for_cleanup_retry)
    }

    fn release_with(
        &mut self,
        mut unregister: impl FnMut(HCMNOTIFICATION) -> CONFIGRET,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        for (ix, handle) in std::mem::take(&mut self.handles).into_iter().enumerate() {
            let result = unregister(handle);
            if result != CR_SUCCESS {
                self.handles.push(handle);
                errors.push(format!(
                    "CM_Unregister_Notification[{ix}] 失败（CONFIGRET {result:#x}）"
                ));
            }
        }
        if errors.is_empty() {
            self.context.take();
            Ok(())
        } else {
            Err(format!(
                "{}；失败句柄和回调上下文仍由监听线程持有，原生通知的完全释放尚未确认。",
                errors.join("；")
            ))
        }
    }

    fn fail_delivery(&self, error: &str) {
        if let Some(events) = self.context.as_deref() {
            events.failed(error.to_owned());
            events.close();
        }
    }

    fn release_until_complete_with(
        &mut self,
        mut unregister: impl FnMut(HCMNOTIFICATION) -> CONFIGRET,
        mut retry_wait: impl FnMut(),
    ) -> Result<(), String> {
        let mut first_failure = None;
        let mut failed_rounds = 0u64;
        loop {
            match self.release_with(&mut unregister) {
                Ok(()) => {
                    return match first_failure {
                        None => Ok(()),
                        Some(error) => Err(format!(
                            "{error}；经过 {failed_rounds} 轮后台重试，全部通知句柄及回调上下文现已释放。"
                        )),
                    };
                }
                Err(error) => {
                    if first_failure.is_none() {
                        self.fail_delivery(&error);
                        first_failure = Some(error);
                    }
                    failed_rounds = failed_rounds.saturating_add(1);
                    // The owning worker cannot terminate while a callback may
                    // still reference this context. Successful handles are
                    // removed once; only failed registrations are retried.
                    retry_wait();
                }
            }
        }
    }
}

fn wait_for_cleanup_retry() {
    std::thread::sleep(std::time::Duration::from_millis(50));
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
    event_data: *const CM_NOTIFY_EVENT_DATA,
    event_data_size: u32,
) -> u32 {
    let action = match action {
        CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL => DeviceInterfaceAction::Arrival,
        CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL => DeviceInterfaceAction::Removal,
        _ => return 0,
    };
    if context.is_null() {
        return 0;
    }
    // Registration owns this stable context until unregistration has waited
    // for every callback. Only bounded copies and queue operations run here.
    let events = unsafe { &*context.cast::<EventPublisher>() };
    let length = event_data_size as usize;
    let error = if event_data.is_null() {
        Some(DeviceNotificationError::MissingData)
    } else if length < INTERFACE_PATH_OFFSET + size_of::<u16>() {
        Some(DeviceNotificationError::DataTooShort)
    } else if length > MAX_INTERFACE_EVENT_BYTES {
        Some(DeviceNotificationError::DataTooLong)
    } else {
        None
    };
    let path = if let Some(error) = error {
        Err(error)
    } else {
        // CM supplies exactly event_data_size readable bytes during this
        // invocation. Do not dereference the full flexible-array structure or
        // read a WCHAR beyond that byte extent, including for its terminator.
        interface_path(unsafe { slice::from_raw_parts(event_data.cast::<u8>(), length) })
    };
    match path {
        Ok(path_utf16) => events.interface(action, path_utf16),
        Err(error) => events.lost(DeviceChangeLoss::InvalidNotification(error)),
    }
    0
}

const INTERFACE_PATH_OFFSET: usize =
    offset_of!(CM_NOTIFY_EVENT_DATA, u) + offset_of!(CM_NOTIFY_EVENT_DATA_0_0, SymbolicLink);
// Bound the callback's copy to the Windows Unicode path extent. A larger or
// malformed notification invalidates observation instead of truncating an ID.
const MAX_INTERFACE_PATH_UNITS: usize = 32_767;
const MAX_INTERFACE_EVENT_BYTES: usize =
    INTERFACE_PATH_OFFSET + (MAX_INTERFACE_PATH_UNITS + 1) * size_of::<u16>();

fn interface_path(data: &[u8]) -> Result<Vec<u16>, DeviceNotificationError> {
    if data.len() < INTERFACE_PATH_OFFSET + size_of::<u16>() {
        return Err(DeviceNotificationError::DataTooShort);
    }
    if data.len() > MAX_INTERFACE_EVENT_BYTES {
        return Err(DeviceNotificationError::DataTooLong);
    }
    let filter_offset = offset_of!(CM_NOTIFY_EVENT_DATA, FilterType);
    let filter = i32::from_ne_bytes(
        data[filter_offset..filter_offset + size_of::<CM_NOTIFY_FILTER_TYPE>()]
            .try_into()
            .expect("the interface path extent includes FilterType"),
    );
    if filter != CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE {
        return Err(DeviceNotificationError::UnexpectedFilterType);
    }
    let path_bytes = &data[INTERFACE_PATH_OFFSET..];
    if !path_bytes.len().is_multiple_of(size_of::<u16>()) {
        return Err(DeviceNotificationError::OddPathByteCount);
    }
    let mut path = Vec::new();
    for pair in path_bytes.chunks_exact(size_of::<u16>()) {
        let unit = u16::from_ne_bytes([pair[0], pair[1]]);
        if unit == 0 {
            return if path.is_empty() {
                Err(DeviceNotificationError::EmptyPath)
            } else {
                Ok(path)
            };
        }
        path.push(unit);
    }
    Err(DeviceNotificationError::MissingPathTerminator)
}

impl Drop for Notifications {
    fn drop(&mut self) {
        // Notifications never leaves its dedicated observation worker. Even
        // unwind must finish unregistering before the stable context is freed.
        let _ = self.release_until_complete_with(unregister, wait_for_cleanup_retry);
    }
}

#[cfg(test)]
#[path = "device_changes/tests.rs"]
mod tests;
