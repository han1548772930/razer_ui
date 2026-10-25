//! Identity-scoped FIFO for current analog intents. Worker cancellation stops
//! subsequent setters and preserves the source's independent optimistic UI.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::keyboard_submission;
use razer_discovery::discovery;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[path = "keyboard_actuation_worker.rs"]
mod worker;
use worker::{Admission, Command, Reply};
pub(super) struct KeyboardActuationSave {
    workspace: Entity<ProductWorkspace>,
    request: razer_pages::features::keyboard_products::KeyboardActuationRequested,
    identity: String,
    profile: razer_model::model::Profile,
    intent: keyboard_submission::Intent,
}
impl KeyboardActuationSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }

    fn publishes_mapping(&self) -> bool {
        self.intent.publishes_mapping() || self.request.payload()["type"] == "ON_SET_KEYMAPPING"
    }
}

pub(super) fn should_publish_mapping(product: Option<&ProductSave>, identity: &str) -> bool {
    match product {
        Some(ProductSave::KeyboardActuation(save)) if save.identity == identity => {
            save.publishes_mapping()
        }
        Some(ProductSave::SourceProfileImport(_) | ProductSave::KeyboardLocalProfiles(_)) => false,
        _ => true,
    }
}

pub(super) struct Session {
    identity: String,
    profile: String,
    revision: u64,
    worker_id: thread::ThreadId,
    valid: Arc<AtomicBool>,
    admission: Admission<(
        razer_pages::features::keyboard_products::KeyboardActuationRequested,
        keyboard_submission::Intent,
    )>,
    _delivery: Option<Task<()>>,
}
pub(super) struct RetiredDelivery {
    valid: Arc<AtomicBool>,
    worker_id: thread::ThreadId,
    _delivery: Task<()>,
}
impl Session {
    fn owns(&self, valid: &Arc<AtomicBool>, worker_id: thread::ThreadId) -> bool {
        self.worker_id == worker_id && Arc::ptr_eq(&self.valid, valid)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
    }
}

impl AppShell {
    pub(super) fn install_keyboard_actuation_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let mut deliveries = std::mem::take(&mut this.keyboard_actuation_retired)
                .into_iter()
                .map(|retired| retired._delivery)
                .collect::<Vec<_>>();
            for (_, mut session) in std::mem::take(&mut this.keyboard_actuation_operations) {
                if let Some(delivery) = session._delivery.take() {
                    deliveries.push(delivery);
                }
            }
            let workers = std::mem::take(&mut this.keyboard_actuation_workers);
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    if worker.join().is_err() {
                        eprintln!("[keyboard submission cleanup] worker panicked");
                    }
                }
            });
            async move {
                cleanup.await;
                drop(deliveries);
            }
        });
    }

    /// Called after accepted saves have synchronously submitted their native
    /// follow-ups. Closing each sender preserves the actor's accepted FIFO.
    pub(super) fn finish_keyboard_actuation_exit(&mut self, cx: &mut Context<Self>) -> bool {
        for session in self.keyboard_actuation_operations.values_mut() {
            session.admission.stop_after_pending();
        }
        self.reap_keyboard_actuation_workers(cx);
        !self.keyboard_actuation_operations.is_empty()
            || !self.keyboard_actuation_workers.is_empty()
            || !self.keyboard_actuation_retired.is_empty()
    }

    fn reap_keyboard_actuation_workers(&mut self, cx: &mut Context<Self>) {
        let mut index = 0;
        while index < self.keyboard_actuation_workers.len() {
            if self.keyboard_actuation_workers[index].is_finished() {
                if self
                    .keyboard_actuation_workers
                    .swap_remove(index)
                    .join()
                    .is_err()
                {
                    self.keyboard_actuation_diagnostic(
                        "Keyboard submission worker panicked before completing cleanup".into(),
                        cx,
                    );
                }
            } else {
                index += 1;
            }
        }
    }

    fn keyboard_actuation_diagnostic(&mut self, error: String, cx: &mut Context<Self>) {
        eprintln!("[keyboard submission] {error}");
        self.status = format!("键盘设置提交：{error}");
        if self.close_requested {
            self.cancel_pending_exit(cx);
        }
        cx.notify();
    }

    pub(super) fn submit_keyboard_actuation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardActuationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = request.generation();
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .keyboard_actuation_request_current(generation, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(
                    generation,
                    Err("Keyboard owner changed before submission".into()),
                    cx,
                )
            });
            return;
        }
        let prepared = (|| -> anyhow::Result<_> {
            ensure!(
                !self.close_requested,
                "Keyboard submission canceled during shutdown"
            );
            ensure!(
                self.storage_error.is_none(),
                "Keyboard profile store is unreadable"
            );
            let device = workspace.read(cx).snapshot(cx);
            let mut profile = device
                .profiles
                .iter()
                .find(|profile| {
                    profile.id == device.active_profile || profile.guid == device.active_profile
                })
                .context("Active keyboard profile is absent")?
                .clone();
            let mut settings = request.profile().clone();
            settings["guid"] = serde_json::json!(profile.guid);
            let mut previous = profile
                .source_settings
                .clone()
                .context("Active keyboard source profile has not been read")?;
            previous["guid"] = serde_json::json!(profile.guid);
            let (settings, mut intent) = keyboard_submission::prepare_local(
                device.product_id,
                device.layout_id,
                request.payload(),
                &settings,
                &previous,
            )?;
            if let Some(macro_sync) = intent.macro_synchronization_mut() {
                let mut profiles = device
                    .profiles
                    .iter()
                    .map(|source| -> anyhow::Result<_> {
                        let mut settings = source.source_settings.clone().context(
                            "Keyboard macro synchronization source profile has not been read",
                        )?;
                        settings["guid"] = serde_json::json!(source.guid);
                        Ok(settings)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                if let Some(active) = profiles
                    .iter_mut()
                    .find(|source| source["guid"] == profile.guid)
                {
                    *active = settings.clone();
                }
                let macros = self
                    .macro_library
                    .read(cx)
                    .snapshot()
                    .source_macro_documents()
                    .map_err(anyhow::Error::msg)?;
                let candidates = (0..256)
                    .map(|_| {
                        let bytes = uuid::Uuid::new_v4();
                        u16::from_le_bytes(bytes.as_bytes()[..2].try_into().unwrap())
                    })
                    .collect();
                *macro_sync = Some(razer_device::onboard_macro_codec::prepare_synchronization(
                    device.product_id,
                    &profiles,
                    &macros,
                    candidates,
                )?);
            }
            profile.source_settings = Some(settings);
            Ok(KeyboardActuationSave {
                workspace: workspace.clone(),
                request: request.clone(),
                identity: workspace.read(cx).identity(cx),
                profile,
                intent,
            })
        })();
        let save = match prepared {
            Ok(save) => save,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_actuation(generation, Err(format!("{error:#}")), cx)
                });
                return;
            }
        };
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::KeyboardActuation(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_keyboard_actuation_save(&self, save: &mut PreparedSave, cx: &App) {
        let Some(ProductSave::KeyboardActuation(control)) = &save.product else {
            return;
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == control.identity {
                    if let Some(profile) = saved
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == control.profile.id)
                    {
                        profile.source_settings = control.profile.source_settings.clone();
                    } else {
                        saved.profiles.push(control.profile.clone());
                    }
                }
                saved
            })
            .collect();
    }

    pub(super) fn finish_keyboard_actuation_save(
        &mut self,
        save: KeyboardActuationSave,
        succeeded: bool,
        _window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let generation = save.request.generation();
        let current = self.devices.contains(&save.workspace)
            && save.workspace.read(cx).identity(cx) == save.identity
            && {
                let device = save.workspace.read(cx).device(cx);
                device.active_profile == save.profile.id
                    || device.active_profile == save.profile.guid
            }
            && save
                .workspace
                .read(cx)
                .keyboard_actuation_request_current(generation, cx);
        if !succeeded || !current {
            let error = if succeeded {
                "Keyboard submission scope ended after local persistence"
            } else {
                "Keyboard local profile persistence failed"
            };
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(generation, Err(error.into()), cx)
            });
            self.keyboard_actuation_diagnostic(error.into(), cx);
            return;
        }
        self.submit_keyboard_actuation_prepared(save.workspace, &save.request, save.intent, cx);
    }

    fn submit_keyboard_actuation_prepared(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardActuationRequested,
        intent: keyboard_submission::Intent,
        cx: &mut Context<Self>,
    ) {
        let generation = request.generation();
        if !workspace
            .read(cx)
            .keyboard_actuation_request_current(generation, cx)
        {
            self.keyboard_actuation_diagnostic(
                "Keyboard submission scope ended before native admission".into(),
                cx,
            );
            return;
        }
        let selected = (|| -> anyhow::Result<_> {
            let device = workspace.read(cx).device(cx);
            let profile = device
                .profiles
                .iter()
                .find(|profile| {
                    profile.id == device.active_profile || profile.guid == device.active_profile
                })
                .context("Active keyboard profile is absent")?;
            keyboard_submission::validate(device.product_id, &intent)?;
            let routes = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(device)
                        && route.peer_product_id().is_none()
                        && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
                })
                .collect::<Vec<_>>();
            let route = if !intent.requires_device() {
                None
            } else {
                let [route] = routes.as_slice() else {
                    anyhow::bail!("Keyboard submission requires a unique observed wired collection")
                };
                Some((**route).clone())
            };
            Ok((
                intent,
                route,
                workspace.read(cx).identity(cx),
                device.active_profile.clone(),
                profile.guid.clone(),
            ))
        })();
        let (intent, route, identity, profile, guid) = match selected {
            Ok(selected) => selected,
            Err(error) => {
                let error = format!("{error:#}");
                self.keyboard_actuation_diagnostic(error.clone(), cx);
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_actuation(generation, Err(error), cx)
                });
                return;
            }
        };
        if !intent.requires_device() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(generation, Ok(()), cx)
            });
            return;
        }
        let route = route.expect("Device intents require the observed route");
        let key = workspace.entity_id();
        if self
            .keyboard_actuation_operations
            .get(&key)
            .is_some_and(|session| {
                session.identity != identity
                    || session.profile != profile
                    || session.revision != self.discovery_revision
            })
        {
            if let Some(mut session) = self.keyboard_actuation_operations.remove(&key) {
                if let Some(delivery) = session._delivery.take() {
                    self.keyboard_actuation_retired.push(RetiredDelivery {
                        valid: session.valid.clone(),
                        worker_id: session.worker_id,
                        _delivery: delivery,
                    });
                }
                let reason = "Keyboard connection/profile scope ended";
                let (pending, waiting) = session.admission.retire();
                for generation in pending
                    .into_iter()
                    .chain(waiting.into_iter().map(|(request, _)| request.generation()))
                {
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_keyboard_actuation(generation, Err(reason.into()), cx)
                    });
                }
            }
        }
        if !self.keyboard_actuation_operations.contains_key(&key) {
            let (commands, incoming) = mpsc::sync_channel(128);
            let (sender, replies) = mpsc::channel();
            let valid = Arc::new(AtomicBool::new(true));
            let signal = valid.clone();
            let worker = match thread::Builder::new()
                .name("keyboard-actuation-owner".into())
                .spawn(move || worker::run(route, guid, signal, incoming, sender))
            {
                Ok(worker) => worker,
                Err(error) => {
                    self.keyboard_actuation_diagnostic(format!("{error:#}"), cx);
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_keyboard_actuation(
                            generation,
                            Err(format!("{error:#}")),
                            cx,
                        )
                    });
                    return;
                }
            };
            self.reap_keyboard_actuation_workers(cx);
            let worker_id = worker.thread().id();
            self.keyboard_actuation_workers.push(worker);
            let revision = self.discovery_revision;
            self.keyboard_actuation_operations.insert(
                key,
                Session {
                    identity: identity.clone(),
                    profile: profile.clone(),
                    revision,
                    worker_id,
                    valid: valid.clone(),
                    admission: Admission::new(commands),
                    _delivery: None,
                },
            );
            let target = workspace.clone();
            let delivery = cx.spawn(async move |this, cx| {
                let mut stopped = false;
                loop {
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                    let mut events = Vec::new();
                    let disconnected = loop {
                        match replies.try_recv() {
                            Ok(event) => events.push(event),
                            Err(mpsc::TryRecvError::Empty) => break false,
                            Err(mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    let mut stop = false;
                    if this.update(cx, |this, cx| {
                        let mut completed_deliveries = Vec::new();
                        let current = this.devices.contains(&target) && this.discovery_revision == revision
                            && target.read(cx).identity(cx) == identity
                            && target.read(cx).device(cx).active_profile == profile;
                        if !current { valid.store(false, Ordering::Release); }
                        for event in events {
                            match event {
                                Reply::Finished(generation, result) => {
                                    if let Some(session) = this.keyboard_actuation_operations.get_mut(&key).filter(|session| session.owns(&valid, worker_id)) {
                                        session.admission.finish(generation);
                                    }
                                    let result = if current && valid.load(Ordering::Acquire) { result } else { Err("Keyboard connection/profile scope ended".into()) };
                                    if let Err(error) = &result {
                                        this.keyboard_actuation_diagnostic(error.clone(), cx);
                                    }
                                    target.update(cx, |target, cx| target.finish_keyboard_actuation(generation, result, cx));
                                }
                                Reply::CleanupFailed(error) => this.keyboard_actuation_diagnostic(error, cx),
                                Reply::Stopped => {
                                    stopped = true;
                                }
                            }
                        }
                        if disconnected && !stopped {
                            stopped = true;
                            this.keyboard_actuation_diagnostic(
                                "Keyboard submission channel closed before its cleanup receipt".into(), cx);
                        }
                        let finished = this.keyboard_actuation_workers.iter()
                            .find(|worker| worker.thread().id() == worker_id)
                            .is_none_or(thread::JoinHandle::is_finished);
                        if stopped && finished {
                            stop = true;
                            if this.keyboard_actuation_operations.get(&key)
                                .is_some_and(|session| session.owns(&valid, worker_id))
                            {
                                let mut session = this.keyboard_actuation_operations.remove(&key)
                                    .expect("observed keyboard actor");
                                let (unfinished, waiting) = session.admission.retire();
                                let continuing = current && valid.load(Ordering::Acquire);
                                if let Some(delivery) = session._delivery.take() {
                                    completed_deliveries.push(delivery);
                                }
                                drop(session);
                                for generation in unfinished {
                                    let error = "Keyboard submission worker exited before completing this request";
                                    target.update(cx, |target, cx| target.finish_keyboard_actuation(generation, Err(error.into()), cx));
                                    this.keyboard_actuation_diagnostic(error.into(), cx);
                                }
                                for (request,intent) in waiting {
                                    if continuing {
                                        this.submit_keyboard_actuation_prepared(target.clone(), &request, intent, cx);
                                    } else {
                                        let error = "Keyboard connection/profile scope ended before successor admission";
                                        target.update(cx, |target, cx| target.finish_keyboard_actuation(request.generation(), Err(error.into()), cx));
                                        this.keyboard_actuation_diagnostic(error.into(), cx);
                                    }
                                }
                            }
                            if let Some(index) = this.keyboard_actuation_retired.iter().position(|retired|
                                retired.worker_id == worker_id && Arc::ptr_eq(&retired.valid, &valid))
                            {
                                completed_deliveries.push(this.keyboard_actuation_retired.swap_remove(index)._delivery);
                            }
                            this.reap_keyboard_actuation_workers(cx);
                        }
                        if this.close_requested && this.save_task.is_none() {
                            this.continue_save_queue(true, cx);
                        }
                        // No further await follows a completed actor. Keep its
                        // delivery owner through the final exit continuation.
                        drop(completed_deliveries);
                    }).is_err() { valid.store(false, Ordering::Release); break; }
                    if stop { break; }
                }
            });
            self.keyboard_actuation_operations
                .get_mut(&key)
                .unwrap()
                ._delivery = Some(delivery);
        }
        let queued = self
            .keyboard_actuation_operations
            .get_mut(&key)
            .expect("observed keyboard actor")
            .admission
            .submit(
                Command {
                    generation,
                    intent: intent.clone(),
                },
                (request.clone(), intent),
            );
        if let Err(error) = queued {
            self.keyboard_actuation_diagnostic(error.clone(), cx);
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(generation, Err(error), cx)
            });
        }
    }
}
