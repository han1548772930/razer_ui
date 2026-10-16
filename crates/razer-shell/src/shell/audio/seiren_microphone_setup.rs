//! UI7405 global localStorage micSetupPopupSeen, outside device profiles.
use super::*;
use razer_storage::source_local;

#[derive(Default)]
pub(super) struct State {
    seen: Option<bool>,
    epoch: u64,
    tasks: Vec<Task<()>>,
}
impl AppShell {
    pub(super) fn install_seiren_microphone_setup_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, _| {
            let tasks = std::mem::take(&mut this.seiren_microphone_setup.tasks);
            async move {
                for task in tasks {
                    task.await;
                }
            }
        }));
    }
    pub(super) fn restore_seiren_microphone_setup_seen(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(workspace.read(cx).device(cx).product_id, 1422 | 1446) {
            return;
        }
        if let Some(seen) = self.seiren_microphone_setup.seen {
            workspace.update(cx, |workspace, cx| {
                workspace.set_seiren_microphone_setup_seen(seen, window, cx)
            });
            return;
        }
        let workspace = workspace.clone();
        let epoch = self.seiren_microphone_setup.epoch;
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    source_local::get(&source_local::path(), "micSetupPopupSeen")
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.seiren_microphone_setup.epoch != epoch {
                    return;
                }
                match result {
                    Ok(value) => {
                        let seen = value.and_then(|value| value.as_bool()).unwrap_or(false);
                        this.seiren_microphone_setup.seen = Some(seen);
                        workspace.update(cx, |workspace, cx| {
                            workspace.set_seiren_microphone_setup_seen(seen, window, cx)
                        });
                    }
                    Err(error) => {
                        eprintln!("[microphone setup local storage] read failed: {error:#}")
                    }
                }
            });
        });
        self.seiren_microphone_setup.tasks.push(task);
    }
    pub(super) fn save_seiren_microphone_setup_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.seiren_microphone_setup.seen = Some(seen);
        self.seiren_microphone_setup.epoch = self.seiren_microphone_setup.epoch.wrapping_add(1);
        // The source closes optimistically. A failed local write is diagnostic;
        // it must not invent a reopened wizard or a device-save acknowledgement.
        let task = cx.spawn(async move |_, cx| {
            let result = cx
                .background_spawn(async move {
                    source_local::set(
                        &source_local::path(),
                        "micSetupPopupSeen",
                        &serde_json::json!(seen),
                    )
                })
                .await;
            if let Err(error) = result {
                eprintln!("[microphone setup local storage] write failed: {error:#}");
            }
        });
        self.seiren_microphone_setup.tasks.push(task);
    }
}
