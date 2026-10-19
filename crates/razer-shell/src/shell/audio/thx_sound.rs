//! Current THX Sound request -> local draft -> owned worker -> transport receipt.
//! UI controls/messages remain in the source-audited audio_sound component.
//! Extra Rust getters are diagnostics, not MW_UPDATE_THX_SPATIAL_AUDIO events.
use super::*;
use anyhow::{Context as _, Result, ensure};
use razer_device::{
    thx_lifecycle_discovery::{ThxConnectionEpoch, ThxConnectionIdentity},
    thx_submission::{
        ThxOperationStatus, ThxSubmissionPlan, ThxSubmissionProgress, ThxSubmissionScope,
    },
    thx_v4::{ThxRenderSetupTrigger, capability},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::{HelpOwnerCommand, HelpOwnerRequested};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) struct Session {
    revision: u64,
    render_connection: Option<ThxConnectionIdentity>,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    _pump: Task<()>,
}

enum SubmissionGuard {
    Sound(HelpOwnerRequested),
    RenderSetup(ThxConnectionEpoch),
}
/// One child owns one verified physical connection. It survives ordinary
/// Sound jobs and is never reused for a different HID path/connection epoch.
pub(super) struct RetainedOwner {
    connection: ThxConnectionEpoch,
    releasing: AtomicBool,
    session: Mutex<OwnerClient>,
}
struct OwnerClient {
    client: Option<ServiceClient>,
    render_started: bool,
    id: String,
    scope: ThxSubmissionScope,
}
pub(super) type OwnerSlot = Arc<RetainedOwner>;
pub(super) struct RetiredOwner {
    owner: OwnerSlot,
    source_close: bool,
}
impl RetainedOwner {
    pub(super) fn connection(&self) -> &ThxConnectionEpoch {
        &self.connection
    }
    fn acquire(&self, canceled: &AtomicBool) -> Result<std::sync::MutexGuard<'_, OwnerClient>> {
        let accepting = || -> Result<()> {
            ensure!(!canceled.load(Ordering::Acquire), "THX request is canceled");
            ensure!(
                !self.releasing.load(Ordering::Acquire),
                "THX owner is releasing its transport"
            );
            Ok(())
        };
        accepting()?;
        let owner = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("THX owner lock failed"))?;
        accepting()?;
        Ok(owner)
    }
}

fn record_receipt(
    history: &Arc<Mutex<std::collections::VecDeque<ThxSubmissionProgress>>>,
    progress: ThxSubmissionProgress,
) {
    if let Ok(mut receipts) = history.lock() {
        if receipts.len() == 256 {
            receipts.pop_front();
        }
        receipts.push_back(progress);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
    }
}
impl Session {
    fn retire(mut self) -> Option<JoinHandle<()>> {
        self.canceled.store(true, Ordering::Release);
        self.worker.take()
    }
}
fn local_key(scope: &ThxSubmissionScope, field: &str) -> String {
    // Explicit app-local persistence; no claim to Chromium or vendor storage.
    format!(
        "thx_local:{}:{}:{}:{field}",
        scope.product_id,
        scope.container_id.to_ascii_lowercase(),
        scope.profile_id
    )
}
fn roller_local_key(scope: &ThxSubmissionScope) -> String {
    // Current RollerMappingFeature explicitly overrides Storage_Profile with
    // Storage_Global. A profile switch must not select a different mapping.
    format!(
        "blackshark_roller:{}:{}:mappings",
        scope.product_id,
        scope.container_id.to_ascii_lowercase()
    )
}
fn local_presets(scope: &ThxSubmissionScope, device: &Device) -> Result<Vec<Value>> {
    let path = razer_storage::source_local::path()?;
    let key = local_key(scope, "cloudAudioEqualizer");
    let profile = device
        .profiles
        .iter()
        .find(|profile| profile.id == scope.profile_id)
        .and_then(|profile| profile.source_settings.as_ref())
        .and_then(|settings| settings.get("cloudAudioEqualizer"));
    let saved = razer_storage::source_local::get(&path, &key)?;
    let value = profile.cloned().or(saved).or_else(|| {
        device
            .source_device_settings
            .as_ref()
            .and_then(|settings| settings.get("cloudAudioEqualizer"))
            .cloned()
    });
    let presets = match value {
        Some(Value::Array(presets)) => presets,
        Some(_) => anyhow::bail!("本地 cloudAudioEqualizer 格式无效；原数据已保留"),
        None => razer_device::thx_blackshark_firmware::default_blackshark_local_eq_presets(
            scope.product_id,
        )?
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?,
    };
    // Source built-ins are local presets, never hardware observations.
    for preset in &presets {
        razer_device::thx_blackshark_firmware::BlackSharkLocalEqPreset::parse(preset)?;
    }
    razer_storage::source_local::set(&path, &key, &json!(presets))?;
    Ok(presets)
}
fn run(
    scope: ThxSubmissionScope,
    mut plan: ThxSubmissionPlan,
    device: Device,
    cancellation: &AtomicBool,
    owner_slot: &OwnerSlot,
) -> Result<ThxSubmissionProgress> {
    let check = || {
        ensure!(!cancellation.load(Ordering::Acquire), "THX 请求已取消");
        Ok(())
    };
    check()?;
    if let ThxSubmissionPlan::SetSpatial {
        enabled,
        presets,
        timer_tick,
    } = &mut plan
    {
        *presets = local_presets(&scope, &device)?;
        check()?;
        razer_storage::source_local::set(
            &razer_storage::source_local::path()?,
            &local_key(&scope, "spatialDraft"),
            &json!({"isEnabled":enabled,"timerTick":timer_tick}),
        )?;
    }
    check()?;
    let mut owner = owner_slot.acquire(cancellation)?;
    ensure!(
        owner.scope.product_id == scope.product_id
            && owner
                .scope
                .container_id
                .eq_ignore_ascii_case(&scope.container_id)
            && owner.scope.serial_number == scope.serial_number,
        "THX retained owner identity differs"
    );
    let first = owner.client.is_none();
    if first {
        ensure!(
            plan.is_direct_hid()
                || matches!(
                    plan,
                    ThxSubmissionPlan::SetupRender {
                        trigger: ThxRenderSetupTrigger::InitialDeviceSetup
                    }
                ),
            "THX render owner setup is not available; local draft retained"
        );
        owner.client = Some(ServiceClient::spawn()?);
    }
    owner.scope = scope.clone();
    if matches!(plan, ThxSubmissionPlan::SetupRender { .. }) {
        owner.render_started = true;
    }
    let owner_id = owner.id.clone();
    let client = owner.client.as_mut().expect("Retained child initialized");
    let id = uuid::Uuid::new_v4().to_string();
    let mut started = false;
    let mut terminal = false;
    let mut result = (|| {
        check()?;
        let request = if first {
            ServiceRequest::ThxOwnerStart {
                owner_id: owner_id.clone(),
                operation_id: id.clone(),
                scope: scope.clone(),
                plan: plan.clone(),
            }
        } else {
            ServiceRequest::ThxOwnerSubmit {
                owner_id: owner_id.clone(),
                operation_id: id.clone(),
                scope: scope.clone(),
                plan: plan.clone(),
            }
        };
        let reply = client.request(request)?;
        started = true;
        ensure!(
            reply["owner_id"] == owner_id
                && reply["operation_id"] == id
                && reply["started"] == true,
            "THX 任务接收回执不匹配"
        );
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut cancel_sent = false;
        loop {
            if (cancellation.load(Ordering::Acquire) || Instant::now() >= deadline) && !cancel_sent
            {
                let reply = client.request(ServiceRequest::ThxCancel {
                    operation_id: id.clone(),
                })?;
                ensure!(
                    reply["operation_id"] == id && reply["cancel_requested"] == true,
                    "THX 取消回执不匹配"
                );
                cancel_sent = true;
            }
            let status: ThxOperationStatus =
                serde_json::from_value(client.request(ServiceRequest::ThxPoll {
                    operation_id: id.clone(),
                })?)
                .context("THX 任务响应格式无效")?;
            if let ThxOperationStatus::Finished(progress) = status {
                terminal = true;
                ensure!(
                    progress.scope == scope && progress.plan == plan,
                    "THX 结果身份或请求不匹配"
                );
                return Ok(progress);
            }
            ensure!(
                Instant::now() < deadline + Duration::from_secs(8),
                "THX 取消清理未返回；已发送设置的结果未确认"
            );
            thread::sleep(Duration::from_millis(25));
        }
    })();
    if started && !terminal {
        let _ = client.request(ServiceRequest::ThxCancel { operation_id: id });
    }
    drop(owner);
    if matches!(
        plan,
        ThxSubmissionPlan::SetupRender {
            trigger: ThxRenderSetupTrigger::InitialDeviceSetup
        }
    ) && !cancellation.load(Ordering::Acquire)
    {
        // DeviceConnected owns initFromDevice independently of THX readiness.
        // Reuse the same retained collection even when the local THX RPC fails.
        // Firmware info is initialized locally on the same physical owner.
        // A failed THX service must not suppress these independent HID reads.
        let firmware = run(
            scope.clone(),
            ThxSubmissionPlan::ReadFirmwareVersion,
            device.clone(),
            cancellation,
            owner_slot,
        );
        let mapping = run(
            scope.clone(),
            ThxSubmissionPlan::ReadRollerMapping,
            device,
            cancellation,
            owner_slot,
        );
        let progress = merge_local_initialization(result, scope.clone(), plan.clone(), firmware);
        result = Ok(merge_local_initialization(
            Ok(progress),
            scope,
            plan,
            mapping,
        ));
    }
    result
}

/// Help uses the same retained child and serialized client as Sound jobs.
/// Source resetHardware is a no-op while DeviceResetFeature is installed.
pub(super) fn run_help_factory_reset1398(
    scope: ThxSubmissionScope,
    device: Device,
    canceled: &AtomicBool,
    owner: &OwnerSlot,
    history: &Arc<Mutex<std::collections::VecDeque<ThxSubmissionProgress>>>,
) -> Result<ThxSubmissionProgress> {
    ensure!(
        device.product_id == razer_device::blackshark_reset::PRODUCT_ID,
        "No current Help hardware reset owner for this product"
    );
    let result = run(
        scope,
        ThxSubmissionPlan::FactoryReset1398,
        device,
        canceled,
        owner,
    );
    if let Ok(progress) = &result {
        record_receipt(history, progress.clone());
    }
    result
}

/// Keep independent local initialization receipts when the render RPC fails.
/// This bookkeeping does not manufacture a source UI success notification.
fn merge_local_initialization(
    render: Result<ThxSubmissionProgress>,
    scope: ThxSubmissionScope,
    plan: ThxSubmissionPlan,
    local: Result<ThxSubmissionProgress>,
) -> ThxSubmissionProgress {
    let mut progress = render.unwrap_or_else(|error| ThxSubmissionProgress {
        scope,
        plan,
        render_setup: None,
        render_shutdown: None,
        roller_mapping: None,
        initial_observation: None,
        outcome: None,
        firmware: vec![],
        error: Some(format!("{error:#}")),
        cleanup_error: None,
    });
    let error = match local {
        Ok(local) => {
            if local.roller_mapping.is_some() {
                progress.roller_mapping = local.roller_mapping;
            }
            progress.firmware.extend(local.firmware);
            if let Some(error) = local.cleanup_error {
                progress.cleanup_error = Some(match progress.cleanup_error.take() {
                    Some(previous) => format!("{previous}; local init: {error}"),
                    None => error,
                });
            }
            local.error
        }
        Err(error) => Some(format!("{error:#}")),
    };
    if let Some(error) = error {
        progress.error = Some(match progress.error.take() {
            Some(previous) => format!("{previous}; local init: {error}"),
            None => error,
        });
    }
    progress
}

/// Explicit shutdown runs the source render leaf against the retained owner.
/// Discovery expiry uses only owner release. Application quit is a Rust
/// lifecycle adapter; the original host's shouldClose producer and capture
/// shutdown remain unimplemented. Host tab hiding does not enter this path.
fn release_owner(
    slot: OwnerSlot,
    source_close: bool,
    history: &Arc<Mutex<std::collections::VecDeque<ThxSubmissionProgress>>>,
) {
    slot.releasing.store(true, Ordering::Release);
    let Ok(mut owner) = slot.session.lock() else {
        return;
    };
    let scope = owner.scope.clone();
    let owner_id = owner.id.clone();
    let Some(mut client) = owner.client.take() else {
        return;
    };
    if source_close && owner.render_started {
        let operation_id = uuid::Uuid::new_v4().to_string();
        let plan = ThxSubmissionPlan::ShutdownRender;
        let mut diagnostic = ThxSubmissionProgress {
            scope: scope.clone(),
            plan: plan.clone(),
            render_setup: None,
            render_shutdown: None,
            roller_mapping: None,
            initial_observation: None,
            outcome: None,
            firmware: vec![],
            error: None,
            cleanup_error: None,
        };
        let result = (|| -> Result<()> {
            let reply = client.request(ServiceRequest::ThxOwnerSubmit {
                owner_id: owner_id.clone(),
                operation_id: operation_id.clone(),
                scope: scope.clone(),
                plan: plan.clone(),
            })?;
            ensure!(
                reply["started"] == true
                    && reply["owner_id"] == owner_id
                    && reply["operation_id"] == operation_id,
                "THX shutdown acceptance differs"
            );
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                match serde_json::from_value(client.request(ServiceRequest::ThxPoll {
                    operation_id: operation_id.clone(),
                })?)? {
                    ThxOperationStatus::Finished(progress) => {
                        ensure!(
                            progress.scope == scope && progress.plan == plan,
                            "THX shutdown receipt identity differs"
                        );
                        diagnostic = progress;
                        return Ok(());
                    }
                    ThxOperationStatus::Running => {
                        if Instant::now() >= deadline {
                            let _ = client.request(ServiceRequest::ThxCancel {
                                operation_id: operation_id.clone(),
                            });
                            anyhow::bail!(
                                "THX shutdown deadline; submitted settings outcome unknown"
                            );
                        }
                        thread::sleep(Duration::from_millis(25));
                    }
                }
            }
        })();
        if let Err(error) = result {
            diagnostic.error = Some(format!("{error:#}"));
        }
        // Shutdown receipt poll releases that source owner. If source shutdown
        // could not run, this independent release sends no guessed setters.
        if diagnostic.render_shutdown.is_none() {
            if let Err(error) = client.request(ServiceRequest::ThxOwnerRelease {
                owner_id: owner_id.clone(),
            }) {
                diagnostic.cleanup_error = Some(format!("{error:#}"));
            }
        }
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            diagnostic.cleanup_error = Some(format!("{error:#}"));
        }
        record_receipt(history, diagnostic);
    } else {
        let released = client.request(ServiceRequest::ThxOwnerRelease { owner_id });
        let stopped = client.request(ServiceRequest::Shutdown);
        let error = released.err().or_else(|| stopped.err());
        if let Some(error) = error {
            eprintln!("[thx-local] owner release: {error:#}");
        }
    }
}
impl AppShell {
    pub(super) fn help_hardware_reset_owner_current(
        &self,
        identity: &str,
        owner: &OwnerSlot,
    ) -> bool {
        !owner.releasing.load(Ordering::Acquire)
            && self
                .thx_owners
                .get(identity)
                .is_some_and(|current| Arc::ptr_eq(current, owner))
            && self
                .thx_render_discovery
                .current_epoch(&owner.connection.identity)
                .as_ref()
                == Some(&owner.connection)
    }
    pub(super) fn help_factory_reset1398_owner(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        scope: &ThxSubmissionScope,
        cx: &App,
    ) -> Result<OwnerSlot> {
        ensure!(
            self.devices.contains(workspace) && !self.close_requested,
            "Help reset workspace is no longer accepting requests"
        );
        ensure!(
            self.thx_cleanup.iter().all(JoinHandle::is_finished)
                && self.thx_retired_owners.is_empty(),
            "Previous headset owner is still releasing its transport"
        );
        let device = workspace.read(cx).device(cx);
        ensure!(
            device.product_id == razer_device::blackshark_reset::PRODUCT_ID,
            "Help reset is not a current 1398 owner"
        );
        let connection = self
            .observed_thx_connection(device)?
            .and_then(|identity| self.thx_render_discovery.current_epoch(&identity))
            .context("Help reset has no retained physical connection epoch")?;
        self.retained_thx_owner(workspace.read(cx).identity(cx), scope, connection)
    }

    fn retained_thx_owner(
        &mut self,
        identity: String,
        scope: &ThxSubmissionScope,
        connection: ThxConnectionEpoch,
    ) -> Result<OwnerSlot> {
        let owner = self
            .thx_owners
            .entry(identity)
            .or_insert_with(|| {
                Arc::new(RetainedOwner {
                    connection: connection.clone(),
                    releasing: AtomicBool::new(false),
                    session: Mutex::new(OwnerClient {
                        client: None,
                        render_started: false,
                        id: uuid::Uuid::new_v4().to_string(),
                        scope: scope.clone(),
                    }),
                })
            })
            .clone();
        ensure!(
            owner.connection == connection,
            "Retained headset owner belongs to a previous physical connection"
        );
        ensure!(
            !owner.releasing.load(Ordering::Acquire),
            "Retained headset owner is releasing its transport"
        );
        Ok(owner)
    }

    pub(super) fn install_thx_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            this.cancel_thx_jobs();
            let workers = std::mem::take(&mut this.thx_cleanup);
            let owners = std::mem::take(&mut this.thx_owners);
            let retired = std::mem::take(&mut this.thx_retired_owners);
            let history = this.thx_receipt_history.clone();
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    let _ = worker.join();
                }
                for retired in retired {
                    release_owner(retired.owner, retired.source_close, &history);
                }
                for (_, owner) in owners {
                    release_owner(owner, true, &history);
                }
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn cancel_thx_jobs(&mut self) {
        for (_, session) in std::mem::take(&mut self.thx_jobs) {
            self.thx_cleanup.extend(session.retire());
        }
    }
    fn retire_thx_owner(&mut self, identity: &str, source_close: bool) {
        let Some(owner) = self.thx_owners.remove(identity) else {
            return;
        };
        let history = self.thx_receipt_history.clone();
        // Preserve the owner if the OS rejects thread creation. Its client
        // must remain with a background release owner, including this failure.
        let release = owner.clone();
        match thread::Builder::new()
            .name("thx-owner-release".into())
            .spawn(move || release_owner(release, source_close, &history))
        {
            Ok(worker) => self.thx_cleanup.push(worker),
            Err(error) => {
                self.thx_retired_owners.push(RetiredOwner {
                    owner,
                    source_close,
                });
                self.status = format!("THX 清理线程启动失败：{error}");
                eprintln!("[thx-local] {}", self.status);
            }
        }
    }
    pub(super) fn cancel_thx_sound_jobs(&mut self) {
        let owners = self
            .thx_jobs
            .iter()
            .filter(|(_, job)| job.render_connection.is_none())
            .map(|(owner, _)| owner.clone())
            .collect::<Vec<_>>();
        for owner in owners {
            self.thx_cleanup
                .extend(self.thx_jobs.remove(&owner).and_then(Session::retire));
        }
    }
    fn observed_thx_connection(&self, device: &Device) -> Result<Option<ThxConnectionIdentity>> {
        use razer_model::model::DeviceConnectionObservation;
        if capability(device.product_id).is_none() || device.use_ble {
            return Ok(None);
        }
        let serial = device.serial_number.to_ascii_uppercase();
        if serial.starts_with("PREVIEW-") || serial.starts_with("DEMO-") {
            return Ok(None);
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(device))
            .collect::<Vec<_>>();
        if routes.is_empty() {
            return Ok(None);
        }
        let [route] = routes.as_slice() else {
            anyhow::bail!("THX physical observation is ambiguous");
        };
        if !matches!(
            route.connection(),
            DeviceConnectionObservation::UsbPresent
                | DeviceConnectionObservation::HidPresent
                | DeviceConnectionObservation::ReceiverPeer(1)
        ) {
            return Ok(None);
        }
        let node = route
            .hid_node()
            .context("THX observation lacks a retained HID collection")?;
        ThxConnectionIdentity::new(
            device.product_id,
            device.serial_number.clone(),
            device.device_container_id.clone(),
            node.product_id,
            node.path.clone(),
        )
        .map(Some)
    }
    /// First verified connection starts source render init independently of
    /// page mounting. Unchanged scans/reads do not repeat initialization. An
    /// incomplete snapshot remains unknown, not a fabricated reconnect.
    pub(super) fn sync_thx_render_lifecycle(
        &mut self,
        complete: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut identities_complete = complete;
        let observations = self
            .devices
            .iter()
            .filter_map(|workspace| {
                match self.observed_thx_connection(workspace.read(cx).device(cx)) {
                    Ok(Some(connection)) => Some((workspace.clone(), connection)),
                    Ok(None) => None,
                    Err(_) => {
                        identities_complete = false;
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        let connections = observations
            .iter()
            .map(|(_, connection)| connection.clone())
            .collect::<Vec<_>>();
        for (index, (_, connection)) in observations.iter().enumerate() {
            if observations[..index]
                .iter()
                .any(|(_, previous)| previous == connection)
            {
                // Duplicate physical workspaces remain ambiguous. Distinct
                // containers can use direct HID; RPC separately checks its
                // product/endpoint association before every request.
                identities_complete = false;
            }
        }
        let added = self
            .thx_render_discovery
            .observe(identities_complete.then_some(connections.as_slice()));
        let stale = self
            .thx_jobs
            .iter()
            .filter_map(|(owner, job)| {
                job.render_connection
                    .as_ref()
                    .filter(|connection| !identities_complete || !connections.contains(connection))
                    .map(|_| owner.clone())
            })
            .collect::<Vec<_>>();
        for owner in stale {
            self.thx_cleanup
                .extend(self.thx_jobs.remove(&owner).and_then(Session::retire));
        }
        if identities_complete {
            let expired = self
                .thx_owners
                .iter()
                .filter(|(_, slot)| !connections.contains(&slot.connection.identity))
                .map(|(identity, _)| identity.clone())
                .collect::<Vec<_>>();
            for identity in expired {
                self.retire_thx_owner(&identity, false);
            }
        }
        for epoch in added {
            let Some((workspace, _)) = observations
                .iter()
                .find(|(_, connection)| *connection == epoch.identity)
            else {
                continue;
            };
            self.submit_thx_job(
                workspace.clone(),
                ThxSubmissionPlan::SetupRender {
                    trigger: ThxRenderSetupTrigger::InitialDeviceSetup,
                },
                SubmissionGuard::RenderSetup(epoch),
                window,
                cx,
            );
        }
    }
    pub(super) fn refresh_thx_spatial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for workspace in self.devices.clone() {
            if self
                .thx_jobs
                .get(&workspace.read(cx).identity(cx))
                .is_some_and(|job| job.render_connection.is_some())
            {
                continue;
            }
            let request = workspace.read(cx).help_owner_observation_request(cx);
            if let Some(request) =
                request.filter(|request| capability(request.product_id()).is_some())
            {
                self.request_thx_sound(workspace, request, window, cx);
            }
        }
    }
    pub(super) fn request_thx_sound(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpOwnerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if capability(request.product_id()).is_none()
            || !workspace.read(cx).accepts_help_audio_sound(&request, cx)
        {
            return;
        }
        let plan = if request.command() == HelpOwnerCommand::Observe {
            let device = workspace.read(cx).snapshot(cx);
            let identity = workspace.read(cx).identity(cx);
            if self
                .observed_thx_connection(&device)
                .ok()
                .flatten()
                .is_some()
                && let Some(version) = self
                    .thx_render_progress
                    .get(&identity)
                    .and_then(ThxSubmissionProgress::observed_firmware_version)
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_help_audio_sound(
                        &device,
                        &request,
                        "currentFWVersion",
                        json!(version),
                        window,
                        cx,
                    );
                });
            }
            let scope = ThxSubmissionScope {
                product_id: device.product_id,
                container_id: device.device_container_id.clone(),
                serial_number: device.serial_number.clone(),
                profile_id: device.active_profile.clone(),
                generation: request.audio_revision(),
            };
            match razer_storage::source_local::path().and_then(|path| {
                razer_storage::source_local::get(&path, &roller_local_key(&scope))
            }) {
                Ok(Some(mappings)) if mappings.is_array() => {
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_help_audio_sound(
                            &device,
                            &request,
                            "mappingList",
                            mappings,
                            window,
                            cx,
                        );
                    });
                }
                Ok(_) => {}
                Err(error) => eprintln!("[roller-mapping] local storage: {error:#}"),
            }
            ThxSubmissionPlan::ReadSpatial
        } else {
            let messages = request
                .audio_sound_messages()
                .iter()
                .filter_map(|message| message.get("transport"))
                .filter(|transport| {
                    matches!(
                        transport["type"].as_str(),
                        Some("ON_SET_SPATIAL_ENABLE" | "ON_SET_KEYMAPPING")
                    )
                })
                .collect::<Vec<_>>();
            let [transport] = messages.as_slice() else {
                return;
            };
            if transport["type"] == "ON_SET_KEYMAPPING" {
                let Some(mappings) = transport["payload"]["mappingList"].as_array() else {
                    return;
                };
                ThxSubmissionPlan::SetRollerMappings {
                    mappings: mappings.clone(),
                }
            } else {
                let Some(enabled) = transport["payload"]["thxEnabled"].as_bool() else {
                    return;
                };
                let Some(timer_tick) = transport["timerTick"].as_u64() else {
                    return;
                };
                ThxSubmissionPlan::SetSpatial {
                    enabled,
                    presets: vec![],
                    timer_tick,
                }
            }
        };
        self.submit_thx_job(workspace, plan, SubmissionGuard::Sound(request), window, cx);
    }
    fn submit_thx_job(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        plan: ThxSubmissionPlan,
        guard: SubmissionGuard,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let device = workspace.read(cx).snapshot(cx);
        let identity = workspace.read(cx).identity(cx);
        let container = device.device_container_id.clone();
        let profile = device.active_profile.clone();
        let serial = device.serial_number.clone();
        let discovery_revision = self.discovery_revision;
        let scope_generation = match &guard {
            SubmissionGuard::Sound(request) => request.audio_revision(),
            SubmissionGuard::RenderSetup(connection) => connection.epoch,
        };
        let render_connection = match &guard {
            SubmissionGuard::RenderSetup(connection) => Some(connection.identity.clone()),
            SubmissionGuard::Sound(_) => None,
        };
        let scope = ThxSubmissionScope {
            product_id: device.product_id,
            container_id: container.clone(),
            serial_number: device.serial_number.clone(),
            profile_id: profile.clone(),
            generation: scope_generation,
        };
        if let Err(error) = scope.validate() {
            self.status = format!("THX 请求身份无效：{error:#}");
            cx.notify();
            return;
        }
        if let ThxSubmissionPlan::SetRollerMappings { mappings } = &plan {
            // The source updates global storage and UI before enqueueing the
            // device task. Persist once here, so a canceled older worker can
            // never overwrite the newer local mapping after waiting its turn.
            let saved = razer_device::blackshark_roller_mapping::validate_mappings(mappings)
                .and_then(|_| {
                    razer_storage::source_local::set(
                        &razer_storage::source_local::path()?,
                        &roller_local_key(&scope),
                        &json!(mappings),
                    )
                });
            if let Err(error) = saved {
                self.status = format!("旋钮映射本地保存失败；设备任务未提交：{error:#}");
                cx.notify();
                return;
            }
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(&device))
            .collect::<Vec<_>>();
        let current = self.devices.contains(&workspace)
            && routes.len() == 1
            && !device.use_ble
            && !container.is_empty()
            && !container.starts_with("hid-collection:")
            && !matches!(routes.first().map(|route| route.connection()), Some(razer_model::model::DeviceConnectionObservation::ReceiverPeer(status)) if status != 1);
        if !current {
            self.status = "THX 未提交：当前设备/容器观察不可用".into();
            cx.notify();
            return;
        }
        let Some(connection) = self
            .observed_thx_connection(&device)
            .ok()
            .flatten()
            .and_then(|identity| self.thx_render_discovery.current_epoch(&identity))
        else {
            self.status = "THX 未提交：已确认的连接周期不可用".into();
            return;
        };
        let owner_slot = match self.retained_thx_owner(identity.clone(), &scope, connection) {
            Ok(owner) => owner,
            Err(error) => {
                self.status = format!("THX retained owner is unavailable: {error:#}");
                cx.notify();
                return;
            }
        };
        let mut pending = vec![];
        for worker in std::mem::take(&mut self.thx_cleanup) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.thx_cleanup = pending;
        // Discovery may already have retired the previous owner into cleanup.
        // Transfer every unfinished teardown to the successor worker so even
        // path/container changes across snapshots cannot overlap transports.
        let mut predecessors = std::mem::take(&mut self.thx_cleanup);
        predecessors.extend(self.thx_jobs.remove(&identity).and_then(Session::retire));
        let predecessor = Arc::new(Mutex::new(predecessors));
        let retired = Arc::new(Mutex::new(std::mem::take(&mut self.thx_retired_owners)));
        self.thx_job_sequence = self.thx_job_sequence.wrapping_add(1);
        let job_revision = self.thx_job_sequence;
        if render_connection.is_some() {
            self.thx_render_progress.remove(&identity);
        } else {
            self.thx_progress.remove(&identity);
        }
        let previous = predecessor.clone();
        let retired_owners = retired.clone();
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let retained_scope = scope.clone();
        let history_scope = scope.clone();
        let history_plan = plan.clone();
        let history = self.thx_receipt_history.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name("thx-page-worker".into())
            .spawn(move || {
                // New settings wait for predecessor teardown, avoiding duplicate
                // writes or contention with the old retained HID collection.
                let workers = std::mem::take(&mut *previous.lock().unwrap());
                for worker in workers {
                    let _ = worker.join();
                }
                for retired in std::mem::take(&mut *retired_owners.lock().unwrap()) {
                    release_owner(retired.owner, retired.source_close, &history);
                }
                let result = (|| -> Result<ThxSubmissionProgress> {
                    let needs_render_setup = !owner_slot
                        .session
                        .lock()
                        .map_err(|_| anyhow::anyhow!("THX owner lock failed"))?
                        .render_started;
                    if needs_render_setup
                        && !plan.is_direct_hid()
                        && !matches!(
                            plan,
                            ThxSubmissionPlan::SetupRender {
                                trigger: ThxRenderSetupTrigger::InitialDeviceSetup
                            }
                        )
                    {
                        // A missing local owner first needs its separate setup
                        // operation. This is a Rust lifetime adapter, not a
                        // source getter side effect or a host tab-close trigger.
                        let setup = run(
                            retained_scope.clone(),
                            ThxSubmissionPlan::SetupRender {
                                trigger: ThxRenderSetupTrigger::InitialDeviceSetup,
                            },
                            device.clone(),
                            &signal,
                            &owner_slot,
                        )?;
                        let ready = setup
                            .render_setup
                            .as_ref()
                            .is_some_and(|setup| setup.setup_succeeded);
                        record_receipt(&history, setup);
                        ensure!(ready, "THX new render owner setup did not complete");
                    }
                    run(retained_scope, plan, device, &signal, &owner_slot)
                })()
                .map_err(|error| format!("{error:#}"));
                let diagnostic = match &result {
                    Ok(progress) => progress.clone(),
                    Err(error) => ThxSubmissionProgress {
                        scope: history_scope,
                        plan: history_plan,
                        render_setup: None,
                        render_shutdown: None,
                        roller_mapping: None,
                        initial_observation: None,
                        outcome: None,
                        firmware: vec![],
                        error: Some(error.clone()),
                        cleanup_error: None,
                    },
                };
                // Preserve old scopes without adopting their state in the UI.
                // This bounded in-memory history is not vendor persistence.
                record_receipt(&history, diagnostic);
                let _ = sender.send(result);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.thx_cleanup
                    .extend(std::mem::take(&mut *predecessor.lock().unwrap()));
                self.thx_retired_owners
                    .extend(std::mem::take(&mut *retired.lock().unwrap()));
                self.status = format!("THX 工作线程启动失败：{error}");
                cx.notify();
                return;
            }
        };
        let owner = identity.clone();
        let pump = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let incoming = receiver.try_recv();
                let keep = this.update_in(cx, |this, window, cx| {
                    if !this
                        .thx_jobs
                        .get(&owner)
                        .is_some_and(|session| session.revision == job_revision)
                    {
                        return false;
                    }
                    let device = workspace.read(cx).device(cx);
                    let phase_current = match &guard {
                        SubmissionGuard::Sound(request) => {
                            this.discovery_revision == discovery_revision
                                && device.active_profile == profile
                                && workspace.read(cx).accepts_help_audio_sound(request, cx)
                        }
                        SubmissionGuard::RenderSetup(epoch) => {
                            this.thx_render_discovery
                                .current_epoch(&epoch.identity)
                                .as_ref()
                                == Some(epoch)
                                && this.observed_thx_connection(device).ok().flatten().as_ref()
                                    == Some(&epoch.identity)
                        }
                    };
                    let current = this.devices.contains(&workspace)
                        && phase_current
                        && workspace.read(cx).identity(cx) == owner
                        && device.serial_number == serial
                        && device.device_container_id == container
                        && this
                            .device_observations
                            .iter()
                            .any(|route| route.matches(device));
                    if !current {
                        this.thx_cleanup
                            .extend(this.thx_jobs.remove(&owner).and_then(Session::retire));
                        return false;
                    }
                    match incoming {
                        Ok(Ok(progress)) => {
                            if progress.scope != scope {
                                this.status = "THX 响应身份不匹配；结果未采用".into();
                            } else {
                                let mut messages = vec![];
                                if let Some(version) = progress.observed_firmware_version()
                                    && let Some(observation) =
                                        workspace.read(cx).help_owner_observation_request(cx)
                                {
                                    let device = workspace.read(cx).snapshot(cx);
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_help_audio_sound(
                                            &device,
                                            &observation,
                                            "currentFWVersion",
                                            json!(version),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                                if let Some(setup) = &progress.render_setup {
                                    messages.extend(setup.warnings.clone());
                                    messages.extend(setup.error.clone());
                                }
                                if let Some(outcome) = &progress.outcome {
                                    messages.extend(outcome.warnings.clone());
                                    messages.extend(outcome.error.clone());
                                }
                                if let Some(mapping) = &progress.roller_mapping {
                                    messages.extend(mapping.error.clone());
                                    messages.extend(mapping.rejected_mappings.clone());
                                    if mapping.source_result == Some(false) {
                                        messages.push(
                                            "旋钮映射写入未获全部成功回执；本地映射已保留".into(),
                                        );
                                    }
                                    if let Some(mappings) = &mapping.source_mappings {
                                        if let Err(error) = razer_storage::source_local::path()
                                            .and_then(|path| {
                                                razer_storage::source_local::set(
                                                    &path,
                                                    &roller_local_key(&scope),
                                                    &json!(mappings),
                                                )
                                            })
                                        {
                                            messages
                                                .push(format!("旋钮映射本地保存失败：{error:#}"));
                                        }
                                        let observation =
                                            workspace.read(cx).help_owner_observation_request(cx);
                                        if let Some(observation) = observation {
                                            let device = workspace.read(cx).snapshot(cx);
                                            workspace.update(cx, |workspace, cx| {
                                                workspace.observe_help_audio_sound(
                                                    &device,
                                                    &observation,
                                                    "mappingList",
                                                    json!(mappings),
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                }
                                messages.extend(progress.error.clone());
                                messages.extend(progress.cleanup_error.clone());
                                // Source MW refresh uses stored profile/runtimeCache, not
                                // GetProperty. Keep actual readback outside the optimistic UI,
                                // including mismatch/failure, without dropping partial ACKs.
                                if matches!(&guard, SubmissionGuard::RenderSetup(_)) {
                                    this.thx_render_progress.insert(owner.clone(), progress);
                                } else {
                                    this.thx_progress.insert(owner.clone(), progress);
                                }
                                // Source failures log and retain optimistic local state. The shell's
                                // existing status carries diagnostics; no new source dialog is invented.
                                if !messages.is_empty() {
                                    this.status = messages.join("；");
                                    eprintln!("[thx-local] {}", this.status);
                                }
                            }
                            this.thx_cleanup
                                .extend(this.thx_jobs.remove(&owner).and_then(Session::retire));
                            cx.notify();
                            return false;
                        }
                        Ok(Err(error)) => {
                            this.status = format!("THX 提交失败：{error}");
                            this.thx_cleanup
                                .extend(this.thx_jobs.remove(&owner).and_then(Session::retire));
                            cx.notify();
                            return false;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            this.status = "THX 工作线程未返回结果；设置状态未确认".into();
                            this.thx_cleanup
                                .extend(this.thx_jobs.remove(&owner).and_then(Session::retire));
                            cx.notify();
                            return false;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    true
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        });
        self.thx_jobs.insert(
            identity,
            Session {
                revision: job_revision,
                render_connection,
                canceled,
                worker: Some(worker),
                _pump: pump,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn help_hardware_reset_waiting_owner_rejects_release_or_cancel_before_reopening_transport() {
        for cancel in [false, true] {
            let scope = ThxSubmissionScope {
                product_id: 1398,
                container_id: "simulated".into(),
                serial_number: "simulated".into(),
                profile_id: "profile".into(),
                generation: 1,
            };
            let owner = Arc::new(RetainedOwner {
                connection: ThxConnectionEpoch {
                    identity: ThxConnectionIdentity::new(
                        1398,
                        "simulated",
                        "simulated",
                        1398,
                        b"simulated-hid".to_vec(),
                    )
                    .unwrap(),
                    epoch: 1,
                },
                releasing: AtomicBool::new(false),
                session: Mutex::new(OwnerClient {
                    client: None,
                    render_started: false,
                    id: "simulated-owner".into(),
                    scope,
                }),
            });
            let signal = Arc::new(AtomicBool::new(false));
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let guard = owner.session.lock().unwrap();
            let waiting = owner.clone();
            let waiting_signal = signal.clone();
            let waiting_barrier = barrier.clone();
            let worker = thread::spawn(move || {
                waiting_barrier.wait();
                waiting.acquire(&waiting_signal).is_err()
            });
            barrier.wait();
            if cancel {
                signal.store(true, Ordering::Release);
            } else {
                owner.releasing.store(true, Ordering::Release);
            }
            drop(guard);
            assert!(worker.join().unwrap());
            assert!(owner.session.lock().unwrap().client.is_none());
        }
    }

    #[::core::prelude::v1::test]
    fn render_failure_keeps_independent_roller_observation_and_cleanup_errors() {
        let scope = ThxSubmissionScope {
            product_id: 1398,
            container_id: "simulated".into(),
            serial_number: "simulated".into(),
            profile_id: "profile".into(),
            generation: 7,
        };
        let plan = ThxSubmissionPlan::SetupRender {
            trigger: ThxRenderSetupTrigger::InitialDeviceSetup,
        };
        let mapping = ThxSubmissionProgress {
            scope: scope.clone(),
            plan: ThxSubmissionPlan::ReadRollerMapping,
            render_setup: None,
            render_shutdown: None,
            roller_mapping: Some(
                razer_device::blackshark_roller_mapping::RollerMappingProgress {
                    observed_index: Some(47),
                    source_mappings: Some(vec![]),
                    ..Default::default()
                },
            ),
            initial_observation: None,
            outcome: None,
            firmware: vec![],
            error: None,
            cleanup_error: Some("simulated cleanup failure".into()),
        };
        let result = merge_local_initialization(
            Err(anyhow::anyhow!("simulated RPC failure")),
            scope.clone(),
            plan.clone(),
            Ok(mapping),
        );
        assert_eq!(result.scope, scope);
        assert_eq!(result.plan, plan);
        assert_eq!(result.error.as_deref(), Some("simulated RPC failure"));
        assert_eq!(
            result.cleanup_error.as_deref(),
            Some("simulated cleanup failure")
        );
        assert_eq!(result.roller_mapping.unwrap().observed_index, Some(47));
        assert!(result.render_setup.is_none());
    }
}
