//! Accepted Seiren operations outlive their presentation window and share one FIFO.
use super::*;
use razer_device::seiren_eq::Report;
use razer_discovery::discovery::ObservedDevice;
use razer_pages::features::{SeirenEqRequested, SeirenStreamMixerRequested};
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}, mpsc::Receiver};

#[derive(Clone)]
pub(super) enum Request {
    Eq(SeirenEqRequested),
    Mixer(SeirenStreamMixerRequested),
}
impl Request {
    fn guard(&self) -> Arc<AtomicBool> {
        match self { Self::Eq(request) => request.guard(), Self::Mixer(request) => request.guard() }
    }
    fn sequence(&self) -> u64 {
        match self { Self::Eq(request) => request.sequence(), Self::Mixer(request) => request.sequence() }
    }
    fn matches(&self, workspace: &ProductWorkspace, cx: &App) -> bool {
        match self {
            Self::Eq(request) => workspace.seiren_eq_request_matches(request, cx),
            Self::Mixer(request) => workspace.seiren_stream_mixer_request_matches(request, cx),
        }
    }
    fn same(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
            && self.sequence() == other.sequence() && Arc::ptr_eq(&self.guard(), &other.guard())
    }
}

enum Receipt {
    Eq(Report),
    Mixer(audio_notifications::MixerReceipt),
}
enum Pending {
    Eq(Receiver<anyhow::Result<Report>>),
    Mixer(Receiver<anyhow::Result<audio_notifications::MixerReceipt>>),
}
impl Pending {
    fn receive(self) -> anyhow::Result<Receipt> {
        use anyhow::Context as _;
        match self {
            Self::Eq(receiver) => receiver.recv().context("Audio EQ owner closed before its receipt")?.map(Receipt::Eq),
            Self::Mixer(receiver) => receiver.recv().context("Audio mixer owner closed before its receipt")?.map(Receipt::Mixer),
        }
    }
}

struct Operation {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    session_id: Option<u64>,
    route: Option<ObservedDevice>,
    request: Request,
    window: AnyWindowHandle,
    delivery: Task<()>,
}
impl Operation {
    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else { return false; };
        shell.devices.contains(&workspace) && workspace.read(cx).identity(cx) == self.identity
            && self.request.matches(workspace.read(cx), cx)
            && self.session_id.is_some_and(|id| shell.audio_notifications.get(&self.identity).is_some_and(|session| session.id() == id))
            && current_route(self.route.as_ref(), observations, workspace.read(cx).device(cx))
    }
}

fn current_route(route: Option<&ObservedDevice>, observations: &[ObservedDevice], device: &Device) -> bool {
    match route {
        Some(route) => device_discovery::retained_connection(std::slice::from_ref(route), observations, device),
        None => !observations.iter().any(|observed| observed.matches(device)),
    }
}

#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    operations: BTreeMap<u64, Operation>,
}
struct Accepted(Arc<AtomicUsize>);
impl Drop for Accepted {
    fn drop(&mut self) { self.0.fetch_sub(1, Ordering::AcqRel); }
}
impl State {
    pub(super) fn pending(&self) -> bool {
        !self.operations.is_empty() || self.accepted.load(Ordering::Acquire) > 0
    }
    fn accept(&mut self) -> Option<(u64, Accepted)> {
        if self.stopped { return None; }
        self.next_id = self.next_id.checked_add(1).expect("Seiren operation identity exhausted");
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some((self.next_id, Accepted(self.accepted.clone())))
    }
    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        std::mem::take(&mut self.operations).into_values().map(|operation| {
            operation.request.guard().store(false, Ordering::Release);
            operation.delivery
        }).collect()
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() { operation.request.guard().store(false, Ordering::Release); }
    }
}

impl AppShell {
    pub(super) fn install_seiren_audio_operation_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.seiren_audio_operations.take_for_shutdown();
            async move { for delivery in deliveries { delivery.await; } }
        });
    }
    pub(super) fn retain_seiren_audio_operation_connections(&self, observations: &[ObservedDevice], cx: &App) {
        for operation in self.seiren_audio_operations.operations.values() {
            if !operation.matches(self, observations, cx) { operation.request.guard().store(false, Ordering::Release); }
        }
    }
    pub(super) fn start_seiren_audio_operation(
        &mut self, workspace: Entity<ProductWorkspace>, request: Request,
        route: Option<ObservedDevice>, window: AnyWindowHandle, cx: &mut Context<Self>,
    ) {
        if self.seiren_audio_operations.operations.values().any(|operation| {
            operation.workspace == workspace.downgrade() && operation.request.same(&request)
        }) { return; }
        let Some((id, accepted)) = self.seiren_audio_operations.accept() else {
            self.finish_seiren_audio_request(&workspace, &request, None, None, cx);
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let session = self.audio_notifications.get(&identity);
        let session_id = session.map(audio_notifications::Session::id);
        let pending = session.ok_or_else(|| anyhow::anyhow!("Mounted Audio_StreamMixer owner is unavailable"))
            .and_then(|session| match &request {
                Request::Eq(request) => session.request(request.clone()).map(Pending::Eq),
                Request::Mixer(request) => session.mixer(request.clone()).map(Pending::Mixer),
            });
        let delivery = cx.spawn(async move |shell, cx| {
            let result = cx.background_spawn(async move {
                let _accepted = accepted;
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pending?.receive()))
                    .map_err(|_| "Audio operation receiver panicked".to_owned())?
                    .map_err(|error| format!("{error:#}"))
            }).await;
            let _ = shell.update(cx, |shell, cx| {
                if let Some(operation) = shell.seiren_audio_operations.operations.remove(&id) {
                    let current = operation.matches(shell, &shell.device_observations, cx);
                    if let Some(workspace) = operation.workspace.upgrade() {
                        let message = current.then(|| operation_message(&operation.request, &result));
                        let error = current.then(|| result.as_ref().err().cloned()).flatten();
                        shell.finish_seiren_audio_request(&workspace, &operation.request,
                            current.then(|| result.ok()).flatten(), error, cx);
                        if current {
                            let _ = operation.window.update(cx, |_, window, cx| workspace.update(cx, |workspace, cx| workspace.sync_seiren_audio_controls(window, cx)));
                        }
                        if let Some(message) = message { shell.status = message; }
                    }
                }
                if shell.close_requested && shell.save_task.is_none() { shell.continue_save_queue(true, cx); }
                cx.notify();
            });
        });
        self.seiren_audio_operations.operations.insert(id, Operation {
            workspace: workspace.downgrade(), identity, session_id, route, request, window, delivery,
        });
    }
    fn finish_seiren_audio_request(
        &mut self, workspace: &Entity<ProductWorkspace>, request: &Request,
        receipt: Option<Receipt>, error: Option<String>, cx: &mut Context<Self>,
    ) {
        workspace.update(cx, |workspace, cx| match request {
            Request::Eq(request) => workspace.finish_seiren_eq(request, receipt.and_then(|receipt| match receipt { Receipt::Eq(report) => Some(report), _ => None }), cx),
            Request::Mixer(request) => workspace.finish_seiren_stream_mixer(request, error, cx),
        });
    }
}

fn operation_message(request: &Request, result: &Result<Receipt, String>) -> String {
    match request {
        Request::Eq(request) => {
            let local = if request.persists_local_settings() { "；本地设置已保存" } else { "" };
            match result {
                Ok(Receipt::Eq(report)) if report.confirmed() => {
                    if report.native_levels().is_some() { "已读取麦克风／耳机的实际音量和静音状态。".into() }
                    else if !report.setter_sent() { "设备当前旋钮模式与所选配置一致。".into() }
                    else { format!("麦克风设置已获设备／服务回执{local}。") }
                }
                Ok(Receipt::Eq(report)) => {
                    let failures = report.stages().iter().filter_map(|stage| stage.error())
                        .chain(report.native_stages().iter().filter_map(|stage| stage.error())).collect::<Vec<_>>().join("；");
                    if let Some(reason) = report.source_skipped() { format!("麦克风设置按原始调用条件跳过：{reason}{failures}{local}。") }
                    else { format!("麦克风设置尚未全部确认：{failures}{local}。") }
                }
                Err(error) => format!("麦克风写入尚未确认：{error}{local}。"),
                _ => "麦克风操作收到另一种请求的回执。".into(),
            }
        }
        Request::Mixer(_) => match result {
            Ok(Receipt::Mixer(audio_notifications::MixerReceipt::LocalSaved)) => "混音器设置已保存到本机；当前设备未连接，未写入设备。".into(),
            Ok(Receipt::Mixer(audio_notifications::MixerReceipt::DeviceSubmitted)) => "混音器操作已取得设备／服务回执。".into(),
            Err(error) => format!("混音器操作尚未确认：{error}"),
            _ => "混音器操作收到另一种请求的回执。".into(),
        }
    }
}

#[cfg(test)]
#[path = "seiren_audio_operations_tests.rs"]
mod tests;
