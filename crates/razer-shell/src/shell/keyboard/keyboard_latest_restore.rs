//! Current756 retained restore owner. The UI owns the debounce/FIFO and
//! generation guard; one joined worker owns actual device/service lifetime.
use super::*;
use razer_device::keyboard_latest_restore::{Coordinator, Generation, STORAGE_FIELD, Work};
use razer_discovery::discovery::ObservedDevice;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[path = "keyboard_latest_restore_worker.rs"]
mod worker;

struct Request {
    document: Value,
    previous: Option<Value>,
    running_games: Option<usize>,
    full: bool,
    games_only: bool,
    ble_switch: Option<razer_device::keyboard_latest_restore::BleSwitch>,
    switch_input: Option<Value>,
    defer_ble_persistence: bool,
    intent: Option<(u64, Value)>,
}
enum Reply {
    Ready,
    Candidate {
        revision: u64,
        generation: Generation,
        document: Value,
        persist: bool,
        manual: bool,
    },
    Event(Value),
    Finished(u64, Generation, Result<(), String>),
    IntentFinished(u64, Result<(), String>),
    Diagnostic(String),
    Stopped,
}
struct Owner {
    identity: String,
    route: String,
    valid: Arc<AtomicBool>,
    commands: mpsc::Sender<Work<Request>>,
    replies: mpsc::Receiver<Reply>,
    coordinator: Coordinator<Request>,
    ready: bool,
    revision: u64,
    revision_signal: Arc<AtomicU64>,
    document: Option<Value>,
    source: Option<Value>,
    running_games: Option<usize>,
    brightness_repeats: BTreeMap<String, (Instant, Value)>,
    ble: bool,
    macro_exclusive: Arc<AtomicBool>,
    pending_intents: BTreeSet<u64>,
    cancelled_intents: Vec<u64>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.coordinator.invalidate();
        self.valid.store(false, Ordering::Release);
    }
}
impl Owner {
    fn enqueue(&mut self, document: Value, running_games: Option<usize>, games_only: bool) {
        let full = self
            .document
            .as_ref()
            .is_none_or(|old| old["activeProfile"] != document["activeProfile"]);
        if !games_only && self.document.as_ref() == Some(&document) {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.revision_signal.store(self.revision, Ordering::Release);
        let request = Request {
            previous: self.document.clone(),
            document: document.clone(),
            running_games,
            full,
            games_only,
            ble_switch: None,
            switch_input: None,
            defer_ble_persistence: false,
            intent: None,
        };
        self.running_games = running_games;
        self.document = Some(document);
        if full {
            self.cancelled_intents
                .extend(std::mem::take(&mut self.pending_intents));
            self.coordinator.request(
                self.revision,
                request,
                Instant::now(),
                Duration::from_millis(50),
            );
        } else {
            self.coordinator.enqueue_update(self.revision, request);
        }
    }
    fn switch(
        &mut self,
        target: razer_device::keyboard_latest_restore::BleSwitch,
        defer: bool,
        input: Option<Value>,
    ) -> anyhow::Result<()> {
        let document = self
            .document
            .clone()
            .ok_or_else(|| anyhow::anyhow!("BLE 配置切换尚未取得当前本地配置"))?;
        self.revision = self.revision.wrapping_add(1);
        self.revision_signal.store(self.revision, Ordering::Release);
        self.cancelled_intents
            .extend(std::mem::take(&mut self.pending_intents));
        let request = Request {
            previous: Some(document.clone()),
            document,
            running_games: self.running_games,
            full: true,
            games_only: false,
            ble_switch: Some(target),
            switch_input: input,
            defer_ble_persistence: defer,
            intent: None,
        };
        self.coordinator
            .request(self.revision, request, Instant::now(), Duration::ZERO);
        Ok(())
    }
}
#[derive(Default)]
pub(super) struct State {
    owners: BTreeMap<EntityId, Owner>,
    workers: Vec<JoinHandle<()>>,
    retry: BTreeMap<EntityId, Instant>,
}
fn source_signature(device: &Device) -> Value {
    let mut settings = device.source_device_settings.clone();
    if let Some(settings) = settings.as_mut().and_then(Value::as_object_mut) {
        settings.remove(STORAGE_FIELD);
    }
    serde_json::json!({"active":device.active_profile,"layout":device.layout_id,
        "profiles":device.profiles.iter().map(|profile|serde_json::json!({"id":profile.id,"guid":profile.guid,"name":profile.name,"source":profile.source_settings})).collect::<Vec<_>>(),
        "device":settings})
}
impl AppShell {
    pub(super) fn submit_keyboard_latest_intent(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &razer_pages::features::keyboard_products::KeyboardActuationRequested,
        cx: &mut Context<Self>,
    ) -> bool {
        if workspace.read(cx).device(cx).product_id != 756
            || !matches!(
                request.payload()["type"].as_str(),
                Some("ON_SYNC_GLOBAL_ACTUATION" | "ON_SYNC_GLOBAL_RAPID_TRIGGER")
            )
        {
            return false;
        }
        let result = (|| -> Result<(), String> {
            let mut device = workspace.read(cx).snapshot(cx);
            super::mouse_dpi_70::prepare_mapping_document(
                &mut device,
                &self.macro_library.read(cx).snapshot(),
            )
            .map_err(|error| format!("{error:#}"))?;
            let document = device
                .source_device_settings
                .as_ref()
                .and_then(|settings| settings.get(STORAGE_FIELD))
                .cloned()
                .ok_or_else(|| "全局行程同步没有当前配置文档".to_owned())?;
            let owner = self
                .keyboard_latest_restore
                .owners
                .get_mut(&workspace.entity_id())
                .ok_or_else(|| "全局行程同步没有当前设备连接".to_owned())?;
            owner.enqueue(document.clone(), self.game_observer.running_games(), false);
            owner.pending_intents.insert(request.generation());
            owner.coordinator.enqueue_update(
                owner.revision,
                Request {
                    previous: Some(document.clone()),
                    document,
                    running_games: owner.running_games,
                    full: false,
                    games_only: false,
                    ble_switch: None,
                    switch_input: None,
                    defer_ble_persistence: false,
                    intent: Some((request.generation(), request.payload().clone())),
                },
            );
            Ok(())
        })();
        if let Err(error) = result {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_actuation(request.generation(), Err(error), cx)
            });
        }
        true
    }
    pub(super) fn switch_keyboard_latest_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        guid: String,
        defer_persistence: bool,
        cx: &mut Context<Self>,
    ) {
        let mut diagnostic = None;
        if let Some(owner) = self
            .keyboard_latest_restore
            .owners
            .get_mut(&workspace.entity_id())
        {
            if owner.ble {
                if let Err(error) = owner.switch(
                    razer_device::keyboard_latest_restore::BleSwitch::Guid(guid),
                    defer_persistence,
                    None,
                ) {
                    diagnostic = Some(error.to_string());
                }
            } else {
                diagnostic = Some("配置切换请求的 BLE 连接已改变".to_owned());
            }
        } else {
            diagnostic = Some("BLE 配置切换尚无当前连接所有者".to_owned());
        }
        if let Some(error) = diagnostic {
            self.keyboard_latest_diagnostic(error, cx);
        }
    }
    /// Onboard slot selection keeps the source slot identity: the retained
    /// owner switches by slot id and the guid stays in the diagnostic.
    pub(super) fn switch_keyboard_latest_onboard_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        slot_id: u8,
        guid: String,
        cx: &mut Context<Self>,
    ) {
        let mut diagnostic = None;
        if let Some(owner) = self
            .keyboard_latest_restore
            .owners
            .get_mut(&workspace.entity_id())
        {
            if owner.ble {
                if let Err(error) = owner.switch(
                    razer_device::keyboard_latest_restore::BleSwitch::Slot(slot_id),
                    false,
                    None,
                ) {
                    diagnostic = Some(error.to_string());
                }
            } else {
                diagnostic = Some("板载配置切换请求的 BLE 连接已改变".to_owned());
            }
        } else {
            diagnostic = Some("板载配置切换尚无当前连接所有者".to_owned());
        }
        if let Some(error) = diagnostic {
            self.keyboard_latest_diagnostic(format!("{error}（槽位 {slot_id}，{guid}）"), cx);
        }
    }
    pub(super) fn install_keyboard_latest_restore_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|shell, cx| {
            shell.keyboard_latest_restore.owners.clear();
            let workers = std::mem::take(&mut shell.keyboard_latest_restore.workers);
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    if worker.join().is_err() {
                        eprintln!("[keyboard756] restore worker panicked during cleanup");
                    }
                }
            });
            async move { cleanup.await }
        }));
    }
    pub(super) fn publish_keyboard_latest_restore(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        document: Value,
        cx: &mut Context<Self>,
    ) {
        if document["productId"] != 756 || !self.devices.contains(&workspace) {
            return;
        }
        let running = self.game_observer.running_games();
        if let Some(owner) = self
            .keyboard_latest_restore
            .owners
            .get_mut(&workspace.entity_id())
        {
            owner.enqueue(document, running, false);
            owner.source = Some(source_signature(workspace.read(cx).device(cx)));
        }
    }
    fn keyboard_latest_diagnostic(&mut self, error: String, cx: &mut Context<Self>) {
        eprintln!("[keyboard756 restore] {error}");
        self.status = format!("键盘本地恢复：{error}");
        cx.notify();
    }
    pub(super) fn sync_keyboard_latest_restore(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let candidates = self
            .devices
            .iter()
            .filter_map(|workspace| {
                let view = workspace.read(cx);
                let device = view.device(cx);
                if device.product_id != 756 {
                    return None;
                }
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|observed| {
                        observed.matches(device)
                            && observed.peer_product_id().is_none()
                            && matches!(observed.physical_product_id(), 756 | 757 | 758)
                    })
                    .collect::<Vec<_>>();
                let [observed] = routes.as_slice() else { return None };
                Some((
                    workspace.clone(),
                    view.identity(cx),
                    format!(
                        "{}:{}",
                        observed.container(),
                        observed.physical_product_id()
                    ),
                    (**observed).clone(),
                ))
            })
            .collect::<Vec<_>>();
        let mut cancelled = self
            .keyboard_latest_restore
            .owners
            .iter()
            .filter(|(id, owner)| {
                !candidates.iter().any(|(workspace, identity, route, _)| {
                    workspace.entity_id() == **id
                        && owner.identity == *identity
                        && owner.route == *route
                })
            })
            .flat_map(|(id, owner)| {
                owner
                    .pending_intents
                    .iter()
                    .copied()
                    .chain(owner.cancelled_intents.iter().copied())
                    .map(move |generation| (*id, generation))
            })
            .collect::<Vec<_>>();
        self.keyboard_latest_restore.owners.retain(|id, owner| {
            candidates.iter().any(|(workspace, identity, route, _)| {
                workspace.entity_id() == *id && owner.identity == *identity && owner.route == *route
            })
        });
        let mut diagnostics = Vec::new();
        let exclusive = self.receiver_macro_exclusive(cx);
        for workspace in &self.devices {
            let ble = candidates.iter().any(|(candidate, _, _, observed)| {
                candidate == workspace && observed.physical_product_id() == 758
            });
            workspace.update(cx, |workspace, cx| {
                workspace.set_keyboard_latest_ble(ble, cx)
            });
        }
        for owner in self.keyboard_latest_restore.owners.values() {
            owner.macro_exclusive.store(exclusive, Ordering::Release);
        }
        for (workspace, identity, route, observed) in candidates {
            let id = workspace.entity_id();
            if self.keyboard_latest_restore.owners.contains_key(&id)
                || self
                    .keyboard_latest_restore
                    .retry
                    .get(&id)
                    .is_some_and(|retry| *retry > Instant::now())
            {
                continue;
            }
            let (commands, receiver) = mpsc::channel();
            let (sender, replies) = mpsc::channel();
            let valid = Arc::new(AtomicBool::new(true));
            let signal = valid.clone();
            let ble = observed.physical_product_id() == 758;
            let macro_exclusive = Arc::new(AtomicBool::new(exclusive));
            let worker_exclusive = macro_exclusive.clone();
            let revision_signal = Arc::new(AtomicU64::new(0));
            let worker_revision = revision_signal.clone();
            match thread::Builder::new()
                .name("keyboard756-restore".into())
                .spawn(move || worker::run(observed, signal, worker_exclusive, worker_revision, receiver, sender))
            {
                Ok(worker) => {
                    self.keyboard_latest_restore.workers.push(worker);
                    self.keyboard_latest_restore.owners.insert(
                        id,
                        Owner {
                            identity,
                            route,
                            valid,
                            commands,
                            replies,
                            coordinator: Coordinator::default(),
                            ready: false,
                            revision: 0,
                            revision_signal,
                            document: None,
                            source: None,
                            running_games: None,
                            brightness_repeats: BTreeMap::new(),
                            ble,
                            macro_exclusive,
                            pending_intents: BTreeSet::new(),
                            cancelled_intents: Vec::new(),
                        },
                    );
                }
                Err(error) => {
                    self.keyboard_latest_restore
                        .retry
                        .insert(id, Instant::now() + Duration::from_secs(1));
                    diagnostics.push(error.to_string());
                }
            }
        }
        let mut messages = Vec::new();
        for (id, owner) in &self.keyboard_latest_restore.owners {
            loop {
                match owner.replies.try_recv() {
                    Ok(reply) => {
                        let stopped = matches!(&reply, Reply::Stopped);
                        messages.push((*id, reply));
                        if stopped {
                            break;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        messages.push((*id, Reply::Stopped));
                        break;
                    }
                }
            }
        }
        for (id, reply) in messages {
            let workspace = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
                .cloned();
            match reply {
                Reply::Ready => {
                    if let Some(owner) = self.keyboard_latest_restore.owners.get_mut(&id) {
                        owner.ready = true;
                    }
                }
                Reply::Candidate {
                    revision,
                    generation,
                    document,
                    persist,
                    manual,
                } => {
                    if !generation.is_current()
                        || self
                            .keyboard_latest_restore
                            .owners
                            .get(&id)
                            .is_none_or(|owner| owner.revision != revision)
                    {
                        continue;
                    }
                    if let Some(workspace) = workspace {
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_latest_document(document.clone(), cx)
                        });
                        // Source BLE slot ACK/initial active-slot getter emits
                        // the profile selection action before local persistence.
                        let active = document["activeProfile"].as_str().unwrap_or_default();
                        let target = workspace
                            .read(cx)
                            .device(cx)
                            .profiles
                            .iter()
                            .find(|profile| profile.guid == active)
                            .map(|profile| profile.id.clone());
                        if let Some(target) = target {
                            if workspace.read(cx).device(cx).active_profile != target {
                                workspace.update(cx, |workspace, cx| {
                                    workspace
                                        .confirm_keyboard_latest_profile(active, manual, window, cx)
                                });
                            }
                        }
                        if let Some(owner) = self.keyboard_latest_restore.owners.get_mut(&id) {
                            owner.document = Some(document);
                            owner.source = Some(source_signature(workspace.read(cx).device(cx)));
                        }
                        if persist {
                            self.persist_legacy_keyboard_profile(workspace, window, cx);
                        }
                    }
                }
                Reply::Event(event) => {
                    if let Some(workspace) = workspace {
                        // The workspace owns the mounted mapping-event consumers.
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_latest_mapping_event(event.clone(), cx)
                        });
                        if let Some(output) = event["jsonEvent"]["output"]
                            .as_str()
                            .and_then(|text| serde_json::from_str::<Value>(text).ok())
                        {
                            if output["type"] == "switchProfile"
                                || output["type"] == "navigateProfile"
                            {
                                if let Some(owner) =
                                    self.keyboard_latest_restore.owners.get_mut(&id)
                                {
                                    {
                                        let target = if output["type"] == "switchProfile" {
                                            razer_device::keyboard_latest_restore::BleSwitch::Guid(
                                                output["guid"].as_str().unwrap_or("").into(),
                                            )
                                        } else {
                                            razer_device::keyboard_latest_restore::BleSwitch::Navigation(output["name"].as_str().unwrap_or("").into())
                                        };
                                        if let Err(error) = owner.switch(
                                            target,
                                            true,
                                            Some(event["jsonEvent"]["input"].clone()),
                                        ) {
                                            diagnostics.push(error.to_string());
                                        }
                                        continue;
                                    }
                                }
                            }
                            if output["type"] == "backlight" {
                                if exclusive {
                                    if let Some(owner) =
                                        self.keyboard_latest_restore.owners.get_mut(&id)
                                    {
                                        owner.brightness_repeats.clear();
                                    }
                                    continue;
                                }
                                if let Some(name) = output["name"].as_str() {
                                    if let Some(owner) =
                                        self.keyboard_latest_restore.owners.get_mut(&id)
                                    {
                                        if output["flag"] == 1 {
                                            owner.brightness_repeats.remove(name);
                                        } else if output["flag"] == 0 && name != "BrightnessToggle"
                                        {
                                            owner.brightness_repeats.insert(
                                                name.into(),
                                                (
                                                    Instant::now() + Duration::from_millis(800),
                                                    event.clone(),
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_latest_mapping_event(event, cx)
                        });
                    }
                }
                Reply::Finished(_id, generation, result) => {
                    if let Some(owner) = self.keyboard_latest_restore.owners.get_mut(&id) {
                        owner.coordinator.finish();
                    }
                    if generation.is_current() {
                        if let Err(error) = result {
                            diagnostics.push(error);
                        }
                    }
                }
                Reply::IntentFinished(generation, result) => {
                    let pending = self
                        .keyboard_latest_restore
                        .owners
                        .get_mut(&id)
                        .is_some_and(|owner| owner.pending_intents.remove(&generation));
                    if let Some(workspace) = workspace.filter(|_| pending) {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_keyboard_actuation(generation, result, cx)
                        });
                    }
                }
                Reply::Diagnostic(error) => diagnostics.push(error),
                Reply::Stopped => {
                    if let Some(mut owner) = self.keyboard_latest_restore.owners.remove(&id) {
                        cancelled.extend(
                            std::mem::take(&mut owner.pending_intents)
                                .into_iter()
                                .chain(std::mem::take(&mut owner.cancelled_intents))
                                .map(|generation| (id, generation)),
                        );
                    }
                    self.keyboard_latest_restore
                        .retry
                        .insert(id, Instant::now() + Duration::from_secs(1));
                }
            }
        }
        let mut repeats = Vec::new();
        for (id, owner) in &mut self.keyboard_latest_restore.owners {
            if exclusive {
                owner.brightness_repeats.clear();
            }
            for (next, event) in owner.brightness_repeats.values_mut() {
                if *next <= Instant::now() {
                    *next = Instant::now() + Duration::from_millis(110);
                    repeats.push((*id, event.clone()));
                }
            }
        }
        for (id, event) in repeats {
            if let Some(workspace) = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_keyboard_latest_mapping_event(event, cx)
                });
            }
        }
        let running = self.game_observer.running_games();
        let macros = self.macro_library.read(cx).snapshot();
        for workspace in &self.devices {
            let id = workspace.entity_id();
            let Some(owner) = self.keyboard_latest_restore.owners.get_mut(&id) else {
                continue;
            };
            let device = workspace.read(cx).device(cx);
            let signature = source_signature(device);
            if owner.source.as_ref() != Some(&signature) {
                let mut snapshot = device.clone();
                match super::mouse_dpi_70::prepare_mapping_document(&mut snapshot, &macros) {
                    Ok(()) => {
                        if let Some(document) = snapshot
                            .source_device_settings
                            .as_ref()
                            .and_then(|settings| settings.get(STORAGE_FIELD))
                        {
                            owner.enqueue(document.clone(), running, false);
                        }
                        owner.source = Some(signature);
                    }
                    Err(error) => {
                        diagnostics.push(format!("配置编译：{error:#}"));
                        owner.source = Some(signature);
                    }
                }
            } else if owner.running_games != running {
                if let Some(document) = owner.document.clone() {
                    owner.enqueue(document, running, true);
                }
            }
            if owner.ready {
                if let Some(work) = owner.coordinator.take_ready(Instant::now()) {
                    if owner.commands.send(work).is_err() {
                        owner.coordinator.finish();
                        diagnostics.push("键盘恢复线程已结束".into());
                    }
                }
            }
        }
        for worker in std::mem::take(&mut self.keyboard_latest_restore.workers) {
            if worker.is_finished() {
                if worker.join().is_err() {
                    diagnostics.push("键盘恢复线程异常退出".into());
                }
            } else {
                self.keyboard_latest_restore.workers.push(worker);
            }
        }
        for (id, owner) in &mut self.keyboard_latest_restore.owners {
            cancelled.extend(
                owner
                    .cancelled_intents
                    .drain(..)
                    .map(|generation| (*id, generation)),
            );
        }
        for (id, generation) in cancelled {
            if let Some(workspace) = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_actuation(
                        generation,
                        Err("全局行程同步已因配置或连接改变而取消".into()),
                        cx,
                    )
                });
            }
        }
        for error in diagnostics {
            self.keyboard_latest_diagnostic(error, cx);
        }
        for workspace in &self.devices {
            let connected = self
                .keyboard_latest_restore
                .owners
                .get(&workspace.entity_id())
                .is_some_and(|owner| owner.ready);
            workspace.update(cx, |workspace, cx| {
                workspace.set_keyboard_latest_connected(connected, cx)
            });
        }
    }
}
