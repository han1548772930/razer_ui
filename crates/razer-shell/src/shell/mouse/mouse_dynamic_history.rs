//! Connection-scoped speed publications outlive individual capture requests.
use super::*;
use service::{SpeedHistory, SpeedHistoryInput};

pub(super) struct HistoryOwner {
    identity: String,
    workspace: WeakEntity<ProductWorkspace>,
    connection: MouseDynamicConnection,
    route: ObservedDevice,
    revision: u64,
    pub(super) input: SpeedHistoryInput,
    pub(super) delivery: Task<()>,
}
impl HistoryOwner {
    pub(super) fn matches(
        &self,
        shell: &AppShell,
        observations: &[ObservedDevice],
        cx: &App,
    ) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        !self.input.is_closed()
            && shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && workspace.read(cx).connection_revision() == self.revision
            && workspace
                .read(cx)
                .mouse_dynamic_connection_is_current(self.connection, cx)
            && super::super::device_discovery::retained_connection(
                std::slice::from_ref(&self.route),
                observations,
                workspace.read(cx).device(cx),
            )
            && direct::mouse_dynamic::available(&self.route)
    }
}

impl AppShell {
    pub(super) fn register_mouse_dynamic_observer(
        &mut self,
        queued: &Queued,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let identity = queued.workspace.read(cx).identity(cx);
        let connection = queued.request.scope().connection();
        let history = self
            .mouse_dynamic
            .histories
            .values()
            .find(|history| {
                history.identity == identity
                    && history.connection == connection
                    && history.matches(self, &self.device_observations, cx)
            })
            .ok_or_else(|| anyhow::anyhow!("PID226 dynamic history is not retained"))?;
        history.input.register_observer()
    }

    pub(super) fn unregister_mouse_dynamic_observer(
        &mut self,
        queued: &Queued,
        cx: &mut Context<Self>,
    ) {
        let identity = queued.workspace.read(cx).identity(cx);
        let connection = queued.request.scope().connection();
        if let Some(history) = self.mouse_dynamic.histories.values().find(|history| {
            history.identity == identity
                && history.connection == connection
                && history.matches(self, &self.device_observations, cx)
        }) {
            history.input.unregister_observer();
        }
    }

    pub(super) fn mouse_dynamic_history(
        &mut self,
        queued: &Queued,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<SpeedHistoryInput> {
        let connection = queued.request.scope().connection();
        let identity = queued.workspace.read(cx).identity(cx);
        for history in self.mouse_dynamic.histories.values() {
            if history.identity == identity {
                if history.connection == connection
                    && history.matches(self, &self.device_observations, cx)
                {
                    history.input.register_observer()?;
                    return Ok(history.input.clone());
                }
                history.input.close();
            }
        }
        let (input, history) = SpeedHistory::new(queued.route.product_id())?;
        let id = self.mouse_dynamic.allocate_id();
        let mailbox = Arc::new(Mutex::new(None));
        let output = mailbox.clone();
        let executor = cx.background_executor().clone();
        let worker = cx.background_spawn(async move {
            history
                .run(
                    |speed| {
                        *output.lock().unwrap_or_else(|error| error.into_inner()) = Some(speed);
                    },
                    |delay| executor.timer(delay),
                )
                .await;
        });
        let closing = input.clone();
        let workspace = queued.workspace.downgrade();
        let window = queued.window;
        let delivery = cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let speed = mailbox.try_lock().ok().and_then(|mut slot| slot.take());
                let retained = shell.update(cx, |shell, cx| {
                    if !shell
                        .mouse_dynamic
                        .histories
                        .get(&id)
                        .is_some_and(|history| {
                            history.matches(shell, &shell.device_observations, cx)
                        })
                    {
                        return false;
                    }
                    if let (Some(speed), Some(workspace)) = (speed, workspace.upgrade()) {
                        let _ = window.update(cx, |_, window, cx| {
                            workspace.update(cx, |workspace, cx| {
                                workspace
                                    .observe_mouse_dynamic_speed(connection, speed, window, cx);
                            });
                        });
                    }
                    true
                });
                if !retained.unwrap_or(false) {
                    closing.close();
                }
                if closing.is_closed() {
                    break;
                }
            }
            worker.await;
            let _ = shell.update(cx, |shell, _| {
                shell.mouse_dynamic.histories.remove(&id);
            });
        });
        self.mouse_dynamic.histories.insert(
            id,
            HistoryOwner {
                identity,
                workspace: queued.workspace.downgrade(),
                connection,
                route: queued.route.clone(),
                revision: queued.revision,
                input: input.clone(),
                delivery,
            },
        );
        Ok(input)
    }
}
