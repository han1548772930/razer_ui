//! The shell binds an ordered thermal owner to one retained product connection.
use super::*;
use razer_device::hanbo_settings::LocalChange;
use razer_device::hanbo_settings::ProfileRestore;
use razer_discovery::discovery::{ObservedDevice, ObservedTransport};
use razer_pages::features::HanboCoolerRequested;
use razer_service::hanbo_cooler::{Reply, Session};

fn restoration_session<T>(
    initial: Option<&T>,
    current: Option<T>,
    accepting_same_owner: impl FnOnce(&T, &T) -> bool,
    ensure_current: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    match initial {
        Some(initial) => current
            .filter(|current| accepting_same_owner(initial, current))
            .ok_or_else(|| "Hanbo startup restoration belongs to an expired session".into()),
        None => ensure_current(),
    }
}

pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile: Option<razer_model::model::Profile>,
    change: LocalChange,
    command: Option<HanboCoolerRequested>,
    commands: Vec<razer_device::hanbo_cooler::CoolerCommand>,
    connection_revision: u64,
    active_profile: String,
    profile_revision: Option<u64>,
}

impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

pub(super) struct ProfileSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    device: Device,
    restore: Option<ProfileRestore>,
    commands: Vec<razer_device::hanbo_cooler::CoolerCommand>,
    connection_revision: u64,
    initial_session: Option<Session>,
    profile_revision: Option<u64>,
}

impl ProfileSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

pub(super) struct Owner {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    route: ObservedDevice,
    session: Session,
    startup_profile_queued: bool,
    startup_restore_admitted: bool,
    telemetry_activated: bool,
    task: Option<Task<()>>,
}

impl Owner {
    fn matches(
        &self,
        workspace: &Entity<ProductWorkspace>,
        route: &ObservedDevice,
        cx: &App,
    ) -> bool {
        self.workspace == workspace.downgrade()
            && self.identity == workspace.read(cx).identity(cx)
            && super::device_discovery::retained_connection(
                std::slice::from_ref(&self.route),
                std::slice::from_ref(route),
                workspace.read(cx).device(cx),
            )
    }

    fn current_workspace(&self, shell: &AppShell, cx: &App) -> Option<Entity<ProductWorkspace>> {
        let workspace = self.workspace.upgrade()?;
        (shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && super::device_discovery::retained_connection(
                std::slice::from_ref(&self.route),
                &shell.device_observations,
                workspace.read(cx).device(cx),
            ))
        .then_some(workspace)
    }
}

impl AppShell {
    pub(super) fn persist_hanbo_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        restore: Option<ProfileRestore>,
        initial: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || self.storage_error.is_some()
        {
            eprintln!("[hanbo profile] local persistence owner is unavailable");
            return false;
        }
        let device = workspace.read(cx).snapshot(cx);
        if device.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return false;
        }
        let identity = workspace.read(cx).identity(cx);
        let save = ProfileSave {
            workspace: workspace.clone(),
            initial_session: initial
                .then(|| {
                    self.hanbo_cooler_owners
                        .get(&identity)
                        .map(|owner| owner.session.clone())
                })
                .flatten(),
            identity,
            device,
            restore,
            commands: Vec::new(),
            connection_revision: workspace.read(cx).connection_revision(),
            profile_revision: workspace.read(cx).hanbo_profile_revision(cx),
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
            Some(ProductSave::HanboProfile(save)),
            cx,
        ));
        self.start_pending_save(cx);
        true
    }

    pub(super) fn rebase_hanbo_profile_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::HanboProfile(profile)) = &mut save.product else {
            return Ok(());
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|owner| owner.read(cx).saved_snapshot(cx))
            .collect();
        let previous = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == profile.identity)
            .ok_or_else(|| anyhow::anyhow!("Hanbo profile owner is absent"))?;
        if let Some(restore) = profile.restore {
            profile.commands = restore.commands(
                previous
                    .active_profile_obj()
                    .and_then(|profile| profile.source_settings.as_ref()),
                profile
                    .device
                    .active_profile_obj()
                    .and_then(|profile| profile.source_settings.as_ref()),
            )?;
        }
        *previous = profile.device.clone();
        Ok(())
    }

    pub(super) fn finish_hanbo_profile_save(
        &mut self,
        save: ProfileSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> Result<(), String> {
            if !succeeded {
                return Err("Hanbo profile was not persisted before restoration".into());
            }
            if save.restore.is_none() {
                return Ok(());
            }
            if !self.devices.contains(&save.workspace)
                || save.workspace.read(cx).identity(cx) != save.identity
                || save.workspace.read(cx).connection_revision() != save.connection_revision
                || save.workspace.read(cx).hanbo_profile_revision(cx) != save.profile_revision
                || save.workspace.read(cx).device(cx).active_profile != save.device.active_profile
            {
                return Err("Hanbo connection or profile changed before restoration".into());
            }
            // An initial save can only restore its original live session.
            // Its delayed completion must not create another startup save.
            let session = restoration_session(
                save.initial_session.as_ref(),
                self.hanbo_cooler_owners
                    .get(&save.identity)
                    .map(|owner| owner.session.clone()),
                |initial, current| initial.same_owner(current) && current.is_accepting(),
                || self.ensure_hanbo_cooler_session(&save.workspace, true, cx),
            )?;
            let commands = if self
                .hanbo_cooler_owners
                .get(&save.identity)
                .is_some_and(|owner| owner.startup_restore_admitted)
            {
                save.commands
            } else {
                razer_device::hanbo_settings::restore_commands(
                    save.device
                        .active_profile_obj()
                        .and_then(|profile| profile.source_settings.as_ref()),
                )
                .map_err(|error| format!("{error:#}"))?
            };
            if !commands.is_empty() {
                session.restore(commands)?;
            }
            self.hanbo_cooler_owners
                .get_mut(&save.identity)
                .expect("current Hanbo owner")
                .startup_restore_admitted = true;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("[hanbo profile] {error}");
            if let Some(initial) = &save.initial_session {
                if let Some(owner) = self
                    .hanbo_cooler_owners
                    .get_mut(&save.identity)
                    .filter(|owner| owner.session.same_owner(initial))
                {
                    owner.startup_profile_queued = false;
                }
            }
        }
    }

    /// ON_SET_PORT_VALUES persists before its diff dispatches the NORMAL
    /// thermal task. Local storage and native acknowledgement remain distinct.
    pub(super) fn persist_hanbo_settings(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &razer_pages::features::HanboLocalSettingsChanged,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prepare = (|| -> anyhow::Result<LocalSave> {
            anyhow::ensure!(
                !self.close_requested && self.devices.contains(&workspace),
                "Hanbo local settings owner ended"
            );
            anyhow::ensure!(
                self.storage_error.is_none(),
                "Hanbo local settings store is unreadable"
            );
            let device = workspace.read(cx).device(cx);
            anyhow::ensure!(
                device.product_id == razer_device::hanbo_cooler::PRODUCT_ID,
                "Hanbo local settings product differs"
            );
            let profile = if matches!(request.change, LocalChange::Thermal { .. }) {
                Some(
                    device
                        .profiles
                        .iter()
                        .find(|profile| profile.id == device.active_profile)
                        .ok_or_else(|| anyhow::anyhow!("Hanbo active profile is absent"))?
                        .clone(),
                )
            } else {
                None
            };
            Ok(LocalSave {
                identity: workspace.read(cx).identity(cx),
                workspace: workspace.clone(),
                profile,
                change: request.change.clone(),
                command: request.command.clone(),
                commands: Vec::new(),
                connection_revision: workspace.read(cx).connection_revision(),
                active_profile: device.active_profile.clone(),
                profile_revision: workspace.read(cx).hanbo_profile_revision(cx),
            })
        })();
        let save = match prepare {
            Ok(save) => save,
            Err(error) => {
                self.status = format!("Hanbo local settings: {error:#}");
                cx.notify();
                return;
            }
        };
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
            Some(ProductSave::HanboSettings(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_hanbo_settings_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::HanboSettings(change)) = &mut save.product else {
            return Ok(());
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).saved_snapshot(cx))
            .collect();
        let device = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == change.identity)
            .ok_or_else(|| anyhow::anyhow!("Hanbo local settings owner is absent"))?;
        let mut empty = None;
        let profile = if let Some(captured) = &change.profile {
            let existing = device
                .profiles
                .iter()
                .position(|profile| profile.id == captured.id && profile.guid == captured.guid);
            let index = match existing {
                Some(index) => index,
                None => {
                    device.profiles.push(captured.clone());
                    device.profiles.len() - 1
                }
            };
            &mut device.profiles[index].source_settings
        } else {
            &mut empty
        };
        let previous = profile.clone();
        change
            .change
            .apply(profile, &mut device.source_device_settings)?;
        change.commands = change
            .change
            .commands_after_save(previous.as_ref(), profile.as_ref())?;
        Ok(())
    }

    pub(super) fn finish_hanbo_settings_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if !succeeded {
            eprintln!("[hanbo local settings] captured local change was not persisted");
            self.status = "Hanbo local settings could not be persisted".into();
            cx.notify();
        }
        let Some(command) = save.command else {
            return;
        };
        let result = if !succeeded {
            Err("Hanbo local settings were not persisted before the device task".into())
        } else if !self.devices.contains(&save.workspace)
            || save.workspace.read(cx).identity(cx) != save.identity
            || save.workspace.read(cx).connection_revision() != save.connection_revision
            || save.workspace.read(cx).hanbo_profile_revision(cx) != save.profile_revision
            || save.workspace.read(cx).device(cx).active_profile != save.active_profile
        {
            Err("Hanbo connection or profile changed before native admission".into())
        } else {
            if save.commands.is_empty() {
                Err("Hanbo change was persisted without an active thermal device task".into())
            } else {
                self.submit_hanbo_cooler(&save.workspace, command.generation, save.commands, cx)
            }
        };
        if let Err(error) = result {
            eprintln!("[hanbo command] {error}");
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_hanbo_command(command.generation, Err(error), cx)
            });
        }
    }

    pub(super) fn install_hanbo_cooler_cleanup(&mut self) {
        self.register_exit_cleanup(|shell, cx| {
            let owners = std::mem::take(&mut shell.hanbo_cooler_owners)
                .into_values()
                .chain(std::mem::take(&mut shell.hanbo_cooler_retired))
                .collect::<Vec<_>>();
            for owner in &owners {
                owner.session.cancel();
            }
            cx.background_spawn(async move {
                for mut owner in owners {
                    owner.task.take();
                    if let Err(error) = owner.session.wait() {
                        eprintln!("[hanbo cleanup] {error}");
                    }
                }
            })
        });
    }

    pub(super) fn finish_hanbo_cooler_exit(&mut self) -> bool {
        for owner in self.hanbo_cooler_owners.values() {
            owner.session.finish();
        }
        !self.hanbo_cooler_owners.is_empty() || !self.hanbo_cooler_retired.is_empty()
    }

    pub(super) fn retain_hanbo_cooler_connections(
        &mut self,
        retained: &[Entity<ProductWorkspace>],
        cx: &App,
    ) {
        let retired = self
            .hanbo_cooler_owners
            .iter()
            .filter_map(|(key, owner)| {
                (!owner.workspace.upgrade().is_some_and(|workspace| {
                    retained.contains(&workspace)
                        && workspace.read(cx).identity(cx) == owner.identity
                }))
                .then_some(key.clone())
            })
            .collect::<Vec<_>>();
        for key in retired {
            if let Some(owner) = self.hanbo_cooler_owners.remove(&key) {
                owner.session.cancel();
                self.hanbo_cooler_retired.push(owner);
            }
        }
    }

    fn submit_hanbo_cooler(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        generation: u64,
        commands: Vec<razer_device::hanbo_cooler::CoolerCommand>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let session = self.ensure_hanbo_cooler_session(workspace, true, cx)?;
        let identity = workspace.read(cx).identity(cx);
        if !self
            .hanbo_cooler_owners
            .get(&identity)
            .is_some_and(|owner| owner.startup_restore_admitted)
        {
            let saved = workspace.read(cx).saved_snapshot(cx);
            let restore = razer_device::hanbo_settings::restore_commands(
                saved
                    .active_profile_obj()
                    .and_then(|profile| profile.source_settings.as_ref()),
            )
            .map_err(|error| format!("{error:#}"))?;
            if !restore.is_empty() {
                session.restore(restore)?;
            }
            self.hanbo_cooler_owners
                .get_mut(&identity)
                .expect("current Hanbo owner")
                .startup_restore_admitted = true;
        }
        session.submit_batch(generation, commands)
    }

    pub(super) fn sync_hanbo_cooler_connections(&mut self, cx: &mut Context<Self>) {
        if self.close_requested {
            return;
        }
        let workspaces = self
            .devices
            .iter()
            .filter(|workspace| {
                workspace.read(cx).device(cx).product_id == razer_device::hanbo_cooler::PRODUCT_ID
            })
            .cloned()
            .collect::<Vec<_>>();
        for workspace in workspaces {
            if let Err(error) = self.ensure_hanbo_cooler_session(&workspace, false, cx) {
                eprintln!("[hanbo connection] {error}");
            }
        }
    }

    pub(super) fn activate_hanbo_telemetry(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.ensure_hanbo_cooler_session(workspace, false, cx) {
            eprintln!("[hanbo telemetry activation] {error}");
        }
    }

    fn ensure_hanbo_cooler_session(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        accepted: bool,
        cx: &mut Context<Self>,
    ) -> Result<Session, String> {
        if !self.devices.contains(workspace)
            || self.close_requested && !accepted
            || workspace.read(cx).device(cx).product_id != razer_device::hanbo_cooler::PRODUCT_ID
        {
            return Err("Hanbo command rejected before submission".into());
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && route.physical_product_id() == razer_device::hanbo_cooler::PRODUCT_ID
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(ObservedTransport::Wired))
                    && route.hid_node().is_some()
            })
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            return Err("Hanbo cooler has no unique wired HID collection".into());
        };
        let route = (*route).clone();
        let identity = workspace.read(cx).identity(cx);
        if !self
            .hanbo_cooler_owners
            .get(&identity)
            .is_some_and(|owner| {
                owner.matches(workspace, &route, cx) && owner.session.is_accepting()
            })
        {
            if let Some(owner) = self.hanbo_cooler_owners.remove(&identity) {
                if owner.matches(workspace, &route, cx) {
                    owner.session.finish();
                } else {
                    owner.session.cancel();
                }
                self.hanbo_cooler_retired.push(owner);
            }
            let predecessors = self
                .hanbo_cooler_retired
                .iter()
                .filter(|owner| owner.identity == identity)
                .map(|owner| owner.session.clone())
                .collect();
            let (session, replies) = Session::start(
                route
                    .hid_node()
                    .cloned()
                    .ok_or("Hanbo HID collection is absent")?,
                workspace.read(cx).device(cx).device_container_id.clone(),
                predecessors,
            )
            .map_err(|error| format!("{error:#}"))?;
            self.hanbo_cooler_owners.insert(
                identity.clone(),
                Owner {
                    workspace: workspace.downgrade(),
                    identity: identity.clone(),
                    route,
                    session: session.clone(),
                    startup_profile_queued: false,
                    startup_restore_admitted: false,
                    telemetry_activated: false,
                    task: None,
                },
            );
            let key = identity.clone();
            let task = cx.spawn(async move |shell, cx| {
                while let Ok(reply) = replies.recv().await {
                    match reply {
                        Reply::Restored(result) => {
                            if let Err(error) = result {
                                eprintln!("[hanbo profile restore] {error}");
                            }
                        }
                        Reply::Finished { generation, result } => {
                            if let Err(error) = &result {
                                eprintln!("[hanbo command] {error}");
                            }
                            let _ = shell.update(cx, |shell, cx| {
                                let Some(owner) = shell
                                    .hanbo_cooler_owners
                                    .get(&key)
                                    .filter(|owner| owner.session.same_owner(&session))
                                else {
                                    return;
                                };
                                let Some(workspace) = owner.current_workspace(shell, cx) else {
                                    return;
                                };
                                workspace.update(cx, |workspace, cx| {
                                    workspace.finish_hanbo_command(generation, result, cx)
                                });
                            });
                        }
                        Reply::Observed(sample) => match sample {
                            Ok(telemetry) => {
                                let _ = shell.update(cx, |shell, cx| {
                                    let Some(owner) = shell
                                        .hanbo_cooler_owners
                                        .get(&key)
                                        .filter(|owner| owner.session.same_owner(&session))
                                    else {
                                        return;
                                    };
                                    let Some(workspace) = owner.current_workspace(shell, cx) else {
                                        return;
                                    };
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_hanbo_telemetry(&telemetry, cx)
                                    });
                                });
                            }
                            Err(error) => eprintln!("[hanbo telemetry] {error}"),
                        },
                        Reply::SystemObserved(sample) => match sample {
                            Ok(sample) => {
                                let _ = shell.update(cx, |shell, cx| {
                                    let Some(owner) = shell
                                        .hanbo_cooler_owners
                                        .get(&key)
                                        .filter(|owner| owner.session.same_owner(&session))
                                    else {
                                        return;
                                    };
                                    let Some(workspace) = owner.current_workspace(shell, cx) else {
                                        return;
                                    };
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_hanbo_system_sample(&sample, cx)
                                    });
                                });
                            }
                            Err(error) => eprintln!("[hanbo system monitor] {error}"),
                        },
                        Reply::SystemWriteFinished(result) => {
                            if let Err(error) = result {
                                eprintln!("[hanbo temperature write] {error}");
                            }
                        }
                        Reply::Stopped(result) => {
                            if let Err(error) = result {
                                eprintln!("[hanbo cleanup] {error}");
                            }
                            break;
                        }
                    }
                }
                let finished = session.clone();
                let result = cx.background_spawn(async move { finished.wait() }).await;
                let _ = shell.update(cx, |shell, cx| {
                    if let Err(error) = result {
                        shell.status = format!("Hanbo cleanup failed: {error}");
                        shell.cancel_pending_exit(cx);
                    }
                    if shell
                        .hanbo_cooler_owners
                        .get(&key)
                        .is_some_and(|owner| owner.session.same_owner(&session))
                    {
                        shell.hanbo_cooler_owners.remove(&key);
                    }
                    shell
                        .hanbo_cooler_retired
                        .retain(|owner| !owner.session.same_owner(&session));
                    if shell.close_requested && shell.save_task.is_none() {
                        shell.continue_save_queue(true, cx);
                    }
                    cx.notify();
                });
            });
            self.hanbo_cooler_owners
                .get_mut(&identity)
                .expect("inserted owner")
                .task = Some(task);
        }
        let session = self
            .hanbo_cooler_owners
            .get(&identity)
            .map(|owner| owner.session.clone())
            .ok_or_else(|| "Hanbo connection owner is absent".to_owned())?;
        if workspace.read(cx).hanbo_telemetry_requested(cx)
            && !self
                .hanbo_cooler_owners
                .get(&identity)
                .is_some_and(|owner| owner.telemetry_activated)
        {
            session.activate_telemetry()?;
            self.hanbo_cooler_owners
                .get_mut(&identity)
                .expect("current Hanbo owner")
                .telemetry_activated = true;
        }
        if !self
            .hanbo_cooler_owners
            .get(&identity)
            .is_some_and(|owner| owner.startup_profile_queued)
        {
            self.hanbo_cooler_owners
                .get_mut(&identity)
                .expect("current Hanbo owner")
                .startup_profile_queued = true;
            if !self.persist_hanbo_profile(
                workspace.clone(),
                Some(ProfileRestore::All),
                true,
                self.main_window,
                cx,
            ) {
                self.hanbo_cooler_owners
                    .get_mut(&identity)
                    .expect("current Hanbo owner")
                    .startup_profile_queued = false;
            }
        }
        Ok(session)
    }
}

#[cfg(test)]
#[path = "hanbo_cooler_tests.rs"]
mod tests;
