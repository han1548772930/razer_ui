use super::recorder::InputRecorder;
use super::*;
use razer_platform::macro_recorder::{Events, Mode};
use std::collections::BTreeSet;

#[derive(Default)]
struct Input {
    mode: Option<Mode>,
    prepared: bool,
    terminal_shutdown_failure: bool,
}
impl InputRecorder for Input {
    fn prepare(&mut self) -> Result<()> {
        self.prepared = true;
        Ok(())
    }
    fn start(&mut self, mode: Mode) -> Result<()> {
        ensure!(self.prepared && self.mode.is_none(), "invalid start");
        self.mode = Some(mode);
        Ok(())
    }
    fn mode(&self) -> Result<Option<Mode>> {
        Ok(self.mode)
    }
    fn stop(&mut self) -> Result<()> {
        self.mode = None;
        Ok(())
    }
    fn events(&self) -> Result<Events> {
        Ok(Events {
            events: Vec::new(),
            overflow: false,
        })
    }
    fn shutdown(&mut self) -> Result<()> {
        self.prepared = false;
        ensure!(
            !self.terminal_shutdown_failure,
            "Recorder hook cleanup failed after release"
        );
        Ok(())
    }
    fn cleanup_pending(&self) -> bool {
        self.prepared
    }
}
#[derive(Default)]
struct Controls {
    calls: Vec<String>,
    failures: BTreeSet<String>,
    rejected: BTreeSet<String>,
}
impl Controls {
    fn call(&mut self, name: String) -> Result<ActionReply> {
        self.calls.push(name.clone());
        ensure!(!self.failures.contains(&name), "transport {name}");
        Ok(ActionReply::new(
            !self.rejected.contains(&name),
            format!("source {name}"),
        ))
    }
}
impl Host for Controls {
    fn set_mappings(&mut self, _: Uuid, enabled: bool) -> Result<ActionReply> {
        self.call(format!("mapping:{enabled}"))
    }
    fn set_shortcuts(&mut self, _: Uuid, enabled: bool) -> Result<ActionReply> {
        self.call(format!("shortcuts:{enabled}"))
    }
    fn add_target(&mut self, _: Uuid, _: &Target) -> Result<()> {
        self.call("add".into()).and_then(confirmed)
    }
    fn redirect(&mut self, _: Uuid, _: &Target, kind: RedirectKind, enabled: bool) -> Result<()> {
        self.call(format!("{kind:?}:{enabled}")).and_then(confirmed)
    }
    fn remove_target(&mut self, _: Uuid, _: &Target) -> Result<()> {
        self.call("remove".into()).and_then(confirmed)
    }
    fn subscribe_modes(&mut self, _: Uuid, enabled: bool) -> Result<()> {
        self.call(format!("modes:{enabled}")).and_then(confirmed)
    }
    fn global_mode(&mut self) -> Result<Value> {
        Ok(json!({"mode":"hypershift"}))
    }
    fn publish_storage(&mut self, _: Uuid, key: &str, _: &Value) -> Result<()> {
        self.call(format!("storage:{key}")).and_then(confirmed)
    }
}
fn target() -> Target {
    Target {
        vendor_id: 5426,
        product_id: 740,
        container_id: "{11111111-2222-3333-4444-555555555555}".into(),
        usb_instance_id: Some("USB\\observed".into()),
        guid: Uuid::new_v4(),
        keyboard_only: false,
    }
}
fn selection(target: Target) -> targets::Selection {
    targets::Selection {
        targets: vec![target],
        omitted: Vec::new(),
    }
}
fn request(
    state: &mut State<Input>,
    scope: Uuid,
    request: ServiceRequest,
    host: &mut Controls,
) -> Result<Value> {
    state.handle(scope, &request, host).unwrap()
}

#[test]
fn recorder_terminal_cleanup_failure_releases_scope_for_the_next_recorder() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    request(
        &mut state,
        scope,
        ServiceRequest::PrepareOtfmRecorder,
        &mut host,
    )
    .unwrap();
    state
        .recorder
        .as_mut()
        .unwrap()
        .input_mut()
        .terminal_shutdown_failure = true;
    assert!(state.release(scope, &mut host).is_err());
    assert!(!state.cleanup_pending(scope));
    assert!(state.recorder.is_none());
    request(
        &mut state,
        Uuid::new_v4(),
        ServiceRequest::PrepareOtfmRecorder,
        &mut host,
    )
    .unwrap();
}

#[test]
fn capture_registers_real_owner_before_redirect_and_restores_before_release() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    state
        .start(scope, Kind::Keys, selection(target()), &mut host)
        .unwrap();
    assert_eq!(
        host.calls,
        ["mapping:false", "add", "Mouse:true", "Keyboard:true"]
    );
    state.release(Uuid::new_v4(), &mut host).unwrap();
    assert_eq!(host.calls.len(), 4);
    state.release(scope, &mut host).unwrap();
    assert_eq!(
        &host.calls[4..],
        ["mapping:true", "Keyboard:false", "Mouse:false", "remove"]
    );
    assert!(state.capture.is_none());
}

#[test]
fn failed_capture_acquisition_and_cleanup_keep_only_unreleased_resources() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    host.failures
        .extend(["Keyboard:true", "remove", "mapping:true"].map(String::from));
    assert!(
        state
            .start(scope, Kind::Keys, selection(target()), &mut host)
            .is_err()
    );
    assert_eq!(
        host.calls,
        [
            "mapping:false",
            "add",
            "Mouse:true",
            "Keyboard:true",
            "mapping:true",
            "Keyboard:false",
            "Mouse:false",
            "remove"
        ]
    );
    assert!(state.capture.as_ref().unwrap().suspended);
    assert_eq!(state.capture.as_ref().unwrap().registrations.len(), 1);
    assert!(
        state
            .start(
                Uuid::new_v4(),
                Kind::Mode,
                targets::Selection::default(),
                &mut host
            )
            .is_err()
    );
    host.failures.clear();
    host.calls.clear();
    state.release(scope, &mut host).unwrap();
    assert_eq!(host.calls, ["mapping:true", "remove"]);
    assert!(state.capture.is_none());
}

#[test]
fn mixed_capture_preserves_actor_order_and_rejects_retired_scope_events() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    let device = target();
    let guid = device.guid;
    state
        .start(scope, Kind::Global, selection(device), &mut host)
        .unwrap();
    state.input(scope, guid, json!({"scancode":29,"flag":0}), 900);
    state.mode(json!({"event":"{\"isEnabled\":true}","timeTick":800}));
    state.input(Uuid::new_v4(), guid, json!({"scancode":7}), 1000);
    state.input(scope, guid, json!({"scancode":29,"flag":1}), 1);
    let batch: razer_ipc::global_shortcut_capture::Batch =
        serde_json::from_value(state.events(scope, Kind::Global).unwrap()).unwrap();
    use razer_ipc::global_shortcut_capture::Event as Wire;
    let events = batch.into_events();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0],Wire::Input(value) if value["timeTick"]==900));
    assert_eq!(events[1], Wire::Mode(true));
    assert!(matches!(&events[2],Wire::Input(value) if value["timeTick"]==1));
    state.release(scope, &mut host).unwrap();
    let successor = Uuid::new_v4();
    state
        .start(successor, Kind::Keys, selection(target()), &mut host)
        .unwrap();
    state.input(scope, guid, json!({"scancode":99}), 3);
    assert_eq!(
        state.events(successor, Kind::Keys).unwrap(),
        json!({"events":[]})
    );
}

#[test]
fn malformed_modes_and_input_overflow_are_errors_without_silent_partial_drains() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    let device = target();
    let guid = device.guid;
    state
        .start(scope, Kind::Global, selection(device), &mut host)
        .unwrap();
    state.input(scope, guid, json!({"scancode":29}), 1);
    state.mode(json!({"event":"{}"}));
    assert!(state.events(scope, Kind::Global).is_err());
    assert_eq!(state.capture.as_ref().unwrap().events.len(), 2);
    for _ in 0..512 {
        state.input(scope, guid, json!({"scancode":29}), 1);
    }
    assert!(
        state
            .events(scope, Kind::Global)
            .unwrap_err()
            .to_string()
            .contains("overflow")
    );
    state.release(scope, &mut host).unwrap();
}

#[test]
fn recorder_restoration_attempts_both_controls_after_a_transport_failure() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    request(
        &mut state,
        scope,
        ServiceRequest::StartMacroRecording,
        &mut host,
    )
    .unwrap();
    request(
        &mut state,
        scope,
        ServiceRequest::SuspendMacroMappings,
        &mut host,
    )
    .unwrap();
    host.calls.clear();
    host.failures.insert("mapping:true".into());
    assert!(
        request(
            &mut state,
            scope,
            ServiceRequest::ResumeMacroMappings,
            &mut host
        )
        .is_err()
    );
    assert_eq!(host.calls, ["mapping:true", "shortcuts:true"]);
    assert!(state.release(scope, &mut host).is_err());
    assert!(state.recorder.is_some());
    host.failures.clear();
    state.release(scope, &mut host).unwrap();
    assert!(state.recorder.is_none());
}

#[test]
fn source_false_restoration_is_response_data_but_does_not_release_suppression_ownership() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    request(
        &mut state,
        scope,
        ServiceRequest::PrepareOtfmRecorder,
        &mut host,
    )
    .unwrap();
    request(
        &mut state,
        scope,
        ServiceRequest::SetOtfmMappingsSuspended { suspended: true },
        &mut host,
    )
    .unwrap();
    request(
        &mut state,
        scope,
        ServiceRequest::StartOtfmRecording,
        &mut host,
    )
    .unwrap();
    let global = request(
        &mut state,
        scope,
        ServiceRequest::MacroRecorderGlobalState,
        &mut host,
    )
    .unwrap();
    assert_eq!(
        global,
        json!({"macroRecordingMode":"kOTFM","globalMode":{"mode":"hypershift"}})
    );
    host.rejected.insert("mapping:true".into());
    let receipt = request(
        &mut state,
        scope,
        ServiceRequest::ResumeMacroMappings,
        &mut host,
    )
    .unwrap();
    assert_eq!(
        receipt,
        json!({"suspended":true,"mapping":{"result":false,"reason":"source mapping:true"},"global_shortcuts":{"result":true,"reason":"source shortcuts:true"}})
    );
    assert!(state.release(scope, &mut host).is_err());
    host.rejected.clear();
    state.release(scope, &mut host).unwrap();
    assert!(state.recorder.is_none());
}

#[test]
fn stopped_recorder_keeps_final_event_owner_and_cannot_be_read_or_released_by_a_peer() {
    let mut state = State::<Input>::default();
    let mut host = Controls::default();
    let scope = Uuid::new_v4();
    request(
        &mut state,
        scope,
        ServiceRequest::StartMacroRecording,
        &mut host,
    )
    .unwrap();
    request(
        &mut state,
        scope,
        ServiceRequest::StopMacroRecording,
        &mut host,
    )
    .unwrap();
    assert!(state.recorder.is_some());
    assert_eq!(
        request(
            &mut state,
            scope,
            ServiceRequest::QueryMacroRecordingStarted,
            &mut host
        )
        .unwrap()["result"],
        false
    );
    assert!(
        request(
            &mut state,
            Uuid::new_v4(),
            ServiceRequest::MacroRecordingEvents,
            &mut host
        )
        .is_err()
    );
    state.release(Uuid::new_v4(), &mut host).unwrap();
    assert!(state.recorder.is_some());
    request(
        &mut state,
        scope,
        ServiceRequest::MacroRecordingEvents,
        &mut host,
    )
    .unwrap();
    state.release(scope, &mut host).unwrap();
    assert!(state.recorder.is_none());
}
