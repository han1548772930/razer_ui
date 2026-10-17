//! Current Kitsune Chroma caller, local profile selection and host navigation.
//! Local storage receipts and native lighting diagnostics remain distinct.
use super::*;
use anyhow::{Context as _, ensure};
use razer_pages::features::gamepad_products::{
    ControllerChromaCommand, ControllerChromaObservation, ControllerChromaProfile,
    ControllerChromaRequested,
};
use razer_storage::{
    chroma_profiles::{self, Change, Snapshot},
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
const DEVICES_KEY: &str = "lighting-engine-chroma-devices";

struct Pending {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: ControllerChromaRequested,
    window: AnyWindowHandle,
}
struct Watcher {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    guard: Arc<AtomicBool>,
    window: AnyWindowHandle,
}
#[derive(Default)]
pub(super) struct State {
    pub(in crate::shell) enabled: Option<bool>,
    queue: VecDeque<Pending>,
    task: Option<Task<()>>,
    watch: Option<Task<()>>,
    watchers: BTreeMap<String, Watcher>,
    last_observation: Option<Value>,
    revision: u64,
    watch_guard: Arc<AtomicBool>,
}

pub(super) struct ControllerChromaSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    request: ControllerChromaRequested,
}

struct Readback {
    snapshot: Snapshot,
    installation: Result<razer_platform::chroma_installation::Installation, String>,
}
fn readback() -> anyhow::Result<Readback> {
    Ok(Readback {
        snapshot: chroma_profiles::load(
            &chroma_profiles::path(),
            &razer_storage::source_local::path(),
        )?,
        installation: razer_platform::chroma_installation::inspect()
            .map_err(|error| format!("{error:#}")),
    })
}
impl Readback {
    fn observation(&self) -> anyhow::Result<ControllerChromaObservation> {
        let mut observation = ControllerChromaObservation::new();
        if let Some(configuration) = self.snapshot.configuration() {
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
        if let Some(enabled) = self.snapshot.dynamic_lighting() {
            observation = observation.with_dynamic_lighting(enabled);
        }
        if let Some(installed) = self.snapshot.module_installed() {
            observation = observation.with_module_installed(installed);
        }
        if let Ok(installation) = &self.installation {
            observation = observation.with_fully_installed(installation.fully_installed());
        }
        Ok(observation)
    }
    fn fingerprint(&self) -> Value {
        json!({
            "configuration":self.snapshot.configuration(),
            "origin":format!("{:?}",self.snapshot.origin()),
            "dynamic":self.snapshot.dynamic_lighting(),
            "module":self.snapshot.module_installed(),
            "installation":self.installation.as_ref().map(|value| (value.folder_exists(),value.engine_registered())).map_err(String::as_str),
        })
    }
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
        self.controller_chroma.revision = self.controller_chroma.revision.wrapping_add(1);
        self.controller_chroma
            .watch_guard
            .store(false, Ordering::Release);
        let navigation = match request.command() {
            ControllerChromaCommand::LaunchStudio => Some(self.try_open_chroma_studio(cx)),
            ControllerChromaCommand::OpenSettings => {
                Some(self.open_product_settings(false, window, cx))
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
        self.controller_chroma.queue.push_back(Pending {
            workspace,
            identity,
            request: request.clone(),
            window: window.window_handle(),
        });
        self.controller_lighting_workers += 1;
        self.start_controller_chroma_request(cx);
    }

    fn start_controller_chroma_request(&mut self, cx: &mut Context<Self>) {
        if self.controller_chroma.task.is_some() {
            return;
        }
        let Some(pending) = self.controller_chroma.queue.pop_front() else {
            return;
        };
        let command = pending.request.command().clone();
        let guard = pending.request.guard();
        let owners = self
            .controller_lighting_owners
            .values()
            .map(|session| session.owner())
            .collect::<Vec<_>>();
        self.controller_chroma.task = Some(cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                ensure!(guard.load(Ordering::Acquire), "Chroma request belongs to a replaced profile");
                let (change, sync) = match command {
                    ControllerChromaCommand::Observe => (None, None),
                    ControllerChromaCommand::Toggle {enabled} => (Some(Change::Toggle(enabled)),None),
                    ControllerChromaCommand::SelectProfile {id} => (Some(Change::SelectProfile(id)),None),
                    ControllerChromaCommand::SyncQuick {source_effect} => (Some(Change::Toggle(false)),Some(QuickEnvelope::from_caller(&source_effect,tick()?)?)),
                    _ => anyhow::bail!("Chroma command does not belong to the local configuration queue"),
                };
                let snapshot = if let Some(change) = change {
                    chroma_profiles::update(&chroma_profiles::path(), &razer_storage::source_local::path(), change, &guard)?
                } else { chroma_profiles::load(&chroma_profiles::path(), &razer_storage::source_local::path())? };
                let mut runtime_failures = Vec::new();
                if let Some(configuration) = snapshot.configuration() {
                    for owner in owners {
                        if let Err(error) = owner.set_advanced(configuration.enabled(), &guard) { runtime_failures.push(format!("{error:#}")); }
                    }
                }
                ensure!(guard.load(Ordering::Acquire), "Chroma response belongs to a replaced profile");
                Ok::<_,anyhow::Error>((Readback { snapshot, installation: razer_platform::chroma_installation::inspect().map_err(|error| format!("{error:#}")) },sync,runtime_failures))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.controller_chroma.task = None;
                this.controller_lighting_workers = this.controller_lighting_workers.saturating_sub(1);
                let current = this.devices.contains(&pending.workspace)
                    && pending.workspace.read(cx).identity(cx) == pending.identity
                    && pending.workspace.read(cx).accepts_controller_chroma(&pending.request,cx);
                if current {
                    let completion = match result {
                        Ok((readback,sync,failures)) => {
                            let observation = this.apply_controller_chroma_readback(&readback,cx);
                            let delivery = sync.map(|sync| this.broadcast_controller_sync_quick(&sync,&pending.workspace,cx)).transpose();
                            match (observation,delivery,failures.is_empty()) {
                                (Ok(observation),Ok(_),true) => Ok(Some(observation)),
                                (Err(error),_,_) | (_,Err(error),_) => Err(format!("{error:#}")),
                                (_,_,false) => Err(format!("Chroma selection was saved locally; native lighting transition failed: {}", failures.join("; "))),
                            }
                        }
                        Err(error) => Err(format!("{error:#}")),
                    };
                    if let Err(error) = &completion { this.status = format!("Chroma：{error}"); }
                    this.finish_controller_chroma(&pending.workspace,&pending.request,completion,pending.window,cx);
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
        let reply = self.host_storage.call(HostStorageCall {
            store: HostStoreKind::Memory,
            sender_id: HOST_VIEW,
            action: "getMemoryStorageItem".into(),
            payload: json!({"key":DEVICES_KEY}),
            target_url_array: Vec::new(),
        })?;
        #[derive(Deserialize)]
        struct Row {
            value: String,
        }
        #[derive(Deserialize)]
        struct Envelope {
            payload: Devices,
        }
        #[derive(Deserialize)]
        struct Devices {
            devices: Vec<Product>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Product {
            product_id: u32,
        }
        ensure!(reply.defined, "Chroma memory getter did not return a value");
        let rows: Vec<Row> = serde_json::from_str(
            reply
                .value
                .as_str()
                .context("Chroma memory response is not a string")?,
        )?;
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let devices: Envelope = serde_json::from_str(&row.value)?;
        Ok(Some(
            devices
                .payload
                .devices
                .into_iter()
                .map(|device| device.product_id)
                .collect(),
        ))
    }

    fn apply_controller_chroma_readback(
        &mut self,
        readback: &Readback,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<ControllerChromaObservation> {
        let mut observation = readback.observation()?;
        self.controller_chroma.enabled = readback
            .snapshot
            .configuration()
            .map(|configuration| configuration.enabled());
        if let Some(products) = self.observed_chroma_products()? {
            observation = observation.with_connected_products(products);
        }
        if let Err(error) = &readback.installation {
            self.status = format!("Chroma 安装状态未读取：{error}");
        }
        self.controller_chroma.last_observation = Some(readback.fingerprint());
        self.publish_controller_chroma_observation(observation.clone(), cx);
        Ok(observation)
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
                    let guard = Arc::new(AtomicBool::new(true));
                    this.controller_chroma.watch_guard = guard.clone();
                    Some((
                        this.controller_chroma.revision,
                        this.controller_chroma.task.is_none()
                            && this.controller_chroma.queue.is_empty(),
                        this.controller_lighting_owners
                            .values()
                            .map(|session| session.owner())
                            .collect::<Vec<_>>(),
                        guard,
                    ))
                });
                let Ok(Some((revision, ready, owners, guard))) = active else {
                    break;
                };
                if !ready {
                    continue;
                }
                let result = cx
                    .background_spawn(async move {
                        let readback = readback()?;
                        let mut failures = Vec::new();
                        if let Some(configuration) = readback.snapshot.configuration() {
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
                        if this.controller_chroma.revision != revision
                            || this.controller_chroma.task.is_some()
                        {
                            return;
                        }
                        match result {
                            Ok((readback, failures)) => {
                                if this.controller_chroma.last_observation.as_ref()
                                    != Some(&readback.fingerprint())
                                {
                                    if let Err(error) =
                                        this.apply_controller_chroma_readback(&readback, cx)
                                    {
                                        this.status = format!("Chroma 状态未读取：{error:#}");
                                        cx.notify();
                                    }
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
        self.pending_saves.push_back(PreparedSave {
            window: window.window_handle(),
            file,
            product: Some(ProductSave::ControllerChroma(ControllerChromaSave {
                workspace,
                identity,
                profile_id,
                request,
            })),
        });
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
        if let ControllerChromaCommand::CustomColors(colors) = chroma.request.command() {
            save.file.custom_colors = *colors;
        }
        Ok(())
    }

    pub(super) fn finish_controller_chroma_save(
        &mut self,
        save: ControllerChromaSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if succeeded {
            self.refresh_controller_power_configuration(cx);
        }
        let result = if succeeded {
            Ok(None)
        } else {
            Err("Chroma local profile persistence did not complete".into())
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
            #[derive(Deserialize, Serialize)]
            struct Color {
                color: String,
                #[serde(flatten)]
                fields: Map<String, Value>,
            }
            let quick = profile
                .get_mut("quickEffects")
                .and_then(Value::as_object_mut)
                .context("Quick-effect settings are missing")?;
            let old: Vec<Color> = quick
                .get("customColors")
                .map(|value| serde_json::from_value(value.clone()))
                .transpose()?
                .unwrap_or_default();
            let next = colors
                .iter()
                .flatten()
                .map(|rgb| {
                    let color = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                    let fields = old
                        .iter()
                        .find(|old| old.color.eq_ignore_ascii_case(&color))
                        .map(|old| old.fields.clone())
                        .unwrap_or_default();
                    Color { color, fields }
                })
                .collect::<Vec<_>>();
            quick.insert("customColors".into(), serde_json::to_value(next)?);
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
        let mut colors = [None; 16];
        colors[0] = Some([17, 34, 51]);
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
