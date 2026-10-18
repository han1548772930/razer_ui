//! Device setter chains and source local storage are independent source events.
use super::*;
use razer_device::mouse_tracking::{Progress, SmartTracking};
use razer_discovery::direct;
use razer_ipc::ServiceClient;
use razer_pages::features::mouse_products::MouseCalibrationRequested;

fn calibration_completion_current(
    owner_current: bool,
    request_current: bool,
    retained: &razer_discovery::discovery::ObservedDevice,
    observations: &[razer_discovery::discovery::ObservedDevice],
    device: &Device,
) -> bool {
    owner_current
        && request_current
        && super::device_discovery::retained_connection(
            std::slice::from_ref(retained),
            observations,
            device,
        )
}

pub(super) struct TrackingTask {
    request: MouseCalibrationRequested,
    _task: Task<()>,
}

#[derive(Clone, Copy)]
enum HardwareSetting {
    Orca(SmartTracking),
    Philips(razer_device::mouse_philips::Setting),
}
enum HardwareProgress {
    Orca(Progress),
    Philips(razer_device::mouse_philips::Progress),
}
impl HardwareSetting {
    fn available(self, route: &razer_discovery::discovery::ObservedDevice) -> bool {
        match self {
            Self::Orca(_) => direct::mouse_tracking::available(route),
            Self::Philips(_) => direct::mouse_philips::available(route),
        }
    }
    fn request(
        self,
        client: &mut ServiceClient,
        route: &razer_discovery::discovery::ObservedDevice,
        operation_id: String,
        valid: &std::sync::atomic::AtomicBool,
    ) -> (Option<HardwareProgress>, Option<String>) {
        match self {
            Self::Orca(tracking) => {
                let result = direct::mouse_tracking::request(
                    client,
                    route,
                    operation_id,
                    tracking.setting(),
                    valid,
                );
                (result.progress.map(HardwareProgress::Orca), result.error)
            }
            Self::Philips(setting) => {
                let result =
                    direct::mouse_philips::request(client, route, operation_id, setting, valid);
                (result.progress.map(HardwareProgress::Philips), result.error)
            }
        }
    }
}
impl Drop for TrackingTask {
    fn drop(&mut self) {
        self.request.cancel();
    }
}
pub(super) struct TrackingSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: MouseCalibrationRequested,
    tracking: SmartTracking,
}
impl TrackingSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}
impl AppShell {
    pub(super) fn dispatch_mouse_calibration(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.philips().is_some() || request.measurement_save().is_some() {
            self.persist_mouse_philips(workspace, request, window, cx);
            return;
        }
        if let Some(kind) = request.measurement() {
            self.submit_philips_measurement(workspace, request, kind, window, cx);
            return;
        }
        if request.product_id() == 70
            && matches!(
                request.command(),
                "ON_CALIBRATION_START" | "ON_CALIBRATION_ABORTED"
            )
        {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_mouse_philips_context(&request, cx)
            });
            return;
        }
        let Some(tracking) = request.tracking() else {
            workspace.update(cx, |workspace, cx| {
                workspace.reject_mouse_calibration(
                    &request,
                    format!(
                        "Calibration source branch {} for product {} has no connected transport",
                        request.command(),
                        request.product_id()
                    ),
                    cx,
                )
            });
            return;
        };
        if !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_calibration_request_matches(&request, cx)
            || self.close_requested
        {
            request.cancel();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_calibration(&request, None, cx)
            });
            return;
        }
        match request.command() {
            "SET_SMART_TRACKING_VALUE" => {
                self.submit_mouse_tracking(workspace, request, tracking, cx)
            }
            "SET_SMART_CALIBRATION_DATA_TO_LOCAL_STORAGE_FROM_UI" => {
                self.persist_mouse_tracking(workspace, request, tracking, window, cx)
            }
            _ => workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_calibration(
                    &request,
                    Some("Unknown typed tracking action".into()),
                    cx,
                )
            }),
        }
    }

    fn persist_mouse_tracking(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        tracking: SmartTracking,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(error) = self.storage_error.clone() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_calibration(&request, Some(error), cx)
            });
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|workspace| workspace.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::MouseTracking(TrackingSave {
                workspace,
                identity,
                request,
                tracking,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_mouse_tracking_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MouseTracking(tracking)) = &save.product else {
            return Ok(());
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| {
                let mut device = workspace.read(cx).saved_snapshot(cx);
                if workspace == &tracking.workspace
                    && workspace.read(cx).identity(cx) == tracking.identity
                {
                    tracking
                        .tracking
                        .merge_local_settings(&mut device.source_device_settings)?;
                }
                Ok(device)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }

    pub(super) fn finish_mouse_tracking_save(
        &mut self,
        save: TrackingSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let error = (!succeeded).then(|| {
            "Smart tracking local storage write failed; editor values are retained".into()
        });
        save.workspace.update(cx, |workspace, cx| {
            workspace.finish_mouse_calibration(&save.request, error, cx)
        });
    }

    fn submit_mouse_tracking(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        tracking: SmartTracking,
        cx: &mut Context<Self>,
    ) {
        self.submit_mouse_calibration_worker(
            workspace,
            request,
            HardwareSetting::Orca(tracking),
            cx,
        );
    }
    pub(super) fn submit_mouse_philips(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        setting: razer_device::mouse_philips::Setting,
        cx: &mut Context<Self>,
    ) {
        self.submit_mouse_calibration_worker(
            workspace,
            request,
            HardwareSetting::Philips(setting),
            cx,
        );
    }
    fn submit_philips_measurement(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        kind: razer_device::mouse_philips::measurement::Kind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_calibration_request_matches(&request, cx)
        {
            request.cancel();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_calibration(&request, None, cx)
            });
            return;
        }
        let routes: Vec<_> = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && direct::philips_measurement::available(route)
            })
            .collect();
        if routes.len() != 1 {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_calibration(
                    &request,
                    Some("Philips measurement has no unique current native USB route".into()),
                    cx,
                )
            });
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let key = format!(
            "philips-measurement-{}-{}-{}",
            workspace.entity_id().as_u64(),
            request.request_id(),
            uuid::Uuid::new_v4()
        );
        let operation_id = key.clone();
        let weak = workspace.downgrade();
        let guard = request.clone();
        let retained_request = request.clone();
        let retained_key = key.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(32);
        let task = cx.spawn_in(window, async move |owner, cx| {
            let selected = route.clone();
            let background = cx.background_spawn(async move {
                use std::sync::mpsc::TrySendError;
                direct::philips_measurement::run(
                    &selected,
                    operation_id,
                    kind,
                    &guard.guard(),
                    |progress| match sender.try_send(progress) {
                        Ok(()) | Err(TrySendError::Full(_)) => Ok(()),
                        Err(TrySendError::Disconnected(_)) => {
                            anyhow::bail!("Philips measurement page observer closed")
                        }
                    },
                )
            });
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let mut latest = None;
                let disconnected = loop {
                    match receiver.try_recv() {
                        Ok(progress) => latest = Some(progress),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
                    }
                };
                let delivered = owner.update_in(cx, |owner, window, cx| {
                    let Some(workspace) = weak.upgrade() else {
                        request.cancel();
                        return;
                    };
                    let current = calibration_completion_current(
                        owner.devices.contains(&workspace)
                            && workspace.read(cx).identity(cx) == identity,
                        workspace
                            .read(cx)
                            .mouse_calibration_request_matches(&request, cx),
                        &route,
                        &owner.device_observations,
                        workspace.read(cx).device(cx),
                    );
                    if !current {
                        request.cancel();
                    } else if let Some(progress) = latest {
                        // Snapshots contain the cumulative notification/ACK
                        // journal, so bounded delivery can coalesce old frames.
                        workspace.update(cx, |workspace, cx| {
                            workspace
                                .record_mouse_philips_measurement(&request, progress, window, cx)
                        });
                    }
                });
                if delivered.is_err() {
                    request.cancel();
                }
                if disconnected {
                    break;
                }
            }
            let result = background.await;
            let _ = owner.update_in(cx, |owner, window, cx| {
                if let Some(workspace) = weak.upgrade() {
                    let current = calibration_completion_current(
                        owner.devices.contains(&workspace)
                            && workspace.read(cx).identity(cx) == identity,
                        workspace
                            .read(cx)
                            .mouse_calibration_request_matches(&request, cx),
                        &route,
                        &owner.device_observations,
                        workspace.read(cx).device(cx),
                    );
                    let (progress, error) = match result {
                        Ok(completion) => (completion.progress, completion.error),
                        Err(error) => (None, Some(format!("Philips measurement: {error:#}"))),
                    };
                    if !current {
                        request.cancel();
                    }
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_philips_measurement(
                            &request,
                            progress,
                            current.then_some(error).flatten(),
                            window,
                            cx,
                        )
                    });
                }
                owner.mouse_tracking_tasks.remove(&key);
                if owner.close_requested && owner.save_task.is_none() {
                    owner.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        });
        self.mouse_tracking_tasks.insert(
            retained_key,
            TrackingTask {
                request: retained_request,
                _task: task,
            },
        );
    }
    fn submit_mouse_calibration_worker(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        setting: HardwareSetting,
        cx: &mut Context<Self>,
    ) {
        let routes: Vec<_> = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx)) && setting.available(route)
            })
            .collect();
        if routes.len() != 1 {
            workspace.update(cx, |workspace, cx| workspace.finish_mouse_calibration(&request, Some("Tracking device has no unique live wired or receiver route; no hardware write was confirmed".into()), cx));
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let key = format!(
            "tracking-{}-{}",
            workspace.entity_id().as_u64(),
            request.request_id()
        );
        let operation_id = key.clone();
        let weak = workspace.downgrade();
        let guard = request.clone();
        let retained_request = request.clone();
        let retained_key = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            let selected = route.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Tracking scope canceled before worker creation"
                    );
                    let mut client = ServiceClient::spawn()?;
                    // Cancel is a protocol operation. Killing the worker from an
                    // IPC request guard would bypass adapter cleanup and ACK reporting.
                    Ok::<_, anyhow::Error>(setting.request(
                        &mut client,
                        &selected,
                        operation_id,
                        &guard.guard(),
                    ))
                })
                .await;
            let _ = owner.update(cx, |owner, cx| {
                if let Some(workspace) = weak.upgrade() {
                    let current = calibration_completion_current(
                        owner.devices.contains(&workspace)
                            && workspace.read(cx).identity(cx) == identity,
                        workspace
                            .read(cx)
                            .mouse_calibration_request_matches(&request, cx),
                        &route,
                        &owner.device_observations,
                        workspace.read(cx).device(cx),
                    );
                    let error = match result {
                        Ok((progress, transport_error)) => {
                            let mut errors: Vec<_> = transport_error.into_iter().collect();
                            if let Some(progress) = progress {
                                errors.extend(match &progress {
                                    HardwareProgress::Orca(progress) => tracking_error(progress),
                                    HardwareProgress::Philips(progress) => philips_error(progress),
                                });
                                if current {
                                    workspace.update(cx, |workspace, cx| match progress {
                                        HardwareProgress::Orca(progress) => workspace
                                            .record_mouse_tracking_report(&request, progress, cx),
                                        HardwareProgress::Philips(progress) => workspace
                                            .record_mouse_philips_report(&request, progress, cx),
                                    });
                                }
                            } else if errors.is_empty() {
                                errors.push(
                                    "Tracking completion contains no hardware outcome".into(),
                                );
                            }
                            (!errors.is_empty()).then(|| errors.join("; "))
                        }
                        Err(error) => Some(format!(
                            "Tracking hardware completion is unknown: {error:#}"
                        )),
                    };
                    if current {
                        if let Some(error) = &error {
                            eprintln!("[mouse-tracking] {error}");
                        }
                    }
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_calibration(
                            &request,
                            current.then_some(error).flatten(),
                            cx,
                        )
                    });
                }
                owner.mouse_tracking_tasks.remove(&key);
                if owner.close_requested && owner.save_task.is_none() {
                    owner.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        });
        self.mouse_tracking_tasks.insert(
            retained_key,
            TrackingTask {
                request: retained_request,
                _task: task,
            },
        );
    }
}

fn philips_error(progress: &razer_device::mouse_philips::Progress) -> Option<String> {
    if !progress.terminal {
        return Some("Philips calibration has not completed cleanup".into());
    }
    if let Some(error) = &progress.error {
        return Some(error.clone());
    }
    if progress.canceled {
        return Some("Philips calibration canceled; acknowledged commands remain recorded".into());
    }
    let Some(report) = &progress.report else {
        return Some("Philips terminal response contains no hardware report".into());
    };
    let mut errors: Vec<_> = report.initialization_error.iter().cloned().collect();
    match &report.outcome {
        Ok(result) if result.source_chain_acknowledged && !result.verified => {}
        Ok(_) => errors.push("Philips source setter chain was not acknowledged".into()),
        Err(error) => errors.push(format!(
            "{error}; {} setter acknowledgements retained",
            error.acknowledgements.len()
        )),
    }
    (!errors.is_empty()).then(|| errors.join("; "))
}

fn tracking_error(progress: &Progress) -> Option<String> {
    if !progress.terminal {
        return Some("Tracking operation has not completed cleanup".into());
    }
    if let Some(error) = &progress.error {
        return Some(error.clone());
    }
    if progress.canceled {
        return Some("Tracking operation canceled; acknowledged commands remain recorded".into());
    }
    let Some(report) = progress.report.as_ref() else {
        return Some("Tracking terminal response contains no hardware report".into());
    };
    if let Some(error) = &report.observation_error {
        return Some(error.clone());
    }
    match report.outcome.as_ref() {
        Some(Err(error)) => Some(format!(
            "{error}; {} setter acknowledgements retained",
            error.acknowledgements.len()
        )),
        Some(Ok(result)) if result.source_chain_acknowledged && !result.verified => None,
        _ => Some("Tracking source setter chain was not acknowledged".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn progress() -> Progress {
        Progress {
            operation_id: "simulated-operation".into(),
            target: razer_device::mouse_tracking::Target::WindowsContainer(
                razer_device::device_reads::DeviceReadTarget {
                    product_id: 170,
                    physical_product_id: 170,
                    peer_product_id: None,
                    device_container_id: "simulated-container".into(),
                    path: "simulated-path".into(),
                },
            ),
            requested: razer_device::mouse_tracking::TrackingSetting::Symmetric {
                tracking_distance: 2,
            },
            terminal: true,
            canceled: false,
            report: None,
            error: None,
        }
    }
    #[::core::prelude::v1::test]
    fn tracking_missing_report_and_nonterminal_acceptance_are_not_success() {
        let mut progress = progress();
        assert!(
            tracking_error(&progress)
                .unwrap()
                .contains("no hardware report")
        );
        progress.terminal = false;
        assert!(
            tracking_error(&progress)
                .unwrap()
                .contains("not completed cleanup")
        );
    }
    #[::core::prelude::v1::test]
    fn philips_missing_report_and_unjoined_acceptance_are_not_success() {
        let mut progress = razer_device::mouse_philips::Progress {
            operation_id: "philips-operation".into(),
            target: progress().target,
            requested: razer_device::mouse_philips::Setting::Disabled,
            terminal: false,
            canceled: false,
            report: None,
            error: None,
        };
        assert!(
            philips_error(&progress)
                .unwrap()
                .contains("not completed cleanup")
        );
        progress.terminal = true;
        assert!(
            philips_error(&progress)
                .unwrap()
                .contains("no hardware report")
        );
    }
    #[::core::prelude::v1::test]
    fn calibration_completion_survives_rescan_but_rejects_replaced_owner_and_route() {
        use razer_discovery::discovery::project_receiver_query;
        use serde_json::json;
        let project = |container: &str, status: u8| {
            let cap = razer_device::receiver_capabilities::capability(179).unwrap();
            project_receiver_query(179, container, &json!({
                "query":"receiver_wireless_status_v2","vendor_id":cap.vendor_id,"product_id":179,
                "claim_interface":cap.claim_interface,"report_bytes":cap.report_bytes,
                "path":"simulated-route","device_instance_id":"simulated-instance","device_container_id":container,
                "device_count":1,"devices":[{"product_id":183,"status":status}]
            })).unwrap()
        };
        let original = project("{7A1F7C04-2B84-4BA3-A66C-17023B63E99F}", 1);
        let repeat = project("{7a1f7c04-2b84-4ba3-a66c-17023b63e99f}", 1);
        let offline = project("{7A1F7C04-2B84-4BA3-A66C-17023B63E99F}", 0);
        let replacement = project("{7A1F7C04-2B84-4BA3-A66C-17023B63E990}", 1);
        let retained = &original.devices()[0];
        let device = retained.clone().into_device().unwrap();
        for _scan in 0..3 {
            assert!(calibration_completion_current(
                true,
                true,
                retained,
                repeat.devices(),
                &device
            ));
        }
        assert!(!calibration_completion_current(
            false,
            true,
            retained,
            repeat.devices(),
            &device
        ));
        // A canceled request scope stays retired even after the exact same
        // path returns; route equality cannot resurrect a prior connection.
        assert!(!calibration_completion_current(
            true,
            false,
            retained,
            repeat.devices(),
            &device
        ));
        for observations in [offline.devices(), replacement.devices(), &[]] {
            assert!(!calibration_completion_current(
                true,
                true,
                retained,
                observations,
                &device
            ));
        }
        let ambiguous = [retained.clone(), retained.clone()];
        assert!(!calibration_completion_current(
            true, true, retained, &ambiguous, &device
        ));
    }
}
