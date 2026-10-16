//! Identity-scoped FIFO for current analog intents. Worker cancellation stops
//! subsequent setters and preserves the source's independent optimistic UI.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::keyboard_submission::{self, Intent};
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

struct Command {
    generation: u64,
    intent: Intent,
}
pub(super) struct KeyboardActuationSave {
    workspace: Entity<ProductWorkspace>,
    request: razer_pages::features::keyboard_products::KeyboardActuationRequested,
    identity: String,
    profile: razer_model::model::Profile,
}
pub(super) struct Session {
    identity: String,
    profile: String,
    revision: u64,
    valid: Arc<AtomicBool>,
    commands: mpsc::SyncSender<Command>,
    pending: std::collections::BTreeSet<u64>,
    _delivery: Option<Task<()>>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
    }
}
enum Reply {
    Finished(u64, Result<(), String>),
    CleanupFailed(String),
    Stopped,
}

fn worker(
    route_observation: discovery::ObservedDevice,
    profile_guid: String,
    valid: Arc<AtomicBool>,
    commands: mpsc::Receiver<Command>,
    replies: mpsc::Sender<Reply>,
) {
    let result = (|| -> anyhow::Result<()> {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(valid.clone());
        let result = (|| -> anyhow::Result<()> {
            while valid.load(Ordering::Acquire) {
                let command = match commands.recv_timeout(Duration::from_millis(16)) {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let guard = || -> anyhow::Result<()> {
                    ensure!(
                        valid.load(Ordering::Acquire),
                        "Keyboard connection/profile task was canceled"
                    );
                    Ok(())
                };
                let submitted = (|| -> anyhow::Result<()> {
                    guard()?;
                    let cap = keyboard_submission::validate(
                        route_observation.product_id(),
                        &command.intent,
                    )?;
                    let route = direct::resolve(
                        &mut client,
                        &route_observation,
                        &cap.device_read_capability(),
                    )?;
                    route.submit_keyboard_control(
                        &mut client,
                        &profile_guid,
                        command.intent,
                        &guard,
                    )?;
                    Ok(())
                })()
                .map_err(|error| format!("{error:#}"));
                if replies
                    .send(Reply::Finished(command.generation, submitted))
                    .is_err()
                {
                    break;
                }
            }
            Ok(())
        })();
        let cleanup = if client.is_stopped() {
            Ok(serde_json::json!({}))
        } else {
            client.request(ServiceRequest::Shutdown)
        };
        if let Err(error) = cleanup {
            let _ = replies.send(Reply::CleanupFailed(format!("{error:#}")));
        }
        result
    })();
    if let Err(error) = result {
        let reason = format!("{error:#}");
        for command in commands.try_iter() {
            let _ = replies.send(Reply::Finished(command.generation, Err(reason.clone())));
        }
        let _ = replies.send(Reply::CleanupFailed(reason));
    }
    let _ = replies.send(Reply::Stopped);
}

impl AppShell {
    pub(super) fn install_keyboard_actuation_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            this.keyboard_actuation_operations.clear();
            let workers = std::mem::take(&mut this.keyboard_actuation_workers);
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    if worker.join().is_err() {
                        eprintln!("[keyboard submission cleanup] worker panicked");
                    }
                }
            });
            async move { cleanup.await }
        }));
    }

    pub(super) fn submit_keyboard_actuation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardActuationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = request.generation();
        if !workspace
            .read(cx)
            .keyboard_actuation_request_current(generation, cx)
        {
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
            keyboard_submission::intent(device.product_id, request.payload())?;
            let mut profile = device
                .profiles
                .iter()
                .find(|profile| {
                    profile.id == device.active_profile || profile.guid == device.active_profile
                })
                .context("Active keyboard profile is absent")?
                .clone();
            let mut settings = request.profile().clone();
            keyboard_submission::persist_local(request.payload(), &mut settings)?;
            profile.source_settings = Some(settings);
            Ok(KeyboardActuationSave {
                workspace: workspace.clone(),
                request: request.clone(),
                identity: workspace.read(cx).identity(cx),
                profile,
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
        let file = store::WorkspaceFile::new(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            self.tracking_intro_seen,
        )
        .with_shortcuts(self.shortcuts.read(cx).saved_snapshot())
        .with_preferences(self.settings.read(cx).snapshot())
        .with_custom_colors(cx.global::<CustomColors>().colors())
        .with_host_tab_order(self.host_tabs.order())
        .with_dashboard(self.dashboard_state.read(cx).snapshot())
        .with_module_services(self.module_catalog.read(cx).service_snapshot())
        .with_macros(self.macro_library.read(cx).snapshot());
        self.pending_saves.push_back(PreparedSave {
            leviathan: None,
            window: window.window_handle(),
            file,
            mouse_polling: None,
            mouse_dpi_stages: None,
            mouse_dpi: None,
            mouse_current_dpi: None,
            mouse_current_power: None,
            controller_trigger: None,
            seiren_eq: None,
            mouse_profile: None,
            keyboard_actuation: Some(save),
        });
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_keyboard_actuation_save(&self, save: &mut PreparedSave, cx: &App) {
        let Some(control) = &save.keyboard_actuation else {
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
        window: AnyWindowHandle,
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
        if !succeeded || !current || self.close_requested {
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(
                    generation,
                    Err(if succeeded {
                        "Keyboard submission scope ended after local persistence"
                    } else {
                        "Keyboard local profile persistence failed"
                    }
                    .into()),
                    cx,
                )
            });
            return;
        }
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let workspace = save.workspace.clone();
            if window
                .update(cx, |_, window, cx| {
                    owner.update(cx, |this, cx| {
                        this.submit_keyboard_actuation_device(
                            save.workspace,
                            &save.request,
                            window,
                            cx,
                        )
                    })
                })
                .ok()
                .is_none_or(|result| result.is_err())
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_actuation(
                        generation,
                        Err("Keyboard owner window closed after local persistence".into()),
                        cx,
                    )
                });
            }
        });
    }

    pub(super) fn submit_keyboard_actuation_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardActuationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let generation = request.generation();
        if !workspace
            .read(cx)
            .keyboard_actuation_request_current(generation, cx)
        {
            return;
        }
        let selected = (|| -> anyhow::Result<_> {
            let device = workspace.read(cx).device(cx);
            let intent = keyboard_submission::intent(device.product_id, request.payload())?;
            let profile = device
                .profiles
                .iter()
                .find(|profile| {
                    profile.id == device.active_profile || profile.guid == device.active_profile
                })
                .context("Active keyboard profile is absent")?;
            let routes = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(device)
                        && route.peer_product_id().is_none()
                        && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
                })
                .collect::<Vec<_>>();
            let [route] = routes.as_slice() else {
                anyhow::bail!("Keyboard submission requires a unique observed wired collection")
            };
            Ok((
                intent,
                (**route).clone(),
                workspace.read(cx).identity(cx),
                device.active_profile.clone(),
                profile.guid.clone(),
            ))
        })();
        let (intent, route, identity, profile, guid) = match selected {
            Ok(selected) => selected,
            Err(error) => {
                let error = format!("{error:#}");
                eprintln!("[keyboard submission] {error}");
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_actuation(generation, Err(error), cx)
                });
                return;
            }
        };
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
            self.keyboard_actuation_operations.remove(&key);
        }
        if !self.keyboard_actuation_operations.contains_key(&key) {
            let (commands, incoming) = mpsc::sync_channel(128);
            let (sender, replies) = mpsc::channel();
            let valid = Arc::new(AtomicBool::new(true));
            let signal = valid.clone();
            let worker = match thread::Builder::new()
                .name("keyboard-actuation-owner".into())
                .spawn(move || worker(route, guid, signal, incoming, sender))
            {
                Ok(worker) => worker,
                Err(error) => {
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
            self.keyboard_actuation_workers
                .retain(|worker| !worker.is_finished());
            self.keyboard_actuation_workers.push(worker);
            let revision = self.discovery_revision;
            self.keyboard_actuation_operations.insert(
                key,
                Session {
                    identity: identity.clone(),
                    profile: profile.clone(),
                    revision,
                    valid: valid.clone(),
                    commands,
                    pending: Default::default(),
                    _delivery: None,
                },
            );
            let target = workspace.clone();
            let delivery = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_millis(16)).await;
                    let events: Vec<_> = replies.try_iter().collect();
                    let mut stop = false;
                    if this.update_in(cx, |this, _, cx| {
                        let current = this.devices.contains(&target) && this.discovery_revision == revision
                            && target.read(cx).identity(cx) == identity
                            && target.read(cx).device(cx).active_profile == profile;
                        if !current { valid.store(false, Ordering::Release); }
                        for event in events {
                            match event {
                                Reply::Finished(generation, result) => {
                                    if let Some(session) = this.keyboard_actuation_operations.get_mut(&key).filter(|session| Arc::ptr_eq(&session.valid, &valid)) {
                                        session.pending.remove(&generation);
                                    }
                                    let result = if current && valid.load(Ordering::Acquire) { result } else { Err("Keyboard connection/profile scope ended".into()) };
                                    target.update(cx, |target, cx| target.finish_keyboard_actuation(generation, result, cx));
                                }
                                Reply::CleanupFailed(error) => eprintln!("[keyboard submission cleanup] {error}"),
                                Reply::Stopped => {
                                    stop = true;
                                    {
                                        if let Some(session) = this.keyboard_actuation_operations.get_mut(&key).filter(|session| Arc::ptr_eq(&session.valid, &valid)) {
                                            for generation in std::mem::take(&mut session.pending) {
                                                target.update(cx, |target, cx| target.finish_keyboard_actuation(generation, Err("Keyboard submission worker exited before completing this request".into()), cx));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if stop && this.keyboard_actuation_operations.get(&key).is_some_and(|session| Arc::ptr_eq(&session.valid, &valid)) {
                            this.keyboard_actuation_operations.remove(&key);
                        }
                    }).is_err() { valid.store(false, Ordering::Release); break; }
                    if stop { break; }
                }
            });
            self.keyboard_actuation_operations
                .get_mut(&key)
                .unwrap()
                ._delivery = Some(delivery);
        }
        let queued = self.keyboard_actuation_operations[&key]
            .commands
            .try_send(Command { generation, intent });
        if let Err(error) = queued {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(
                    generation,
                    Err(format!("Keyboard queue unavailable: {error}")),
                    cx,
                )
            });
        } else {
            self.keyboard_actuation_operations
                .get_mut(&key)
                .unwrap()
                .pending
                .insert(generation);
        }
    }
}
