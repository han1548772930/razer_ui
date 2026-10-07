//! Source xa/Gr countdown and recorder state, backed by DLL recorder events.
use super::recording_actor::{Command, Update};
use super::*;
use std::sync::mpsc;

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    #[default]
    Idle,
    Countdown(u8),
    Starting,
    Recording,
    Stopping,
}
#[derive(Default)]
pub(super) struct RecordingUi {
    pub(super) stage: Stage,
    pub(super) error: Option<String>,
    pub(super) status: String,
    pub(super) count: usize,
    pub(super) preview: Vec<ActionItem>,
    pub(super) preview_offset: usize,
    pub(super) scroll: ScrollHandle,
    generation: u64,
    command: Option<mpsc::Sender<Command>>,
    document: Option<u64>,
}
impl MacroPage {
    pub(in crate::shell) fn recording_busy(&self) -> bool {
        self.recording.stage != Stage::Idle
    }
    pub(super) fn recording_controls_active(&self) -> bool {
        matches!(
            self.recording.stage,
            Stage::Starting | Stage::Recording | Stage::Stopping
        )
    }
    pub(super) fn record_countdown_label(&self) -> Option<String> {
        let Stage::Countdown(n) = self.recording.stage else {
            return None;
        };
        Some(tr("RECORDING_IN").replace("{{time}}", &n.to_string()))
    }
    pub(super) fn record_label(&self) -> String {
        match self.recording.stage {
            Stage::Idle => tr("TEXT_ACTION_BAR_RECORD").to_uppercase(),
            Stage::Countdown(_) => tr("TEXT_ACTION_BAR_RECORD").to_uppercase(),
            Stage::Starting => "正在开始…".into(),
            Stage::Recording => tr("TEXT_ACTION_BAR_STOP").to_uppercase(),
            Stage::Stopping => "正在停止…".into(),
        }
    }
    pub(super) fn toggle_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.recording.stage {
            Stage::Countdown(_) => {
                self.cancel_recording(cx);
                return;
            }
            Stage::Starting | Stage::Recording => {
                if let Some(sender) = &self.recording.command {
                    let _ = sender.send(Command::Stop);
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
        self.recording.preview_offset = 0;
        self.recording.document = self.current;
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
        cx.spawn(async move |this, cx| {
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
        })
        .detach();
    }
    fn start_recording(&mut self, cx: &mut Context<Self>) {
        let options = match self.recording_options(cx) {
            Ok(options) => options,
            Err(error) => {
                self.recording.stage = Stage::Idle;
                self.recording.error = Some(error);
                cx.notify();
                return;
            }
        };
        let (sender, receiver) = match recording_actor::spawn(options) {
            Ok(session) => session,
            Err(error) => {
                self.recording.stage = Stage::Idle;
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
        cx.spawn(async move |this, cx| {
            while let Ok(update) = receiver.recv().await {
                let finished = matches!(&update, Update::Finished(_));
                if this
                    .update(cx, |this, cx| {
                        if this.recording.generation != generation {
                            return;
                        }
                        match update {
                            Update::Recording => {
                                if this.recording.stage != Stage::Stopping {
                                    this.recording.stage = Stage::Recording;
                                    this.recording.status = "正在全局录制；按 Esc 取消".into();
                                }
                            }
                            Update::Progress(count) => this.recording.count = count,
                            Update::Preview { first_row, rows } => {
                                let baseline = this.actions.len();
                                this.recording.preview.truncate(baseline);
                                this.recording.preview_offset = first_row;
                                this.recording.preview.extend(rows);
                                // le.createMacroItemUI scrolls after appending,
                                // including when the saved rows are selected.
                                this.recording.scroll.scroll_to_bottom();
                            }
                            Update::Finished(result) => {
                                this.recording.command = None;
                                this.recording.stage = Stage::Idle;
                                this.recording.preview.clear();
                                match result {
                                    Ok(Some(actions))
                                        if this.recording.document == this.current
                                            && this.actions_for == this.current =>
                                    {
                                        let count = actions.len();
                                        if count > 0 {
                                            this.undo.push(this.actions.clone());
                                            this.actions.extend(actions);
                                            this.redo.clear();
                                            this.selected_actions.clear();
                                            if this.current_macro_type()
                                                == crate::features::macro_library::MacroType::Phased
                                            {
                                                this.normalize_phased_rows();
                                            }
                                        }
                                        this.recording.status = format!(
                                            "录制完成，新增 {count} 行本地草稿；保存仅写入本机"
                                        );
                                    }
                                    Ok(None) => {
                                        this.recording.status = "录制已取消，未更改草稿".into()
                                    }
                                    Ok(Some(_)) => {
                                        this.recording.error =
                                            Some("录制期间文档已变化，未提交结果".into())
                                    }
                                    Err(error) => {
                                        this.recording.status = "录制失败，未更改草稿".into();
                                        this.recording.error = Some(error);
                                    }
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                    || finished
                {
                    break;
                }
            }
        })
        .detach();
    }
    /// Called by shell close/navigation. Actor shutdown stays on its thread.
    pub(in crate::shell) fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(sender) = &self.recording.command {
            let _ = sender.send(Command::Cancel);
            self.recording.stage = Stage::Stopping;
            self.recording.status = "正在取消原生录制…".into();
        } else {
            self.recording.generation = self.recording.generation.wrapping_add(1);
            self.recording.stage = Stage::Idle;
            self.recording.status = "录制已取消，未更改草稿".into();
        }
        cx.notify();
    }
    pub(super) fn recording_status(&self) -> AnyElement {
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
            .into_any_element()
    }
}
