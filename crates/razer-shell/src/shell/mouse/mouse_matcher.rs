//! Matcher composition. The retained service owns capture; the common workspace
//! queue owns durable publication and rejects expired profile/connection owners.
use super::*;
use anyhow::{Context as _, ensure};
use razer_pages::features::{
    mouse_dpi_matcher::{MatcherAction, MatcherCommand, MatcherObservation},
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
    id: uuid::Uuid,
    workspace: WeakEntity<ProductWorkspace>,
    scope: MouseMatcherScope,
    connection_revision: u64,
    handle: Option<Handle>,
    _delivery: Task<()>,
}
pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    session_id: uuid::Uuid,
    connection_revision: u64,
    scope: MouseMatcherScope,
    mutation: razer_model::mouse_matcher::Mutation,
    reply: Option<Sender<Result<(), String>>>,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
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
        self.mouse_matcher
            .cleanup
            .retain(|cleanup| !cleanup.reported || !cleanup.completion.is_released());
        if let Some(mut session) = self.mouse_matcher.sessions.remove(identity) {
            if let Some(handle) = session.handle.take() {
                handle.cancel();
                let completion = handle.completion();
                let received = completion.clone();
                let id = session.id;
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
            if cleanup.completion.is_finished() && !cleanup.completion.is_released() {
                return Err(cleanup
                    .completion
                    .cleanup_error()
                    .unwrap_or_else(|| "DPI Matcher 传输资源尚未确认释放".into()));
            }
        }
        let identities = self
            .mouse_matcher
            .sessions
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for identity in identities {
            self.retire_mouse_matcher(&identity, cx);
        }
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
                session.workspace.upgrade().as_ref() == Some(&workspace)
                    && session.scope == *request.scope()
                    && session.connection_revision == revision
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
            let (handle, updates) = Handle::start(options, predecessors);
            let weak = workspace.downgrade();
            let scope = request.scope().clone();
            let retained_identity = identity.clone();
            let retained_route = route.clone();
            let session_id = uuid::Uuid::new_v4();
            let delivery = cx.spawn_in(window, async move |shell, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    let current = cx.update(|window, cx| {
                        shell.update(cx, |shell, cx| {
                            let Some(workspace) = weak.upgrade() else {
                                return false;
                            };
                            let current = !shell.close_requested
                                && scope.is_valid()
                                && shell
                                    .mouse_matcher
                                    .sessions
                                    .get(&retained_identity)
                                    .is_some_and(|session| session.id == session_id)
                                && shell.devices.contains(&workspace)
                                && workspace.read(cx).connection_revision() == revision
                                && workspace.read(cx).mouse_matcher_scope(cx).as_ref()
                                    == Some(&scope)
                                && super::device_discovery::retained_connection(
                                    std::slice::from_ref(&retained_route),
                                    &shell.device_observations,
                                    workspace.read(cx).device(cx),
                                );
                            if !current {
                                return false;
                            }
                            for _ in 0..64 {
                                match updates.try_next() {
                                    Ok(Some(update)) => shell.receive_mouse_matcher(
                                        workspace.clone(),
                                        &scope,
                                        session_id,
                                        revision,
                                        update,
                                        window,
                                        cx,
                                    ),
                                    Ok(None) => return true,
                                    Err(_) => return false,
                                }
                            }
                            true
                        })
                    });
                    if !matches!(current, Ok(Ok(true))) {
                        break;
                    }
                }
                let _ = shell.update(cx, |shell, cx| {
                    if shell
                        .mouse_matcher
                        .sessions
                        .get(&retained_identity)
                        .is_some_and(|session| session.id == session_id)
                    {
                        shell.retire_mouse_matcher(&retained_identity, cx);
                    }
                });
            });
            self.mouse_matcher.sessions.insert(
                identity.clone(),
                Session {
                    id: session_id,
                    workspace: workspace.downgrade(),
                    scope: request.scope().clone(),
                    connection_revision: revision,
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
        workspace: Entity<ProductWorkspace>,
        scope: &MouseMatcherScope,
        session_id: uuid::Uuid,
        connection_revision: u64,
        update: Update,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                    identity: workspace.read(cx).identity(cx),
                    workspace,
                    session_id,
                    connection_revision,
                    scope: scope.clone(),
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
                    window.window_handle(),
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
                    Ok(draft) => workspace.update(cx, |workspace, cx| {
                        workspace.observe_mouse_matcher(
                            scope,
                            MatcherObservation::Stages {
                                stages: draft.stages,
                                active: draft.active,
                            },
                            window,
                            cx,
                        )
                    }),
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
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_mouse_matcher(scope, observation, window, cx)
                });
            }
            Update::Failure(error) => {
                eprintln!("[dpi-matcher] {error}");
                workspace.update(cx, |workspace, cx| {
                    workspace.reject_mouse_matcher(scope, error, cx)
                });
            }
        }
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
        let target = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == matcher.identity)
            .context("Matcher workspace was removed before persistence")?;
        matcher.mutation.apply(target)
    }

    pub(super) fn finish_mouse_matcher_save(
        &mut self,
        mut save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let current = succeeded && self.mouse_matcher_save_current(&save, cx);
        if let Some(reply) = save.reply.take() {
            let _ = reply.send(if current {
                Ok(())
            } else {
                Err("Matcher save failed or its profile owner changed".into())
            });
        }
    }

    fn mouse_matcher_save_current(&self, save: &LocalSave, cx: &App) -> bool {
        save.scope.is_valid()
            && save.workspace.read(cx).connection_revision() == save.connection_revision
            && save.workspace.read(cx).mouse_matcher_scope(cx).as_ref() == Some(&save.scope)
            && self
                .mouse_matcher
                .sessions
                .get(&save.identity)
                .is_some_and(|session| {
                    session.id == save.session_id
                        && session.workspace.upgrade().as_ref() == Some(&save.workspace)
                })
    }
}
