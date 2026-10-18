//! Retain local observation work through cancellation and background completion.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::mouse_brightness::Submission;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_pages::features::keyboard_products::KeyboardLightingHostObservation;
use razer_storage::host::HostStorageView;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Clone)]
enum Request {
    Keyboard(KeyboardLightingRequested),
    Mouse(MouseLightingRequested),
}
impl Request {
    fn guard(&self) -> Arc<AtomicBool> {
        match self {
            Self::Keyboard(request) => request.guard(),
            Self::Mouse(request) => request.guard(),
        }
    }
    fn accepts(&self, workspace: &Entity<ProductWorkspace>, cx: &App) -> bool {
        match self {
            Self::Keyboard(request) => workspace.read(cx).accepts_keyboard_lighting(request, cx),
            Self::Mouse(request) => workspace
                .read(cx)
                .mouse_lighting_request_matches(request, cx),
        }
    }
    fn product_id(&self) -> u32 {
        match self {
            Self::Keyboard(request) => request.owner.product_id,
            Self::Mouse(request) => request.product_id(),
        }
    }
    fn abandon(&self, workspace: &Entity<ProductWorkspace>, error: Option<String>, cx: &mut App) {
        workspace.update(cx, |workspace, cx| match self {
            Self::Keyboard(request) => workspace.abandon_keyboard_lighting(request, error, cx),
            Self::Mouse(request) => workspace.abandon_mouse_lighting(request, error, cx),
        });
    }
    fn reject(
        &self,
        workspace: &Entity<ProductWorkspace>,
        error: String,
        window: &mut Window,
        cx: &mut App,
    ) {
        workspace.update(cx, |workspace, cx| match self {
            Self::Keyboard(request) => {
                workspace.complete_keyboard_lighting(request, Err(error), window, cx)
            }
            Self::Mouse(request) => {
                workspace.complete_mouse_lighting(request, Err(error), window, cx)
            }
        });
    }
    fn reject_at(
        &self,
        workspace: &Entity<ProductWorkspace>,
        error: String,
        handle: AnyWindowHandle,
        cx: &mut App,
    ) {
        if handle
            .update(cx, |_, window, cx| {
                self.reject(workspace, error.clone(), window, cx)
            })
            .is_err()
        {
            self.abandon(workspace, Some(error), cx);
        }
    }
}

#[derive(Clone)]
struct Scope {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
    route: Option<ObservedDevice>,
}
impl Scope {
    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && workspace.read(cx).connection_revision() == self.connection_revision
            && self.route.as_ref().is_none_or(|route| {
                super::super::device_discovery::retained_connection(
                    std::slice::from_ref(route),
                    observations,
                    workspace.read(cx).device(cx),
                ) && direct::mouse_brightness::available(route)
            })
    }
}
struct Operation {
    scope: Scope,
    request: Request,
    canceled: Arc<AtomicBool>,
    delivery: Task<()>,
}
#[derive(Default)]
pub(in crate::shell) struct State {
    operations: BTreeMap<u64, Operation>,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    stopped: bool,
}
struct Accepted(Arc<AtomicUsize>);
impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl State {
    pub(in crate::shell) fn pending(&self) -> bool {
        !self.operations.is_empty() || self.accepted.load(Ordering::Acquire) > 0
    }
    fn accept(&mut self, closing: bool, persisted: bool) -> Option<(u64, Accepted)> {
        if self.stopped || closing && !persisted {
            return None;
        }
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("Product lighting operation identity exhausted");
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some((self.next_id, Accepted(self.accepted.clone())))
    }
    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        std::mem::take(&mut self.operations)
            .into_values()
            .map(|operation| {
                operation.canceled.store(true, Ordering::Release);
                operation.delivery
            })
            .collect()
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() {
            operation.canceled.store(true, Ordering::Release);
        }
    }
}

fn keyboard_host(readback: &product_chroma::Readback) -> KeyboardLightingHostObservation {
    let mut host = KeyboardLightingHostObservation::default();
    if let Ok(snapshot) = &readback.snapshot {
        if let Some(configuration) = snapshot.configuration() {
            host = host.with_configuration(
                configuration
                    .profiles()
                    .iter()
                    .map(|profile| json!({"id":profile.id(), "name":profile.name()}))
                    .collect(),
                configuration.selected_profile().map(str::to_owned),
                configuration.enabled(),
            );
        }
        if let Some(installed) = snapshot.module_installed() {
            host = host.with_module_installed(installed);
        }
        if let Some(enabled) = snapshot.dynamic_lighting() {
            host = host.with_dynamic_lighting(enabled);
        }
    }
    if let Ok(installation) = &readback.installation {
        host = host.with_fully_installed(installation.fully_installed());
    }
    host
}
fn mouse_host(readback: &product_chroma::Readback) -> Value {
    let mut host = Map::new();
    if let Ok(snapshot) = &readback.snapshot {
        if let Some(configuration) = snapshot.configuration() {
            host.insert(
                "chromaProfiles".into(),
                json!(
                    configuration
                        .profiles()
                        .iter()
                        .map(|profile| json!({"id":profile.id(),"name":profile.name()}))
                        .collect::<Vec<_>>()
                ),
            );
            host.insert(
                "selectedChromaProfileGuid".into(),
                json!(configuration.selected_profile()),
            );
            host.insert("isChromaEnabled".into(), json!(configuration.enabled()));
        }
        if let Some(installed) = snapshot.module_installed() {
            host.insert("chromaInstalledModule".into(), json!(installed));
        }
        if let Some(enabled) = snapshot.dynamic_lighting() {
            host.insert("isDynamicLighting".into(), json!(enabled));
        }
    }
    if let Ok(installation) = &readback.installation {
        host.insert(
            "allChromaResourcesInstalled".into(),
            json!(installation.fully_installed()),
        );
    }
    Value::Object(host)
}

impl AppShell {
    pub(in crate::shell) fn install_product_lighting_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.product_lighting.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }
    pub(in crate::shell) fn retain_product_lighting_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.product_lighting.operations.values() {
            if !operation.scope.matches(self, observations, cx)
                || operation
                    .scope
                    .workspace
                    .upgrade()
                    .is_none_or(|workspace| !operation.request.accepts(&workspace, cx))
            {
                operation.canceled.store(true, Ordering::Release);
            }
        }
    }
    fn product_connected_chroma(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        product_id: u32,
    ) -> anyhow::Result<Option<Vec<u32>>> {
        let sender_id = workspace.entity_id().as_u64();
        self.host_storage.register_view(HostStorageView {
            id: sender_id,
            url: format!("/synapse/products/{product_id}/ui/"),
            remote: false,
            destroyed: false,
            crashed: false,
            disposed: false,
            tracked_url: true,
        });
        product_chroma::connected_products(&mut self.host_storage, sender_id)
    }
    pub(super) fn observe_keyboard_product_lighting(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: KeyboardLightingRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_product_lighting_observation(workspace, Request::Keyboard(request), window, cx);
    }
    pub(super) fn observe_mouse_product_lighting(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseLightingRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let validation = (|| -> anyhow::Result<()> {
            match request.command() {
                "getMemoryStorageItem" => {
                    #[derive(Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Memory {
                        key: String,
                    }
                    let memory: Memory = serde_json::from_value(request.payload().clone())?;
                    ensure!(
                        memory.key == product_chroma::CONNECTED_DEVICES_KEY,
                        "Unexpected product lighting memory key"
                    );
                }
                "checkIsFullyInstall" => {
                    #[derive(Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Module {
                        module: String,
                    }
                    let module: Module = serde_json::from_value(request.payload().clone())?;
                    ensure!(
                        module.module == "chroma-app",
                        "Unexpected product lighting installation identity"
                    );
                }
                _ => anyhow::bail!("Request is not a product lighting observation"),
            }
            Ok(())
        })();
        if let Err(error) = validation {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_mouse_lighting(&request, Err(format!("{error:#}")), window, cx)
            });
            return;
        }
        if request.command() == "getMemoryStorageItem" {
            let result = self.product_connected_chroma(&workspace, request.product_id()).map(|products| products.map(|products| json!({"chromaDevices":products.into_iter().map(|product_id| json!({"productId":product_id})).collect::<Vec<_>>()}))).map_err(|error| format!("{error:#}"));
            workspace.update(cx, |workspace, cx| {
                workspace.complete_mouse_lighting(
                    &request,
                    result
                        .as_ref()
                        .map(|value| value.as_ref())
                        .map_err(Clone::clone),
                    window,
                    cx,
                )
            });
            return;
        }
        self.start_product_lighting_observation(workspace, Request::Mouse(request), window, cx);
    }
    fn start_product_lighting_observation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.start_product_lighting_operation(
            workspace,
            request,
            worker::Work::Observe,
            window.window_handle(),
            cx,
        );
    }
    pub(super) fn start_mouse_lighting_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseLightingRequested,
        route: ObservedDevice,
        submission: Submission,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        self.start_product_lighting_operation(
            workspace,
            Request::Mouse(request),
            worker::Work::Brightness { route, submission },
            window,
            cx,
        );
    }
    fn start_product_lighting_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: Request,
        work: worker::Work,
        handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let persisted = work.was_persisted();
        let Some((id, accepted)) = self
            .product_lighting
            .accept(self.close_requested, persisted)
        else {
            request.reject_at(
                &workspace,
                "Product lighting request canceled during shutdown".into(),
                handle,
                cx,
            );
            return;
            return;
        };
        let route = match &work {
            worker::Work::Observe => None,
            worker::Work::Brightness { route, .. } => Some(route.clone()),
        };
        let scope = Scope {
            workspace: workspace.downgrade(),
            identity: workspace.read(cx).identity(cx),
            connection_revision: workspace.read(cx).connection_revision(),
            route,
        };
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let delivery_scope = scope.clone();
        let delivery_request = request.clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let background_canceled = signal.clone();
            let active = delivery_request.guard();
            let result = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(work, active, background_canceled)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                if shell
                    .product_lighting
                    .operations
                    .get(&id)
                    .is_some_and(|operation| Arc::ptr_eq(&operation.canceled, &signal))
                {
                    shell.product_lighting.operations.remove(&id);
                }
                let current = !signal.load(Ordering::Acquire)
                    && (!shell.close_requested || persisted)
                    && delivery_scope.matches(shell, &shell.device_observations, cx);
                if let Some(workspace) = delivery_scope.workspace.upgrade() {
                    if current && delivery_request.accepts(&workspace, cx) {
                        let products = (matches!(&delivery_request, Request::Keyboard(_))
                            && matches!(&result, Ok(worker::Reply::Observation(_))))
                        .then(|| {
                            shell
                                .product_connected_chroma(&workspace, delivery_request.product_id())
                        });
                        if let Ok(worker::Reply::Observation(readback)) = &result {
                            let diagnostics = [
                                readback
                                    .snapshot
                                    .as_ref()
                                    .err()
                                    .map(|error| format!("Chroma 本地配置未读取：{error}")),
                                readback
                                    .installation
                                    .as_ref()
                                    .err()
                                    .map(|error| format!("Chroma 安装状态未读取：{error}")),
                                products
                                    .as_ref()
                                    .and_then(|products| products.as_ref().err())
                                    .map(|error| format!("Chroma 已发布设备未读取：{error:#}")),
                            ]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>();
                            if !diagnostics.is_empty() {
                                shell.status = diagnostics.join("；");
                            }
                        }
                        let delivered = handle.update(cx, |_, window, cx| {
                            complete(&delivery_request, &workspace, result, products, window, cx);
                        });
                        if delivered.is_err() {
                            delivery_request.abandon(
                                &workspace,
                                Some("Product lighting window closed before completion".into()),
                                cx,
                            );
                        }
                    } else {
                        delivery_request.abandon(&workspace, None, cx);
                    }
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        });
        self.product_lighting.operations.insert(
            id,
            Operation {
                scope,
                request,
                canceled,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "product_lighting_observation_tests.rs"]
mod tests;

fn complete(
    request: &Request,
    workspace: &Entity<ProductWorkspace>,
    result: anyhow::Result<worker::Reply>,
    products: Option<anyhow::Result<Option<Vec<u32>>>>,
    window: &mut Window,
    cx: &mut App,
) {
    let reply = match result {
        Ok(reply) => reply,
        Err(error) => {
            request.reject(workspace, format!("{error:#}"), window, cx);
            return;
        }
    };
    match (request, reply) {
        (Request::Keyboard(request), worker::Reply::Observation(readback)) => {
            let mut host = keyboard_host(&readback);
            if let Some(products) = products.and_then(Result::ok).flatten() {
                host = host.with_connected_products(products);
            }
            workspace.update(cx, |workspace, cx| {
                workspace.complete_keyboard_host_lighting(request, host, window, cx)
            });
        }
        (Request::Mouse(request), worker::Reply::Observation(readback)) => {
            // The installation check and the independent memory getter have
            // separate source requests and completion semantics.
            let completion = readback
                .installation
                .as_ref()
                .map_err(|error| anyhow::anyhow!(error.clone()))
                .context("Chroma installation observation failed")
                .map(|_| mouse_host(&readback))
                .map_err(|error| format!("{error:#}"));
            workspace.update(cx, |workspace, cx| {
                workspace.complete_mouse_lighting(
                    request,
                    completion.as_ref().map(Some).map_err(Clone::clone),
                    window,
                    cx,
                )
            });
        }
        (Request::Mouse(request), worker::Reply::Submitted) => {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_mouse_lighting(request, Ok(None), window, cx)
            });
        }
        (Request::Keyboard(_), worker::Reply::Submitted) => request.reject(
            workspace,
            "Keyboard request received a mouse brightness completion".into(),
            window,
            cx,
        ),
    }
}
