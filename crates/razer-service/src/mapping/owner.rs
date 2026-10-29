//! One actor owns the mapping libraries and all device/capture scopes. A client
//! release drains that client's work; it cannot shut down another client.
use anyhow::{Context, Result, ensure};
use razer_ipc::ServiceRequest;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(crate) trait Backend: 'static {
    fn acquire(&mut self, scope: Uuid) -> Result<()>;
    fn request(&mut self, scope: Uuid, request: ServiceRequest) -> Result<Value>;
    fn poll(&mut self) -> Result<()>;
    fn release(&mut self, scope: Uuid) -> Result<()>;
    fn release_cleanup_pending(&self, scope: Uuid) -> bool;
    fn shutdown(&mut self) -> Result<()>;
}

type Response = mpsc::SyncSender<Result<Value, String>>;
enum Command {
    Acquire {
        scope: Uuid,
        response: Response,
    },
    Request {
        scope: Uuid,
        request: ServiceRequest,
        response: Response,
    },
    Release {
        scope: Uuid,
        response: Option<Response>,
        released: Arc<AtomicBool>,
    },
}

struct Actor {
    sender: mpsc::Sender<Command>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Actor {
    fn drop(&mut self) {
        // All client releases precede dropping the last Sender. The worker
        // drains them before channel disconnect triggers final shutdown.
        let (replacement, receiver) = mpsc::channel();
        drop(receiver);
        drop(std::mem::replace(&mut self.sender, replacement));
        if let Some(worker) = self
            .thread
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            if worker.join().is_err() {
                eprintln!("Mapping actor panicked during final cleanup");
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Registry {
    actor: Mutex<Weak<Actor>>,
    lifecycle: Arc<Mutex<()>>,
}

impl Registry {
    pub(crate) fn acquire<B: Backend>(
        &self,
        create: impl FnOnce() -> Result<B> + Send + 'static,
    ) -> Result<Client> {
        let mut registry = self
            .actor
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping actor registry poisoned"))?;
        let actor = if let Some(actor) = registry.upgrade() {
            actor
        } else {
            let (sender, receiver) = mpsc::channel();
            let (ready, initialized) = mpsc::sync_channel(1);
            let lifecycle = self.lifecycle.clone();
            let worker = thread::Builder::new()
                .name("razer-mapping-owner".into())
                .spawn(move || {
                    let _lifecycle = lifecycle.lock().unwrap_or_else(|error| error.into_inner());
                    let backend = super::guarded(create);
                    match backend {
                        Ok(backend) => {
                            let _ = ready.send(Ok(()));
                            run(backend, receiver);
                        }
                        Err(error) => {
                            let _ = ready.send(Err(format!("{error:#}")));
                        }
                    }
                })?;
            match initialized
                .recv()
                .context("Mapping actor initialization disconnected")?
            {
                Ok(()) => {}
                Err(error) => {
                    let _ = worker.join();
                    anyhow::bail!(error);
                }
            }
            let actor = Arc::new(Actor {
                sender,
                thread: Mutex::new(Some(worker)),
            });
            *registry = Arc::downgrade(&actor);
            actor
        };
        let scope = Uuid::new_v4();
        let (response, reply) = mpsc::sync_channel(1);
        actor
            .sender
            .send(Command::Acquire { scope, response })
            .context("Mapping actor stopped")?;
        receive(reply)?;
        Ok(Client {
            actor,
            scope,
            stopped: false,
            released: Arc::new(AtomicBool::new(false)),
        })
    }
}

/// Retained by the existing device, recorder and focused-capture workers. A
/// request waits only on those background workers, never on the GUI thread.
pub struct Client {
    actor: Arc<Actor>,
    scope: Uuid,
    stopped: bool,
    released: Arc<AtomicBool>,
}

impl Client {
    pub fn spawn() -> Result<Self> {
        crate::runtime::mapping_client()
    }

    pub fn request(&mut self, request: ServiceRequest) -> Result<Value> {
        ensure!(!self.is_stopped(), "Mapping scope has been released");
        let (response, reply) = mpsc::sync_channel(1);
        let shutdown = matches!(request, ServiceRequest::Shutdown);
        let command = if shutdown {
            Command::Release {
                scope: self.scope,
                response: Some(response),
                released: self.released.clone(),
            }
        } else {
            Command::Request {
                scope: self.scope,
                request,
                response,
            }
        };
        self.actor
            .sender
            .send(command)
            .context("Mapping actor disconnected")?;
        let result = receive(reply);
        if shutdown {
            self.stopped = self.released.load(Ordering::Acquire);
        }
        result
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped || self.released.load(Ordering::Acquire)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if !self.is_stopped() {
            self.stopped = true;
            let _ = self.actor.sender.send(Command::Release {
                scope: self.scope,
                response: None,
                released: self.released.clone(),
            });
        }
    }
}

fn receive(reply: mpsc::Receiver<Result<Value, String>>) -> Result<Value> {
    reply
        .recv()
        .context("Mapping reply disconnected")?
        .map_err(anyhow::Error::msg)
}

fn respond(response: Response, result: Result<Value>) {
    let _ = response.send(result.map_err(|error| format!("{error:#}")));
}

#[derive(Default)]
struct Scope {
    releasing: bool,
    failures: BTreeSet<String>,
    abandoned: bool,
    released: Option<Arc<AtomicBool>>,
    retry_at: Option<Instant>,
}

fn release_scope(
    backend: &mut impl Backend,
    scopes: &mut BTreeMap<Uuid, Scope>,
    scope: Uuid,
    released: Arc<AtomicBool>,
    failure: Option<&str>,
) -> Result<Value> {
    let state = scopes
        .get_mut(&scope)
        .context("Mapping client scope changed")?;
    state.releasing = true;
    state.released = Some(released.clone());
    state.retry_at = Some(Instant::now() + Duration::from_millis(50));
    if let Some(failure) = failure {
        state.failures.insert(failure.into());
    }
    if let Err(error) = super::guarded(|| backend.release(scope)) {
        state.failures.insert(format!("{error:#}"));
    }
    let failures = state.failures.iter().cloned().collect::<Vec<_>>();
    if !backend.release_cleanup_pending(scope) {
        scopes.remove(&scope);
        released.store(true, Ordering::Release);
    }
    ensure!(failures.is_empty(), "{}", failures.join("; "));
    ensure!(
        released.load(Ordering::Acquire),
        "Mapping release returned before its retained owners terminated"
    );
    Ok(json!({"stopped": true}))
}

fn run(mut backend: impl Backend, receiver: mpsc::Receiver<Command>) {
    let mut scopes = BTreeMap::<Uuid, Scope>::new();
    let mut failure = None::<String>;
    loop {
        match receiver.recv_timeout(Duration::from_millis(4)) {
            Ok(Command::Acquire { scope, response }) => {
                let result = super::guarded(|| {
                    ensure!(failure.is_none(), "Mapping owner failed: {failure:?}");
                    backend.acquire(scope)?;
                    scopes.insert(scope, Scope::default());
                    Ok(json!({"acquired":true}))
                });
                respond(response, result);
            }
            Ok(Command::Request {
                scope,
                request,
                response,
            }) => {
                let result = super::guarded(|| {
                    let state = scopes.get(&scope).context("Mapping client scope changed")?;
                    ensure!(!state.releasing, "Mapping client scope is closing");
                    ensure!(failure.is_none(), "Mapping owner failed: {failure:?}");
                    backend.request(scope, request)
                });
                respond(response, result);
            }
            Ok(Command::Release {
                scope,
                response,
                released,
            }) => {
                if response.is_none() {
                    if let Some(state) = scopes.get_mut(&scope) {
                        state.abandoned = true;
                    }
                }
                let result = super::guarded(|| {
                    release_scope(
                        &mut backend,
                        &mut scopes,
                        scope,
                        released,
                        failure.as_deref(),
                    )
                });
                if let Some(response) = response {
                    respond(response, result);
                } else if let Err(error) = result {
                    eprintln!("Mapping abandoned scope cleanup failed: {error:#}");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if failure.is_none() {
            if let Err(error) = super::guarded(|| backend.poll()) {
                failure = Some(format!("Mapping actor poll failed: {error:#}"));
            }
        }
        let abandoned = scopes
            .iter()
            .filter_map(|(scope, state)| {
                (state.abandoned && state.retry_at.is_some_and(|due| Instant::now() >= due))
                    .then(|| {
                        state
                            .released
                            .as_ref()
                            .map(|released| (*scope, released.clone()))
                    })
                    .flatten()
            })
            .collect::<Vec<_>>();
        for (scope, released) in abandoned {
            let result = super::guarded(|| {
                release_scope(
                    &mut backend,
                    &mut scopes,
                    scope,
                    released.clone(),
                    failure.as_deref(),
                )
            });
            if released.load(Ordering::Acquire) {
                if let Err(error) = result {
                    eprintln!("Mapping abandoned scope cleanup completed with failures: {error:#}");
                }
            }
        }
    }
    for (scope, _) in scopes {
        if let Err(error) = super::guarded(|| backend.release(scope)) {
            eprintln!("Mapping final scope cleanup failed: {error:#}");
        }
    }
    if let Err(error) = super::guarded(|| backend.shutdown()) {
        eprintln!("Mapping actor shutdown failed: {error:#}");
    }
}

impl super::Transport for Client {
    fn request(&mut self, request: ServiceRequest) -> Result<Value> {
        Client::request(self, request)
    }
    fn is_stopped(&self) -> bool {
        Client::is_stopped(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Observations {
        starts: usize,
        shutdowns: usize,
        released: Vec<Uuid>,
        documents: Vec<String>,
        fail_release: bool,
        release_pending: bool,
        terminal_release_failure: bool,
        poll_failure: bool,
        poll_failure_observed: Option<mpsc::SyncSender<()>>,
        release_observed: Option<mpsc::Sender<Uuid>>,
    }
    struct TestBackend(Arc<Mutex<Observations>>);
    impl Backend for TestBackend {
        fn acquire(&mut self, _scope: Uuid) -> Result<()> {
            Ok(())
        }
        fn request(&mut self, _scope: Uuid, request: ServiceRequest) -> Result<Value> {
            let mut state = self.0.lock().unwrap();
            match request {
                ServiceRequest::MacroRecorderStorageWrite { key, .. } => {
                    state.documents.push(key);
                    Ok(json!({"published":true}))
                }
                ServiceRequest::GlobalMode => Ok(json!({"documents":state.documents})),
                _ => anyhow::bail!("Unexpected test request"),
            }
        }
        fn poll(&mut self) -> Result<()> {
            let mut state = self.0.lock().unwrap();
            if state.poll_failure {
                if let Some(observed) = state.poll_failure_observed.take() {
                    let _ = observed.send(());
                }
                anyhow::bail!("Test mapping polling failed");
            }
            Ok(())
        }
        fn release(&mut self, scope: Uuid) -> Result<()> {
            let mut state = self.0.lock().unwrap();
            if let Some(observed) = &state.release_observed {
                let _ = observed.send(scope);
            }
            if std::mem::take(&mut state.fail_release) {
                state.release_pending = true;
                anyhow::bail!("held output release failed");
            }
            state.release_pending = false;
            state.released.push(scope);
            ensure!(
                !state.terminal_release_failure,
                "Output restore failed after actual release"
            );
            Ok(())
        }
        fn release_cleanup_pending(&self, _scope: Uuid) -> bool {
            self.0.lock().unwrap().release_pending
        }
        fn shutdown(&mut self) -> Result<()> {
            self.0.lock().unwrap().shutdowns += 1;
            Ok(())
        }
    }
    fn acquire(registry: &Registry, state: &Arc<Mutex<Observations>>) -> Client {
        let state = state.clone();
        registry
            .acquire(move || {
                state.lock().unwrap().starts += 1;
                Ok(TestBackend(state))
            })
            .unwrap()
    }

    #[test]
    fn publication_and_release_share_one_owner_without_shutting_other_scopes() {
        let registry = Registry::default();
        let state = Arc::new(Mutex::new(Observations::default()));
        let mut device = acquire(&registry, &state);
        let mut recorder = acquire(&registry, &state);
        recorder
            .request(ServiceRequest::MacroRecorderStorageWrite {
                key: "macro-library".into(),
                value: json!([]),
            })
            .unwrap();
        assert_eq!(
            device.request(ServiceRequest::GlobalMode).unwrap()["documents"],
            json!(["macro-library"])
        );
        recorder.request(ServiceRequest::Shutdown).unwrap();
        assert!(recorder.is_stopped());
        drop(recorder);
        assert_eq!(state.lock().unwrap().shutdowns, 0);
        assert_eq!(
            device.request(ServiceRequest::GlobalMode).unwrap()["documents"],
            json!(["macro-library"])
        );
        device.request(ServiceRequest::Shutdown).unwrap();
        drop(device);
        let state = state.lock().unwrap();
        assert_eq!(state.starts, 1);
        assert_eq!(state.shutdowns, 1);
        assert_eq!(state.released.len(), 2);
        assert_ne!(state.released[0], state.released[1]);
    }

    #[test]
    fn failed_scope_release_is_not_acknowledged_or_forgotten() {
        let registry = Registry::default();
        let state = Arc::new(Mutex::new(Observations {
            fail_release: true,
            ..Default::default()
        }));
        let mut client = acquire(&registry, &state);
        assert!(
            client
                .request(ServiceRequest::Shutdown)
                .unwrap_err()
                .to_string()
                .contains("held output")
        );
        assert!(!client.is_stopped());
        assert!(state.lock().unwrap().released.is_empty());
        assert!(client.request(ServiceRequest::GlobalMode).is_err());
        assert!(client.request(ServiceRequest::Shutdown).is_err());
        assert!(client.is_stopped());
        drop(client);
        let state = state.lock().unwrap();
        assert_eq!(state.released.len(), 1);
        assert_eq!(state.shutdowns, 1);
    }

    #[test]
    fn terminal_restore_failure_releases_scope_without_acknowledging_success() {
        let registry = Registry::default();
        let state = Arc::new(Mutex::new(Observations {
            terminal_release_failure: true,
            ..Default::default()
        }));
        let mut client = acquire(&registry, &state);
        assert!(
            client
                .request(ServiceRequest::Shutdown)
                .unwrap_err()
                .to_string()
                .contains("actual release")
        );
        assert!(client.is_stopped());
        assert!(client.request(ServiceRequest::Shutdown).is_err());
        drop(client);
        let observed = state.lock().unwrap();
        assert_eq!(observed.released.len(), 1);
        assert_eq!(observed.shutdowns, 1);
    }

    #[test]
    fn polling_failure_keeps_the_actor_available_for_real_cleanup() {
        let registry = Registry::default();
        let state = Arc::new(Mutex::new(Observations::default()));
        let mut client = acquire(&registry, &state);
        let (observed, poll_failed) = mpsc::sync_channel(1);
        {
            let mut state = state.lock().unwrap();
            state.poll_failure = true;
            state.poll_failure_observed = Some(observed);
        }
        let _ = client.request(ServiceRequest::GlobalMode);
        poll_failed.recv().unwrap();
        assert!(client.request(ServiceRequest::GlobalMode).is_err());
        assert!(client.request(ServiceRequest::Shutdown).is_err());
        assert!(client.is_stopped());
        drop(client);
        let observed = state.lock().unwrap();
        assert_eq!(observed.released.len(), 1);
        assert_eq!(observed.shutdowns, 1);
    }

    #[test]
    fn abandoned_pending_scope_is_retried_while_another_client_remains_live() {
        let registry = Registry::default();
        let state = Arc::new(Mutex::new(Observations::default()));
        let mut live = acquire(&registry, &state);
        let abandoned = acquire(&registry, &state);
        let abandoned_scope = abandoned.scope;
        let (observed, releases) = mpsc::channel();
        {
            let mut state = state.lock().unwrap();
            state.fail_release = true;
            state.release_observed = Some(observed);
        }
        drop(abandoned);
        assert_eq!(releases.recv().unwrap(), abandoned_scope);
        assert_eq!(releases.recv().unwrap(), abandoned_scope);
        assert!(live.request(ServiceRequest::GlobalMode).is_ok());
        assert_eq!(state.lock().unwrap().released, [abandoned_scope]);
        live.request(ServiceRequest::Shutdown).unwrap();
        drop(live);
        assert_eq!(state.lock().unwrap().shutdowns, 1);
    }

    #[test]
    fn drop_releases_scope_and_next_generation_initializes_after_cleanup() {
        let registry = Registry::default();
        let state = Arc::new(Mutex::new(Observations::default()));
        drop(acquire(&registry, &state));
        {
            let observed = state.lock().unwrap();
            assert_eq!(observed.released.len(), 1);
            assert_eq!(observed.shutdowns, 1);
        }
        drop(acquire(&registry, &state));
        let observed = state.lock().unwrap();
        assert_eq!(observed.starts, 2);
        assert_eq!(observed.shutdowns, 2);
        assert_ne!(observed.released[0], observed.released[1]);
    }
}
