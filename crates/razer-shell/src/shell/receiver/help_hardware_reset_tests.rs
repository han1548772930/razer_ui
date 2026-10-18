use super::*;
use razer_device::{
    backend::HidNode,
    thx_blackshark_firmware::{
        BlackSharkFirmwareAttempt, BlackSharkFirmwareCommand, BlackSharkFirmwareCommandProgress,
        BlackSharkFirmwareIdentity, BlackSharkFirmwareReply,
    },
    thx_lifecycle_discovery::ThxConnectionIdentity,
    thx_submission::{ThxFirmwareProgress, ThxSubmissionPlan},
};
use serde_json::json;

const GUID: &str = "5a0573c1-576b-41ec-ad30-bf69894bc169";

fn local_device() -> Device {
    let mut device = razer_model::product_preview::registered_preview(1398).unwrap();
    device.source_device_settings = Some(json!({"version":10,"extension":{"gain":4}}));
    device.profiles = vec![razer_model::model::Profile {
        id: "previous".into(),
        guid: "previous".into(),
        name: "PC-Default".into(),
        source_settings: Some(json!({"guid":"previous","name":"PC-Default","unknown":[9,4]})),
        settings: None,
        dpi_stages: None,
    }];
    device.active_profile = "previous".into();
    device
}

#[::core::prelude::v1::test]
fn help_hardware_reset_candidate_preserves_profiles_and_extensions_without_mutating_its_owner() {
    let device = local_device();
    let before = serde_json::to_value(&device).unwrap();
    let reset = candidate(&device, "PC", GUID).unwrap();
    assert_eq!(serde_json::to_value(&device).unwrap(), before);
    assert_eq!(reset.profiles.len(), 2);
    assert_eq!(reset.profiles[0], device.profiles[0]);
    assert_eq!(reset.profiles[1].name, "PC-Default 1");
    assert_eq!(reset.active_profile, GUID);
    assert_eq!(
        reset.source_device_settings.as_ref().unwrap()["version"],
        11
    );
    assert_eq!(
        reset.source_device_settings.as_ref().unwrap()["extension"],
        json!({"gain":4})
    );
    assert_eq!(reset.serial_number, device.serial_number);
    assert_eq!(reset.device_container_id, device.device_container_id);
    let duplicate = candidate(&reset, "PC", GUID);
    assert!(duplicate.is_err());
}

#[::core::prelude::v1::test]
fn help_hardware_reset_conflict_fields_track_profile_edits_without_rejecting_power_observations() {
    let device = local_device();
    let mut observed = device.clone();
    observed.name.values.clear();
    observed.setup_status = razer_model::model::SetupStatus::Unknown;
    assert_eq!(reset_fields(&device), reset_fields(&observed));
    observed.profiles[0].name.push_str(" renamed");
    assert_ne!(reset_fields(&device), reset_fields(&observed));
}

fn receipt() -> (ThxSubmissionProgress, ThxConnectionIdentity) {
    let scope = ThxSubmissionScope {
        product_id: 1398,
        container_id: "observed-container".into(),
        serial_number: "observed-serial".into(),
        profile_id: GUID.into(),
        generation: 4,
    };
    let physical = ThxConnectionIdentity::new(
        1398,
        &scope.serial_number,
        &scope.container_id,
        1399,
        b"observed-hid".to_vec(),
    )
    .unwrap();
    let command = BlackSharkFirmwareCommandProgress {
        command: BlackSharkFirmwareCommand::FactoryReset1398,
        identity: BlackSharkFirmwareIdentity::new(
            1398,
            HidNode {
                path: physical.hid_path.clone(),
                vendor_id: 0x1532,
                product_id: 1399,
                usage_page: 0xff00,
                usage: 1,
                interface_number: 5,
                release_number: 0,
                serial_number: Some(scope.serial_number.clone()),
                manufacturer: None,
                product: None,
            },
            &scope.container_id,
            1,
        )
        .unwrap(),
        attempts: vec![BlackSharkFirmwareAttempt {
            output_attempted: true,
            output_completed: true,
            reply: Some(BlackSharkFirmwareReply {
                status: 2,
                resend_requested: false,
                header: vec![2, 2, 0x60, 0, 0, 0, 5, 0, 0, 0, 239, 1, 1],
                data: vec![0],
            }),
            ..Default::default()
        }],
        completed: true,
        failure: None,
    };
    (
        ThxSubmissionProgress {
            scope,
            plan: ThxSubmissionPlan::FactoryReset1398,
            render_setup: None,
            render_shutdown: None,
            roller_mapping: None,
            initial_observation: None,
            outcome: None,
            firmware: vec![ThxFirmwareProgress::Command(command)],
            error: None,
            cleanup_error: None,
        },
        physical,
    )
}

#[::core::prelude::v1::test]
fn help_hardware_reset_requires_the_actual_factory_receipt_from_the_retained_physical_owner() {
    let (confirmed, physical) = receipt();
    assert!(reset_error(&confirmed, &confirmed.scope, &physical).is_none());
    for change in 0..9 {
        let mut progress = confirmed.clone();
        match change {
            0 => progress.scope.generation += 1,
            1 => progress.firmware.clear(),
            2 => progress.error = Some("cancel requested".into()),
            3 => progress.cleanup_error = Some("release failed".into()),
            4..=8 => {
                let ThxFirmwareProgress::Command(command) = &mut progress.firmware[0] else {
                    unreachable!()
                };
                match change {
                    4 => command.completed = false,
                    5 => command.identity.node.path = b"replacement-hid".to_vec(),
                    6 => command.identity.node.product_id = 1398,
                    7 => command.identity.node.serial_number = Some("replacement-serial".into()),
                    8 => command.attempts.clear(),
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
        assert!(
            reset_error(&progress, &confirmed.scope, &physical).is_some(),
            "case {change}"
        );
    }
}
