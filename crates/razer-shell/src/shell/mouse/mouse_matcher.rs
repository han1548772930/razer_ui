//! Matcher composition. The retained service owns capture; the common workspace
//! queue owns durable publication and rejects expired profile/connection owners.
use super::*;
use anyhow::{Context as _, ensure};
use razer_pages::features::{
    mouse_dpi_matcher::{MatcherAction, MatcherCommand, MatcherObservation},
    mouse_polling::MousePollingScope,
    mouse_products::{MouseMatcherRequest, MouseMatcherScope},
};
use razer_service::dpi_matcher::worker::{
    Command, Completion, ConnectedDevice, Handle, Options, Update,
};
use serde_json::Value;
use std::sync::mpsc::Sender;

#[derive(Default)]
pub(super) struct State {
    sessions: BTreeMap<String, Session>,
    cleanup: Vec<Cleanup>,
}
struct Cleanup {
    id: uuid::Uuid,
    identity: String,
    completion: Completion,
    reported: bool,
    task: Task<()>,
}

impl State {
    pub(super) fn pending(&self) -> bool {
        !self.sessions.is_empty() || self.cleanup.iter().any(|cleanup| !cleanup.reported)
    }
}
struct Session {
    owner: Owner,
    closing: bool,
    handle: Option<Handle>,
    _delivery: Task<()>,
}
#[derive(Clone)]
struct Owner {
    id: uuid::Uuid,
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    scope: MouseMatcherScope,
    domain_scope: MousePollingScope,
    connection_revision: u64,
    profile_id: String,
    profile_guid: String,
    route: razer_discovery::discovery::ObservedDevice,
}

impl Owner {
    fn current(&self, shell: &AppShell, draining: bool, cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        let workspace_state = workspace.read(cx);
        let device = workspace_state.device(cx);
        shell.devices.contains(&workspace)
            && workspace_state.identity(cx) == self.identity
            && workspace_state.connection_revision() == self.connection_revision
            && workspace_state.mouse_polling_scope(cx) == Some(self.domain_scope)
            && device.active_profile == self.profile_id
            && device
                .profiles
                .iter()
                .any(|profile| profile.id == self.profile_id && profile.guid == self.profile_guid)
            && workspace_state.retains_mouse_matcher_scope(&self.scope, cx)
            && (draining
                || self.scope.is_valid()
                    && workspace_state.mouse_matcher_scope(cx).as_ref() == Some(&self.scope))
            && super::device_discovery::retained_connection(
                std::slice::from_ref(&self.route),
                &shell.device_observations,
                device,
            )
    }
}
pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    owner: Owner,
    mutation: razer_model::mouse_matcher::Mutation,
    reply: Option<Sender<Result<(), String>>>,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.owner.identity, self.workspace.downgrade())
    }
}
impl Drop for LocalSave {
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.send(Err(
                "Matcher save was canceled before acknowledgement".into()
            ));
        }
    }
}

impl AppShell {
    pub(super) fn install_mouse_matcher_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, cx| {
            let sessions = std::mem::take(&mut shell.mouse_matcher.sessions);
            let cleanup = std::mem::take(&mut shell.mouse_matcher.cleanup);
            for session in sessions.values() {
                if let Some(handle) = &session.handle {
                    handle.cancel();
                }
            }
            let background = cx.background_spawn(async move {
                for mut session in sessions.into_values() {
                    if let Some(handle) = session.handle.take() {
                        if let Err(error) = handle.finish() {
                            eprintln!("[dpi-matcher cleanup] {error:#}");
                        }
                    }
                }
            });
            async move {
                background.await;
                for cleanup in cleanup {
                    cleanup.task.await;
                }
            }
        });
    }

    fn retire_mouse_matcher(&mut self, identity: &str, cx: &mut Context<Self>) {
        self.mouse_matcher.cleanup.retain(|cleanup| {
            !cleanup.reported
                || !cleanup.completion.is_released()
                || cleanup.completion.cleanup_error().is_some()
        });
        if let Some(mut session) = self.mouse_matcher.sessions.remove(identity) {
            if let Some(handle) = session.handle.take() {
                handle.cancel();
                let completion = handle.completion();
                let received = completion.clone();
                let id = session.owner.id;
                let task = cx.spawn(async move |shell, cx| {
                    let result = cx.background_spawn(async move { handle.finish() }).await;
                    let _ = shell.update(cx, |shell, cx| {
                        if let Some(cleanup) = shell
                            .mouse_matcher
                            .cleanup
                            .iter_mut()
                            .find(|cleanup| cleanup.id == id)
                        {
                            cleanup.reported = true;
                        }
                        if let Err(error) = result {
                            eprintln!("[dpi-matcher completion] {error:#}");
                        }
                        if let Some(error) = received.cleanup_error() {
                            shell.status = format!("DPI Matcher 清理尚未确认：{error}");
                            shell.cancel_pending_exit(cx);
                        }
                        if shell.close_requested && shell.save_task.is_none() {
                            shell.continue_save_queue(true, cx);
                        }
                        cx.notify();
                    });
                });
                self.mouse_matcher.cleanup.push(Cleanup {
                    id,
                    identity: identity.into(),
                    completion,
                    reported: false,
                    task,
                });
            }
        }
    }

    pub(super) fn prepare_mouse_matcher_exit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        for cleanup in &self.mouse_matcher.cleanup {
            if cleanup.completion.is_finished()
                && (!cleanup.completion.is_released()
                    || cleanup.completion.cleanup_error().is_some())
            {
                return Err(cleanup
                    .completion
                    .cleanup_error()
                    .unwrap_or_else(|| "DPI Matcher 传输资源尚未确认释放".into()));
            }
        }
        for session in self.mouse_matcher.sessions.values_mut() {
            session.closing = true;
            if let Some(handle) = &mut session.handle {
                handle.begin_exit();
            }
        }
        cx.notify();
        Ok(())
    }

    pub(super) fn dispatch_mouse_matcher(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &MouseMatcherRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !request.scope().is_valid()
            || workspace.read(cx).mouse_matcher_scope(cx).as_ref() != Some(request.scope())
        {
            return;
        }
        // AS emits its local X-stage projection immediately before Select. The
        // Select command owns that same native stage submission and local save.
        if matches!(request.action(), MatcherAction::SetStageValueX { .. }) {
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let revision = workspace.read(cx).connection_revision();
        let retained = !matches!(
            request.action(),
            MatcherAction::Command(MatcherCommand::Start)
        ) && self
            .mouse_matcher
            .sessions
            .get(&identity)
            .is_some_and(|session| {
                !session.closing
                    && session.owner.workspace.upgrade().as_ref() == Some(&workspace)
                    && session.owner.scope == *request.scope()
                    && session.owner.connection_revision == revision
            });
        if !retained {
            self.retire_mouse_matcher(&identity, cx);
            let routes = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(workspace.read(cx).device(cx))
                        && razer_discovery::direct::mouse_current_dpi::available(route)
                })
                .cloned()
                .collect::<Vec<_>>();
            let [route] = routes.as_slice() else {
                if matches!(
                    request.action(),
                    MatcherAction::Command(MatcherCommand::GetState)
                ) {
                    let state = razer_service::dpi_matcher::Controller::default().response();
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_mouse_matcher(
                            request.scope(),
                            MatcherObservation::Calibration(state["payload"].clone()),
                            window,
                            cx,
                        )
                    });
                } else {
                    workspace.update(cx, |workspace, cx| {
                        workspace.reject_mouse_matcher(
                            request.scope(),
                            "DPI Matcher requires one current mouse input/DPI route".into(),
                            cx,
                        )
                    });
                }
                return;
            };
            let connected = self
                .devices
                .iter()
                .filter_map(|workspace| {
                    let device = workspace.read(cx).device(cx);
                    self.device_observations
                        .iter()
                        .any(|route| route.matches(device))
                        .then(|| ConnectedDevice::from_device(device))
                })
                .collect();
            let options = match Options::new(
                route.clone(),
                connected,
                self.local_pairing_records.as_ref(),
            ) {
                Ok(options) => options,
                Err(error) => {
                    workspace.update(cx, |workspace, cx| {
                        workspace.reject_mouse_matcher(request.scope(), format!("{error:#}"), cx)
                    });
                    return;
                }
            };
            let predecessors = self
                .mouse_matcher
                .cleanup
                .iter()
                .filter(|cleanup| cleanup.identity == identity)
                .map(|cleanup| cleanup.completion.clone())
                .collect();
            let device = workspace.read(cx).device(cx);
            let Some(profile) = device
                .profiles
                .iter()
                .find(|profile| profile.id == device.active_profile)
            else {
                return;
            };
            let Some(domain_scope) = workspace.read(cx).mouse_polling_scope(cx) else {
                return;
            };
            let owner = Owner {
                id: uuid::Uuid::new_v4(),
                workspace: workspace.downgrade(),
                identity: identity.clone(),
                scope: request.scope().clone(),
                domain_scope,
                connection_revision: revision,
                profile_id: profile.id.clone(),
                profile_guid: profile.guid.clone(),
                route: route.clone(),
            };
            let (handle, updates) = Handle::start(options, predecessors);
            let delivery_owner = owner.clone();
            let window = window.window_handle();
            let delivery = cx.spawn(async move |shell, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    let current = shell.update(cx, |shell, cx| {
                        let current = shell
                            .mouse_matcher
                            .sessions
                            .get(&delivery_owner.identity)
                            .is_some_and(|session| {
                                session.owner.id == delivery_owner.id
                                    && (!shell.close_requested || session.closing)
                                    && delivery_owner.current(shell, session.closing, cx)
                            });
                        if !current {
                            return false;
                        }
                        for _ in 0..64 {
                            match updates.try_next() {
                                Ok(Some(update)) => {
                                    shell.receive_mouse_matcher(&delivery_owner, update, window, cx)
                                }
                                Ok(None) => return true,
                                Err(_) => return false,
                            }
                        }
                        true
                    });
                    if !matches!(current, Ok(true)) {
                        break;
                    }
                }
                let _ = shell.update(cx, |shell, cx| {
                    if shell
                        .mouse_matcher
                        .sessions
                        .get(&delivery_owner.identity)
                        .is_some_and(|session| session.owner.id == delivery_owner.id)
                    {
                        shell.retire_mouse_matcher(&delivery_owner.identity, cx);
                    }
                });
            });
            self.mouse_matcher.sessions.insert(
                identity.clone(),
                Session {
                    owner,
                    closing: false,
                    handle: Some(handle),
                    _delivery: delivery,
                },
            );
        }
        let command = match request.action() {
            MatcherAction::SelectProfile(guid) => Command::Select(guid.clone()),
            MatcherAction::RenameProfile(_) => {
                Command::UpdateProfiles(request.profile_payload().clone())
            }
            MatcherAction::Command(command) => match command {
                MatcherCommand::GetState => Command::GetState,
                MatcherCommand::Start => Command::Start,
                MatcherCommand::Confirm => Command::Confirm,
                MatcherCommand::Cancel => Command::Cancel,
                MatcherCommand::Reset => Command::Reset,
                MatcherCommand::DeleteProfile(guid) => Command::DeleteProfile(guid.clone()),
                MatcherCommand::DeleteAllProfiles => Command::DeleteAllProfiles,
            },
            MatcherAction::SetStageValueX { .. } => unreachable!("handled local stage projection"),
        };
        if let Some(session) = self.mouse_matcher.sessions.get(&identity) {
            if let Some(handle) = &session.handle {
                if let Err(error) = handle.send(command) {
                    workspace.update(cx, |workspace, cx| {
                        workspace.reject_mouse_matcher(request.scope(), format!("{error:#}"), cx)
                    });
                }
            }
        }
    }

    fn receive_mouse_matcher(
        &mut self,
        owner: &Owner,
        update: Update,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .mouse_matcher
            .sessions
            .get(&owner.identity)
            .is_some_and(|session| {
                session.owner.id == owner.id && owner.current(self, session.closing, cx)
            });
        let Some(workspace) = owner.workspace.upgrade().filter(|_| current) else {
            match update {
                Update::ReadSource(reply) => {
                    let _ = reply.send(Err("Matcher source owner changed".into()));
                }
                Update::Persist { reply, .. } => {
                    let _ = reply.send(Err("Matcher save owner changed".into()));
                }
                _ => {}
            }
            return;
        };
        let scope = &owner.scope;
        match update {
            Update::ReadSource(reply) => {
                let result = razer_model::mouse_matcher::source_record(
                    &workspace.read(cx).saved_snapshot(cx),
                );
                let _ = reply.send(result.map_err(|error| format!("{error:#}")));
            }
            Update::Persist { mutation, reply } => {
                if let Some(error) = &self.storage_error {
                    let _ = reply.send(Err(error.clone()));
                    return;
                }
                let save = LocalSave {
                    workspace,
                    owner: owner.clone(),
                    mutation,
                    reply: Some(reply),
                };
                let file = self.workspace_file(
                    self.devices
                        .iter()
                        .map(|owner| owner.read(cx).saved_snapshot(cx))
                        .collect(),
                    cx,
                );
                self.pending_saves.push_back(self.prepare_save(
                    window,
                    file,
                    Some(ProductSave::MouseMatcher(save)),
                    cx,
                ));
                self.start_pending_save(cx);
            }
            Update::StageDraft(stages) => {
                #[derive(serde::Deserialize)]
                struct StageDraft {
                    stages: Vec<Value>,
                    active: usize,
                }
                match serde_json::from_value::<StageDraft>(stages) {
                    Ok(draft) => self.publish_mouse_matcher_observation(
                        owner,
                        MatcherObservation::Stages {
                            stages: draft.stages,
                            active: draft.active,
                        },
                        window,
                        cx,
                    ),
                    Err(error) => workspace.update(cx, |workspace, cx| {
                        workspace.reject_mouse_matcher(
                            scope,
                            format!("Matcher stage response: {error}"),
                            cx,
                        )
                    }),
                }
            }
            Update::Response(response) => {
                #[derive(serde::Deserialize)]
                #[serde(tag = "type", content = "payload")]
                enum Response {
                    #[serde(rename = "MW_SET_DPI_MATCHER_STATE")]
                    Calibration(Value),
                    #[serde(rename = "MW_SET_SENSITIVITY_PROFILE_TO_UI")]
                    Profiles(Profiles),
                }
                #[derive(serde::Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Profiles {
                    sensitivity_matcher: Library,
                    selected_sensitivity_matcher: Option<String>,
                }
                #[derive(serde::Deserialize)]
                struct Library {
                    profiles: Vec<Value>,
                }
                let observation = match serde_json::from_value::<Response>(response) {
                    Ok(Response::Calibration(value)) => MatcherObservation::Calibration(value),
                    Ok(Response::Profiles(value)) => MatcherObservation::Profiles {
                        profiles: value.sensitivity_matcher.profiles,
                        selected: value.selected_sensitivity_matcher.unwrap_or_default(),
                    },
                    Err(error) => {
                        workspace.update(cx, |workspace, cx| {
                            workspace.reject_mouse_matcher(scope, format!("{error:#}"), cx)
                        });
                        return;
                    }
                };
                self.publish_mouse_matcher_observation(owner, observation, window, cx);
            }
            Update::Failure(error) => {
                eprintln!("[dpi-matcher] {error}");
                workspace.update(cx, |workspace, cx| {
                    workspace.reject_mouse_matcher(scope, error.clone(), cx)
                });
                if self.close_requested {
                    self.status = format!("DPI Matcher 操作未完成：{error}");
                    self.cancel_pending_exit(cx);
                }
            }
        }
    }

    fn publish_mouse_matcher_observation(
        &mut self,
        owner: &Owner,
        observation: MatcherObservation,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !owner.current(self, false, cx) {
            return;
        }
        let Some(workspace) = owner.workspace.upgrade() else {
            return;
        };
        let scope = owner.scope.clone();
        let _ = window.update(cx, |_, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_mouse_matcher(&scope, observation, window, cx)
            });
        });
    }

    pub(super) fn rebase_mouse_matcher_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MouseMatcher(matcher)) = &save.product else {
            return Ok(());
        };
        ensure!(
            self.mouse_matcher_save_current(matcher, cx),
            "Matcher profile owner changed before persistence"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).saved_snapshot(cx))
            .collect();
        let target = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == matcher.owner.identity)
            .context("Matcher workspace was removed before persistence")?;
        matcher.mutation.apply(target)
    }

    pub(super) fn finish_mouse_matcher_save(
        &mut self,
        mut save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let mut current = succeeded && self.mouse_matcher_save_current(&save, cx);
        if current {
            if let Err(error) = save.workspace.update(cx, |workspace, cx| {
                workspace.accept_mouse_matcher_saved_profiles(&save.owner.scope, cx)
            }) {
                eprintln!("[dpi-matcher saved profiles] {error:#}");
                current = false;
            }
        }
        if let Some(reply) = save.reply.take() {
            let _ = reply.send(if current {
                Ok(())
            } else {
                Err("Matcher save failed or its profile owner changed".into())
            });
        }
        if !current {
            self.status = "DPI Matcher 本地保存失败或其设备、配置会话已变化。".into();
            self.cancel_pending_exit(cx);
        }
    }

    fn mouse_matcher_save_current(&self, save: &LocalSave, cx: &App) -> bool {
        self.mouse_matcher
            .sessions
            .get(&save.owner.identity)
            .is_some_and(|session| {
                session.owner.id == save.owner.id
                    && session.owner.scope == save.owner.scope
                    && session.owner.workspace.upgrade().as_ref() == Some(&save.workspace)
                    && save.owner.current(self, session.closing, cx)
            })
    }
}
