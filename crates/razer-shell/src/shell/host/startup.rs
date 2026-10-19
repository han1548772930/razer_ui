//! Settings owns optimistic editor values; this owner serializes real OS work.
use super::AppShell;
use gpui_kit::*;
use razer_settings::settings_page::StartupSetting;
use razer_state::LocalStartupDraft;

#[derive(Clone, Copy)]
enum Work {
    Read(LocalStartupDraft),
    Write(LocalStartupDraft),
}

#[derive(Default)]
pub(super) struct State {
    task: Option<Task<()>>,
    pending: Option<Work>,
    revision: u64,
    stopping: bool,
    launched_hidden: bool,
}

impl State {
    fn request(&mut self, draft: LocalStartupDraft) {
        self.revision = self.revision.wrapping_add(1);
        self.pending = Some(Work::Write(draft));
    }

    fn can_observe(&self, revision: u64) -> bool {
        !self.stopping && self.revision == revision
    }
}

impl AppShell {
    pub(in crate::shell) fn install_startup(&mut self, cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, cx| {
            shell.startup.stopping = false;
            shell.run_startup_work(cx);
        });
        let draft = self
            .settings
            .read(cx)
            .snapshot()
            .startup_draft
            .unwrap_or_default();
        let arguments: Vec<_> = std::env::args().collect();
        self.startup.launched_hidden =
            razer_platform::startup::starts_hidden(arguments.iter().map(String::as_str));
        self.startup.pending = Some(Work::Read(draft));
        self.run_startup_work(cx);
        self.register_exit_cleanup(|this, cx| {
            this.startup.stopping = true;
            let task = this.startup.task.take();
            let pending = this.startup.pending.take();
            let executor = cx.background_executor().clone();
            async move {
                if let Some(task) = task {
                    task.await;
                }
                // A latest click queued behind an active operation must reach
                // the OS before exit. A query has no mutation to flush.
                if let Some(Work::Write(draft)) = pending {
                    let result = executor
                        .spawn(async move {
                            razer_platform::startup::set_auto_start(
                                draft.auto_start,
                                draft.start_minimized,
                            )
                        })
                        .await;
                    if let Err(error) = result {
                        eprintln!("[startup] final OS registration failed: {error:#}");
                    }
                }
            }
        });
    }

    pub(in crate::shell) fn request_startup(
        &mut self,
        setting: StartupSetting,
        cx: &mut Context<Self>,
    ) {
        let mut draft = self
            .settings
            .read(cx)
            .snapshot()
            .startup_draft
            .unwrap_or_default();
        match setting {
            StartupSetting::AutoStart(value) => draft.auto_start = value,
            StartupSetting::MinimizedOnStartup(value) => draft.start_minimized = value,
        }
        self.startup.request(draft);
        self.run_startup_work(cx);
    }

    fn run_startup_work(&mut self, cx: &mut Context<Self>) {
        if self.startup.stopping || self.startup.task.is_some() {
            return;
        }
        let Some(work) = self.startup.pending.take() else {
            return;
        };
        let revision = self.startup.revision;
        self.startup.task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    match work {
                        Work::Read(mut draft) => {
                            draft.auto_start = razer_platform::startup::auto_start()?;
                            Ok(Some(draft))
                        }
                        Work::Write(draft) => razer_platform::startup::set_auto_start(
                            draft.auto_start,
                            draft.start_minimized,
                        )
                        .map(|()| None),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.startup.task = None;
                match result {
                    Ok(Some(draft)) if this.startup.can_observe(revision) => {
                        this.settings
                            .update(cx, |settings, cx| settings.observe_startup(draft, cx));
                    }
                    // Source setters have no acknowledgement-driven rollback
                    // or getter refresh. Only actual initial queries update UI.
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("[startup] local OS operation failed: {error:#}");
                        this.status = format!("启动设置操作失败：{error}");
                        cx.notify();
                    }
                }
                this.run_startup_work(cx);
            });
        }));
    }

    pub(in crate::shell) fn reveal_startup_without_tray(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.startup.launched_hidden && self.tray.is_none() {
            self.startup.launched_hidden = false;
            // Match the shell's existing reachable-window fallback when tray
            // registration fails; never strand a hidden standalone process.
            razer_tray::show_main(window);
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalStartupDraft, State, Work};

    #[test]
    fn newer_edits_suppress_initial_readback_and_keep_the_latest_os_write() {
        let mut state = State::default();
        assert!(state.can_observe(0));
        let mut draft = LocalStartupDraft::default();
        draft.auto_start = false;
        state.request(draft);
        draft.auto_start = true;
        draft.start_minimized = false;
        state.request(draft);
        assert!(!state.can_observe(0));
        assert!(matches!(state.pending.take(), Some(Work::Write(value)) if value == draft));
        assert!(state.pending.is_none());
        state.stopping = true;
        assert!(!state.can_observe(state.revision));
    }
}
