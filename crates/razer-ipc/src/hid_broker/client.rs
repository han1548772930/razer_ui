use super::{
    BrokerEndpoint, BrokerInstanceId, BrokerRequestId, ClientFrame, ClientHello,
    CloseSessionReceipt, FrameReader, FrameWriter, ServerFrame, ServerHello, SessionIdentity,
    read_frame, write_frame,
};
use crate::{RequestEnvelope, ResponseEnvelope};
use anyhow::{Context as _, bail, ensure};
use std::{fmt, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf},
    runtime::{Builder, Runtime},
    time::{Instant, timeout, timeout_at},
};

trait LocalStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> LocalStream for T {}
type Stream = Box<dyn LocalStream>;

#[derive(Debug)]
pub(super) struct MissingEndpoint;

impl fmt::Display for MissingEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("shared broker local endpoint does not exist")
    }
}
impl std::error::Error for MissingEndpoint {}

fn connect_error(error: std::io::Error) -> anyhow::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        anyhow::Error::new(error).context(MissingEndpoint)
    } else {
        anyhow::Error::new(error).context("shared broker local endpoint could not be connected")
    }
}

/// Cancellation retains the actual response and never turns a cancel write into
/// proof that a device operation stopped or that its recovery completed.
#[non_exhaustive]
pub struct CanceledRequest {
    pub response: ResponseEnvelope,
}

impl CanceledRequest {
    pub fn new(response: ResponseEnvelope) -> Self {
        Self { response }
    }
}

impl fmt::Debug for CanceledRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanceledRequest")
            .field("response_id", &self.response.id)
            .field("error", &self.response.error)
            .field("fatal", &self.response.fatal)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CanceledRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "broker request was canceled; its actual response was received"
        )?;
        if let Some(error) = &self.response.error {
            write!(formatter, ": {error}")?;
        }
        Ok(())
    }
}
impl std::error::Error for CanceledRequest {}

#[derive(Clone, Copy)]
struct PendingRequest {
    request_id: BrokerRequestId,
    envelope_id: u64,
}

/// Blocking transport for a caller-owned I/O worker. This owner creates no
/// process, job, task or independent broker lifetime; dropping it closes only
/// this session's stream. Explicit close waits for the broker's cleanup receipt.
pub struct BrokerClient {
    runtime: Runtime,
    reader: FrameReader<ReadHalf<Stream>>,
    writer: FrameWriter<WriteHalf<Stream>>,
    identity: SessionIdentity,
    control_timeout: Duration,
    accepting_requests: bool,
    pending: Option<PendingRequest>,
    close_request: Option<BrokerRequestId>,
    closed: Option<CloseSessionReceipt>,
}

impl BrokerClient {
    pub fn connect(
        endpoint: &BrokerEndpoint,
        expected_instance: Option<BrokerInstanceId>,
        connect_timeout: Duration,
    ) -> anyhow::Result<Self> {
        ensure!(
            !connect_timeout.is_zero(),
            "broker connection timeout must be positive"
        );
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .context("broker client I/O runtime could not be created")?;
        let (stream, identity) = runtime.block_on(async {
            timeout(connect_timeout, async {
                let mut stream = connect_stream(endpoint).await?;
                let hello = ClientHello::new(endpoint.id().clone(), expected_instance);
                write_frame(&mut stream, &hello).await?;
                let response: ServerHello = read_frame(&mut stream).await?;
                response.validate(&hello)?;
                Ok::<_, anyhow::Error>((stream, response.identity))
            })
            .await
            .context("broker connection or handshake timed out")?
        })?;
        Ok(Self::from_stream(
            runtime,
            stream,
            identity,
            connect_timeout,
        ))
    }

    fn from_stream(
        runtime: Runtime,
        stream: Stream,
        identity: SessionIdentity,
        control_timeout: Duration,
    ) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        Self {
            runtime,
            reader: FrameReader::new(reader),
            writer: FrameWriter::new(writer),
            identity,
            control_timeout,
            accepting_requests: true,
            pending: None,
            close_request: None,
            closed: None,
        }
    }

    pub fn identity(&self) -> &SessionIdentity {
        &self.identity
    }

    /// The broker's terminal resource receipt, including restoration failure.
    /// A pending cleanup or transport failure cannot establish release.
    pub fn close_receipt(&self) -> Option<&CloseSessionReceipt> {
        self.closed.as_ref()
    }

    pub fn close_confirmation(&self) -> Option<&CloseSessionReceipt> {
        self.closed
            .as_ref()
            .filter(|receipt| receipt.validate().is_ok())
    }

    pub fn request(
        &mut self,
        envelope: RequestEnvelope,
        cancellation: &dyn Fn() -> bool,
        request_timeout: Duration,
    ) -> anyhow::Result<ResponseEnvelope> {
        ensure!(
            self.accepting_requests && self.closed.is_none(),
            "broker session no longer accepts requests"
        );
        ensure!(
            self.pending.is_none(),
            "broker session still owns an unresolved request"
        );
        ensure!(
            !request_timeout.is_zero(),
            "broker request timeout must be positive"
        );
        ensure!(
            !cancellation(),
            "broker request was canceled before submission"
        );
        let pending = PendingRequest {
            request_id: BrokerRequestId::new(),
            envelope_id: envelope.id,
        };
        self.pending = Some(pending);
        let identity = self.identity.clone();
        let reader = &mut self.reader;
        let writer = &mut self.writer;
        let control_timeout = self.control_timeout;
        let result = self.runtime.block_on(async {
            let deadline = Instant::now() + request_timeout;
            let frame = ClientFrame::Request {
                identity: identity.clone(),
                request_id: pending.request_id,
                envelope,
            };
            timeout_at(deadline, writer.write(&frame))
                .await
                .context("broker request write timed out")??;
            await_response(
                reader,
                writer,
                &identity,
                pending,
                cancellation,
                deadline,
                control_timeout,
            )
            .await
        });
        match result {
            Ok((response, canceled)) => {
                self.pending = None;
                if response.fatal {
                    self.accepting_requests = false;
                }
                if canceled {
                    Err(CanceledRequest { response }.into())
                } else {
                    Ok(response)
                }
            }
            Err(error) => {
                self.accepting_requests = false;
                Err(error)
            }
        }
    }

    pub fn close_session(&mut self) -> anyhow::Result<CloseSessionReceipt> {
        if let Some(receipt) = &self.closed {
            receipt.validate()?;
            return Ok(receipt.clone());
        }
        self.accepting_requests = false;
        let identity = self.identity.clone();
        let send_close = self.close_request.is_none();
        let request_id = *self.close_request.get_or_insert_with(BrokerRequestId::new);
        let reader = &mut self.reader;
        let writer = &mut self.writer;
        let pending_request = &mut self.pending;
        let control_timeout = self.control_timeout;
        let receipt = self.runtime.block_on(async {
            let deadline = Instant::now() + control_timeout;
            if send_close {
                let frame = ClientFrame::CloseSession {
                    identity: identity.clone(),
                    request_id,
                };
                timeout_at(deadline, writer.write(&frame))
                    .await
                    .context("broker session close write timed out; cleanup is unconfirmed")??;
            } else {
                timeout_at(deadline, writer.flush_pending())
                    .await
                    .context(
                        "broker pending session close write timed out; cleanup is unconfirmed",
                    )??;
            }
            loop {
                let response: ServerFrame = timeout_at(deadline, reader.read())
                    .await
                    .context("broker session cleanup timed out; cleanup is unconfirmed")??;
                ensure!(
                    response.identity() == &identity,
                    "broker close response session identity mismatch"
                );
                if let Some(pending) = *pending_request {
                    if response.request_id() == pending.request_id {
                        match response {
                            ServerFrame::Response { envelope, .. } => {
                                validate_response(&envelope, pending.envelope_id)?;
                                *pending_request = None;
                                continue;
                            }
                            ServerFrame::SessionClosed { .. } => {
                                bail!("broker request UUID was reused for a close receipt")
                            }
                        }
                    }
                }
                response.validate(&identity, request_id)?;
                match response {
                    ServerFrame::SessionClosed { receipt, .. } => {
                        return Ok::<_, anyhow::Error>(receipt);
                    }
                    ServerFrame::Response { .. } => {
                        bail!("broker returned a device response for session close")
                    }
                }
            }
        })?;
        self.close_request = None;
        if receipt.resources_released() {
            self.closed = Some(receipt.clone());
        }
        receipt.validate()?;
        Ok(receipt)
    }
}

async fn await_response(
    reader: &mut FrameReader<ReadHalf<Stream>>,
    writer: &mut FrameWriter<WriteHalf<Stream>>,
    identity: &SessionIdentity,
    pending: PendingRequest,
    cancellation: &dyn Fn() -> bool,
    deadline: Instant,
    control_timeout: Duration,
) -> anyhow::Result<(ResponseEnvelope, bool)> {
    let response = reader.read::<ServerFrame>();
    tokio::pin!(response);
    let mut check_cancellation = tokio::time::interval(Duration::from_millis(10));
    check_cancellation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut canceled = false;
    loop {
        tokio::select! {
            result = &mut response => {
                let response = result?;
                response.validate(identity, pending.request_id)?;
                match response {
                    ServerFrame::Response { envelope, .. } => {
                        validate_response(&envelope, pending.envelope_id)?;
                        return Ok((envelope, canceled || cancellation()));
                    }
                    ServerFrame::SessionClosed { .. } => bail!("broker session closed before its request response"),
                }
            }
            _ = check_cancellation.tick(), if !canceled => {
                if cancellation() {
                    let frame = ClientFrame::Cancel { identity: identity.clone(), request_id: pending.request_id };
                    timeout_at(deadline, writer.write(&frame)).await
                        .context("broker cancel write timed out; device termination is unconfirmed")??;
                    canceled = true;
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                if !canceled {
                    let frame = ClientFrame::Cancel { identity: identity.clone(), request_id: pending.request_id };
                    match timeout(control_timeout, writer.write(&frame)).await {
                        Ok(Ok(())) => {},
                        Ok(Err(error)) => return Err(error).context("broker request timed out and cancel write failed; device termination is unconfirmed"),
                        Err(_) => bail!("broker request and cancel write timed out; device termination is unconfirmed"),
                    }
                }
                bail!("broker request timed out; no device response or cleanup confirmation was received");
            }
        }
    }
}

fn validate_response(response: &ResponseEnvelope, envelope_id: u64) -> anyhow::Result<()> {
    ensure!(
        response.id == envelope_id,
        "broker response envelope request ID mismatch"
    );
    response.validate_outcome()
}

#[cfg(windows)]
async fn connect_stream(endpoint: &BrokerEndpoint) -> anyhow::Result<Stream> {
    use tokio::net::windows::named_pipe::ClientOptions;
    loop {
        match ClientOptions::new().open(endpoint.as_windows_pipe()) {
            Ok(pipe) => {
                endpoint.verify_windows_server(&pipe)?;
                return Ok(Box::new(pipe));
            }
            Err(error)
                if error.raw_os_error()
                    == Some(windows_sys::Win32::Foundation::ERROR_PIPE_BUSY as i32) =>
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => return Err(connect_error(error)),
        }
    }
}

#[cfg(unix)]
async fn connect_stream(endpoint: &BrokerEndpoint) -> anyhow::Result<Stream> {
    let stream = tokio::net::UnixStream::connect(endpoint.as_unix_path())
        .await
        .map_err(connect_error)?;
    endpoint.verify_unix_peer(&stream)?;
    Ok(Box::new(stream))
}

#[cfg(test)]
pub(crate) fn simulated_client(
    runtime: Runtime,
    stream: tokio::io::DuplexStream,
    identity: SessionIdentity,
    timeout: Duration,
) -> BrokerClient {
    BrokerClient::from_stream(runtime, Box::new(stream), identity, timeout)
}

#[cfg(test)]
pub(super) fn set_control_timeout(client: &mut BrokerClient, timeout: Duration) {
    client.control_timeout = timeout;
}
