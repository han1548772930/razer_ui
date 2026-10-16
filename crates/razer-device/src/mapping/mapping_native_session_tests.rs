use super::*;
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

#[derive(Default)]
struct SimTransport {
    opened_path: Option<String>,
    open_error: Option<NativeTransportError>,
    submit_error: Option<NativeTransportError>,
    poll_error: Option<NativeTransportError>,
    close_error: Option<NativeTransportError>,
    cancel_errors: BTreeMap<NativeSubmissionId, NativeTransportError>,
    next_id: u64,
    completions: BTreeMap<NativeSubmissionId, VecDeque<NativeCompletion>>,
    submitted: Vec<(NativeSubmissionId, NativeOperation)>,
    canceled: Vec<NativeSubmissionId>,
    close_calls: usize,
}

impl SimTransport {
    fn complete(&mut self, id: NativeSubmissionId, result: Result<Vec<u8>, NativeTransportError>) {
        let queue = self.completions.entry(id).or_default();
        queue.clear();
        queue.push_back(NativeCompletion::Completed(result));
    }
}

impl NativeSessionTransport for SimTransport {
    fn open(&mut self, path: &NativeDriverPath) -> Result<(), NativeTransportError> {
        if let Some(error) = self.open_error.take() {
            return Err(error);
        }
        self.opened_path = Some(path.as_str().to_owned());
        Ok(())
    }

    fn submit(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError> {
        if let Some(error) = self.submit_error.take() {
            return Err(error);
        }
        self.next_id += 1;
        let id = NativeSubmissionId(self.next_id);
        self.submitted.push((id, operation));
        self.completions
            .entry(id)
            .or_insert_with(|| VecDeque::from([NativeCompletion::Pending]));
        Ok(id)
    }

    fn poll(
        &mut self,
        id: NativeSubmissionId,
        _wait: Duration,
    ) -> Result<NativeCompletion, NativeTransportError> {
        if let Some(error) = self.poll_error.take() {
            return Err(error);
        }
        let queue = self
            .completions
            .get_mut(&id)
            .ok_or(NativeTransportError::UnknownSubmission(id))?;
        let completion = queue.pop_front().unwrap_or(NativeCompletion::Pending);
        if matches!(completion, NativeCompletion::Completed(_)) {
            self.completions.remove(&id);
        }
        Ok(completion)
    }

    fn cancel(&mut self, id: NativeSubmissionId) -> Result<(), NativeTransportError> {
        self.canceled.push(id);
        self.cancel_errors.remove(&id).map_or(Ok(()), Err)
    }

    fn close(&mut self) -> Result<(), NativeTransportError> {
        assert!(
            self.completions.is_empty(),
            "cannot free buffers with accepted requests outstanding"
        );
        self.close_calls += 1;
        if let Some(error) = self.close_error.take() {
            return Err(error);
        }
        Ok(())
    }
}

fn route() -> NativeRoute {
    NativeRoute::windows(
        NativeDriverPath::new("\\\\?\\acme#mapping#1").unwrap(),
        NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard),
        NativeCapabilitySet::empty()
            .with(NativeCapability::KeyboardInput)
            .with(NativeCapability::JoystickOutput)
            .with(NativeCapability::AnalogOutput),
    )
}

fn session(transport: SimTransport) -> NativeSession<SimTransport> {
    NativeSession::new(
        route(),
        NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard),
        NativeCapabilitySet::from(NativeCapability::KeyboardInput),
        transport,
    )
    .unwrap()
}

fn connect(session: &mut NativeSession<SimTransport>) -> NativeSubmissionId {
    let accepted = session.open().unwrap();
    assert_eq!(session.state(), NativeSessionState::Connected);
    assert_eq!(
        session.poll_input(Duration::ZERO).unwrap(),
        NativeInputPoll::Pending
    );
    accepted.id()
}

#[test]
fn path_and_product_capability_binding_are_explicit() {
    assert!(NativeDriverPath::new("").is_err());
    assert!(NativeDriverPath::new("\\\\?\\路径").is_err());
    assert!(NativeDriverPath::new("abc\0def").is_err());
    let route = route();
    route
        .validate(
            NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard),
            NativeCapabilitySet::from(NativeCapability::KeyboardInput),
        )
        .unwrap();
    assert!(matches!(
        route.validate(
            NativeProductIdentity::razer(1235, NativeDeviceKind::Keyboard),
            NativeCapabilitySet::empty()
        ),
        Err(NativeRouteError::ProductMismatch { .. })
    ));
    assert!(matches!(
        route.validate(
            NativeProductIdentity::new(0x1533, 1234, NativeDeviceKind::Keyboard),
            NativeCapabilitySet::empty()
        ),
        Err(NativeRouteError::VendorMismatch { .. })
    ));
    assert!(matches!(
        route.validate(
            NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard),
            NativeCapabilitySet::from(NativeCapability::GamepadOutput)
        ),
        Err(NativeRouteError::MissingCapabilities { .. })
    ));
}

#[test]
fn pending_input_stream_does_not_block_connected_output() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    assert_eq!(session.input_stream_submission(), Some(read));
    assert_eq!(session.pending_count(), 1);
    assert_eq!(
        session.transport_mut().submitted[0].1,
        NativeOperation::read_input()
    );
    assert_eq!(
        session.transport_mut().opened_path.as_deref(),
        Some("\\\\?\\acme#mapping#1")
    );
    let accepted = session
        .submit_ioctl(DRIVER_SEND_INPUT_IOCTL, [1, 2, 3, 4], 0)
        .unwrap();
    assert_eq!(session.pending_count(), 2);
    assert_eq!(
        session
            .poll_submission(accepted.id(), Duration::ZERO)
            .unwrap(),
        NativeCompletion::Pending
    );
    session
        .transport_mut()
        .complete(accepted.id(), Ok(Vec::new()));
    assert_eq!(
        session
            .poll_submission(accepted.id(), Duration::ZERO)
            .unwrap(),
        NativeCompletion::Completed(Ok(Vec::new()))
    );
    assert_eq!(session.pending_count(), 1);
    assert_eq!(session.state(), NativeSessionState::Connected);
}

#[test]
fn completed_input_is_rearmed_and_returns_actual_payload() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    let payload = (0..64).collect::<Vec<u8>>();
    session.transport_mut().complete(read, Ok(payload.clone()));
    assert_eq!(
        session.poll_input(Duration::ZERO).unwrap(),
        NativeInputPoll::Completed {
            output: payload,
            rearmed: Ok(NativeSubmissionId(2))
        }
    );
    assert_eq!(
        session.input_stream_submission(),
        Some(NativeSubmissionId(2))
    );
    assert_eq!(session.pending_count(), 1);
    assert_eq!(
        session.transport_mut().submitted[1].1,
        NativeOperation::read_input()
    );
    assert_eq!(
        session.poll_input(Duration::ZERO).unwrap(),
        NativeInputPoll::Pending
    );
    assert!(matches!(
        session.poll_submission(NativeSubmissionId(2), Duration::ZERO),
        Err(NativeSessionError::InvalidState { .. })
    ));
}

#[test]
fn rearm_failure_keeps_completed_payload_and_requires_cleanup() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    session.transport_mut().complete(read, Ok(vec![9, 8, 7]));
    session.transport_mut().submit_error = Some(NativeTransportError::Submit { code: 6 });
    assert_eq!(
        session.poll_input(Duration::ZERO).unwrap(),
        NativeInputPoll::Completed {
            output: vec![9, 8, 7],
            rearmed: Err(NativeTransportError::Submit { code: 6 })
        }
    );
    assert_eq!(session.state(), NativeSessionState::Failed);
    assert_eq!(session.pending_count(), 0);
    assert_eq!(session.input_stream_submission(), None);
    assert_eq!(session.retry(), Err(NativeSessionError::CleanupRequired));
    assert_eq!(session.retry_plan(), None);
    session.begin_disconnect().unwrap();
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Completed(Ok(Vec::new()))
    );
}

#[test]
fn failed_input_does_not_invent_an_event_or_retry() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    session
        .transport_mut()
        .complete(read, Err(NativeTransportError::Poll { code: 6 }));
    assert_eq!(
        session.poll_input(Duration::ZERO).unwrap(),
        NativeInputPoll::Failed(NativeTransportError::Poll { code: 6 })
    );
    assert_eq!(session.state(), NativeSessionState::Failed);
    assert_eq!(session.retry_plan(), None);
    session.begin_disconnect().unwrap();
    session.poll_disconnect(Duration::ZERO).unwrap();
    assert_eq!(session.state(), NativeSessionState::Closed);
}

#[test]
fn observation_error_retains_input_buffer_and_read_id() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    session.transport_mut().poll_error = Some(NativeTransportError::Poll { code: 6 });
    assert!(matches!(
        session.poll_input(Duration::ZERO),
        Err(NativeSessionError::Transport(NativeTransportError::Poll {
            code: 6
        }))
    ));
    assert_eq!(session.input_stream_submission(), Some(read));
    assert_eq!(session.pending_count(), 1);
    assert_eq!(
        session.poll_input(Duration::ZERO).unwrap(),
        NativeInputPoll::Pending
    );
}

#[test]
fn open_retry_policy_is_separate_from_filter_probe_retry() {
    let mut session = session(SimTransport::default());
    for attempt in 1..=11 {
        session.transport_mut().open_error = Some(NativeTransportError::Open { code: 2 });
        let result = if attempt == 1 {
            session.open()
        } else {
            session.retry()
        };
        assert!(matches!(
            result,
            Err(NativeSessionError::Transport(NativeTransportError::Open {
                code: 2
            }))
        ));
        let plan = session.retry_plan().unwrap();
        assert_eq!(plan.attempt, attempt);
        assert_eq!(plan.delay, Duration::from_secs(1 << attempt));
        assert_eq!(plan.exhausted, attempt > 10);
        assert_eq!(session.state(), NativeSessionState::Failed);
        assert_eq!(session.pending_count(), 0);
    }
    assert_eq!(session.retry(), Err(NativeSessionError::RetryExhausted));
    for (attempt, seconds, exhausted) in [
        (1, 4, false),
        (2, 8, false),
        (3, 10, false),
        (9, 10, false),
        (10, 10, true),
    ] {
        assert_eq!(
            native_filter_probe_retry_plan(attempt),
            NativeRetryPlan {
                attempt,
                delay: Duration::from_secs(seconds),
                exhausted
            }
        );
    }
}

#[test]
fn failed_read_submission_closes_before_retry() {
    let mut transport = SimTransport::default();
    transport.submit_error = Some(NativeTransportError::Submit { code: 5 });
    let mut session = session(transport);
    assert!(session.open().is_err());
    assert_eq!(session.transport_mut().close_calls, 1);
    let read = session.retry().unwrap();
    assert_eq!(session.input_stream_submission(), Some(read.id()));
    assert_eq!(session.state(), NativeSessionState::Connected);
}

#[test]
fn failed_close_after_rejected_read_blocks_retry_until_cleanup() {
    let mut transport = SimTransport::default();
    transport.submit_error = Some(NativeTransportError::Submit { code: 5 });
    transport.close_error = Some(NativeTransportError::Close { code: 6 });
    let mut session = session(transport);
    assert!(session.open().is_err());
    assert_eq!(session.retry(), Err(NativeSessionError::CleanupRequired));
    assert_eq!(
        session.cleanup_errors(),
        &[NativeTransportError::Close { code: 6 }]
    );
    session.begin_disconnect().unwrap();
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Completed(Err(NativeTransportError::Close { code: 6 }))
    );
    assert_eq!(session.state(), NativeSessionState::Closed);
}

#[test]
fn stream_and_output_are_canceled_and_drained_before_close() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    let report = crate::mapping_native_output::encode_driver_input(
        crate::mapping_native_output::DriverInput::JoystickPov([0, -1, 0]),
    );
    let accepted = session.submit_input_report(report).unwrap();
    session.begin_disconnect().unwrap();
    assert_eq!(session.transport_mut().canceled, vec![read, accepted.id()]);
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Pending
    );
    assert_eq!(session.transport_mut().close_calls, 0);
    session.transport_mut().complete(read, Ok(vec![1]));
    session
        .transport_mut()
        .complete(accepted.id(), Err(NativeTransportError::Poll { code: 995 }));
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Completed(Ok(Vec::new()))
    );
    assert_eq!(session.state(), NativeSessionState::Closed);
    assert_eq!(session.transport_mut().submitted.len(), 2);
    assert_eq!(session.transport_mut().close_calls, 1);
}

#[test]
fn cancellation_failure_is_retained_and_operation_still_drains() {
    let mut session = session(SimTransport::default());
    let read = connect(&mut session);
    session
        .transport_mut()
        .cancel_errors
        .insert(read, NativeTransportError::Cancel { code: 5 });
    session.begin_disconnect().unwrap();
    assert_eq!(
        session.cleanup_errors(),
        &[NativeTransportError::Cancel { code: 5 }]
    );
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Pending
    );
    session.cancel_submission(read).unwrap();
    session
        .transport_mut()
        .complete(read, Err(NativeTransportError::Poll { code: 995 }));
    assert_eq!(
        session.poll_disconnect(Duration::ZERO).unwrap(),
        NativeCompletion::Completed(Err(NativeTransportError::Cancel { code: 5 }))
    );
    assert_eq!(session.state(), NativeSessionState::Closed);
}

#[test]
fn failed_operation_is_final_but_keeps_connected_stream() {
    let mut session = session(SimTransport::default());
    connect(&mut session);
    let accepted = session
        .submit_ioctl(DRIVER_SEND_INPUT_IOCTL, [1, 2, 3, 4], 0)
        .unwrap();
    session
        .transport_mut()
        .complete(accepted.id(), Err(NativeTransportError::Poll { code: 995 }));
    assert_eq!(
        session
            .poll_submission(accepted.id(), Duration::ZERO)
            .unwrap(),
        NativeCompletion::Completed(Err(NativeTransportError::Poll { code: 995 }))
    );
    assert_eq!(session.state(), NativeSessionState::Connected);
    assert_eq!(session.pending_count(), 1);
}

#[test]
fn filter_discovery_matches_class_container_and_pid_without_first_hid_fallback() {
    let container = NativeGuid::new(123, 456, 789, [1; 8]);
    let query = NativeDiscoveryQuery {
        container,
        product: NativeProductIdentity::razer(0x02a7, NativeDeviceKind::Keyboard),
        verified_capabilities: NativeCapabilitySet::from(NativeCapability::KeyboardInput),
    };
    let hid_class = NativeGuid::new(
        0x4d1e55b2,
        0xf16f,
        0x11cf,
        [0x88, 0xcb, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30],
    );
    assert_eq!(
        query.route_for(hid_class, container, "\\\\?\\hid#vid_1532&pid_02a7"),
        Ok(None)
    );
    assert_eq!(
        query.route_for(
            FILTER_DRIVER_INTERFACE_CLASS,
            NativeGuid::new(124, 456, 789, [1; 8]),
            "\\\\?\\hid#vid_1532&pid_02a7"
        ),
        Ok(None)
    );
    for path in [
        "\\\\?\\hid#vid_1532&pid_02a8",
        "\\\\?\\hid#pid_02a",
        "\\\\?\\hid#pid_xxxx",
        "\\\\?\\hid#vid_1532",
    ] {
        assert_eq!(
            query.route_for(FILTER_DRIVER_INTERFACE_CLASS, container, path),
            Ok(None)
        );
    }
    let paths = [
        "\\\\?\\HID#VID_1532&PID_02A7&COL01",
        "\\\\?\\HID#VID_1532&PID_02A7&COL02",
    ];
    let routes = paths
        .iter()
        .filter_map(|path| {
            query
                .route_for(FILTER_DRIVER_INTERFACE_CLASS, container, path)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].path().as_str(), paths[0].to_ascii_lowercase());
    assert_eq!(routes[1].path().as_str(), paths[1].to_ascii_lowercase());
    assert_eq!(routes[1].container(), Some(container));
    assert_eq!(routes[1].capabilities(), query.verified_capabilities);
    let invalid = NativeDiscoveryQuery {
        container: NativeGuid::new(0, 0, 0, [0; 8]),
        ..query
    };
    assert_eq!(invalid.validate(), Err(NativeRouteError::InvalidContainer));
}

#[test]
fn unsupported_platform_stays_explicit() {
    let mut session = NativeSession::new(
        route(),
        NativeProductIdentity::razer(1234, NativeDeviceKind::Keyboard),
        NativeCapabilitySet::from(NativeCapability::KeyboardInput),
        UnsupportedNativeSessionTransport,
    )
    .unwrap();
    assert!(matches!(
        session.open(),
        Err(NativeSessionError::Transport(
            NativeTransportError::UnsupportedPlatform
        ))
    ));
    assert_eq!(session.state(), NativeSessionState::Failed);
    assert_eq!(session.pending_count(), 0);
}
