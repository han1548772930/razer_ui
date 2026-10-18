//! Retained hardware HID collections and their input-thread lifetime.
use super::{driver, identity::DeviceInfo};
use anyhow::{Context as _, Result, ensure};
use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[path = "hardware_transport.rs"]
mod platform;

const RECONNECT_DELAY: Duration = Duration::from_secs(1);
const RECONNECT_LIMIT: u8 = 60;
const INPUT_CAPACITY: usize = 512;

#[derive(Default)]
struct Probe {
    attempts: u32,
    quiet: u8,
    due: Option<Instant>,
}
impl Probe {
    fn observed(&mut self, now: Instant, any: bool, added: bool) {
        self.attempts = self.attempts.saturating_add(1);
        let again = if added {
            self.quiet = 0;
            true
        } else if !any {
            self.attempts < 10
        } else {
            self.quiet += 1;
            self.quiet < 3
        };
        // AB7F0's microsecond delay is 1000 * min(2 << tries, 10).
        let millis = match self.attempts {
            1 => 4,
            2 => 8,
            _ => 10,
        };
        self.due = again.then_some(now + Duration::from_millis(millis));
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EndpointIdentity {
    pub path: String,
    pub instance: String,
    pub generation: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Packet {
    pub endpoint: EndpointIdentity,
    pub input: driver::Packet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Collection {
    path: String,
    instance: String,
}

struct Read {
    bytes: Vec<u8>,
    rearm_error: Option<io::Error>,
}

trait Reader {
    fn poll(&mut self) -> io::Result<Option<Read>>;
    /// Cancel, observe terminal completion, then release the handle/buffers.
    fn close(&mut self) -> Result<()>;
}

trait Transport {
    type Reader: Reader;
    fn collections(&mut self) -> Result<Vec<Collection>>;
    fn open(&mut self, collection: &Collection) -> Result<Self::Reader>;
    fn capture(&mut self, bytes: Vec<u8>) -> Result<driver::Packet>;
}

struct Retry {
    attempts: u8,
    due: Instant,
    last_failure: String,
}

struct Endpoint<R> {
    collection: Collection,
    reader: Option<R>,
    generation: u64,
    retry: Option<Retry>,
    retired: bool,
}

struct Session<T: Transport> {
    transport: T,
    endpoints: Vec<Endpoint<T::Reader>>,
    observations: Vec<Result<Packet, String>>,
    cleanup_errors: Vec<String>,
    stopped: bool,
}

impl<T: Transport> Session<T> {
    fn open(transport: T) -> Result<Self> {
        let mut session = Self {
            transport,
            endpoints: Vec::new(),
            observations: Vec::new(),
            cleanup_errors: Vec::new(),
            stopped: false,
        };
        session.refresh()?;
        Ok(session)
    }

    fn refresh(&mut self) -> Result<()> {
        ensure!(!self.stopped, "Mapping hardware session is stopped");
        // 119de6 appends all matching collections in SetupDi order;
        // 123fe0 adds only previously unseen paths, without a usage filter.
        let paths = self.transport.collections()?;
        let mut seen = std::collections::BTreeSet::new();
        for path in &paths {
            ensure!(
                seen.insert(path.path.to_ascii_lowercase()),
                "Duplicate mapping hardware collection"
            );
            ensure!(
                !path.instance.is_empty(),
                "Mapping hardware collection lacks instance identity"
            );
        }
        for path in paths {
            if let Some(index) = self
                .endpoints
                .iter()
                .position(|item| item.collection.path.eq_ignore_ascii_case(&path.path))
            {
                if !self.endpoints[index]
                    .collection
                    .instance
                    .eq_ignore_ascii_case(&path.instance)
                    && !self.endpoints[index].retired
                {
                    self.stop_endpoint(index);
                    self.endpoints[index].retry = None;
                    self.endpoints[index].retired = true;
                    self.observations.push(Err(
                        "Mapping hardware collection changed physical instance".into(),
                    ));
                }
                continue;
            }
            let opened = self.transport.open(&path);
            let mut endpoint = Endpoint {
                collection: path,
                reader: None,
                generation: 0,
                retry: None,
                retired: false,
            };
            match opened {
                Ok(reader) => {
                    endpoint.reader = Some(reader);
                    endpoint.generation = 1;
                }
                Err(error) => {
                    // Initial CreateFile failure is retained by 122fd8; it
                    // must not fabricate a connected input endpoint.
                    self.observations
                        .push(Err(format!("Mapping hardware open: {error:#}")));
                }
            }
            self.endpoints.push(endpoint);
        }
        Ok(())
    }

    fn stop_endpoint(&mut self, index: usize) {
        if let Some(mut reader) = self.endpoints[index].reader.take() {
            if let Err(error) = reader.close() {
                let error = format!("Mapping hardware cleanup: {error:#}");
                self.cleanup_errors.push(error.clone());
                self.observations.push(Err(error));
            }
        }
    }

    fn failed(&mut self, index: usize, error: io::Error, now: Instant) {
        self.stop_endpoint(index);
        // 1248a0 schedules reconnect only for ERROR_DEVICE_NOT_CONNECTED.
        // Both synchronous ReadFile failure (1232f8) and completion failure
        // (123600, whose this pointer is offset by 16) reset endpoint+56.
        self.endpoints[index].retry = (error.raw_os_error() == Some(1167)).then_some(Retry {
            attempts: 0,
            due: now + RECONNECT_DELAY,
            last_failure: format!("Read: {error}"),
        });
        if self.endpoints[index].retry.is_none() {
            self.observations
                .push(Err(format!("Mapping hardware read: {error}")));
        }
    }

    fn reconnect(&mut self, index: usize, now: Instant) {
        let Some(retry) = self.endpoints[index].retry.as_mut() else {
            return;
        };
        if now < retry.due {
            return;
        }
        retry.attempts += 1;
        if retry.attempts >= RECONNECT_LIMIT {
            let failure = format!(
                "Mapping hardware reconnect exhausted 59 attempts; {}",
                retry.last_failure
            );
            self.endpoints[index].retry = None;
            self.observations.push(Err(failure));
            return;
        }
        retry.due = now + RECONNECT_DELAY;
        let collection = self.endpoints[index].collection.clone();
        let observed = match self.transport.collections() {
            Ok(paths) => paths,
            Err(error) => {
                self.endpoints[index].retry.as_mut().unwrap().last_failure =
                    format!("Identity query: {error:#}");
                return;
            }
        };
        let Some(current) = observed
            .iter()
            .find(|path| path.path.eq_ignore_ascii_case(&collection.path))
        else {
            self.endpoints[index].retry.as_mut().unwrap().last_failure =
                "Original collection path is absent".into();
            return;
        };
        if !current.instance.eq_ignore_ascii_case(&collection.instance) {
            self.endpoints[index].retry = None;
            self.endpoints[index].retired = true;
            self.observations.push(Err(
                "Mapping hardware reconnect rejected a replacement instance".into(),
            ));
            return;
        }
        match self.transport.open(&collection) {
            Ok(reader) => {
                let endpoint = &mut self.endpoints[index];
                endpoint.reader = Some(reader);
                endpoint.generation += 1;
                endpoint.retry = None;
            }
            // 1233A0 schedules the next retry without notifying the device.
            // 124DA0 reports failure only when the retry limit is reached.
            Err(error) => {
                self.endpoints[index].retry.as_mut().unwrap().last_failure =
                    format!("Open: {error:#}");
            }
        }
    }

    fn poll(&mut self, now: Instant) -> Vec<Result<Packet, String>> {
        for index in 0..self.endpoints.len() {
            self.reconnect(index, now);
            let Some(reader) = self.endpoints[index].reader.as_mut() else {
                continue;
            };
            match reader.poll() {
                Ok(None) => {}
                Err(error) => self.failed(index, error, now),
                Ok(Some(read)) => {
                    // 123600 rearms before 1244e0 forwards actual bytes.
                    if read.bytes.len() > 100 {
                        self.failed(index, io::Error::from_raw_os_error(13), now);
                        continue;
                    }
                    if let Some(error) = read.rearm_error {
                        self.failed(index, error, now);
                    }
                    if !read.bytes.is_empty() {
                        let endpoint = &self.endpoints[index];
                        let identity = EndpointIdentity {
                            path: endpoint.collection.path.clone(),
                            instance: endpoint.collection.instance.clone(),
                            generation: endpoint.generation,
                        };
                        self.observations.push(
                            self.transport
                                .capture(read.bytes)
                                .map(|input| Packet {
                                    endpoint: identity,
                                    input,
                                })
                                .map_err(|error| {
                                    format!("Mapping hardware input clock: {error:#}")
                                }),
                        );
                    }
                }
            }
        }
        std::mem::take(&mut self.observations)
    }

    fn generations(&self) -> BTreeMap<String, (String, u64)> {
        self.endpoints
            .iter()
            .filter(|item| item.generation != 0 && !item.retired)
            .map(|item| {
                (
                    item.collection.path.to_ascii_lowercase(),
                    (
                        item.collection.instance.to_ascii_lowercase(),
                        item.generation,
                    ),
                )
            })
            .collect()
    }

    fn shutdown(&mut self) -> Result<()> {
        self.stopped = true;
        for endpoint in &mut self.endpoints {
            endpoint.retry = None;
            endpoint.retired = true;
            if let Some(mut reader) = endpoint.reader.take() {
                if let Err(error) = reader.close() {
                    self.cleanup_errors.push(format!("{error:#}"));
                }
            }
        }
        self.observations.clear();
        ensure!(
            self.cleanup_errors.is_empty(),
            "Mapping hardware cleanup: {}",
            self.cleanup_errors.join("; ")
        );
        Ok(())
    }
}

impl<T: Transport> Drop for Session<T> {
    fn drop(&mut self) {
        if !self.stopped
            && let Err(error) = self.shutdown()
        {
            eprintln!("{error:#}");
        }
    }
}

enum Command {
    Refresh(mpsc::SyncSender<Result<(), String>>),
    Fence(mpsc::SyncSender<BTreeMap<String, (String, u64)>>),
    Stop,
}

pub(super) struct Owner {
    sender: mpsc::Sender<Command>,
    input: mpsc::Receiver<Result<Packet, String>>,
    overflow: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), String>>>,
    cleanup_result: Option<Result<(), String>>,
}

impl Owner {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        let info = info.clone();
        Self::open_with(move || platform::Windows::new(info))
    }

    fn open_with<T: Transport + 'static>(
        create: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<Self> {
        let (sender, commands) = mpsc::channel();
        let (publish, input) = mpsc::sync_channel(INPUT_CAPACITY);
        let (ready, initialized) = mpsc::sync_channel(1);
        let overflow = Arc::new(AtomicBool::new(false));
        let worker_overflow = overflow.clone();
        let worker = thread::Builder::new()
            .name("razer-mapping-hardware".into())
            .spawn(move || {
                let opened = create().and_then(Session::open);
                let mut session = match opened {
                    Ok(session) => {
                        let _ = ready.send(Ok(()));
                        session
                    }
                    Err(error) => {
                        let error = format!("{error:#}");
                        let _ = ready.send(Err(error.clone()));
                        return Err(error);
                    }
                };
                let mut probe = Probe::default();
                probe.observed(
                    Instant::now(),
                    !session.endpoints.is_empty(),
                    !session.endpoints.is_empty(),
                );
                let mut wait = Duration::ZERO;
                let mut failure = None;
                'worker: loop {
                    match commands.recv_timeout(wait) {
                        Ok(Command::Refresh(reply)) => {
                            let _ =
                                reply.send(session.refresh().map_err(|error| format!("{error:#}")));
                        }
                        Ok(Command::Fence(reply)) => {
                            let _ = reply.send(session.generations());
                        }
                        Ok(Command::Stop) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    let now = Instant::now();
                    if probe.due.is_some_and(|due| now >= due) {
                        let before = session.endpoints.len();
                        match session.refresh() {
                            Ok(()) => probe.observed(
                                now,
                                !session.endpoints.is_empty(),
                                session.endpoints.len() > before,
                            ),
                            Err(error) => {
                                probe.due = None;
                                session.observations.push(Err(format!(
                                    "Mapping hardware collection probe: {error:#}"
                                )));
                            }
                        }
                    }
                    let observations = session.poll(Instant::now());
                    wait = if observations.is_empty() {
                        Duration::from_millis(1)
                    } else {
                        Duration::ZERO
                    };
                    for observation in observations {
                        match publish.try_send(observation) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                worker_overflow.store(true, Ordering::Release);
                                failure = Some("Mapping hardware input receiver overflowed".into());
                                break 'worker;
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => break 'worker,
                        }
                    }
                }
                let cleanup = session.shutdown();
                match (failure, cleanup) {
                    (None, Ok(())) => Ok(()),
                    (Some(error), Ok(())) => Err(error),
                    (None, Err(error)) => Err(format!("{error:#}")),
                    (Some(error), Err(cleanup)) => Err(format!("{error}; {cleanup:#}")),
                }
            })?;
        match initialized.recv() {
            Ok(Ok(())) => Ok(Self {
                sender,
                input,
                overflow,
                worker: Some(worker),
                cleanup_result: None,
            }),
            result => {
                let joined = worker.join();
                anyhow::bail!("Mapping hardware startup: {result:?}; worker: {joined:?}")
            }
        }
    }

    pub fn refresh_paths(&self) -> Result<()> {
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Refresh(reply))
            .context("Mapping hardware worker stopped")?;
        receive
            .recv()
            .context("Mapping hardware refresh disconnected")?
            .map_err(anyhow::Error::msg)
    }

    pub fn inputs(&mut self) -> Result<Vec<Result<Packet, String>>> {
        ensure!(
            !self.overflow.load(Ordering::Acquire),
            "Mapping hardware input receiver overflowed"
        );
        // Drain first, then fence on the worker: a replaced/reopened collection
        // cannot deliver its earlier queued generation afterward. A completed
        // read remains valid if only its next read failed (123600).
        let queued = self.input.try_iter().collect::<Vec<_>>();
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Fence(reply))
            .context("Mapping hardware worker stopped")?;
        let live = receive
            .recv()
            .context("Mapping hardware generation fence disconnected")?;
        Ok(queued
            .into_iter()
            .filter(|event| match event {
                Err(_) => true,
                Ok(packet) => live
                    .get(&packet.endpoint.path.to_ascii_lowercase())
                    .is_some_and(|(instance, generation)| {
                        instance.eq_ignore_ascii_case(&packet.endpoint.instance)
                            && *generation == packet.endpoint.generation
                    }),
            })
            .collect())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        let Some(worker) = self.worker.take() else {
            return self
                .cleanup_result
                .clone()
                .unwrap_or(Ok(()))
                .map_err(anyhow::Error::msg);
        };
        let result = self
            .sender
            .send(Command::Stop)
            .context("Mapping hardware worker stopped");
        let joined = worker
            .join()
            .map_err(|_| anyhow::anyhow!("Mapping hardware worker panicked"))
            .and_then(|result| result.map_err(anyhow::Error::msg));
        self.input.try_iter().for_each(drop);
        let result = match (result, joined) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(join)) => Err(anyhow::anyhow!("{error:#}; {join:#}")),
        };
        self.cleanup_result = Some(
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| format!("{error:#}")),
        );
        result
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("{error:#}");
        }
    }
}

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
