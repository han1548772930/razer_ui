use super::*;

#[test]
fn protocol_headers_match_each_independently_captured_product() {
    let source: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/blackshark-roller-mapping-current.json"
    ))
    .unwrap();
    let products = source["products"].as_array().unwrap();
    assert_eq!(
        products
            .iter()
            .map(|row| row["product_id"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1398, 1401, 2638, 2641, 4124, 4126]
    );
    for product in products {
        let commands = &product["command_headers"];
        assert_eq!(
            commands["Dv"],
            json!(BlackSharkFirmwareCommand::GetRollerFunction.command_header())
        );
        assert_eq!(
            commands["xVt"],
            json!(BlackSharkFirmwareCommand::GetRollerFunction.expected_response())
        );
        assert_eq!(
            commands["bHm"],
            json!(BlackSharkFirmwareCommand::SetRollerFunction(47).command_header())
        );
        assert_eq!(
            commands["$yn"],
            json!(BlackSharkFirmwareCommand::SetRollerFunction(47).expected_response())
        );
        let config = product["feature_config"]["body"].as_str().unwrap();
        assert!(config.contains("initFromDevice: true"));
        assert!(config.contains("Storage_Global"));
        assert_eq!(product["inputs"].as_array().unwrap().len(), 8);
        let firmware = &product["firmware_version"];
        assert_eq!(
            firmware["query"],
            json!(BlackSharkFirmwareCommand::GetFirmwareVersions.command_header())
        );
        assert_eq!(
            firmware["reply"],
            json!(BlackSharkFirmwareCommand::GetFirmwareVersions.expected_response())
        );
        assert!(
            firmware["complete_bodies"]["wired_getter"]
                .as_str()
                .unwrap()
                .contains("this.sendCommand(")
        );
        assert!(
            firmware["complete_bodies"]["dongle_getter"]
                .as_str()
                .unwrap()
                .contains("this.sendCommandToDongle(")
        );
        assert!(
            firmware["complete_bodies"]["select_current_version"]
                .as_str()
                .unwrap()
                .contains("hasMultipleFwVersions")
        );
    }
}

#[test]
fn current_getter_projection_preserves_default_disable_and_named_functions() {
    assert!(mappings_from_index(0).is_empty());
    assert!(mappings_from_index(255).is_empty());
    for (index, name) in [
        (1, "SidetoneLevel"),
        (2, "FootstepScaling"),
        (3, "BluetoothVolume"),
    ] {
        let mappings = mappings_from_index(index);
        assert_eq!(
            mappings[0]["audioGroup"],
            json!({"audioMode":name,"audioAssignment":name})
        );
        assert_eq!(mappings[0]["inputID"], "KEY_SWITCH_AUDIO_EQ");
        assert_eq!(mappings[0]["isHyperShift"], false);
    }
    assert_eq!(mappings_from_index(47)[0]["outputType"], "disableGroup");
}

#[test]
fn unknown_and_foreign_mappings_never_fabricate_a_hardware_ack() {
    let mappings = vec![
        json!({"outputType":"keyboardGroup"}),
        json!({"outputType":"audioGroup","audioGroup":{"audioAssignment":"Unknown"}}),
    ];
    let progress = execute(
        Some(&mappings),
        |_| panic!("No valid firmware command"),
        || Ok(()),
    );
    assert_eq!(progress.source_result, Some(false));
    assert_eq!(progress.ignored_mapping_indices, vec![0]);
    assert_eq!(progress.rejected_mappings.len(), 1);
    assert!(progress.commands.is_empty());
    assert!(progress.observed_index.is_none());
    let ignored = execute(
        Some(&mappings[..1]),
        |_| panic!("Ignored mapping must not write"),
        || Ok(()),
    );
    // The original source bool is true for an entirely ignored list. Keep the
    // empty actual command receipt, so callers cannot call this a device save.
    assert_eq!(ignored.source_result, Some(true));
    assert!(ignored.commands.is_empty());
}

#[test]
fn cancellation_before_exchange_preserves_unknown_outcome() {
    let progress = execute(
        Some(&[]),
        |_| panic!("Canceled request must not write"),
        || anyhow::bail!("canceled"),
    );
    assert!(progress.commands.is_empty());
    assert!(progress.source_result.is_none());
    assert_eq!(progress.error.as_deref(), Some("canceled"));
}
