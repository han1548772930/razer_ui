//! Source device-wide indicator owner. Each actual receiver gets one FIFO;
//! disconnect cancels pending requests, and service cleanup runs off the UI.
use super::*;
use razer_device::keyboard_indicator::{self as indicator, Change, Report};
use razer_discovery::{direct::keyboard_indicator as route, discovery::ObservedDevice};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::keyboard_products::{
    KeyboardIndicatorLedObservation as Observation, KeyboardIndicatorLedRequested,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

enum Reply {
    Initialized(Report),
    Finished(u64, Result<(), String>),
    Diagnostic(String),
    Stopped,
}
struct Session {
    identity: String,
    route: String,
    valid: Arc<AtomicBool>,
    commands: mpsc::Sender<(u64, u8)>,
    replies: mpsc::Receiver<Reply>,
    pending: BTreeSet<u64>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
    }
}
#[derive(Default)]
pub(super) struct State {
    owners: BTreeMap<EntityId, Session>,
    workers: Vec<JoinHandle<()>>,
    retry: BTreeMap<EntityId, u64>,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
fn worker(
    observed: ObservedDevice,
    valid: Arc<AtomicBool>,
    commands: mpsc::Receiver<(u64, u8)>,
    replies: mpsc::Sender<Reply>,
) {
    let result = (|| -> anyhow::Result<()> {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(valid.clone());
        let result = (|| -> anyhow::Result<()> {
            let target = route::resolve(&mut client, &observed)?;
            match route::execute(&mut client, target.clone(), Change::Initialize) {
                Ok(report) => {
                    let _ = replies.send(Reply::Initialized(report));
                }
                Err(error) => {
                    let _ = replies.send(Reply::Diagnostic(format!("{error:#}")));
                }
            }
            while valid.load(Ordering::Acquire) {
                let (generation, status) = match commands.recv_timeout(Duration::from_millis(40)) {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let result = route::execute(&mut client, target.clone(), Change::Set(status))
                    .map(|_| ())
                    .map_err(|error| format!("{error:#}"));
                if replies.send(Reply::Finished(generation, result)).is_err() {
                    break;
                }
            }
            Ok(())
        })();
        client.set_request_guard(Arc::new(AtomicBool::new(true)));
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            let _ = replies.send(Reply::Diagnostic(format!("退出指示灯通信：{error:#}")));
        }
        result
    })();
    if let Err(error) = result {
        let _ = replies.send(Reply::Diagnostic(format!("{error:#}")));
    }
    for (generation, _) in commands.try_iter() {
        let _ = replies.send(Reply::Finished(
            generation,
            Err("键盘接收器通信已结束".into()),
        ));
    }
    let _ = replies.send(Reply::Stopped);
}
impl AppShell {
    pub(super) fn install_keyboard_indicator_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|shell, cx| {
            shell.keyboard_indicator.owners.clear();
            let workers = std::mem::take(&mut shell.keyboard_indicator.workers);
            let cleanup = cx.background_executor().spawn(async move {
                for worker in workers {
                    if worker.join().is_err() {
                        eprintln!("[keyboard indicator] worker panicked during cleanup");
                    }
                }
            });
            async move { cleanup.await }
        }));
    }
    pub(super) fn sync_keyboard_indicator(&mut self, cx: &mut Context<Self>) {
        let candidates = self
            .devices
            .iter()
            .filter_map(|workspace| {
                let view = workspace.read(cx);
                let device = view.device(cx);
                if device.product_id != indicator::PRODUCT_ID {
                    return None;
                }
                let routes = self
                    .device_observations
                    .iter()
                    .filter(|route| {
                        route.matches(device)
                            && route.physical_product_id() == indicator::RECEIVER_ID
                            && route.peer_product_id().is_none()
                            && matches!(
                                route.transport(),
                                Some(razer_discovery::discovery::ObservedTransport::Dongle)
                            )
                    })
                    .collect::<Vec<_>>();
                let [observed] = routes.as_slice() else {
                    return None;
                };
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
        let stale = self
            .keyboard_indicator
            .owners
            .iter()
            .filter_map(|(id, owner)| {
                (!candidates.iter().any(|(workspace, identity, key, _)| {
                    workspace.entity_id() == *id
                        && *identity == owner.identity
                        && *key == owner.route
                }))
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        for id in stale {
            self.keyboard_indicator.owners.remove(&id);
            if let Some(workspace) = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_keyboard_indicator_led(Observation::Disconnected, cx)
                });
            }
        }
        for (workspace, identity, key, observed) in candidates {
            let id = workspace.entity_id();
            if self.keyboard_indicator.owners.contains_key(&id)
                || self
                    .keyboard_indicator
                    .retry
                    .get(&id)
                    .is_some_and(|retry| *retry > now())
            {
                continue;
            }
            let (commands, receiver) = mpsc::channel();
            let (sender, replies) = mpsc::channel();
            let valid = Arc::new(AtomicBool::new(true));
            let signal = valid.clone();
            match thread::Builder::new()
                .name("keyboard-indicator".into())
                .spawn(move || worker(observed, signal, receiver, sender))
            {
                Ok(worker) => {
                    self.keyboard_indicator.workers.push(worker);
                    self.keyboard_indicator.owners.insert(
                        id,
                        Session {
                            identity,
                            route: key,
                            valid,
                            commands,
                            replies,
                            pending: BTreeSet::new(),
                        },
                    );
                    // Connection is a physical fact. One is the source reducer's
                    // initial selection, independent of whether GET succeeds.
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_keyboard_indicator_led(
                            Observation::Connected { status: 1 },
                            cx,
                        )
                    });
                }
                Err(error) => {
                    self.keyboard_indicator
                        .retry
                        .insert(id, now().saturating_add(1_000));
                    self.status = format!("键盘指示灯：{error}");
                    cx.notify();
                }
            }
        }
        let mut replies = Vec::new();
        for (id, owner) in &self.keyboard_indicator.owners {
            loop {
                match owner.replies.try_recv() {
                    Ok(reply) => {
                        let stopped = matches!(&reply, Reply::Stopped);
                        replies.push((*id, reply));
                        if stopped {
                            break;
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        replies.push((*id, Reply::Stopped));
                        break;
                    }
                }
            }
        }
        for (id, reply) in replies {
            let workspace = self
                .devices
                .iter()
                .find(|workspace| workspace.entity_id() == id)
                .cloned();
            match reply {
                Reply::Initialized(report) => {
                    if let (Some(workspace), Some(status)) = (
                        workspace,
                        report
                            .requested_status
                            .filter(|_| report.setter_acknowledgement.is_some()),
                    ) {
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_keyboard_indicator_led(
                                Observation::Connected { status },
                                cx,
                            )
                        });
                    }
                }
                Reply::Finished(generation, result) => {
                    if let Some(owner) = self.keyboard_indicator.owners.get_mut(&id) {
                        owner.pending.remove(&generation);
                    }
                    if let Some(workspace) = workspace {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_keyboard_indicator_led(generation, result, cx)
                        });
                    }
                }
                Reply::Diagnostic(error) => {
                    eprintln!("[keyboard indicator] {error}");
                    self.status = format!("键盘指示灯：{error}");
                    cx.notify();
                }
                Reply::Stopped => {
                    if let Some(owner) = self.keyboard_indicator.owners.remove(&id) {
                        if let Some(workspace) = workspace {
                            for generation in &owner.pending {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.finish_keyboard_indicator_led(
                                        *generation,
                                        Err("键盘指示灯通信已结束".into()),
                                        cx,
                                    )
                                });
                            }
                        }
                    }
                    self.keyboard_indicator
                        .retry
                        .insert(id, now().saturating_add(1_000));
                }
            }
        }
        for worker in std::mem::take(&mut self.keyboard_indicator.workers) {
            if worker.is_finished() {
                if worker.join().is_err() {
                    eprintln!("[keyboard indicator] worker panicked");
                    self.status = "键盘指示灯通信线程异常退出".into();
                    cx.notify();
                }
            } else {
                self.keyboard_indicator.workers.push(worker);
            }
        }
    }
    pub(super) fn submit_keyboard_indicator(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &KeyboardIndicatorLedRequested,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .keyboard_indicator
            .owners
            .get_mut(&workspace.entity_id())
            .ok_or_else(|| "键盘指示灯没有当前接收器连接".to_owned())
            .and_then(|owner| {
                owner
                    .commands
                    .send((request.generation(), request.status()))
                    .map_err(|_| "键盘指示灯通信线程已结束".to_owned())?;
                owner.pending.insert(request.generation());
                Ok(())
            });
        if let Err(error) = result {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_indicator_led(request.generation(), Err(error), cx)
            });
        }
    }
}
