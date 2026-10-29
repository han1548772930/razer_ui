use super::*;
use razer_device::mouse_current_dpi::{self as dpi, DpiChange, DpiReport};

pub fn available(observed: &ObservedDevice) -> bool {
    if observed.transport() == Some(crate::discovery::ObservedTransport::Ble) {
        return cfg!(windows)
            && observed.peer_product_id().is_none()
            && dpi::ble::accepts(observed.product_id(), observed.physical_product_id());
    }
    feature_available(observed)
}

pub(super) fn feature_available(observed: &ObservedDevice) -> bool {
    if observed.peer_product_id().is_some() {
        return receiver_session(observed).is_ok();
    }
    dpi::capability(observed.product_id()).is_some_and(|source| match observed.transport() {
        Some(crate::discovery::ObservedTransport::Wired) => {
            source.is_physical_branch(observed.physical_product_id(), false)
                && observed.peer_product_id().is_none()
        }
        Some(crate::discovery::ObservedTransport::Dongle) => {
            source.is_physical_branch(observed.physical_product_id(), true)
                && observed
                    .peer_product_id()
                    .is_none_or(|peer| peer == source.product_id())
        }
        _ => false,
    })
}

fn receiver_session(
    observed: &ObservedDevice,
) -> anyhow::Result<razer_device::receiver_session::ReceiverSession> {
    ensure!(
        observed.transport() == Some(crate::discovery::ObservedTransport::Dongle)
            && observed.connection()
                == razer_model::model::DeviceConnectionObservation::ReceiverPeer(1),
        "Current DPI requires an online receiver observation"
    );
    dpi::capability(observed.product_id())
        .context("Current DPI source is absent")?
        .receiver_session(
            observed.physical_product_id(),
            observed
                .peer_product_id()
                .context("Receiver peer is absent")?,
        )
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    change: Option<DpiChange>,
) -> anyhow::Result<DpiReport> {
    request_with(observed, change, |request| client.request(request))
}

pub(super) fn request_with(
    observed: &ObservedDevice,
    change: Option<DpiChange>,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DpiReport> {
    ensure!(
        available(observed),
        "Current mouse DPI has no live source route"
    );
    if let Some(change) = &change {
        dpi::validate_change_for(observed.product_id(), change)?;
    }
    if observed.transport() == Some(crate::discovery::ObservedTransport::Ble) {
        #[cfg(windows)]
        {
            return request_ble_with(
                observed.product_id(),
                observed.physical_product_id(),
                observed.container(),
                change,
                &mut request,
            );
        }
        #[cfg(not(windows))]
        anyhow::bail!("Current DPI BLE requires its platform GATT adapter");
    }
    if observed.peer_product_id().is_some() {
        let session = receiver_session(observed)?;
        let target = super::receiver_settings::resolve(observed, &session, &mut request)?;
        let query = match &target {
            DirectRoute::Collection { node, product_id } => {
                ServiceRequest::HidNodeMouseCurrentDpi {
                    node: node.clone(),
                    product_id: *product_id,
                    peer_product_id: observed.peer_product_id(),
                    change: change.clone(),
                }
            }
            #[cfg(windows)]
            DirectRoute::Container(target) => ServiceRequest::DeviceMouseCurrentDpi {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = request(query)?;
        target.check_reply(&reply)?;
        ensure!(
            reply["peer_product_id"] == observed.peer_product_id().unwrap(),
            "DPI response belongs to another receiver peer"
        );
        super::receiver_settings::check_peers(&reply, &session)?;
        ensure!(
            reply["obm_persistence_verified"] == false,
            "Source Mt setter acknowledgements cannot claim OBM readback"
        );
        decode_reply(observed.product_id(), &reply, change)
    } else {
        resolve_with(observed, &mut request)?.mouse_current_dpi_with(change, &mut request)
    }
}

#[cfg(any(windows, test))]
fn request_ble_with(
    product_id: u32,
    physical_product_id: u32,
    container: &str,
    change: Option<DpiChange>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DpiReport> {
    ensure!(
        dpi::ble::accepts(product_id, physical_product_id),
        "Current DPI BLE request has another physical factory"
    );
    if let Some(change) = &change {
        dpi::validate_change_for(product_id, change)?;
    }
    let observation = request(ServiceRequest::HidDevices)?;
    let target =
        dpi::ble::target_from_hid(product_id, physical_product_id, container, &observation)?;
    let reply = request(ServiceRequest::BleMouseCurrentDpi {
        target: target.clone(),
        change: change.clone(),
    })?;
    decode_ble_reply(&target, &reply, change)
}

/// Admit only the actual wired/ordinary-dongle branch of each current factory.
/// The service verifies ordinary receivers online around the device operation.
pub fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<DirectRoute> {
    resolve_with(observed, &mut |request| client.request(request))
}

pub(super) fn resolve_with(
    observed: &ObservedDevice,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<DirectRoute> {
    let source = dpi::capability(observed.product_id())
        .context("Mouse DPI observation has no current source factory capability")?;
    let cap = source.transport();
    ensure!(
        match observed.transport() {
            Some(crate::discovery::ObservedTransport::Wired) =>
                source.is_physical_branch(observed.physical_product_id(), false)
                    && observed.peer_product_id().is_none(),
            Some(crate::discovery::ObservedTransport::Dongle) =>
                source.is_physical_branch(observed.physical_product_id(), true)
                    && observed
                        .peer_product_id()
                        .is_none_or(|peer| peer == source.product_id()),
            _ => false,
        },
        "Mouse DPI observation differs from its source wired/dongle branches"
    );
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == observed.physical_product_id()
                && node.interface_number
                    == i32::from(cap.claim_interface_for(observed.physical_product_id())?),
            "Mouse DPI collection interface differs"
        );
        let current = request(ServiceRequest::HidNodes)?;
        let nodes: Vec<HidNode> = serde_json::from_value(current["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Current mouse DPI collection changed or is ambiguous"
        );
        source_reports_match_with(node, cap.report_bytes, request)?;
        return Ok(DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        });
    }
    #[cfg(windows)]
    {
        windows::resolve_with(observed, cap, request).map(DirectRoute::Container)
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Current mouse DPI has no observed HID collection on this platform")
    }
}

impl DirectRoute {
    pub fn mouse_current_dpi(
        &self,
        client: &mut ServiceClient,
        change: Option<DpiChange>,
    ) -> anyhow::Result<DpiReport> {
        self.mouse_current_dpi_with(change, &mut |request| client.request(request))
    }

    fn mouse_current_dpi_with(
        &self,
        change: Option<DpiChange>,
        request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    ) -> anyhow::Result<DpiReport> {
        let product_id = match self {
            DirectRoute::Collection { product_id, .. } => *product_id,
            #[cfg(windows)]
            DirectRoute::Container(target) => target.product_id,
        };
        if let Some(change) = &change {
            dpi::validate_change_for(product_id, change)?;
        }
        let query = match self {
            DirectRoute::Collection { node, product_id } => {
                ServiceRequest::HidNodeMouseCurrentDpi {
                    node: node.clone(),
                    product_id: *product_id,
                    peer_product_id: None,
                    change: change.clone(),
                }
            }
            #[cfg(windows)]
            DirectRoute::Container(target) => ServiceRequest::DeviceMouseCurrentDpi {
                target: target.clone(),
                change: change.clone(),
            },
        };
        let reply = request(query)?;
        self.check_reply(&reply)?;
        decode_reply(product_id, &reply, change)
    }
}

fn decode_reply(
    product_id: u32,
    reply: &Value,
    change: Option<DpiChange>,
) -> anyhow::Result<DpiReport> {
    let source = dpi::capability(product_id).context("DPI source capability is absent")?;
    #[derive(serde::Deserialize)]
    struct Reply {
        source_class: String,
        profile_scope: String,
        report: DpiReport,
    }
    let reply: Reply = serde_json::from_value(reply.clone())?;
    ensure!(
        reply.source_class == source.transport().source_class
            && reply.profile_scope == "stage_1_active_0",
        "Current mouse DPI reply source or profile scope differs"
    );
    validate_report(product_id, reply.report, change, ReplyTransport::Feature)
}

#[cfg(any(windows, test))]
fn decode_ble_reply(
    target: &razer_device::mouse_runtime_artwork::WindowsBleTarget,
    reply: &Value,
    change: Option<DpiChange>,
) -> anyhow::Result<DpiReport> {
    #[derive(serde::Deserialize)]
    struct Reply {
        target: razer_device::mouse_runtime_artwork::WindowsBleTarget,
        source_class: String,
        transport: String,
        profile_scope: String,
        connection_edition: dpi::Acknowledgement,
        report: DpiReport,
    }
    ensure!(
        dpi::ble::accepts(target.product_id, target.physical_product_id),
        "Current DPI BLE source factory differs"
    );
    let reply: Reply = serde_json::from_value(reply.clone())?;
    ensure!(
        reply.target == *target
            && reply.source_class == "rzDevice30"
            && reply.transport == "windows_gatt"
            && reply.profile_scope == "stage_1_active_0",
        "Current DPI BLE reply identity/transport/profile differs"
    );
    let edition = reply.connection_edition;
    ensure!(
        edition.command == dpi::ble::EDITION
            && edition.profile == Some(0)
            && edition.transaction < 31
            && edition.response_bytes == edition.data.len()
            && (3..=65535).contains(&edition.data.len()),
        "Current DPI BLE connection edition is absent or differs"
    );
    ensure!(
        reply.report.stage_level.is_none()
            && reply.report.serial_number.is_none()
            && reply.report.initialization_mode_error.is_none(),
        "Current DPI BLE reply contains unrelated device operations"
    );
    dpi::ble::validate_readings(&reply.report)?;
    if let Some(change) = &change {
        ensure!(
            reply.report.profile_slots.is_empty() && reply.report.profile_slot_writes.is_empty()
                || change.stages.is_some()
                    && change
                        .profile_guid
                        .as_ref()
                        .is_some_and(|guid| !guid.is_empty()),
            "Current DPI BLE profile synchronization has no requested stage or profile"
        );
        let expected_slots = dpi_profile_slots(change, &reply.report);
        ensure!(
            reply.report.write_errors.stages().is_some()
                || reply.report.profile_slot_writes.len() == expected_slots.len(),
            "Current DPI BLE profile synchronization omits observed matching slots"
        );
        for (index, write) in reply.report.profile_slot_writes.iter().enumerate() {
            ensure!(
                expected_slots.get(index).copied() == Some(write.slot_id)
                    && (write.error.is_some()
                        || write.before.as_ref().is_some_and(|before| {
                            before.profile_id == write.slot_id
                                && if write.was_equal {
                                    write.acknowledged.is_none()
                                } else {
                                    write.acknowledged.as_ref().is_some_and(|ack| {
                                        ack.response_bytes == ack.raw_data.len()
                                            && ack.response_bytes <= 65535
                                    })
                                }
                        })),
                "Current DPI BLE profile write has another slot or absent delivery"
            );
            ensure!(
                write.error.is_none() || reply.report.write_errors.stages().is_some(),
                "Current DPI BLE profile synchronization failure claims stage success"
            );
        }
    }
    validate_report(target.product_id, reply.report, change, ReplyTransport::Ble)
}

#[cfg(any(windows, test))]
fn dpi_profile_slots(change: &DpiChange, report: &DpiReport) -> Vec<u8> {
    change.profile_guid.as_ref().map_or_else(Vec::new, |guid| {
        razer_device::mouse_dpi_stages::select_profile_slots(guid, &report.profile_slots)
            .into_iter()
            .map(|slot| slot.slot_id)
            .collect()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReplyTransport {
    Feature,
    Ble,
}

fn validate_report(
    product_id: u32,
    report: DpiReport,
    change: Option<DpiChange>,
    transport: ReplyTransport,
) -> anyhow::Result<DpiReport> {
    let source = dpi::capability(product_id).context("DPI source capability is absent")?;
    ensure!(
        report.requested == change
            && report.verified == (report.read_errors.is_empty() && report.write_errors.is_empty())
            && report.acknowledgements.iter().all(|ack| {
                ack.transaction < 31
                    && ack.response_bytes == ack.data.len()
                    && if transport == ReplyTransport::Ble {
                        ack.response_bytes <= 65535
                            && matches!(
                                (ack.command, ack.profile),
                                ([255, 11, 4], Some(1)) | ([6, 11, 1], Some(0))
                            )
                    } else {
                        ack.profile.is_none() && ack.response_bytes <= 80
                    }
            }),
        "Current mouse DPI reply does not confirm its exact request"
    );
    if let Some(change) = &change {
        ensure!(
            report.read_errors.is_empty(),
            "DPI write reply contains failed getter observations"
        );
        if change.is_identity_read() {
            ensure!(
                report.verified
                    && matches!(product_id, 182 | 190)
                    && report
                        .serial_number
                        .as_deref()
                        .is_some_and(|serial| !serial.is_empty()
                            && serial.bytes().all(|byte| (32..127).contains(&byte)))
                    && report.acknowledgements.is_empty()
                    && report.stages.is_none()
                    && report.stage_level.is_none()
                    && report.active.is_none()
                    && report.profile_slots.is_empty()
                    && report.profile_slot_writes.is_empty()
                    && report.initialization_mode_error.is_none(),
                "Initialization identity reply is absent or contains unrelated device operations"
            );
        }
        ensure!(
            (change.stages.is_some() || report.write_errors.stages().is_none())
                && (change.active.is_some() || report.write_errors.active().is_none())
                && (transport != ReplyTransport::Ble
                    || report.acknowledgements.iter().all(|ack| match ack.profile {
                        Some(1) => change.stages.is_some(),
                        Some(0) => change.active.is_some(),
                        _ => false,
                    })),
            "DPI reply contains a failure for an unrequested write task"
        );
        if let Some(stages) = &change.stages {
            let packed = dpi::pack_for(product_id, stages)?;
            if source.is_stage_level_only() {
                ensure!(
                    report.write_errors.stages().is_some()
                        || report
                            .acknowledgements
                            .iter()
                            .any(|ack| ack.command == [7, 4, 5]),
                    "DPI level-only stage reply has no profile setter acknowledgement"
                );
            } else {
                ensure!(
                    report.write_errors.stages().is_some()
                        || (report.acknowledgements.iter().any(|ack| {
                            if transport == ReplyTransport::Ble {
                                ack.command == [255, 11, 4] && ack.profile == Some(1)
                            } else {
                                ack.command == [3 + 7 * packed.number_of_active_stages, 4, 6]
                            }
                        }) && report
                            .stages
                            .as_ref()
                            .is_some_and(|reading| dpi::same_stages(&packed, reading))),
                    "Current mouse DPI reply has no stage setter acknowledgement or its readback differs"
                );
            }
        }
        if let Some(active) = change.active {
            ensure!(
                report.write_errors.active().is_some() || report.active == Some(active),
                "Current mouse DPI reply active readback differs"
            );
        }
    } else {
        let stages_present =
            report.stages.is_some() || source.is_stage_level_only() && report.stage_level.is_some();
        ensure!(
            stages_present != report.read_errors.stages().is_some()
                && report.write_errors.is_empty()
                && report.active.is_some() != report.read_errors.active().is_some()
                && report.verified == report.read_errors.is_empty()
                && report.acknowledgements.is_empty()
                && report.profile_slots.is_empty()
                && report.profile_slot_writes.is_empty()
                && report.serial_number.is_none()
                && report.initialization_mode_error.is_none(),
            "Current mouse DPI read reply has absent or contradictory observations"
        );
        if let Some(stages) = &report.stages {
            ensure!(
                stages.profile_id == 1,
                "DPI stages belong to another profile"
            );
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ble_reply() -> (razer_device::mouse_runtime_artwork::WindowsBleTarget, Value) {
        let container = "{01234567-89AB-CDEF-0123-456789ABCDEF}";
        let observation = json!({"complete":true,"interfaces":[{
            "path":r"\\?\hid#dpi#ble",
            "device_container_id":container,"device_instance_id":r"HID\BLE_MOUSE\DPI",
            "parent_instance_id":r"BTHLEDevice\_VID&021532_PID&00E4_REV&0100_A1B2C3D4E5F6",
            "vendor_id":0x1532,"product_id":228
        }]});
        let target = dpi::ble::target_from_hid(226, 228, container, &observation).unwrap();
        let reply = json!({
            "target":target,"source_class":"rzDevice30","transport":"windows_gatt",
            "profile_scope":"stage_1_active_0",
            "connection_edition":{"command":[0,1,134],"profile":0,"transaction":0,
                "response_bytes":3,"data":[1,2,3]},
            "report":{"requested":null,"active":{"x":900,"y":950},"verified":true,
                "acknowledgements":[],
                "stages":{"profile_id":1,"active_stage":9,"number_of_active_stages":7,
                    "records":[{"index":1,"x":400,"y":500,"z":0},
                        {"index":0,"x":0,"y":0,"z":0}],"response_bytes":16}}
        });
        (target, reply)
    }

    #[test]
    fn current_dpi_ble_reply_requires_its_current_physical_owner_and_connection_edition() {
        let (target, reply) = ble_reply();
        let actual = decode_ble_reply(&target, &reply, None).unwrap();
        assert_eq!(actual.active, Some(dpi::DpiPair { x: 900, y: 950 }));
        assert_eq!(actual.stages.unwrap().records[0].x, 400);
        for (path, value) in [
            ("/target/physical_product_id", json!(227)),
            ("/target/product_id", json!(190)),
            ("/target/device_instance_id", json!("stale-instance")),
            ("/target/mac_address", json!(1)),
            ("/source_class", json!("rzDevice25")),
            ("/transport", json!("hid_feature")),
            ("/profile_scope", json!("profile_0")),
            ("/connection_edition/command", json!([0, 11, 129])),
            ("/connection_edition/profile", json!(1)),
            ("/connection_edition/transaction", json!(31)),
            ("/connection_edition/response_bytes", json!(2)),
            ("/connection_edition/data", json!([1, 2])),
            ("/connection_edition", Value::Null),
            ("/report/active/x", json!(0)),
            ("/report/active/y", json!(50001)),
            ("/report/stages/response_bytes", json!(1)),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(decode_ble_reply(&target, &invalid, None).is_err(), "{path}");
        }
    }

    #[test]
    fn current_dpi_ble_dispatch_resolves_the_live_hid_owner_before_the_exact_task() {
        let (target, reply) = ble_reply();
        let observation = json!({"complete":true,"interfaces":[{
            "path":target.hid_path,"device_container_id":target.device_container_id,
            "device_instance_id":target.device_instance_id,
            "parent_instance_id":target.parent_instance_id,"vendor_id":0x1532,"product_id":228
        }]});
        let mut calls = Vec::new();
        let report = request_ble_with(
            226,
            228,
            &target.device_container_id,
            None,
            &mut |request| {
                calls.push(request.clone());
                match request {
                    ServiceRequest::HidDevices => Ok(observation.clone()),
                    ServiceRequest::BleMouseCurrentDpi {
                        target: selected,
                        change: None,
                    } => {
                        assert_eq!(selected, target);
                        Ok(reply.clone())
                    }
                    _ => panic!("BLE DPI must not use a HID Feature Report request"),
                }
            },
        )
        .unwrap();
        assert!(report.verified);
        assert_eq!(calls.len(), 2);
        for observation in [
            json!({"complete":true,"interfaces":[]}),
            json!({"complete":false,"interfaces":[]}),
        ] {
            let mut calls = 0;
            assert!(
                request_ble_with(
                    226,
                    228,
                    &target.device_container_id,
                    None,
                    &mut |request| {
                        calls += 1;
                        assert!(matches!(request, ServiceRequest::HidDevices));
                        Ok(observation.clone())
                    },
                )
                .is_err()
            );
            assert_eq!(calls, 1);
        }
        assert!(
            request_ble_with(
                226,
                227,
                &target.device_container_id,
                None,
                &mut |_| panic!("Wrong physical identity must stop before enumeration")
            )
            .is_err()
        );
    }

    #[test]
    fn current_dpi_ble_stage_write_requires_the_ble_setter_profile_and_actual_getter() {
        let (target, mut reply) = ble_reply();
        let change = DpiChange {
            stages: Some(dpi::SourceStages::new(
                true,
                1,
                vec![dpi::SourceStage::new(
                    Some(400u16.into()),
                    Some(500u16.into()),
                    Some(true),
                    Some(false),
                )],
            )),
            active: None,
            profile_guid: None,
            initialize: false,
        };
        reply["report"]["requested"] = json!(change);
        reply["report"]["active"] = Value::Null;
        reply["report"]["acknowledgements"] = json!([
            {"command":[255,11,4],"profile":1,"transaction":1,"response_bytes":0,"data":[]}
        ]);
        assert!(
            decode_ble_reply(&target, &reply, Some(change.clone()))
                .unwrap()
                .verified
        );
        for (path, value) in [
            ("/report/acknowledgements/0/command", json!([10, 4, 6])),
            ("/report/acknowledgements/0/profile", json!(0)),
            ("/report/acknowledgements/0/profile", Value::Null),
            ("/report/acknowledgements/0/transaction", json!(31)),
            ("/report/acknowledgements", json!([])),
            ("/report/stages/records/0/x", json!(401)),
            ("/report/requested/stages/active_stage", json!(2)),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(
                decode_ble_reply(&target, &invalid, Some(change.clone())).is_err(),
                "{path}"
            );
        }
        reply["report"]["stages"]["records"][0]["x"] = json!(401);
        reply["report"]["write_errors"] = json!({"stages":"readback differs","active":null});
        reply["report"]["verified"] = json!(false);
        let failed = decode_ble_reply(&target, &reply, Some(change.clone())).unwrap();
        assert!(!failed.verified);
        assert_eq!(failed.stages.unwrap().records[0].x, 401);
        reply["source_class"] = json!("rzDevice25");
        assert!(decode_reply(226, &reply, Some(change)).is_err());
    }

    #[test]
    fn current_dpi_ble_partial_read_preserves_observed_axes_without_a_success_claim() {
        let (target, mut reply) = ble_reply();
        reply["report"]["stages"] = Value::Null;
        reply["report"]["read_errors"] =
            json!({"stages":"device rejected stage getter","active":null});
        reply["report"]["verified"] = json!(false);
        let actual = decode_ble_reply(&target, &reply, None).unwrap();
        assert_eq!(actual.active, Some(dpi::DpiPair { x: 900, y: 950 }));
        assert!(actual.stages.is_none());
        assert!(!actual.verified);
        reply["report"]["verified"] = json!(true);
        assert!(decode_ble_reply(&target, &reply, None).is_err());
        reply["report"]["verified"] = json!(false);
        reply["report"]["read_errors"]["stages"] = Value::Null;
        assert!(decode_ble_reply(&target, &reply, None).is_err());
    }

    #[test]
    fn stage_confirmation_requires_its_setter_ack_and_uses_the_source_record_consumer() {
        let change = DpiChange {
            stages: Some(dpi::SourceStages::new(
                true,
                1,
                vec![dpi::SourceStage::new(
                    Some(dpi::SourceNumber::new(400.)),
                    Some(dpi::SourceNumber::new(500.)),
                    Some(true),
                    Some(false),
                )],
            )),
            active: None,
            profile_guid: None,
            initialize: false,
        };
        let mut reply = json!({
            "source_class":dpi::capability(190).unwrap().transport().source_class,
            "profile_scope":"stage_1_active_0",
            "report":{"requested":change,"active":null,"verified":true,"acknowledgements":[],
                "stages":{"profile_id":1,"active_stage":9,"number_of_active_stages":7,
                    "records":[{"index":1,"x":400,"y":500,"z":0},
                        {"index":0,"x":0,"y":0,"z":0}],"response_bytes":17}}
        });
        assert!(decode_reply(190, &reply, Some(change.clone())).is_err());
        reply["report"]["acknowledgements"] = json!([
            {"command":[10,4,6],"transaction":0,"response_bytes":0,"data":[]}
        ]);
        let actual = decode_reply(190, &reply, Some(change.clone())).unwrap();
        assert!(actual.verified);
        assert_eq!(actual.stages.as_ref().unwrap().active_stage, 9);
        reply["report"]["acknowledgements"][0]["command"] = json!([17, 4, 6]);
        assert!(decode_reply(190, &reply, Some(change)).is_err());
    }

    #[test]
    fn tagged_number_echo_keeps_a_failed_write_and_its_actual_getter_distinct() {
        let change = DpiChange {
            stages: Some(dpi::SourceStages::new(
                true,
                1,
                vec![dpi::SourceStage::new(
                    Some(dpi::SourceNumber::new(f64::from_bits(
                        0x7ff8_0000_0000_0042,
                    ))),
                    Some(dpi::SourceNumber::new(0.)),
                    Some(true),
                    Some(false),
                )],
            )),
            active: None,
            profile_guid: None,
            initialize: false,
        };
        let mut reply = json!({
            "source_class":dpi::capability(190).unwrap().transport().source_class,
            "profile_scope":"stage_1_active_0",
            "report":{"requested":change,"active":null,"verified":false,"acknowledgements":[
                {"command":[10,4,6],"transaction":0,"response_bytes":0,"data":[]}],
                "write_errors":{"stages":"strict getter differs","active":null},
                "stages":{"profile_id":1,"active_stage":1,"number_of_active_stages":1,
                    "records":[{"index":1,"x":0,"y":0,"z":0},
                        {"index":0,"x":0,"y":0,"z":0}],"response_bytes":17}}
        });
        let report = decode_reply(190, &reply, Some(change.clone())).unwrap();
        assert!(!report.verified);
        assert_eq!(report.stages.unwrap().records[0].x, 0);
        reply["report"]["verified"] = json!(true);
        reply["report"]["write_errors"] = json!({"stages":null,"active":null});
        assert!(decode_reply(190, &reply, Some(change.clone())).is_err());
        reply["report"]["verified"] = json!(false);
        reply["report"]["write_errors"]["stages"] = json!("strict getter differs");
        reply["report"]["requested"]["stages"]["stages"][0]["x"] = Value::Null;
        assert!(decode_reply(190, &reply, Some(change)).is_err());
    }

    #[test]
    fn failed_write_keeps_actual_dpi_without_accepting_a_success_claim() {
        let change = dpi::DpiChange {
            stages: None,
            active: Some(dpi::DpiPair { x: 2200, y: 2200 }),
            profile_guid: None,
            initialize: false,
        };
        let mut reply = json!({
            "source_class":dpi::capability(190).unwrap().transport().source_class,
            "profile_scope":"stage_1_active_0",
            "report":{
                "requested":change, "stages":null, "active":{"x":2300,"y":2400},
                "acknowledgements":[], "verified":false,
                "write_errors":{"active":"readback differs","stages":null}
            }
        });
        let report = decode_reply(190, &reply, Some(change.clone())).unwrap();
        assert_eq!(report.active, Some(dpi::DpiPair { x: 2300, y: 2400 }));
        assert!(!report.verified);
        reply["report"]["verified"] = json!(true);
        assert!(decode_reply(190, &reply, Some(change.clone())).is_err());
        reply["report"]["verified"] = json!(false);
        reply["report"]["write_errors"]["stages"] = json!("unrequested task");
        assert!(decode_reply(190, &reply, Some(change)).is_err());
    }

    #[test]
    fn partial_getter_reply_preserves_real_values_and_requires_explicit_errors() {
        let source = dpi::capability(190).unwrap();
        let reply = json!({"source_class":source.transport().source_class,"profile_scope":"stage_1_active_0",
            "report":{"requested":null,"stages":null,"active":{"x":2000,"y":2100},
                "acknowledgements":[],"read_errors":{"stages":"stage command rejected","active":null},"verified":false}});
        let report = decode_reply(190, &reply, None).unwrap();
        assert_eq!(report.active, Some(dpi::DpiPair { x: 2000, y: 2100 }));
        assert!(!report.verified);
        for (path, value) in [
            ("/report/verified", json!(true)),
            ("/report/read_errors/stages", json!(null)),
            ("/report/read_errors/active", json!("also failed")),
            ("/report/active", json!(null)),
        ] {
            let mut invalid = reply.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(decode_reply(190, &invalid, None).is_err());
        }
    }

    #[test]
    fn a_partial_getter_cannot_confirm_a_device_write_or_invent_obm_readback() {
        let source = dpi::capability(190).unwrap();
        let change = DpiChange {
            stages: None,
            active: Some(dpi::DpiPair { x: 2000, y: 2100 }),
            profile_guid: None,
            initialize: false,
        };
        let mut reply = json!({"source_class":source.transport().source_class,"profile_scope":"stage_1_active_0",
            "report":{"requested":change,"stages":null,"active":{"x":2000,"y":2100},
                "acknowledgements":[],"read_errors":{"stages":"failed","active":null},"verified":false}});
        assert!(decode_reply(190, &reply, Some(change.clone())).is_err());
        reply["report"]["verified"] = json!(true);
        assert!(decode_reply(190, &reply, Some(change)).is_err());
    }

    #[test]
    fn identity_reply_requires_a_real_serial_without_claiming_settings_were_written() {
        let change = DpiChange {
            stages: None,
            active: None,
            profile_guid: None,
            initialize: true,
        };
        for product_id in [182, 190] {
            let source = dpi::capability(product_id).unwrap();
            let reply = json!({"source_class":source.transport().source_class,"profile_scope":"stage_1_active_0",
            "report":{"requested":change,"serial_number":"SOURCE-SERIAL","stages":null,"active":null,"acknowledgements":[],"verified":true}});
            assert!(decode_reply(product_id, &reply, Some(change.clone())).is_ok());
            for serial in [
                Value::Null,
                json!(""),
                json!("bad\u{0}serial"),
                json!("未读取"),
            ] {
                let mut missing = reply.clone();
                missing["report"]["serial_number"] = serial;
                assert!(decode_reply(product_id, &missing, Some(change.clone())).is_err());
            }
            let mut unrelated = reply;
            unrelated["report"]["active"] = json!({"x":800,"y":800});
            assert!(decode_reply(product_id, &unrelated, Some(change.clone())).is_err());
        }
    }
}
