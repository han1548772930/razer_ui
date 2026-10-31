//! Source xa/Gr countdown and recorder state, fed by the retained recording actor.
use super::recording_actor::{Command, Update};
use super::*;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    Countdown(u8),
    Starting,
    Recording,
    Stopping,
}
#[derive(Default)]
pub struct RecordingUi {
    pub stage: Stage,
    pub error: Option<String>,
    pub status: String,
    pub count: usize,
    pub preview: Vec<ActionItem>,
    preview_rows: Vec<ActionItem>,
    pub preview_offset: usize,
    pub scroll: ScrollHandle,
    generation: u64,
    command: Option<recording_actor::Session>,
    countdown_task: Option<Task<()>>,
    updates_task: Option<Task<()>>,
    cancelled: bool,
    committed_count: Option<usize>,
}
impl MacroPage {
    pub fn recording_busy(&self) -> bool {
        self.recording.stage != Stage::Idle
    }
    pub fn recording_controls_active(&self) -> bool {
        matches!(
            self.recording.stage,
            Stage::Starting | Stage::Recording | Stage::Stopping
        )
    }
    pub fn record_countdown_label(&self) -> Option<String> {
        let Stage::Countdown(n) = self.recording.stage else {
            return None;
        };
        Some(tr("RECORDING_IN").replace("{{time}}", &n.to_string()))
    }
    pub fn record_label(&self) -> String {
        match self.recording.stage {
            Stage::Idle => tr("TEXT_ACTION_BAR_RECORD").to_uppercase(),
            Stage::Countdown(_) => tr("TEXT_ACTION_BAR_RECORD").to_uppercase(),
            Stage::Starting => "正在开始…".into(),
            Stage::Recording => tr("TEXT_ACTION_BAR_STOP").to_uppercase(),
            Stage::Stopping => "正在停止…".into(),
        }
    }
    pub fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_recording_with(false, window, cx);
    }
    pub(super) fn toggle_recording_from_shortcut(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // xa's start-recording-event listener belongs to the mounted editor.
        // br can still stop an active recorder independently of that listener.
        if self.tab != MacroTab::MyMacros && !self.recording_controls_active() {
            return;
        }
        self.toggle_recording_with(true, window, cx);
    }
    fn toggle_recording_with(
        &mut self,
        shortcut: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.recording.stage {
            Stage::Countdown(_) => {
                self.cancel_recording(cx);
                return;
            }
            Stage::Starting | Stage::Recording => {
                if let Some(sender) = &self.recording.command {
                    let _ = sender.send(if shortcut {
                        Command::StopShortcut
                    } else {
                        Command::Stop
                    });
                }
                self.recording.stage = Stage::Stopping;
                self.recording.status = "正在等待原生录制停止…".into();
                cx.notify();
                return;
            }
            Stage::Stopping => return,
            Stage::Idle => {}
        }
        if self.current.is_none() || self.tutorial != Tutorial::Complete {
            return;
        }
        self.finish_pending_edits(window, cx);
        self.dismiss_transient_ui(window, cx);
        self.recording.error = None;
        self.recording.count = 0;
        self.recording.preview = self.actions.clone();
        self.recording.preview_rows.clear();
        self.recording.preview_offset = 0;
        self.recording.cancelled = false;
        self.recording.committed_count = None;
        self.recording.generation = self.recording.generation.wrapping_add(1);
        let generation = self.recording.generation;
        let delay = self
            .record_ui
            .start
            .read(cx)
            .selected_value()
            .and_then(|s| s.strip_suffix('s')?.parse::<u8>().ok())
            .unwrap_or(0);
        if delay == 0 {
            self.start_recording(cx);
            return;
        }
        self.recording.stage = Stage::Countdown(delay);
        self.recording.status = format!("将在 {delay} 秒后开始录制");
        cx.notify();
        self.recording.countdown_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let keep = this
                    .update(cx, |this, cx| {
                        if this.recording.generation != generation {
                            return false;
                        }
                        let Stage::Countdown(n) = this.recording.stage else {
                            return false;
                        };
                        if n <= 1 {
                            this.start_recording(cx);
                            false
                        } else {
                            this.recording.stage = Stage::Countdown(n - 1);
                            this.recording.status = format!("将在 {} 秒后开始录制", n - 1);
                            cx.notify();
                            true
                        }
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }
    fn start_recording(&mut self, cx: &mut Context<Self>) {
        self.recording.countdown_task = None;
        let options = match self.recording_options(cx) {
            Ok(options) => options,
            Err(error) => {
                self.recording.stage = Stage::Idle;
                self.recording.preview.clear();
                self.recording.status = "无法开始录制，未更改草稿".into();
                self.recording.error = Some(error);
                cx.notify();
                return;
            }
        };
        let (sender, receiver) = match recording_actor::spawn(options) {
            Ok(session) => session,
            Err(error) => {
                self.recording.stage = Stage::Idle;
                self.recording.preview.clear();
                self.recording.status = "无法开始录制，未更改草稿".into();
                self.recording.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.recording.command = Some(sender);
        self.recording.stage = Stage::Starting;
        self.recording.status = "正在等待原生录制 started 事件…".into();
        let generation = self.recording.generation;
        cx.notify();
        self.recording.updates_task = Some(cx.spawn(async move |this, cx| {
            while let Ok(update) = receiver.recv().await {
                let finished = matches!(&update, Update::Finished(_));
                if this
                    .update(cx, |this, cx| {
                        this.receive_recording_update(generation, update, cx);
                    })
                    .is_err()
                    || finished
                {
                    return;
                }
            }
            // An actor that exits without Finished cannot leave the editor
            // permanently locked or commit its partial preview as a draft.
            let _ = this.update(cx, |this, cx| {
                if this.recording.generation == generation && this.recording.command.is_some() {
                    this.recording.command = None;
                    this.recording.stage = Stage::Idle;
                    this.recording.preview.clear();
                    this.recording.preview_rows.clear();
                    this.recording.status = if this.recording.committed_count.is_some() {
                        "录制连接中断，已追加的本地草稿仍然保留".into()
                    } else {
                        "录制连接中断，未更改草稿".into()
                    };
                    this.recording.error =
                        Some("录制线程未返回完成结果；无法确认原生会话清理状态".into());
                    cx.notify();
                }
            });
        }));
    }
    pub(super) fn refresh_recording_preview(&mut self) {
        if self.recording_busy() {
            self.recording.preview.clear();
            if self.current.is_some() && self.actions_for == self.current {
                self.recording.preview.extend(self.actions.iter().cloned());
                self.recording
                    .preview
                    .extend(self.recording.preview_rows.iter().cloned());
            }
        }
    }

    pub(super) fn update_recording_preview_options(&mut self, cx: &App) {
        if let Some(sender) = &self.recording.command {
            match self.recording_options(cx) {
                Ok(options) => {
                    if let Err(error) = sender.send(Command::PreviewOptions(options)) {
                        self.recording.error = Some(error);
                    }
                }
                Err(error) => self.recording.error = Some(error),
            }
        }
    }

    pub(super) fn receive_recording_update(
        &mut self,
        generation: u64,
        update: Update,
        cx: &mut Context<Self>,
    ) {
        if self.recording.generation != generation {
            return;
        }
        self.accept_recording_update(update, cx);
        cx.notify();
    }

    pub(super) fn accept_recording_update(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::WindowStorage(change) => cx.emit(change),
            Update::GlobalShortcutsRestored => cx.emit(MacroGlobalShortcutsRestored),
            Update::Recording => {
                if self.recording.stage != Stage::Stopping {
                    self.recording.stage = Stage::Recording;
                    self.recording.status = "正在全局录制".into();
                }
            }
            Update::Progress(count) => self.recording.count = count,
            Update::PreviewFailed(error) | Update::ObservationFailed(error) => {
                self.recording.error = Some(error)
            }
            Update::Preview { first_row, rows } => {
                self.recording.preview_offset = first_row;
                self.recording.preview_rows = rows;
                self.refresh_recording_preview();
                self.recording.scroll.scroll_to_bottom();
            }
            Update::PostProcessing(request) => {
                self.recording.stage = Stage::Stopping;
                let options = if self.recording.cancelled {
                    Err("录制已取消".into())
                } else {
                    self.recording_options(cx)
                };
                if let Err(error) = request.complete(options) {
                    self.recording.error = Some(error);
                }
            }
            Update::Commit(request) => {
                if let Err(error) = request.complete(|rows| self.commit_recorded_actions(rows, cx))
                {
                    self.recording.error = Some(error);
                }
            }
            Update::Finished(result) => {
                self.recording.command = None;
                self.recording.stage = Stage::Idle;
                self.recording.preview.clear();
                self.recording.preview_rows.clear();
                match result {
                    Ok(recording_actor::Completion::Recorded { count })
                        if !self.recording.cancelled =>
                    {
                        self.recording.status =
                            format!("录制完成，新增 {count} 行本地草稿；保存仅写入本机");
                    }
                    Ok(_) => {
                        self.recording.status = self.recording.committed_count.map_or_else(
                            || "录制已取消，未更改草稿".into(),
                            |count| format!("录制已取消，保留已追加的 {count} 行本地草稿"),
                        );
                    }
                    Err(error) => {
                        self.recording.status = if self.recording.committed_count.is_some() {
                            "录制清理失败，已追加的本地草稿仍然保留".into()
                        } else {
                            "录制失败，未更改草稿".into()
                        };
                        self.recording.error = Some(error);
                    }
                }
            }
        }
    }

    pub(super) fn commit_recorded_actions(
        &mut self,
        mut rows: Vec<ActionItem>,
        cx: &App,
    ) -> Result<usize, String> {
        if self.recording.cancelled {
            return Err("录制已取消，未提交动作".into());
        }
        // h.nu reads currentProfile again when its locked reducer runs. It
        // appends to the current events and retains actionLoad and selection.
        let options = self.recording_commit_options(cx)?;
        rebase_recorded_pairs(&mut rows, options.next_pair)?;
        let entry = self
            .entries
            .iter()
            .find(|entry| Some(entry.id) == self.current)
            .ok_or("当前宏文档不存在，无法提交录制结果")?;
        let append = self.library_snapshot.append_recorded_actions(
            entry,
            &self.actions,
            self.execution.as_ref(),
            rows,
            &options,
        )?;
        let count = append.appended;
        self.actions = append.actions;
        self.execution = Some(append.execution);
        self.actions_dirty = true;
        self.recording.committed_count = Some(count);
        self.recording.preview_rows.clear();
        self.refresh_recording_preview();
        Ok(count)
    }
    /// Called by shell close/navigation. Actor shutdown stays on its thread.
    pub fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        // Cancellation remains authoritative even if the actor has already
        // stopped and is restoring mappings before delivering Finished.
        self.recording.cancelled = true;
        if let Some(sender) = &self.recording.command {
            let _ = sender.send(Command::Cancel);
            self.recording.stage = Stage::Stopping;
            self.recording.status = "正在取消原生录制…".into();
        } else {
            self.recording.countdown_task = None;
            self.recording.generation = self.recording.generation.wrapping_add(1);
            self.recording.stage = Stage::Idle;
            self.recording.preview.clear();
            self.recording.preview_rows.clear();
            self.recording.status = "录制已取消，未更改草稿".into();
        }
        cx.notify();
    }
    pub fn recording_status(&self) -> AnyElement {
        v_flex()
            .id("macro-recording-status")
            .test_support()
            .w_full()
            .px(css(12.))
            .py(css(6.))
            .text_size(css(12.))
            .bg(rgb(0x111111))
            .aria_label(format!(
                "{}；原生输入事件 {}",
                self.recording.status, self.recording.count
            ))
            .when(!self.recording.status.is_empty(), |v| {
                v.child(format!(
                    "{}{}",
                    self.recording.status,
                    if self.recording_busy() {
                        format!(" ({})", self.recording.count)
                    } else {
                        String::new()
                    }
                ))
            })
            .when_some(self.recording.error.clone(), |v, error| {
                v.child(
                    div()
                        .id("macro-recording-error")
                        .test_support()
                        .aria_label(error.clone())
                        .text_color(rgb(0xe65a5a))
                        .child(error),
                )
            })
            .when_some(
                self.record_ui.shortcut.registration_error.clone(),
                |v, error| {
                    v.child(
                        div()
                            .id("macro-record-shortcut-error")
                            .test_support()
                            .aria_label(error.clone())
                            .text_color(rgb(0xe65a5a))
                            .child(error),
                    )
                },
            )
            .into_any_element()
    }
}

fn rebase_recorded_pairs(rows: &mut [ActionItem], mut next: u64) -> Result<(), String> {
    let mut pairs = std::collections::HashMap::new();
    for row in rows {
        let pair = row
            .keyboard
            .as_mut()
            .map(|key| &mut key.pair_id)
            .or_else(|| row.mouse.as_mut().map(|mouse| &mut mouse.pair_id));
        if let Some(Some(id)) = pair {
            let mapped = match pairs.get(id) {
                Some(id) => *id,
                None => {
                    let mapped = next;
                    next = next.checked_add(1).ok_or("本地宏事件标识已耗尽")?;
                    pairs.insert(*id, mapped);
                    mapped
                }
            };
            *id = mapped;
        }
    }
    Ok(())
}
