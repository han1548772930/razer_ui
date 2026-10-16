//! Simulated combined local RPC/HID chains; no OS, DLL, device or real socket.
use super::*;
use razer_device::{
    backend::{FeatureTransport, HidNode, ReportLengths},
    thx_blackshark_firmware::default_blackshark_local_eq_presets,
    thx_submission::{ThxSubmissionPlan, ThxSubmissionScope},
    thx_v4::{SpatialStatus, ThxWriteAcknowledgement},
    thx_v4_proto::{Property, PropertyType},
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

#[derive(Default)]
struct State {
    events: Vec<String>,
    outputs: Vec<Vec<u8>>,
    inputs: VecDeque<Vec<u8>>,
    firmware_status: u8,
    cancel_on_read: bool,
    read: bool,
    roller_index: u8,
    versions: [u8; 24],
}
struct Hid(Rc<RefCell<State>>);
impl FeatureTransport for Hid {
    fn send_feature(&self, _: &[u8]) -> Result<()> {
        panic!("Wrong HID channel")
    }
    fn get_feature(&self, _: &mut [u8]) -> Result<usize> {
        panic!("Wrong HID channel")
    }
    fn write_output(&self, report: &[u8]) -> Result<()> {
        let mut state = self.0.borrow_mut();
        state
            .events
            .push(format!("hid:{:02x}:{}", report[10], report[13]));
        state.outputs.push(report.to_vec());
        let command = match report[10] {
            2 => 2,
            0x9e => 0x9e,
            0xe1 => 0xe1,
            0x95 => 0x95,
            0xe0 => 0xe0,
            0x6a => 0x6a,
            0xea => {
                state.roller_index = report[13];
                0xea
            }
            _ => panic!("Unexpected command"),
        };
        // Independently constructed synthetic device success/failure envelope.
        let mut response = vec![0; 64];
        response[0] = 2;
        response[1] = if command == 0x9e { state.firmware_status } else { 2 };
        response[2] = 0x60;
        response[10] = command;
        response[11] = 1;
        response[12] = 1;
        if command == 2 {
            response[12] = 24;
            response[13..37].copy_from_slice(&state.versions);
        }
        if command == 0x6a {
            response[13] = state.roller_index;
        }
        state.inputs.push_back(response);
        Ok(())
    }
    fn read_interrupt(&self, out: &mut [u8], _: u32) -> Result<usize> {
        let mut state = self.0.borrow_mut();
        state.read = true;
        let response = state.inputs.pop_front().expect("Unscripted input");
        out[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }
    fn report_lengths(&self) -> Result<ReportLengths> {
        Ok(ReportLengths {
            input: [(2, 64)].into(),
            output: [(2, 64)].into(),
            feature: Default::default(),
        })
    }
    fn metadata(&self) -> Value {
        json!({"kind":"simulated"})
    }
}
struct Rpc {
    state: Rc<RefCell<State>>,
    responses: VecDeque<Result<Response>>,
    requests: Vec<Request>,
}
impl ThxLocalRpc for Rpc {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Simulated
    }
    fn request(&mut self, identity: &ThxDeviceIdentity, request: Request) -> Result<Response> {
        assert_eq!(identity.product_id, "razer_blacksharkv3pro_pc");
        assert_eq!(identity.endpoint_id, "headphones_game");
        let name = match &request {
            Request::GetVersion => "version",
            Request::GetProperty { .. } => "get",
            Request::SetProperty {
                property: Some(property),
                endpoint_id,
            } => {
                assert_eq!(endpoint_id.as_deref(), Some("headphones_game"));
                self.state.borrow_mut().events.push(format!(
                    "rpc:{}:{}",
                    property.id,
                    property.bool_value.unwrap()
                ));
                "set"
            }
            _ => panic!("Unexpected RPC"),
        };
        if name != "set" {
            self.state.borrow_mut().events.push(format!("rpc:{name}"));
        }
        self.requests.push(request);
        self.responses.pop_front().expect("Unscripted RPC")
    }
}
fn property(enabled: bool) -> Response {
    Response::GetProperty(Property {
        id: "spatial_enabled".into(),
        property_type: PropertyType::Bool,
        bool_value: Some(enabled),
        ..Default::default()
    })
}
fn scope() -> ThxSubmissionScope {
    ThxSubmissionScope {
        product_id: 1398,
        container_id: "mock-container".into(),
        serial_number: "mock-serial".into(),
        profile_id: "local-profile".into(),
        generation: 7,
    }
}
fn presets() -> Vec<Value> {
    default_blackshark_local_eq_presets(1398)
        .unwrap()
        .iter()
        .map(|preset| serde_json::to_value(preset).unwrap())
        .collect()
}
fn transport(responses: Vec<Result<Response>>) -> (DirectThxTransport<Rpc>, Rc<RefCell<State>>) {
    transport_for(1398, 1398, responses)
}
fn transport_for(
    owner: u32,
    connected_pid: u16,
    responses: Vec<Result<Response>>,
) -> (DirectThxTransport<Rpc>, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State {
        firmware_status: 2,
        ..Default::default()
    }));
    let node = HidNode {
        path: b"memory-hid".to_vec(),
        vendor_id: 0x1532,
        product_id: connected_pid,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 5,
        release_number: 0,
        serial_number: Some("mock-serial".into()),
        manufacturer: None,
        product: None,
    };
    let identity = BlackSharkFirmwareIdentity::new(owner, node, "mock-container", 7).unwrap();
    let firmware = BlackSharkFirmwareSession::new(identity, Box::new(Hid(state.clone()))).unwrap();
    let checked = state.clone();
    let rpc = Rpc {
        state: state.clone(),
        responses: responses.into(),
        requests: vec![],
    };
    let transport = DirectThxTransport::new(
        rpc,
        firmware,
        Box::new(move |identity| {
            assert_eq!(identity.generation, 7);
            let state = checked.borrow();
            ensure!(
                !(state.cancel_on_read && state.read),
                "simulated canceled after actual input"
            );
            Ok(())
        }),
        Box::new(|_| Ok(())),
    )
    .unwrap();
    (transport, state)
}
fn fixture(initial: bool, final_value: bool) -> Vec<Result<Response>> {
    vec![
        Ok(Response::GetVersion {
            version: String::new(),
        }),
        Ok(property(initial)),
        Ok(Response::SetProperty),
        Ok(Response::SetProperty),
        Ok(property(final_value)),
    ]
}

fn render_setup_fixture() -> Vec<Result<Response>> {
    vec![
        Ok(Response::GetVersion {
            version: "mock-version".into(),
        }),
        Ok(Response::SetProperty),
        Ok(Response::SetProperty),
        Ok(Response::SetProperty),
        Ok(Response::SetProperty),
    ]
}

#[test]
fn roller_get_and_set_use_retained_hid_without_any_thx_rpc_or_binding() {
    let (transport, state) = transport(vec![]);
    state.borrow_mut().roller_index = 2;
    let mut controller = razer_device::thx_v4::ThxV4Controller::new(1398, transport).unwrap();
    let read = crate::thx::execute_retained(
        scope(),
        ThxSubmissionPlan::ReadRollerMapping,
        &mut controller,
        || Ok(()),
        |_| Ok(()),
    );
    assert_eq!(
        read.roller_mapping.as_ref().unwrap().observed_index,
        Some(2)
    );
    assert!(!controller.transport().bound());
    let set = crate::thx::execute_retained(
        scope(),
        ThxSubmissionPlan::SetRollerMappings {
            mappings: vec![json!({"outputType":"disableGroup"})],
        },
        &mut controller,
        || Ok(()),
        |_| Ok(()),
    );
    assert_eq!(
        set.roller_mapping.as_ref().unwrap().source_result,
        Some(true)
    );
    assert_eq!(state.borrow().events, ["hid:6a:0", "hid:ea:47"]);
    assert_eq!(state.borrow().roller_index, 47);
    assert_eq!(set.firmware.len(), 1);
    assert!(controller.transport().rpc().requests.is_empty());
}

#[test]
fn roller_late_cancel_retains_real_ack_without_claiming_current_completion() {
    let (transport, state) = transport(vec![]);
    state.borrow_mut().cancel_on_read = true;
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetRollerMappings { mappings: vec![] },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    let mapping = result.roller_mapping.unwrap();
    assert!(mapping.source_result.is_none());
    assert!(mapping.commands[0].acknowledged());
    assert!(!mapping.commands[0].succeeded());
    assert_eq!(state.borrow().events, ["hid:ea:0"]);
}

#[test]
fn thx_render_setup_is_explicit_and_has_no_getter_hid_or_spatial_write() {
    let (transport, state) = transport(render_setup_fixture());
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetupRender {
            trigger: razer_device::thx_v4::ThxRenderSetupTrigger::InitialDeviceSetup,
        },
        transport,
        || Ok(()),
        |_| unreachable!(),
    );
    assert!(result.error.is_none() && result.cleanup_error.is_none());
    assert!(result.outcome.is_none() && result.initial_observation.is_none());
    assert!(result.firmware.is_empty());
    let setup = result.render_setup.unwrap();
    assert!(setup.setup_succeeded);
    assert_eq!(setup.receipts.len(), 6);
    assert_eq!(setup.receipts[0].reply, Some(ThxReply::LocalBinding(true)));
    assert_eq!(setup.receipts[1].reply, Some(ThxReply::LocalReady(true)));
    for receipt in &setup.receipts[2..] {
        assert_eq!(receipt.reply, Some(ThxReply::RpcAcknowledged));
    }
    assert_eq!(
        state.borrow().events,
        vec![
            "rpc:version",
            "rpc:upmix_enabled:true",
            "rpc:eq_enabled:true",
            "rpc:spatial_advanced_early_reflections_enabled:true",
            "rpc:spatial_advanced_late_reverb_enabled:true",
        ]
    );
    assert!(state.borrow().outputs.is_empty());
}

#[test]
fn thx_render_setup_status_rejection_continues_and_retains_actual_code() {
    let mut responses = render_setup_fixture();
    responses[1] = Err(anyhow::Error::new(CodecError::GrpcStatus {
        code: 7,
        message: Some("mock permission denied".into()),
    })
    .context("complete received final status"));
    let (transport, state) = transport(responses);
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetupRender {
            trigger: razer_device::thx_v4::ThxRenderSetupTrigger::InitialDeviceSetup,
        },
        transport,
        || Ok(()),
        |_| unreachable!(),
    );
    assert!(result.error.is_none() && result.cleanup_error.is_none());
    let setup = result.render_setup.unwrap();
    assert!(setup.setup_succeeded);
    assert_eq!(setup.warnings.len(), 1);
    assert_eq!(
        setup.receipts[2].reply,
        Some(ThxReply::RpcRejected {
            code: 7,
            message: Some("mock permission denied".into()),
        })
    );
    assert_eq!(state.borrow().events.len(), 5);
    assert!(state.borrow().outputs.is_empty());
}

#[test]
fn thx_render_setup_incomplete_or_uncertain_rpc_stops_and_still_cleans_binding() {
    for error in [
        anyhow::Error::new(CodecError::Truncated),
        anyhow::Error::new(CodecError::MissingTrailers),
        anyhow::Error::new(CodecError::InvalidGrpcStatus),
        anyhow::anyhow!("mock deadline exceeded with unknown outcome"),
    ] {
        let mut responses = render_setup_fixture();
        responses[2] = Err(error);
        let (transport, state) = transport(responses);
        let result = crate::thx::execute(
            scope(),
            ThxSubmissionPlan::SetupRender {
                trigger: razer_device::thx_v4::ThxRenderSetupTrigger::InitialDeviceSetup,
            },
            transport,
            || Ok(()),
            |_| unreachable!(),
        );
        assert!(result.error.is_some() && result.cleanup_error.is_none());
        let setup = result.render_setup.unwrap();
        assert!(!setup.setup_succeeded);
        assert_eq!(
            setup.status,
            razer_device::thx_v4::ThxRenderSetupStatus::TransportFailed
        );
        assert_eq!(setup.receipts.len(), 4);
        assert_eq!(setup.receipts[2].reply, Some(ThxReply::RpcAcknowledged));
        assert!(setup.receipts[3].reply.is_none() && setup.receipts[3].error.is_some());
        assert_eq!(state.borrow().events.len(), 3);
        assert!(state.borrow().outputs.is_empty());
    }
}

#[test]
fn thx_render_setup_force_rebind_logs_absent_owned_binding_without_faking_native_remove() {
    let (transport, _) = transport(render_setup_fixture());
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetupRender {
            trigger: razer_device::thx_v4::ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged,
        },
        transport,
        || Ok(()),
        |_| unreachable!(),
    );
    assert!(result.error.is_none() && result.cleanup_error.is_none());
    let setup = result.render_setup.unwrap();
    assert!(setup.setup_succeeded);
    assert_eq!(setup.receipts.len(), 7);
    assert_eq!(setup.receipts[0].operation, ThxOperation::RemoveDevice);
    assert_eq!(setup.receipts[0].reply, Some(ThxReply::LocalBinding(false)));
    assert_eq!(setup.warnings.len(), 1);
}

#[test]
fn thx_render_setup_cancel_after_rpc_ack_retains_partial_receipt_and_releases_binding() {
    let (transport, state) = transport(render_setup_fixture());
    let observed = state.clone();
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetupRender {
            trigger: razer_device::thx_v4::ThxRenderSetupTrigger::InitialDeviceSetup,
        },
        transport,
        move || {
            ensure!(
                observed.borrow().events.len() < 2,
                "mock canceled after upmix RPC ACK"
            );
            Ok(())
        },
        |_| unreachable!(),
    );
    assert!(result.error.is_some());
    assert!(result.cleanup_error.is_none());
    let setup = result.render_setup.unwrap();
    assert_eq!(
        setup.status,
        razer_device::thx_v4::ThxRenderSetupStatus::Canceled
    );
    assert!(!setup.setup_succeeded);
    assert_eq!(setup.receipts.len(), 3);
    assert_eq!(setup.receipts[2].reply, Some(ThxReply::RpcAcknowledged));
    assert_eq!(
        state.borrow().events,
        vec!["rpc:version", "rpc:upmix_enabled:true"]
    );
}

#[test]
fn thx_render_setup_two_restore_callbacks_reapply_four_settings_on_same_binding() {
    let mut responses = render_setup_fixture();
    responses.extend(render_setup_fixture());
    let (transport, state) = transport(responses);
    let mut controller = razer_device::thx_v4::ThxV4Controller::new(1398, transport).unwrap();
    for _ in 0..2 {
        let setup = controller.setup_render(
            razer_device::thx_v4::ThxRenderSetupTrigger::PostRestoreDeviceData,
            || Ok(()),
            |_| unreachable!(),
        );
        assert!(setup.setup_succeeded);
        assert_eq!(setup.receipts.len(), 6);
    }
    assert!(controller.transport().bound());
    assert_eq!(state.borrow().events.len(), 10);
    assert_eq!(
        state
            .borrow()
            .events
            .iter()
            .filter(|event| *event == "rpc:upmix_enabled:true")
            .count(),
        2
    );
    controller.remove().unwrap();
    assert!(!controller.transport().bound());
}

#[test]
fn thx_render_setup_identity_expiry_after_received_rpc_ack_keeps_ack_and_stops() {
    let (mut transport, state) = transport(render_setup_fixture());
    let live = state.clone();
    transport.validate = Box::new(move |_| {
        ensure!(
            live.borrow().events.len() < 2,
            "mock retained HID identity expired after RPC response"
        );
        Ok(())
    });
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetupRender {
            trigger: razer_device::thx_v4::ThxRenderSetupTrigger::InitialDeviceSetup,
        },
        transport,
        || Ok(()),
        |_| unreachable!(),
    );
    assert!(result.error.is_some() && result.cleanup_error.is_none());
    let setup = result.render_setup.unwrap();
    assert!(!setup.setup_succeeded);
    assert_eq!(setup.receipts.len(), 3);
    assert_eq!(setup.receipts[2].reply, Some(ThxReply::RpcAcknowledged));
    assert!(
        setup.receipts[2]
            .error
            .as_ref()
            .unwrap()
            .contains("identity expired")
    );
    assert_eq!(
        state.borrow().events,
        vec!["rpc:version", "rpc:upmix_enabled:true"]
    );
}
#[test]
fn thx_enable_combines_source_order_with_actual_readback_and_rpc_ack_origin() {
    let (transport, state) = transport(fixture(false, true));
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    let outcome = result.outcome.unwrap();
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(
        outcome.write_acknowledgement,
        Some(ThxWriteAcknowledgement::LocalGrpcStatus)
    );
    assert!(!outcome.native_acknowledged);
    assert_eq!(result.cleanup_error, None);
    assert_eq!(result.firmware.len(), 2);
    assert_eq!(
        state.borrow().events,
        vec![
            "rpc:version",
            "rpc:get",
            "hid:9e:0",
            "rpc:eq_enabled:true",
            "hid:e1:1",
            "hid:95:0",
            "hid:e0:0",
            "hid:e1:2",
            "rpc:spatial_enabled:true",
            "rpc:get"
        ]
    );
    assert_eq!(state.borrow().outputs.len(), 5);
}
#[test]
fn thx_disable_restores_all_nine_local_presets_in_original_order() {
    let (transport, state) = transport(fixture(true, false));
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: false,
            presets: presets(),
            timer_tick: 9,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    assert_eq!(result.outcome.unwrap().status, SpatialStatus::Verified);
    let state = state.borrow();
    assert_eq!(&state.events[2..4], ["rpc:eq_enabled:false", "hid:9e:1"]);
    assert_eq!(state.outputs.len(), 21);
    let indices = state
        .outputs
        .iter()
        .filter(|out| out[10] == 0x95)
        .map(|out| out[13])
        .collect::<Vec<_>>();
    assert_eq!(indices, (0..9).collect::<Vec<_>>());
}
#[test]
fn thx_same_state_skips_restore_but_still_submits_eq_and_spatial() {
    let (transport, state) = transport(fixture(true, true));
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    assert_eq!(result.outcome.unwrap().status, SpatialStatus::Verified);
    assert_eq!(state.borrow().outputs.len(), 1);
    assert_eq!(result.firmware.len(), 1);
}
#[test]
fn thx_read_is_observation_with_no_hid_or_write_ack() {
    let (transport, state) = transport(vec![
        Ok(Response::GetVersion {
            version: String::new(),
        }),
        Ok(property(false)),
    ]);
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::ReadSpatial,
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    let outcome = result.outcome.unwrap();
    assert_eq!(outcome.status, SpatialStatus::Observed);
    assert_eq!(outcome.readback, Some(false));
    assert_eq!(outcome.write_acknowledgement, None);
    assert!(state.borrow().outputs.is_empty());
    assert!(result.render_setup.is_none());
    assert_eq!(state.borrow().events, vec!["rpc:version", "rpc:get"]);
}
#[test]
fn thx_missing_getter_presence_stops_before_any_settings_are_sent() {
    let missing = Response::GetProperty(Property {
        id: "spatial_enabled".into(),
        property_type: PropertyType::Bool,
        ..Default::default()
    });
    let (transport, state) = transport(vec![
        Ok(Response::GetVersion {
            version: "1".into(),
        }),
        Ok(missing),
    ]);
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    assert!(result.error.unwrap().contains("pre-write"));
    assert!(result.outcome.is_none());
    assert!(state.borrow().outputs.is_empty());
}
#[test]
fn thx_spatial_rpc_failure_retains_firmware_and_eq_progress_without_retry() {
    let mut responses = fixture(false, true);
    responses[3] = Err(anyhow::anyhow!("uncertain setter transport failure"));
    let (transport, state) = transport(responses);
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    let outcome = result.outcome.unwrap();
    assert_eq!(outcome.status, SpatialStatus::TransportFailed);
    assert_eq!(outcome.write_acknowledgement, None);
    assert_eq!(result.firmware.len(), 2);
    assert_eq!(
        state
            .borrow()
            .events
            .iter()
            .filter(|event| *event == "rpc:spatial_enabled:true")
            .count(),
        1
    );
}

#[test]
fn thx_spatial_completed_rpc_rejection_is_not_native_bool_or_write_ack() {
    let mut responses = fixture(false, true);
    responses[2] = Err(anyhow::Error::new(CodecError::GrpcStatus {
        code: 14,
        message: Some("mock service rejected EQ".into()),
    }));
    let (transport, state) = transport(responses);
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    assert!(result.cleanup_error.is_none());
    let outcome = result.outcome.unwrap();
    assert_eq!(outcome.status, SpatialStatus::RpcRejected);
    assert_eq!(outcome.write_acknowledgement, None);
    assert!(!outcome.native_acknowledged);
    assert!(outcome.error.unwrap().contains("14"));
    assert_eq!(result.firmware.len(), 1);
    assert_eq!(
        state.borrow().events,
        vec!["rpc:version", "rpc:get", "hid:9e:0", "rpc:eq_enabled:true"]
    );
    assert!(
        state
            .borrow()
            .events
            .iter()
            .all(|event| !event.starts_with("rpc:spatial_enabled"))
    );
}
#[test]
fn thx_readback_mismatch_retains_rpc_ack_and_actual_false() {
    let (transport, _) = transport(fixture(false, false));
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    let outcome = result.outcome.unwrap();
    assert_eq!(outcome.status, SpatialStatus::ReadbackMismatch);
    assert_eq!(outcome.readback, Some(false));
    assert_eq!(
        outcome.write_acknowledgement,
        Some(ThxWriteAcknowledgement::LocalGrpcStatus)
    );
}
#[test]
fn thx_firmware_rejects_three_times_then_source_continues_with_warning() {
    let (transport, state) = transport(fixture(false, true));
    state.borrow_mut().firmware_status = 3;
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    let outcome = result.outcome.unwrap();
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(outcome.warnings.len(), 1);
    let ThxFirmwareProgress::Command(command) = &result.firmware[0] else {
        panic!()
    };
    assert!(!command.succeeded());
    assert_eq!(command.attempts.len(), 3);
}
#[test]
fn thx_late_cancel_preserves_physical_hid_ack_and_prevents_rpc_setters() {
    let (transport, state) = transport(fixture(false, true));
    state.borrow_mut().cancel_on_read = true;
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || Ok(()),
        |_| Ok(()),
    );
    assert_ne!(result.outcome.unwrap().status, SpatialStatus::Verified);
    let ThxFirmwareProgress::Command(command) = &result.firmware[0] else {
        panic!()
    };
    assert!(command.acknowledged());
    assert!(!command.succeeded());
    assert_eq!(
        state.borrow().events,
        vec!["rpc:version", "rpc:get", "hid:9e:0"]
    );
}
#[test]
fn thx_binding_is_real_owned_state_and_removal_needs_no_device_ack() {
    let (mut transport, _) = transport(vec![]);
    let id = ThxDeviceIdentity::new("razer_blacksharkv3pro_pc", ENDPOINT_ID).unwrap();
    assert!(!transport.bound());
    assert_eq!(
        transport.exchange(&id, &ThxOperation::AddDevice).unwrap(),
        ThxReply::LocalBinding(true)
    );
    assert!(transport.bound());
    assert_eq!(
        transport.exchange(&id, &ThxOperation::AddDevice).unwrap(),
        ThxReply::LocalBinding(true)
    );
    assert_eq!(
        transport
            .exchange(&id, &ThxOperation::RemoveDevice)
            .unwrap(),
        ThxReply::LocalBinding(true)
    );
    assert!(!transport.bound());
    assert!(
        transport
            .exchange(&id, &ThxOperation::GetProperty("spatial_enabled".into()))
            .is_err()
    );
}
#[test]
fn thx_identity_and_typed_observation_reject_foreign_routes_and_values() {
    let (mut transport, state) = transport(vec![]);
    let foreign = ThxDeviceIdentity::new("razer_blacksharkv3_pc", ENDPOINT_ID).unwrap();
    assert!(
        transport
            .exchange(&foreign, &ThxOperation::AddDevice)
            .is_err()
    );
    assert!(state.borrow().events.is_empty());
    let mut property = Property {
        id: "other".into(),
        property_type: PropertyType::Bool,
        bool_value: Some(true),
        ..Default::default()
    };
    assert!(DirectThxTransport::<Rpc>::value("spatial_enabled", &property).is_err());
    property.id = "spatial_enabled".into();
    property.property_type = PropertyType::Int;
    assert!(DirectThxTransport::<Rpc>::value("spatial_enabled", &property).is_err());
    property.id = "arbitrary_bool".into();
    property.property_type = PropertyType::Bool;
    property.bool_value = None;
    assert_eq!(
        DirectThxTransport::<Rpc>::value("arbitrary_bool", &property).unwrap(),
        json!(false)
    );
}
#[test]
fn thx_cancellation_after_initial_read_stops_all_writes_and_runs_cleanup() {
    let (transport, state) = transport(fixture(false, true));
    let count = Cell::new(0);
    let result = crate::thx::execute(
        scope(),
        ThxSubmissionPlan::SetSpatial {
            enabled: true,
            presets: presets(),
            timer_tick: 8,
        },
        transport,
        || {
            count.set(count.get() + 1);
            ensure!(count.get() < 5, "task canceled");
            Ok(())
        },
        |_| Ok(()),
    );
    assert!(
        result.error.is_some()
            || result
                .outcome
                .as_ref()
                .is_some_and(|outcome| outcome.status == SpatialStatus::Canceled)
    );
    assert_eq!(result.cleanup_error, None);
    assert!(state.borrow().outputs.is_empty());
}

#[test]
fn firmware_version_reads_six_owners_and_both_audited_connections_without_thx_rpc() {
    for (owner, dongle) in [
        (1398, 1399),
        (1401, 1402),
        (2638, 2645),
        (2641, 2648),
        (4124, 4123),
        (4126, 4126),
    ] {
        let mut connections = vec![dongle];
        if owner != 4126 {
            connections.push(owner as u16);
        }
        for connected in connections {
            let (transport, state) = transport_for(owner, connected, vec![]);
            state.borrow_mut().versions = [
                1, 9, 0, 0, 1, 10, 0, 0, 1, 2, 255, 255, 0, 0, 0, 0, 1, 10, 0, 1, 1, 10, 0, 0,
            ];
            let mut scope = scope();
            scope.product_id = owner;
            let mut controller =
                razer_device::thx_v4::ThxV4Controller::new(owner, transport).unwrap();
            // Consume the constructor's first-command flag before the version query.
            let _ = crate::thx::execute_retained(
                scope.clone(),
                ThxSubmissionPlan::ReadRollerMapping,
                &mut controller,
                || Ok(()),
                |_| Ok(()),
            );
            let read = crate::thx::execute_retained(
                scope,
                ThxSubmissionPlan::ReadFirmwareVersion,
                &mut controller,
                || Ok(()),
                |_| Ok(()),
            );
            assert!(read.error.is_none(), "{:?}", read.error);
            assert_eq!(
                read.observed_firmware_version().as_deref(),
                Some("1.10.0.1")
            );
            assert_eq!(read.firmware.len(), 1);
            assert!(controller.transport().rpc().requests.is_empty());
            let state = state.borrow();
            assert_eq!(
                state.outputs[1][9], 0,
                "Explicit dongle firmware query must not address its peer"
            );
            assert_eq!(&state.outputs[1][10..13], &[2, 0, 0]);
            assert_eq!(state.events, ["hid:6a:0", "hid:02:0"]);
        }
    }
}

#[test]
fn firmware_zero_fallback_and_late_cancellation_are_not_observed_versions() {
    for canceled in [false, true] {
        let (transport, state) = transport(vec![]);
        if canceled {
            state.borrow_mut().versions[0] = 2;
            state.borrow_mut().cancel_on_read = true;
        }
        let read = crate::thx::execute(
            scope(),
            ThxSubmissionPlan::ReadFirmwareVersion,
            transport,
            || Ok(()),
            |_| Ok(()),
        );
        assert!(read.error.is_some());
        assert!(read.observed_firmware_version().is_none());
        let ThxFirmwareProgress::Command(command) = &read.firmware[0] else {
            panic!("Missing raw receipt");
        };
        assert!(command.acknowledged());
        assert_eq!(command.succeeded(), !canceled);
    }
}
