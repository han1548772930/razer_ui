use super::*;
use gpui_kit::{AppContext, TestAppContext};
use std::{cell::RefCell, rc::Rc};

fn device() -> Device {
    let mut device = crate::test_devices::keyboard();
    device.product_id = 3594;
    device.real_product_id = 3594;
    device.device_container_id = "mounted-container".into();
    device
}
fn page_offset() -> usize {
    super::super::records()
        .iter()
        .find(|help| help.product_id == 3594)
        .unwrap()
        .pages[0]
        .offset
}
fn observed(request: HelpRuntimeVersionsRequest) -> HelpRuntimeVersionsObservation {
    HelpRuntimeVersionsObservation::from_storage(
        request,
        Ok(Some(
            r#"[{"windowName":"https://host.example/products/3594/mw/","value":"producer.12.8"}]"#
                .into(),
        )),
        Ok(Some(r#"[{"name":"synapse","version":"7.15.9.2"}]"#.into())),
    )
}

#[gpui_kit::test]
fn help_runtime_mount_observes_once_and_rejects_unmounted_callbacks(cx: &mut TestAppContext) {
    let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(device(), cx)));
    let events = Rc::new(RefCell::new(Vec::new()));
    let captured = events.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&help, move |_, request: &HelpRuntimeVersionsRequest, _| {
            captured.borrow_mut().push(request.clone());
        })
    });
    cx.update(|cx| {
        assert!(help.read(cx).runtime_versions_request().is_none());
        assert_eq!(
            help.read(cx).runtime_versions_status().middleware,
            HelpRuntimeVersionStatus::Unobserved
        );
        help.update(cx, |help, cx| help.set_page(page_offset(), cx));
    });
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 1);
    let request = events.borrow()[0].clone();
    cx.update(|cx| {
        assert_eq!(
            help.read(cx).runtime_versions_status().middleware,
            HelpRuntimeVersionStatus::Loading
        );
        help.update(cx, |help, cx| help.set_page(page_offset(), cx));
        assert!(help.update(cx, |help, cx| {
            help.observe_runtime_versions(observed(request.clone()), cx)
        }));
        assert_eq!(
            help.read(cx).runtime_versions.rows().collect::<Vec<_>>(),
            [
                ("MW_VERSION", "producer.12.8"),
                ("SYNAPSE_VERSION", "4.15.9.2")
            ]
        );
        assert!(help.update(cx, |help, cx| help.observe_runtime_versions(
            HelpRuntimeVersionsObservation::failed(request.clone(), "storage read failed".into()),
            cx
        )));
        assert_eq!(help.read(cx).runtime_versions.rows().count(), 2);
        assert_eq!(
            help.read(cx).runtime_versions_status().middleware,
            HelpRuntimeVersionStatus::Failed("storage read failed".into())
        );
        help.update(cx, |help, cx| help.leave_page(cx));
        assert!(request.is_cancelled());
        assert!(help.read(cx).runtime_versions_request().is_none());
        assert_eq!(help.read(cx).runtime_versions.rows().count(), 0);
        assert!(!help.update(cx, |help, cx| {
            help.observe_runtime_versions(observed(request.clone()), cx)
        }));
        help.update(cx, |help, cx| help.set_page(page_offset(), cx));
        let current = help.read(cx).runtime_versions_request().unwrap();
        assert!(!current.same_mount(&request));
        assert!(!help.update(cx, |help, cx| {
            help.observe_runtime_versions(observed(request.clone()), cx)
        }));
        assert!(help.update(cx, |help, cx| {
            help.observe_runtime_versions(observed(current), cx)
        }));
    });
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 2);
}

#[gpui_kit::test]
fn help_runtime_device_identity_changes_cancel_each_old_scope(cx: &mut TestAppContext) {
    for field in 0..6 {
        let initial = device();
        let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(initial.clone(), cx)));
        let request = cx.update(|cx| {
            help.update(cx, |help, cx| help.set_page(page_offset(), cx));
            help.read(cx).runtime_versions_request().unwrap()
        });
        let mut replacement = initial;
        match field {
            0 => replacement.product_id = 3595,
            1 => replacement.real_product_id += 1,
            2 => replacement.serial_number.push_str("-new"),
            3 => replacement.device_container_id.push_str("-new"),
            4 => replacement.edition_id += 1,
            5 => replacement.layout_id += 1,
            _ => unreachable!(),
        }
        cx.update(|cx| {
            help.update(cx, |help, cx| help.set_device(&replacement, cx));
            assert!(request.is_cancelled(), "field {field}");
            assert!(!help.read(cx).accepts_runtime_versions(&request));
            assert!(!help.update(cx, |help, cx| {
                help.observe_runtime_versions(observed(request.clone()), cx)
            }));
        });
    }
}

#[gpui_kit::test]
fn help_runtime_release_cancels_pending_reads(cx: &mut TestAppContext) {
    let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(device(), cx)));
    let request = cx.update(|cx| {
        help.update(cx, |help, cx| help.set_page(page_offset(), cx));
        help.read(cx).runtime_versions_request().unwrap()
    });
    assert!(!request.is_cancelled());
    drop(help);
    cx.run_until_parked();
    assert!(request.is_cancelled());
}

#[gpui_kit::test]
fn help_runtime_reconnect_and_exit_resume_refresh_the_mounted_request(cx: &mut TestAppContext) {
    use crate::features::ProductWorkspace;
    use crate::nav::Tab;
    use gpui_kit::component::Root;
    use razer_model::model::DeviceConnectionObservation;
    cx.update(gpui_kit::init);
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| {
            ProductWorkspace::new(
                razer_model::product_preview::registered_preview(2684).unwrap(),
                false,
                window,
                cx,
            )
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let workspace = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        workspace.update(cx, |view, cx| {
            view.set_active(true, window, cx);
            view.set_page(Tab::Help, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    let first = cx.update(|cx| {
        workspace
            .read(cx)
            .help_runtime_versions_request(cx)
            .unwrap()
    });
    cx.update(|cx| {
        workspace.update(cx, |view, cx| {
            view.observe_connection(Some(DeviceConnectionObservation::HidPresent), cx)
        });
        assert!(first.is_cancelled());
        let connected = workspace
            .read(cx)
            .help_runtime_versions_request(cx)
            .unwrap();
        let revision = workspace.read(cx).connection_revision();
        workspace.update(cx, |view, cx| {
            view.observe_connection(Some(DeviceConnectionObservation::HidPresent), cx)
        });
        assert_eq!(workspace.read(cx).connection_revision(), revision);
        assert!(
            workspace
                .read(cx)
                .accepts_help_runtime_versions(&connected, cx)
        );
        workspace.update(cx, |view, cx| view.observe_connection(None, cx));
        assert!(connected.is_cancelled());
        let disconnected = workspace
            .read(cx)
            .help_runtime_versions_request(cx)
            .unwrap();
        assert!(
            !workspace
                .read(cx)
                .accepts_help_runtime_versions(&connected, cx)
        );
        workspace.update(cx, |view, cx| view.refresh_help_runtime_versions(cx));
        assert!(disconnected.is_cancelled());
        let resumed = workspace
            .read(cx)
            .help_runtime_versions_request(cx)
            .unwrap();
        assert!(!resumed.is_cancelled());
        assert!(!resumed.same_mount(&disconnected));
    });
}
