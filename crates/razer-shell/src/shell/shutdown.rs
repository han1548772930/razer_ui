//! Application-owned cleanup starts after accepted product work drains and
//! completes before framework quit callbacks enter their bounded grace period.
use super::*;
use std::{future::Future, pin::Pin};

type Completion = Pin<Box<dyn Future<Output = ()> + 'static>>;
type Cleanup = Box<dyn FnMut(&mut AppShell, &mut Context<AppShell>) -> Completion>;
type Resume = Box<dyn FnMut(&mut AppShell, &mut Context<AppShell>)>;

#[derive(Default)]
enum Phase {
    #[default]
    Running,
    Cleaning { cancel_requested: bool },
    Cleaned,
}

impl Phase {
    fn begin(&mut self) -> bool {
        if !matches!(self, Self::Running) {
            return false;
        }
        *self = Self::Cleaning { cancel_requested: false };
        true
    }

    fn defer_cancel(&mut self) -> bool {
        match self {
            Self::Cleaning { cancel_requested } => {
                *cancel_requested = true;
                true
            }
            Self::Running | Self::Cleaned => false,
        }
    }

    fn complete(&mut self) -> bool {
        let cancelled = matches!(self, Self::Cleaning { cancel_requested: true });
        *self = Self::Cleaned;
        cancelled
    }

    fn resume(&mut self) -> bool {
        if !matches!(self, Self::Cleaned) {
            return false;
        }
        *self = Self::Running;
        true
    }
}

#[derive(Default)]
pub(super) struct State {
    hooks: Vec<Cleanup>,
    resumes: Vec<Resume>,
    phase: Phase,
    task: Option<Task<()>>,
}

impl AppShell {
    pub(super) fn register_exit_cleanup<F, Fut>(&mut self, mut cleanup: F)
    where
        F: FnMut(&mut Self, &mut Context<Self>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.service_exit
            .hooks
            .push(Box::new(move |shell, cx| Box::pin(cleanup(shell, cx))));
    }

    pub(super) fn register_exit_resume(
        &mut self,
        resume: impl FnMut(&mut Self, &mut Context<Self>) + 'static,
    ) {
        self.service_exit.resumes.push(Box::new(resume));
    }

    pub(super) fn defer_service_exit_cancellation(&mut self) -> bool {
        self.service_exit.phase.defer_cancel()
    }

    pub(super) fn resume_service_exit(&mut self, cx: &mut Context<Self>) {
        if !self.service_exit.phase.resume() {
            return;
        }
        let mut resumes = std::mem::take(&mut self.service_exit.resumes);
        for resume in &mut resumes {
            resume(self, cx);
        }
        resumes.append(&mut self.service_exit.resumes);
        self.service_exit.resumes = resumes;
    }

    fn take_exit_cleanups(&mut self, cx: &mut Context<Self>) -> Vec<Completion> {
        // Invoke every hook before waiting so independent cancellation starts
        // together. Each feature retains its own verified release order.
        let mut hooks = std::mem::take(&mut self.service_exit.hooks);
        let completions = hooks
            .iter_mut()
            .map(|cleanup| cleanup(self, cx))
            .collect();
        hooks.append(&mut self.service_exit.hooks);
        self.service_exit.hooks = hooks;
        completions
    }

    pub(super) fn install_service_exit_fallback(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|shell, cx| {
            let retained = shell.service_exit.task.take();
            let completions = if retained.is_none() && shell.service_exit.phase.begin() {
                shell.take_exit_cleanups(cx)
            } else {
                Vec::new()
            };
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
        if !self.service_exit.phase.begin() {
            return false;
        }
        let completions = self.take_exit_cleanups(cx);
        self.service_exit.task = Some(cx.spawn(async move |shell, cx| {
            for completion in completions {
                completion.await;
            }
            let _ = shell.update(cx, |shell, cx| {
                shell.service_exit.task = None;
                let cancelled = shell.service_exit.phase.complete();
                if cancelled {
                    shell.cancel_pending_exit(cx);
                } else if shell.close_requested {
                    shell.continue_save_queue(true, cx);
                }
            });
        }));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::Phase;

    #[test]
    fn exit_cancellation_waits_for_all_cleanup_completions() {
        let mut phase = Phase::default();
        assert!(phase.begin());
        assert!(phase.defer_cancel());
        assert!(!phase.resume());
        assert!(!phase.begin());
        assert!(phase.defer_cancel());
        assert!(phase.complete());
        assert!(phase.resume());
        assert!(!phase.resume());
        assert!(phase.begin());
        assert!(!phase.complete());
    }

    #[test]
    fn completed_exit_does_not_repeat_cleanup_until_recovery() {
        let mut phase = Phase::default();
        assert!(!phase.defer_cancel());
        assert!(phase.begin());
        assert!(!phase.complete());
        assert!(!phase.begin());
        assert!(!phase.defer_cancel());
        assert!(phase.resume());
        assert!(phase.begin());
    }
}
