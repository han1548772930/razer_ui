//! Retained local game observer -> current host memory store -> keyboard
//! mapping/indicator publication. No UI draft or device receipt is synthesized.
use super::AppShell;
use gpui_kit::*;
use razer_platform::game_monitor::{
    GameMonitor, GameObservation, GameUpdate, STORAGE_KEY, running_count_from_memory,
};
use razer_storage::linked_profiles::{Association, Effect, RunningGameDetail, Runtime};
use razer_storage::{
    game_catalog::GameSignature,
    host::{HostStorageCall, HostStorageView, HostStoreKind},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[path = "game_monitor_lifecycle.rs"]
mod lifecycle;
#[path = "game_monitor_linked.rs"]
mod linked;
use lifecycle::{Input, ProfileQueue};

const OBSERVER_VIEW: u64 = 0xffff_0200;
const OBSERVER_URL: &str = "razer-local:game-observer";
const FOREGROUND_KEY: &str = "gms-foregroundWindow";

struct ProfileOwner {
    workspace: WeakEntity<razer_pages::features::ProductWorkspace>,
    identity: String,
    product_id: u32,
    epoch: u64,
    valid: Arc<AtomicBool>,
    runtime: Option<Runtime>,
    base: String,
    loading: bool,
    retry_at: u64,
    load_error: Option<String>,
    initialized: bool,
    queue: ProfileQueue,
}
impl Drop for ProfileOwner {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct State {
    monitor: Option<GameMonitor>,
    pump: Option<Task<()>>,
    running_games: Option<usize>,
    linked: Vec<(String, String)>,
    diagnostic: Option<String>,
    linked_worker: Option<linked::Worker>,
    linked_replies: Option<Receiver<linked::Reply>>,
    profiles: BTreeMap<EntityId, ProfileOwner>,
    profile_epoch: u64,
    snapshot: Option<GameUpdate>,
    foreground_requested: bool,
    exit_prepared: bool,
}
impl State {
    pub(super) fn running_games(&self) -> Option<usize> {
        self.running_games
    }
}

impl AppShell {
    pub(super) fn prepare_keyboard_game_exit(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.game_observer.exit_prepared, true) {
            return;
        }
        let mut selections = Vec::new();
        for owner in self.game_observer.profiles.values_mut() {
            for guid in owner.queue.prepare_exit() {
                Self::retain_keyboard_exit_base(owner, &guid);
                selections.push((owner.product_id, guid));
            }
        }
        for (product_id, guid) in selections {
            self.persist_keyboard_linked_base(product_id, guid, cx);
        }
        if let Some(worker) = &self.game_observer.linked_worker {
            if let Err(error) = worker.send(linked::Command::Foreground(false)) {
                self.game_link_diagnostic(format!("{error:#}"), cx);
                self.cancel_pending_exit(cx);
            }
        }
        self.game_observer.foreground_requested = false;
    }

    pub(super) fn keyboard_game_exit_pending(&self) -> bool {
        self.game_observer
            .linked_worker
            .as_ref()
            .is_some_and(linked::Worker::is_pending)
            || self
                .game_observer
                .profiles
                .values()
                .any(|owner| owner.queue.saving)
    }

    pub(super) fn cancel_keyboard_game_exit(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.game_observer.exit_prepared) {
            return;
        }
        for owner in self.game_observer.profiles.values_mut() {
            owner.queue.cancel_exit();
        }
        let count = self.game_observer.running_games;
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_running_games(count, cx)
            });
        }
        let requested = !self.game_observer.profiles.is_empty();
        if let Some(worker) = &self.game_observer.linked_worker {
            match worker.send(linked::Command::Foreground(requested)) {
                Ok(()) => self.game_observer.foreground_requested = requested,
                Err(error) => self.game_link_diagnostic(format!("{error:#}"), cx),
            }
        }
    }

    fn retain_keyboard_exit_base(owner: &mut ProfileOwner, guid: &str) {
        owner.base = guid.to_owned();
        if let Some(runtime) = &mut owner.runtime {
            // The worker will commit this base against its actual latest file.
            let _ = runtime.select_base(guid);
        }
        owner.queue.rebase_pending_stacks(guid);
    }

    fn persist_keyboard_linked_base(
        &mut self,
        product_id: u32,
        guid: String,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .game_observer
            .linked_worker
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Linked-profile storage worker is unavailable"))
            .and_then(|worker| worker.send(linked::Command::SelectBase { product_id, guid }));
        if let Err(error) = result {
            self.game_link_diagnostic(format!("{error:#}"), cx);
            if self.game_observer.exit_prepared {
                self.cancel_pending_exit(cx);
            }
        }
    }

    pub(super) fn install_game_monitor(&mut self, cx: &mut Context<Self>) {
        self.host_storage.register_view(HostStorageView {
            id: OBSERVER_VIEW,
            url: OBSERVER_URL.into(),
            remote: false,
            destroyed: false,
            crashed: false,
            disposed: false,
            tracked_url: true,
        });
        match linked::Worker::start(OBSERVER_URL) {
            Ok((worker, replies)) => {
                self.game_observer.linked_worker = Some(worker);
                self.game_observer.linked_replies = Some(replies);
            }
            Err(error) => self.game_link_diagnostic(format!("{error:#}"), cx),
        }
        let mut observations = match GameMonitor::start() {
            Ok((monitor, observations)) => {
                self.game_observer.monitor = Some(monitor);
                self.sync_game_links(cx);
                Some(observations)
            }
            Err(error) => {
                self.fail_game_observation(format!("{error:#}"), cx);
                None
            }
        };
        let handle = self.main_window;
        self.game_observer.pump = Some(cx.spawn(async move |shell, cx| {
            let mut retry_at = now_millis().saturating_add(1_000);
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let mut pending = Vec::new();
                if let Some(observations) = &observations {
                    while let Ok(observation) = observations.try_recv() {
                        pending.push(observation);
                    }
                }
                let closed = observations
                    .as_ref()
                    .is_some_and(|events| events.is_closed());
                let alive = handle
                    .update(cx, |_, window, cx| {
                        shell.update(cx, |shell, cx| {
                            for observation in pending {
                                match observation {
                                    GameObservation::Updated(update) => {
                                        if let Err(error) =
                                            shell.publish_game_observation(&update, window, cx)
                                        {
                                            shell.fail_game_observation(format!("{error:#}"), cx);
                                        }
                                    }
                                    GameObservation::Failed(error) => {
                                        shell.fail_game_observation(error, cx)
                                    }
                                }
                            }
                            if closed {
                                shell.fail_game_observation("本地游戏观察线程已退出".into(), cx);
                                observations = None;
                                shell.game_observer.monitor = None;
                                shell.game_observer.linked.clear();
                                retry_at = now_millis().saturating_add(1_000);
                            }
                            #[cfg(windows)]
                            if !shell.game_observer.exit_prepared
                                && observations.is_none()
                                && now_millis() >= retry_at
                            {
                                retry_at = now_millis().saturating_add(1_000);
                                match GameMonitor::start() {
                                    Ok((monitor, events)) => {
                                        shell.game_observer.monitor = Some(monitor);
                                        observations = Some(events);
                                    }
                                    Err(error) => {
                                        shell.fail_game_observation(format!("{error:#}"), cx)
                                    }
                                }
                            }
                            if !shell.game_observer.exit_prepared
                                && shell.game_observer.monitor.is_some()
                            {
                                shell.sync_game_links(cx);
                            }
                            shell.sync_keyboard_game_owners(window, cx);
                            shell.poll_keyboard_game_links(window, cx);
                            shell.sync_keyboard_indicator(cx);
                            shell.sync_keyboard_latest_restore(window, cx);
                            shell.sync_keyboard_legacy_calibration(window, cx);
                            if shell.game_observer.exit_prepared
                                && shell.close_requested
                                && shell.save_task.is_none()
                                && !shell.keyboard_game_exit_pending()
                            {
                                shell.continue_save_queue(true, cx);
                            }
                            true
                        })
                    })
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        }));
        self.register_exit_cleanup(|shell, cx| {
            shell.game_observer.pump = None;
            let worker = shell.game_observer.monitor.take().and_then(GameMonitor::retire);
            shell.game_observer.profiles.clear();
            let linked_worker = shell.game_observer.linked_worker.take().and_then(linked::Worker::retire);
            shell.game_observer.linked_replies = None;
            shell.host_storage.close_view(OBSERVER_VIEW);
            let cleanup = cx.background_executor().spawn(async move {
                if let Some(worker) = worker {
                    if worker.join().is_err() { eprintln!("[local game observation] worker panicked during shutdown"); }
                }
                if let Some(worker) = linked_worker {
                    if worker.join().is_err() { eprintln!("[local game observation] linked-profile worker panicked during shutdown"); }
                }
            });
            async move { cleanup.await }
        });
    }

    fn game_memory_call(&mut self, action: &str, value: Option<Value>) -> anyhow::Result<Value> {
        self.game_memory_key_call(STORAGE_KEY, action, value)
    }
    fn game_memory_key_call(
        &mut self,
        key: &str,
        action: &str,
        value: Option<Value>,
    ) -> anyhow::Result<Value> {
        let mut payload = json!({"key":key});
        if let Some(value) = value {
            payload["value"] = value;
        }
        self.host_storage
            .call(HostStorageCall {
                store: HostStoreKind::Memory,
                sender_id: OBSERVER_VIEW,
                action: action.into(),
                payload,
                target_url_array: vec![],
            })
            .map(|reply| reply.value)
    }

    fn publish_game_observation(
        &mut self,
        update: &GameUpdate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        self.game_observer.snapshot = Some(update.clone());
        self.sync_keyboard_game_owners(window, cx);
        for message in update.messages() {
            self.game_memory_call(
                "setMemoryStorageItem",
                Some(Value::String(message.to_string())),
            )?;
            if let Some(started) = match message["action"].as_str() {
                Some("GMS_onGameStartedEvent") => Some(true),
                Some("GMS_onGameStoppedEvent") => Some(false),
                _ => None,
            } {
                for workspace in &self.devices {
                    workspace.update(cx, |workspace, cx| workspace.observe_polling_game_event(started, cx));
                }
            }
            let Some(game) = message["payload"]["entry_guid"].as_str() else {
                continue;
            };
            for owner in self.game_observer.profiles.values_mut() {
                match message["action"].as_str() {
                    Some("GMS_onGameStartedEvent") => {
                        owner.queue.inputs.push_back(Input::Started(game.into()))
                    }
                    Some("GMS_onGameStoppedEvent") => {
                        owner.queue.inputs.push_back(Input::Stopped(game.into()))
                    }
                    _ => {}
                }
            }
        }
        let reply = self.game_memory_call("getMemoryStorageItem", None)?;
        let count = running_count_from_memory(&reply);
        // Original empty startup has no producer message. A later stop stores
        // an observed empty array. Both follow the source's inactive result.
        anyhow::ensure!(
            count == Some(update.count()) || (count.is_none() && update.count() == 0),
            "Local game memory publication disagrees with the observed snapshot"
        );
        self.set_running_game_count(count, cx);
        let diagnostics =
            (!update.diagnostics().is_empty()).then(|| update.diagnostics().join("; "));
        if self.game_observer.diagnostic != diagnostics {
            if let Some(diagnostic) = &diagnostics {
                eprintln!("[local game observation] {diagnostic}");
            }
            self.game_observer.diagnostic = diagnostics;
        }
        Ok(())
    }

    fn set_running_game_count(&mut self, count: Option<usize>, cx: &mut Context<Self>) {
        if self.game_observer.running_games == count {
            return;
        }
        self.game_observer.running_games = count;
        if self.game_observer.exit_prepared {
            return;
        }
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_running_games(count, cx)
            });
        }
    }

    fn fail_game_observation(&mut self, error: String, cx: &mut Context<Self>) {
        // Losing access is not a successful empty read or a stopped-game event.
        if let Err(storage_error) = self.game_memory_call("removeMemoryStorageItem", None) {
            eprintln!("[local game observation] clear unavailable observation: {storage_error:#}");
        }
        self.set_running_game_count(None, cx);
        self.game_observer.snapshot = None;
        if self.game_observer.diagnostic.as_ref() != Some(&error) {
            eprintln!("[local game observation] {error}");
            self.status = format!("本地游戏观察：{error}");
            self.game_observer.diagnostic = Some(error);
            cx.notify();
        }
    }

    pub(super) fn select_keyboard_linked_base(
        &mut self,
        workspace: Entity<razer_pages::features::ProductWorkspace>,
        guid: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace) || guid.is_empty() {
            return;
        }
        if let Some(owner) = self.game_observer.profiles.get_mut(&workspace.entity_id()) {
            let commit = owner.queue.select_base(guid.into());
            owner.base = guid.into();
            if let Some(guid) = commit {
                Self::retain_keyboard_exit_base(owner, &guid);
                let product_id = owner.product_id;
                self.persist_keyboard_linked_base(product_id, guid, cx);
            }
        } else {
            let product_id = workspace.read(cx).device(cx).product_id;
            if !matches!(product_id, 515 | 679 | 740 | 746 | 756) {
                return;
            }
            self.persist_keyboard_linked_base(product_id, guid.into(), cx);
        }
        cx.notify();
    }

    fn game_link_diagnostic(&mut self, error: String, cx: &mut Context<Self>) {
        eprintln!("[keyboard linked profile] {error}");
        self.status = format!("键盘游戏配置：{error}");
        cx.notify();
    }

    fn sync_keyboard_game_owners(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.game_observer.exit_prepared {
            return;
        }
        use razer_model::model::DeviceConnectionObservation;
        let candidates = self
            .devices
            .iter()
            .filter_map(|workspace| {
                let view = workspace.read(cx);
                let device = view.device(cx);
                if !matches!(device.product_id, 515 | 679 | 740 | 746 | 756)
                    || !matches!(
                        device.dashboard.connection_observation,
                        Some(
                            DeviceConnectionObservation::UsbPresent
                                | DeviceConnectionObservation::HidPresent
                        )
                    )
                {
                    return None;
                }
                let guid = active_guid(device)?;
                Some((
                    workspace.clone(),
                    view.identity(cx),
                    device.product_id,
                    guid,
                ))
            })
            .collect::<Vec<_>>();
        let stale = self
            .game_observer
            .profiles
            .iter()
            .filter_map(|(id, owner)| {
                (!candidates.iter().any(|(workspace, identity, _, _)| {
                    workspace.entity_id() == *id && *identity == owner.identity
                }))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in stale {
            if let Some(owner) = self.game_observer.profiles.remove(&id) {
                if let Some(guid) = owner
                    .queue
                    .inputs
                    .iter()
                    .rev()
                    .find_map(|input| match input {
                        Input::Base(guid) => Some(guid.clone()),
                        _ => None,
                    })
                {
                    let result = self
                        .game_observer
                        .linked_worker
                        .as_ref()
                        .ok_or_else(|| {
                            anyhow::anyhow!("Linked-profile storage worker is unavailable")
                        })
                        .and_then(|worker| {
                            worker.send(linked::Command::SelectBase {
                                product_id: owner.product_id,
                                guid,
                            })
                        });
                    if let Err(error) = result {
                        self.game_link_diagnostic(format!("{error:#}"), cx);
                    }
                }
            }
        }
        for (workspace, identity, product_id, base) in candidates {
            let id = workspace.entity_id();
            if self.game_observer.profiles.contains_key(&id) {
                continue;
            }
            self.game_observer.profile_epoch = self.game_observer.profile_epoch.wrapping_add(1);
            let epoch = self.game_observer.profile_epoch;
            let valid = Arc::new(AtomicBool::new(true));
            let command = linked::Command::Load {
                owner: id,
                epoch,
                product_id,
                base: base.clone(),
                valid: valid.clone(),
            };
            let result = self
                .game_observer
                .linked_worker
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Linked-profile storage worker is unavailable"))
                .and_then(|worker| worker.send(command));
            match result {
                Ok(()) => {
                    self.game_observer.profiles.insert(
                        id,
                        ProfileOwner {
                            workspace: workspace.downgrade(),
                            identity,
                            product_id,
                            epoch,
                            valid,
                            runtime: None,
                            base,
                            loading: true,
                            retry_at: 0,
                            load_error: None,
                            initialized: false,
                            queue: ProfileQueue::default(),
                        },
                    );
                }
                Err(error) => self.game_link_diagnostic(format!("{error:#}"), cx),
            }
        }
        let requested = !self.game_observer.profiles.is_empty();
        if requested != self.game_observer.foreground_requested {
            let result = self
                .game_observer
                .linked_worker
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Foreground IPC worker is unavailable"))
                .and_then(|worker| worker.send(linked::Command::Foreground(requested)));
            match result {
                Ok(()) => self.game_observer.foreground_requested = requested,
                Err(error) => self.game_link_diagnostic(format!("{error:#}"), cx),
            }
        }
    }

    fn poll_keyboard_game_links(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let replies = self
            .game_observer
            .linked_replies
            .as_ref()
            .map(|replies| replies.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for reply in replies {
            match reply {
                linked::Reply::Loaded {
                    owner,
                    epoch,
                    result,
                    _pending,
                } => {
                    if let Some(owner) = self
                        .game_observer
                        .profiles
                        .get_mut(&owner)
                        .filter(|owner| owner.epoch == epoch)
                    {
                        owner.loading = false;
                        match result {
                            Ok(stack) => {
                                owner.runtime =
                                    Some(Runtime::new(owner.queue.reconcile_loaded_stack(stack)));
                                owner.load_error = None;
                            }
                            Err(error) => {
                                owner.retry_at = now_millis().saturating_add(1_000);
                                if owner.load_error.as_ref() != Some(&error) {
                                    owner.load_error = Some(error.clone());
                                    self.game_link_diagnostic(error, cx);
                                }
                            }
                        }
                    }
                }
                linked::Reply::Saved {
                    owner,
                    epoch,
                    result,
                    _pending,
                } => {
                    if let Some(owner) = self
                        .game_observer
                        .profiles
                        .get_mut(&owner)
                        .filter(|owner| owner.epoch == epoch)
                    {
                        owner.queue.acknowledge_save(&result);
                        if let Err(error) = result {
                            // UI selection is optimistic. A failed local commit
                            // prevents its following task and does not roll it back.
                            self.game_link_diagnostic(error, cx);
                            if self.game_observer.exit_prepared {
                                self.cancel_pending_exit(cx);
                            }
                        }
                    }
                }
                linked::Reply::Foreground(events) => {
                    for event in events {
                        if event["event"] != "foregroundWindow" {
                            continue;
                        }
                        let Some(name) = event["data"]["name"].as_str() else {
                            continue;
                        };
                        let Some(snapshot) = &self.game_observer.snapshot else {
                            continue;
                        };
                        let details = source_running_details(snapshot);
                        let message = json!({"action":"foregroundWindow","payload":{
                            "exe_name":name,
                            "runningGamesDetail":details,
                        }});
                        if !self.game_observer.exit_prepared
                            && let Err(error) = self.game_memory_key_call(
                                FOREGROUND_KEY,
                                "setMemoryStorageItem",
                                Some(Value::String(message.to_string())),
                            )
                        {
                            self.game_link_diagnostic(format!("{error:#}"), cx);
                            continue;
                        }
                        for owner in self.game_observer.profiles.values_mut() {
                            owner.queue.inputs.push_back(Input::Foreground(name.into()));
                        }
                    }
                }
                linked::Reply::Diagnostic(error) => self.game_link_diagnostic(error, cx),
                linked::Reply::BaseSelected { result, _pending }
                | linked::Reply::ForegroundStopped { result, _pending } => {
                    if let Err(error) = result {
                        self.game_link_diagnostic(error, cx);
                        if self.game_observer.exit_prepared {
                            self.cancel_pending_exit(cx);
                        }
                    }
                }
            }
        }
        let ids = self
            .game_observer
            .profiles
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for id in ids {
            let Some(mut owner) = self.game_observer.profiles.remove(&id) else {
                continue;
            };
            if !owner.queue.automatic() {
                self.game_observer.profiles.insert(id, owner);
                continue;
            }
            let Some(workspace) = owner
                .workspace
                .upgrade()
                .filter(|workspace| self.devices.contains(workspace))
            else {
                continue;
            };
            if owner.runtime.is_none() && !owner.loading && now_millis() >= owner.retry_at {
                let result = self
                    .game_observer
                    .linked_worker
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Linked-profile storage worker is unavailable"))
                    .and_then(|worker| {
                        worker.send(linked::Command::Load {
                            owner: id,
                            epoch: owner.epoch,
                            product_id: owner.product_id,
                            base: owner.base.clone(),
                            valid: owner.valid.clone(),
                        })
                    });
                match result {
                    Ok(()) => owner.loading = true,
                    Err(error) => {
                        owner.retry_at = now_millis().saturating_add(1_000);
                        let error = format!("{error:#}");
                        if owner.load_error.as_ref() != Some(&error) {
                            owner.load_error = Some(error.clone());
                            self.game_link_diagnostic(error, cx);
                        }
                    }
                }
            }
            // User choices made while the initial store read was in flight
            // must become the base before initialization can restore anything.
            if !owner.initialized
                && let Some(runtime) = &mut owner.runtime
            {
                while matches!(owner.queue.inputs.front(), Some(Input::Base(_))) {
                    if let Some(Input::Base(guid)) = owner.queue.inputs.pop_front() {
                        owner.queue.effects.extend(runtime.select_base(guid));
                    }
                }
            }
            if !owner.initialized && !owner.queue.effects.is_empty() {
                self.flush_keyboard_link_effects(&workspace, &mut owner, window, cx);
            }
            if let (Some(snapshot), Some(runtime)) =
                (&self.game_observer.snapshot, &mut owner.runtime)
            {
                runtime.configure(
                    associations(workspace.read(cx).device(cx), snapshot),
                    snapshot.available_game_ids().clone(),
                );
                runtime.observe_running(
                    snapshot
                        .running_games()
                        .iter()
                        .map(|game| game.entry_guid().to_owned())
                        .collect(),
                );
                if !owner.initialized {
                    owner.queue.effects.extend(runtime.initialize(now_millis()));
                    owner.initialized = true;
                }
            }
            if owner.initialized && !owner.queue.saving {
                self.flush_keyboard_link_effects(&workspace, &mut owner, window, cx);
                while !owner.queue.saving && owner.queue.effects.is_empty() {
                    let Some(input) = owner.queue.inputs.pop_front() else {
                        break;
                    };
                    let profile = active_guid(workspace.read(cx).device(cx));
                    let runtime = owner
                        .runtime
                        .as_mut()
                        .expect("initialized linked-profile owner");
                    let tick = now_millis();
                    let effects = match input {
                        Input::Base(guid) => runtime.select_base(guid),
                        Input::Started(game) => {
                            runtime.game_started(&game, profile.as_deref(), tick)
                        }
                        Input::Stopped(game) => {
                            runtime.game_stopped(&game, false, profile.as_deref(), tick)
                        }
                        Input::Foreground(executable) => {
                            let details = self
                                .game_observer
                                .snapshot
                                .as_ref()
                                .map(running_details)
                                .unwrap_or_default();
                            runtime.foreground(&executable, &details, profile.as_deref(), tick)
                        }
                    };
                    owner.queue.effects.extend(effects);
                    self.flush_keyboard_link_effects(&workspace, &mut owner, window, cx);
                }
                if !owner.queue.saving
                    && owner.queue.effects.is_empty()
                    && self.game_observer.snapshot.is_some()
                {
                    let profile = active_guid(workspace.read(cx).device(cx));
                    owner.queue.effects.extend(
                        owner
                            .runtime
                            .as_mut()
                            .expect("initialized owner")
                            .tick(now_millis(), profile.as_deref()),
                    );
                    self.flush_keyboard_link_effects(&workspace, &mut owner, window, cx);
                }
            }
            self.game_observer.profiles.insert(id, owner);
        }
    }

    fn flush_keyboard_link_effects(
        &mut self,
        workspace: &Entity<razer_pages::features::ProductWorkspace>,
        owner: &mut ProfileOwner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        while let Some(effect) = owner.queue.next_effect() {
            match effect {
                Effect::RuntimeProfile(guid) => {
                    if !workspace.update(cx, |workspace, cx| {
                        workspace.observe_keyboard_runtime_profile(&guid, window, cx)
                    }) {
                        self.game_link_diagnostic(format!("本地配置 {guid} 已不存在"), cx);
                    }
                }
                Effect::SwitchProfile(guid) => {
                    if !workspace.update(cx, |workspace, cx| {
                        workspace.select_keyboard_runtime_profile(&guid, window, cx)
                    }) {
                        self.game_link_diagnostic(format!("无法切换到本地配置 {guid}"), cx);
                    }
                }
                Effect::GameActive(active) => workspace.update(cx, |workspace, cx| {
                    workspace.observe_keyboard_linked_game_active(active, cx)
                }),
                Effect::SaveStack(stack) => {
                    let command = linked::Command::Save {
                        owner: workspace.entity_id(),
                        epoch: owner.epoch,
                        product_id: owner.product_id,
                        stack,
                        valid: owner.valid.clone(),
                    };
                    match self
                        .game_observer
                        .linked_worker
                        .as_ref()
                        .ok_or_else(|| {
                            anyhow::anyhow!("Linked-profile storage worker is unavailable")
                        })
                        .and_then(|worker| worker.send(command))
                    {
                        Ok(()) => owner.queue.saving = true,
                        Err(error) => {
                            owner.queue.effects.clear();
                            self.game_link_diagnostic(format!("{error:#}"), cx);
                            break;
                        }
                    }
                }
            }
        }
    }

    fn sync_game_links(&mut self, cx: &mut Context<Self>) {
        let mut linked = BTreeMap::new();
        for workspace in &self.devices {
            let workspace = workspace.read(cx);
            for profile in &workspace.device(cx).profiles {
                if let Some(settings) = &profile.settings {
                    for game in &settings.linked_games {
                        if !game.executable.is_empty() {
                            linked
                                .entry(game.executable.replace('/', "\\").to_uppercase())
                                .or_insert_with(|| (game.name.clone(), game.executable.clone()));
                        }
                    }
                }
            }
        }
        let linked = linked.into_values().collect::<Vec<_>>();
        if linked == self.game_observer.linked {
            return;
        }
        let signatures = linked
            .iter()
            .map(|(name, executable)| GameSignature::linked(name, executable))
            .collect::<anyhow::Result<Vec<_>>>();
        let result = signatures.and_then(|signatures| {
            self.game_observer
                .monitor
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("本地游戏观察线程不可用"))?
                .set_linked(signatures)
        });
        match result {
            Ok(()) => self.game_observer.linked = linked,
            Err(error) => self.fail_game_observation(format!("{error:#}"), cx),
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}
fn active_guid(device: &razer_model::model::Device) -> Option<String> {
    let profile = device
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile)?;
    Some(
        profile
            .source_settings
            .as_ref()
            .and_then(|source| source["guid"].as_str())
            .filter(|guid| !guid.is_empty())
            .unwrap_or(&profile.guid)
            .to_owned(),
    )
}
fn associations(device: &razer_model::model::Device, snapshot: &GameUpdate) -> Vec<Association> {
    let mut links = Vec::new();
    for profile in &device.profiles {
        let Some(settings) = &profile.settings else {
            continue;
        };
        let guid = profile
            .source_settings
            .as_ref()
            .and_then(|source| source["guid"].as_str())
            .filter(|guid| !guid.is_empty())
            .unwrap_or(&profile.guid);
        for game in &settings.linked_games {
            let Some(identity) = snapshot.linked_identities().iter().find(|identity| {
                windows_path(identity.executable()) == windows_path(&game.executable)
            }) else {
                continue;
            };
            if !links
                .iter()
                .any(|link: &Association| link.game_id() == identity.game_id())
            {
                links.push(Association::new(identity.game_id(), guid));
            }
        }
    }
    links
}
fn windows_path(path: &str) -> String {
    path.replace('/', "\\").to_uppercase()
}
fn source_running_details(snapshot: &GameUpdate) -> Vec<Value> {
    snapshot.running_games().iter().map(|game| json!({"entry_guid":game.entry_guid(),
        "exe_name":game.default_executables().into_iter().filter(|exe| !exe.is_empty()).collect::<Vec<_>>()
    })).collect()
}
fn running_details(snapshot: &GameUpdate) -> Vec<RunningGameDetail> {
    snapshot
        .running_games()
        .iter()
        .map(|game| {
            RunningGameDetail::new(
                game.entry_guid(),
                game.default_executables()
                    .into_iter()
                    .filter(|exe| !exe.is_empty())
                    .map(str::to_owned)
                    .collect(),
            )
        })
        .collect()
}
