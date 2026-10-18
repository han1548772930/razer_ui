use super::*;

fn request(owner: u64, generation: u64, audio_streams: bool) -> HelpResetRequest {
    HelpResetRequest {
        owner: EntityId::from(owner),
        generation,
        audio_streams,
        source_action: (!audio_streams).then_some("ON_RESET_DEVICE"),
        source_payload: false,
        source_payload_value: None,
        oled_reset_obm: None,
        firmware_hardware_reset: false,
        oled_factory_reset: false,
        oled_source_clone_failure: false,
    }
}

#[::core::prelude::v1::test]
fn help_reset_identity_rejects_a_replacement_help_owner_with_the_same_counter() {
    let original = request(18, 1, false);
    let replacement = request(19, 1, false);
    assert!(!original.same_operation(replacement));
    assert!(!replacement.same_operation(original));
    assert_eq!(original.owner(), EntityId::from(18));
}

#[::core::prelude::v1::test]
fn help_reset_identity_keeps_repeated_audio_clicks_independent() {
    let first = request(18, 1, true);
    let second = request(18, 2, true);
    assert!(!first.same_operation(second));
    assert!(first.same_operation(first));
    assert!(second.same_operation(second));
}

#[::core::prelude::v1::test]
fn help_reset_identity_separates_audio_restart_from_profile_reset() {
    assert!(!request(18, 7, true).same_operation(request(18, 7, false)));
}

#[::core::prelude::v1::test]
fn help_reset_identity_does_not_use_source_payload_as_the_operation_owner() {
    let original = request(18, 7, false);
    let mut canceled = original;
    canceled.firmware_hardware_reset = true;
    canceled.source_payload = true;
    assert!(original.same_operation(canceled));
}
