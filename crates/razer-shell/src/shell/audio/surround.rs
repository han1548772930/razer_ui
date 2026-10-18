//! One retained worker owns a product's local routing session, preference writes,
//! current endpoint observations and cooperative restoration.
use super::*;
use anyhow::{Context as _, Result, ensure};
use razer_device::audio_surround::{
    Endpoint, Identity, Outcome, Product, Settings,
    observed::{ObservationReply, ServiceObservation},
};
use razer_device::simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{SurroundObservation, SurroundOperation, SurroundRequest};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) struct Owner {
    identity: Identity,
    alive: Arc<AtomicBool>,
    sender: mpsc::Sender<SurroundRequest>,
    worker: Option<JoinHandle<Result<()>>>,
    _pump: Task<()>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}
struct Delivery {
    revision: Option<u64>,
    result: Result<SurroundObservation, String>,
}
#[derive(Deserialize)]
struct EndpointRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
}
#[derive(Deserialize)]
struct Changed {
    identity: Identity,
    service: ServiceObservation,
    outcomes: Vec<Outcome>,
}
#[derive(Deserialize)]
struct Closed {
    identity: Identity,
    closed: bool,
    cleanup_failure: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Preferences {
    version: u32,
    settings: Settings,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

fn local_key(identity: &Identity) -> String {
    format!(
        "surround:{}:{}",
        u32::from(identity.product()),
        identity.container_id().to_ascii_lowercase()
    )
}
fn preferences(identity: &Identity) -> Result<Settings> {
    match razer_storage::source_local::get(
        &razer_storage::source_local::path(),
        &local_key(identity),
    )? {
        Some(value) => {
            let stored: Preferences = serde_json::from_value(value)?;
            ensure!(
                stored.version == 1,
                "Unsupported Surround preference version"
            );
            Ok(stored.settings)
        }
        None => Ok(Settings::default()),
    }
}
fn persist(identity: &Identity, change: &razer_device::audio_surround::Change) -> Result<()> {
    if !change.is_persisted() {
        return Ok(());
    }
    razer_storage::source_local::update(
        &razer_storage::source_local::path(),
        &local_key(identity),
        |previous| {
            let mut stored = match previous {
                Some(value) => serde_json::from_value::<Preferences>(value)?,
                None => Preferences {
                    version: 1,
                    settings: Settings::default(),
                    extensions: BTreeMap::new(),
                },
            };
            ensure!(
                stored.version == 1,
                "Unsupported Surround preference version"
            );
            stored.settings.apply(change);
            Ok(Some(serde_json::to_value(stored)?))
        },
    )?;
    Ok(())
}

fn source_device_name(product: Product) -> &'static str {
    // Current DeviceInfo.deviceName, used by MW updateToggleAudioOutput.
    match product {
        Product::Clio => "Clio",
        Product::Phantom => "Razer Carol T2",
        Product::VirtualRoute => "Razer Soma Pro",
    }
}
fn observe(
    client: &mut ServiceClient,
    identity: &Identity,
    reply: Option<ObservationReply>,
    mut failure: Option<String>,
) -> Result<SurroundObservation> {
    let reply = match reply {
        Some(reply) => reply,
        None => serde_json::from_value(client.request(ServiceRequest::SurroundObserve {
            identity: identity.clone(),
        })?)?,
    };
    ensure!(
        reply.identity() == identity,
        "Surround publication owner differs"
    );
    if !reply.service().connected {
        return Ok(SurroundObservation::new(reply, vec![], None, failure));
    }
    let candidates = (|| -> Result<Vec<Endpoint>> {
        let endpoints = serde_json::from_value::<Vec<EndpointRecord>>(
            client.request(ServiceRequest::AudioDevices)?,
        )?;
        endpoints
            .into_iter()
            .filter(|e| {
                e.kind == "speaker" && !e.name.contains(source_device_name(identity.product()))
            })
            .filter(|e| {
                identity.product() != Product::VirtualRoute
                    || !e.name.trim().to_lowercase().contains("marcit1")
            })
            .map(|e| Endpoint::new(e.id, e.name))
            .collect::<Result<Vec<_>>>()
    })();
    let candidates = match candidates {
        Ok(candidates) => candidates,
        Err(error) => {
            let error = format!("{error:#}");
            failure =
                Some(failure.map_or(error.clone(), |previous| format!("{previous}; {error}")));
            Vec::new()
        }
    };
    let master = reply
        .service()
        .observations
        .endpoint_with_role(1)
        .and_then(|endpoint| {
            let reading: AudioVolumeReadResult =
                serde_json::from_value(client.request(ServiceRequest::AudioVolumeRead {
                    device_id: endpoint.endpoint_id.clone(),
                })?)?;
            ensure!(
                reading.device_id == endpoint.endpoint_id,
                "Surround master endpoint response differs"
            );
            ensure!(
                reading.result && reading.volume <= 100,
                "Surround master volume is unobserved: {}",
                reading.reason
            );
            Ok(reading)
        });
    let master = match master {
        Ok(value) => Some(value),
        Err(error) => {
            let error = format!("{error:#}");
            failure =
                Some(failure.map_or(error.clone(), |previous| format!("{previous}; {error}")));
            None
        }
    };
    Ok(SurroundObservation::new(reply, candidates, master, failure))
}

fn run(
    initial: SurroundRequest,
    commands: mpsc::Receiver<SurroundRequest>,
    deliveries: mpsc::Sender<Delivery>,
    previous: Option<JoinHandle<Result<()>>>,
) -> Result<()> {
    if let Some(previous) = previous {
        previous
            .join()
            .map_err(|_| anyhow::anyhow!("Previous Surround owner panicked"))??;
    }
    ensure!(initial.is_current(), "Surround scope closed before opening");
    let identity = initial.identity().clone();
    let mut client = ServiceClient::spawn()?;
    client.set_request_guard(initial.guard());
    let mut opened = false;
    let result = (|| {
        let settings = preferences(&identity)?;
        let reply: ObservationReply =
            serde_json::from_value(client.request(ServiceRequest::SurroundOpen {
                identity: identity.clone(),
                settings,
            })?)?;
        opened = true;
        let observation = observe(&mut client, &identity, Some(reply), None);
        deliveries.send(Delivery {
            revision: Some(initial.revision()),
            result: observation.map_err(|e| format!("{e:#}")),
        })?;
        while initial.is_current() {
            let request = match commands.recv_timeout(Duration::from_millis(200)) {
                Ok(request) => Some(request),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if !initial.is_current() {
                break;
            }
            let mut reply = None;
            let mut failure = None;
            if let Some(request) = &request {
                ensure!(
                    request.identity() == &identity && request.is_current(),
                    "Surround request belongs to an expired owner"
                );
                let operation = (|| -> Result<()> {
                    match request.operation() {
                        SurroundOperation::Open => {}
                        SurroundOperation::Change(change) => {
                            persist(&identity, change)?;
                            let response: Changed = serde_json::from_value(client.request(
                                ServiceRequest::SurroundChange {
                                    identity: identity.clone(),
                                    change: change.clone(),
                                },
                            )?)?;
                            ensure!(
                                response.identity == identity,
                                "Surround mutation response owner differs"
                            );
                            reply =
                                Some(ObservationReply::new(response.identity, response.service));
                            ensure!(
                                !response.outcomes.is_empty(),
                                "Surround mutation has no completion receipt"
                            );
                            for outcome in response.outcomes {
                                ensure!(
                                    outcome.is_completed(),
                                    "{}",
                                    outcome
                                        .failure()
                                        .unwrap_or("Surround operation was not completed")
                                );
                            }
                        }
                        SurroundOperation::Master { mute, volume } => {
                            let state: ObservationReply = serde_json::from_value(client.request(
                                ServiceRequest::SurroundObserve {
                                    identity: identity.clone(),
                                },
                            )?)?;
                            ensure!(
                                state.identity() == &identity && state.service().connected,
                                "Surround master scope is disconnected"
                            );
                            let endpoint = state
                                .service()
                                .observations
                                .endpoint_with_role(1)?
                                .endpoint_id
                                .clone();
                            let written: AudioVolumeWriteResult = serde_json::from_value(
                                client.request(ServiceRequest::AudioVolumeWrite {
                                    device_id: endpoint.clone(),
                                    mute: *mute,
                                    volume: *volume,
                                })?,
                            )?;
                            ensure!(
                                written.previous.device_id == endpoint,
                                "Surround master write endpoint differs"
                            );
                            ensure!(
                                written.source_response.result,
                                "Surround master write failed: {}",
                                written.source_response.reason
                            );
                            let actual = written
                                .observation
                                .context("Surround master readback is absent")?;
                            ensure!(
                                actual.result
                                    && actual.device_id == endpoint
                                    && actual.volume == *volume
                                    && actual.muted == *mute,
                                "Surround master readback differs"
                            );
                        }
                    }
                    Ok(())
                })();
                failure = operation.err().map(|e| format!("{e:#}"));
            }
            let observation = observe(&mut client, &identity, reply, failure);
            deliveries.send(Delivery {
                revision: request.as_ref().map(|r| r.revision()),
                result: observation.map_err(|e| format!("{e:#}")),
            })?;
        }
        Ok::<_, anyhow::Error>(())
    })();
    client.set_request_guard(Arc::new(AtomicBool::new(true)));
    let cleanup = (|| -> Result<()> {
        if opened && !client.is_stopped() {
            let closed: Closed =
                serde_json::from_value(client.request(ServiceRequest::SurroundClose {
                    identity: identity.clone(),
                })?)?;
            ensure!(
                closed.identity == identity && closed.closed,
                "Surround cleanup owner differs"
            );
            ensure!(
                closed.cleanup_failure.is_none(),
                "Surround cleanup incomplete: {:?}",
                closed.cleanup_failure
            );
        }
        Ok(())
    })();
    let shutdown = if client.is_stopped() {
        Ok(serde_json::Value::Null)
    } else {
        client.request(ServiceRequest::Shutdown)
    };
    let failures = [result.err(), cleanup.err(), shutdown.err()]
        .into_iter()
        .flatten()
        .map(|e| format!("{e:#}"))
        .collect::<Vec<_>>();
    ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
}

impl AppShell {
    pub(super) fn request_surround(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: SurroundRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if self.close_requested || !page.read(cx).surround_request_current(&request) {
            return;
        }
        let key = request.identity().container_id().to_ascii_lowercase();
        if !matches!(request.operation(), SurroundOperation::Open) {
            let sent = self
                .surround_owners
                .get(&key)
                .filter(|owner| owner.identity == *request.identity())
                .context("Surround worker is absent")
                .and_then(|owner| owner.sender.send(request.clone()).map_err(Into::into));
            if let Err(error) = sent {
                page.update(cx, |page, cx| {
                    page.finish_surround(
                        request.identity(),
                        Some(request.revision()),
                        Err(format!("{error:#}")),
                        window,
                        cx,
                    )
                });
            }
            return;
        }
        if !self.surround_cleanup_installed {
            self.surround_cleanup_installed = true;
            self.register_exit_cleanup(|shell, cx| {
                let mut owners = std::mem::take(&mut shell.surround_owners);
                for owner in owners.values_mut() {
                    owner.alive.store(false, Ordering::Release);
                }
                let workers = owners
                    .values_mut()
                    .filter_map(|owner| owner.worker.take())
                    .collect::<Vec<_>>();
                cx.background_spawn(async move {
                    for worker in workers {
                        match worker.join() {
                            Ok(Ok(())) => {}
                            result => eprintln!("Surround exit cleanup: {result:?}"),
                        }
                    }
                })
            });
        }
        let previous = self.surround_owners.remove(&key).and_then(|mut owner| {
            owner.alive.store(false, Ordering::Release);
            owner.worker.take()
        });
        let (sender, commands) = mpsc::channel();
        let (delivery, events) = mpsc::channel();
        let initial = request.clone();
        let worker = thread::Builder::new()
            .name("razer-surround-client".into())
            .spawn(move || {
                let revision = initial.revision();
                let result = run(initial, commands, delivery.clone(), previous);
                if let Err(error) = &result {
                    let _ = delivery.send(Delivery {
                        revision: Some(revision),
                        result: Err(format!("{error:#}")),
                    });
                }
                result
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                page.update(cx, |page, cx| {
                    page.finish_surround(
                        request.identity(),
                        Some(request.revision()),
                        Err(error.to_string()),
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let identity = request.identity().clone();
        let alive = request.guard();
        let guard = alive.clone();
        let scope = identity.clone();
        let window_handle = window.window_handle();
        let pump = cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let mut disconnected = false;
                loop {
                    match events.try_recv() {
                        Ok(delivery) => {
                            let _ = window_handle.update(cx, |_, window, cx| {
                                page.update(cx, |page, cx| {
                                    page.finish_surround(
                                        &scope,
                                        delivery.revision,
                                        delivery.result,
                                        window,
                                        cx,
                                    )
                                })
                            });
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                    }
                }
                if disconnected || !guard.load(Ordering::Acquire) {
                    break;
                }
                let current = shell.update(cx, |shell, cx| {
                    shell.devices.contains(&workspace)
                        && workspace.read(cx).device(cx).device_container_id == scope.container_id()
                });
                if !matches!(current, Ok(true)) {
                    guard.store(false, Ordering::Release);
                    break;
                }
            }
        });
        self.surround_owners.insert(
            key,
            Owner {
                identity,
                alive,
                sender,
                worker: Some(worker),
                _pump: pump,
            },
        );
    }
}
