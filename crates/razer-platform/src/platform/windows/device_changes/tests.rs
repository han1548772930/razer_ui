use super::*;

fn simulated_notifications(events: Sender<DeviceChange>) -> Notifications {
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
    let (events, changes) = async_channel::bounded(1);
    let mut notifications = simulated_notifications(events);
    let mut handles = Vec::new();

    notifications
        .release_with(|handle| {
            assert_eq!(changes.sender_count(), 1);
            handles.push(handle as usize);
            CR_SUCCESS
        })
        .unwrap();

    assert_eq!(handles, [1, 2]);
    assert!(notifications.handles.is_empty());
    assert!(notifications.context.is_none());
    assert_eq!(changes.sender_count(), 0);
    assert!(changes.is_closed());
}

#[test]
fn failed_unregistration_retains_the_context_and_reports_every_native_error() {
    let (events, changes) = async_channel::bounded(1);
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

    assert_eq!(handles, [1, 2]);
    assert!(error.contains("CM_Unregister_Notification[0]"));
    assert!(error.contains("CONFIGRET 0x33"));
    assert!(error.contains("CM_Unregister_Notification[1]"));
    assert!(error.contains("CONFIGRET 0x13"));
    assert!(error.contains("回调上下文已保留"));
    assert!(error.contains("完全释放尚未确认"));
    assert!(notifications.handles.is_empty());
    assert!(notifications.context.is_none());
    assert_eq!(changes.sender_count(), 1);
    assert!(!changes.is_closed());
    // The owning observation worker closes delivery even when the native
    // callback's retained sender cannot safely be destroyed.
    changes.close();
}

#[test]
fn partial_registration_failure_rolls_back_the_owned_registration() {
    let (events, changes) = async_channel::bounded(1);
    let mut registrations = 0;
    let mut removed = Vec::new();
    let result = Notifications::register_with(
        events,
        |filter, context, handle| {
            assert!(!context.is_null());
            assert_eq!(filter.cbSize as usize, size_of::<CM_NOTIFY_FILTER>());
            assert_eq!(filter.FilterType, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE);
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
    );
    let error = registration_error(result);

    assert_eq!(registrations, 2);
    assert_eq!(removed, [1]);
    assert!(error.contains("注册失败（CONFIGRET 0x1f）"));
    assert!(!error.contains("完全释放尚未确认"));
    assert!(changes.is_closed());
    assert_eq!(changes.sender_count(), 0);
}

#[test]
fn failed_registration_preserves_rollback_failure_and_the_callback_context() {
    let (events, changes) = async_channel::bounded(1);
    let mut registrations = 0;
    let mut removed = Vec::new();
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
            CR_ACCESS_DENIED
        },
    );
    let error = registration_error(result);

    assert_eq!(registrations, 2);
    assert_eq!(removed, [1]);
    assert!(error.contains("注册失败（CONFIGRET 0x1f）"));
    assert!(error.contains("CM_Unregister_Notification[0]"));
    assert!(error.contains("CONFIGRET 0x33"));
    assert!(error.contains("回调上下文已保留"));
    assert_eq!(changes.sender_count(), 1);
    assert!(!changes.is_closed());
    changes.close();
}
