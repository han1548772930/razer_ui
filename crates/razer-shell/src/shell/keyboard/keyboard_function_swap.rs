//! Explicit Fn-key changes finish their setter before entering profile storage.
use super::*;
use razer_pages::features::KeyboardFunctionSwapRequested;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Clone)]
struct Pending {
    workspace: Entity<ProductWorkspace>,
    request: KeyboardFunctionSwapRequested,
    identity: String,
    connection: u64,
    window: AnyWindowHandle,
}
impl Pending {
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        shell.devices.contains(&self.workspace)
            && self.workspace.read(cx).identity(cx) == self.identity
            && self.workspace.read(cx).connection_revision() == self.connection
            && self
                .workspace
                .read(cx)
                .keyboard_function_swap_current(&self.request, cx)
    }
}
struct Worker {
    pending: Pending,
    active: Arc<AtomicBool>,
    task: Task<()>,
}
#[derive(Default)]
pub(super) struct State {
    registered: bool,
    queued: BTreeMap<EntityId, VecDeque<Pending>>,
    workers: BTreeMap<EntityId, Worker>,
    pump: Option<Task<()>>,
}
impl State {
    pub(super) fn pending(&self) -> bool {
        !self.workers.is_empty() || self.queued.values().any(|queue| !queue.is_empty())
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.active.store(false, Ordering::Release);
        }
    }
}

impl AppShell {
    pub(super) fn request_keyboard_function_swap(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: KeyboardFunctionSwapRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .keyboard_function_swap_current(&request, cx)
        {
            return;
        }
        if self.close_requested || self.storage_error.is_some() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_function_swap(
                    &request,
                    Err(
                        "Function-key change cannot enter a closing or unavailable save queue"
                            .into(),
                    ),
                    cx,
                );
            });
            return;
        }
        let key = workspace.entity_id();
        if self
            .keyboard_function_swap
            .workers
            .get(&key)
            .is_some_and(|worker| worker.pending.request.id() == request.id())
            || self
                .keyboard_function_swap
                .queued
                .get(&key)
                .is_some_and(|queue| {
                    queue
                        .iter()
                        .any(|pending| pending.request.id() == request.id())
                })
        {
            return;
        }
        let pending = Pending {
            identity: workspace.read(cx).identity(cx),
            connection: workspace.read(cx).connection_revision(),
            workspace,
            request,
            window: window.window_handle(),
        };
        self.keyboard_function_swap
            .queued
            .entry(key)
            .or_default()
            .push_back(pending);
        if !self.keyboard_function_swap.registered {
            self.keyboard_function_swap.registered = true;
            self.register_exit_cleanup(|shell, _| {
                shell.keyboard_function_swap.pump = None;
                shell.keyboard_function_swap.queued.clear();
                let workers = std::mem::take(&mut shell.keyboard_function_swap.workers);
                for worker in workers.values() {
                    worker.active.store(false, Ordering::Release);
                }
                async move {
                    for (_, worker) in workers {
                        worker.task.await;
                    }
                }
            });
        }
        self.start_keyboard_function_swaps(cx);
        if self.keyboard_function_swap.pump.is_none() {
            self.keyboard_function_swap.pump = Some(cx.spawn(async move |shell, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(25))
                        .await;
                    if !shell
                        .update(cx, |shell, cx| {
                            for worker in shell.keyboard_function_swap.workers.values() {
                                if !worker.pending.current(shell, cx) {
                                    worker.active.store(false, Ordering::Release);
                                }
                            }
                            shell.start_keyboard_function_swaps(cx);
                            let pending = shell.keyboard_function_swap.pending();
                            if !pending {
                                shell.keyboard_function_swap.pump = None;
                                if shell.close_requested && shell.save_task.is_none() {
                                    shell.continue_save_queue(true, cx);
                                }
                            }
                            pending
                        })
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            }));
        }
    }

    fn start_keyboard_function_swaps(&mut self, cx: &mut Context<Self>) {
        let keys = self
            .keyboard_function_swap
            .queued
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for key in keys {
            if self.keyboard_function_swap.workers.contains_key(&key) {
                continue;
            }
            let pending = loop {
                let next = self
                    .keyboard_function_swap
                    .queued
                    .get_mut(&key)
                    .and_then(VecDeque::pop_front);
                let Some(next) = next else {
                    break None;
                };
                if next.current(self, cx) {
                    break Some(next);
                }
                next.workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_function_swap(
                        &next.request,
                        Err("Function-key request scope changed before transmission".into()),
                        cx,
                    );
                });
            };
            let Some(pending) = pending else {
                self.keyboard_function_swap.queued.remove(&key);
                continue;
            };
            let routes = self
                .device_observations
                .iter()
                .filter(|route| route.matches(pending.workspace.read(cx).device(cx)))
                .collect::<Vec<_>>();
            let [route] = routes.as_slice() else {
                pending.workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_function_swap(
                        &pending.request,
                        Err("Function-key setter has no unique current device observation".into()),
                        cx,
                    );
                });
                continue;
            };
            let route = (*route).clone();
            let active = Arc::new(AtomicBool::new(true));
            let guard = active.clone();
            let current = pending.clone();
            let primary = pending.request.primary();
            let task = cx.spawn(async move |shell, cx| {
                let completion = cx
                    .background_spawn(async move {
                        razer_service::keyboard_function_swap::write(route, primary, guard)
                    })
                    .await;
                let _ = shell.update(cx, |shell, cx| {
                    let result = match (completion.result, completion.cleanup_failure) {
                        (Ok(report), None) => Ok(report),
                        (Err(error), None) => Err(error),
                        (Ok(_), Some(error)) => Err(format!(
                            "Function-key setter acknowledged, but cleanup failed: {error}"
                        )),
                        (Err(error), Some(cleanup)) => Err(format!("{error}; cleanup: {cleanup}")),
                    };
                    let result = if current.current(shell, cx) {
                        result
                    } else {
                        Err(format!(
                            "Function-key connection or request changed{}",
                            result
                                .err()
                                .map(|error| format!("; {error}"))
                                .unwrap_or_default()
                        ))
                    };
                    let snapshot = current.workspace.update(cx, |workspace, cx| {
                        workspace.finish_keyboard_function_swap(&current.request, result, cx)
                    });
                    shell.keyboard_function_swap.workers.remove(&key);
                    if let Some(snapshot) = snapshot {
                        shell.queue_function_swap_profile(&current, snapshot, cx);
                    }
                    shell.start_keyboard_function_swaps(cx);
                    if shell.close_requested && shell.save_task.is_none() {
                        shell.continue_save_queue(true, cx);
                    }
                });
            });
            self.keyboard_function_swap.workers.insert(
                key,
                Worker {
                    pending,
                    active,
                    task,
                },
            );
        }
    }

    fn queue_function_swap_profile(
        &mut self,
        pending: &Pending,
        snapshot: Device,
        cx: &mut Context<Self>,
    ) {
        if let Some(error) = &self.storage_error {
            self.status =
                format!("Function-key setter acknowledged; profile was not saved: {error}");
            self.cancel_pending_exit(cx);
            cx.notify();
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|workspace| workspace.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            pending.window,
            file,
            Some(ProductSave::MouseProfile(MouseLegacyProfileSave {
                workspace: pending.workspace.clone(),
                scope: None,
                identity: pending.identity.clone(),
                device: snapshot,
                apply: false,
                scroll: None,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }
}
