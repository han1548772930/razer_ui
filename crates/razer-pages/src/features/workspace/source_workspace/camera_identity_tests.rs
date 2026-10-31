use super::*;
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};
use std::{cell::RefCell, rc::Rc};

fn camera(product_id: u32) -> Device {
    let mut device = crate::test_devices::keyboard();
    device.product_id = product_id;
    device.real_product_id = product_id;
    device.serial_number = "LOCAL-SAVED-SERIAL".into();
    device.device_container_id = "{e528a5e9-5fb3-4e75-a64f-6bab5ad57124}".into();
    device.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
    device
}

#[::core::prelude::v1::test]
fn camera_connection_retains_one_identity_request_when_its_observed_serial_changes() {
    for product_id in 3594..=3596 {
        let mut owner = Owner::default();
        let mut device = camera(product_id);
        let request = owner
            .ensure_connection(EntityId::from(7), 11, &device, false)
            .unwrap();
        assert!(!owner.attempted());
        assert!(
            owner
                .ensure_connection(EntityId::from(7), 11, &device, false)
                .is_none()
        );
        assert!(owner.complete(&request));
        assert!(owner.attempted());
        device.serial_number = "AB2401RZ0112345".into();
        device.edition_id = 2;
        device.layout_id = 1;
        assert!(
            owner
                .ensure_connection(EntityId::from(7), 11, &device, false)
                .is_none()
        );
        assert!(!request.is_cancelled());
        assert!(owner.request.as_ref().unwrap().same_connection(&request));
        assert!(owner.attempted());
    }
}

#[::core::prelude::v1::test]
fn camera_reconnection_owner_and_endpoint_changes_reject_previous_completion() {
    for change in 0..5 {
        let mut owner = Owner::default();
        let mut device = camera(3594);
        let request = owner
            .ensure_connection(EntityId::from(7), 11, &device, false)
            .unwrap();
        let (mut entity, mut generation) = (EntityId::from(7), 11);
        match change {
            0 => entity = EntityId::from(8),
            1 => generation += 1,
            2 => device.product_id = 3595,
            3 => device.real_product_id = 3595,
            4 => device.device_container_id = "{305d3c59-17ca-4c58-9316-bc2df5abc36a}".into(),
            _ => unreachable!(),
        }
        let next = owner
            .ensure_connection(entity, generation, &device, false)
            .unwrap();
        assert!(request.is_cancelled());
        assert!(!next.same_connection(&request));
        assert!(!owner.complete(&request));
        assert!(!owner.attempted());
        assert!(owner.complete(&next));
        owner.invalidate();
        assert!(next.is_cancelled());
        assert!(!owner.attempted());
        let resumed = owner
            .ensure_connection(entity, generation, &device, false)
            .unwrap();
        assert!(!resumed.same_connection(&next));
        assert!(!owner.complete(&next));
    }
}

#[::core::prelude::v1::test]
fn camera_initialization_requires_a_physical_connection_and_closes_with_its_owner() {
    for disconnected in [None, Some(DeviceConnectionObservation::ReceiverPeer(1))] {
        let mut owner = Owner::default();
        let mut device = camera(3594);
        let request = owner
            .ensure_connection(EntityId::from(7), 0, &device, false)
            .unwrap();
        device.observe_connection(disconnected);
        assert!(
            owner
                .ensure_connection(EntityId::from(7), 1, &device, false)
                .is_none()
        );
        assert!(request.is_cancelled());
    }
    for serial in ["PREVIEW-camera", "DEMO-camera"] {
        let mut owner = Owner::default();
        let mut device = camera(3594);
        device.serial_number = serial.into();
        assert!(
            owner
                .ensure_connection(EntityId::from(7), 0, &device, false)
                .is_none()
        );
    }
    let mut owner = Owner::default();
    let device = camera(3594);
    let request = owner
        .ensure_connection(EntityId::from(7), 0, &device, false)
        .unwrap();
    assert!(
        owner
            .ensure_connection(EntityId::from(7), 0, &device, true)
            .is_none()
    );
    assert!(request.is_cancelled());
    let request = owner
        .ensure_connection(EntityId::from(7), 0, &device, false)
        .unwrap();
    drop(owner);
    assert!(request.is_cancelled());
}

#[gpui_kit::test]
fn camera_workspace_publishes_connection_read_failures_and_restarts_after_canceled_exit(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let mut device = razer_model::product_preview::registered_preview(3594).unwrap();
    device.serial_number = "LOCAL-SAVED-SERIAL".into();
    device.device_container_id = "{e528a5e9-5fb3-4e75-a64f-6bab5ad57124}".into();
    device.observe_connection(None);
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let workspace = workspace.unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let captured = events.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&workspace, move |_, event: &WorkspaceEvent, _| {
            if let WorkspaceEvent::CameraIdentityRequested(request) = event {
                captured.borrow_mut().push(request.clone());
            }
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        workspace.update(cx, |view, cx| {
            view.set_page_key("HELP", window, cx);
            assert!(
                view.current_page()
                    .is_some_and(|page| page.role() == ProductPageRole::Help)
            );
            view.observe_connection(Some(DeviceConnectionObservation::UsbPresent), cx);
            assert_eq!(
                view.device.dashboard.readonly_camera_serial,
                CameraSerialObservation::Reading
            );
        });
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 1);
    let first = events.borrow()[0].clone();
    cx.update_window(handle.into(), |_, window, cx| {
        workspace.update(cx, |view, cx| {
            assert!(view.observe_camera_identity(
                &first,
                Err("actual camera read failed".into()),
                cx
            ));
            assert!(view.camera_identity.attempted());
            assert_eq!(view.device.serial_number, "LOCAL-SAVED-SERIAL");
            assert_eq!(
                view.device.dashboard.readonly_camera_serial,
                CameraSerialObservation::Failed("actual camera read failed".into())
            );
            assert_eq!(
                view.saved.dashboard.readonly_camera_serial,
                view.device.dashboard.readonly_camera_serial
            );
            view.set_active(false, window, cx);
            assert!(view.accepts_camera_identity(&first));
            view.observe_connection(None, cx);
            assert!(first.is_cancelled());
            assert_eq!(
                view.device.dashboard.readonly_camera_serial,
                CameraSerialObservation::Unobserved
            );
            view.observe_connection(Some(DeviceConnectionObservation::UsbPresent), cx);
            assert!(!view.observe_camera_identity(&first, Err("late old failure".into()), cx));
            let current = view.camera_identity_request().unwrap().clone();
            assert_eq!(
                view.device.dashboard.readonly_camera_serial,
                CameraSerialObservation::Reading
            );
            view.prepare_exit(window, cx);
            assert!(current.is_cancelled());
            assert!(view.camera_identity_request().is_none());
            view.cancel_exit(false, window, cx);
            let resumed = view.camera_identity_request().unwrap();
            assert!(!resumed.same_connection(&current));
            assert!(!resumed.is_cancelled());
            assert!(!view.camera_identity.attempted());
        });
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 3);
}
