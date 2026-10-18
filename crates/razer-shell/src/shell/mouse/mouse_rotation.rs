//! Rotation saves source profile intent before an explicit device setter.
//! Native requests retain one physical driver registration across user edits.
use super::*;
use razer_device::mouse_rotation::{self as rotation, RotationRoute, Setting};
use razer_discovery::{
    direct,
    discovery::{ObservedDevice, ObservedTransport},
};
use razer_pages::features::mouse_products::{
    MouseRotationCompletion, MouseRotationRequested, MouseRotationSource,
};
use razer_service::mapping::mouse_rotation as native;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[path = "mouse_rotation_save.rs"]
mod save;
pub(super) use save::LocalSave;

#[derive(Clone, Copy)]
enum Admission {
    New,
    AcceptedWrite,
}

#[derive(Clone)]
struct Queued {
    workspace: Entity<ProductWorkspace>,
    request: MouseRotationRequested,
    route: ObservedDevice,
    revision: u64,
    transport: RotationRoute,
}
impl Queued {
    fn current(
        &self,
        shell: &AppShell,
        key: &str,
        observations: &[ObservedDevice],
        cx: &App,
    ) -> bool {
        current(
            shell,
            &self.workspace,
            &self.request,
            key,
            &self.route,
            self.revision,
            observations,
            cx,
        ) && self
            .workspace
            .read(cx)
            .mouse_rotation_request_is_current(&self.request, cx)
    }
}
struct Direct {
    id: Uuid,
    owner: Queued,
    delivery: Task<()>,
}
struct Native {
    id: Uuid,
    workspace: WeakEntity<ProductWorkspace>,
    scope: MouseRotationRequested,
    route: ObservedDevice,
    revision: u64,
    active: Option<Queued>,
    submitted: bool,
    keep: Arc<AtomicBool>,
    delivery: Task<()>,
}
impl Native {
    fn current(
        &self,
        shell: &AppShell,
        key: &str,
        observations: &[ObservedDevice],
        cx: &App,
    ) -> bool {
        self.keep.load(Ordering::Acquire)
            && self.workspace.upgrade().is_some_and(|workspace| {
                current(
                    shell,
                    &workspace,
                    &self.scope,
                    key,
                    &self.route,
                    self.revision,
                    observations,
                    cx,
                )
            })
    }
}

#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    direct: BTreeMap<String, Direct>,
    native: BTreeMap<String, Native>,
    queued: BTreeMap<String, Queued>,
}
impl State {
    fn admits(&self, closing: bool, admission: Admission) -> bool {
        !self.stopped && (!closing || matches!(admission, Admission::AcceptedWrite))
    }
    pub(super) fn pending(&self) -> bool {
        !self.direct.is_empty()
            || !self.queued.is_empty()
            || self.native.values().any(|owner| owner.active.is_some())
    }
    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        for queued in std::mem::take(&mut self.queued).into_values() {
            queued.request.guard().store(false, Ordering::Release);
        }
        let mut tasks = Vec::new();
        for owner in std::mem::take(&mut self.direct).into_values() {
            owner.owner.request.guard().store(false, Ordering::Release);
            tasks.push(owner.delivery);
        }
        for owner in std::mem::take(&mut self.native).into_values() {
            owner.keep.store(false, Ordering::Release);
            tasks.push(owner.delivery);
        }
        tasks
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for owner in self.direct.values() {
            owner.owner.request.guard().store(false, Ordering::Release);
        }
        for owner in self.native.values() {
            owner.keep.store(false, Ordering::Release);
        }
    }
}

fn current(
    shell: &AppShell,
    workspace: &Entity<ProductWorkspace>,
    request: &MouseRotationRequested,
    identity: &str,
    route: &ObservedDevice,
    revision: u64,
    observations: &[ObservedDevice],
    cx: &App,
) -> bool {
    shell.devices.contains(workspace)
        && workspace.read(cx).identity(cx) == identity
        && workspace.read(cx).connection_revision() == revision
        && workspace
            .read(cx)
            .mouse_rotation_scope_is_current(request, cx)
        && super::device_discovery::retained_connection(
            std::slice::from_ref(route),
            observations,
            workspace.read(cx).device(cx),
        )
}

fn native_target(
    route: &ObservedDevice,
) -> anyhow::Result<razer_ipc::mouse_rotation::NativeTarget> {
    anyhow::ensure!(
        cfg!(windows)
            && route.hid_node().is_none()
            && matches!(
                route.transport(),
                Some(ObservedTransport::Wired | ObservedTransport::Dongle)
            ),
        "Rotation native fallback requires an observed Windows USB filter-driver route"
    );
    razer_ipc::mouse_rotation::NativeTarget::new(
        route.physical_product_id(),
        route.container().into(),
    )
}

impl AppShell {
    fn finish_mouse_rotation(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &MouseRotationRequested,
        completion: MouseRotationCompletion,
        cx: &mut Context<Self>,
    ) {
        if let Some(message) = workspace.update(cx, |workspace, cx| {
            workspace.complete_mouse_rotation(request, completion, cx)
        }) {
            self.status = message;
            cx.notify();
        }
    }

    pub(super) fn install_mouse_rotation_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, _| {
            let tasks = shell.mouse_rotation.take_for_shutdown();
            async move {
                for task in tasks {
                    task.await;
                }
            }
        });
    }

    pub(super) fn stop_mouse_rotation_native(&self) {
        for owner in self.mouse_rotation.native.values() {
            owner.keep.store(false, Ordering::Release);
        }
    }

    pub(super) fn retain_mouse_rotation_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for (key, owner) in &self.mouse_rotation.direct {
            if !owner.owner.current(self, key, observations, cx) {
                owner.owner.request.guard().store(false, Ordering::Release);
            }
        }
        for (key, owner) in &self.mouse_rotation.native {
            if !owner.current(self, key, observations, cx) {
                owner.keep.store(false, Ordering::Release);
            }
        }
        for (key, queued) in &self.mouse_rotation.queued {
            if !queued.current(self, key, observations, cx) {
                queued.request.guard().store(false, Ordering::Release);
            }
        }
    }

    pub(super) fn dispatch_mouse_rotation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &MouseRotationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_rotation_request_is_current(request, cx)
        {
            return;
        }
        if !self
            .mouse_rotation
            .admits(self.close_requested, Admission::New)
        {
            self.finish_mouse_rotation(&workspace, request, MouseRotationCompletion::Cancelled, cx);
            return;
        }
        match request.source() {
            MouseRotationSource::UserEdit => {
                self.persist_mouse_rotation(workspace, request.clone(), window, cx)
            }
            MouseRotationSource::ProfileRestore => {
                self.submit_mouse_rotation(workspace, request.clone(), Admission::New, cx)
            }
        }
    }

    fn submit_mouse_rotation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseRotationRequested,
        admission: Admission,
        cx: &mut Context<Self>,
    ) {
        if !self.mouse_rotation.admits(self.close_requested, admission)
            || self.receiver_macro_exclusive(cx)
            || !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_rotation_request_is_current(&request, cx)
        {
            self.finish_mouse_rotation(
                &workspace,
                &request,
                MouseRotationCompletion::Cancelled,
                cx,
            );
            return;
        }
        let selected = (|| -> anyhow::Result<_> {
            request.setting().validate()?;
            let device = workspace.read(cx).device(cx);
            let mut routes = self
                .device_observations
                .iter()
                .filter(|route| route.matches(device));
            let route = routes
                .next()
                .ok_or_else(|| anyhow::anyhow!("Rotation physical device is absent"))?;
            anyhow::ensure!(
                routes.next().is_none(),
                "Rotation physical device is ambiguous"
            );
            let transport = rotation::route(
                request.product_id(),
                device
                    .dashboard
                    .readonly_values
                    .as_ref()
                    .and_then(|values| values.firmware.as_deref()),
            )?;
            match transport {
                RotationRoute::DirectFeature => anyhow::ensure!(
                    direct::mouse_rotation::available(route),
                    "Rotation direct route is unavailable"
                ),
                RotationRoute::NativeMappingEngine => {
                    native_target(route)?;
                }
            }
            Ok((route.clone(), transport))
        })();
        let (route, transport) = match selected {
            Ok(selected) => selected,
            Err(error) => {
                self.finish_mouse_rotation(
                    &workspace,
                    &request,
                    MouseRotationCompletion::Failed(format!("{error:#}")),
                    cx,
                );
                return;
            }
        };
        let key = workspace.read(cx).identity(cx);
        let queued = Queued {
            revision: workspace.read(cx).connection_revision(),
            workspace,
            request,
            route,
            transport,
        };
        if self.mouse_rotation.direct.contains_key(&key) {
            self.queue_mouse_rotation(key, queued, cx);
        } else if let Some(owner) = self.mouse_rotation.native.get(&key) {
            let reusable = owner.current(self, &key, &self.device_observations, cx)
                && transport == RotationRoute::NativeMappingEngine;
            if reusable && owner.active.is_none() {
                let owner = self.mouse_rotation.native.get_mut(&key).unwrap();
                owner.scope = queued.request.clone();
                owner.active = Some(queued);
                owner.submitted = false;
            } else {
                if !reusable {
                    owner.keep.store(false, Ordering::Release);
                }
                self.queue_mouse_rotation(key, queued, cx);
            }
        } else {
            self.start_mouse_rotation(key, queued, cx);
        }
    }

    fn queue_mouse_rotation(&mut self, key: String, queued: Queued, cx: &mut Context<Self>) {
        if let Some(skipped) = self.mouse_rotation.queued.insert(key, queued) {
            self.finish_mouse_rotation(
                &skipped.workspace,
                &skipped.request,
                MouseRotationCompletion::Cancelled,
                cx,
            );
        }
    }

    fn next_mouse_rotation(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(queued) = self.mouse_rotation.queued.remove(key) {
            if queued.current(self, key, &self.device_observations, cx) {
                self.submit_mouse_rotation(
                    queued.workspace,
                    queued.request,
                    Admission::AcceptedWrite,
                    cx,
                );
            } else {
                self.finish_mouse_rotation(
                    &queued.workspace,
                    &queued.request,
                    MouseRotationCompletion::Cancelled,
                    cx,
                );
            }
        }
        if self.close_requested && self.save_task.is_none() {
            self.continue_save_queue(true, cx);
        }
    }

    fn start_mouse_rotation(&mut self, key: String, queued: Queued, cx: &mut Context<Self>) {
        if !queued.current(self, &key, &self.device_observations, cx) {
            self.finish_mouse_rotation(
                &queued.workspace,
                &queued.request,
                MouseRotationCompletion::Cancelled,
                cx,
            );
            return;
        }
        match queued.transport {
            RotationRoute::DirectFeature => self.start_mouse_rotation_direct(key, queued, cx),
            RotationRoute::NativeMappingEngine => self.start_mouse_rotation_native(key, queued, cx),
        }
    }

    fn start_mouse_rotation_direct(&mut self, key: String, queued: Queued, cx: &mut Context<Self>) {
        let id = Uuid::new_v4();
        let route = queued.route.clone();
        let setting = queued.request.setting();
        let active = queued.request.guard();
        let worker = cx.background_spawn(async move {
            razer_service::mouse_rotation::device(route, Some(setting), active)
        });
        let delivery_key = key.clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = worker.await;
            let _ = shell.update(cx, |shell, cx| {
                if !shell
                    .mouse_rotation
                    .direct
                    .get(&delivery_key)
                    .is_some_and(|owner| owner.id == id)
                {
                    return;
                }
                let owner = shell
                    .mouse_rotation
                    .direct
                    .remove(&delivery_key)
                    .unwrap()
                    .owner;
                if let Some(error) = completion.cleanup_failure {
                    log::warn!("[rotation cleanup] {error}");
                }
                if owner.current(shell, &delivery_key, &shell.device_observations, cx) {
                    let result = match completion.result {
                        Ok(report) => MouseRotationCompletion::Direct(report),
                        Err(error) => MouseRotationCompletion::Failed(error),
                    };
                    shell.finish_mouse_rotation(&owner.workspace, &owner.request, result, cx);
                }
                shell.next_mouse_rotation(&delivery_key, cx);
            });
        });
        self.mouse_rotation.direct.insert(
            key,
            Direct {
                id,
                owner: queued,
                delivery,
            },
        );
    }

    fn start_mouse_rotation_native(&mut self, key: String, queued: Queued, cx: &mut Context<Self>) {
        let target = match native_target(&queued.route) {
            Ok(target) => target,
            Err(error) => {
                self.finish_mouse_rotation(
                    &queued.workspace,
                    &queued.request,
                    MouseRotationCompletion::Failed(format!("{error:#}")),
                    cx,
                );
                return;
            }
        };
        let id = Uuid::new_v4();
        let keep = Arc::new(AtomicBool::new(true));
        let signal = keep.clone();
        let delivery_key = key.clone();
        let create = cx.background_spawn(async move { native::Session::start(target) });
        let delivery = cx.spawn(async move |shell, cx| {
            let created = create.await;
            let mut failure = None;
            if let Ok((session, replies)) = created {
                let mut ready = false;
                let mut pending = None;
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(16))
                        .await;
                    let request = shell
                        .update(cx, |shell, cx| {
                            let Some(owner) = shell
                                .mouse_rotation
                                .native
                                .get(&delivery_key)
                                .filter(|owner| owner.id == id)
                            else {
                                return None;
                            };
                            if !owner.current(shell, &delivery_key, &shell.device_observations, cx)
                            {
                                signal.store(false, Ordering::Release);
                                return None;
                            }
                            if !ready || owner.submitted {
                                return None;
                            }
                            let Some(active) = &owner.active else {
                                return None;
                            };
                            if !active.current(shell, &delivery_key, &shell.device_observations, cx)
                            {
                                signal.store(false, Ordering::Release);
                                return None;
                            }
                            let request = active.request.clone();
                            shell
                                .mouse_rotation
                                .native
                                .get_mut(&delivery_key)
                                .unwrap()
                                .submitted = true;
                            Some(request)
                        })
                        .ok()
                        .flatten();
                    if let Some(request) = request {
                        match request
                            .setting()
                            .native_request()
                            .map_err(|error| format!("{error:#}"))
                            .and_then(|request| session.request(request))
                        {
                            Ok(request_id) => pending = Some((request_id, request)),
                            Err(error) => {
                                failure = Some(error);
                                signal.store(false, Ordering::Release);
                            }
                        }
                    }
                    if !signal.load(Ordering::Acquire) {
                        session.request_stop();
                    }
                    let mut stopped = false;
                    for reply in replies.try_iter() {
                        match reply {
                            native::Reply::Ready => ready = true,
                            native::Reply::Completed {
                                id: request_id,
                                result,
                            } => {
                                let Some((expected, request)) = pending.take() else {
                                    continue;
                                };
                                if expected != request_id {
                                    failure =
                                        Some("Rotation native request identity changed".into());
                                    signal.store(false, Ordering::Release);
                                    continue;
                                }
                                let _ = shell.update(cx, |shell, cx| {
                                    let Some(owner) = shell
                                        .mouse_rotation
                                        .native
                                        .get(&delivery_key)
                                        .filter(|owner| owner.id == id)
                                    else {
                                        return;
                                    };
                                    let current = owner.current(
                                        shell,
                                        &delivery_key,
                                        &shell.device_observations,
                                        cx,
                                    );
                                    let active = shell
                                        .mouse_rotation
                                        .native
                                        .get_mut(&delivery_key)
                                        .unwrap()
                                        .active
                                        .take();
                                    if let Some(active) = active.filter(|active| {
                                        active.request.sequence() == request.sequence()
                                            && active.request.scope() == request.scope()
                                    }) {
                                        if current
                                            && active.current(
                                                shell,
                                                &delivery_key,
                                                &shell.device_observations,
                                                cx,
                                            )
                                        {
                                            let result = match result {
                                                Ok(reply) => MouseRotationCompletion::Native(reply),
                                                Err(error) => {
                                                    MouseRotationCompletion::Failed(error)
                                                }
                                            };
                                            shell.finish_mouse_rotation(
                                                &active.workspace,
                                                &active.request,
                                                result,
                                                cx,
                                            );
                                        }
                                    }
                                    shell.next_mouse_rotation(&delivery_key, cx);
                                });
                            }
                            native::Reply::Failed(error) => {
                                failure = Some(error);
                                signal.store(false, Ordering::Release);
                            }
                            native::Reply::Stopped(result) => {
                                if let Err(error) = result {
                                    failure = Some(error);
                                }
                                stopped = true;
                            }
                        }
                    }
                    if stopped || session.is_finished() {
                        break;
                    }
                    if shell.upgrade().is_none() {
                        signal.store(false, Ordering::Release);
                    }
                }
                if let Err(error) = cx
                    .background_executor()
                    .spawn(async move { session.stop() })
                    .await
                {
                    failure = Some(error);
                }
            } else if let Err(error) = created {
                failure = Some(format!("{error:#}"));
            }
            let _ = shell.update(cx, |shell, cx| {
                if !shell
                    .mouse_rotation
                    .native
                    .get(&delivery_key)
                    .is_some_and(|owner| owner.id == id)
                {
                    return;
                }
                let owner = shell.mouse_rotation.native.remove(&delivery_key).unwrap();
                if let Some(error) = &failure {
                    log::warn!("[rotation native cleanup] {error}");
                }
                if let Some(active) = owner.active {
                    if active.current(shell, &delivery_key, &shell.device_observations, cx) {
                        let result = failure.map_or(
                            MouseRotationCompletion::Cancelled,
                            MouseRotationCompletion::Failed,
                        );
                        shell.finish_mouse_rotation(&active.workspace, &active.request, result, cx);
                    }
                }
                shell.next_mouse_rotation(&delivery_key, cx);
            });
        });
        self.mouse_rotation.native.insert(
            key,
            Native {
                id,
                workspace: queued.workspace.downgrade(),
                scope: queued.request.clone(),
                route: queued.route.clone(),
                revision: queued.revision,
                active: Some(queued),
                submitted: false,
                keep,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_rotation_lifecycle_tests.rs"]
mod lifecycle_tests;
