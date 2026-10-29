use super::*;
use crate::{RequestEnvelope, ResponseEnvelope, ServiceRequest};
use anyhow::{bail, ensure};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::{
    io::{AsyncWriteExt, DuplexStream},
    runtime::{Builder, Runtime},
};

fn runtime() -> Runtime {
    Builder::new_current_thread().enable_all().build().unwrap()
}

fn identity() -> SessionIdentity {
    SessionIdentity {
        endpoint: EndpointId::try_from("0123456789abcdef0123456789abcdef".to_owned()).unwrap(),
        instance: BrokerInstanceId::new(),
        session: ClientSessionId::new(),
    }
}

fn request(id: u64) -> RequestEnvelope {
    RequestEnvelope {
        id,
        request: ServiceRequest::Shutdown,
    }
}

fn reply(identity: SessionIdentity, request_id: BrokerRequestId, id: u64) -> ServerFrame {
    ServerFrame::Response {
        identity,
        request_id,
        envelope: ResponseEnvelope {
            diagnostics: None,
            id,
            data: Some(serde_json::json!({"observed":137})),
            error: None,
            fatal: false,
        },
    }
}

async fn receive_request(
    stream: &mut DuplexStream,
    identity: &SessionIdentity,
) -> anyhow::Result<(BrokerRequestId, u64)> {
    let frame: ClientFrame = read_frame(stream).await?;
    ensure!(frame.identity() == identity, "unexpected simulated session");
    match frame {
        ClientFrame::Request {
            request_id,
            envelope,
            ..
        } => Ok((request_id, envelope.id)),
        _ => bail!("expected a full service request"),
    }
}

async fn close(
    stream: &mut DuplexStream,
    identity: &SessionIdentity,
    receipt: CloseSessionReceipt,
) -> anyhow::Result<()> {
    let frame: ClientFrame = read_frame(stream).await?;
    ensure!(frame.identity() == identity, "unexpected close identity");
    let ClientFrame::CloseSession { request_id, .. } = frame else {
        bail!("expected session close");
    };
    write_frame(
        stream,
        &ServerFrame::SessionClosed {
            identity: identity.clone(),
            request_id,
            receipt,
        },
    )
    .await
}

struct Simulation {
    client: Option<BrokerClient>,
    server: Option<JoinHandle<anyhow::Result<()>>>,
}

impl Simulation {
    fn new<F, Fut>(server: F) -> Self
    where
        F: FnOnce(DuplexStream, SessionIdentity) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let identity = identity();
        let (client_stream, server_stream) = tokio::io::duplex(64 * 1024);
        let client = super::client::simulated_client(
            runtime(),
            client_stream,
            identity.clone(),
            Duration::from_secs(2),
        );
        let server =
            std::thread::spawn(move || runtime().block_on(server(server_stream, identity)));
        Self {
            client: Some(client),
            server: Some(server),
        }
    }

    fn client(&mut self) -> &mut BrokerClient {
        self.client.as_mut().unwrap()
    }

    fn finish(mut self) {
        drop(self.client.take());
        self.server.take().unwrap().join().unwrap().unwrap();
    }
}

impl Drop for Simulation {
    fn drop(&mut self) {
        drop(self.client.take());
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

#[test]
fn wire_matches_independent_big_endian_prefix_and_json_bytes() {
    runtime().block_on(async {
        let mut output = Vec::new();
        write_frame(&mut output, &137u32).await.unwrap();
        assert_eq!(output, b"\0\0\0\x03137");
        assert_eq!(
            read_frame::<_, u32>(&mut &b"\0\0\0\x03137"[..])
                .await
                .unwrap(),
            137
        );
    });
}

#[test]
fn close_receipt_requires_explicit_resource_and_restoration_outcomes() {
    let cases = [
        (
            br#"{"resource_state":"pending","error":"unregister failed"}"#.as_slice(),
            CloseSessionReceipt::pending("unregister failed".to_owned()),
            false,
            false,
        ),
        (
            br#"{"resource_state":"released","outcome":{"result":"confirmed"}}"#.as_slice(),
            CloseSessionReceipt::confirmed(),
            true,
            true,
        ),
        (
            br#"{"resource_state":"released","outcome":{"result":"failed","error":"restore failed"}}"#.as_slice(),
            CloseSessionReceipt::restoration_failed("restore failed".to_owned()),
            true,
            false,
        ),
    ];
    for (bytes, expected, released, restored) in cases {
        let parsed: CloseSessionReceipt = serde_json::from_slice(bytes).unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(serde_json::to_vec(&expected).unwrap(), bytes);
        assert_eq!(parsed.resources_released(), released);
        assert_eq!(parsed.validate().is_ok(), restored);
    }
    for invalid in [
        r#"{}"#,
        r#"{"cleanup_confirmed":true,"error":null}"#,
        r#"{"resource_state":"unknown"}"#,
        r#"{"resource_state":"pending"}"#,
        r#"{"resource_state":"released"}"#,
        r#"{"resource_state":"released","outcome":null}"#,
        r#"{"resource_state":"released","outcome":{"result":"unknown"}}"#,
        r#"{"resource_state":"released","outcome":{"result":"failed"}}"#,
        r#"{"resource_state":"released","outcome":{"result":"confirmed","error":"contradiction"}}"#,
        r#"{"resource_state":"pending","error":"busy","outcome":{"result":"confirmed"}}"#,
    ] {
        assert!(
            serde_json::from_str::<CloseSessionReceipt>(invalid).is_err(),
            "accepted ambiguous close receipt: {invalid}"
        );
    }
}

#[test]
fn frames_reject_zero_oversize_truncation_eof_and_wrong_json_type() {
    runtime().block_on(async {
        let samples: &[&[u8]] = &[
            b"",
            b"\0",
            b"\0\0\0",
            b"\0\0\0\0",
            b"\0\x40\0\0",
            b"\0\0\0\x0313",
            b"\0\0\0\x04null",
            b"\0\0\0\x03[] ",
        ];
        for sample in samples {
            let mut input = *sample;
            assert!(
                read_frame::<_, u32>(&mut input).await.is_err(),
                "accepted {sample:?}"
            );
        }
        let oversized = "x".repeat(crate::MAX_FRAME_BYTES);
        assert!(write_frame(&mut Vec::new(), &oversized).await.is_err());
    });
}

#[test]
fn wire_uuid_types_reject_noncanonical_nonrandom_and_missing_identity_fields() {
    for invalid in [
        "",
        "00112233-4455-4677-8899-aabbccddeeff",
        "00112233445546778899AABBCCDDEEFF",
        "00112233445516778899aabbccddeeff",
        "00112233445546770899aabbccddeeff",
    ] {
        assert!(BrokerRequestId::try_from(invalid.to_owned()).is_err());
        assert!(ClientSessionId::try_from(invalid.to_owned()).is_err());
        assert!(BrokerInstanceId::try_from(invalid.to_owned()).is_err());
    }
    assert!(serde_json::from_str::<SessionIdentity>("{}").is_err());
    assert!(EndpointId::try_from("0123456789abcdef0123456789abcdeg".to_owned()).is_err());
}

#[test]
fn handshake_rejects_version_endpoint_nonce_and_instance_replacement() {
    let identity = identity();
    let hello = ClientHello::new(identity.endpoint.clone(), Some(identity.instance));
    hello
        .validate(&identity.endpoint, identity.instance)
        .unwrap();
    let accepted = ServerHello::new(&hello, identity.instance).unwrap();
    accepted.validate(&hello).unwrap();
    assert_eq!(PROTOCOL_VERSION, 3);
    for version in [1, 2] {
        let mut legacy_hello = hello.clone();
        legacy_hello.version = version;
        assert!(
            legacy_hello
                .validate(&identity.endpoint, identity.instance)
                .is_err()
        );
        let mut legacy_reply = accepted.clone();
        legacy_reply.version = version;
        assert!(legacy_reply.validate(&hello).is_err());
    }
    let mut wrong = accepted.clone();
    wrong.version += 1;
    assert!(wrong.validate(&hello).is_err());
    wrong = accepted.clone();
    wrong.nonce = HandshakeNonce::new();
    assert!(wrong.validate(&hello).is_err());
    wrong = accepted.clone();
    wrong.identity.instance = BrokerInstanceId::new();
    assert!(wrong.validate(&hello).is_err());
    wrong = accepted;
    wrong.identity.endpoint =
        EndpointId::try_from("ffffffffffffffffffffffffffffffff".to_owned()).unwrap();
    assert!(wrong.validate(&hello).is_err());
}

#[test]
fn response_requires_full_session_identity_and_random_request_association() {
    let identity = identity();
    let request_id = BrokerRequestId::new();
    let response = reply(identity.clone(), request_id, 29);
    response.validate(&identity, request_id).unwrap();
    assert!(
        response
            .validate(&identity, BrokerRequestId::new())
            .is_err()
    );
    let mut replacement = identity.clone();
    replacement.session = ClientSessionId::new();
    assert!(response.validate(&replacement, request_id).is_err());
    replacement = identity.clone();
    replacement.instance = BrokerInstanceId::new();
    assert!(response.validate(&replacement, request_id).is_err());
    replacement = identity;
    replacement.endpoint =
        EndpointId::try_from("ffffffffffffffffffffffffffffffff".to_owned()).unwrap();
    assert!(response.validate(&replacement, request_id).is_err());
}

#[test]
fn interrupted_reader_keeps_partial_prefix_and_body_for_cleanup_drain() {
    runtime().block_on(async {
        for prefix_length in 1..7 {
            let (mut writer, reader) = tokio::io::duplex(64);
            let bytes = b"\0\0\0\x03137";
            writer.write_all(&bytes[..prefix_length]).await.unwrap();
            let mut reader = FrameReader::new(reader);
            let mut future = Box::pin(reader.read::<u32>());
            let result = std::future::poll_fn(|context| {
                std::task::Poll::Ready(std::future::Future::poll(future.as_mut(), context))
            })
            .await;
            assert!(result.is_pending());
            drop(future);
            writer.write_all(&bytes[prefix_length..]).await.unwrap();
            assert_eq!(reader.read::<u32>().await.unwrap(), 137);
        }
    });
}

#[test]
fn interrupted_writer_finishes_previous_frame_before_a_control_frame() {
    runtime().block_on(async {
        let (writer, mut reader) = tokio::io::duplex(3);
        let mut writer = FrameWriter::new(writer);
        let mut future = Box::pin(writer.write(&137u32));
        let result = std::future::poll_fn(|context| {
            std::task::Poll::Ready(std::future::Future::poll(future.as_mut(), context))
        })
        .await;
        assert!(result.is_pending());
        drop(future);
        let (write, read) = tokio::join!(writer.write(&42u32), async {
            assert_eq!(read_frame::<_, u32>(&mut reader).await.unwrap(), 137);
            assert_eq!(read_frame::<_, u32>(&mut reader).await.unwrap(), 42);
        },);
        write.unwrap();
        read
    });
}

#[test]
fn complete_service_request_and_measured_reply_are_preserved_until_confirmed_close() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        let (request_id, id) = receive_request(&mut stream, &identity).await?;
        ensure!(id == 29, "inner request identity was changed");
        write_frame(&mut stream, &reply(identity.clone(), request_id, id)).await?;
        close(&mut stream, &identity, CloseSessionReceipt::confirmed()).await
    });
    let response = simulation
        .client()
        .request(request(29), &|| false, Duration::from_secs(2))
        .unwrap();
    #[derive(serde::Deserialize)]
    struct Measured {
        observed: u16,
    }
    let measured: Measured = serde_json::from_value(response.data.unwrap()).unwrap();
    assert_eq!(measured.observed, 137);
    assert!(simulation.client().close_confirmation().is_none());
    assert!(
        simulation
            .client()
            .close_session()
            .unwrap()
            .validate()
            .is_ok()
    );
    assert!(
        simulation
            .client()
            .close_confirmation()
            .unwrap()
            .validate()
            .is_ok()
    );
    assert!(
        simulation
            .client()
            .request(request(30), &|| false, Duration::from_secs(2))
            .is_err()
    );
    simulation.finish();
}

#[test]
fn cancellation_travels_while_device_response_waits_and_retains_actual_failed_reply() {
    let cancel = Arc::new(AtomicBool::new(false));
    let server_cancel = cancel.clone();
    let mut simulation = Simulation::new(move |mut stream, identity| async move {
        let (request_id, id) = receive_request(&mut stream, &identity).await?;
        server_cancel.store(true, Ordering::Release);
        let control: ClientFrame = read_frame(&mut stream).await?;
        ensure!(
            matches!(control, ClientFrame::Cancel { .. }),
            "cancel was not independently delivered"
        );
        ensure!(
            control.identity() == &identity && control.request_id() == request_id,
            "cancel association changed"
        );
        write_frame(
            &mut stream,
            &ServerFrame::Response {
                identity: identity.clone(),
                request_id,
                envelope: ResponseEnvelope {
                    diagnostics: Some(crate::failure_diagnostics::sample()),
                    id,
                    data: None,
                    error: Some("operation canceled after confirmed restoration".to_owned()),
                    fatal: false,
                },
            },
        )
        .await?;
        close(&mut stream, &identity, CloseSessionReceipt::confirmed()).await
    });
    let result = simulation.client().request(
        request(31),
        &|| cancel.load(Ordering::Acquire),
        Duration::from_secs(2),
    );
    let error = result.err().unwrap();
    let canceled = error.downcast_ref::<CanceledRequest>().unwrap();
    assert_eq!(canceled.response.id, 31);
    assert!(canceled.response.data.is_none());
    assert_eq!(
        canceled.response.error.as_deref(),
        Some("operation canceled after confirmed restoration")
    );
    let Some(crate::FailureDiagnostics::MixerHardware(snapshot)) = &canceled.response.diagnostics
    else {
        panic!("broker discarded the actual canceled response diagnostic")
    };
    assert_eq!(snapshot.sliders, [17, 29, 43, 61]);
    assert!(simulation.client().close_confirmation().is_none());
    simulation.client().close_session().unwrap();
    simulation.finish();
}

#[test]
fn timeout_remains_unconfirmed_until_late_response_and_cleanup_are_drained() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        let (request_id, id) = receive_request(&mut stream, &identity).await?;
        let control: ClientFrame = read_frame(&mut stream).await?;
        ensure!(
            matches!(control, ClientFrame::Cancel { .. }),
            "timeout did not send cancel"
        );
        ensure!(
            control.request_id() == request_id,
            "cancel did not identify pending request"
        );
        // Wait for close before returning the old request so deadline expiry is
        // deterministic and requires no scheduling delay in the mock device.
        let closing: ClientFrame = read_frame(&mut stream).await?;
        ensure!(
            matches!(closing, ClientFrame::CloseSession { .. }),
            "expected cleanup after timeout"
        );
        write_frame(&mut stream, &reply(identity.clone(), request_id, id)).await?;
        write_frame(
            &mut stream,
            &ServerFrame::SessionClosed {
                identity,
                request_id: closing.request_id(),
                receipt: CloseSessionReceipt::confirmed(),
            },
        )
        .await
    });
    let result = simulation
        .client()
        .request(request(32), &|| false, Duration::from_millis(30));
    assert!(result.err().unwrap().to_string().contains("timed out"));
    assert!(simulation.client().close_confirmation().is_none());
    assert!(
        simulation
            .client()
            .request(request(33), &|| false, Duration::from_secs(2))
            .is_err()
    );
    simulation.client().close_session().unwrap();
    simulation.finish();
}

#[test]
fn cancel_write_without_terminal_device_response_is_eof_failure() {
    let cancel = Arc::new(AtomicBool::new(false));
    let server_cancel = cancel.clone();
    let mut simulation = Simulation::new(move |mut stream, identity| async move {
        let _ = receive_request(&mut stream, &identity).await?;
        server_cancel.store(true, Ordering::Release);
        let frame: ClientFrame = read_frame(&mut stream).await?;
        ensure!(
            matches!(frame, ClientFrame::Cancel { .. }),
            "expected cancel"
        );
        Ok(())
    });
    let result = simulation.client().request(
        request(34),
        &|| cancel.load(Ordering::Acquire),
        Duration::from_secs(2),
    );
    let error = result.err().unwrap();
    assert!(error.downcast_ref::<CanceledRequest>().is_none());
    assert!(simulation.client().close_confirmation().is_none());
    assert!(simulation.client().close_session().is_err());
    simulation.finish();
}

#[test]
fn client_rejects_wrong_inner_request_id_even_with_matching_session_and_uuid() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        let (request_id, id) = receive_request(&mut stream, &identity).await?;
        write_frame(&mut stream, &reply(identity, request_id, id + 1)).await
    });
    assert!(
        simulation
            .client()
            .request(request(35), &|| false, Duration::from_secs(2))
            .is_err()
    );
    assert!(simulation.client().close_confirmation().is_none());
    simulation.finish();
}

#[test]
fn client_rejects_missing_conflicting_fatal_success_and_success_with_diagnostics_outcomes() {
    for (data, error, fatal, diagnostics) in [
        (None, None, false, None),
        (
            Some(serde_json::json!({"observed": 137})),
            Some("failed".to_owned()),
            false,
            None,
        ),
        (Some(serde_json::json!({"observed": 137})), None, true, None),
        (
            Some(serde_json::json!({"observed": 137})),
            None,
            false,
            Some(crate::failure_diagnostics::sample()),
        ),
    ] {
        let mut simulation = Simulation::new(move |mut stream, identity| async move {
            let (request_id, id) = receive_request(&mut stream, &identity).await?;
            write_frame(
                &mut stream,
                &ServerFrame::Response {
                    identity,
                    request_id,
                    envelope: ResponseEnvelope {
                        diagnostics,
                        id,
                        data,
                        error,
                        fatal,
                    },
                },
            )
            .await
        });
        let failure = simulation
            .client()
            .request(request(35), &|| false, Duration::from_secs(2));
        assert!(failure.is_err());
        assert!(simulation.client().close_confirmation().is_none());
        simulation.finish();
    }
}

#[test]
fn client_rejects_replaced_instance_and_stale_uuid() {
    for replace_instance in [false, true] {
        let mut simulation = Simulation::new(move |mut stream, mut identity| async move {
            let (mut request_id, id) = receive_request(&mut stream, &identity).await?;
            if replace_instance {
                identity.instance = BrokerInstanceId::new();
            } else {
                request_id = BrokerRequestId::new();
            }
            write_frame(&mut stream, &reply(identity, request_id, id)).await
        });
        assert!(
            simulation
                .client()
                .request(request(36), &|| false, Duration::from_secs(2))
                .is_err()
        );
        assert!(simulation.client().close_confirmation().is_none());
        simulation.finish();
    }
}

#[test]
fn duplicate_completed_request_uuid_cannot_satisfy_the_next_request() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        let (request_id, id) = receive_request(&mut stream, &identity).await?;
        write_frame(&mut stream, &reply(identity.clone(), request_id, id)).await?;
        let _ = receive_request(&mut stream, &identity).await?;
        write_frame(&mut stream, &reply(identity, request_id, id)).await
    });
    simulation
        .client()
        .request(request(37), &|| false, Duration::from_secs(2))
        .unwrap();
    assert!(
        simulation
            .client()
            .request(request(38), &|| false, Duration::from_secs(2))
            .is_err()
    );
    simulation.finish();
}

#[test]
fn session_cleanup_failure_is_retained_as_failure() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        close(
            &mut stream,
            &identity,
            CloseSessionReceipt::pending("restore failed".to_owned()),
        )
        .await
    });
    assert!(
        simulation
            .client()
            .close_session()
            .err()
            .unwrap()
            .to_string()
            .contains("restore failed")
    );
    assert!(simulation.client().close_confirmation().is_none());
    simulation.finish();
}

#[test]
fn released_restoration_failure_is_cached_without_repeating_close_or_claiming_success() {
    let receipt = CloseSessionReceipt::restoration_failed("native restore failed".to_owned());
    let server_receipt = receipt.clone();
    let mut simulation = Simulation::new(move |mut stream, identity| async move {
        close(&mut stream, &identity, server_receipt).await
    });
    let first_error = simulation.client().close_session().unwrap_err().to_string();
    assert!(first_error.contains("native restore failed"));
    assert_eq!(simulation.client().close_receipt(), Some(&receipt));
    assert!(simulation.client().close_confirmation().is_none());
    for _ in 0..2 {
        assert_eq!(
            simulation.client().close_session().unwrap_err().to_string(),
            first_error
        );
    }
    assert!(
        simulation
            .client()
            .request(request(39), &|| false, Duration::from_secs(2))
            .err()
            .unwrap()
            .to_string()
            .contains("no longer accepts")
    );
    simulation.finish();
}

#[test]
fn session_cleanup_failure_allows_a_new_explicit_close_with_a_fresh_uuid() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        let mut previous = None;
        for receipt in [
            CloseSessionReceipt::pending("restore failed".to_owned()),
            CloseSessionReceipt::confirmed(),
        ] {
            let frame: ClientFrame = read_frame(&mut stream).await?;
            ensure!(
                frame.identity() == &identity,
                "session owner changed during retry"
            );
            let ClientFrame::CloseSession { request_id, .. } = frame else {
                bail!("expected session close");
            };
            ensure!(
                Some(request_id) != previous,
                "explicit retry reused the completed UUID"
            );
            previous = Some(request_id);
            write_frame(
                &mut stream,
                &ServerFrame::SessionClosed {
                    identity: identity.clone(),
                    request_id,
                    receipt,
                },
            )
            .await?;
        }
        Ok(())
    });
    assert!(simulation.client().close_session().is_err());
    assert!(simulation.client().close_confirmation().is_none());
    simulation.client().close_session().unwrap();
    simulation.finish();
}

#[test]
fn close_timeout_keeps_the_same_pending_close_without_resending_a_new_command() {
    let (resume, resumed) = std::sync::mpsc::channel();
    let mut simulation = Simulation::new(move |mut stream, identity| async move {
        let frame: ClientFrame = read_frame(&mut stream).await?;
        let ClientFrame::CloseSession { request_id, .. } = frame else {
            bail!("expected close");
        };
        resumed.recv().unwrap();
        write_frame(
            &mut stream,
            &ServerFrame::SessionClosed {
                identity,
                request_id,
                receipt: CloseSessionReceipt::confirmed(),
            },
        )
        .await?;
        // If retry sent another CloseSession, it would be observed here.
        use tokio::io::AsyncReadExt;
        let mut byte = [0; 1];
        ensure!(
            stream.read(&mut byte).await? == 0,
            "close retry sent an extra command"
        );
        Ok(())
    });
    super::client::set_control_timeout(simulation.client(), Duration::from_millis(30));
    assert!(simulation.client().close_session().is_err());
    assert!(simulation.client().close_confirmation().is_none());
    resume.send(()).unwrap();
    super::client::set_control_timeout(simulation.client(), Duration::from_secs(2));
    simulation.client().close_session().unwrap();
    simulation.finish();
}

#[test]
fn a_device_success_response_cannot_confirm_session_cleanup() {
    let mut simulation = Simulation::new(|mut stream, identity| async move {
        let frame: ClientFrame = read_frame(&mut stream).await?;
        ensure!(
            matches!(frame, ClientFrame::CloseSession { .. }),
            "expected close"
        );
        write_frame(&mut stream, &reply(identity, frame.request_id(), 39)).await
    });
    assert!(simulation.client().close_session().is_err());
    assert!(simulation.client().close_confirmation().is_none());
    simulation.finish();
}

#[test]
fn installation_endpoint_is_stable_across_clients_without_random_process_identity() {
    let executable = std::env::current_exe().unwrap();
    let first = BrokerEndpoint::for_installation(&executable).unwrap();
    let second = BrokerEndpoint::for_installation(&executable).unwrap();
    assert_eq!(first.id(), second.id());
    #[cfg(windows)]
    assert_eq!(first.as_windows_pipe(), second.as_windows_pipe());
    #[cfg(unix)]
    assert_eq!(first.as_unix_path(), second.as_unix_path());
}
