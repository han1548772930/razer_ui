//! Application-owned cleanup starts after accepted product work drains and
//! completes before framework quit callbacks enter their bounded grace period.
use super::*;
use std::{future::Future, pin::Pin};

type Completion = Pin<Box<dyn Future<Output = ()> + 'static>>;
type Cleanup = Box<dyn FnOnce(&mut AppShell, &mut Context<AppShell>) -> Completion>;

#[derive(Default)]
pub(super) struct State {
    hooks: Vec<Cleanup>,
    task: Option<Task<()>>,
}

impl AppShell {
    pub(super) fn register_exit_cleanup<F, Fut>(&mut self, cleanup: F)
    where
        F: FnOnce(&mut Self, &mut Context<Self>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.service_exit
            .hooks
            .push(Box::new(move |shell, cx| Box::pin(cleanup(shell, cx))));
    }

    fn take_exit_cleanups(&mut self, cx: &mut Context<Self>) -> Vec<Completion> {
        // Invoke every hook before waiting so independent cancellation starts
        // together. Each feature retains its own verified release order.
        std::mem::take(&mut self.service_exit.hooks)
            .into_iter()
            .map(|cleanup| cleanup(self, cx))
            .collect()
    }

    pub(super) fn install_service_exit_fallback(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|shell, cx| {
            let retained = shell.service_exit.task.take();
            let completions = shell.take_exit_cleanups(cx);
            async move {
                if let Some(retained) = retained {
                    retained.await;
                }
                for completion in completions {
                    completion.await;
                }
            }
        }));
    }

    pub(super) fn finish_service_exit(&mut self, cx: &mut Context<Self>) -> bool {
        if self.service_exit.task.is_some() {
            return true;
        }
        if self.service_exit.hooks.is_empty() {
            return false;
        }
        let completions = self.take_exit_cleanups(cx);
        self.service_exit.task = Some(cx.spawn(async move |shell, cx| {
            for completion in completions {
                completion.await;
            }
            let _ = shell.update(cx, |shell, cx| {
                shell.service_exit.task = None;
                if shell.close_requested {
                    shell.continue_save_queue(true, cx);
                }
            });
        }));
        true
    }
}
