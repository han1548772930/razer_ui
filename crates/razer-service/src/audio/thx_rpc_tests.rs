//! HTTP/2/gRPC tests use only Tokio in-memory duplex streams.
use super::*;
use razer_device::thx_v4_proto::{GrpcFrameDecoder, Property, PropertyType, encode_grpc_frame};
use std::{io, pin::Pin, sync::atomic::AtomicUsize, task::Context};
use tokio::io::{DuplexStream, ReadBuf};

struct TrackedIo {
    inner: DuplexStream,
    dropped: Arc<AtomicUsize>,
    cancel_on_drop: Option<Cancellation>,
}
impl Drop for TrackedIo {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
        if let Some(cancel) = &self.cancel_on_drop {
            cancel.cancel();
        }
    }
}
impl AsyncRead for TrackedIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        out: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, out)
    }
}
impl AsyncWrite for TrackedIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
}
fn get() -> Request {
    Request::GetProperty {
        id: "spatial_enabled".into(),
        endpoint_id: Some("headphones_game".into()),
    }
}
fn property() -> Property {
    Property {
        id: "spatial_enabled".into(),
        property_type: PropertyType::Bool,
        bool_value: Some(true),
        ..Default::default()
    }
}
#[derive(Clone)]
struct Fixture {
    http_status: u16,
    content_type: Option<&'static str>,
    bytes: Vec<u8>,
    status: Option<&'static str>,
    header_status: Option<&'static str>,
    duplicate_status: bool,
    fragment: usize,
    window: u32,
    stall: bool,
    reset: bool,
}
impl Fixture {
    fn ok(response: Response) -> Self {
        Self {
            http_status: 200,
            content_type: Some("application/grpc"),
            bytes: encode_grpc_frame(&response.encode().unwrap()).unwrap(),
            status: Some("0"),
            header_status: None,
            duplicate_status: false,
            fragment: 2,
            window: 65535,
            stall: false,
            reset: false,
        }
    }
}
async fn fake_server(
    io: DuplexStream,
    expected: Request,
    fixture: Fixture,
    received: tokio::sync::oneshot::Sender<()>,
) {
    let mut builder = h2::server::Builder::new();
    builder.initial_window_size(fixture.window);
    let mut connection = builder.handshake::<_, Bytes>(io).await.unwrap();
    let (request, mut reply) = connection.accept().await.unwrap().unwrap();
    let handler = tokio::spawn(async move {
        assert_eq!(request.method(), http::Method::POST);
        assert_eq!(request.uri().path(), expected.method().path());
        assert_eq!(request.headers()["content-type"], "application/grpc");
        assert_eq!(request.headers()["te"], "trailers");
        let mut body = request.into_body();
        let mut decoder = GrpcFrameDecoder::new();
        let mut messages = vec![];
        while let Some(chunk) = body.data().await {
            let chunk = chunk.unwrap();
            messages.extend(decoder.push(&chunk).unwrap());
            body.flow_control().release_capacity(chunk.len()).unwrap();
        }
        decoder.finish().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            Request::decode(expected.method(), &messages[0]).unwrap(),
            expected
        );
        let _ = received.send(());
        if fixture.stall {
            std::future::pending::<()>().await;
        }
        if fixture.reset {
            reply.send_reset(h2::Reason::INTERNAL_ERROR);
            return;
        }
        let mut response = http::Response::builder().status(fixture.http_status);
        if let Some(value) = fixture.content_type {
            response = response.header("content-type", value);
        }
        if let Some(value) = fixture.header_status {
            response = response.header("grpc-status", value);
        }
        let trailers_only =
            fixture.bytes.is_empty() && fixture.header_status.is_some() && fixture.status.is_none();
        let mut stream = reply
            .send_response(response.body(()).unwrap(), trailers_only)
            .unwrap();
        if trailers_only {
            return;
        }
        for (i, chunk) in fixture.bytes.chunks(fixture.fragment).enumerate() {
            let final_chunk = (i + 1) * fixture.fragment >= fixture.bytes.len();
            stream
                .send_data(
                    Bytes::copy_from_slice(chunk),
                    final_chunk && fixture.status.is_none(),
                )
                .unwrap();
        }
        if let Some(status) = fixture.status {
            let mut trailers = http::HeaderMap::new();
            trailers.append("grpc-status", status.parse().unwrap());
            if fixture.duplicate_status {
                trailers.append("grpc-status", "0".parse().unwrap());
            }
            stream.send_trailers(trailers).unwrap();
        }
    });
    // accept polls the server connection and flushes DATA/trailers. The client
    // closes its owned duplex after unary completes.
    if let Some(incoming) = connection.accept().await {
        assert!(incoming.is_err(), "Unexpected retry/duplicate THX request");
    }
    // Cancellation may leave a deliberately pending fake handler.
    if !handler.is_finished() {
        handler.abort();
    }
}
fn call(expected: Request, fixture: Fixture) -> Result<Response> {
    call_with_completion_cancel(expected, fixture, None)
}
fn call_with_completion_cancel(
    expected: Request,
    fixture: Fixture,
    cancel_on_drop: Option<Cancellation>,
) -> Result<Response> {
    runtime().block_on(async {
        let (client, server) = tokio::io::duplex(256);
        let dropped = Arc::new(AtomicUsize::new(0));
        let (sent, received) = tokio::sync::oneshot::channel();
        let serving = tokio::spawn(fake_server(server, expected.clone(), fixture, sent));
        let cancel = cancel_on_drop.clone().unwrap_or_default();
        let result = unary(
            TrackedIo {
                inner: client,
                dropped: dropped.clone(),
                cancel_on_drop,
            },
            &expected,
            Duration::from_secs(2),
            &cancel,
        )
        .await;
        assert_eq!(
            dropped.load(Ordering::SeqCst),
            1,
            "owned client connection not closed"
        );
        received.await.unwrap();
        serving.await.unwrap();
        result
    })
}
#[test]
fn fragmented_getter_preserves_endpoint_and_property() {
    let expected = Response::GetProperty(property());
    assert_eq!(
        call(get(), Fixture::ok(expected.clone())).unwrap(),
        expected
    );
}
#[test]
fn setter_requires_real_empty_message_and_success_trailers() {
    let request = Request::SetProperty {
        property: Some(property()),
        endpoint_id: Some(String::new()),
    };
    assert_eq!(
        call(request, Fixture::ok(Response::SetProperty)).unwrap(),
        Response::SetProperty
    );
}

#[test]
fn completed_setter_ack_survives_cancellation_during_connection_drop() {
    let cancel = Cancellation::default();
    let result = call_with_completion_cancel(
        Request::SetProperty {
            property: Some(property()),
            endpoint_id: Some("headphones_game".into()),
        },
        Fixture::ok(Response::SetProperty),
        Some(cancel.clone()),
    );
    // The operation-owned I/O drops before unary returns. This reproduces the
    // response/cancel race without timers or any OS socket. The caller's guard
    // must see cancellation, while the actual completed response stays intact.
    assert!(cancel.check().is_err());
    assert_eq!(result.unwrap(), Response::SetProperty);
}

#[test]
fn completed_rpc_rejection_survives_cancellation_during_connection_drop() {
    let cancel = Cancellation::default();
    let mut fixture = Fixture::ok(Response::SetProperty);
    fixture.status = Some("7");
    let result = call_with_completion_cancel(
        Request::SetProperty {
            property: Some(property()),
            endpoint_id: Some("headphones_game".into()),
        },
        fixture,
        Some(cancel.clone()),
    );
    assert!(cancel.check().is_err());
    let error = format!("{:#}", result.unwrap_err());
    assert!(error.contains("GrpcStatus"), "{error}");
    assert!(!error.contains("canceled"), "{error}");
}
#[test]
fn small_peer_window_and_large_request_complete_without_unbounded_send() {
    let mut p = property();
    p.property_type = PropertyType::String;
    p.bool_value = None;
    p.string_value = Some("x".repeat(90_000));
    let request = Request::SetProperty {
        property: Some(p),
        endpoint_id: Some(String::new()),
    };
    let mut fixture = Fixture::ok(Response::SetProperty);
    fixture.window = 31;
    assert_eq!(call(request, fixture).unwrap(), Response::SetProperty);
}
#[test]
fn service_status_error_is_never_setter_ack() {
    let mut fixture = Fixture::ok(Response::SetProperty);
    fixture.status = Some("7");
    let error = call(
        Request::SetProperty {
            property: Some(property()),
            endpoint_id: Some(String::new()),
        },
        fixture,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("GrpcStatus"));
}
#[test]
fn http_200_without_grpc_status_is_failure() {
    let mut fixture = Fixture::ok(Response::GetProperty(property()));
    fixture.status = None;
    assert!(format!("{:#}", call(get(), fixture).unwrap_err()).contains("MissingTrailers"));
}
#[test]
fn non_200_and_wrong_content_type_reject_complete_body() {
    for (status, content_type) in [
        (503, Some("application/grpc")),
        (200, Some("application/json")),
        (200, None),
    ] {
        let mut fixture = Fixture::ok(Response::GetProperty(property()));
        fixture.http_status = status;
        fixture.content_type = content_type;
        assert!(call(get(), fixture).is_err());
    }
}
#[test]
fn trailers_only_error_is_reported_but_empty_success_is_not_invented() {
    for status in ["0", "14"] {
        let mut fixture = Fixture::ok(Response::SetProperty);
        fixture.bytes.clear();
        fixture.header_status = Some(status);
        fixture.status = None;
        let text = format!("{:#}", call(get(), fixture).unwrap_err());
        assert!(
            text.contains(if status == "0" {
                "UnaryFrameCount"
            } else {
                "GrpcStatus"
            }),
            "{text}"
        );
    }
}
#[test]
fn duplicate_status_and_header_trailer_conflict_reject_response() {
    let mut fixture = Fixture::ok(Response::GetProperty(property()));
    fixture.duplicate_status = true;
    assert!(call(get(), fixture).is_err());
    let mut fixture = Fixture::ok(Response::GetProperty(property()));
    fixture.header_status = Some("0");
    assert!(call(get(), fixture).is_err());
}
#[test]
fn truncated_compressed_and_multiple_frames_are_failures() {
    for bytes in [
        vec![0, 0, 0, 0, 1],
        vec![1, 0, 0, 0, 0],
        vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![0, 0xff, 0xff, 0xff, 0xff],
    ] {
        let mut fixture = Fixture::ok(Response::GetProperty(property()));
        fixture.bytes = bytes;
        assert!(call(get(), fixture).is_err());
    }
}
#[test]
fn reset_does_not_retry_or_return_ack() {
    let mut fixture = Fixture::ok(Response::GetProperty(property()));
    fixture.reset = true;
    assert!(call(get(), fixture).is_err());
}
#[test]
fn deadline_closes_connection_after_request_without_setter_retry() {
    runtime().block_on(async {
        let (client, server) = tokio::io::duplex(256);
        let dropped = Arc::new(AtomicUsize::new(0));
        let (sent, received) = tokio::sync::oneshot::channel();
        let mut fixture = Fixture::ok(Response::SetProperty);
        fixture.stall = true;
        let request = Request::SetProperty {
            property: Some(property()),
            endpoint_id: Some("headphones_game".into()),
        };
        let serving = tokio::spawn(fake_server(server, request.clone(), fixture, sent));
        let result = unary(
            TrackedIo {
                inner: client,
                dropped: dropped.clone(),
                cancel_on_drop: None,
            },
            &request,
            Duration::from_millis(50),
            &Cancellation::default(),
        )
        .await;
        received.await.unwrap();
        assert!(format!("{:#}", result.unwrap_err()).contains("deadline"));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        serving.await.unwrap();
    });
}
#[test]
fn cancellation_during_response_closes_owned_io() {
    runtime().block_on(async {
        let (client, server) = tokio::io::duplex(256);
        let dropped = Arc::new(AtomicUsize::new(0));
        let (sent, received) = tokio::sync::oneshot::channel();
        let cancel = Cancellation::default();
        let trigger = cancel.clone();
        let request = get();
        let mut fixture = Fixture::ok(Response::GetProperty(property()));
        fixture.stall = true;
        let serving = tokio::spawn(fake_server(server, request.clone(), fixture, sent));
        let cancelling = tokio::spawn(async move {
            received.await.unwrap();
            trigger.cancel();
        });
        let result = unary(
            TrackedIo {
                inner: client,
                dropped: dropped.clone(),
                cancel_on_drop: None,
            },
            &request,
            Duration::from_secs(2),
            &cancel,
        )
        .await;
        assert!(format!("{:#}", result.unwrap_err()).contains("canceled"));
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        cancelling.await.unwrap();
        serving.await.unwrap();
    });
}
#[test]
fn canceled_or_streaming_request_performs_no_handshake() {
    runtime().block_on(async {
        for streaming in [false, true] {
            let (client, mut server) = tokio::io::duplex(256);
            let dropped = Arc::new(AtomicUsize::new(0));
            let cancel = Cancellation::default();
            if !streaming {
                cancel.cancel();
            }
            let request = if streaming {
                Request::SubscribeToPropertyChanges {
                    endpoint_id: Some(String::new()),
                }
            } else {
                get()
            };
            assert!(
                unary(
                    TrackedIo {
                        inner: client,
                        dropped: dropped.clone(),
                        cancel_on_drop: None,
                    },
                    &request,
                    Duration::from_secs(1),
                    &cancel
                )
                .await
                .is_err()
            );
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
            let mut buf = [0; 32];
            assert_eq!(
                tokio::io::AsyncReadExt::read(&mut server, &mut buf)
                    .await
                    .unwrap(),
                0
            );
        }
    });
}
