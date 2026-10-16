//! Direct local THX V4 gRPC. No vendor DLL, target process or remote endpoint.
//!
//! The service paths and messages are recovered from the current descriptor.
//! Each call owns its HTTP/2 connection. Deadline/cancellation drops the stream
//! and connection together; no detached connection task or request retry can
//! turn an uncertain setter into a duplicate write.
use anyhow::{Context as _, Result, bail, ensure};
use bytes::Bytes;
use razer_device::thx_v4_proto::{GrpcTrailers, Request, Response, UnaryGrpcResponse};
use std::{
    future::{Future, poll_fn},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(windows)]
#[path = "thx_rpc/windows_stream.rs"]
mod windows_stream;

#[cfg(any(windows, test))]
#[path = "thx_rpc/socket_lifecycle.rs"]
mod socket_lifecycle;

#[cfg(windows)]
pub use windows_stream::LocalUnixStream;

#[cfg(test)]
#[path = "thx_rpc_tests.rs"]
mod tests;

#[derive(Clone, Default)]
pub struct Cancellation {
    canceled: Arc<AtomicBool>,
    operation: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<()> {
        ensure!(
            !self.canceled.load(Ordering::Acquire),
            "THX request canceled"
        );
        let operation = self
            .operation
            .lock()
            .map_err(|_| anyhow::anyhow!("THX cancellation gate failed"))?;
        ensure!(
            !operation
                .as_ref()
                .is_some_and(|signal| signal.load(Ordering::Acquire)),
            "THX operation canceled"
        );
        Ok(())
    }
    /// The owner's lifetime flag stays independent of its current job. A
    /// canceled/completed job can never poison the following operation.
    pub(super) fn owner_gate(owner: &Self) -> Self {
        Self {
            canceled: owner.canceled.clone(),
            operation: Arc::new(Mutex::new(None)),
        }
    }
    pub(super) fn begin_operation(&self, signal: &Self) -> Result<()> {
        ensure!(
            !Arc::ptr_eq(&self.canceled, &signal.canceled),
            "THX job cannot own the lifetime gate"
        );
        *self
            .operation
            .lock()
            .map_err(|_| anyhow::anyhow!("THX cancellation gate failed"))? =
            Some(signal.canceled.clone());
        Ok(())
    }
    pub(super) fn end_operation(&self) -> Result<()> {
        *self
            .operation
            .lock()
            .map_err(|_| anyhow::anyhow!("THX cancellation gate failed"))? = None;
        Ok(())
    }
    async fn wait(&self) {
        while self.check().is_ok() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// A bounded unary exchange over caller-owned bytes. Tests use only duplex
/// memory streams. Runtime OS adapters select the local AF_UNIX path.
pub async fn unary<I>(
    io: I,
    request: &Request,
    deadline: Duration,
    cancel: &Cancellation,
) -> Result<Response>
where
    I: AsyncRead + AsyncWrite + Unpin + 'static,
{
    cancel.check()?;
    ensure!(
        !request.method().server_streaming(),
        "THX notification requires streaming transport"
    );
    let framed = request.encode_framed()?;
    let operation = async {
        let (send, connection) = h2::client::handshake(io)
            .await
            .context("THX HTTP/2 handshake failed")?;
        let exchange = exchange(send, request, framed);
        tokio::pin!(exchange);
        tokio::pin!(connection);
        // Drive both futures in this scope. When the peer closes after trailers,
        // poll the buffered response once more before treating closure as error.
        poll_fn(|cx| {
            if let Poll::Ready(result) = exchange.as_mut().poll(cx) {
                return Poll::Ready(result);
            }
            match connection.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(closed) => match exchange.as_mut().poll(cx) {
                    Poll::Ready(result) => Poll::Ready(result),
                    Poll::Pending => Poll::Ready(Err(match closed {
                        Ok(()) => anyhow::anyhow!("THX connection closed before complete response"),
                        Err(error) => {
                            anyhow::Error::new(error).context("THX HTTP/2 connection failed")
                        }
                    })),
                },
            }
        })
        .await
    };
    let result = tokio::select! {
        biased;
        result = tokio::time::timeout(deadline, operation) =>
            result.context("THX local RPC deadline exceeded; write outcome may be unknown")?,
        _ = cancel.wait() => Err(anyhow::anyhow!("THX request canceled; write outcome may be unknown")),
    };
    // Keep a fully parsed response/status. DirectThxTransport's post-guard
    // records it together with cancellation and stops subsequent writes.
    result
}

/// Synchronous worker entry point for the local Windows AF_UNIX endpoint. The
/// runtime owns the Tokio executor and stream for exactly one request.
#[cfg(windows)]
pub fn unary_local(
    uri: &str,
    request: &Request,
    deadline: Duration,
    cancel: &Cancellation,
) -> Result<Response> {
    let path = uri.strip_prefix("unix:").unwrap_or(uri);
    cancel.check()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .context("THX local runtime creation failed")?
        .block_on(async {
            tokio::select! {
                biased;
                result = tokio::time::timeout(deadline, async {
                    let stream = LocalUnixStream::connect(path).await?;
                    unary(stream, request, deadline, cancel).await
                }) => result.context("THX local connect/request deadline exceeded")?,
                _ = cancel.wait() => bail!("THX local connect/request canceled"),
            }
        })
}

async fn exchange(
    mut send: h2::client::SendRequest<Bytes>,
    request: &Request,
    framed: Vec<u8>,
) -> Result<Response> {
    poll_fn(|cx| send.poll_ready(cx)).await?;
    // :authority is framing only: the I/O object is already an AF_UNIX stream.
    // It is never DNS-resolved or used for a TCP/Internet connection.
    let head = http::Request::builder()
        .method("POST")
        .version(http::Version::HTTP_2)
        .uri(format!("http://localhost{}", request.method().path()))
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("grpc-accept-encoding", "identity")
        .body(())?;
    let (response, mut body) = send.send_request(head, false)?;
    // Respect HTTP/2 stream and connection windows, including a peer which
    // grants very small windows. send_data alone can buffer beyond the window.
    let mut bytes = Bytes::from(framed);
    while !bytes.is_empty() {
        body.reserve_capacity(bytes.len());
        let capacity = poll_fn(|cx| body.poll_capacity(cx))
            .await
            .context("THX request stream closed during send")??;
        if capacity == 0 {
            continue;
        }
        let chunk = bytes.split_to(capacity.min(bytes.len()));
        body.send_data(chunk, bytes.is_empty())?;
    }
    let response = response.await.context("THX response headers unavailable")?;
    let status = response.status().as_u16();
    let content_type = header_text(response.headers(), "content-type")?;
    let header_trailers = grpc_trailers(response.headers())?;
    let trailers_only = response.body().is_end_stream();
    let mut response = response.into_body();
    let mut decoder = UnaryGrpcResponse::new();
    while let Some(chunk) = response.data().await {
        let chunk = chunk.context("THX response data failed")?;
        decoder.push(&chunk)?;
        response.flow_control().release_capacity(chunk.len())?;
    }
    let trailers = response
        .trailers()
        .await
        .context("THX response trailers failed")?;
    let trailers = match trailers {
        Some(trailers) => {
            ensure!(
                header_trailers.is_none(),
                "THX grpc-status appeared in headers and trailers"
            );
            grpc_trailers(&trailers)?
        }
        None if trailers_only => header_trailers,
        None => None,
    };
    Ok(decoder.finish(
        request.method(),
        status,
        content_type.as_deref(),
        trailers.as_ref(),
    )?)
}

fn header_text(headers: &http::HeaderMap, name: &str) -> Result<Option<String>> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|value| value.to_str().map(str::to_owned))
        .transpose()?;
    ensure!(values.next().is_none(), "Duplicate THX gRPC header: {name}");
    Ok(value)
}
fn grpc_trailers(headers: &http::HeaderMap) -> Result<Option<GrpcTrailers>> {
    let status = header_text(headers, "grpc-status")?;
    let message = header_text(headers, "grpc-message")?;
    match status {
        Some(status) => Ok(Some(GrpcTrailers { status, message })),
        None if message.is_some() => bail!("THX grpc-message without grpc-status"),
        None => Ok(None),
    }
}
