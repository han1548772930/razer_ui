//! Current trigger intent -> owned worker -> genuine progress/storage observations.
use super::*;
use anyhow::{Context as _, ensure};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::gamepad_products::{
    TriggerCalibrationIntent, TriggerCalibrationObservation,
};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(super) struct Session {
    generation: u64,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    _pump: Task<()>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Session {
    fn retire(mut self) -> Option<JoinHandle<()>> {
        self.canceled.store(true, Ordering::Release);
        self.worker.take()
    }
}

enum Event {
    Accepted,
    Poll(Value),
    Failure(String),
}
fn send(events: &SyncSender<Event>, event: Event) -> anyhow::Result<()> {
    events
        .try_send(event)
        .context("Calibration observation consumer unavailable")
}

fn run(
    route: razer_discovery::discovery::ObservedDevice,
    part: u8,
    canceled: &AtomicBool,
    events: &SyncSender<Event>,
) -> anyhow::Result<()> {
    if canceled.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut client = ServiceClient::spawn()?;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let mut started = false;
    let mut terminal = false;
    let result = (|| {
        let node = razer_discovery::controller_calibration::resolve(&mut client, &route)?;
        if canceled.load(Ordering::Acquire) {
            return Ok(());
        }
        let reply = client.request(ServiceRequest::ControllerTriggerCalibrationStart {
            operation_id: operation_id.clone(),
            node: node.clone(),
            product_id: route.product_id(),
            part_id: part,
        })?;
        started = true;
        ensure!(
            reply["operation_id"] == operation_id
                && reply["part_id"] == part
                && reply["state"] == "running"
                && reply["terminal"] == false,
            "Calibration acceptance does not match the request"
        );
        send(events, Event::Accepted)?;
        let mut cancel_sent = false;
        loop {
            if canceled.load(Ordering::Acquire) && !cancel_sent {
                let reply = client.request(ServiceRequest::ControllerTriggerCalibrationCancel {
                    operation_id: operation_id.clone(),
                })?;
                ensure!(
                    reply["operation_id"] == operation_id && reply["state"] == "canceling",
                    "Calibration cancellation does not match the request"
                );
                cancel_sent = true;
            }
            let reply = client.request(ServiceRequest::ControllerTriggerCalibrationPoll {
                operation_id: operation_id.clone(),
            })?;
            ensure!(
                reply["operation_id"] == operation_id && reply["observations"].is_array(),
                "Calibration poll does not match the request"
            );
            terminal = reply["terminal"]
                .as_bool()
                .context("Missing calibration terminal observation")?;
            if terminal {
                ensure!(
                    matches!(
                        reply["state"].as_str(),
                        Some("completed" | "failed" | "canceled")
                    ),
                    "Unknown calibration terminal state"
                );
                if reply["state"] == "completed" {
                    ensure!(
                        reply["result"]["node"] == serde_json::to_value(&node)?
                            && reply["result"]["product_id"] == route.product_id()
                            && reply["result"]["part_id"] == part
                            && reply["result"]["device_write_verified"] == true,
                        "Calibration completion lacks exact device readback"
                    );
                }
            } else {
                ensure!(
                    matches!(reply["state"].as_str(), Some("running" | "canceling")),
                    "Unknown calibration running state"
                );
            }
            // Apply observed progress before handling its terminal outcome.
            if !reply["observations"].as_array().unwrap().is_empty() || terminal {
                send(events, Event::Poll(reply))?;
            }
            if terminal {
                break;
            }
            thread::sleep(Duration::from_millis(33));
        }
        Ok::<_, anyhow::Error>(())
    })();
    if started && !terminal {
        let _ = client.request(ServiceRequest::ControllerTriggerCalibrationCancel { operation_id });
    }
    // Worker Drop cancels and joins any operation before Shutdown replies.
    let shutdown = client.request(ServiceRequest::Shutdown);
    result.and(shutdown.map(|_| ()))
}

impl AppShell {
    pub(super) fn install_controller_calibration_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let sessions = std::mem::take(&mut this.controller_calibrations);
            let mut retired = std::mem::take(&mut this.controller_calibration_cleanup);
            let stopped = std::mem::take(&mut this.controller_calibration_stop_cleanup);
            for (_, session) in sessions {
                retired.extend(session.retire());
            }
            let cleanup = cx.background_executor().spawn(async move {
                for worker in retired {
                    let _ = worker.join();
                }
                for (task, _) in stopped {
                    task.await;
                }
            });
            async move { cleanup.await }
        }));
    }

    pub(super) fn request_controller_calibration(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: TriggerCalibrationIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = request.generation();
        if workspace.read(cx).trigger_calibration_generation(cx) != Some(generation) {
            return;
        }
        if request.payload()["action"] != "stop" {
            self.initialize_controller_calibration_focus(&workspace, window, cx);
        }
        // Retain pending teardown without accumulating finished thread handles.
        let mut pending = Vec::new();
        for worker in std::mem::take(&mut self.controller_calibration_cleanup) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.controller_calibration_cleanup = pending;
        self.controller_calibration_stop_cleanup
            .retain(|(_, done)| !done.load(Ordering::Acquire));
        let identity = workspace.read(cx).identity(cx);
        let previous = self
            .controller_calibrations
            .remove(&identity)
            .and_then(Session::retire);
        if request.payload()["action"] == "stop" {
            // Stop acknowledgement follows actual worker cleanup; it is never
            // converted into a device completion or synthetic Step_Error.
            let (finished, receipt) = mpsc::channel();
            let done = Arc::new(AtomicBool::new(false));
            let completed = Arc::clone(&done);
            let cleanup = cx.background_executor().spawn(async move {
                let outcome = previous.map_or(Ok(()), |worker| {
                    worker
                        .join()
                        .map_err(|_| "Calibration cleanup thread failed".to_owned())
                });
                let _ = finished.send(outcome);
                completed.store(true, Ordering::Release);
            });
            self.controller_calibration_stop_cleanup
                .push((cleanup, done));
            cx.spawn_in(window, async move |_, cx| {
                let outcome = loop {
                    match receipt.try_recv() {
                        Ok(outcome) => break outcome,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            break Err("Calibration cleanup result unavailable".into());
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            cx.background_executor()
                                .timer(Duration::from_millis(33))
                                .await
                        }
                    }
                };
                let _ = workspace.update(cx, |workspace, cx| {
                    workspace.finish_trigger_calibration_submission(generation, outcome, cx)
                });
            })
            .detach();
            return;
        }
        let device = workspace.read(cx).device(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(device))
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.controller_calibration_cleanup.extend(previous);
            workspace.update(cx, |workspace, cx| {
                workspace.finish_trigger_calibration_submission(
                    generation,
                    Err("No unique current calibration device identity".into()),
                    cx,
                )
            });
            return;
        };
        let route = (**route).clone();
        let container = device.device_container_id.clone();
        let physical_pid = route.physical_product_id();
        let retained_node = route.hid_node().cloned();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&canceled);
        let (events, receiver) = mpsc::sync_channel(512);
        // Keep the retired handle recoverable if OS thread creation fails.
        let predecessor = Arc::new(std::sync::Mutex::new(previous));
        let previous_worker = Arc::clone(&predecessor);
        let worker = match thread::Builder::new()
            .name("controller-calibration-owner".into())
            .spawn(move || {
                if let Some(previous) = previous_worker.lock().unwrap().take() {
                    let _ = previous.join();
                }
                if let Err(error) = run(route, request.part(), &signal, &events) {
                    let _ = send(&events, Event::Failure(format!("{error:#}")));
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.controller_calibration_cleanup
                    .extend(predecessor.lock().unwrap().take());
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_trigger_calibration_submission(
                        generation,
                        Err(error.to_string()),
                        cx,
                    )
                });
                return;
            }
        };
        let owner = identity.clone();
        let pump = cx.spawn_in(window, async move |this, cx| {
            let mut sequence = 0_u64;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let mut incoming = Vec::new();
                let disconnected = loop {
                    match receiver.try_recv() {
                        Ok(event) => incoming.push(event),
                        Err(mpsc::TryRecvError::Empty) => break false,
                        Err(mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                let keep = this.update_in(cx, |this, window, cx| {
                    if !this
                        .controller_calibrations
                        .get(&owner)
                        .is_some_and(|s| s.generation == generation)
                    {
                        return false;
                    }
                    let current = this.devices.contains(&workspace)
                        && workspace.read(cx).identity(cx) == owner
                        && workspace.read(cx).trigger_calibration_generation(cx)
                            == Some(generation)
                        && this.device_observations.iter().any(|route| {
                            route.matches(workspace.read(cx).device(cx))
                                && route.container().eq_ignore_ascii_case(&container)
                                && route.physical_product_id() == physical_pid
                                && route.hid_node() == retained_node.as_ref()
                        });
                    if !current {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_trigger_calibration_submission(
                                generation,
                                Err("Calibration device identity changed".into()),
                                cx,
                            )
                        });
                    } else {
                        for event in incoming {
                            match event {
                                Event::Accepted => {
                                    workspace.update(cx, |w, cx| {
                                        w.finish_trigger_calibration_submission(
                                            generation,
                                            Ok(()),
                                            cx,
                                        )
                                    });
                                }
                                Event::Failure(error) => {
                                    workspace.update(cx, |w, cx| {
                                        w.finish_trigger_calibration_submission(
                                            generation,
                                            Err(error),
                                            cx,
                                        )
                                    });
                                }
                                Event::Poll(reply) => {
                                    for value in reply["observations"].as_array().unwrap() {
                                        sequence = sequence.wrapping_add(1);
                                        let observation = match value["kind"].as_str() {
                                            Some("progress") => value["payload"]["step"]
                                                .as_i64()
                                                .and_then(|s| i8::try_from(s).ok())
                                                .zip(value["payload"]["isStepValid"].as_bool())
                                                .and_then(|(step, valid)| {
                                                    TriggerCalibrationObservation::progress(
                                                        generation, sequence, step, valid, None,
                                                    )
                                                }),
                                            Some("movement") => {
                                                value["payload"]["t"].as_f64().and_then(|t| {
                                                    TriggerCalibrationObservation::user_movement(
                                                        generation, sequence, t,
                                                    )
                                                })
                                            }
                                            // Source storage listener ignores null newValue;
                                            // removal must preserve its last actual trigger t.
                                            Some("remove_movement") => None,
                                            _ => None,
                                        };
                                        if let Some(observation) = observation {
                                            workspace.update(cx, |w, cx| {
                                                w.observe_trigger_calibration(
                                                    observation,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }
                                    }
                                    if reply["terminal"] == true && reply["state"] == "failed" {
                                        let error = reply["result"]["error"]
                                            .as_str()
                                            .or_else(|| reply["error"].as_str())
                                            .unwrap_or("Calibration operation failed")
                                            .to_owned();
                                        workspace.update(cx, |w, cx| {
                                            w.finish_trigger_calibration_submission(
                                                generation,
                                                Err(error),
                                                cx,
                                            )
                                        });
                                    }
                                }
                            }
                        }
                    }
                    if !current || disconnected {
                        this.controller_calibration_cleanup.extend(
                            this.controller_calibrations
                                .remove(&owner)
                                .and_then(Session::retire),
                        );
                        return false;
                    }
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        });
        self.controller_calibrations.insert(
            identity,
            Session {
                generation,
                canceled,
                worker: Some(worker),
                _pump: pump,
            },
        );
    }
}
