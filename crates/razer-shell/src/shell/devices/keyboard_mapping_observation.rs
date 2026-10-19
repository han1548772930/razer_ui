//! Mounted mapping forms own real audio reads and host memory subscriptions.
use super::{MappingTargetOwner, *};
use anyhow::ensure;
use razer_device::simple_audio::{EndpointType, SimpleAudioEndpoint};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::keyboard_products::{
    KeyboardMappingBrightnessObservation, KeyboardMappingBrightnessReadRequested,
    KeyboardMappingDialObservation, KeyboardMappingDialReadRequested,
};
use razer_storage::host::{HostStorageCall, HostStorageView, HostStoreKind};
use serde::Deserialize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Clone)]
enum Request {
    Brightness(KeyboardMappingBrightnessReadRequested),
    Dial(KeyboardMappingDialReadRequested),
}
impl Request {
    fn accepts(&self, workspace: &Entity<ProductWorkspace>, cx: &App) -> bool {
        match self {
            Self::Brightness(request) => workspace
                .read(cx)
                .accepts_keyboard_mapping_brightness(request, cx),
            Self::Dial(request) => workspace
                .read(cx)
                .accepts_keyboard_mapping_dial(request, cx),
        }
    }
    fn slot(&self) -> u8 {
        match self {
            Self::Brightness(_) => 0,
            Self::Dial(_) => 1,
        }
    }
}
struct Observer {
    connection: MappingTargetOwner,
    request: Request,
    device: Device,
    guard: Arc<AtomicBool>,
    view_id: u64,
    subscribed: bool,
    playback: Option<Vec<SimpleAudioEndpoint>>,
    lighting: Option<bool>,
}
impl Observer {
    fn current(&self, workspaces: &[Entity<ProductWorkspace>], cx: &App) -> bool {
        self.guard.load(Ordering::Acquire)
            && workspaces.iter().any(|workspace| {
                self.connection.matches(workspace, cx) && self.request.accepts(workspace, cx)
            })
    }
    fn publish(&self, window: &mut Window, cx: &mut App) {
        let Some(workspace) = self.connection.workspace.upgrade() else {
            return;
        };
        if !self.connection.matches(&workspace, cx) || !self.request.accepts(&workspace, cx) {
            return;
        }
        workspace.update(cx, |workspace, cx| match &self.request {
            Request::Brightness(request) => {
                if let Some(has_led_devices) = self.lighting {
                    workspace.observe_keyboard_mapping_brightness(
                        &self.device,
                        KeyboardMappingBrightnessObservation {
                            product_id: request.product_id,
                            profile_guid: request.profile_guid.clone(),
                            owner: request.owner,
                            generation: request.generation,
                            has_led_devices,
                        },
                        window,
                        cx,
                    );
                }
            }
            Request::Dial(request) => {
                workspace.observe_keyboard_mapping_dial(
                    &self.device,
                    KeyboardMappingDialObservation {
                        product_id: request.product_id,
                        profile_guid: request.profile_guid.clone(),
                        owner: request.owner,
                        generation: request.generation,
                        playback_devices: self.playback.clone(),
                        // commonReducer.validDevices has an independent producer;
                        // connected catalogue entries are not that observation.
                        valid_devices: None,
                        lighting_engine_has_devices: self.lighting,
                    },
                    window,
                    cx,
                );
            }
        });
    }
}
struct Operation {
    guard: Arc<AtomicBool>,
    _delivery: Task<()>,
}
#[derive(Default)]
pub(in crate::shell) struct ObservationState {
    observers: BTreeMap<(EntityId, u8), Observer>,
    operations: BTreeMap<u64, Operation>,
    watch: Option<Task<()>>,
    accepted: Arc<AtomicUsize>,
    stopped: bool,
}
impl Drop for ObservationState {
    fn drop(&mut self) {
        for observer in self.observers.values() {
            observer.guard.store(false, Ordering::Release);
        }
        for operation in self.operations.values() {
            operation.guard.store(false, Ordering::Release);
        }
    }
}
struct Accepted(Arc<AtomicUsize>);
impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0x1_ffff_1600);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn decode_playback(value: Value) -> anyhow::Result<Vec<SimpleAudioEndpoint>> {
    let endpoints: Vec<SimpleAudioEndpoint> = serde_json::from_value(value)?;
    let mut ids = std::collections::BTreeSet::new();
    ensure!(
        endpoints.len() <= 65536,
        "Audio endpoint inventory exceeds its source bound"
    );
    for endpoint in &endpoints {
        ensure!(
            !endpoint.id.is_empty() && ids.insert(&endpoint.id),
            "Audio endpoint identity is missing or duplicated"
        );
    }
    Ok(endpoints
        .into_iter()
        .filter(|endpoint| endpoint.endpoint_type == EndpointType::Speaker)
        .collect())
}
struct AudioReadback {
    endpoints: anyhow::Result<Vec<SimpleAudioEndpoint>>,
    cleanup_error: Option<String>,
}
fn read_playback(guard: Arc<AtomicBool>) -> AudioReadback {
    let started = (|| {
        ensure!(
            guard.load(Ordering::Acquire),
            "Mapping audio observation was canceled"
        );
        ServiceClient::spawn()
    })();
    let mut client = match started {
        Ok(client) => client,
        Err(error) => {
            return AudioReadback {
                endpoints: Err(error),
                cleanup_error: None,
            };
        }
    };
    client.set_request_guard(guard.clone());
    let endpoints = (|| {
        ensure!(
            guard.load(Ordering::Acquire),
            "Mapping audio observation was canceled"
        );
        decode_playback(client.request(ServiceRequest::AudioDevices)?)
    })();
    // ServiceClient requires ShutdownReceipt.stopped=true and completes its
    // response-side process join before returning this result.
    let mut cleanup = client
        .request(ServiceRequest::Shutdown)
        .err()
        .map(|error| format!("Shutdown: {error:#}"))
        .into_iter()
        .collect::<Vec<_>>();
    if let Err(error) = client.finish_termination() {
        cleanup.push(format!("process/pipe cleanup: {error:#}"));
    }
    drop(client);
    let cleanup_error = (!cleanup.is_empty()).then(|| cleanup.join("; "));
    AudioReadback {
        endpoints,
        cleanup_error,
    }
}

impl AppShell {
    pub(in crate::shell) fn install_mapping_observation_cleanup(
        &mut self,
        _cx: &mut Context<Self>,
    ) {
        self.register_exit_cleanup(|shell, _| {
            shell.prepare_mapping_observation_exit();
            let deliveries = std::mem::take(&mut shell.mapping_observations.operations)
                .into_values()
                .map(|operation| operation._delivery)
                .collect::<Vec<_>>();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
        self.register_exit_resume(|shell, _| shell.resume_mapping_observations());
    }
    pub(in crate::shell) fn mapping_observation_pending(&self) -> bool {
        !self.mapping_observations.operations.is_empty()
            || self.mapping_observations.accepted.load(Ordering::Acquire) > 0
    }
    pub(in crate::shell) fn prepare_mapping_observation_exit(&mut self) {
        self.mapping_observations.stopped = true;
        for observer in std::mem::take(&mut self.mapping_observations.observers).into_values() {
            observer.guard.store(false, Ordering::Release);
            self.host_storage.close_view(observer.view_id);
        }
        for operation in self.mapping_observations.operations.values() {
            operation.guard.store(false, Ordering::Release);
        }
        self.mapping_observations.watch = None;
    }
    pub(in crate::shell) fn resume_mapping_observations(&mut self) {
        self.mapping_observations.stopped = false;
    }
    pub(in crate::shell) fn retain_mapping_observation_connections(
        &mut self,
        retained: &[Entity<ProductWorkspace>],
        cx: &App,
    ) {
        let removed = self
            .mapping_observations
            .observers
            .iter()
            .filter_map(|(key, observer)| (!observer.current(retained, cx)).then_some(*key))
            .collect::<Vec<_>>();
        for key in removed {
            if let Some(observer) = self.mapping_observations.observers.remove(&key) {
                observer.guard.store(false, Ordering::Release);
                self.host_storage.close_view(observer.view_id);
            }
        }
    }
    pub(in crate::shell) fn read_keyboard_mapping_brightness(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &KeyboardMappingBrightnessReadRequested,
        cx: &mut Context<Self>,
    ) {
        self.start_mapping_observation(workspace, Request::Brightness(request.clone()), cx);
    }
    pub(in crate::shell) fn read_keyboard_mapping_dial(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &KeyboardMappingDialReadRequested,
        cx: &mut Context<Self>,
    ) {
        self.start_mapping_observation(workspace, Request::Dial(request.clone()), cx);
    }
    fn start_mapping_observation(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: Request,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || self.mapping_observations.stopped
            || !self.devices.contains(workspace)
            || !request.accepts(workspace, cx)
        {
            return;
        }
        let key = (workspace.entity_id(), request.slot());
        let connection = MappingTargetOwner::capture(workspace, cx);
        let device = workspace.read(cx).device(cx).clone();
        let guard = Arc::new(AtomicBool::new(true));
        let view_id = next_id();
        let dial = matches!(&request, Request::Dial(_));
        if let Some(previous) = self.mapping_observations.observers.insert(
            key,
            Observer {
                connection,
                request,
                device,
                guard: guard.clone(),
                view_id,
                subscribed: false,
                playback: None,
                lighting: None,
            },
        ) {
            previous.guard.store(false, Ordering::Release);
            self.host_storage.close_view(previous.view_id);
        }
        self.start_mapping_memory_watch(cx);
        if dial {
            self.mapping_observations
                .accepted
                .fetch_add(1, Ordering::AcqRel);
            let accepted = Accepted(self.mapping_observations.accepted.clone());
            let signal = guard.clone();
            let delivery = cx.spawn(async move |shell, cx| {
                let background_guard = signal.clone();
                let result = cx
                    .background_spawn(async move {
                        let _accepted = accepted;
                        read_playback(background_guard)
                    })
                    .await;
                let _ =
                    shell.update(cx, |shell, cx| {
                        shell.mapping_observations.operations.remove(&view_id);
                        let current = shell.mapping_observations.observers.get(&key).is_some_and(
                            |observer| {
                                observer.view_id == view_id && observer.current(&shell.devices, cx)
                            },
                        );
                        // Cleanup diagnostics are consumed even after the form closes.
                        if let Some(error) = &result.cleanup_error {
                            shell.status = format!("键盘旋钮音频通信未正常结束：{error}");
                        }
                        if current {
                            match result.endpoints {
                                Ok(playback) => {
                                    shell
                                        .mapping_observations
                                        .observers
                                        .get_mut(&key)
                                        .expect("Current mapping observer")
                                        .playback = Some(playback);
                                }
                                Err(error) => {
                                    shell.status = format!("键盘旋钮音频设备未读取：{error:#}");
                                }
                            }
                            shell.subscribe_mapping_memory(key, cx);
                        }
                        if shell.close_requested && shell.save_task.is_none() {
                            shell.continue_save_queue(true, cx);
                        }
                        cx.notify();
                    });
            });
            self.mapping_observations.operations.insert(
                view_id,
                Operation {
                    guard,
                    _delivery: delivery,
                },
            );
        } else {
            self.subscribe_mapping_memory(key, cx);
        }
    }
    fn publish_mapping_observation(&mut self, key: (EntityId, u8), cx: &mut Context<Self>) {
        if let Some(observer) = self.mapping_observations.observers.get(&key)
            && let Err(error) = self
                .main_window
                .update(cx, |_, window, cx| observer.publish(window, cx))
        {
            self.status = format!("键盘映射观察未发布：{error}");
        }
    }
    fn subscribe_mapping_memory(&mut self, key: (EntityId, u8), cx: &mut Context<Self>) {
        let Some(observer) = self.mapping_observations.observers.get_mut(&key) else {
            return;
        };
        self.host_storage.register_view(HostStorageView {
            id: observer.view_id,
            url: format!("/synapse/products/{}/ui/", observer.device.product_id),
            remote: false,
            destroyed: false,
            crashed: false,
            disposed: false,
            tracked_url: true,
        });
        let result = super::super::product_chroma::connected_products(
            &mut self.host_storage,
            observer.view_id,
        );
        let subscribed = self.host_storage.call(HostStorageCall {
            store: HostStoreKind::Memory,
            sender_id: observer.view_id,
            action: "registerMemoryStorageEvent".into(),
            payload: serde_json::json!({}),
            target_url_array: Vec::new(),
        });
        observer.subscribed = subscribed.is_ok();
        if let Err(error) = subscribed {
            self.status = format!("键盘映射灯光事件未订阅：{error:#}");
        }
        match result {
            Ok(devices) => {
                observer.lighting = devices.map(|devices| !devices.is_empty());
            }
            Err(error) => {
                self.status = format!("键盘映射灯光设备未读取：{error:#}");
            }
        }
        self.publish_mapping_observation(key, cx);
        self.start_mapping_memory_watch(cx);
    }
    fn start_mapping_memory_watch(&mut self, cx: &mut Context<Self>) {
        if self.mapping_observations.watch.is_some() {
            return;
        }
        self.mapping_observations.watch = Some(cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let active = shell.update(cx, |shell, cx| {
                    let workspaces = shell.devices.clone();
                    shell.retain_mapping_observation_connections(&workspaces, cx);
                    if shell.close_requested || shell.mapping_observations.observers.is_empty() {
                        return false;
                    }
                    let keys = shell
                        .mapping_observations
                        .observers
                        .keys()
                        .copied()
                        .collect::<Vec<_>>();
                    for key in keys {
                        let observer = shell
                            .mapping_observations
                            .observers
                            .get_mut(&key)
                            .expect("Retained observer key");
                        if !observer.subscribed {
                            continue;
                        }
                        let events = match shell.host_storage.drain_events(observer.view_id) {
                            Ok(events) => events,
                            Err(error) => {
                                shell.status = format!("键盘映射灯光事件未读取：{error:#}");
                                continue;
                            }
                        };
                        let mut changed = false;
                        for event in events {
                            if event.channel != "memoryStorageEvent" {
                                continue;
                            }
                            #[derive(Deserialize)]
                            struct MemoryEvent {
                                key: String,
                                #[serde(flatten)]
                                fields: serde_json::Map<String, Value>,
                            }
                            #[derive(Deserialize)]
                            #[serde(rename_all = "camelCase")]
                            struct DeviceEvent {
                                new_value: String,
                            }
                            let decoded = serde_json::from_value::<MemoryEvent>(event.payload)
                                .and_then(|event| {
                                    if event.key
                                        != super::super::product_chroma::CONNECTED_DEVICES_KEY
                                    {
                                        return Ok(None);
                                    }
                                    serde_json::from_value::<DeviceEvent>(Value::Object(
                                        event.fields,
                                    ))
                                    .map(Some)
                                });
                            let result = match decoded {
                                Ok(None) => continue,
                                Ok(Some(event)) => {
                                    super::super::product_chroma::decode_connected_devices_value(
                                        &event.new_value,
                                    )
                                    .map(|devices| !devices.is_empty())
                                }
                                Err(error) => Err(error.into()),
                            };
                            match result {
                                Ok(has_devices) => {
                                    observer.lighting = Some(has_devices);
                                    changed = true;
                                }
                                Err(error) => {
                                    observer.lighting = None;
                                    shell.status = format!("键盘映射灯光设备未读取：{error:#}");
                                }
                            }
                        }
                        if changed {
                            shell.publish_mapping_observation(key, cx);
                        }
                    }
                    true
                });
                if !matches!(active, Ok(true)) {
                    break;
                }
            }
            let _ = shell.update(cx, |shell, _| shell.mapping_observations.watch = None);
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::decode_playback;
    use serde_json::json;
    #[core::prelude::v1::test]
    fn keyboard_mapping_audio_inventory_preserves_real_order_and_rejects_missing_or_unknown_records()
     {
        let speaker =
            |id: &str| json!({"id":id,"name":id,"containerId":"{source}","type":"speaker"});
        let rows = decode_playback(json!([speaker("second"), {"id":"mic","name":"Microphone","containerId":"{mic}","type":"microphone"},speaker("first")])).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["second", "first"]
        );
        assert!(decode_playback(json!([speaker("same"), speaker("same")])).is_err());
        assert!(decode_playback(json!([{"id":"one","name":"one","type":"speaker"}])).is_err());
        assert!(
            decode_playback(json!([{"id":"one","name":"one","containerId":"c","type":"unknown"}]))
                .is_err()
        );
        assert!(decode_playback(json!([])).unwrap().is_empty());
    }
}
