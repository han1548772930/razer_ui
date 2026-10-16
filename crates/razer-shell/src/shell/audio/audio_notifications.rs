//! Current mounted Audio_StreamMixer AudioEnumerator owner and worker lifetime.
use super::*;
use anyhow::{Context as _, ensure};
use serde_json::Value;
#[path = "audio_notifications_worker.rs"]
mod worker;
use razer_device::audio_notification::AudioNotification;
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use worker::Command;
use worker::Update;

fn source_product(product_id: u32) -> bool {
    // Only these current startup roots register audio_streamMixer. The same
    // class occurs in other bundles without being activated by their roots.
    // See audio-util-active-consumers-current.json, independently of DLL version.
    matches!(product_id, 1422 | 1446)
}

fn broadcast_shortcut_state(
    current: &mut bool,
    enabled: bool,
    owners: impl IntoIterator<Item = Sender<Command>>,
) -> Vec<String> {
    if *current == enabled {
        return Vec::new();
    }
    *current = enabled;
    // Retain intent even with no Mixer mounted. A future owner starts with it.
    owners
        .into_iter()
        .filter_map(|owner| {
            owner
                .send(Command::GlobalShortcutsEnabled(enabled))
                .err()
                .map(|error| error.to_string())
        })
        .collect()
}

pub(super) struct Session {
    stop: Sender<Command>,
    worker: Option<JoinHandle<()>>,
    _status: Task<()>,
    route_key: String,
    view_id: u64,
    live: Arc<AtomicBool>,
    request_guards: Mutex<Vec<Arc<AtomicBool>>>,
}
pub(super) struct Retired {
    identity: String,
    worker: JoinHandle<()>,
}
fn next_view() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x1_ffff_0500);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
fn route_key(route: &razer_discovery::discovery::ObservedDevice) -> String {
    format!(
        "{}:{}:{}:{:?}:{:?}:{:?}",
        route.product_id(),
        route.physical_product_id(),
        route.container(),
        route.hid_node(),
        route.peer_product_id(),
        route.transport()
    )
}

impl Drop for Session {
    fn drop(&mut self) {
        // Wake the owned thread immediately. Never block the GPUI thread on IPC.
        self.cancel();
    }
}

impl Session {
    fn retain_guard(&self, guard: Arc<AtomicBool>) {
        let mut guards = self
            .request_guards
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        guards.retain(|guard| guard.load(Ordering::Acquire));
        if !guards.iter().any(|current| Arc::ptr_eq(current, &guard)) {
            guards.push(guard);
        }
    }
    fn cancel(&self) {
        self.live.store(false, Ordering::Release);
        for guard in self
            .request_guards
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
        {
            guard.store(false, Ordering::Release);
        }
        let _ = self.stop.send(Command::Stop);
    }
    fn mixer(
        &self,
        request: razer_pages::features::SeirenStreamMixerRequested,
    ) -> anyhow::Result<Receiver<anyhow::Result<worker::MixerReceipt>>> {
        self.retain_guard(request.guard());
        let (reply, receiver) = mpsc::channel();
        self.stop
            .send(Command::Mixer { request, reply })
            .context("Audio_StreamMixer owner stopped")?;
        Ok(receiver)
    }
    fn capture(
        &self,
        request: razer_pages::features::SeirenStreamMixerCaptureChanged,
    ) -> anyhow::Result<()> {
        self.retain_guard(request.guard());
        self.stop
            .send(Command::Capture(request))
            .context("Audio_StreamMixer owner stopped")
    }
    pub(super) fn request(
        &self,
        request: razer_pages::features::SeirenEqRequested,
    ) -> anyhow::Result<Receiver<anyhow::Result<razer_device::seiren_eq::Report>>> {
        self.retain_guard(request.guard());
        let (reply, receiver) = mpsc::channel();
        self.stop
            .send(Command::Eq { request, reply })
            .context("Audio_StreamMixer owner stopped")?;
        Ok(receiver)
    }
    fn retire(mut self, identity: String) -> Option<Retired> {
        self.cancel();
        self.worker
            .take()
            .map(|worker| Retired { identity, worker })
    }

    fn finish(mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn remove_view(storage: &mut razer_storage::host::HostStorage, view_id: u64) {
    use razer_storage::host::{HostStorageCall, HostStoreKind};
    let _ = storage.call(HostStorageCall {
        store: HostStoreKind::Window,
        sender_id: view_id,
        action: "removeWindowStorageItem".into(),
        payload: serde_json::json!({"key":razer_device::seiren_stream_mixer::ACTIVE_KEY}),
        target_url_array: vec![],
    });
    storage.close_view(view_id);
}

fn enabled(client: &mut ServiceClient, enable: bool) -> anyhow::Result<()> {
    let reply = client.request(ServiceRequest::AudioNotificationsEnable { enable })?;
    ensure!(
        reply["response"]["enabled"].as_i64() == Some(i64::from(enable)),
        "AudioEnumerator EnableNotification 响应与请求不匹配"
    );
    Ok(())
}

impl AppShell {
    /// yn/Rr capture and RECORDING.enablingMapping target one source hotkey
    /// service. Forward those operations to the retained Rust Mixer owners.
    pub(super) fn set_audio_global_shortcuts_enabled(&mut self, enabled: bool) {
        let owners = self
            .audio_notifications
            .values()
            .map(|session| session.stop.clone());
        for error in
            broadcast_shortcut_state(&mut self.audio_global_shortcuts_enabled, enabled, owners)
        {
            eprintln!("[Audio_StreamMixer] shortcut owner closed: {error}");
        }
    }

    pub(super) fn install_audio_notification_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let sessions = std::mem::take(&mut this.audio_notifications);
            let retired = std::mem::take(&mut this.audio_notification_cleanup);
            // All retained workers begin teardown concurrently; sequential
            // joining must not defer the next product's disable request.
            for session in sessions.values() {
                session.cancel();
                this.cancel_profile_notification_sender(session.view_id);
                remove_view(&mut this.host_storage, session.view_id);
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, session) in sessions {
                    session.finish();
                }
                for worker in retired {
                    let _ = worker.worker.join();
                }
            });
            async move { cleanup.await }
        }));
    }

    pub(super) fn sync_audio_notifications(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let desired = self.devices.iter().filter_map(|workspace| {
            let workspace = workspace.read(cx);
            let device = workspace.device(cx);
            if !source_product(device.product_id) { return None; }
            let mut routes = self.device_observations.iter().filter(|route| route.matches(device));
            let route = routes.next().cloned();
            let route = route.filter(|_| routes.next().is_none());
            // A receiver-presence observation alone does not initialize a peer product.
            let route=route.filter(|route| !matches!(route.connection(), razer_model::model::DeviceConnectionObservation::ReceiverPeer(status) if status != 1));
            let rate=device.source_device_settings.as_ref().and_then(|value|value["samplingRate"].as_u64()).unwrap_or(48_000) as u32;
            Some((workspace.identity(cx), (device.product_id,route,rate)))
        }).collect::<BTreeMap<_, _>>();
        let removed = self
            .audio_notifications
            .keys()
            .filter(|key| {
                desired.get(*key).is_none_or(|(_, route, _)| {
                    self.audio_notifications.get(*key).is_none_or(|owner| {
                        owner.route_key
                            != route
                                .as_ref()
                                .map(route_key)
                                .unwrap_or_else(|| "local".into())
                    })
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in removed {
            if let Some(session) = self.audio_notifications.remove(&key) {
                self.cancel_profile_notification_sender(session.view_id);
                remove_view(&mut self.host_storage, session.view_id);
                if let Some(worker) = session.retire(key) {
                    self.audio_notification_cleanup.push(worker);
                }
            }
        }
        // Joining only finished threads keeps teardown ownership without
        // blocking input while COM unregister or an IPC shutdown is pending.
        let mut pending = Vec::new();
        for worker in std::mem::take(&mut self.audio_notification_cleanup) {
            if worker.worker.is_finished() {
                let _ = worker.worker.join();
            } else {
                pending.push(worker);
            }
        }
        self.audio_notification_cleanup = pending;
        for (identity, (product_id, observation, rate)) in desired {
            let workspace = self
                .devices
                .iter()
                .find(|workspace| workspace.read(cx).identity(cx) == identity)
                .cloned()
                .expect("retained audio workspace");
            if self.audio_notifications.contains_key(&identity)
                || self
                    .audio_notification_cleanup
                    .iter()
                    .any(|retired| retired.identity == identity)
            {
                continue;
            }
            let (stop, stop_rx) = mpsc::channel();
            let (status_tx, status_rx) = mpsc::channel();
            let live = Arc::new(AtomicBool::new(true));
            let worker_live = live.clone();
            let route_key = observation
                .as_ref()
                .map(route_key)
                .unwrap_or_else(|| "local".into());
            let view_id = next_view();
            let view_name = format!("razer-local:audio:{identity}");
            let shortcuts_enabled = self.audio_global_shortcuts_enabled;
            self.host_storage
                .register_view(razer_storage::host::HostStorageView {
                    id: view_id,
                    url: view_name.clone(),
                    remote: false,
                    destroyed: false,
                    crashed: false,
                    disposed: false,
                    tracked_url: true,
                });
            let worker = match thread::Builder::new()
                .name(format!("razer-audio-{product_id}"))
                .spawn(move || {
                    if let Err(error) = worker::run(
                        stop_rx,
                        status_tx.clone(),
                        observation,
                        product_id,
                        rate,
                        view_id,
                        view_name,
                        worker_live,
                        shortcuts_enabled,
                    ) {
                        worker::report(
                            &status_tx,
                            format!("AudioEnumerator 初始化或通知读取失败：{error:#}"),
                        );
                    }
                }) {
                Ok(worker) => worker,
                Err(error) => {
                    remove_view(&mut self.host_storage, view_id);
                    window.push_notification(
                        format!("AudioEnumerator 工作线程启动失败：{error}"),
                        cx,
                    );
                    continue;
                }
            };
            let owner = identity.clone();
            let status = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
                        .await;
                    let mut messages = Vec::new();
                    let disconnected = loop {
                        match status_rx.try_recv() {
                            Ok(message) => messages.push(message),
                            Err(mpsc::TryRecvError::Empty) => break false,
                            Err(mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    let current = this.update_in(cx, |this, window, cx| {
                        if !this
                            .audio_notifications
                            .get(&owner)
                            .is_some_and(|session| session.view_id == view_id)
                        {
                            return false;
                        }
                        for message in messages {
                            match message {
                                Update::Diagnostic(message) => {
                                    this.status = message.clone();
                                    window.push_notification(message, cx);
                                }
                                Update::Ready(ready) => {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_seiren_sampling_ready(ready, cx)
                                    });
                                }
                                Update::Native(event) => {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_seiren_native_event(event, window, cx)
                                    });
                                }
                                Update::Host(call, reply) => {
                                    let result = this
                                        .host_storage
                                        .call(call)
                                        .map_err(|error| format!("{error:#}"));
                                    let _ = reply.send(result);
                                }
                                Update::Mixer(observation) => {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.observe_seiren_stream_mixer(
                                            observation,
                                            window,
                                            cx,
                                        )
                                    });
                                }
                                Update::CaptureMode(request, enabled) => {
                                    if workspace
                                        .read(cx)
                                        .seiren_stream_mixer_capture_matches(&request, cx)
                                    {
                                        workspace.update(cx, |workspace, cx| {
                                            workspace.observe_seiren_stream_mixer_mode(enabled, cx)
                                        });
                                    }
                                }
                                Update::DeviceVersion(request, reply) => {
                                    this.persist_seiren_device_version(
                                        workspace.clone(),
                                        request,
                                        reply,
                                        window,
                                        cx,
                                    );
                                }
                                Update::ProfileNotification {
                                    sender_id,
                                    product_id,
                                    container_id,
                                    preset_name,
                                    timer_tick,
                                } => {
                                    let edition = workspace.read(cx).device(cx).edition_id;
                                    let locale = razer_i18n::locale().to_ascii_lowercase();
                                    let result = razer_device::seiren_stream_mixer::product_name(
                                        product_id, edition, &locale,
                                    )
                                    .map(str::to_owned)
                                    .and_then(|product_name| {
                                        this.publish_stream_mixer_profile_notification(
                                            sender_id,
                                            container_id,
                                            product_name,
                                            preset_name,
                                            timer_tick,
                                            window,
                                            cx,
                                        )
                                    });
                                    if let Err(error) = result {
                                        this.status =
                                            format!("Mixer profile notification: {error:#}");
                                    }
                                }
                            }
                            cx.notify();
                        }
                        let sessions = this
                            .audio_notifications
                            .values()
                            .map(|session| (session.view_id, session.stop.clone()))
                            .collect::<Vec<_>>();
                        for (id, command) in sessions {
                            if let Ok(events) = this.host_storage.drain_events(id) {
                                let events = events
                                    .into_iter()
                                    .filter(|event| event.channel == "windowStorageEvent")
                                    .map(|event| event.payload)
                                    .collect::<Vec<_>>();
                                if !events.is_empty() {
                                    let _ = command.send(Command::HostEvents(events));
                                }
                            }
                        }
                        true
                    });
                    if disconnected || !matches!(current, Ok(true)) {
                        break;
                    }
                }
            });
            self.audio_notifications.insert(
                identity,
                Session {
                    stop,
                    worker: Some(worker),
                    _status: status,
                    route_key,
                    view_id,
                    live,
                    request_guards: Mutex::new(Vec::new()),
                },
            );
        }
    }
    pub(super) fn request_seiren_stream_mixer(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: razer_pages::features::SeirenStreamMixerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !workspace
                .read(cx)
                .seiren_stream_mixer_request_matches(&request, cx)
        {
            return;
        }
        self.sync_audio_notifications(window, cx);
        let identity = workspace.read(cx).identity(cx);
        let receiver = self
            .audio_notifications
            .get(&identity)
            .context("Mixer owner is still cleaning up")
            .and_then(|owner| owner.mixer(request.clone()));
        self.seiren_eq_workers += 1;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    receiver?
                        .recv()
                        .context("Mixer owner closed before operation receipt")?
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.seiren_eq_workers = this.seiren_eq_workers.saturating_sub(1);
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace
                        .read(cx)
                        .seiren_stream_mixer_request_matches(&request, cx);
                if current {
                    let error = match result {
                        Ok(worker::MixerReceipt::LocalSaved) => {
                            this.status =
                                "混音器设置已保存到本机；当前设备未连接，未写入设备。".into();
                            None
                        }
                        Ok(worker::MixerReceipt::DeviceSubmitted) => None,
                        Err(error) => Some(format!("混音器操作尚未确认：{error:#}")),
                    };
                    if let Some(message) = &error {
                        this.status = message.clone();
                    }
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_seiren_stream_mixer(&request, error, window, cx)
                    });
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn set_seiren_stream_mixer_capture(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: razer_pages::features::SeirenStreamMixerCaptureChanged,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !workspace
                .read(cx)
                .seiren_stream_mixer_capture_matches(&request, cx)
        {
            return;
        }
        self.sync_audio_notifications(window, cx);
        let identity = workspace.read(cx).identity(cx);
        let result = self
            .audio_notifications
            .get(&identity)
            .context("当前混音器未连接本地音频服务")
            .and_then(|owner| owner.capture(request));
        if let Err(error) = result {
            self.status = format!("混音器快捷键捕获尚未启动：{error:#}");
            cx.notify();
        }
    }
}

#[cfg(test)]
mod audio_shortcut_lifecycle_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn capture_before_mixer_mount_is_retained_and_focus_transitions_reach_every_owner() {
        let mut enabled = true;
        assert!(broadcast_shortcut_state(&mut enabled, false, []).is_empty());
        assert!(!enabled); // sync_audio_notifications passes this to new workers.
        let (first, first_events) = mpsc::channel();
        let (second, second_events) = mpsc::channel();
        assert!(
            broadcast_shortcut_state(&mut enabled, true, [first.clone(), second.clone()])
                .is_empty()
        );
        assert!(matches!(
            first_events.try_recv(),
            Ok(Command::GlobalShortcutsEnabled(true))
        ));
        assert!(matches!(
            second_events.try_recv(),
            Ok(Command::GlobalShortcutsEnabled(true))
        ));
        assert!(
            broadcast_shortcut_state(&mut enabled, false, [first.clone(), second.clone()])
                .is_empty()
        );
        assert!(matches!(
            first_events.try_recv(),
            Ok(Command::GlobalShortcutsEnabled(false))
        ));
        assert!(matches!(
            second_events.try_recv(),
            Ok(Command::GlobalShortcutsEnabled(false))
        ));
        broadcast_shortcut_state(&mut enabled, false, [first, second]);
        assert!(first_events.try_recv().is_err());
        assert!(second_events.try_recv().is_err());
    }

    #[::core::prelude::v1::test]
    fn closing_one_mixer_does_not_prevent_the_other_from_resuming() {
        let mut enabled = false;
        let (closed, gone) = mpsc::channel();
        drop(gone);
        let (live, events) = mpsc::channel();
        let errors = broadcast_shortcut_state(&mut enabled, true, [closed, live]);
        assert_eq!(errors.len(), 1);
        assert!(enabled);
        assert!(matches!(
            events.try_recv(),
            Ok(Command::GlobalShortcutsEnabled(true))
        ));
    }
}
