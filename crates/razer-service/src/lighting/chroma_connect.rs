//! The application owns one local Chroma Connect connection. Device workers
//! borrow its typed request bridge, so a dock cannot uninitialize another dock.
use anyhow::{Context as _, ensure};
use razer_device::receiver_lighting::ChromaAttributeReceipt;
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream},
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, client::client_with_config, handshake::HandshakeError};

const ENDPOINT: &str = "ws://127.0.0.1:1337/razer/rzapichromasdk";
const APP_ID: &str = "{9BF2B22C-8F41-42C0-822D-3E811070315C}";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const IO_SLICE: Duration = Duration::from_millis(50);
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const RECONNECT_DELAYS: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(30),
    Duration::from_secs(60),
];

#[derive(Debug)]
struct Canceled;
impl std::fmt::Display for Canceled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Local Chroma Connect request was canceled")
    }
}
impl std::error::Error for Canceled {}
fn guard(validate: &dyn Fn() -> anyhow::Result<()>, deadline: Instant) -> anyhow::Result<()> {
    validate().map_err(|_| Canceled)?;
    ensure!(
        Instant::now() < deadline,
        "Local Chroma Connect request timed out"
    );
    Ok(())
}

trait Channel {
    fn send(
        &mut self,
        text: String,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()>;
    fn receive(
        &mut self,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<String>;
    fn finish(&mut self) -> anyhow::Result<()>;
}

struct PollStream(TcpStream);
fn poll_result<T>(result: io::Result<T>) -> io::Result<T> {
    result.map_err(|error| {
        if error.kind() == io::ErrorKind::TimedOut {
            io::Error::new(io::ErrorKind::WouldBlock, error)
        } else {
            error
        }
    })
}
impl Read for PollStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        poll_result(self.0.read(bytes))
    }
}
impl Write for PollStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        poll_result(self.0.write(bytes))
    }
    fn flush(&mut self) -> io::Result<()> {
        poll_result(self.0.flush())
    }
}
struct Socket(WebSocket<PollStream>);
fn would_block(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(error) if error.kind() == io::ErrorKind::WouldBlock)
}
impl Socket {
    fn connect(validate: &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<Self> {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        guard(validate, deadline)?;
        let stream =
            TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, 1337)), IO_SLICE)?;
        stream.set_read_timeout(Some(IO_SLICE))?;
        stream.set_write_timeout(Some(IO_SLICE))?;
        stream.set_nodelay(true)?;
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES))
            .max_write_buffer_size(MAX_MESSAGE_BYTES);
        let mut result = client_with_config(ENDPOINT, PollStream(stream), Some(config));
        loop {
            guard(validate, deadline)?;
            match result {
                Ok((socket, _)) => return Ok(Self(socket)),
                Err(HandshakeError::Interrupted(handshake)) => result = handshake.handshake(),
                Err(HandshakeError::Failure(error)) => return Err(error.into()),
            }
        }
    }
    fn flush(
        &mut self,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        loop {
            guard(validate, deadline)?;
            match self.0.flush() {
                Ok(()) => return Ok(()),
                Err(error) if would_block(&error) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn observe_connection(
        &mut self,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        // No request is in flight under the application lock. Drain bounded
        // unsolicited notifications so a closed service is noticed while idle.
        for _ in 0..32 {
            guard(validate, deadline)?;
            match self.0.read() {
                Ok(Message::Text(_) | Message::Ping(_) | Message::Pong(_)) => {}
                Ok(Message::Close(_)) => anyhow::bail!("Local Chroma Connect socket closed"),
                Ok(_) => anyhow::bail!("Local Chroma Connect returned a non-text notification"),
                Err(error) if would_block(&error) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}
impl Channel for Socket {
    fn send(
        &mut self,
        text: String,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        guard(validate, deadline)?;
        match self.0.send(Message::Text(text.into())) {
            Ok(()) => Ok(()),
            // Tungstenite retains this frame on WouldBlock. Resending it would
            // duplicate the vendor mutation; flush the accepted frame instead.
            Err(error) if would_block(&error) => self.flush(deadline, validate),
            Err(error) => Err(error.into()),
        }
    }
    fn receive(
        &mut self,
        deadline: Instant,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<String> {
        loop {
            guard(validate, deadline)?;
            match self.0.read() {
                Ok(Message::Text(text)) => return Ok(text.to_string()),
                Ok(Message::Ping(_) | Message::Pong(_)) => {}
                Ok(Message::Close(_)) => anyhow::bail!("Local Chroma Connect socket closed"),
                Ok(_) => anyhow::bail!("Local Chroma Connect returned a non-text response"),
                Err(error) if would_block(&error) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn finish(&mut self) -> anyhow::Result<()> {
        let close = match self.0.close(None) {
            Ok(())
            | Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                Ok(())
            }
            Err(error) if would_block(&error) => {
                self.flush(Instant::now() + REQUEST_TIMEOUT, &|| Ok(()))
            }
            Err(error) => Err(error.into()),
        };
        let shutdown = self.0.get_mut().0.shutdown(Shutdown::Both);
        let shutdown = match shutdown {
            Err(error) if error.kind() == io::ErrorKind::NotConnected => Ok(()),
            result => result,
        };
        close.and(shutdown.map_err(Into::into))
    }
}

#[derive(Serialize)]
struct AppParameters {
    appid: &'static str,
}
#[derive(Serialize)]
struct AttributeParameters {
    pid: u16,
    attribute: u8,
    value: u8,
}
#[derive(Serialize)]
struct StatusParameters {
    online: bool,
}
#[derive(Serialize)]
struct Request<'a, P> {
    func: &'static str,
    param: &'a P,
    token: RequestToken<'a, P>,
}
#[derive(Serialize)]
struct RequestToken<'a, P> {
    id: u16,
    param: &'a P,
}
#[derive(Deserialize)]
struct ResponseToken {
    id: u16,
    param: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct Response {
    token: Option<ResponseToken>,
    result: Option<serde_json::Value>,
}
#[derive(Debug)]
struct Reply {
    json: String,
    result: i64,
}
fn exchange<P: Serialize>(
    channel: &mut impl Channel,
    counter: &mut u16,
    function: &'static str,
    parameters: &P,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Reply> {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    guard(validate, deadline)?;
    *counter = counter.wrapping_add(1);
    let id = *counter;
    let expected_parameters = serde_json::to_value(parameters)?;
    let request = Request {
        func: function,
        param: parameters,
        token: RequestToken {
            id,
            param: parameters,
        },
    };
    channel.send(serde_json::to_string(&request)?, deadline, validate)?;
    loop {
        let json = channel.receive(deadline, validate)?;
        guard(validate, deadline)?;
        ensure!(
            json.len() <= MAX_MESSAGE_BYTES,
            "Local Chroma Connect response exceeds its limit"
        );
        let response: Response =
            serde_json::from_str(&json).context("Invalid local Chroma Connect response")?;
        if let Some(token) = response.token.filter(|token| token.id == id) {
            ensure!(
                token.param.as_ref() == Some(&expected_parameters),
                "Local Chroma Connect response token does not echo its request parameters"
            );
            let result = response
                .result
                .and_then(|value| value.as_i64())
                .context("Local Chroma Connect response omitted its numeric result")?;
            ensure!(
                (i32::MIN as i64..=u32::MAX as i64).contains(&result),
                "Local Chroma Connect result exceeds its native integer range"
            );
            return Ok(Reply { json, result });
        }
    }
}

fn set_synapse_status(
    channel: &mut impl Channel,
    counter: &mut u16,
    online: bool,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let reply = exchange(
        channel,
        counter,
        "SetSynapseStatus",
        &StatusParameters { online },
        validate,
    )?;
    ensure!(
        reply.result == 0,
        "Local Chroma Connect SetSynapseStatus was not confirmed: {}",
        reply.json
    );
    Ok(())
}

#[derive(Default)]
struct Registration {
    pending: bool,
}
struct Initialization {
    result: i64,
    status_error: Option<String>,
}
impl Registration {
    fn initialize(
        &mut self,
        channel: &mut impl Channel,
        counter: &mut u16,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Reply> {
        validate().map_err(|_| Canceled)?;
        self.pending = true;
        exchange(
            channel,
            counter,
            "Init",
            &AppParameters { appid: APP_ID },
            validate,
        )
    }
    fn initialize_application(
        &mut self,
        channel: &mut impl Channel,
        counter: &mut u16,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Initialization> {
        let reply = self.initialize(channel, counter, validate)?;
        // ON_INITIALIZE awaits this API but does not gate device calls on its
        // result. Its failure is distinct from Init and physical device receipts.
        let status_error = if reply.result == 0 {
            set_synapse_status(channel, counter, true, validate)
                .err()
                .map(|error| format!("{error:#}"))
        } else {
            None
        };
        Ok(Initialization {
            result: reply.result,
            status_error,
        })
    }
    fn uninitialize(
        &mut self,
        channel: &mut impl Channel,
        counter: &mut u16,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let reply = exchange(
            channel,
            counter,
            "UnInit",
            &AppParameters { appid: APP_ID },
            validate,
        )?;
        ensure!(
            reply.result == 0,
            "Local Chroma Connect UnInit was not confirmed: {}",
            reply.json
        );
        self.pending = false;
        Ok(())
    }
    fn release_on_new_connection(
        &mut self,
        channel: &mut impl Channel,
        counter: &mut u16,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        // Each socket has a fresh server endpoint with no loaded module.
        // Init loads that endpoint's module; 1247 preserves the existing global
        // registration. The source exit order clears online before UnInit.
        // UnInit's result confirms its API only: the server retains older
        // endpoint objects, and its response can hide an unregister failure.
        let mut errors = Vec::new();
        match self.initialize(channel, counter, validate) {
            Ok(reply) if matches!(reply.result, 0 | 1247) => {}
            Ok(reply) => errors.push(format!(
                "Local Chroma Connect cleanup Init returned {}",
                reply.result
            )),
            Err(error) => errors.push(format!(
                "Local Chroma Connect cleanup Init failed: {error:#}"
            )),
        }
        if let Err(error) = set_synapse_status(channel, counter, false, validate)
            .context("Local Chroma Connect cleanup offline status was not confirmed")
        {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.uninitialize(channel, counter, validate) {
            errors.push(format!("{error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn ensure_released(&self) -> anyhow::Result<()> {
        ensure!(
            !self.pending,
            "Local Chroma Connect registration release remains unconfirmed"
        );
        Ok(())
    }
}

/// Blocking local I/O; retain one instance in the application and call it only
/// from background work. Construction performs no network or filesystem I/O.
pub struct Session {
    socket: Option<Socket>,
    initialized: bool,
    registration: Registration,
    counter: u16,
    next_attempt: Option<Instant>,
    reconnect_index: usize,
    init_retry: u8,
    last_error: Option<String>,
    status_error: Option<String>,
    finish_receipt: Option<Result<(), String>>,
}
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
impl Session {
    pub fn new() -> Self {
        Self {
            socket: None,
            initialized: false,
            registration: Registration::default(),
            counter: 0,
            next_attempt: None,
            reconnect_index: 0,
            init_retry: 0,
            last_error: None,
            status_error: None,
            finish_receipt: None,
        }
    }
    /// Consume the source's non-blocking status-call diagnostic independently
    /// of initialization and physical device receipts.
    pub fn take_status_error(&mut self) -> Option<String> {
        self.status_error.take()
    }
    pub fn maintain(&mut self, validate: &dyn Fn() -> anyhow::Result<()>) -> anyhow::Result<()> {
        if self.finish_receipt.is_some() {
            return Ok(());
        }
        if self.initialized {
            let result = self
                .socket
                .as_mut()
                .context("Initialized local service has no socket")?
                .observe_connection(validate);
            if let Err(error) = &result {
                self.last_error = Some(format!("{error:#}"));
                self.schedule_reconnect();
            }
            return result;
        }
        if self.next_attempt.is_some_and(|time| Instant::now() < time) {
            return Ok(());
        }
        self.next_attempt = None;
        let result = (|| {
            let socket = Socket::connect(validate)?;
            self.socket = Some(socket);
            let initialization = self.registration.initialize_application(
                self.socket.as_mut().unwrap(),
                &mut self.counter,
                validate,
            )?;
            let result = initialization.result;
            self.reconnect_index = 0;
            if result == 0 {
                self.initialized = true;
                self.init_retry = 0;
                self.next_attempt = None;
                self.last_error = None;
                self.status_error = initialization.status_error;
                return Ok(());
            }
            if result == 1247 && self.init_retry < 5 {
                self.init_retry += 1;
                let cleanup = self.registration.uninitialize(
                    self.socket.as_mut().unwrap(),
                    &mut self.counter,
                    validate,
                );
                self.drop_socket();
                self.next_attempt = Some(Instant::now() + Duration::from_millis(500));
                cleanup.context("Local Chroma Connect recovery UnInit failed")?;
            } else {
                self.init_retry = 0;
            }
            anyhow::bail!("Local Chroma Connect Init returned {result}")
        })();
        if let Err(error) = &result {
            self.last_error = Some(format!("{error:#}"));
            if self.socket.is_some() || self.next_attempt.is_none() {
                self.schedule_reconnect();
            }
        }
        result
    }
    pub fn set_attribute(
        &mut self,
        pid: u16,
        attribute: u8,
        value: u8,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> ChromaAttributeReceipt {
        if validate().is_err() {
            return ChromaAttributeReceipt::Canceled;
        }
        if let Err(error) = self.maintain(validate) {
            if error.downcast_ref::<Canceled>().is_some() {
                return ChromaAttributeReceipt::Canceled;
            }
        }
        let Some(socket) = self.socket.as_mut().filter(|_| self.initialized) else {
            return ChromaAttributeReceipt::NotSent {
                reason: self
                    .last_error
                    .clone()
                    .unwrap_or_else(|| "Local Chroma Connect is not initialized".into()),
            };
        };
        let result = exchange(
            socket,
            &mut self.counter,
            "SetAttribute",
            &AttributeParameters {
                pid,
                attribute,
                value,
            },
            validate,
        );
        match result {
            Ok(reply) => ChromaAttributeReceipt::Replied {
                response_json: reply.json,
            },
            Err(error) => {
                let canceled = error.downcast_ref::<Canceled>().is_some();
                self.last_error = Some(format!("{error:#}"));
                // Close this connection on cancellation as well: an old token
                // response cannot satisfy a request after the counter wraps.
                self.schedule_reconnect();
                if canceled {
                    ChromaAttributeReceipt::Canceled
                } else {
                    ChromaAttributeReceipt::Failed {
                        message: format!("{error:#}"),
                    }
                }
            }
        }
    }
    fn drop_socket(&mut self) {
        self.initialized = false;
        self.socket.take();
    }
    fn schedule_reconnect(&mut self) {
        self.drop_socket();
        self.next_attempt = Some(Instant::now() + RECONNECT_DELAYS[self.reconnect_index]);
        self.reconnect_index = (self.reconnect_index + 1).min(RECONNECT_DELAYS.len() - 1);
    }
    pub fn finish(&mut self) -> anyhow::Result<()> {
        if let Some(receipt) = &self.finish_receipt {
            return receipt.clone().map_err(anyhow::Error::msg);
        }
        let mut errors = Vec::new();
        let reconnected = self.registration.pending && self.socket.is_none();
        if reconnected {
            match Socket::connect(&|| Ok(())) {
                Ok(socket) => self.socket = Some(socket),
                Err(error) => errors.push(format!(
                    "Local Chroma Connect cleanup connection: {error:#}"
                )),
            }
        }
        if let Some(socket) = self.socket.as_mut() {
            if self.initialized {
                if let Err(error) = set_synapse_status(socket, &mut self.counter, false, &|| Ok(()))
                {
                    errors.push(format!("{error:#}"));
                }
            }
            if self.registration.pending {
                let cleanup = if reconnected {
                    self.registration
                        .release_on_new_connection(socket, &mut self.counter, &|| Ok(()))
                } else {
                    self.registration
                        .uninitialize(socket, &mut self.counter, &|| Ok(()))
                };
                if let Err(error) = cleanup {
                    errors.push(format!("{error:#}"));
                }
            }
            if let Err(error) = socket.finish() {
                errors.push(format!("{error:#}"));
            }
        }
        self.drop_socket();
        if let Err(error) = self.registration.ensure_released() {
            errors.push(format!("{error:#}"));
        }
        let receipt = if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        };
        self.finish_receipt = Some(receipt.clone());
        receipt.map_err(anyhow::Error::msg)
    }
}

#[cfg(test)]
#[path = "chroma_connect_tests.rs"]
mod tests;
