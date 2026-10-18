//! Rotation uses the current physical mouse factory and retained receiver
//! routing; its typed setter result never substitutes for a GET.
use super::*;
use razer_device::mouse_rotation::{self as rotation, Report, Setting};

pub fn available(observed: &ObservedDevice) -> bool {
    let ble = cfg!(windows)
        && observed.transport() == Some(crate::discovery::ObservedTransport::Ble)
        && observed.peer_product_id().is_none()
        && rotation::ble::accepts(observed.product_id(), observed.physical_product_id());
    ble || rotation::capability(observed.product_id()).is_some()
        && super::mouse_current_dpi::available(observed)
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    change: Option<Setting>,
) -> anyhow::Result<Report> {
    request_with(observed, change, &mut |request| client.request(request))
}

fn request_with(
    observed: &ObservedDevice,
    change: Option<Setting>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Report> {
    ensure!(
        available(observed),
        "Rotation has no current physical route"
    );
    if let Some(change) = &change {
        change.validate()?;
    }
    if observed.transport() == Some(crate::discovery::ObservedTransport::Ble) {
        #[cfg(windows)]
        {
            let observation = request(ServiceRequest::HidDevices)?;
            let target = rotation::ble::target_from_hid(
                observed.product_id(),
                observed.physical_product_id(),
                observed.container(),
                &observation,
            )?;
            let reply = request(ServiceRequest::BleMouseRotation {
                target: target.clone(),
                change,
            })?;
            return decode_ble(&target, &reply, change.as_ref());
        }
        #[cfg(not(windows))]
        anyhow::bail!("Rotation BLE requires its platform GATT adapter");
    }
    let session = observed
        .peer_product_id()
        .map(|peer| {
            rotation::capability(observed.product_id())
                .context("Rotation factory is absent")?
                .receiver_session(observed.physical_product_id(), peer)
        })
        .transpose()?;
    let target = if let Some(session) = &session {
        super::receiver_settings::resolve(observed, session, request)?
    } else {
        super::mouse_current_dpi::resolve_with(observed, request)?
    };
    let query = match &target {
        DirectRoute::Collection { node, product_id } => ServiceRequest::HidNodeMouseRotation {
            node: node.clone(),
            product_id: *product_id,
            peer_product_id: observed.peer_product_id(),
            change: change.clone(),
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => ServiceRequest::DeviceMouseRotation {
            target: target.clone(),
            change: change.clone(),
        },
    };
    let reply = request(query)?;
    target.check_reply(&reply)?;
    if let Some(session) = &session {
        ensure!(
            reply["peer_product_id"] == observed.peer_product_id().unwrap(),
            "Rotation reply belongs to another receiver peer"
        );
        super::receiver_settings::check_peers(&reply, session)?;
    }
    decode(observed.product_id(), &reply, change.as_ref())
}

fn decode(product_id: u32, reply: &Value, change: Option<&Setting>) -> anyhow::Result<Report> {
    let source = rotation::capability(product_id).context("Rotation source is absent")?;
    #[derive(serde::Deserialize)]
    struct Reply {
        source_class: String,
        profile_scope: String,
        report: Report,
    }
    let reply: Reply = serde_json::from_value(reply.clone())?;
    ensure!(
        reply.source_class == source.transport().source_class
            && reply.profile_scope == "rotation_profile_1",
        "Rotation reply source or profile scope differs"
    );
    reply.report.validate_reply(change)?;
    reply
        .report
        .validate_transport(rotation::TransportKind::Feature)?;
    Ok(reply.report)
}

#[cfg(any(windows, test))]
fn decode_ble(
    target: &razer_device::mouse_runtime_artwork::WindowsBleTarget,
    reply: &Value,
    change: Option<&Setting>,
) -> anyhow::Result<Report> {
    #[derive(serde::Deserialize)]
    struct Reply {
        target: razer_device::mouse_runtime_artwork::WindowsBleTarget,
        source_class: String,
        transport: String,
        profile_scope: String,
        report: Report,
    }
    ensure!(
        rotation::ble::accepts(target.product_id, target.physical_product_id),
        "Rotation BLE source differs"
    );
    let reply: Reply = serde_json::from_value(reply.clone())?;
    ensure!(
        reply.target == *target
            && reply.source_class == "rzDevice30"
            && reply.transport == "windows_gatt"
            && reply.profile_scope == "rotation_profile_1",
        "Rotation BLE reply identity/transport/profile differs"
    );
    reply.report.validate_reply(change)?;
    reply
        .report
        .validate_transport(rotation::TransportKind::Ble)?;
    Ok(reply.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mouse_rotation_ble_reply_retains_physical_identity_and_exact_protocol_ack() {
        let container = "{01234567-89AB-CDEF-0123-456789ABCDEF}";
        let observation = json!({"complete":true,"interfaces":[{
            "path":r"\\?\hid#rotation#ble",
            "device_container_id":container,"device_instance_id":r"HID\BLE_MOUSE\ROTATION",
            "parent_instance_id":r"BTHLEDevice\_VID&021532_PID&00E4_REV&0100_A1B2C3D4E5F6",
            "vendor_id":0x1532,"product_id":228
        }]});
        let target = rotation::ble::target_from_hid(226, 228, container, &observation).unwrap();
        assert_eq!(target.mac_address, 0xa1b2c3d4e5f6);
        assert!(rotation::ble::target_from_hid(226, 227, container, &observation).is_err());
        let setting = Setting {
            enabled: true,
            angle_degrees: -44,
        };
        let reply = json!({"target":target,"source_class":"rzDevice30","transport":"windows_gatt",
            "profile_scope":"rotation_profile_1","report":{"operation":"ble_write",
                "requested":{"enabled":true,"angle_degrees":-44},"failure":null,
                "delivery":{"attempted_writes":1,"acknowledgement":{
                    "command":[2,15,20],"profile":1,"transaction":1,"payload":[1,212],"data":[]}}}});
        assert!(
            decode_ble(&target, &reply, Some(&setting))
                .unwrap()
                .source_acknowledged()
        );
        for (path, value) in [
            ("/target/physical_product_id", json!(227)),
            ("/target/device_instance_id", json!("another-instance")),
            ("/target/mac_address", json!(1)),
            ("/source_class", json!("rzDevice25")),
            ("/profile_scope", json!("profile_0")),
            ("/report/delivery/acknowledgement/profile", json!(0)),
            (
                "/report/delivery/acknowledgement/command",
                json!([3, 11, 20]),
            ),
            ("/report/delivery/acknowledgement/payload", json!([1, 44])),
            ("/report/delivery/acknowledgement/transaction", json!(31)),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(
                decode_ble(&target, &invalid, Some(&setting)).is_err(),
                "{path}"
            );
        }
        let mut usb_reply = reply.clone();
        usb_reply["source_class"] = json!("rzDevice25");
        assert!(decode(226, &usb_reply, Some(&setting)).is_err());
        assert!(decode_ble(&target, &reply, None).is_err());
    }

    #[test]
    fn mouse_rotation_reply_rejects_wrong_scope_operation_and_forged_delivery() {
        let setting = Setting {
            enabled: true,
            angle_degrees: -44,
        };
        let reply = json!({"source_class":"rzDevice25","profile_scope":"rotation_profile_1",
            "report":{"operation":"write","requested":{"enabled":true,"angle_degrees":-44},
                "delivery":{"attempted_writes":1,"acknowledgements":[{"command":[3,11,20],"payload":[1,1,212],"data":[]}]},"failure":null}});
        assert!(
            decode(190, &reply, Some(&setting))
                .unwrap()
                .source_acknowledged()
        );
        assert!(decode(190, &reply, None).is_err());
        for (path, value) in [
            ("/source_class", json!("rzDevice14")),
            ("/profile_scope", json!("dynamic_profile_1")),
            ("/report/requested/angle_degrees", json!(12)),
            ("/report/delivery/attempted_writes", json!(0)),
            (
                "/report/delivery/acknowledgements/0/payload",
                json!([1, 1, 12]),
            ),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(decode(190, &invalid, Some(&setting)).is_err(), "{path}");
        }
        let read = json!({"source_class":"rzDevice25","profile_scope":"rotation_profile_1",
            "report":{"operation":"read","reading":{"raw_state":254,"raw_angle":200}}});
        let Report::Read { reading } = decode(190, &read, None).unwrap() else {
            panic!("read kind changed")
        };
        assert_eq!((reading.enabled(), reading.angle_degrees()), (None, None));
        assert!(decode(70, &read, None).is_err());
        assert!(decode(190, &read, Some(&setting)).is_err());
    }
}
