use super::*;
use serde_json::{Value, json};

const CONTAINER: &str = "4715b865-8085-428a-9829-f2fe5984f0b6";

fn node() -> HidNode {
    HidNode {
        path: b"observed-mixer-collection".to_vec(),
        vendor_id: 0x1532,
        product_id: 1342,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 4,
        release_number: 0,
        serial_number: Some("observed-serial".into()),
        manufacturer: None,
        product: None,
    }
}

fn reply() -> Value {
    json!({
        "node": node(),
        "product_id": 1342,
        "device_container_id": CONTAINER,
        "result": {
            "source_property": "RazerT2ResetStream",
            "reset": {
                "completed_indices": [0, 1, 2, 3, 4, 5, 6, 7, 8],
                "return_codes": [[0,0], [1,0], [2,0], [3,0], [4,0], [5,0], [6,0], [7,0], [8,0]],
                "source_sequence_completed": true,
                "confirmation": "ioctl_completion_only_no_audio_state_readback"
            }
        }
    })
}

#[::core::prelude::v1::test]
fn mixer_restart_receipt_preserves_the_complete_source_sequence() {
    let accepted = accept_reply(reply(), &node(), CONTAINER).unwrap();
    assert_eq!(accepted.completed_indices, [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(accepted.return_codes.len(), 9);
    assert!(accepted.source_sequence_completed);
    assert_eq!(
        accepted.confirmation,
        razer_device::audio_mixer::StreamResetConfirmation::IoctlCompletion
    );
}

#[::core::prelude::v1::test]
fn mixer_restart_receipt_rejects_unverified_confirmation() {
    for confirmation in [
        json!("audio_state_readback"),
        json!("ioctl_completion"),
        json!(""),
        Value::Null,
        json!(true),
        json!({"ioctl_completion_only_no_audio_state_readback": null}),
    ] {
        let mut response = reply();
        response["result"]["reset"]["confirmation"] = confirmation.clone();
        assert!(
            accept_reply(response, &node(), CONTAINER).is_err(),
            "{confirmation}"
        );
    }
}

#[::core::prelude::v1::test]
fn mixer_restart_receipt_retains_nonzero_native_codes_without_claiming_recovered_audio() {
    let mut response = reply();
    response["result"]["reset"]["return_codes"][3] = json!([3, 0x10003]);
    response["result"]["reset"]["completed_indices"] = json!([0, 1, 2, 4, 5, 6, 7, 8]);
    let accepted = accept_reply(response, &node(), CONTAINER).unwrap();
    assert_eq!(accepted.return_codes[3], (3, 0x10003));
    assert!(!accepted.completed_indices.contains(&3));
    assert!(accepted.source_sequence_completed);
}

#[::core::prelude::v1::test]
fn mixer_restart_receipt_rejects_other_device_product_collection_and_property() {
    for (field, value) in [
        ("product_id", json!(1343)),
        ("device_container_id", json!("different-container")),
        ("node", {
            let mut other = node();
            other.path = b"different-collection".to_vec();
            serde_json::to_value(other).unwrap()
        }),
    ] {
        let mut response = reply();
        response[field] = value;
        assert!(
            accept_reply(response, &node(), CONTAINER).is_err(),
            "{field}"
        );
    }
    let mut response = reply();
    response["result"]["source_property"] = json!("RazerT2MixerSettingControl");
    assert!(accept_reply(response, &node(), CONTAINER).is_err());
}

#[::core::prelude::v1::test]
fn mixer_restart_receipt_rejects_missing_malformed_or_partial_results() {
    for field in [
        "completed_indices",
        "return_codes",
        "source_sequence_completed",
        "confirmation",
    ] {
        let mut response = reply();
        response["result"]["reset"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            accept_reply(response, &node(), CONTAINER).is_err(),
            "{field}"
        );
    }
    for value in [json!(false), Value::Null, json!("true")] {
        let mut response = reply();
        response["result"]["reset"]["source_sequence_completed"] = value;
        assert!(accept_reply(response, &node(), CONTAINER).is_err());
    }
    let mut response = reply();
    response["result"]["reset"]["return_codes"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(accept_reply(response, &node(), CONTAINER).is_err());
}

#[::core::prelude::v1::test]
fn mixer_restart_receipt_rejects_reordered_duplicate_or_fabricated_completions() {
    let mut reordered = reply();
    reordered["result"]["reset"]["return_codes"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(accept_reply(reordered, &node(), CONTAINER).is_err());
    let mut duplicate = reply();
    duplicate["result"]["reset"]["return_codes"][4] = json!([3, 0]);
    assert!(accept_reply(duplicate, &node(), CONTAINER).is_err());
    let mut fabricated = reply();
    fabricated["result"]["reset"]["return_codes"][4] = json!([4, 0x10001]);
    assert!(accept_reply(fabricated, &node(), CONTAINER).is_err());
}
