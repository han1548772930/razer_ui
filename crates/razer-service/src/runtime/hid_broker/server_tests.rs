use super::*;
use razer_ipc::hid_broker::{ClientSessionId, EndpointId};
use std::{
    rc::Rc,
    sync::{Mutex, atomic::AtomicUsize},
    thread::ThreadId,
};
use tokio::{io::DuplexStream, sync::oneshot};

#[derive(Default)]
struct State {
    requests: AtomicUsize,
    closes: AtomicUsize,
    close_failures: AtomicUsize,
    terminal_close_failure: AtomicBool,
    dropped: AtomicBool,
    wait_for_cancel: AtomicBool,
    started: Mutex<Option<oneshot::Sender<()>>>,
    creation_thread: Mutex<Option<ThreadId>>,
    cancellation_addresses: Mutex<Vec<usize>>,
}

struct MockExecutor {
    state: Arc<State>,
    owner: ThreadId,
    _thread_affinity: Rc<()>,
}

impl MockExecutor {
    fn new(state: Arc<State>) -> Self {
        let owner = thread::current().id();
        *state.creation_thread.lock().unwrap() = Some(owner);
        Self {
            state,
            owner,
            _thread_affinity: Rc::new(()),
        }
    }
}

impl Drop for MockExecutor {
    fn drop(&mut self) {
        assert_eq!(thread::current().id(), self.owner);
        self.state.dropped.store(true, Ordering::Release);
    }
}

impl SessionExecutor for MockExecutor {
    fn request(&mut self, envelope: RequestEnvelope, canceled: &AtomicBool) -> ResponseEnvelope {
        assert_eq!(thread::current().id(), self.owner);
        self.state.requests.fetch_add(1, Ordering::AcqRel);
        self.state
            .cancellation_addresses
            .lock()
            .unwrap()
            .push(canceled as *const AtomicBool as usize);
        if let Some(started) = self.state.started.lock().unwrap().take() {
            let _ = started.send(());
        }
        if self.state.wait_for_cancel.load(Ordering::Acquire) {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            while !canceled.load(Ordering::Acquire) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "control loop failed to cancel the active executor"
                );
                thread::yield_now();
            }
            return ResponseEnvelope {
                diagnostics: None,
                id: envelope.id,
                data: None,
                error: Some("confirmed canceled request".to_owned()),
                fatal: false,
            };
        }
        if matches!(envelope.request, ServiceRequest::Shutdown) {
            let mut receipt = self.close().into_response();
            receipt.id = envelope.id;
            return receipt;
        }
        ResponseEnvelope {
            diagnostics: None,
            id: envelope.id,
            data: Some(serde_json::json!({"observed":137})),
            error: None,
            fatal: false,
        }
    }

    fn close(&mut self) -> CleanupProgress {
        assert_eq!(thread::current().id(), self.owner);
        self.state.closes.fetch_add(1, Ordering::AcqRel);
        let failed = self
            .state
            .close_failures
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_sub(1)
            })
            .is_ok();
        let terminal_failure = self.state.terminal_close_failure.load(Ordering::Acquire);
        let response = ResponseEnvelope {
            diagnostics: None,
            id: 0,
            data: (!failed && !terminal_failure).then(|| serde_json::json!({"stopped":true})),
            error: (failed || terminal_failure).then(|| {
                if terminal_failure {
                    "restore failed; native owners already released".to_owned()
                } else {
                    "restore failed; native owner retained".to_owned()
                }
            }),
            fatal: false,
        };
        if failed {
            CleanupProgress::Pending(response)
        } else {
            CleanupProgress::Released(response)
        }
    }
}

fn identity() -> SessionIdentity {
    SessionIdentity::new(
        EndpointId::try_from("0123456789abcdef0123456789abcdef".to_owned()).unwrap(),
        BrokerInstanceId::new(),
        ClientSessionId::new(),
    )
}

fn query(id: u64) -> RequestEnvelope {
    RequestEnvelope {
        id,
        request: ServiceRequest::KeyboardLayoutRead,
    }
}

fn request(identity: &SessionIdentity, request_id: BrokerRequestId, id: u64) -> ClientFrame {
    ClientFrame::Request {
        identity: identity.clone(),
        request_id,
        envelope: query(id),
    }
}

fn spawn_session(
    state: Arc<State>,
) -> (
    DuplexStream,
    SessionIdentity,
    JoinHandle<anyhow::Result<()>>,
    watch::Sender<bool>,
) {
    let (client, server) = tokio::io::duplex(32 * 1024);
    let identity = identity();
    let server_identity = identity.clone();
    let (stopping, mut receiver) = watch::channel(false);
    let task = tokio::spawn(async move {
        serve_session(server, server_identity, &mut receiver, move || {
            MockExecutor::new(state)
        })
        .await
    });
    (client, identity, task, stopping)
}

async fn read_response(
    stream: &mut DuplexStream,
    identity: &SessionIdentity,
    request_id: BrokerRequestId,
    id: u64,
) -> ResponseEnvelope {
    let response: ServerFrame = timeout(Duration::from_secs(3), read_frame(stream))
        .await
        .unwrap()
        .unwrap();
    response.validate(identity, request_id).unwrap();
    let ServerFrame::Response { envelope, .. } = response else {
        panic!("expected a device response before cleanup");
    };
    assert_eq!(envelope.id, id);
    envelope
}

async fn close_session(
    stream: &mut DuplexStream,
    identity: &SessionIdentity,
) -> CloseSessionReceipt {
    let request_id = BrokerRequestId::new();
    write_frame(
        stream,
        &ClientFrame::CloseSession {
            identity: identity.clone(),
            request_id,
        },
    )
    .await
    .unwrap();
    let response: ServerFrame = timeout(Duration::from_secs(3), read_frame(stream))
        .await
        .unwrap()
        .unwrap();
    response.validate(identity, request_id).unwrap();
    let ServerFrame::SessionClosed { receipt, .. } = response else {
        panic!("expected cleanup receipt");
    };
    receipt
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn broker_session_keeps_thread_affine_executor_on_one_os_thread_and_joins_before_receipt() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let caller = thread::current().id();
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let request_id = BrokerRequestId::new();
        write_frame(&mut client, &request(&identity, request_id, 71))
            .await
            .unwrap();
        let response = read_response(&mut client, &identity, request_id, 71).await;
        #[derive(serde::Deserialize)]
        struct Observation {
            observed: u16,
        }
        assert_eq!(
            serde_json::from_value::<Observation>(response.data.unwrap())
                .unwrap()
                .observed,
            137
        );
        assert!(!state.dropped.load(Ordering::Acquire));
        close_session(&mut client, &identity)
            .await
            .validate()
            .unwrap();
        assert!(state.dropped.load(Ordering::Acquire));
        assert_ne!(state.creation_thread.lock().unwrap().unwrap(), caller);
        session.await.unwrap().unwrap();
        assert_eq!(state.requests.load(Ordering::Acquire), 1);
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_control_reader_cancels_a_blocked_native_request_before_its_real_response() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.wait_for_cancel.store(true, Ordering::Release);
        let (started, start) = oneshot::channel();
        *state.started.lock().unwrap() = Some(started);
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let request_id = BrokerRequestId::new();
        write_frame(&mut client, &request(&identity, request_id, 72))
            .await
            .unwrap();
        start.await.unwrap();
        write_frame(
            &mut client,
            &ClientFrame::Cancel {
                identity: identity.clone(),
                request_id,
            },
        )
        .await
        .unwrap();
        let response = read_response(&mut client, &identity, request_id, 72).await;
        assert_eq!(
            response.error.as_deref(),
            Some("confirmed canceled request")
        );
        close_session(&mut client, &identity)
            .await
            .validate()
            .unwrap();
        session.await.unwrap().unwrap();
        assert!(state.dropped.load(Ordering::Acquire));
    });
}

#[test]
fn broker_close_cancels_pending_work_then_returns_device_reply_before_cleanup() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.wait_for_cancel.store(true, Ordering::Release);
        let (started, start) = oneshot::channel();
        *state.started.lock().unwrap() = Some(started);
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let request_id = BrokerRequestId::new();
        write_frame(&mut client, &request(&identity, request_id, 73))
            .await
            .unwrap();
        start.await.unwrap();
        let close_id = BrokerRequestId::new();
        write_frame(
            &mut client,
            &ClientFrame::CloseSession {
                identity: identity.clone(),
                request_id: close_id,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            read_response(&mut client, &identity, request_id, 73)
                .await
                .error
                .as_deref(),
            Some("confirmed canceled request")
        );
        let closed: ServerFrame = read_frame(&mut client).await.unwrap();
        closed.validate(&identity, close_id).unwrap();
        let ServerFrame::SessionClosed { receipt, .. } = closed else {
            panic!("cleanup did not follow pending response");
        };
        receipt.validate().unwrap();
        assert!(state.dropped.load(Ordering::Acquire));
        session.await.unwrap().unwrap();
    });
}

#[test]
fn broker_cleanup_failure_retains_same_runtime_until_an_explicit_retry_succeeds() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.close_failures.store(1, Ordering::Release);
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let receipt = close_session(&mut client, &identity).await;
        assert!(matches!(&receipt, CloseSessionReceipt::Pending { error } if error.contains("restore failed")));
        assert!(!receipt.resources_released());
        assert!(receipt.validate().is_err());
        assert!(!state.dropped.load(Ordering::Acquire));
        assert!(!session.is_finished());
        close_session(&mut client, &identity)
            .await
            .validate()
            .unwrap();
        assert!(state.dropped.load(Ordering::Acquire));
        session.await.unwrap().unwrap();
        assert_eq!(state.closes.load(Ordering::Acquire), 2);
    });
}

#[test]
fn broker_terminal_restoration_failure_joins_before_failed_receipt_and_ends_the_session() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.terminal_close_failure.store(true, Ordering::Release);
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let receipt = close_session(&mut client, &identity).await;
        assert!(receipt.resources_released());
        assert!(
            receipt
                .validate()
                .unwrap_err()
                .to_string()
                .contains("native owners already released")
        );
        assert!(state.dropped.load(Ordering::Acquire));
        let error = session.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("restore failed"));
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
        assert!(read_frame::<_, ServerFrame>(&mut client).await.is_err());
    });
}

#[test]
fn listener_shutdown_waits_only_for_pending_resources_and_keeps_all_failures() {
    let mut rounds = std::collections::VecDeque::from([
        super::super::ShutdownProgress::Pending {
            failures: vec!["unregister initially failed".into()],
        },
        super::super::ShutdownProgress::Released {
            failures: vec!["final restoration failed".into()],
        },
    ]);
    let mut waits = 0;
    let error = prepare_listener_shutdown_with(
        || {
            rounds
                .pop_front()
                .expect("must not retry after actual native release")
        },
        || waits += 1,
    )
    .unwrap_err();
    assert_eq!(waits, 1);
    assert!(rounds.is_empty());
    assert!(error.to_string().contains("unregister initially failed"));
    assert!(error.to_string().contains("final restoration failed"));
}

#[test]
fn listener_exit_preserves_session_native_and_endpoint_failures_together() {
    let mut failures = Vec::new();
    log_session(
        Some(Ok(Err(anyhow::anyhow!(
            "terminal session restoration failed"
        )))),
        &mut failures,
    );
    let error = listener_result(
        Some(anyhow::anyhow!("listener accept failed")),
        failures,
        Err(anyhow::anyhow!("global cleanup had a historical failure")),
        Err(anyhow::anyhow!("socket release failed")),
    )
    .unwrap_err();
    for expected in [
        "terminal session restoration failed",
        "listener accept failed",
        "global cleanup had a historical failure",
        "socket release failed",
    ] {
        assert!(error.to_string().contains(expected));
    }
}

#[test]
fn broker_shutdown_success_is_reused_for_close_without_repeating_native_restore() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let request_id = BrokerRequestId::new();
        write_frame(
            &mut client,
            &ClientFrame::Request {
                identity: identity.clone(),
                request_id,
                envelope: RequestEnvelope {
                    id: 74,
                    request: ServiceRequest::Shutdown,
                },
            },
        )
        .await
        .unwrap();
        cleanup_result(&read_response(&mut client, &identity, request_id, 74).await).unwrap();
        close_session(&mut client, &identity)
            .await
            .validate()
            .unwrap();
        session.await.unwrap().unwrap();
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_request_cancellation_storage_is_stable_for_the_entire_session() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        for id in [75, 76] {
            let request_id = BrokerRequestId::new();
            write_frame(&mut client, &request(&identity, request_id, id))
                .await
                .unwrap();
            assert!(
                read_response(&mut client, &identity, request_id, id)
                    .await
                    .error
                    .is_none()
            );
        }
        close_session(&mut client, &identity)
            .await
            .validate()
            .unwrap();
        session.await.unwrap().unwrap();
        let addresses = state.cancellation_addresses.lock().unwrap();
        assert_eq!(addresses.len(), 2);
        assert_eq!(addresses[0], addresses[1]);
    });
}

#[test]
fn broker_eof_cancels_active_work_and_waits_for_actual_restoration() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.wait_for_cancel.store(true, Ordering::Release);
        let (started, start) = oneshot::channel();
        *state.started.lock().unwrap() = Some(started);
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        write_frame(&mut client, &request(&identity, BrokerRequestId::new(), 77))
            .await
            .unwrap();
        start.await.unwrap();
        drop(client);
        assert!(
            timeout(Duration::from_secs(3), session)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert!(state.dropped.load(Ordering::Acquire));
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_rejects_foreign_session_identity_and_releases_its_own_runtime() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, mut identity, session, _stopping) = spawn_session(state.clone());
        identity.session = ClientSessionId::new();
        write_frame(&mut client, &request(&identity, BrokerRequestId::new(), 78))
            .await
            .unwrap();
        assert!(
            timeout(Duration::from_secs(3), session)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(state.requests.load(Ordering::Acquire), 0);
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
        assert!(state.dropped.load(Ordering::Acquire));
    });
}

#[test]
fn broker_duplicate_uuid_cannot_execute_a_second_service_request() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let request_id = BrokerRequestId::new();
        write_frame(&mut client, &request(&identity, request_id, 79))
            .await
            .unwrap();
        read_response(&mut client, &identity, request_id, 79).await;
        write_frame(&mut client, &request(&identity, request_id, 80))
            .await
            .unwrap();
        assert!(
            timeout(Duration::from_secs(3), session)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(state.requests.load(Ordering::Acquire), 1);
        assert!(state.dropped.load(Ordering::Acquire));
    });
}

#[test]
fn broker_rejects_second_pending_request_and_cancels_only_the_current_owner() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.wait_for_cancel.store(true, Ordering::Release);
        let (started, start) = oneshot::channel();
        *state.started.lock().unwrap() = Some(started);
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        write_frame(&mut client, &request(&identity, BrokerRequestId::new(), 81))
            .await
            .unwrap();
        start.await.unwrap();
        write_frame(&mut client, &request(&identity, BrokerRequestId::new(), 82))
            .await
            .unwrap();
        assert!(
            timeout(Duration::from_secs(3), session)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(state.requests.load(Ordering::Acquire), 1);
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_late_cancel_for_just_completed_request_is_idempotent_and_cannot_cancel_successor() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        let request_id = BrokerRequestId::new();
        write_frame(&mut client, &request(&identity, request_id, 83))
            .await
            .unwrap();
        read_response(&mut client, &identity, request_id, 83).await;
        write_frame(
            &mut client,
            &ClientFrame::Cancel {
                identity: identity.clone(),
                request_id,
            },
        )
        .await
        .unwrap();
        let successor = BrokerRequestId::new();
        write_frame(&mut client, &request(&identity, successor, 84))
            .await
            .unwrap();
        assert!(
            read_response(&mut client, &identity, successor, 84)
                .await
                .error
                .is_none()
        );
        close_session(&mut client, &identity)
            .await
            .validate()
            .unwrap();
        session.await.unwrap().unwrap();
        assert_eq!(state.requests.load(Ordering::Acquire), 2);
    });
}

#[test]
fn broker_stale_cancel_uuid_does_not_stop_another_session_request() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, identity, session, _stopping) = spawn_session(state.clone());
        write_frame(
            &mut client,
            &ClientFrame::Cancel {
                identity,
                request_id: BrokerRequestId::new(),
            },
        )
        .await
        .unwrap();
        assert!(
            timeout(Duration::from_secs(3), session)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(state.requests.load(Ordering::Acquire), 0);
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_handshake_checks_actual_endpoint_before_creating_native_state() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, server) = tokio::io::duplex(32 * 1024);
        let endpoint = BrokerEndpoint::for_installation(&std::env::current_exe().unwrap()).unwrap();
        let instance = BrokerInstanceId::new();
        let (_stopping, receiver) = watch::channel(false);
        let factory_state = state.clone();
        let serving = tokio::spawn(serve_stream(
            server,
            endpoint,
            instance,
            receiver,
            move || MockExecutor::new(factory_state),
        ));
        write_frame(&mut client, &ClientHello::new(identity().endpoint, None))
            .await
            .unwrap();
        assert!(serving.await.unwrap().is_err());
        assert!(state.creation_thread.lock().unwrap().is_none());
        assert_eq!(state.requests.load(Ordering::Acquire), 0);
    });
}

#[test]
fn broker_listener_stop_cancels_requests_and_still_completes_session_cleanup() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        state.wait_for_cancel.store(true, Ordering::Release);
        let (started, start) = oneshot::channel();
        *state.started.lock().unwrap() = Some(started);
        let (mut client, identity, session, stopping) = spawn_session(state.clone());
        write_frame(&mut client, &request(&identity, BrokerRequestId::new(), 85))
            .await
            .unwrap();
        start.await.unwrap();
        stopping.send(true).unwrap();
        timeout(Duration::from_secs(3), session)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(state.dropped.load(Ordering::Acquire));
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_one_session_closing_does_not_release_another_live_session() {
    runtime().block_on(async {
        let first_state = Arc::new(State::default());
        let second_state = Arc::new(State::default());
        let (mut first, first_identity, first_session, _first_stopping) =
            spawn_session(first_state.clone());
        let (mut second, second_identity, second_session, _second_stopping) =
            spawn_session(second_state.clone());
        close_session(&mut first, &first_identity)
            .await
            .validate()
            .unwrap();
        first_session.await.unwrap().unwrap();
        assert!(first_state.dropped.load(Ordering::Acquire));
        assert!(!second_state.dropped.load(Ordering::Acquire));
        let request_id = BrokerRequestId::new();
        write_frame(&mut second, &request(&second_identity, request_id, 86))
            .await
            .unwrap();
        assert!(
            read_response(&mut second, &second_identity, request_id, 86)
                .await
                .error
                .is_none()
        );
        close_session(&mut second, &second_identity)
            .await
            .validate()
            .unwrap();
        second_session.await.unwrap().unwrap();
        assert!(second_state.dropped.load(Ordering::Acquire));
    });
}

#[test]
fn broker_foreign_close_request_cannot_confirm_cleanup_for_another_session() {
    runtime().block_on(async {
        let state = Arc::new(State::default());
        let (mut client, mut identity, session, _stopping) = spawn_session(state.clone());
        identity.instance = BrokerInstanceId::new();
        write_frame(
            &mut client,
            &ClientFrame::CloseSession {
                identity,
                request_id: BrokerRequestId::new(),
            },
        )
        .await
        .unwrap();
        assert!(
            timeout(Duration::from_secs(3), session)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        let response = read_frame::<_, ServerFrame>(&mut client).await;
        assert!(response.is_err());
        assert!(state.dropped.load(Ordering::Acquire));
        assert_eq!(state.closes.load(Ordering::Acquire), 1);
    });
}

#[test]
fn broker_admission_reopens_only_after_an_owned_session_slot_is_released() {
    let mut active = 0;
    while session_available(active) {
        active += 1;
    }
    assert!(active > 1);
    assert!(!session_available(active));
    assert!(!session_available(active + 1));
    assert!(session_available(active - 1));
}
