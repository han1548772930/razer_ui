//! Serial-owned receiver profiles use the workspace persistence queue.
use super::*;
use razer_discovery::receiver::ReceiverRoute;
use razer_pages::features::{ReceiverBrightnessRequested, ReceiverProfileSelectionRequested};
use razer_storage::receiver_reset::{ReceiverProfileRevision, ReceiverProfileSnapshot, ResetPlan};
use serde::Deserialize;

#[path = "receiver_profile_submission.rs"]
mod submission;

#[derive(Clone, Copy)]
pub(in crate::shell) enum Operation {
    Help(HelpResetRequest),
    Brightness(ReceiverBrightnessRequested),
    Selection(ReceiverProfileSelectionRequested),
}

struct Owner {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
    original: Device,
    expected: ReceiverProfileRevision,
    saved: ReceiverProfileRevision,
    route: ReceiverRoute,
    canceled: Arc<AtomicBool>,
    operation: Operation,
}

pub(in crate::shell) struct SelectionSession {
    generation: u64,
    pub(super) canceled: Arc<AtomicBool>,
    pub(super) worker: Option<JoinHandle<()>>,
    _delivery: Option<Task<()>>,
}
impl Drop for SelectionSession {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}

struct PreparedProfile {
    snapshot: ReceiverProfileSnapshot,
    path: String,
    refresh: Option<ResetPlan>,
}

pub(in crate::shell) struct LocalSave {
    owner: Owner,
    profile: PreparedProfile,
}

pub(in crate::shell) fn shutdown(client: &mut ServiceClient) -> anyhow::Result<Value> {
    if client.is_stopped() {
        client.finish_termination()?;
        Ok(Value::Null)
    } else {
        client.request(ServiceRequest::Shutdown)
    }
}

impl LocalSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.owner.identity, self.owner.workspace.downgrade())
    }
}

#[derive(Deserialize)]
struct SerialResponse {
    product_id: u32,
    path: String,
    device_container_id: String,
    serial_number: String,
}

#[derive(Deserialize)]
struct BrightnessResponse {
    product_id: u32,
    path: String,
    device_container_id: String,
    result: razer_device::receiver_brightness::Submission,
}

fn guard(canceled: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "Receiver profile operation canceled"
    );
    Ok(())
}

fn serial(
    client: &mut ServiceClient,
    device: &Device,
    path: &str,
    canceled: &AtomicBool,
) -> anyhow::Result<String> {
    guard(canceled)?;
    let response: SerialResponse =
        serde_json::from_value(client.request(ServiceRequest::ReceiverIdentityRead {
            path: path.into(),
            device_container_id: device.device_container_id.clone(),
        })?)?;
    guard(canceled)?;
    ensure!(
        response.product_id == device.product_id
            && response.path == path
            && response.device_container_id == device.device_container_id
            && !response.serial_number.is_empty(),
        "Receiver serial response belongs to a different owner"
    );
    Ok(response.serial_number)
}

fn prepare_inputs(
    device: &Device,
    operation: Operation,
    canceled: &Arc<AtomicBool>,
) -> anyhow::Result<PreparedProfile> {
    guard(canceled)?;
    let mut client = ServiceClient::spawn()?;
    client.set_cancellation_guard(canceled.clone());
    let result = (|| {
        let path = super::super::receiver_pairing::route::command_path(
            &mut client,
            device.product_id,
            &device.device_container_id,
        )?;
        let serial = serial(&mut client, device, &path, canceled)?;
        let computer = razer_platform::system::computer_name()?;
        let document = match ReceiverProfileSnapshot::in_device(device, &serial)? {
            Some(snapshot) => {
                ensure!(
                    matches!(operation, Operation::Selection(_))
                        || snapshot.active_profile() == device.active_profile,
                    "Receiver source active profile changed before preparation"
                );
                snapshot.document()?
            }
            None => {
                // Read the former per-receiver file only when this owner has
                // not yet entered the workspace. It is no longer a writer.
                let local = razer_storage::receiver_reset::local_path(
                    device.product_id,
                    &device.device_container_id,
                    &serial,
                )?;
                match razer_storage::receiver_reset::load(
                    &local,
                    device.product_id,
                    &device.device_container_id,
                    &serial,
                )? {
                    Some(document) => document,
                    None => {
                        ensure!(
                            !matches!(operation, Operation::Selection(request) if !request.is_initialization()),
                            "Receiver selection has no serial-backed source profile document"
                        );
                        razer_storage::receiver_reset::initial_document(
                            device.product_id,
                            device.edition_id,
                            &serial,
                            &computer,
                            &uuid::Uuid::new_v4().to_string(),
                        )?
                    }
                }
            }
        };
        let cache = razer_storage::receiver_reset::prepare_serial_cache(
            device.product_id,
            device.edition_id,
            &document,
            &serial,
            &computer,
            &uuid::Uuid::new_v4().to_string(),
        )?;
        let (document, refresh) = match operation {
            Operation::Help(_) => {
                let plan = ResetPlan::prepare(
                    device.product_id,
                    device.edition_id,
                    &cache,
                    &serial,
                    &device.device_container_id,
                    &computer,
                    &uuid::Uuid::new_v4().to_string(),
                )?;
                (plan.document().clone(), Some(plan))
            }
            Operation::Selection(request) => {
                let snapshot = ReceiverProfileSnapshot::parse(
                    device.product_id,
                    device.edition_id,
                    &serial,
                    &cache,
                )?;
                let snapshot = if request.is_initialization() {
                    snapshot
                } else {
                    snapshot.select_profile(&device.active_profile)?
                };
                (snapshot.document()?, None)
            }
            Operation::Brightness(request) => {
                let snapshot = ReceiverProfileSnapshot::parse(
                    device.product_id,
                    device.edition_id,
                    &serial,
                    &cache,
                )?;
                (
                    razer_storage::receiver_reset::brightness_document(
                        &cache,
                        snapshot.active_profile(),
                        request.enabled(),
                        request.value(),
                    )?,
                    None,
                )
            }
        };
        guard(canceled)?;
        let snapshot = ReceiverProfileSnapshot::parse(
            device.product_id,
            device.edition_id,
            &serial,
            &document,
        )?;
        snapshot.brightness()?;
        Ok(PreparedProfile {
            snapshot,
            path,
            refresh,
        })
    })();
    let cleanup = shutdown(&mut client);
    match (result, cleanup) {
        (Ok(profile), Ok(_)) => Ok(profile),
        (Err(error), Ok(_)) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("Receiver profile preparation cleanup failed"),
        (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
            "{error:#}; preparation cleanup failed: {cleanup:#}"
        )),
    }
}

impl AppShell {
    pub(in crate::shell) fn receiver_profile_exit_pending(&self) -> bool {
        !self.receiver_profile_selections.is_empty()
            || !self.receiver_reset.is_empty()
            || !self.receiver_brightness_page.is_empty()
            || self
                .receiver_reset_cleanup
                .iter()
                .chain(&self.receiver_brightness_cleanup)
                .any(|worker| !worker.is_finished())
    }

    pub(in crate::shell) fn wait_for_receiver_profile_exit(&mut self, cx: &mut Context<Self>) {
        if self.receiver_profile_exit_wait.is_some() {
            return;
        }
        self.receiver_profile_exit_wait = Some(cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let keep = shell.update(cx, |shell, cx| {
                    if shell.close_requested && shell.receiver_profile_exit_pending() {
                        return true;
                    }
                    shell.receiver_profile_exit_wait = None;
                    if shell.close_requested {
                        shell.continue_save_queue(true, cx);
                    }
                    false
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        }));
    }

    fn retire_receiver_profile_selection(&mut self, key: &str) {
        if let Some(mut session) = self.receiver_profile_selections.remove(key) {
            session.canceled.store(true, Ordering::Release);
            if let Some(worker) = session.worker.take() {
                self.receiver_reset_cleanup.push(worker);
            }
        }
    }

    fn receiver_profile_session_current(&self, owner: &Owner) -> bool {
        self.receiver_profile_session_matches(&owner.identity, owner.operation, &owner.canceled)
    }

    fn receiver_profile_session_matches(
        &self,
        identity: &str,
        operation: Operation,
        canceled: &Arc<AtomicBool>,
    ) -> bool {
        match operation {
            Operation::Selection(request) => self
                .receiver_profile_selections
                .get(identity)
                .is_some_and(|session| {
                    session.generation == request.generation()
                        && Arc::ptr_eq(&session.canceled, canceled)
                }),
            Operation::Help(request) => self
                .receiver_reset
                .get(identity)
                .is_some_and(|session| session.matches(request, canceled)),
            Operation::Brightness(request) => self
                .receiver_brightness_page
                .get(identity)
                .is_some_and(|session| session.matches(request.generation(), canceled)),
        }
    }

    fn receiver_profile_current(&self, owner: &Owner, cx: &App) -> bool {
        !owner.canceled.load(Ordering::Acquire)
            && self.receiver_profile_session_current(owner)
            && self.devices.contains(&owner.workspace)
            && owner.workspace.read(cx).identity(cx) == owner.identity
            && owner.workspace.read(cx).connection_revision() == owner.connection_revision
            && self.device_observations.iter().any(|observed| {
                observed.matches(owner.workspace.read(cx).device(cx))
                    && owner.route.matches(observed)
            })
            && ReceiverProfileRevision::capture(owner.workspace.read(cx).device(cx))
                .is_ok_and(|revision| revision == owner.expected)
            && match owner.operation {
                Operation::Selection(request) => {
                    owner.workspace.read(cx).receiver_profile_selection_matches(
                        request.generation(),
                        &owner.original.active_profile,
                        cx,
                    )
                }
                Operation::Help(request) => {
                    owner.workspace.read(cx).help_reset_matches(request, cx)
                }
                Operation::Brightness(request) => {
                    owner.workspace.read(cx).receiver_brightness_matches(
                        request.generation(),
                        Some(request.percent()),
                        true,
                        cx,
                    )
                }
            }
    }

    fn retire_receiver_profile(&mut self, owner: &Owner) {
        self.retire_receiver_profile_parts(&owner.identity, owner.operation, &owner.canceled);
    }

    fn retire_receiver_profile_parts(
        &mut self,
        identity: &str,
        operation: Operation,
        canceled: &Arc<AtomicBool>,
    ) {
        if !self.receiver_profile_session_matches(identity, operation, canceled) {
            return;
        }
        match operation {
            Operation::Selection(_) => self.retire_receiver_profile_selection(identity),
            Operation::Help(_) => self.retire_help_reset(identity),
            Operation::Brightness(_) => self.retire_receiver_brightness(identity),
        }
    }

    fn finish_receiver_profile_error(
        &mut self,
        owner: &Owner,
        error: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.receiver_profile_session_current(owner) {
            return;
        }
        self.retire_receiver_profile(owner);
        self.status = error.clone();
        owner
            .workspace
            .update(cx, |view, cx| match owner.operation {
                Operation::Selection(request) => {
                    view.finish_receiver_profile_selection(request, Some(error), window, cx)
                }
                Operation::Help(request) => {
                    view.finish_help_reset(request, None, Some(error), window, cx)
                }
                Operation::Brightness(request) => view.finish_receiver_brightness(
                    request.generation(),
                    Some(request.percent()),
                    None,
                    None,
                    None,
                    Some(error),
                    false,
                    window,
                    cx,
                ),
            });
        cx.notify();
    }

    pub(in crate::shell) fn request_receiver_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prepared = (|| -> anyhow::Result<Owner> {
            ensure!(
                self.storage_error.is_none() && !self.close_requested,
                "Receiver workspace storage is unavailable"
            );
            let original = workspace.read(cx).snapshot(cx);
            ensure!(
                matches!(original.product_id, 164 | 241)
                    && original.real_product_id == original.product_id,
                "Receiver source profile product differs"
            );
            let route = device_discovery::receiver_route_for_owner(
                &self.device_observations,
                &original,
                &original.device_container_id,
                original.product_id,
            )?;
            ensure!(
                uuid::Uuid::parse_str(&original.device_container_id).is_ok_and(|id| !id.is_nil()),
                "Receiver profile requires the current physical container"
            );
            Ok(Owner {
                identity: workspace.read(cx).identity(cx),
                connection_revision: workspace.read(cx).connection_revision(),
                expected: ReceiverProfileRevision::capture(&original)?,
                saved: ReceiverProfileRevision::capture(&workspace.read(cx).saved_snapshot(cx))?,
                original,
                workspace: workspace.clone(),
                operation,
                route,
                canceled: Arc::new(AtomicBool::new(false)),
            })
        })();
        let owner = match prepared {
            Ok(owner) => owner,
            Err(error) => {
                workspace.update(cx, |view, cx| match operation {
                    Operation::Selection(request) => view.finish_receiver_profile_selection(
                        request,
                        Some(format!("无法准备接收器配置：{error:#}")),
                        window,
                        cx,
                    ),
                    Operation::Help(request) => view.finish_help_reset(
                        request,
                        None,
                        Some(format!("无法准备接收器配置：{error:#}")),
                        window,
                        cx,
                    ),
                    Operation::Brightness(request) => view.finish_receiver_brightness(
                        request.generation(),
                        Some(request.percent()),
                        None,
                        None,
                        None,
                        Some(format!("{error:#}")),
                        false,
                        window,
                        cx,
                    ),
                });
                return;
            }
        };
        // Both operations mutate one serial document. Replacing either one
        // cancels its predecessor before admitting the next workspace write.
        self.retire_help_reset(&owner.identity);
        self.retire_receiver_brightness(&owner.identity);
        self.retire_receiver_profile_selection(&owner.identity);
        match operation {
            Operation::Selection(request) => {
                self.receiver_profile_selections.insert(
                    owner.identity.clone(),
                    SelectionSession {
                        generation: request.generation(),
                        canceled: owner.canceled.clone(),
                        worker: None,
                        _delivery: None,
                    },
                );
            }
            Operation::Help(request) => {
                self.receiver_reset.insert(
                    owner.identity.clone(),
                    Session {
                        request,
                        canceled: owner.canceled.clone(),
                        worker: None,
                        _delivery: None,
                    },
                );
            }
            Operation::Brightness(request) => self.begin_receiver_brightness_profile(
                owner.identity.clone(),
                request.generation(),
                owner.canceled.clone(),
            ),
        }
        // Entity handles stay on the UI thread; only immutable preparation
        // inputs and the cancellation signal cross to the worker.
        let original = owner.original.clone();
        let canceled = owner.canceled.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("receiver-profile-prepare".into())
            .spawn(move || {
                let result = prepare_inputs(&original, operation, &canceled)
                    .map_err(|error| format!("{error:#}"));
                let _ = sender.send(result);
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                self.finish_receiver_profile_error(
                    &owner,
                    format!("无法启动接收器配置任务：{error}"),
                    window,
                    cx,
                );
                return;
            }
        };
        let retained = owner.canceled.clone();
        let key = owner.identity.clone();
        let request = owner.operation;
        let delivery = cx.spawn_in(window, async move |shell, cx| {
            let signal = owner.canceled.clone();
            let identity = owner.identity.clone();
            let mut retained_owner = Some(owner);
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let incoming = receiver.try_recv();
                let terminal = !matches!(incoming, Err(mpsc::TryRecvError::Empty));
                let current = shell.update_in(cx, |shell, window, cx| {
                    let Some(owner) = retained_owner.as_ref() else {
                        return false;
                    };
                    if !shell.receiver_profile_current(owner, cx) {
                        shell.finish_receiver_profile_error(
                            owner,
                            "接收器配置会话已变化；未提交旧配置。".into(),
                            window,
                            cx,
                        );
                        return false;
                    }
                    match incoming {
                        Ok(Ok(profile)) => {
                            let file = shell.workspace_file(
                                shell
                                    .devices
                                    .iter()
                                    .map(|workspace| workspace.read(cx).saved_snapshot(cx))
                                    .collect(),
                                cx,
                            );
                            let save = shell.prepare_save(
                                window.window_handle(),
                                file,
                                Some(ProductSave::ReceiverProfile(LocalSave {
                                    owner: retained_owner
                                        .take()
                                        .expect("receiver preparation owner"),
                                    profile,
                                })),
                                cx,
                            );
                            shell.pending_saves.push_back(save);
                            shell.start_pending_save(cx);
                            false
                        }
                        Ok(Err(error)) => {
                            shell.finish_receiver_profile_error(
                                owner,
                                format!("接收器配置准备失败：{error}"),
                                window,
                                cx,
                            );
                            false
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            shell.finish_receiver_profile_error(
                                owner,
                                "接收器配置准备没有返回结果。".into(),
                                window,
                                cx,
                            );
                            false
                        }
                        Err(mpsc::TryRecvError::Empty) => true,
                    }
                });
                if current.is_err() {
                    signal.store(true, Ordering::Release);
                    let _ = shell.update(cx, |shell, _| {
                        shell.retire_receiver_profile_parts(&identity, request, &signal);
                    });
                }
                if terminal || !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.retain_receiver_profile_worker_parts(&key, request, retained, worker, delivery);
    }

    fn retain_receiver_profile_worker_parts(
        &mut self,
        identity: &str,
        operation: Operation,
        canceled: Arc<AtomicBool>,
        worker: JoinHandle<()>,
        delivery: Task<()>,
    ) {
        match operation {
            Operation::Selection(request) => {
                if let Some(session) =
                    self.receiver_profile_selections
                        .get_mut(identity)
                        .filter(|session| {
                            session.generation == request.generation()
                                && Arc::ptr_eq(&session.canceled, &canceled)
                        })
                {
                    if let Some(previous) = session.worker.replace(worker) {
                        self.receiver_reset_cleanup.push(previous);
                    }
                    session._delivery = Some(delivery);
                } else {
                    canceled.store(true, Ordering::Release);
                    self.receiver_reset_cleanup.push(worker);
                }
            }
            Operation::Help(request) => {
                if let Some(session) = self
                    .receiver_reset
                    .get_mut(identity)
                    .filter(|session| session.matches(request, &canceled))
                {
                    if let Some(previous) = session.worker.replace(worker) {
                        self.receiver_reset_cleanup.push(previous);
                    }
                    session._delivery = Some(delivery);
                } else {
                    canceled.store(true, Ordering::Release);
                    self.receiver_reset_cleanup.push(worker);
                }
            }
            Operation::Brightness(_) => {
                self.retain_receiver_brightness_worker(identity, worker, delivery)
            }
        }
    }

    pub(in crate::shell) fn rebase_receiver_profile_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::ReceiverProfile(profile)) = &mut save.product else {
            return Ok(());
        };
        let owner = &profile.owner;
        ensure!(
            self.receiver_profile_current(owner, cx),
            "Receiver profile owner expired before persistence"
        );
        let target = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == owner.identity)
            .context("Receiver save owner is absent")?;
        ensure!(
            ReceiverProfileRevision::capture(target)? == owner.saved,
            "Receiver saved profiles changed while the operation was queued"
        );
        *target = profile.profile.snapshot.project(target)?;
        Ok(())
    }

    pub(in crate::shell) fn finish_receiver_profile_save(
        &mut self,
        mut save: LocalSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.entity().downgrade();
        cx.defer(move |cx| {
            let signal = save.owner.canceled.clone();
            let key = save.owner.identity.clone();
            let operation = save.owner.operation;
            let result = window.update(cx, |_, window, cx| {
                weak.update(cx, |shell, cx| {
                    if !shell.receiver_profile_current(&save.owner, cx) || !succeeded {
                        shell.finish_receiver_profile_error(
                            &save.owner,
                            "接收器本地保存未完成或会话已失效；未提交设备。".into(),
                            window,
                            cx,
                        );
                        return;
                    }
                    let published =
                        save.owner
                            .workspace
                            .update(cx, |view, cx| match save.owner.operation {
                                Operation::Selection(request) => view
                                    .publish_receiver_selection_profile(
                                        request,
                                        save.profile.snapshot.clone(),
                                        window,
                                        cx,
                                    ),
                                Operation::Help(request) => view.publish_receiver_help_profile(
                                    request,
                                    save.profile.snapshot.clone(),
                                    window,
                                    cx,
                                ),
                                Operation::Brightness(request) => view
                                    .publish_receiver_brightness_snapshot(
                                        request.generation(),
                                        Some(request.percent()),
                                        save.profile.snapshot.clone(),
                                        window,
                                        cx,
                                    ),
                            });
                    if !published {
                        shell.finish_receiver_profile_error(
                            &save.owner,
                            "接收器配置已保存；页面会话已变化，未提交设备。".into(),
                            window,
                            cx,
                        );
                        return;
                    }
                    save.owner.expected =
                        ReceiverProfileRevision::capture(save.owner.workspace.read(cx).device(cx))
                            .expect("validated receiver publication");
                    shell.start_receiver_profile_submission(save, window, cx);
                })
            });
            if !matches!(result, Ok(Ok(()))) {
                signal.store(true, Ordering::Release);
                let _ = weak.update(cx, |shell, _| {
                    shell.retire_receiver_profile_parts(&key, operation, &signal);
                });
            }
        });
    }
}
