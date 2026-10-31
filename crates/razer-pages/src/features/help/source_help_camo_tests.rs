use super::*;
use gpui_kit::{AppContext, TestAppContext};
use razer_device::{
    camera_extensions::{CameraExtensionTransport, CameraKsCallOutcome},
    camera_focus_family::CameraFamilyFocusTransport,
    camera_serial::CameraSerialState,
};
use razer_model::model::DeviceConnectionObservation;
use razer_platform::camera_apps::{CamoDriver, CamoStudio};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

fn device(product_id: u32) -> Device {
    let mut device = crate::test_devices::keyboard();
    device.product_id = product_id;
    device.real_product_id = product_id;
    device.serial_number = "LOCAL-SAVED-SERIAL".into();
    device.device_container_id = "{e528a5e9-5fb3-4e75-a64f-6bab5ad57124}".into();
    device.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
    device
}

fn page_offset(product_id: u32) -> usize {
    super::super::records()
        .iter()
        .find(|help| help.product_id == product_id)
        .unwrap()
        .pages[0]
        .offset
}

struct Wire(VecDeque<[u8; 8]>);
impl Wire {
    fn property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> anyhow::Result<CameraKsCallOutcome> {
        match u32::from_le_bytes(header[20..24].try_into().unwrap()) {
            0x1000_0002 => {}
            0x1000_0001 => *data = self.0.pop_front().expect("audited serial reply"),
            flags => panic!("unexpected camera property flags {flags:#x}"),
        }
        Ok(CameraKsCallOutcome {
            hresult: 0,
            bytes_returned: 8,
        })
    }
}
impl CameraExtensionTransport for Wire {
    fn begin_node(&mut self, node: u32) -> anyhow::Result<()> {
        assert_eq!(node, 3);
        Ok(())
    }
    fn end_node(&mut self) {}
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> anyhow::Result<CameraKsCallOutcome> {
        self.property(header, data)
    }
    fn wait_ms(&mut self, _: u32) -> anyhow::Result<()> {
        Ok(())
    }
}
impl CameraFamilyFocusTransport for Wire {
    fn begin_topology(&mut self) -> anyhow::Result<u32> {
        Ok(1)
    }
    fn end_topology(&mut self) {}
    fn begin_node(&mut self, node: u32) -> anyhow::Result<()> {
        assert_eq!(node, 0);
        Ok(())
    }
    fn end_node(&mut self) {}
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> anyhow::Result<CameraKsCallOutcome> {
        self.property(header, data)
    }
    fn wait_ms(&mut self, _: u32) -> anyhow::Result<()> {
        Ok(())
    }
}

fn serial(product_id: u32) -> CameraSerialProgress {
    // GenericCamera acknowledged chunks; the local document has a different value.
    let mut wire = Wire(VecDeque::from([
        [0, 0, 0, b'A', b'B', b'2', b'4', b'0'],
        [0, 0, 0, b'1', b'R', b'Z', b'0', b'1'],
        [0, 0, 0, b'1', b'2', b'3', b'4', b'5'],
        [0, 0, 0, 0, 0, 0, 0, 0],
    ]));
    let mut state = CameraSerialState::new(product_id).unwrap();
    match product_id {
        3594 => state.read_pro(&mut wire, &|| Ok(())),
        _ => state.read_family(&mut wire, &|| Ok(())),
    }
}

#[gpui_kit::test]
fn help_camo_mount_observes_installation_and_only_retry_requests_serial(cx: &mut TestAppContext) {
    for product_id in [3594, 3595] {
        let mut initial = device(product_id);
        assert!(initial.observe_camera_serial(CameraSerialObservation::Reading));
        let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(initial, cx)));
        let events = Rc::new(RefCell::new(Vec::new()));
        let captured = events.clone();
        let _subscription = cx.update(|cx| {
            cx.subscribe(&help, move |_, request: &HelpCamoRequested, _| {
                captured.borrow_mut().push(request.clone());
            })
        });
        cx.update(|cx| {
            help.update(cx, |help, cx| help.set_page(page_offset(product_id), cx));
            let view = help.read(cx);
            assert_eq!(view.camo.serial, "NOSERIALNUMBER");
            assert!(view.camo.invalid_serial);
            assert!(!view.camo.loading_serial);
            assert_eq!(view.camo.installation, HelpCamoInstallationStatus::Loading);
        });
        cx.run_until_parked();
        assert_eq!(events.borrow().len(), 1);
        assert_eq!(events.borrow()[0].command(), &HelpCamoCommand::Observe);
        cx.update(|cx| {
            help.update(cx, |help, cx| {
                help.retry_camo_serial(cx);
                help.retry_camo_serial(cx);
                assert!(help.camo.loading_serial);
            })
        });
        cx.run_until_parked();
        assert_eq!(events.borrow().len(), 2);
        let retry = events.borrow()[1].clone();
        assert_eq!(retry.command(), &HelpCamoCommand::GetSerialNumber);
        cx.update(|cx| {
            assert!(help.update(cx, |help, cx| help.observe_camo(
                HelpCamoObservation::serial(retry.clone(), Err("actual serial read failed".into())),
                cx,
            )));
            let view = help.read(cx);
            assert!(!view.camo.loading_serial);
            assert!(view.camo.invalid_serial);
            assert_eq!(
                view.camo.serial_error.as_deref(),
                Some("actual serial read failed")
            );
            assert_eq!(view.camo.serial, "NOSERIALNUMBER");
            help.update(cx, |help, cx| help.leave_page(cx));
            assert!(retry.is_cancelled());
            assert!(!help.update(cx, |help, cx| help.observe_camo(
                HelpCamoObservation::serial(retry, Ok(serial(product_id))),
                cx,
            )));
        });
    }
}

#[gpui_kit::test]
fn help_camo_serial_observation_preserves_the_physical_mount_and_never_uses_saved_serial(
    cx: &mut TestAppContext,
) {
    for product_id in [3594, 3595] {
        let mut current = device(product_id);
        let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(current.clone(), cx)));
        cx.update(|cx| {
            help.update(cx, |help, cx| help.set_page(page_offset(product_id), cx));
            let mount = help
                .read(cx)
                .camo_request(HelpCamoCommand::Observe)
                .unwrap();
            assert_eq!(help.read(cx).displayed_serial(), "NOSERIALNUMBER");
            assert!(
                current.observe_camera_serial(CameraSerialObservation::Read(serial(product_id)))
            );
            help.update(cx, |help, cx| help.set_device(&current, cx));
            assert!(!mount.is_cancelled());
            assert!(help.read(cx).accepts_camo_request(&mount));
            assert_eq!(help.read(cx).displayed_serial(), "AB2401RZ0112345");
            assert!(!help.read(cx).camo.invalid_serial);
            assert!(
                current
                    .observe_camera_serial(CameraSerialObservation::Failed("read failed".into()))
            );
            help.update(cx, |help, cx| help.set_device(&current, cx));
            assert_eq!(current.serial_number, "AB2401RZ0112345");
            assert_eq!(help.read(cx).displayed_serial(), "NOSERIALNUMBER");
            assert_eq!(
                help.read(cx).camo.serial_error.as_deref(),
                Some("read failed")
            );
            let request = help
                .read(cx)
                .camo_request(HelpCamoCommand::GetSerialNumber)
                .unwrap();
            let other_product = if product_id == 3594 { 3595 } else { 3594 };
            assert!(!help.update(cx, |help, cx| help.observe_camo(
                HelpCamoObservation::serial(request, Ok(serial(other_product))),
                cx,
            )));
            assert_eq!(help.read(cx).displayed_serial(), "NOSERIALNUMBER");
        });
    }
}

#[gpui_kit::test]
fn help_camo_installation_and_launch_preserve_independent_real_errors(cx: &mut TestAppContext) {
    let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(device(3594), cx)));
    cx.update(|cx| {
        help.update(cx, |help, cx| help.set_page(page_offset(3594), cx));
        let request = help
            .read(cx)
            .camo_request(HelpCamoCommand::Observe)
            .unwrap();
        let installed = CamoInstallation::from_observations(
            Ok(CamoStudio::Installed {
                version: Err("version query failed".into()),
            }),
            Ok(CamoDriver::Installed { enabled: Ok(true) }),
        );
        assert!(help.update(cx, |help, cx| help.observe_camo(
            HelpCamoObservation::installation(request.clone(), installed.clone()),
            cx,
        )));
        assert!(help.read(cx).camo.installed());
        assert_eq!(
            help.read(cx).camo.installation,
            HelpCamoInstallationStatus::Observed(installed)
        );
        assert!(!help.update(cx, |help, cx| help.observe_camo(
            HelpCamoObservation::serial(request.clone(), Ok(serial(3594))),
            cx,
        )));
        assert_eq!(help.read(cx).displayed_serial(), "NOSERIALNUMBER");
        assert!(help.update(cx, |help, cx| help.observe_camo(
            HelpCamoObservation::failed(request, "registry access denied".into()),
            cx,
        )));
        assert!(!help.read(cx).camo.installed());
        assert_eq!(
            help.read(cx).camo.installation,
            HelpCamoInstallationStatus::Failed("registry access denied".into())
        );
        let launch = help
            .read(cx)
            .camo_request(HelpCamoCommand::LaunchInstalled)
            .unwrap();
        assert!(help.update(cx, |help, cx| help.observe_camo(
            HelpCamoObservation::launched(launch, Err("package and shortcut launch failed".into())),
            cx,
        )));
        assert_eq!(
            help.read(cx).camo.launch,
            Some(Err("package and shortcut launch failed".into()))
        );
    });
}

#[gpui_kit::test]
fn help_camo_reconnection_variant_change_unmount_and_release_cancel_old_requests(
    cx: &mut TestAppContext,
) {
    for field in 0..6 {
        let initial = device(3594);
        let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(initial.clone(), cx)));
        let request = cx.update(|cx| {
            help.update(cx, |help, cx| help.set_page(page_offset(3594), cx));
            help.read(cx)
                .camo_request(HelpCamoCommand::Observe)
                .unwrap()
        });
        let mut replacement = initial;
        match field {
            0 => replacement.product_id = 3595,
            1 => replacement.real_product_id += 1,
            2 => replacement.device_container_id.push('x'),
            3 => replacement.edition_id += 1,
            4 => replacement.layout_id += 1,
            5 => replacement.observe_connection(None),
            _ => unreachable!(),
        }
        cx.update(|cx| {
            help.update(cx, |help, cx| help.set_device(&replacement, cx));
            assert!(request.is_cancelled(), "identity field {field}");
            assert!(!help.read(cx).accepts_camo_request(&request));
        });
    }
    let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(device(3594), cx)));
    let request = cx.update(|cx| {
        help.update(cx, |help, cx| help.set_page(page_offset(3594), cx));
        help.read(cx)
            .camo_request(HelpCamoCommand::GetSerialNumber)
            .unwrap()
    });
    drop(help);
    cx.run_until_parked();
    assert!(request.is_cancelled());
}

#[gpui_kit::test]
fn camera_without_camo_shows_only_the_current_connection_serial(cx: &mut TestAppContext) {
    let mut current = device(3596);
    let help = cx.update(|cx| cx.new(|cx| SourceHelp::new(current.clone(), cx)));
    cx.update(|cx| {
        help.update(cx, |help, cx| help.set_page(page_offset(3596), cx));
        assert!(
            help.read(cx)
                .camo_request(HelpCamoCommand::Observe)
                .is_none()
        );
        assert_eq!(help.read(cx).displayed_serial(), "");
        assert!(current.observe_camera_serial(CameraSerialObservation::Read(serial(3596))));
        help.update(cx, |help, cx| help.set_device(&current, cx));
        assert_eq!(help.read(cx).displayed_serial(), "AB2401RZ0112345");
        current.observe_connection(None);
        help.update(cx, |help, cx| help.set_device(&current, cx));
        assert_eq!(help.read(cx).displayed_serial(), "");
    });
}

#[::core::prelude::v1::test]
fn help_camo_validation_matches_source_utf16_slices_and_ascii_digit_rules() {
    assert!(valid_serial("AB2401RZ0112345"));
    assert!(valid_serial("😀2401RZ0112345"));
    for invalid in [
        "LOCAL-SAVED-SERIAL",
        "AB２４０１RZ0112345",
        "AB2401R\n0112345",
        "AB2401RZ011234X",
        "AB24011Z0112345",
    ] {
        assert!(!valid_serial(invalid), "{invalid:?}");
    }
}
