//! Current Kitsune Chroma caller, local profile selection and host navigation.
//! Local storage receipts and native lighting diagnostics remain distinct.
use super::product_chroma::{Readback, readback};
use super::product_mapping::MappingTargetOwner;
use super::*;
use anyhow::{Context as _, ensure};
use razer_pages::features::gamepad_products::{
    ControllerChromaCommand, ControllerChromaObservation, ControllerChromaProfile,
    ControllerChromaRequested,
};
use razer_storage::{
    chroma_profiles::{self, Change},
    host::{HostStorageCall, HostStorageView, HostStoreKind},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const HOST_VIEW: u64 = 0x4354_524C_4348_524F;
const SOURCE_UI_URL: &str = "/synapse/products/4115/ui/";
const SYNC_KEY: &str = "RzWinStorage_sync_quick_effect";

struct ControllerPending {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: ControllerChromaRequested,
    window: AnyWindowHandle,
    revision: u64,
}
struct MappingPending {
    change: Change,
    guard: Arc<AtomicBool>,
}
enum Pending {
    Controller(ControllerPending),
    Mapping(MappingPending),
}
struct MappingGuard {
    owners: Vec<MappingTargetOwner>,
    guard: Arc<AtomicBool>,
}
impl MappingGuard {
    fn is_current(&self, workspaces: &[Entity<ProductWorkspace>], cx: &App) -> bool {
        self.guard.load(Ordering::Acquire)
            && !self.owners.is_empty()
            && self.owners.iter().all(|owner| {
                workspaces
                    .iter()
                    .any(|workspace| owner.matches(workspace, cx))
            })
    }
}
enum ConfigurationCommand {
    Controller(ControllerChromaCommand),
    Mapping(Change),
}
impl Pending {
    fn guard(&self) -> Arc<AtomicBool> {
        match self {
            Self::Controller(pending) => pending.request.guard(),
            Self::Mapping(pending) => pending.guard.clone(),
        }
    }
    fn command(&self) -> ConfigurationCommand {
        match self {
            Self::Controller(pending) => {
                ConfigurationCommand::Controller(pending.request.command().clone())
            }
            Self::Mapping(pending) => ConfigurationCommand::Mapping(pending.change.clone()),
        }
    }
}
struct Watcher {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    guard: Arc<AtomicBool>,
    window: AnyWindowHandle,
}
struct DynamicLightingWatcher {
    owner: Watcher,
    pending: Vec<ControllerChromaRequested>,
    last_result: Option<Result<Option<bool>, String>>,
}
#[derive(Default)]
pub(super) struct State {
    pub(in crate::shell) enabled: Option<bool>,
    queue: VecDeque<Pending>,
    task: Option<Task<()>>,
    watch: Option<Task<()>>,
    watchers: BTreeMap<String, Watcher>,
    dynamic_watch: Option<Task<()>>,
    dynamic_watchers: BTreeMap<String, DynamicLightingWatcher>,
    last_observation: Option<ControllerChromaObservation>,
    last_snapshot: Option<chroma_profiles::Snapshot>,
    configuration_revision: u64,
    revision: u64,
    watch_guard: Arc<AtomicBool>,
    mapping_guards: Vec<MappingGuard>,
}
impl Drop for State {
    fn drop(&mut self) {
        for owner in &self.mapping_guards {
            owner.guard.store(false, Ordering::Release);
        }
    }
}
impl State {
    fn retain_configuration(
        &mut self,
        snapshot: &chroma_profiles::Snapshot,
        started: Option<u64>,
    ) -> chroma_profiles::Snapshot {
        if let Some(current) = &self.last_snapshot
            && (current.supersedes(snapshot)
                || (started.is_some_and(|revision| revision != self.configuration_revision)
                    && !snapshot.supersedes(current)))
        {
            return current.clone();
        }
        if self.last_snapshot.as_ref() != Some(snapshot) {
            self.configuration_revision = self.configuration_revision.wrapping_add(1);
            self.last_snapshot = Some(snapshot.clone());
        }
        snapshot.clone()
    }
}

pub(super) struct ControllerChromaSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    request: ControllerChromaRequested,
}
impl ControllerChromaSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

fn controller_observation(readback: &Readback) -> anyhow::Result<ControllerChromaObservation> {
    let mut observation = ControllerChromaObservation::new();
    if let Ok(snapshot) = &readback.snapshot {
        if let Some(configuration) = snapshot.configuration() {
            observation = observation
                .with_profiles(
                    configuration
                        .profiles()
                        .iter()
                        .map(|profile| ControllerChromaProfile::new(profile.id(), profile.name()))
                        .collect::<anyhow::Result<Vec<_>>>()?,
                    configuration.selected_profile().map(str::to_owned),
                )?
                .with_enabled(configuration.enabled());
        }
    }
    if let Ok(Some(enabled)) = readback.dynamic_lighting {
        observation = observation.with_dynamic_lighting(enabled);
    }
    if let Ok(Some(installed)) = readback.module_installed {
        observation = observation.with_module_installed(installed);
    }
    if let Ok(Some(installation)) = &readback.installation {
        observation = observation.with_fully_installed(installation.fully_installed());
    }
    Ok(observation)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuickInput {
    selected_effect_id: u8,
    effect_settings: Map<String, Value>,
    index_direction: Option<u8>,
    number_of_wave_direction: Option<u8>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuickEnvelope {
    selected_effect_id: u8,
    effect_settings: Map<String, Value>,
    timer_tick: u64,
}
impl QuickEnvelope {
    fn from_caller(value: &Value, timer_tick: u64) -> anyhow::Result<Self> {
        let input: QuickInput = serde_json::from_value(value.clone())?;
        ensure!(
            (1..=5).contains(&input.selected_effect_id),
            "Kitsune sync effect ID is not mounted"
        );
        ensure!(
            input
                .effect_settings
                .get("effectId")
                .and_then(Value::as_u64)
                == Some(u64::from(input.selected_effect_id)),
            "Sync quick effect ID does not match its setting"
        );
        let mut effect_settings = input.effect_settings;
        if let Some(direction) = input.index_direction {
            effect_settings.insert("indexDirection".into(), direction.into());
        }
        if let Some(count) = input.number_of_wave_direction {
            effect_settings.insert("numberOfWaveDirection".into(), count.into());
        }
        Ok(Self {
            selected_effect_id: input.selected_effect_id,
            effect_settings,
            timer_tick,
        })
    }
    fn is_fresh(&self, current_tick: u64) -> bool {
        // Current middleware compares signed ticks and accepts future ticks.
        (i128::from(current_tick) - i128::from(self.timer_tick)) < 100_000_000
    }
}
fn tick() -> anyhow::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()
        .map_err(Into::into)
}

impl AppShell {
    pub(super) fn request_controller_chroma(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &ControllerChromaRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace.read(cx).accepts_controller_chroma(request, cx) {
            return;
        }
        if self.close_requested {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_controller_chroma(
                    request,
                    Err("Chroma request canceled during shutdown".into()),
                    window,
                    cx,
                )
            });
            return;
        }
        if matches!(
            request.command(),
            ControllerChromaCommand::ObserveDynamicLighting
        ) {
            self.request_controller_dynamic_lighting(workspace, request, window, cx);
            return;
        }
        self.controller_chroma.revision = self.controller_chroma.revision.wrapping_add(1);
        self.controller_chroma
            .watch_guard
            .store(false, Ordering::Release);
        let navigation = match request.command() {
            ControllerChromaCommand::LaunchStudio => Some(self.try_open_chroma_studio(cx)),
            ControllerChromaCommand::OpenSettings { chroma_app } => {
                Some(self.open_product_settings(*chroma_app, window, cx))
            }
            ControllerChromaCommand::ManageApps => Some(self.manage_product_chroma_apps(false, cx)),
            _ => None,
        };
        if let Some(result) = navigation {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_controller_chroma(request, result.map(|_| None), window, cx)
            });
            return;
        }
        if matches!(
            request.command(),
            ControllerChromaCommand::SwitchOffLighting(_)
                | ControllerChromaCommand::CustomColors(_)
        ) {
            self.persist_controller_chroma(workspace, request.clone(), window, cx);
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        if matches!(request.command(), ControllerChromaCommand::Observe) {
            self.controller_chroma.watchers.insert(
                identity.clone(),
                Watcher {
                    workspace: workspace.downgrade(),
                    identity: identity.clone(),
                    guard: request.guard(),
                    window: window.window_handle(),
                },
            );
            self.start_controller_chroma_watch(cx);
        }
        self.controller_chroma
            .queue
            .push_back(Pending::Controller(ControllerPending {
                workspace,
                identity,
                request: request.clone(),
                window: window.window_handle(),
                revision: self.controller_chroma.revision,
            }));
        self.controller_lighting_workers += 1;
        self.start_controller_chroma_request(cx);
    }

    pub(super) fn request_mapping_chroma(
        &mut self,
        owners: Vec<MappingTargetOwner>,
        changes: Vec<Change>,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || changes.is_empty() {
            return;
        }
        let guard = Arc::new(AtomicBool::new(true));
        let owner = MappingGuard {
            owners,
            guard: guard.clone(),
        };
        if !owner.is_current(&self.devices, cx) {
            return;
        }
        self.controller_chroma
            .watch_guard
            .store(false, Ordering::Release);
        self.controller_chroma.mapping_guards.push(owner);
        for change in changes {
            self.controller_chroma.revision = self.controller_chroma.revision.wrapping_add(1);
            self.controller_chroma
                .queue
                .push_back(Pending::Mapping(MappingPending {
                    change,
                    guard: guard.clone(),
                }));
            self.controller_lighting_workers += 1;
        }
        self.start_controller_chroma_request(cx);
    }

    pub(super) fn retain_mapping_chroma_connections(
        &mut self,
        retained: &[Entity<ProductWorkspace>],
        cx: &App,
    ) {
        self.controller_chroma.mapping_guards.retain(|owner| {
            let current = owner.is_current(retained, cx);
            if !current {
                owner.guard.store(false, Ordering::Release);
            }
            current
        });
    }

    fn request_controller_dynamic_lighting(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &ControllerChromaRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        let guard = request.guard();
        if let Some(watcher) = self.controller_chroma.dynamic_watchers.get_mut(&identity)
            && Arc::ptr_eq(&watcher.owner.guard, &guard)
        {
            watcher.pending.push(request.clone());
            watcher.owner.window = window.window_handle();
        } else {
            self.controller_chroma.dynamic_watchers.insert(
                identity.clone(),
                DynamicLightingWatcher {
                    owner: Watcher {
                        workspace: workspace.downgrade(),
                        identity,
                        guard,
                        window: window.window_handle(),
                    },
                    pending: vec![request.clone()],
                    last_result: None,
                },
            );
        }
        self.start_controller_dynamic_lighting_watch(cx);
    }

    fn start_controller_dynamic_lighting_watch(&mut self, cx: &mut Context<Self>) {
        if self.controller_chroma.dynamic_watch.is_some() {
            return;
        }
        self.controller_chroma.dynamic_watch = Some(cx.spawn(async move |this, cx| {
            loop {
                let active = this.update(cx, |this, cx| {
                    this.controller_chroma
                        .dynamic_watchers
                        .retain(|_, watcher| {
                            watcher.owner.guard.load(Ordering::Acquire)
                                && watcher.owner.workspace.upgrade().is_some_and(|workspace| {
                                    this.devices.contains(&workspace)
                                        && workspace.read(cx).identity(cx) == watcher.owner.identity
                                })
                        });
                    if this.close_requested || this.controller_chroma.dynamic_watchers.is_empty() {
                        this.controller_chroma.dynamic_watchers.clear();
                        return None;
                    }
                    this.controller_lighting_workers += 1;
                    Some(
                        this.controller_chroma
                            .dynamic_watchers
                            .iter()
                            .map(|(identity, watcher)| {
                                (identity.clone(), watcher.owner.guard.clone())
                            })
                            .collect::<Vec<_>>(),
                    )
                });
                let Ok(Some(owners)) = active else {
                    break;
                };
                let result = cx
                    .background_spawn(async {
                        chroma_profiles::load_dynamic_lighting(&razer_storage::source_local::path())
                            .map_err(|error| format!("{error:#}"))
                    })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.controller_lighting_workers =
                            this.controller_lighting_workers.saturating_sub(1);
                        let mut deliveries = Vec::new();
                        for (identity, guard) in owners {
                            let Some(watcher) =
                                this.controller_chroma.dynamic_watchers.get_mut(&identity)
                            else {
                                continue;
                            };
                            if !guard.load(Ordering::Acquire)
                                || !Arc::ptr_eq(&guard, &watcher.owner.guard)
                            {
                                continue;
                            }
                            let Some(workspace) = watcher.owner.workspace.upgrade() else {
                                continue;
                            };
                            if !this.devices.contains(&workspace)
                                || workspace.read(cx).identity(cx) != watcher.owner.identity
                            {
                                continue;
                            }
                            let changed = watcher.last_result.as_ref() != Some(&result);
                            watcher.last_result = Some(result.clone());
                            deliveries.push((
                                workspace,
                                watcher.owner.window,
                                std::mem::take(&mut watcher.pending),
                                changed,
                            ));
                        }
                        for (workspace, handle, pending, changed) in deliveries {
                            if changed {
                                match &result {
                                    Ok(Some(enabled)) => {
                                        let observation = ControllerChromaObservation::new()
                                            .with_dynamic_lighting(*enabled);
                                        let _ = handle.update(cx, |_, window, cx| {
                                            workspace.update(cx, |workspace, cx| {
                                                workspace.observe_controller_chroma(
                                                    observation,
                                                    window,
                                                    cx,
                                                )
                                            })
                                        });
                                    }
                                    Err(error) => {
                                        this.status = format!("动态灯光状态未读取：{error}");
                                        cx.notify();
                                    }
                                    Ok(None) => {}
                                }
                            }
                            for request in pending {
                                this.finish_controller_chroma(
                                    &workspace,
                                    &request,
                                    result.clone().map(|_| None),
                                    handle,
                                    cx,
                                );
                            }
                        }
                        if this.close_requested && this.save_task.is_none() {
                            this.continue_save_queue(true, cx);
                        }
                    })
                    .is_err()
                {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
            }
            let _ = this.update(cx, |this, _| this.controller_chroma.dynamic_watch = None);
        }));
    }

    fn start_controller_chroma_request(&mut self, cx: &mut Context<Self>) {
        if self.controller_chroma.task.is_some() {
            return;
        }
        let Some(pending) = self.controller_chroma.queue.pop_front() else {
            return;
        };
        let command = pending.command();
        let guard = pending.guard();
        let owners = self
            .controller_lighting_owners
            .values()
            .map(|session| session.owner())
            .collect::<Vec<_>>();
        self.controller_chroma.task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                ensure!(guard.load(Ordering::Acquire), "Chroma request belongs to a replaced profile");
                let existing = matches!(&command, ConfigurationCommand::Mapping(_));
                let (change, sync) = match command {
                    ConfigurationCommand::Mapping(change) => (Some(change), None),
                    ConfigurationCommand::Controller(command) => match command {
                        ControllerChromaCommand::Observe => (None, None),
                        ControllerChromaCommand::Toggle {enabled} => (Some(Change::Toggle(enabled)),None),
                        ControllerChromaCommand::SelectProfile {id} => (Some(Change::SelectProfile(id)),None),
                        ControllerChromaCommand::SyncQuick {source_effect} => (Some(Change::Toggle(false)),Some(QuickEnvelope::from_caller(&source_effect,tick()?)?)),
                        _ => anyhow::bail!("Chroma command does not belong to the local configuration queue"),
                    },
                };
                let committed = change.is_some();
                let readback = if let Some(change) = change {
                    let update = if existing { chroma_profiles::update_existing } else { chroma_profiles::update };
                    super::product_chroma::from_snapshot(update(&chroma_profiles::path(), &razer_storage::source_local::path(), change, &guard)?)
                } else { readback() };
                let mut runtime_failures = Vec::new();
                if let Ok(snapshot) = &readback.snapshot && let Some(configuration) = snapshot.configuration() {
                    for owner in owners {
                        if let Err(error) = owner.set_advanced(configuration.enabled(), &guard) { runtime_failures.push(format!("{error:#}")); }
                    }
                }
                if committed && !guard.load(Ordering::Acquire) {
                    runtime_failures.push("Chroma request was canceled after its local configuration commit".into());
                } else {
                    ensure!(guard.load(Ordering::Acquire), "Chroma response belongs to a replaced profile");
                }
                Ok::<_,anyhow::Error>((readback,sync,runtime_failures,committed))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.controller_chroma.task = None;
                this.controller_lighting_workers = this.controller_lighting_workers.saturating_sub(1);
                // The local commit belongs to the shared library even when
                // its initiating page or connection has already been replaced.
                let mut publication = match &result {
                    Ok((readback, _, _, true)) => Some(this.apply_controller_chroma_readback(readback, cx)),
                    _ => None,
                };
                if let Some(Err(error)) = &publication {
                    this.status = format!("Chroma 本地配置已提交，页面状态未发布：{error:#}");
                }
                if let Ok((_, _, failures, true)) = &result && !failures.is_empty() {
                    this.status = format!("Chroma selection was saved locally; native lighting transition failed: {}", failures.join("; "));
                }
                match pending {
                    Pending::Controller(pending) => {
                    let current = this.devices.contains(&pending.workspace)
                        && pending.workspace.read(cx).identity(cx) == pending.identity
                        && pending.workspace.read(cx).accepts_controller_chroma(&pending.request,cx);
                    if current {
                    let completion = match result {
                        Ok((readback,sync,failures,committed)) => {
                            let observation = if pending.revision == this.controller_chroma.revision {
                                if committed {
                                    publication.take().expect("Committed Chroma mutation has a publication result").map(Some)
                                } else {
                                    this.apply_controller_chroma_readback(&readback,cx).map(Some)
                                }
                            } else {
                                Ok(None)
                            };
                            let delivery = sync.map(|sync| this.broadcast_controller_sync_quick(&sync,&pending.workspace,cx)).transpose();
                            match (observation,delivery,failures.is_empty()) {
                                (Ok(observation),Ok(_),true) => Ok(observation),
                                (Err(error),_,_) | (_,Err(error),_) => Err(format!("{error:#}")),
                                (_,_,false) => Err(format!("Chroma selection was saved locally; native lighting transition failed: {}", failures.join("; "))),
                            }
                        }
                        Err(error) => Err(format!("{error:#}")),
                    };
                    if let Err(error) = &completion { this.status = format!("Chroma：{error}"); }
                    this.finish_controller_chroma(&pending.workspace,&pending.request,completion,pending.window,cx);
                    }
                    }
                    Pending::Mapping(pending) => {
                        // A committed global local configuration survives the
                        // initiating connection. Publishing it does not accept
                        // an old device response or acknowledge its native I/O.
                        let completion = match result {
                                Ok((_, _, failures, _)) => {
                                    let observation = publication.take().expect("Mapped Chroma mutation has a publication result");
                                    match (observation, failures.is_empty()) {
                                        (Ok(_), true) => Ok(()),
                                        (Err(error), _) => Err(format!("{error:#}")),
                                        (_, false) => Err(format!("Chroma selection was saved locally; native lighting transition failed: {}", failures.join("; "))),
                                    }
                                }
                                Err(error) => Err(format!("{error:#}")),
                        };
                        if let Err(error) = completion {
                            this.status = format!("Chroma：{error}");
                        }
                        let still_queued = this.controller_chroma.queue.iter().any(|queued| {
                            matches!(queued, Pending::Mapping(queued) if Arc::ptr_eq(&queued.guard, &pending.guard))
                        });
                        if !still_queued {
                            this.controller_chroma.mapping_guards.retain(|owner| !Arc::ptr_eq(&owner.guard, &pending.guard));
                        }
                    }
                }
                this.start_controller_chroma_request(cx);
                if this.close_requested && this.save_task.is_none() { this.continue_save_queue(true,cx); }
                cx.notify();
            });
        }));
    }

    fn register_controller_chroma_host(&mut self) {
        self.host_storage.register_view(HostStorageView {
            id: HOST_VIEW,
            url: SOURCE_UI_URL.into(),
            remote: false,
            destroyed: false,
            crashed: false,
            disposed: false,
            tracked_url: true,
        });
    }

    fn observed_chroma_products(&mut self) -> anyhow::Result<Option<Vec<u32>>> {
        self.register_controller_chroma_host();
        super::product_chroma::connected_products(&mut self.host_storage, HOST_VIEW)
    }

    fn apply_controller_chroma_readback(
        &mut self,
        readback: &Readback,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<ControllerChromaObservation> {
        let mut readback = readback.clone();
        if let Ok(snapshot) = &readback.snapshot {
            readback.snapshot = Ok(self.controller_chroma.retain_configuration(snapshot, None));
        }
        let readback = &readback;
        let mut observation = controller_observation(readback)?;
        self.controller_chroma.enabled = readback
            .snapshot
            .as_ref()
            .ok()
            .and_then(|snapshot| snapshot.configuration())
            .map(|configuration| configuration.enabled());
        let mut diagnostics = Vec::new();
        if let Err(error) = &readback.snapshot {
            diagnostics.push(format!("本地配置：{error}"));
        }
        if let Err(error) = &readback.dynamic_lighting {
            diagnostics.push(format!("动态灯光状态：{error}"));
        }
        if let Err(error) = &readback.module_installed {
            diagnostics.push(format!("模块状态：{error}"));
        }
        match self.observed_chroma_products() {
            Ok(Some(products)) => observation = observation.with_connected_products(products),
            Ok(None) => {}
            Err(error) => diagnostics.push(format!("共享设备列表：{error:#}")),
        }
        if let Err(error) = &readback.installation {
            diagnostics.push(format!("安装状态：{error}"));
        }
        if !diagnostics.is_empty() {
            let status = format!("Chroma 部分状态未读取：{}", diagnostics.join("；"));
            if self.status != status {
                self.status = status;
                cx.notify();
            }
        }
        if self.controller_chroma.last_observation.as_ref() != Some(&observation) {
            self.controller_chroma.last_observation = Some(observation.clone());
            self.publish_controller_chroma_observation(observation.clone(), cx);
        }
        if let Ok(snapshot) = &readback.snapshot
            && let Some(configuration) = snapshot.configuration()
        {
            self.publish_shared_chroma_configuration(configuration, cx)?;
        }
        Ok(observation)
    }

    fn publish_shared_chroma_configuration(
        &mut self,
        configuration: &chroma_profiles::Configuration,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let workspaces = self.devices.clone();
        self.main_window
            .update(cx, |_, window, cx| {
                for workspace in workspaces {
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_chroma_configuration(configuration, window, cx)
                    });
                }
            })
            .context("The product window closed during Chroma configuration publication")
    }

    pub(super) fn shared_chroma_revision(&self) -> u64 {
        self.controller_chroma.configuration_revision
    }

    pub(super) fn reconcile_chroma_configuration(&mut self, readback: &mut Readback, started: u64) {
        if let Ok(snapshot) = &readback.snapshot {
            readback.snapshot = Ok(self
                .controller_chroma
                .retain_configuration(snapshot, Some(started)));
        }
    }

    pub(super) fn publish_committed_chroma_configuration(
        &mut self,
        snapshot: &chroma_profiles::Snapshot,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.apply_controller_chroma_readback(
            &Readback {
                snapshot: Ok(snapshot.clone()),
                dynamic_lighting: Ok(None),
                module_installed: Ok(None),
                installation: Ok(None),
            },
            cx,
        )?;
        Ok(())
    }

    /// A delayed host query may still deliver installation and device fields;
    /// it cannot replace a newer committed shared Chroma selection.
    pub(super) fn retain_newer_chroma_configuration(
        &mut self,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        if revision != self.controller_chroma.configuration_revision
            && let Some(configuration) = self
                .controller_chroma
                .last_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.configuration())
                .cloned()
            && let Err(error) = self.publish_shared_chroma_configuration(&configuration, cx)
        {
            self.status = format!("Chroma 状态未发布：{error:#}");
        }
    }

    fn publish_controller_chroma_observation(
        &mut self,
        observation: ControllerChromaObservation,
        cx: &mut Context<Self>,
    ) {
        let watchers = self
            .controller_chroma
            .watchers
            .values()
            .filter_map(|watcher| {
                let workspace = watcher.workspace.upgrade()?;
                (watcher.guard.load(Ordering::Acquire)
                    && self.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == watcher.identity)
                    .then(|| (workspace, watcher.window))
            })
            .collect::<Vec<_>>();
        for (workspace, handle) in watchers {
            let observation = observation.clone();
            let _ = handle.update(cx, |_, window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_controller_chroma(observation, window, cx)
                })
            });
        }
    }

    fn start_controller_chroma_watch(&mut self, cx: &mut Context<Self>) {
        if self.controller_chroma.watch.is_some() {
            return;
        }
        self.controller_chroma.watch = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let active = this.update(cx, |this, cx| {
                    this.controller_chroma.watchers.retain(|_, watcher| {
                        watcher.guard.load(Ordering::Acquire)
                            && watcher.workspace.upgrade().is_some_and(|workspace| {
                                this.devices.contains(&workspace)
                                    && workspace.read(cx).identity(cx) == watcher.identity
                            })
                    });
                    if this.close_requested || this.controller_chroma.watchers.is_empty() {
                        return None;
                    }
                    if this.controller_chroma.task.is_some()
                        || !this.controller_chroma.queue.is_empty()
                    {
                        return Some(None);
                    }
                    this.controller_lighting_workers += 1;
                    let guard = Arc::new(AtomicBool::new(true));
                    this.controller_chroma.watch_guard = guard.clone();
                    Some(Some((
                        this.controller_chroma.revision,
                        this.controller_lighting_owners
                            .values()
                            .map(|session| session.owner())
                            .collect::<Vec<_>>(),
                        guard,
                    )))
                });
                let Ok(Some(operation)) = active else {
                    break;
                };
                let Some((revision, owners, guard)) = operation else {
                    continue;
                };
                let delivery_guard = guard.clone();
                let result = cx
                    .background_spawn(async move {
                        ensure!(
                            guard.load(Ordering::Acquire),
                            "Chroma observation was canceled"
                        );
                        let readback = readback();
                        let mut failures = Vec::new();
                        if let Ok(snapshot) = &readback.snapshot
                            && let Some(configuration) = snapshot.configuration()
                        {
                            for owner in owners {
                                if let Err(error) =
                                    owner.set_advanced(configuration.enabled(), &guard)
                                {
                                    failures.push(format!("{error:#}"));
                                }
                            }
                        }
                        Ok::<_, anyhow::Error>((readback, failures))
                    })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.controller_lighting_workers =
                            this.controller_lighting_workers.saturating_sub(1);
                        if this.close_requested {
                            if this.save_task.is_none() {
                                this.continue_save_queue(true, cx);
                            }
                            return;
                        }
                        if this.controller_chroma.revision != revision
                            || this.controller_chroma.task.is_some()
                            || !delivery_guard.load(Ordering::Acquire)
                        {
                            return;
                        }
                        match result {
                            Ok((readback, failures)) => {
                                if let Err(error) =
                                    this.apply_controller_chroma_readback(&readback, cx)
                                {
                                    this.status = format!("Chroma 状态未读取：{error:#}");
                                    cx.notify();
                                }
                                if !failures.is_empty() {
                                    this.status = format!(
                                        "Chroma 本地状态已读取；灯效切换未确认：{}",
                                        failures.join("；")
                                    );
                                    cx.notify();
                                }
                            }
                            Err(error) => {
                                this.status = format!("Chroma 本地记录未读取：{error:#}");
                                cx.notify();
                            }
                        }
                    })
                    .is_err()
                {
                    return;
                }
            }
            let _ = this.update(cx, |this, _| this.controller_chroma.watch = None);
        }));
    }

    pub(super) fn prepare_controller_chroma_exit(&mut self) {
        self.controller_chroma
            .watch_guard
            .store(false, Ordering::Release);
        self.controller_chroma.watchers.clear();
        self.controller_chroma.dynamic_watchers.clear();
        for owner in self.controller_chroma.mapping_guards.drain(..) {
            owner.guard.store(false, Ordering::Release);
        }
    }

    fn broadcast_controller_sync_quick(
        &mut self,
        sync: &QuickEnvelope,
        sender: &Entity<ProductWorkspace>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if !sync.is_fresh(tick()?) {
            return Ok(());
        }
        self.register_controller_chroma_host();
        let source_effect = serde_json::to_value(sync)?;
        self.host_storage.call(HostStorageCall {
            store: HostStoreKind::Window,
            sender_id: HOST_VIEW,
            action: "setWindowStorageItem".into(),
            payload: json!({"key":SYNC_KEY,"value":serde_json::to_string(sync)?}),
            target_url_array: Vec::new(),
        })?;
        let workspaces = self.devices.clone();
        for workspace in workspaces {
            if workspace == *sender {
                continue;
            }
            let result = self.main_window.update(cx, |_, window, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace.apply_controller_sync_quick(&source_effect, window, cx)
                })
            });
            result
                .context("The current Synapse window closed during quick-effect synchronization")?
                .map_err(anyhow::Error::msg)?;
        }
        Ok(())
    }

    fn finish_controller_chroma(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &ControllerChromaRequested,
        result: Result<Option<ControllerChromaObservation>, String>,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let _ = handle.update(cx, |_, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_controller_chroma(request, result, window, cx)
            })
        });
    }

    fn persist_controller_chroma(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerChromaRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(error) = self.storage_error.clone() {
            self.finish_controller_chroma(
                &workspace,
                &request,
                Err(error),
                window.window_handle(),
                cx,
            );
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let profile_id = workspace.read(cx).device(cx).active_profile.clone();
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
            Some(ProductSave::ControllerChroma(ControllerChromaSave {
                workspace,
                identity,
                profile_id,
                request,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_controller_chroma_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::ControllerChroma(chroma)) = &save.product else {
            return Ok(());
        };
        ensure!(
            self.devices.contains(&chroma.workspace)
                && chroma.workspace.read(cx).identity(cx) == chroma.identity
                && chroma
                    .workspace
                    .read(cx)
                    .accepts_controller_chroma(&chroma.request, cx),
            "Chroma profile owner changed before persistence"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).saved_snapshot(cx))
            .collect();
        let position = self
            .devices
            .iter()
            .position(|workspace| workspace.read(cx).identity(cx) == chroma.identity)
            .context("Chroma device was removed before persistence")?;
        let device = save
            .file
            .devices
            .get_mut(position)
            .context("Chroma device snapshot is missing")?;
        let profile = device
            .profiles
            .iter_mut()
            .find(|profile| profile.id == chroma.profile_id)
            .context("Chroma profile was removed before persistence")?;
        let source = profile
            .source_settings
            .as_mut()
            .context("Chroma source settings are missing")?;
        rebase_profile_settings(source, chroma.request.command())?;
        Ok(())
    }

    pub(super) fn finish_controller_chroma_save(
        &mut self,
        save: ControllerChromaSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let current = self.devices.contains(&save.workspace)
            && save.workspace.read(cx).identity(cx) == save.identity;
        if succeeded && current {
            self.refresh_controller_power_configuration(cx);
        }
        let result = if !succeeded {
            Err("Chroma local profile persistence did not complete".into())
        } else if current {
            Ok(None)
        } else {
            Err("Chroma owner changed after local persistence".into())
        };
        self.finish_controller_chroma(&save.workspace, &save.request, result, window, cx);
    }
}

fn rebase_profile_settings(
    source: &mut Value,
    command: &ControllerChromaCommand,
) -> anyhow::Result<()> {
    let profile = source
        .get_mut("profile")
        .and_then(Value::as_object_mut)
        .context("Chroma profile is not an object")?;
    match command {
        ControllerChromaCommand::SwitchOffLighting(setting) => {
            let policy = profile
                .get_mut("switchOffLighting")
                .and_then(Value::as_object_mut)
                .context("Switch-off-lighting settings are missing")?;
            policy.insert("isDisplayOn".into(), setting.display_off().into());
            policy.insert("isIdleEnabled".into(), setting.idle_enabled().into());
            policy.insert("idleMinutes".into(), setting.idle_minutes().into());
        }
        ControllerChromaCommand::CustomColors(colors) => {
            let quick = profile
                .get_mut("quickEffects")
                .and_then(Value::as_object_mut)
                .context("Quick-effect settings are missing")?;
            quick.insert("customColors".into(), serde_json::to_value(colors)?);
        }
        _ => anyhow::bail!("Chroma command is not an active-profile persistence operation"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_pages::features::gamepad_products::ControllerSwitchOffLighting;
    #[core::prelude::v1::test]
    fn chroma_configuration_commits_reject_late_local_reads_and_ignore_request_admission() {
        let directory =
            std::env::temp_dir().join(format!("razer-shared-chroma-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("configuration.json");
        let source = directory.join("source.json");
        let missing = chroma_profiles::load(&path, &source).unwrap();
        let mut state = State::default();
        let started = state.configuration_revision;
        state.revision = 7;
        assert_eq!(state.configuration_revision, started);
        let first =
            chroma_profiles::update(&path, &source, Change::Toggle(true), &AtomicBool::new(true))
                .unwrap();
        state.retain_configuration(&first, None);
        assert_eq!(state.retain_configuration(&missing, Some(started)), first);
        let admitted = state.configuration_revision;
        state.revision = 8;
        let second = chroma_profiles::update(
            &path,
            &source,
            Change::Toggle(false),
            &AtomicBool::new(true),
        )
        .unwrap();
        state.retain_configuration(&second, None);
        assert_eq!(state.retain_configuration(&first, Some(admitted)), second);
        assert_eq!(state.retain_configuration(&first, None), second);
        assert_eq!(state.last_snapshot.as_ref(), Some(&second));
        for file in std::fs::read_dir(&directory).unwrap() {
            std::fs::remove_file(file.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(&directory).unwrap();
    }
    #[core::prelude::v1::test]
    fn sync_direction_is_nested_and_source_freshness_is_signed() {
        let envelope=QuickEnvelope::from_caller(&json!({"selectedEffectId":4,"effectSettings":{"effectId":4,"direction":2},"indexDirection":1,"numberOfWaveDirection":2}),20).unwrap();
        assert_eq!(
            envelope.effect_settings.get("indexDirection"),
            Some(&json!(1))
        );
        assert!(!envelope.effect_settings.contains_key("totalIdxDirection"));
        assert!(envelope.is_fresh(10));
        assert!(envelope.is_fresh(100_000_019));
        assert!(!envelope.is_fresh(100_000_020));
        assert!(
            QuickEnvelope::from_caller(
                &json!({"selectedEffectId":3,"effectSettings":{"effectId":4}}),
                20
            )
            .is_err()
        );
    }
    #[core::prelude::v1::test]
    fn profile_rebase_preserves_shutdown_later_effects_and_color_metadata() {
        let mut source = json!({"profile":{"switchOffLighting":{"isShutdown":true,"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":1,"sourceExtension":7},"quickEffects":{"selectedEffectId":4,"selectedEffectSetting":{"direction":1},"customColors":[{"color":"#112233","picked":true,"extra":"retained"}]}},"other":true});
        rebase_profile_settings(
            &mut source,
            &ControllerChromaCommand::SwitchOffLighting(
                ControllerSwitchOffLighting::new(true, true, 5).unwrap(),
            ),
        )
        .unwrap();
        assert_eq!(source["profile"]["switchOffLighting"]["isShutdown"], true);
        assert_eq!(source["profile"]["switchOffLighting"]["sourceExtension"], 7);
        let mut colors: razer_model::preferences::ProfileCustomColors =
            serde_json::from_value(source["profile"]["quickEffects"]["customColors"].clone())
                .unwrap();
        colors
            .apply_edit(razer_model::preferences::CustomColorEdit::Set {
                slot: 0,
                color: [17, 34, 51],
            })
            .unwrap();
        rebase_profile_settings(&mut source, &ControllerChromaCommand::CustomColors(colors))
            .unwrap();
        assert_eq!(
            source["profile"]["quickEffects"]["customColors"][0]["extra"],
            "retained"
        );
        assert_eq!(
            source["profile"]["quickEffects"]["selectedEffectSetting"]["direction"],
            1
        );
    }
}
