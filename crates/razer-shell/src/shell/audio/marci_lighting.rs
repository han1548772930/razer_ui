//! Local profile publication owns one independent Marci rendering connection.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::{
    marci_lighting::{Operation, Status},
    receiver_lighting::QuickEffect,
};
use razer_discovery::discovery::ObservedDevice;
use razer_ipc::{
    ServiceClient, ServiceRequest,
    device_tasks::{Lease, Registry, Scope},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

pub(in crate::shell) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    snapshot: Device,
    effect: QuickEffect,
}
impl LocalSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    pub(in crate::shell) fn snapshot(&self) -> &Device {
        &self.snapshot
    }
}

fn profile_effect(device: &Device) -> anyhow::Result<QuickEffect> {
    let value = device
        .active_profile_obj()
        .and_then(|profile| profile.source_settings.as_ref())
        .and_then(|settings| settings.pointer("/profile/quickEffects"))
        .context("Marci profile has no source Quick effect")?;
    QuickEffect::parse(value)
}

#[derive(Default)]
struct Connection {
    client: Option<ServiceClient>,
    started: bool,
    lease: Option<Lease>,
    cleanup: Option<Result<(), String>>,
}
pub(in crate::shell) struct Owner {
    route: ObservedDevice,
    id: String,
    accepting: AtomicBool,
    generation: AtomicU64,
    pending: Mutex<Option<(u64, QuickEffect)>>,
    connection: Mutex<Connection>,
    predecessor: Option<Arc<Owner>>,
}
impl Owner {
    fn publish(&self, effect: QuickEffect) -> anyhow::Result<()> {
        ensure!(
            self.accepting.load(Ordering::Acquire),
            "Marci physical owner has retired"
        );
        let generation = self
            .generation
            .try_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map_err(|_| anyhow::anyhow!("Marci generation exhausted"))?
            + 1;
        *self
            .pending
            .lock()
            .map_err(|_| anyhow::anyhow!("Marci pending effect is poisoned"))? =
            Some((generation, effect));
        Ok(())
    }
    fn current(&self, generation: u64) -> bool {
        self.accepting.load(Ordering::Acquire)
            && self.generation.load(Ordering::Acquire) == generation
    }
    fn request(&self, client: &mut ServiceClient, operation: Operation) -> anyhow::Result<Status> {
        let status: Status =
            serde_json::from_value(client.request(ServiceRequest::MarciLighting {
                device_container_id: self.route.container().into(),
                owner_id: self.id.clone(),
                lighting_operation: operation,
            })?)?;
        ensure!(
            status.owner_id == self.id
                && status
                    .device_container_id
                    .eq_ignore_ascii_case(self.route.container()),
            "Marci reply belongs to another physical owner"
        );
        Ok(status)
    }
    fn step(&self) -> anyhow::Result<Option<Status>> {
        if !self.accepting.load(Ordering::Acquire) {
            return Ok(None);
        }
        let pending = self
            .pending
            .lock()
            .map_err(|_| anyhow::anyhow!("Marci pending effect is poisoned"))?
            .take();
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Marci connection is poisoned"))?;
        if !self.accepting.load(Ordering::Acquire) {
            return Ok(None);
        }
        if let Some((generation, effect)) = pending {
            if !self.current(generation) {
                return Ok(None);
            }
            if connection.client.is_none() {
                if let Some(predecessor) = &self.predecessor {
                    predecessor.release()?;
                }
                connection.lease = Some(
                    Registry::global()
                        .try_exclusive(Scope::container(3942, self.route.container())?)?,
                );
                connection.client = Some(ServiceClient::spawn()?);
            }
            if !self.current(generation) {
                return Ok(None);
            }
            // An unsuccessful Apply can still have opened HID handles.
            connection.started = true;
            let status = self.request(
                connection.client.as_mut().unwrap(),
                Operation::Apply { generation, effect },
            )?;
            ensure!(
                status.generation == generation,
                "Marci Apply returned another generation"
            );
            return Ok(self.current(generation).then_some(status));
        }
        if !connection.started {
            return Ok(None);
        }
        let generation = self.generation.load(Ordering::Acquire);
        let status = self.request(
            connection
                .client
                .as_mut()
                .context("Marci child connection is absent")?,
            Operation::Poll,
        )?;
        Ok(self.current(generation).then_some(status))
    }
    fn retire(&self) {
        self.accepting.store(false, Ordering::Release);
    }
    fn release(&self) -> anyhow::Result<()> {
        self.retire();
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Marci connection is poisoned during release"))?;
        if let Some(result) = &connection.cleanup {
            return result.clone().map_err(anyhow::Error::msg);
        }
        let mut errors = Vec::new();
        if connection.started {
            if let Some(client) = connection.client.as_mut() {
                match self.request(client, Operation::Stop) {
                    Ok(status) if status.cleanup_completed() => {}
                    Ok(status) => errors.push(format!(
                        "Marci restoration or release is unconfirmed: {status:?}"
                    )),
                    Err(error) => errors.push(format!("Marci Stop: {error:#}")),
                }
            } else {
                errors.push("Marci active owner has no child connection".into());
            }
        }
        if let Some(client) = connection.client.as_mut() {
            if !client.is_stopped() {
                if let Err(error) = client.request(ServiceRequest::Shutdown) {
                    errors.push(format!("Marci child shutdown: {error:#}"));
                }
            }
        }
        // ServiceClient's own teardown owns process termination. The retained
        // result records failed device restoration even after handles close.
        connection.client.take();
        connection.lease.take();
        connection.started = false;
        let result = if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        };
        connection.cleanup = Some(result.clone());
        result.map_err(anyhow::Error::msg)
    }
}

struct Session {
    owner: Arc<Owner>,
    _poll: Task<()>,
}
#[derive(Default)]
pub(in crate::shell) struct Runtime {
    owners: BTreeMap<String, Session>,
    retired: Vec<Arc<Owner>>,
    tasks: BTreeMap<u64, Task<()>>,
    next_task: u64,
    statuses: BTreeMap<String, Status>,
    cleanup_errors: Vec<String>,
}
impl Runtime {
    pub(in crate::shell) fn pending(&self) -> bool {
        !self.tasks.is_empty()
    }
}

impl AppShell {
    pub(in crate::shell) fn persist_marci_lighting(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        effect: QuickEffect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || self.storage_error.is_some()
            || !self.devices.contains(&workspace)
        {
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        if device.product_id != 3942
            || !profile_effect(&device).is_ok_and(|current| current.same_ui_effect(&effect))
        {
            return;
        }
        let mut snapshot = workspace.read(cx).saved_snapshot(cx);
        let active = device
            .active_profile_obj()
            .and_then(|profile| profile.source_settings.as_ref())
            .and_then(|settings| settings.pointer("/profile/quickEffects"))
            .cloned();
        let Some(active) = active else {
            self.status = "3942 灯光设置缺少当前配置档的效果集合；尚未保存或提交。".into();
            cx.notify();
            return;
        };
        let Some(target) = snapshot
            .profiles
            .iter_mut()
            .find(|profile| profile.id == device.active_profile)
        else {
            self.status = "3942 灯光设置的活动配置档已变更；尚未保存或提交。".into();
            cx.notify();
            return;
        };
        let Some(target) = target
            .source_settings
            .as_mut()
            .and_then(|settings| settings.pointer_mut("/profile/quickEffects"))
        else {
            self.status = "3942 灯光设置缺少持久化效果集合；尚未保存或提交。".into();
            cx.notify();
            return;
        };
        *target = active;
        snapshot.active_profile = device.active_profile.clone();
        let save = LocalSave {
            workspace: workspace.clone(),
            identity: workspace.read(cx).identity(cx),
            profile_id: device.active_profile,
            snapshot,
            effect,
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
            Some(ProductSave::MarciLighting(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(in crate::shell) fn finish_marci_lighting_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if !succeeded || self.close_requested || !self.devices.contains(&save.workspace) {
            return;
        }
        let device = save.workspace.read(cx).snapshot(cx);
        if save.workspace.read(cx).identity(cx) != save.identity
            || device.active_profile != save.profile_id
            || !profile_effect(&device).is_ok_and(|effect| effect.same_ui_effect(&save.effect))
        {
            return;
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(&device)
                    && route.product_id() == 3942
                    && route.physical_product_id() == 3942
                    && route.peer_product_id().is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.status = "3942 灯光已保存到本机；尚无唯一的真实设备连接。".into();
            cx.notify();
            return;
        };
        let owner = self.marci_owner(&save.identity, route.clone(), cx);
        if let Err(error) = owner.publish(save.effect) {
            self.status = format!("3942 灯光尚未提交：{error:#}");
        }
    }
    fn marci_owner(
        &mut self,
        identity: &str,
        route: ObservedDevice,
        cx: &mut Context<Self>,
    ) -> Arc<Owner> {
        if let Some(session) = self.marci_lighting.owners.get(identity) {
            if same_route(&session.owner.route, &route)
                && session.owner.accepting.load(Ordering::Acquire)
            {
                return session.owner.clone();
            }
        }
        if let Some(session) = self.marci_lighting.owners.remove(identity) {
            self.release_marci_owner(session.owner.clone(), cx);
        }
        let predecessor = self
            .marci_lighting
            .retired
            .iter()
            .rev()
            .find(|owner| {
                owner
                    .route
                    .container()
                    .eq_ignore_ascii_case(route.container())
            })
            .cloned();
        let owner = Arc::new(Owner {
            route,
            id: format!(
                "marci-{}-{}",
                std::process::id(),
                self.marci_lighting.next_task
            ),
            accepting: AtomicBool::new(true),
            generation: AtomicU64::new(0),
            pending: Mutex::new(None),
            connection: Mutex::new(Connection::default()),
            predecessor,
        });
        self.marci_lighting.next_task += 1;
        let selected = owner.clone();
        let key = identity.to_owned();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if !selected.accepting.load(Ordering::Acquire) {
                    break;
                }
                let worker = selected.clone();
                let result = cx.background_spawn(async move { worker.step() }).await;
                if !selected.accepting.load(Ordering::Acquire) {
                    break;
                }
                let keep = this
                    .update(cx, |this, cx| {
                        let current = this
                            .marci_lighting
                            .owners
                            .get(&key)
                            .is_some_and(|session| Arc::ptr_eq(&session.owner, &selected));
                        if !current {
                            return false;
                        }
                        let mut failed = false;
                        match result {
                            Ok(Some(status)) if selected.current(status.generation) => {
                                if let Some(error) = status.errors.last() {
                                    this.status = format!("3942 灯光输出失败：{error}");
                                } else if let Some(error) = status
                                    .screen
                                    .as_ref()
                                    .and_then(|screen| screen.last_error.as_deref())
                                {
                                    this.status = format!("3942 环境感知屏幕采集失败：{error}");
                                }
                                failed = status.stopped;
                                if failed {
                                    selected.retire();
                                }
                                this.marci_lighting.statuses.insert(key.clone(), status);
                            }
                            Ok(_) => {}
                            Err(error) => {
                                this.status = format!("3942 灯光尚未确认：{error:#}");
                                selected.retire();
                                failed = true;
                            }
                        }
                        if failed {
                            this.marci_lighting.owners.remove(&key);
                            this.release_marci_owner(selected.clone(), cx);
                        }
                        cx.notify();
                        !failed
                    })
                    .unwrap_or(false);
                if !keep {
                    selected.retire();
                    break;
                }
            }
        });
        self.marci_lighting.owners.insert(
            identity.into(),
            Session {
                owner: owner.clone(),
                _poll: poll,
            },
        );
        owner
    }
    fn release_marci_owner(&mut self, owner: Arc<Owner>, cx: &mut Context<Self>) {
        owner.retire();
        if self
            .marci_lighting
            .retired
            .iter()
            .any(|previous| Arc::ptr_eq(previous, &owner))
        {
            return;
        }
        self.marci_lighting.retired.push(owner.clone());
        let id = self.marci_lighting.next_task;
        self.marci_lighting.next_task += 1;
        let task = cx.spawn(async move |this, cx| {
            let selected = owner.clone();
            let result = cx.background_spawn(async move { selected.release() }).await;
            let _ = this.update(cx, |this, cx| {
                this.marci_lighting.tasks.remove(&id);
                match result {
                    Err(error) => {
                        this.marci_lighting
                            .cleanup_errors
                            .push(format!("{error:#}"));
                        this.status = format!("3942 灯光恢复或释放未确认：{error:#}");
                        this.cancel_pending_exit(cx);
                    }
                    Ok(()) => this
                        .marci_lighting
                        .retired
                        .retain(|retained| !Arc::ptr_eq(retained, &owner)),
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        });
        self.marci_lighting.tasks.insert(id, task);
    }
    pub(in crate::shell) fn sync_marci_lighting_owners(
        &mut self,
        complete: bool,
        cx: &mut Context<Self>,
    ) {
        if !complete {
            return;
        }
        let stale = self
            .marci_lighting
            .owners
            .iter()
            .filter_map(|(key, session)| {
                let retained = self
                    .devices
                    .iter()
                    .find(|workspace| workspace.read(cx).identity(cx) == *key);
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|route| {
                        retained
                            .is_some_and(|workspace| route.matches(workspace.read(cx).device(cx)))
                    })
                    .collect::<Vec<_>>();
                (!matches!(routes.as_slice(), [route] if same_route(route, &session.owner.route)))
                    .then(|| key.clone())
            })
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(session) = self.marci_lighting.owners.remove(&key) {
                self.release_marci_owner(session.owner.clone(), cx);
            }
            self.marci_lighting.statuses.remove(&key);
        }
    }
    pub(in crate::shell) fn retire_marci_lighting_owners(&mut self, cx: &mut Context<Self>) {
        for (_, session) in std::mem::take(&mut self.marci_lighting.owners) {
            self.release_marci_owner(session.owner.clone(), cx);
        }
    }
    pub(in crate::shell) fn install_marci_lighting_cleanup(&mut self) {
        self.register_exit_cleanup(|this, cx| {
            let mut owners = std::mem::take(&mut this.marci_lighting.owners)
                .into_values()
                .map(|session| {
                    session.owner.retire();
                    session.owner.clone()
                })
                .collect::<Vec<_>>();
            owners.extend(this.marci_lighting.retired.iter().cloned());
            cx.background_executor().spawn(async move {
                for owner in owners {
                    if let Err(error) = owner.release() {
                        eprintln!("[Marci exit cleanup] {error:#}");
                    }
                }
            })
        });
    }
}

fn same_route(first: &ObservedDevice, second: &ObservedDevice) -> bool {
    first.product_id() == second.product_id()
        && first.physical_product_id() == second.physical_product_id()
        && first.container().eq_ignore_ascii_case(second.container())
        && first.hid_node() == second.hid_node()
}
