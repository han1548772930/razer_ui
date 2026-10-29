use super::*;
use crate::device_changes::{DeviceChange, EventQueue, ExactDeviceChange};
use async_channel::Receiver;
use std::sync::mpsc;

fn coalesced_publisher() -> (EventPublisher, Receiver<DeviceChange>) {
    let (events, changes) = async_channel::bounded(1);
    let (stop, _) = mpsc::sync_channel(1);
    (
        EventPublisher::new(EventQueue::Coalesced(events), stop),
        changes,
    )
}

fn exact_publisher(
    capacity: usize,
) -> (
    EventPublisher,
    Receiver<ExactDeviceChange>,
    mpsc::Receiver<()>,
) {
    let (events, changes) = EventQueue::exact(capacity);
    let (stop, stopped) = mpsc::sync_channel(1);
    (EventPublisher::new(events, stop), changes, stopped)
}

fn simulated_notifications(events: EventPublisher) -> Notifications {
    Notifications {
        handles: vec![1usize as HCMNOTIFICATION, 2usize as HCMNOTIFICATION],
        context: Some(Box::new(events)),
    }
}

fn registration_error(result: Result<Notifications, String>) -> String {
    match result {
        Err(error) => error,
        Ok(mut notifications) => {
            // Fake registrations must never reach a real Windows unregister,
            // including a regression that unexpectedly accepts registration.
            notifications.release_with(|_| CR_SUCCESS).unwrap();
            panic!("simulated registration should have failed");
        }
    }
}

#[test]
fn unregisters_every_handle_before_releasing_the_callback_context() {
    let (events, changes) = coalesced_publisher();
    let mut notifications = simulated_notifications(events);
    let mut handles = Vec::new();
    let mut retained_during_unregister = true;

    notifications
        .release_with(|handle| {
            retained_during_unregister &= changes.sender_count() == 1;
            handles.push(handle as usize);
            CR_SUCCESS
        })
        .unwrap();

    assert_eq!(handles, [1, 2]);
    assert!(retained_during_unregister);
    assert!(notifications.handles.is_empty());
    assert!(notifications.context.is_none());
    assert_eq!(changes.sender_count(), 0);
    assert!(changes.is_closed());
}

#[test]
fn failed_unregistration_retains_failed_handles_and_context_until_retry_succeeds() {
    let (events, changes) = coalesced_publisher();
    let mut notifications = simulated_notifications(events);
    let mut handles = Vec::new();

    let error = notifications
        .release_with(|handle| {
            handles.push(handle as usize);
            if handle as usize == 1 {
                CR_ACCESS_DENIED
            } else {
                CR_FAILURE
            }
        })
        .unwrap_err();

    let retained_handles = notifications
        .handles
        .iter()
        .map(|handle| *handle as usize)
        .collect::<Vec<_>>();
    let retained_context = notifications.context.is_some();
    let sender_count = changes.sender_count();
    let still_open = !changes.is_closed();
    notifications
        .release_with(|handle| {
            handles.push(handle as usize);
            CR_SUCCESS
        })
        .unwrap();

    assert_eq!(handles, [1, 2, 1, 2]);
    assert_eq!(retained_handles, [1, 2]);
    assert!(retained_context);
    assert_eq!(sender_count, 1);
    assert!(still_open);
    assert!(error.contains("CM_Unregister_Notification[0]"));
    assert!(error.contains("CONFIGRET 0x33"));
    assert!(error.contains("CM_Unregister_Notification[1]"));
    assert!(error.contains("CONFIGRET 0x13"));
    assert!(error.contains("失败句柄和回调上下文仍由监听线程持有"));
    assert!(error.contains("完全释放尚未确认"));
    assert!(notifications.handles.is_empty());
    assert!(notifications.context.is_none());
    assert_eq!(changes.sender_count(), 0);
    assert!(changes.is_closed());
}

#[test]
fn partial_registration_failure_rolls_back_the_owned_registration() {
    let (events, changes) = coalesced_publisher();
    let mut registrations = 0;
    let mut valid_filters = true;
    let mut removed = Vec::new();
    let result = Notifications::register_with(
        events,
        |filter, context, handle| {
            // Defer assertions until fake registrations have been rolled back;
            // unwinding here must not send a fake handle to the real OS Drop.
            valid_filters &= !context.is_null()
                && filter.cbSize as usize == size_of::<CM_NOTIFY_FILTER>()
                && filter.FilterType == CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE;
            registrations += 1;
            if registrations == 1 {
                *handle = 1usize as HCMNOTIFICATION;
                CR_SUCCESS
            } else {
                CR_INVALID_DATA
            }
        },
        |handle| {
            removed.push(handle as usize);
            CR_SUCCESS
        },
        || panic!("successful rollback must not require a retry"),
    );
    let error = registration_error(result);

    assert!(valid_filters);
    assert_eq!(registrations, 2);
    assert_eq!(removed, [1]);
    assert!(error.contains("注册失败（CONFIGRET 0x1f）"));
    assert!(!error.contains("完全释放尚未确认"));
    assert!(changes.is_closed());
    assert_eq!(changes.sender_count(), 0);
}

#[test]
fn failed_registration_retries_rollback_before_releasing_the_context() {
    let (events, changes) = coalesced_publisher();
    let mut registrations = 0;
    let mut removed = Vec::new();
    let mut retry_waits = 0;
    let mut retained_during_retry = false;
    let result = Notifications::register_with(
        events,
        |_, _, handle| {
            registrations += 1;
            if registrations == 1 {
                *handle = 1usize as HCMNOTIFICATION;
                CR_SUCCESS
            } else {
                CR_INVALID_DATA
            }
        },
        |handle| {
            removed.push(handle as usize);
            if removed.len() == 1 {
                CR_ACCESS_DENIED
            } else {
                CR_SUCCESS
            }
        },
        || {
            retry_waits += 1;
            retained_during_retry = changes.sender_count() == 1;
        },
    );
    let error = registration_error(result);

    assert_eq!(registrations, 2);
    assert_eq!(removed, [1, 1]);
    assert_eq!(retry_waits, 1);
    assert!(retained_during_retry);
    assert!(error.contains("注册失败（CONFIGRET 0x1f）"));
    assert!(error.contains("CM_Unregister_Notification[0]"));
    assert!(error.contains("CONFIGRET 0x33"));
    assert!(error.contains("失败句柄和回调上下文仍由监听线程持有"));
    assert!(error.contains("全部通知句柄及回调上下文现已释放"));
    assert_eq!(changes.sender_count(), 0);
    assert!(changes.is_closed());
}

#[test]
fn cleanup_retries_only_failed_registrations_and_waits_before_terminal_release() {
    let (events, changes) = coalesced_publisher();
    let mut notifications = simulated_notifications(events);
    let mut handles = Vec::new();
    let mut handle_two_attempts = 0;
    let mut waits = 0;
    let mut context_retained_each_round = true;
    let error = notifications
        .release_until_complete_with(
            |handle| {
                handles.push(handle as usize);
                if handle as usize == 2 {
                    handle_two_attempts += 1;
                    if handle_two_attempts < 3 {
                        return CR_ACCESS_DENIED;
                    }
                }
                CR_SUCCESS
            },
            || {
                waits += 1;
                context_retained_each_round &= changes.sender_count() == 1;
            },
        )
        .unwrap_err();
    assert_eq!(handles, [1, 2, 2, 2]);
    assert_eq!(waits, 2);
    assert!(context_retained_each_round);
    assert!(notifications.handles.is_empty());
    assert!(notifications.context.is_none());
    assert!(error.contains("CONFIGRET 0x33"));
    assert!(error.contains("经过 2 轮后台重试"));
    assert!(error.contains("全部通知句柄及回调上下文现已释放"));
    assert_eq!(changes.sender_count(), 0);
    assert!(changes.is_closed());
}

fn event_bytes(path_utf16: &[u16]) -> Vec<u8> {
    let mut bytes = vec![0; INTERFACE_PATH_OFFSET];
    let offset = offset_of!(CM_NOTIFY_EVENT_DATA, FilterType);
    bytes[offset..offset + size_of::<CM_NOTIFY_FILTER_TYPE>()]
        .copy_from_slice(&CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE.to_ne_bytes());
    for unit in path_utf16 {
        bytes.extend_from_slice(&unit.to_ne_bytes());
    }
    bytes
}

fn path_utf16() -> Vec<u16> {
    r"\\?\HID#VID_1532&PID_0098#same".encode_utf16().collect()
}

fn emit_callback(publisher: &EventPublisher, action: CM_NOTIFY_ACTION, bytes: &[u8]) {
    // Synthetic callback bytes stay readable for the entire direct call;
    // these tests never register or wait for a real Windows notification.
    unsafe {
        on_change(
            ptr::null_mut(),
            (publisher as *const EventPublisher).cast(),
            action,
            bytes.as_ptr().cast(),
            bytes.len() as u32,
        );
    }
}

#[test]
fn callback_copies_original_paths_and_preserves_remove_then_reinsert_actions() {
    let (publisher, changes, stopped) = exact_publisher(3);
    publisher.ready();
    let original = path_utf16();
    let mut terminated = original.clone();
    terminated.push(0);
    let mut bytes = event_bytes(&terminated);
    emit_callback(&publisher, CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL, &bytes);
    emit_callback(&publisher, CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, &bytes);
    bytes.fill(0);

    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    for (sequence, action) in [
        (2, DeviceInterfaceAction::Removal),
        (3, DeviceInterfaceAction::Arrival),
    ] {
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Interface {
                sequence,
                action,
                path_utf16: original.clone(),
            }
        );
    }
    assert_eq!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert!(changes.is_empty());
    assert!(!changes.is_closed());
}

#[test]
fn callback_during_registration_is_preserved_before_ready() {
    let (publisher, changes, _stopped) = exact_publisher(3);
    let mut path = path_utf16();
    path.push(0);
    let bytes = event_bytes(&path);
    let mut registrations = 0;
    let mut notifications = Notifications::register_with(
        publisher.clone(),
        |_, context, handle| {
            registrations += 1;
            *handle = registrations as HCMNOTIFICATION;
            // Simulate callbacks before each registration call returns.
            unsafe {
                on_change(
                    *handle,
                    context,
                    CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                );
            }
            CR_SUCCESS
        },
        |_| CR_SUCCESS,
        || panic!("successful registration must not roll back"),
    )
    .unwrap_or_else(|error| panic!("simulated registration: {error}"));
    publisher.ready();
    // Release all fake registrations before any assertion can unwind into
    // Notifications::drop; no fake handle may reach the real OS unregister.
    notifications.release_with(|_| CR_SUCCESS).unwrap();

    assert_eq!(registrations, 2);
    for sequence in 1..=2 {
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Interface {
                sequence,
                action: DeviceInterfaceAction::Arrival,
                path_utf16: path_utf16(),
            }
        );
    }
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 3 }
    );
    assert!(changes.is_empty());
}

#[test]
fn raw_utf16_identity_is_not_lossily_decoded_or_read_after_its_terminator() {
    let bytes = event_bytes(&[b'x' as u16, 0xd800, 0, 0x1234]);
    assert_eq!(interface_path(&bytes).unwrap(), [b'x' as u16, 0xd800]);
}

#[test]
fn malformed_callback_payload_loses_observation_and_cannot_resume() {
    let mut wrong_filter = event_bytes(&[b'x' as u16, 0]);
    let offset = offset_of!(CM_NOTIFY_EVENT_DATA, FilterType);
    wrong_filter[offset..offset + size_of::<CM_NOTIFY_FILTER_TYPE>()]
        .copy_from_slice(&CM_NOTIFY_FILTER_TYPE_DEVICEHANDLE.to_ne_bytes());
    let mut odd = event_bytes(&[b'x' as u16, 0]);
    odd.push(0);
    for (bytes, error) in [
        (
            vec![0; INTERFACE_PATH_OFFSET],
            DeviceNotificationError::DataTooShort,
        ),
        (wrong_filter, DeviceNotificationError::UnexpectedFilterType),
        (odd, DeviceNotificationError::OddPathByteCount),
        (
            event_bytes(&[b'x' as u16]),
            DeviceNotificationError::MissingPathTerminator,
        ),
        (event_bytes(&[0]), DeviceNotificationError::EmptyPath),
    ] {
        let (publisher, changes, stopped) = exact_publisher(1);
        publisher.ready();
        emit_callback(&publisher, CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, &bytes);
        publisher.ready();
        emit_callback(
            &publisher,
            CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL,
            &event_bytes(&[b'y' as u16, 0]),
        );
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Ready { sequence: 1 }
        );
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Lost {
                after_sequence: 1,
                reason: DeviceChangeLoss::InvalidNotification(error),
            }
        );
        assert_eq!(stopped.try_recv(), Ok(()));
        assert!(changes.is_closed());
        assert!(changes.try_recv().is_err());
    }
}

#[test]
fn callback_rejects_null_and_out_of_bounds_extents_before_reading_memory() {
    let data = CM_NOTIFY_EVENT_DATA::default();
    for (pointer, length, error) in [
        (ptr::null(), 0, DeviceNotificationError::MissingData),
        (
            &data as *const CM_NOTIFY_EVENT_DATA,
            (INTERFACE_PATH_OFFSET + 1) as u32,
            DeviceNotificationError::DataTooShort,
        ),
        (
            &data as *const CM_NOTIFY_EVENT_DATA,
            u32::MAX,
            DeviceNotificationError::DataTooLong,
        ),
    ] {
        let (publisher, changes, stopped) = exact_publisher(1);
        // Invalid extents are rejected before constructing a readable slice;
        // the oversized claim deliberately exceeds this small backing object.
        unsafe {
            on_change(
                ptr::null_mut(),
                (&publisher as *const EventPublisher).cast(),
                CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL,
                pointer,
                length,
            );
        }
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Lost {
                after_sequence: 0,
                reason: DeviceChangeLoss::InvalidNotification(error),
            }
        );
        assert_eq!(stopped.try_recv(), Ok(()));
        assert!(changes.is_closed());
    }
}

#[test]
fn callback_overflow_does_not_replace_an_interface_event() {
    let (publisher, changes, stopped) = exact_publisher(1);
    let bytes = event_bytes(&[b'x' as u16, 0]);
    emit_callback(&publisher, CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL, &bytes);
    emit_callback(&publisher, CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, &bytes);
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface {
            sequence: 1,
            action: DeviceInterfaceAction::Removal,
            path_utf16: vec![b'x' as u16],
        }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: 1,
            reason: DeviceChangeLoss::QueueOverflow,
        }
    );
    assert_eq!(stopped.try_recv(), Ok(()));
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn unrelated_actions_do_not_read_an_interface_payload_or_invalidate_observation() {
    let (publisher, changes, stopped) = exact_publisher(1);
    unsafe {
        on_change(
            ptr::null_mut(),
            (&publisher as *const EventPublisher).cast(),
            CM_NOTIFY_ACTION_DEVICEQUERYREMOVE,
            ptr::null(),
            u32::MAX,
        );
    }
    assert!(changes.is_empty());
    assert!(!changes.is_closed());
    assert_eq!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty));
}
