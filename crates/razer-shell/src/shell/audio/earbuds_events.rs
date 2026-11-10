//! One physical notification owner gates earbud operations and releases last.
use super::*;
use razer_discovery::discovery::ObservedDevice;
use razer_service::earbuds_auto_pause::MonitorUpdate;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
use uuid::Uuid;

struct Operations {
    count: AtomicUsize,
    admission: Mutex<Admission>,
}
enum Admission {
    Accepting(Vec<Weak<AtomicBool>>),
    Closed,
}
impl Default for Operations {
    fn default() -> Self {
        Self {
            count: AtomicUsize::new(0),
            admission: Mutex::new(Admission::Accepting(Vec::new())),
        }
    }
}
impl Operations {
    fn acquire(self: &Arc<Self>, guard: &Arc<AtomicBool>) -> Result<Hold, String> {
        let mut admission = self
            .admission
            .lock()
            .map_err(|_| "Earbud operation guards poisoned".to_owned())?;
        let Admission::Accepting(guards) = &mut *admission else {
            return Err("Earbud notification owner is closing".into());
        };
        if !guard.load(Ordering::Acquire) {
            return Err("Earbud operation was canceled".into());
        }
        guards.retain(|guard| guard.strong_count() != 0);
        guards.push(Arc::downgrade(guard));
        self.count.fetch_add(1, Ordering::AcqRel);
        Ok(Hold(self.clone()))
    }
    fn cancel(&self) -> anyhow::Result<()> {
        let mut admission = self
            .admission
            .lock()
            .map_err(|_| anyhow::anyhow!("Earbud operation guards poisoned"))?;
        if let Admission::Accepting(guards) = std::mem::replace(&mut *admission, Admission::Closed)
        {
            for guard in guards.iter().filter_map(Weak::upgrade) {
                guard.store(false, Ordering::Release);
            }
        }
        Ok(())
    }
    fn finish(&self) -> anyhow::Result<()> {
        self.cancel()?;
        while self.count.load(Ordering::Acquire) != 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}

/// The actual background operation retains this until its native cleanup ends.
pub(super) struct Lease {
    _hold: Hold,
    route: ObservedDevice,
}
impl Lease {
    pub(super) fn route(&self) -> &ObservedDevice {
        &self.route
    }
}
struct Hold(Arc<Operations>);
impl Drop for Hold {
    fn drop(&mut self) {
        let previous = self.0.count.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0);
    }
}

type Continuation = Box<dyn FnOnce(Result<Lease, String>, &mut AppShell, &mut Context<AppShell>)>;
struct Waiting {
    guard: Arc<AtomicBool>,
    continuation: Continuation,
}
struct Owner {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    connection: u64,
}
impl Owner {
    fn matches(&self, workspace: &Entity<ProductWorkspace>, cx: &App) -> bool {
        self.workspace == *workspace
            && workspace.read(cx).identity(cx) == self.identity
            && workspace.read(cx).connection_revision() == self.connection
    }
    fn current(&self, shell: &AppShell, cx: &App) -> Option<Entity<ProductWorkspace>> {
        self.workspace.upgrade().filter(|workspace| {
            shell.devices.contains(workspace)
                && self.matches(workspace, cx)
                && workspace.read(cx).earbuds_notifications_active(cx)
        })
    }
}
struct Monitor {
    owner: Owner,
    route: ObservedDevice,
    physical: (u32, String),
    active: Arc<AtomicBool>,
    refresh: Arc<AtomicBool>,
    operations: Arc<Operations>,
    auto_pause: Option<Arc<AtomicBool>>,
    counter_ready: bool,
    radio_ready: bool,
    closing: bool,
    waiting: Vec<Waiting>,
    task: Option<Task<()>>,
}
#[derive(Default)]
pub(super) struct State {
    registered: bool,
    exiting: bool,
    monitors: BTreeMap<Uuid, Monitor>,
}
impl Drop for State {
    fn drop(&mut self) {
        for monitor in self.monitors.values() {
            monitor.active.store(false, Ordering::Release);
            if let Err(error) = monitor.operations.cancel() {
                log::warn!("{error:#}");
            }
        }
    }
}

impl AppShell {
    pub(super) fn with_earbuds_session(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        guard: Arc<AtomicBool>,
        auto_pause: Option<Arc<AtomicBool>>,
        cx: &mut Context<Self>,
        continuation: impl FnOnce(Result<Lease, String>, &mut Self, &mut Context<Self>) + 'static,
    ) {
        if self.close_requested
            || self.earbuds_events.exiting
            || !guard.load(Ordering::Acquire)
            || !self.devices.contains(&workspace)
            || !workspace.read(cx).earbuds_notifications_active(cx)
        {
            continuation(Err("Earbud operation scope is inactive".into()), self, cx);
            return;
        }
        let existing = self
            .earbuds_events
            .monitors
            .iter()
            .find_map(|(id, monitor)| {
                (!monitor.closing
                    && monitor.active.load(Ordering::Acquire)
                    && monitor.owner.matches(&workspace, cx))
                .then_some(*id)
            });
        let id = if let Some(id) = existing {
            id
        } else {
            let routes = self
                .device_observations
                .iter()
                .filter(|route| route.matches(workspace.read(cx).device(cx)))
                .collect::<Vec<_>>();
            let [route] = routes.as_slice() else {
                continuation(
                    Err("Earbud operation has no unique current observation".into()),
                    self,
                    cx,
                );
                return;
            };
            let route = (*route).clone();
            let id = Uuid::new_v4();
            self.earbuds_events.monitors.insert(
                id,
                Monitor {
                    owner: Owner {
                        workspace: workspace.downgrade(),
                        identity: workspace.read(cx).identity(cx),
                        connection: workspace.read(cx).connection_revision(),
                    },
                    physical: (
                        route.physical_product_id(),
                        route.container().to_ascii_lowercase(),
                    ),
                    route,
                    active: Arc::new(AtomicBool::new(true)),
                    refresh: Arc::new(AtomicBool::new(false)),
                    operations: Arc::new(Operations::default()),
                    auto_pause: None,
                    counter_ready: false,
                    radio_ready: false,
                    closing: false,
                    waiting: Vec::new(),
                    task: None,
                },
            );
            id
        };
        let monitor = self
            .earbuds_events
            .monitors
            .get_mut(&id)
            .expect("selected notification owner");
        if auto_pause.is_some() {
            monitor.auto_pause = auto_pause;
        }
        monitor.waiting.push(Waiting {
            guard,
            continuation: Box::new(continuation),
        });
        if monitor.counter_ready && !monitor.radio_ready {
            monitor.refresh.store(true, Ordering::Release);
        }
        self.install_earbuds_event_cleanup();
        self.start_waiting_earbuds_monitors(cx);
        self.release_earbud_operations(id, None, cx);
    }

    fn install_earbuds_event_cleanup(&mut self) {
        if self.earbuds_events.registered {
            return;
        }
        self.earbuds_events.registered = true;
        self.register_exit_resume(|shell, _| shell.earbuds_events.exiting = false);
        self.register_exit_cleanup(|shell, cx| {
            shell.earbuds_events.exiting = true;
            let monitors = std::mem::take(&mut shell.earbuds_events.monitors);
            let mut tasks = Vec::new();
            for (_, mut monitor) in monitors {
                monitor.active.store(false, Ordering::Release);
                if let Err(error) = monitor.operations.cancel() {
                    log::warn!("{error:#}");
                }
                for waiting in std::mem::take(&mut monitor.waiting) {
                    (waiting.continuation)(
                        Err("Earbud shutdown admission closed".into()),
                        shell,
                        cx,
                    );
                }
                tasks.extend(monitor.task);
            }
            async move {
                for task in tasks {
                    task.await;
                }
            }
        });
    }

    fn release_earbud_operations(
        &mut self,
        id: Uuid,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(monitor) = self.earbuds_events.monitors.get_mut(&id) else {
            return;
        };
        if error.is_none() && (monitor.closing || !monitor.counter_ready || !monitor.radio_ready) {
            return;
        }
        let waiting = std::mem::take(&mut monitor.waiting);
        let operations = monitor.operations.clone();
        let route = monitor.route.clone();
        for waiting in waiting {
            let result = if let Some(error) = &error {
                Err(error.clone())
            } else if !waiting.guard.load(Ordering::Acquire) {
                Err("Earbud request was canceled before readiness".into())
            } else {
                operations.acquire(&waiting.guard).map(|hold| Lease {
                    _hold: hold,
                    route: route.clone(),
                })
            };
            (waiting.continuation)(result, self, cx);
        }
    }

    fn start_waiting_earbuds_monitors(&mut self, cx: &mut Context<Self>) {
        let ids = self
            .earbuds_events
            .monitors
            .iter()
            .filter_map(|(id, monitor)| (!monitor.closing && monitor.task.is_none()).then_some(*id))
            .collect::<Vec<_>>();
        for id in ids {
            let Some(monitor) = self.earbuds_events.monitors.get(&id) else {
                continue;
            };
            if self.close_requested
                || self.earbuds_events.exiting
                || !monitor.active.load(Ordering::Acquire)
                || monitor.owner.current(self, cx).is_none()
            {
                self.release_earbud_operations(
                    id,
                    Some("Earbud scope changed before notification startup".into()),
                    cx,
                );
                self.earbuds_events.monitors.remove(&id);
                continue;
            }
            let physical = self.earbuds_events.monitors[&id].physical.clone();
            if self.earbuds_events.monitors.iter().any(|(other, monitor)| {
                *other != id && monitor.physical == physical && monitor.task.is_some()
            }) {
                continue;
            }
            let monitor = self
                .earbuds_events
                .monitors
                .get_mut(&id)
                .expect("waiting notification owner");
            let route = monitor.route.clone();
            let active = monitor.active.clone();
            let refresh = monitor.refresh.clone();
            let operations = monitor.operations.clone();
            let (send, receive) = async_channel::bounded(128);
            monitor.task = Some(cx.spawn(async move |shell, cx| {
                let keep = active.clone();
                let worker = cx.background_spawn(async move {
                    let stop = keep.clone();
                    razer_service::earbuds_auto_pause::monitor(
                        route,
                        keep,
                        refresh,
                        |update| {
                            send.try_send(update).map_err(|_| {
                                anyhow::anyhow!("Earbud notification consumer is absent or full")
                            })
                        },
                        move || {
                            stop.store(false, Ordering::Release);
                            operations.finish()
                        },
                    )
                });
                loop {
                    let mut updates = Vec::new();
                    while let Ok(update) = receive.try_recv() {
                        updates.push(update);
                    }
                    if !shell
                        .update(cx, |shell, cx| shell.poll_earbuds_monitor(id, updates, cx))
                        .unwrap_or(false)
                    {
                        active.store(false, Ordering::Release);
                    }
                    if receive.is_closed() && receive.is_empty() {
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
                let completion = worker.await;
                let error = match (completion.result, completion.cleanup_failure) {
                    (Ok(()), None) => None,
                    (Err(error), None) | (Ok(()), Some(error)) => Some(error),
                    (Err(error), Some(cleanup)) => Some(format!("{error}; cleanup: {cleanup}")),
                };
                if let Some(error) = &error {
                    log::warn!("[earbud notification cleanup {id}] {error}");
                }
                let _ = shell.update(cx, |shell, cx| {
                    shell.finish_earbuds_event_owner(id, error, cx)
                });
            }));
        }
    }

    fn poll_earbuds_monitor(
        &mut self,
        id: Uuid,
        updates: Vec<MonitorUpdate>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(monitor) = self.earbuds_events.monitors.get(&id) else {
            return false;
        };
        let workspace = monitor.owner.current(self, cx);
        if workspace.is_none() || self.close_requested || !monitor.active.load(Ordering::Acquire) {
            let monitor = self
                .earbuds_events
                .monitors
                .get_mut(&id)
                .expect("observed notification owner");
            monitor.closing = true;
            monitor.active.store(false, Ordering::Release);
            if let Err(error) = monitor.operations.cancel() {
                log::warn!("{error:#}");
            }
            self.release_earbud_operations(
                id,
                Some("Earbud notification scope changed".into()),
                cx,
            );
            return false;
        }
        let workspace = workspace.expect("current notification workspace");
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return false;
        };
        for update in updates {
            match update {
                MonitorUpdate::Initial(Ok(report)) => {
                    self.earbuds_events
                        .monitors
                        .get_mut(&id)
                        .unwrap()
                        .radio_ready = true;
                    page.update(cx, |page, cx| {
                        page.observe_earbuds_shared_connection(&report, cx)
                    });
                    self.release_earbud_operations(id, None, cx);
                }
                MonitorUpdate::Initial(Err(error)) => {
                    self.earbuds_events
                        .monitors
                        .get_mut(&id)
                        .unwrap()
                        .radio_ready = false;
                    page.update(cx, |page, cx| {
                        page.observe_earbuds_shared_connection_error(error.clone(), cx)
                    });
                    self.release_earbud_operations(id, Some(error), cx);
                }
                MonitorUpdate::Ready => {
                    self.earbuds_events
                        .monitors
                        .get_mut(&id)
                        .unwrap()
                        .counter_ready = true;
                    self.release_earbud_operations(id, None, cx);
                }
                MonitorUpdate::Notification(event) => {
                    let auto_pause = self.earbuds_events.monitors[&id].auto_pause.clone();
                    page.update(cx, |page, cx| {
                        page.observe_earbuds_power_notification(&event, cx);
                        page.observe_earbuds_microphone_notification(&event, cx);
                        if let Some(guard) = auto_pause {
                            page.observe_earbuds_notification(&guard, event, cx);
                        }
                    });
                }
            }
        }
        true
    }

    fn finish_earbuds_event_owner(
        &mut self,
        id: Uuid,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.release_earbud_operations(
            id,
            Some(
                error
                    .clone()
                    .unwrap_or_else(|| "Earbud notification owner stopped".into()),
            ),
            cx,
        );
        let Some(monitor) = self.earbuds_events.monitors.remove(&id) else {
            return;
        };
        if let Some(workspace) = monitor.owner.current(self, cx) {
            let superseded = self
                .earbuds_events
                .monitors
                .values()
                .any(|next| next.owner.matches(&workspace, cx));
            if !superseded && let Some(page) = workspace.read(cx).audio_mixer_page(cx) {
                let error = error
                    .unwrap_or_else(|| "Earbud notification source stopped unexpectedly".into());
                page.update(cx, |page, cx| {
                    if let Some(guard) = &monitor.auto_pause {
                        page.finish_earbuds_monitor(guard, Some(error.clone()), cx);
                    }
                    page.observe_earbuds_shared_connection_error(error, cx);
                });
            }
        }
        self.start_waiting_earbuds_monitors(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::Operations;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    #[test]
    fn notification_shutdown_cancels_admitted_operations_and_waits_for_cleanup() {
        let operations = Arc::new(Operations::default());
        std::thread::scope(|threads| {
            let active = Arc::new(AtomicBool::new(true));
            let hold = operations.acquire(&active).unwrap();
            operations.cancel().unwrap();
            assert!(!active.load(Ordering::Acquire));
            assert!(
                operations
                    .acquire(&Arc::new(AtomicBool::new(true)))
                    .is_err()
            );
            let (started, begin) = mpsc::channel();
            let (finished, end) = mpsc::channel();
            let worker = operations.clone();
            threads.spawn(move || {
                started.send(()).unwrap();
                worker.finish().unwrap();
                finished.send(()).unwrap();
            });
            begin.recv().unwrap();
            assert_eq!(end.try_recv(), Err(mpsc::TryRecvError::Empty));
            drop(hold);
            end.recv().unwrap();
        });
    }

    #[test]
    fn canceled_or_closed_scope_never_acquires_a_new_physical_operation() {
        let predecessor = Arc::new(Operations::default());
        assert!(
            predecessor
                .acquire(&Arc::new(AtomicBool::new(false)))
                .is_err()
        );
        predecessor.finish().unwrap();
        let successor = Arc::new(Operations::default());
        let active = Arc::new(AtomicBool::new(true));
        let hold = successor.acquire(&active).unwrap();
        predecessor.cancel().unwrap();
        assert!(active.load(Ordering::Acquire));
        assert!(predecessor.acquire(&active).is_err());
        drop(hold);
        successor.finish().unwrap();
    }
}
