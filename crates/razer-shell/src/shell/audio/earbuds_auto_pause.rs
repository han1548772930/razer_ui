//! Existing workspace save queue precedes the earbuds' actual device operation.
use super::*;
use razer_device::earbuds_auto_pause::Operation;
use razer_pages::features::EarbudsAutoPauseRequested;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Default)]
pub(super) struct State {
    registered: bool,
    workers: BTreeMap<Uuid, (Arc<AtomicBool>, Task<()>)>,
    monitors: BTreeMap<Uuid, Arc<AtomicBool>>,
}
impl Drop for State {
    fn drop(&mut self) {
        for (guard, _) in self.workers.values() {
            guard.store(false, Ordering::Release);
        }
    }
}
pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: EarbudsAutoPauseRequested,
    source_name: Option<String>,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}
impl AppShell {
    pub(super) fn request_earbuds_auto_pause(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsAutoPauseRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if !page.read(cx).earbuds_request_current(&request) {
            return;
        }
        if self.close_requested || (request.needs_persistence() && self.storage_error.is_some()) {
            page.update(cx, |page, cx| {
                page.finish_earbuds_request(
                    &request,
                    Err("Local persistence is unavailable".into()),
                    cx,
                )
            });
            return;
        }
        if !request.needs_persistence() {
            self.submit_earbuds_auto_pause(workspace, request, cx);
            return;
        }
        if request.initializes_profile() {
            let handle = window.window_handle();
            let id = request.id();
            let guard = request.guard();
            let workspace = workspace.downgrade();
            let page = page.downgrade();
            let task = cx.spawn(async move |shell, cx| {
                let name = cx
                    .background_spawn(async { razer_platform::system::computer_name() })
                    .await;
                let _ = shell.update(cx, |shell, cx| {
                    if let (Some(workspace), Some(page)) = (workspace.upgrade(), page.upgrade()) {
                        if page.read(cx).earbuds_request_current(&request) {
                            match name {
                                Ok(name) => shell.queue_earbuds_save(
                                    workspace,
                                    request,
                                    Some(format!("{name}-Default")),
                                    handle,
                                    cx,
                                ),
                                Err(error) => page.update(cx, |page, cx| {
                                    page.finish_earbuds_request(
                                        &request,
                                        Err(format!("{error:#}")),
                                        cx,
                                    )
                                }),
                            }
                        } else {
                            page.update(cx, |page, cx| {
                                page.finish_earbuds_request(
                                    &request,
                                    Err("Initial earbud profile scope was canceled".into()),
                                    cx,
                                )
                            });
                        }
                    }
                    shell.earbuds_auto_pause.workers.remove(&id);
                });
            });
            self.earbuds_auto_pause.workers.insert(id, (guard, task));
            return;
        }
        self.queue_earbuds_save(workspace, request, None, window.window_handle(), cx);
    }

    fn queue_earbuds_save(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsAutoPauseRequested,
        source_name: Option<String>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let save = LocalSave {
            identity: workspace.read(cx).identity(cx),
            workspace,
            request,
            source_name,
        };
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|workspace| workspace.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window,
            file,
            Some(ProductSave::EarbudsAutoPause(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_earbuds_auto_pause_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::EarbudsAutoPause(local)) = &save.product else {
            return Ok(());
        };
        anyhow::ensure!(
            local
                .workspace
                .read(cx)
                .audio_mixer_page(cx)
                .is_some_and(|page| page.read(cx).earbuds_request_current(&local.request)),
            "Earbud profile or connection changed before save"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| {
                let mut saved = workspace.read(cx).saved_snapshot(cx);
                if workspace.read(cx).identity(cx) == local.identity {
                    if local.request.initializes_profile() {
                        razer_storage::earbuds_profiles::initialize_auto_pause_profile(
                            &mut saved,
                            local.request.profile_id(),
                            local.source_name.as_deref().ok_or_else(|| {
                                anyhow::anyhow!("Initial earbud profile has no source name")
                            })?,
                        )?;
                    }
                    if !local.request.initializes_profile() || !local.request.is_profile_restore() {
                        use razer_storage::earbuds_profiles as profiles;
                        let id = local.request.profile_id();
                        match local.request.operation() {
                            Operation::Equalizer { setting } => profiles::merge_equalizer(&mut saved, id, setting)?,
                            Operation::Anc { setting, .. } => {
                                profiles::merge_anc(&mut saved, id, setting)?
                            }
                            Operation::Write { enabled } => {
                                profiles::merge_auto_pause(&mut saved, id, enabled)?
                            }
                            Operation::DoNotDisturb { enabled } => {
                                profiles::merge_do_not_disturb(&mut saved, id, enabled)?
                            }
                            Operation::VoicePrompt { enabled } => {
                                profiles::merge_voice_prompt(&mut saved, id, enabled)?
                            }
                            Operation::Gesture { enabled } => {
                                profiles::merge_gesture(&mut saved, id, enabled)?
                            }
                            Operation::TapSpeed { speed } => {
                                profiles::merge_tap_speed(&mut saved, id, speed)?
                            }
                            _ => anyhow::bail!(
                                "Only an explicit earbud edit may merge an existing profile"
                            ),
                        }
                    }
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }

    pub(super) fn finish_earbuds_auto_pause_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = save.workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if !succeeded {
            page.update(cx, |page, cx| {
                page.finish_earbuds_request(
                    &save.request,
                    Err("Auto-pause profile save failed; no device setting was sent".into()),
                    cx,
                )
            });
            return;
        }
        if !page.update(cx, |page, _| page.mark_earbuds_persisted(&save.request)) {
            page.update(cx, |page, cx| {
                page.finish_earbuds_request(
                    &save.request,
                    Err("Earbud save belongs to an inactive scope".into()),
                    cx,
                )
            });
            return;
        }
        if save.request.is_hardware_notification() {
            page.update(cx, |page, cx| {
                page.finish_earbuds_notification_save(&save.request, None, cx)
            });
        } else {
            self.submit_earbuds_auto_pause(save.workspace, save.request, cx);
        }
    }

    fn submit_earbuds_auto_pause(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsAutoPauseRequested,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        let device = workspace.read(cx).device(cx);
        let route = self
            .device_observations
            .iter()
            .find(|route| route.matches(device))
            .cloned();
        let Some(route) = route else {
            page.update(cx, |page, cx| {
                page.finish_earbuds_request(
                    &request,
                    Err("No current earbud device observation".into()),
                    cx,
                )
            });
            return;
        };
        if !page.read(cx).earbuds_request_current(&request) {
            return;
        }
        if !self.earbuds_auto_pause.registered {
            self.earbuds_auto_pause.registered = true;
            self.register_exit_cleanup(|shell, _| {
                let workers = std::mem::take(&mut shell.earbuds_auto_pause.workers);
                shell.earbuds_auto_pause.monitors.clear();
                for (guard, _) in workers.values() {
                    guard.store(false, Ordering::Release);
                }
                async move {
                    for (_, (_, task)) in workers {
                        task.await;
                    }
                }
            });
        }
        let monitor = request.monitor_guard();
        if !self
            .earbuds_auto_pause
            .monitors
            .values()
            .any(|current| Arc::ptr_eq(current, &monitor))
        {
            self.start_earbuds_monitor(workspace, request, route, cx);
            return;
        }
        let id = request.id();
        let guard = request.guard();
        let active = guard.clone();
        let revision = workspace.read(cx).connection_revision();
        let identity = workspace.read(cx).identity(cx);
        let operation = request.operation();
        let task = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    razer_service::earbuds_auto_pause::device(route, operation, active)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                let current = shell.devices.contains(&workspace)
                    && workspace.read(cx).connection_revision() == revision
                    && workspace.read(cx).identity(cx) == identity;
                if !current {
                    request.guard().store(false, Ordering::Release);
                }
                let result = match (completion.result, completion.cleanup_failure) {
                    (Ok(report), None) => Ok(report),
                    (Err(error), None) | (Ok(_), Some(error)) => Err(error),
                    (Err(error), Some(cleanup)) => Err(format!("{error}; cleanup: {cleanup}")),
                };
                if let Err(error) = &result {
                    shell.status = format!("耳塞设置操作失败：{error}");
                }
                if let Ok(report) = &result {
                    if let Some(razer_device::earbuds_protocol40::ToggleObservation::ReadFailed(
                        error,
                    )) = &report.voice_prompt
                    {
                        shell.status = format!("语音提示状态读取失败：{error}");
                    }
                    if let razer_device::earbuds_protocol40::AutoPauseObservation::ReadFailed(
                        error,
                    ) = &report.observation
                    {
                        shell.status = format!("自动暂停状态读取失败：{error}");
                    }
                    if let Some(razer_device::earbuds_do_not_disturb::Observation::ReadFailed(
                        error,
                    )) = &report.do_not_disturb
                    {
                        shell.status = format!("请勿打扰状态读取失败：{error}");
                    }
                    if let razer_device::earbuds_protocol40::WriteOutcome::Failed(error) =
                        &report.write
                    {
                        shell.status = format!("耳塞设置未确认：{error}");
                    }
                }
                page.update(cx, |page, cx| {
                    page.finish_earbuds_request(&request, result, cx)
                });
                shell.earbuds_auto_pause.workers.remove(&id);
            });
        });
        self.earbuds_auto_pause.workers.insert(id, (guard, task));
    }

    fn start_earbuds_monitor(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsAutoPauseRequested,
        route: razer_discovery::discovery::ObservedDevice,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        let id = Uuid::new_v4();
        let guard = request.monitor_guard();
        let keep = guard.clone();
        let revision = workspace.read(cx).connection_revision();
        let identity = workspace.read(cx).identity(cx);
        let (send, receive) = async_channel::bounded(128);
        let workspace = workspace.downgrade();
        let page = page.downgrade();
        let task = cx.spawn(async move |shell, cx| {
            let signal = keep.clone();
            let worker = cx.background_spawn(async move {
                razer_service::earbuds_auto_pause::monitor(route, signal, |update| {
                    send.try_send(update)
                        .map_err(|_| anyhow::anyhow!("Earbud event consumer is absent or full"))
                })
            });
            while let Ok(update) = receive.recv().await {
                let accepted = shell
                    .update(cx, |shell, cx| {
                        let (Some(workspace), Some(page)) = (workspace.upgrade(), page.upgrade())
                        else {
                            keep.store(false, Ordering::Release);
                            return false;
                        };
                        if !shell.devices.contains(&workspace)
                            || workspace.read(cx).connection_revision() != revision
                            || workspace.read(cx).identity(cx) != identity
                            || !keep.load(Ordering::Acquire)
                        {
                            keep.store(false, Ordering::Release);
                            return false;
                        }
                        match update {
                            razer_service::earbuds_auto_pause::MonitorUpdate::Ready => shell
                                .submit_earbuds_auto_pause(workspace.clone(), request.clone(), cx),
                            razer_service::earbuds_auto_pause::MonitorUpdate::Notification(
                                event,
                            ) => {
                                if let razer_device::earbuds_events::Notification::Connection {
                                    status,
                                    ..
                                } = &event
                                {
                                    if let Err(error) =
                                        razer_device::earbuds_auto_pause::connection_is_online(
                                            *status,
                                        )
                                    {
                                        shell.status = format!("耳塞连接状态无效：{error:#}");
                                    }
                                }
                                page.update(cx, |page, cx| {
                                    page.observe_earbuds_notification(&keep, event, cx)
                                });
                            }
                        }
                        true
                    })
                    .unwrap_or(false);
                if !accepted {
                    keep.store(false, Ordering::Release);
                    break;
                }
            }
            drop(receive);
            let completion = worker.await;
            let _ = shell.update(cx, |shell, cx| {
                let error = completion.cleanup_failure.or_else(|| {
                    keep.load(Ordering::Acquire).then(|| {
                        completion
                            .result
                            .err()
                            .unwrap_or_else(|| "Earbud event source stopped unexpectedly".into())
                    })
                });
                if let Some(error) = &error {
                    shell.status = format!("耳塞状态观察失败：{error}");
                }
                if let Some(page) = page.upgrade() {
                    page.update(cx, |page, cx| {
                        page.finish_earbuds_request(
                            &request,
                            Err(error
                                .clone()
                                .unwrap_or_else(|| "Earbud event scope canceled".into())),
                            cx,
                        );
                        page.finish_earbuds_monitor(&keep, error, cx);
                    });
                }
                shell.earbuds_auto_pause.monitors.remove(&id);
                shell.earbuds_auto_pause.workers.remove(&id);
            });
        });
        self.earbuds_auto_pause.monitors.insert(id, guard.clone());
        self.earbuds_auto_pause.workers.insert(id, (guard, task));
    }
}
