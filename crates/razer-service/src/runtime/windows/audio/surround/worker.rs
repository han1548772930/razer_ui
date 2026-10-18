use super::{Command, Snapshot, queue_name};
use anyhow::{Context as _, Result, ensure};
use razer_device::audio_surround::{Channel, NativeCall, Product, protocol::*, routing, wire::Payload};
use std::{path::PathBuf, sync::{Arc, Mutex, mpsc}, time::Duration};
use tokio::{sync::mpsc::UnboundedReceiver, time::timeout};
use zeromq::{ReqSocket, SubSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const INITIAL_TIMEOUT: Duration = Duration::from_secs(5);

fn service_directory(product: Product) -> Result<PathBuf> {
    use windows_sys::Win32::{System::Com::CoTaskMemFree, UI::Shell::{SHGetKnownFolderPath, FOLDERID_ProgramData}};
    let mut pointer = std::ptr::null_mut();
    // SAFETY: the system allocates the terminated path; it is copied before freeing with CoTaskMemFree.
    let result = unsafe { SHGetKnownFolderPath(&FOLDERID_ProgramData, 0, std::ptr::null_mut(), &mut pointer) };
    ensure!(result >= 0 && !pointer.is_null(), "Cannot locate routing service ProgramData directory ({result:#x})");
    let path = unsafe {
        let mut length = 0;
        while *pointer.add(length) != 0 { length += 1; }
        let path = String::from_utf16(std::slice::from_raw_parts(pointer, length));
        CoTaskMemFree(pointer.cast());
        PathBuf::from(path?)
    };
    Ok(path.join("THX").join(queue_name(product)))
}

fn single_frame(message: ZmqMessage) -> Result<Vec<u8>> {
    ensure!(message.len() == 1, "Routing service returned an unexpected multipart message");
    Ok(message.get(0).context("Routing reply frame is absent")?.to_vec())
}

struct Client {
    product: Product,
    request_address: String,
    requests: Option<ReqSocket>,
    events: SubSocket,
    state: Snapshot,
    combo: bool,
    replicate: bool,
    front_mapper: routing::FrontMapper,
    front_gains: [f32; 3],
    requested_delay: f32,
    cancellation: Option<Arc<std::sync::atomic::AtomicBool>>,
}
impl Client {
    async fn connect(product: Product) -> Result<Self> {
        let directory = service_directory(product)?;
        let request_address = format!("ipc://{}", directory.join("thx_reqrep").display());
        let event_address = format!("ipc://{}", directory.join("thx_pubsub").display());
        let mut events = SubSocket::new();
        events.subscribe("").await?;
        match timeout(INITIAL_TIMEOUT, events.connect(&event_address)).await {
            Ok(Ok(())) => {},
            result => { let _ = events.close().await; anyhow::bail!("Cannot connect routing subscription socket: {result:?}"); }
        }
        let mut client = Self { product, request_address, requests: None, events, state: Snapshot::default(), combo: false, replicate: false, front_mapper: routing::FrontMapper::new(product), front_gains: [0.0; 3], requested_delay: 0.0, cancellation: None };
        let initialized = client.initialize().await;
        if let Err(error) = initialized { client.close_sockets().await; return Err(error); }
        Ok(client)
    }
    async fn initialize(&mut self) -> Result<()> {
        let request = Payload::ClientConnect(ClientConnect { file_version: env!("CARGO_PKG_VERSION").to_owned(), api_version: "6".to_owned() });
        let bytes = self.exchange(&request).await?;
        let Payload::ClientConnectAcknowledge(ack) = Payload::decode(&bytes)? else { anyhow::bail!("Routing service returned no handshake acknowledgment"); };
        self.state.version = Some(ack.file_version);
        timeout(INITIAL_TIMEOUT, async {
            while !self.state.observations.ready(self.product) { self.receive().await?; }
            Ok::<_, anyhow::Error>(())
        }).await.context("Timed out waiting for required routing states")??;
        self.state.connected = true;
        self.observe_route();
        Ok(())
    }
    fn observe_route(&mut self) {
        if let Some(front) = self.state.observations.active_front() {
            self.state.front_endpoint = Some(front.to_owned());
            self.combo = true;
            if let Some(replicate) = self.state.observations.replicated() { self.replicate = replicate; }
        }
        if self.state.front_endpoint.as_ref().is_some_and(|id| self.state.observations.endpoint(id).is_none()) {
            self.state.front_endpoint = None;
            self.combo = false;
        }
    }
    async fn receive(&mut self) -> Result<()> {
        let bytes = single_frame(self.events.recv().await?)?;
        let payload = Payload::decode(&bytes)?;
        self.state.observations.apply(payload)?;
        self.observe_route();
        Ok(())
    }
    fn check_cancellation(&self) -> Result<()> {
        ensure!(!self.cancellation.as_ref().is_some_and(|c| c.load(std::sync::atomic::Ordering::Acquire)), "Surround operation cancelled");
        Ok(())
    }
    async fn exchange(&mut self, payload: &Payload) -> Result<Vec<u8>> {
        self.check_cancellation()?;
        payload.validate()?;
        if self.requests.is_none() {
            let mut requests = ReqSocket::new();
            match timeout(REQUEST_TIMEOUT, requests.connect(&self.request_address)).await {
                Ok(Ok(())) => self.requests = Some(requests),
                result => { let _ = requests.close().await; anyhow::bail!("Cannot connect routing request socket: {result:?}"); }
            }
        }
        let requests = self.requests.as_mut().context("Routing request socket is absent")?;
        let result = timeout(REQUEST_TIMEOUT, async {
            requests.send(payload.encode().into()).await?;
            single_frame(requests.recv().await?)
        }).await;
        match result {
            Ok(Ok(reply)) => Ok(reply),
            result => {
                if let Some(requests) = self.requests.take() { let _ = requests.close().await; }
                anyhow::bail!("Routing request failed without a confirmed result: {result:?}")
            }
        }
    }
    async fn commit(&mut self, request: Payload) -> Result<()> {
        let response = Payload::decode(&self.exchange(&request).await?)?;
        ensure!(request.name() == response.name() && request.header().map(|v| &v.id) == response.header().map(|v| &v.id), "Routing reply belongs to a different state");
        let matches = routing::same_committed_values(&request, &response);
        let update = self.state.observations.apply(response)?;
        ensure!(update != razer_device::audio_surround::observed::Update::Stale, "Routing service returned a stale commit");
        ensure!(matches, "Routing service committed different values; actual state retained");
        self.check_cancellation()?;
        Ok(())
    }
    fn endpoint(&self, channel: Channel) -> Result<String> {
        Ok(routing::endpoint_for(&self.state.observations, self.state.front_endpoint.as_deref(), channel)?.to_owned())
    }
    async fn set_eq(&mut self, endpoint: &str, gains: &[f32]) -> Result<()> {
        for band in routing::equalizer_bands(gains)? {
            self.commit(self.state.observations.equalizer_request(endpoint, band)?).await?;
        }
        Ok(())
    }
    async fn rebuild_route(&mut self) -> Result<()> {
        let rear = self.endpoint(Channel::Rear)?;
        let front = self.state.front_endpoint.clone();
        if self.combo { ensure!(front.is_some(), "Combo routing requires a selected front endpoint"); }
        let active = self.state.observations.manager.as_ref().context("Routing stream manager is unobserved")?.active_streams.clone();
        for stream in active {
            if stream.endpoint_id != rear && (!self.combo || front.as_deref() != Some(stream.endpoint_id.as_str())) {
                self.commit(self.state.observations.stream_request(&stream.endpoint_id, false)?).await?;
            }
        }
        if self.combo {
            let front = front.context("Front endpoint is absent")?;
            let map = self.front_mapper.map(self.state.observations.endpoint(&front).context("Front endpoint disappeared")?)?;
            self.commit(self.state.observations.map_request(map)?).await?;
            self.commit(self.state.observations.stream_request(&front, true)?).await?;
        }
        let map = routing::rear_map(&rear, self.combo, self.replicate);
        self.commit(self.state.observations.map_request(map)?).await
    }
    async fn call(&mut self, call: &NativeCall) -> Result<bool> {
        match call {
            NativeCall::SetEnabled(enabled) => { self.combo = *enabled; self.rebuild_route().await?; }
            NativeCall::SetReplicate(enabled) => { self.replicate = *enabled; self.rebuild_route().await?; }
            NativeCall::SetFrontEndpoint(id) => {
                ensure!(self.state.observations.endpoint(id).is_some(), "Requested front endpoint is not connected");
                // SetFrontEndpoint copies the client's retained three-band command gains,
                // then reapplies its signed delay to the previous front endpoint (100d0).
                self.set_eq(id, &self.front_gains.clone()).await?;
                if let Some(old) = self.state.front_endpoint.clone() {
                    self.commit(self.state.observations.delay_request(&old, self.requested_delay.max(0.0))?).await?;
                }
                self.state.front_endpoint = Some(id.clone());
                self.rebuild_route().await?;
            }
            NativeCall::SetVolume { channel, volume } => {
                let endpoint = self.endpoint(*channel)?;
                self.commit(self.state.observations.volume_request(&endpoint, Some(volume.native_value()), None)?).await?;
            }
            NativeCall::SetRearMute(muted) => {
                let endpoint = self.endpoint(Channel::Rear)?;
                self.commit(self.state.observations.volume_request(&endpoint, None, Some(*muted))?).await?;
            }
            NativeCall::SetEqualizer { channel, equalizer } => {
                let endpoint = self.endpoint(*channel)?;
                let gains = equalizer.bands().map(f32::from);
                if *channel == Channel::Front { self.front_gains = gains; }
                self.set_eq(&endpoint, &gains).await?;
            }
            NativeCall::SetDelay(delay) => {
                let front = self.endpoint(Channel::Front)?;
                let rear = self.endpoint(Channel::Rear)?;
                let milliseconds = f32::from(delay.milliseconds());
                self.requested_delay = milliseconds;
                let (first, second, value) = if milliseconds > 0.0 { (rear, front, milliseconds) } else { (front, rear, -milliseconds) };
                self.commit(self.state.observations.delay_request(&first, 0.0)?).await?;
                // The client clears the opposite path before enabling the new delay (11ec0).
                tokio::time::sleep(Duration::from_millis(10)).await;
                self.commit(self.state.observations.delay_request(&second, value)?).await?;
            }
            NativeCall::SetTestSignal(enabled) => {
                self.state.signal_requested = Some(*enabled);
                let request = if *enabled { Payload::PlayDelayTestSignal(PlayDelayTestSignal {}) } else { Payload::StopDelayTestSignal(StopDelayTestSignal {}) };
                let reply = self.exchange(&request).await?;
                ensure!(reply.is_empty(), "Unexpected routing audition reply");
                // onRequestReceived returns an empty reply without exposing playback success.
                self.state.signal_delivered = Some(*enabled);
            }
        }
        self.check_cancellation()?;
        Ok(true)
    }
    async fn restore(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(e) = self.call(&NativeCall::SetTestSignal(false)).await { errors.push(format!("stop signal: {e:#}")); }
        if let Err(e) = self.call(&NativeCall::SetEnabled(false)).await { errors.push(format!("disable combo: {e:#}")); }
        for (role, count) in [(Some(1), 10), (Some(2), 3), (None, 3)] {
            let endpoint = match role {
                Some(role) => self.state.observations.endpoint_with_role(role).map(|e| e.endpoint_id.clone()),
                None => self.endpoint(Channel::Front),
            };
            match endpoint {
                Ok(id) => if let Err(e) = self.set_eq(&id, &vec![0.0; count]).await { errors.push(format!("restore EQ {id}: {e:#}")); },
                Err(e) => errors.push(format!("restore endpoint: {e:#}")),
            }
        }
        ensure!(errors.is_empty(), "Surround cleanup incomplete: {}", errors.join("; "));
        Ok(())
    }
    async fn close_sockets(&mut self) {
        if let Some(requests) = self.requests.take() { let _ = requests.close().await; }
        let events = std::mem::replace(&mut self.events, SubSocket::new());
        let _ = events.close().await;
    }
}

fn publish(shared: &Mutex<Snapshot>, state: &Snapshot) {
    if let Ok(mut current) = shared.lock() { *current = state.clone(); }
}

pub(super) async fn run(product: Product, mut commands: UnboundedReceiver<Command>, shared: Arc<Mutex<Snapshot>>, started: mpsc::SyncSender<Result<()>>) {
    let mut client = match Client::connect(product).await {
        Ok(client) => client,
        Err(error) => { let _ = started.send(Err(error)); return; }
    };
    publish(&shared, &client.state);
    let _ = started.send(Ok(()));
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Call(call, cancellation, reply)) => {
                    client.cancellation = Some(cancellation);
                    let result = client.call(&call).await;
                    client.cancellation = None;
                    client.state.failure = result.as_ref().err().map(|e| format!("{e:#}"));
                    publish(&shared, &client.state);
                    let _ = reply.send(result);
                }
                Some(Command::Shutdown(reply)) => {
                    let result = client.restore().await;
                    client.close_sockets().await;
                    client.state.connected = false;
                    client.state.failure = result.as_ref().err().map(|e| format!("{e:#}"));
                    publish(&shared, &client.state);
                    let _ = reply.send(result);
                    return;
                }
                None => { let _ = client.restore().await; break; }
            },
            result = client.receive(), if client.state.connected => {
                if let Err(error) = result { client.state.connected = false; client.state.failure = Some(format!("{error:#}")); }
                publish(&shared, &client.state);
            }
        }
    }
    client.close_sockets().await;
}
