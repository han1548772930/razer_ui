//! Fixed wire bytes cover operation correlation, physical target and source units.
use super::*;

#[test]
fn mouse_tracking_start_wire_has_a_fixed_owner_target_and_asymmetric_shape() {
    let bytes = br#"{"id":17,"request":{"operation":"mouse_tracking_start","operation_id":"scope-1","target":{"kind":"windows_container","product_id":226,"physical_product_id":227,"peer_product_id":227,"device_container_id":"container","path":"hid-path"},"setting":{"Asymmetric":{"landing_distance":12,"lift_off_distance":13}}}}"#;
    let envelope: RequestEnvelope = serde_json::from_slice(bytes).unwrap();
    assert_eq!(envelope.id, 17);
    let ServiceRequest::MouseTrackingStart {
        operation_id,
        target,
        setting,
    } = &envelope.request
    else {
        panic!("Different operation");
    };
    assert_eq!(operation_id, "scope-1");
    assert_eq!(
        setting,
        &razer_device::mouse_tracking::TrackingSetting::Asymmetric {
            landing_distance: 12,
            lift_off_distance: 13
        }
    );
    let razer_device::mouse_tracking::Target::WindowsContainer(target) = target else {
        panic!("Different target");
    };
    assert_eq!(
        (
            target.product_id,
            target.physical_product_id,
            target.peer_product_id
        ),
        (226, 227, Some(227))
    );
    assert_eq!(serde_json::to_vec(&envelope).unwrap(), bytes);
}

#[test]
fn mouse_tracking_wire_rejects_missing_scope_target_and_wrong_setting_types() {
    for bytes in [
        r#"{"operation":"mouse_tracking_start","operation_id":"owner","setting":{"Symmetric":{"tracking_distance":2}}}"#,
        r#"{"operation":"mouse_tracking_cancel"}"#,
        r#"{"operation":"mouse_tracking_start","operation_id":"owner","target":{"kind":"windows_container","product_id":170,"physical_product_id":170,"peer_product_id":null,"device_container_id":"container","path":"hid"},"setting":{"Symmetric":{"tracking_distance":"2"}}}"#,
    ] {
        assert!(serde_json::from_str::<ServiceRequest>(bytes).is_err());
    }
}

#[test]
fn mouse_philips_wire_keeps_its_feature_and_full_target_identity() {
    let bytes = br#"{"id":19,"request":{"operation":"mouse_philips_start","operation_id":"philips-owner","target":{"kind":"windows_container","product_id":70,"physical_product_id":70,"peer_product_id":null,"device_container_id":"container","path":"hid-path"},"setting":"Disabled"}}"#;
    let envelope: RequestEnvelope = serde_json::from_slice(bytes).unwrap();
    let ServiceRequest::MousePhilipsStart {
        operation_id,
        target,
        setting,
    } = &envelope.request
    else {
        panic!("Different feature");
    };
    assert_eq!(operation_id, "philips-owner");
    assert_eq!(target.product_id(), 70);
    assert_eq!(*setting, razer_device::mouse_philips::Setting::Disabled);
    assert_eq!(serde_json::to_vec(&envelope).unwrap(), bytes);
}

#[test]
fn mouse_philips_wire_rejects_missing_numeric_data_and_orca_setting() {
    for bytes in [
        r#"{"operation":"mouse_philips_cancel"}"#,
        r#"{"operation":"mouse_philips_start","operation_id":"owner","target":{"kind":"windows_container","product_id":70,"physical_product_id":70,"peer_product_id":null,"device_container_id":"container","path":"hid"},"setting":{"Apply":{"lift_off_threshold":357,"calibration":{"xOffset":1}}}}"#,
        r#"{"operation":"mouse_philips_start","operation_id":"owner","target":{"kind":"windows_container","product_id":70,"physical_product_id":70,"peer_product_id":null,"device_container_id":"container","path":"hid"},"setting":{"Symmetric":{"tracking_distance":2}}}"#,
    ] {
        assert!(serde_json::from_str::<ServiceRequest>(bytes).is_err());
    }
}

#[test]
fn philips_measurement_wire_keeps_full_identity_kind_and_cancellation_owner() {
    let bytes = br#"{"id":23,"request":{"operation":"mouse_philips_measurement_start","operation_id":"measurement-owner","target":{"product_id":70,"physical_product_id":70,"peer_product_id":null,"device_container_id":"container","path":"hid-path"},"kind":"Mini"}}"#;
    let envelope: RequestEnvelope = serde_json::from_slice(bytes).unwrap();
    let ServiceRequest::MousePhilipsMeasurementStart {
        operation_id,
        target,
        kind,
    } = &envelope.request
    else {
        panic!()
    };
    assert_eq!(operation_id, "measurement-owner");
    assert_eq!(target.product_id, 70);
    assert_eq!(target.physical_product_id, 70);
    assert_eq!(target.peer_product_id, None);
    assert_eq!(target.path, "hid-path");
    assert_eq!(*kind, razer_device::mouse_philips::measurement::Kind::Mini);
    assert_eq!(serde_json::to_vec(&envelope).unwrap(), bytes);
    for bytes in [
        r#"{"operation":"mouse_philips_measurement_cancel"}"#,
        r#"{"operation":"mouse_philips_measurement_start","operation_id":"owner","kind":"Manual"}"#,
        r#"{"operation":"mouse_philips_measurement_start","operation_id":"owner","target":{"product_id":70,"physical_product_id":70,"peer_product_id":null,"device_container_id":"container","path":"hid"},"kind":"Preset"}"#,
    ] {
        assert!(serde_json::from_str::<ServiceRequest>(bytes).is_err());
    }
}
