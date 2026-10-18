//! Mn import/export -> 25572.FM, kept local; no DLL mutation or playback.
use super::*;
use razer_model::macro_library::xml;
use razer_storage::macro_transfer::{Operation, Transfer};
use std::path::Path;

pub(super) struct PendingTransfer {
    request: Transfer,
    _task: Task<()>,
}

impl Drop for PendingTransfer {
    fn drop(&mut self) {
        self.request.cancel();
    }
}

fn read(request: &Operation, path: &Path) -> Result<Entry, String> {
    let mut entry = request.read_xml(path)?;
    if entry
        .actions
        .iter()
        .any(|item| item.mouse_movement.is_some())
    {
        let observed = razer_platform::macro_geometry::snapshot().map_err(|error| {
            format!("Imported mouse movement monitor observation failed: {error:#}")
        })?;
        for item in &mut entry.actions {
            if let Some(movement) = &mut item.mouse_movement {
                movement.geometry = Some(razer_model::macro_library::RecordingGeometry {
                    monitors_before: observed.monitors.clone(),
                    monitors_after: observed.monitors.clone(),
                    screen_before: observed.screen.clone(),
                    screen_after: observed.screen.clone(),
                });
            }
        }
    }
    Ok(entry)
}

/// 25572.N with the hc operation is also the current import naming rule.
fn import_name(entries: &[Entry], original: &str) -> String {
    let mut base = original.to_owned();
    for index in 0..100 {
        let suffix = format!(" ({index})");
        if base.contains(&suffix) {
            base = base.replacen(&suffix, "", 1).trim().to_owned();
            break;
        }
    }
    let matching = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Macro && entry.name.contains(&base))
        .collect::<Vec<_>>();
    let mut suffix = " (1)".to_owned();
    for (index, _) in matching.iter().enumerate() {
        for item in &matching {
            if item.name.contains(&suffix) {
                suffix = format!(" ({})", index + 2);
            }
        }
    }
    if matching.is_empty() {
        base
    } else {
        format!("{base}{suffix}")
    }
}

impl MacroPage {
    pub fn cancel_transfer(&mut self, cx: &mut Context<Self>) {
        self.transfer_task = None;
        self.transfer_busy = false;
        cx.notify();
    }

    pub fn import_xml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.transfer_busy || self.recording_busy() {
            return;
        }
        self.finish_pending_edits(window, cx);
        self.more_open = false;
        self.transfer_busy = true;
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(format!("{} (.xml)", tr("TEXT_PROFILE_BAR_S3_DROPDOWN_IMPORT")).into()),
        });
        let request = Transfer::new();
        let current = request.clone();
        let operation = match request.accept() {
            Ok(operation) => operation,
            Err(error) => {
                self.transfer_busy = false;
                window.push_notification(error, cx);
                return;
            }
        };
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => Some(
                        cx.background_executor()
                            .spawn(async move { read(&operation, &path) })
                            .await,
                    ),
                    None => None,
                },
                Ok(Ok(None)) => None,
                _ => Some(Err("无法打开宏 XML 文件选择器。".into())),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if !this
                    .transfer_task
                    .as_ref()
                    .is_some_and(|pending| pending.request.is_same(&current))
                {
                    return;
                }
                this.transfer_busy = false;
                this.transfer_task = None;
                if let Some(result) = result {
                    match result.and_then(|entry| this.accept_xml(entry, cx)) {
                        Ok(name) => window.push_notification(
                            format!("已导入本地宏“{name}”。本地保存状态见窗口底部。"),
                            cx,
                        ),
                        Err(error) => window.push_notification(error, cx),
                    }
                }
                cx.notify();
            });
        });
        self.transfer_task = Some(PendingTransfer {
            request,
            _task: task,
        });
        cx.notify();
    }

    fn accept_xml(&mut self, mut entry: Entry, cx: &mut Context<Self>) -> Result<String, String> {
        if self.recording_busy() {
            return Err("录制期间无法导入；请停止录制后重试。".into());
        }
        if self.next_id >= u64::MAX - 1 {
            return Err("本地宏标识已耗尽，未导入文件。".into());
        }
        entry.id = self.next_id;
        entry.name = import_name(&self.entries, &entry.name);
        entry.xml_guid = Some(uuid::Uuid::new_v4().to_string());
        for action in &mut entry.actions {
            if let Some(guid) = &action.xml_macro_guid {
                action.macro_id = self
                    .entries
                    .iter()
                    .find(|e| e.kind == EntryKind::Macro && e.xml_guid.as_ref() == Some(guid))
                    .map(|e| e.id);
            }
        }
        let was_empty = self.macro_count() == 0;
        let mut candidate = razer_pages::features::macro_library::MacroLibraryFile {
            next_id: self.next_id + 1,
            entries: self.entries.clone(),
            current: if was_empty { Some(entry.id) } else { self.current },
            tutorial: Tutorial::Complete,
            record_shortcut: self.record_ui.shortcut.selected.clone(),
        };
        candidate.entries.push(entry.clone());
        candidate.validate()?;
        self.next_id = candidate.next_id;
        self.entries.push(entry.clone());
        if was_empty {
            self.current = Some(entry.id);
            self.tree_selection = self.current;
            self.load_current_actions();
        }
        self.tutorial = Tutorial::Complete;
        self.publish_library(cx);
        Ok(entry.name)
    }

    pub fn export_xml(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.transfer_busy || self.recording_busy() {
            return;
        }
        self.finish_pending_edits(window, cx);
        self.more_open = false;
        // Old local documents predate interchange IDs. Assign once and persist
        // metadata through the existing local store, never derive a device GUID.
        for entry in &mut self.entries {
            if entry.kind == EntryKind::Macro && entry.xml_guid.is_none() {
                entry.xml_guid = Some(uuid::Uuid::new_v4().to_string());
            }
        }
        self.publish_library(cx);
        let Some(mut entry) = self
            .entries
            .iter()
            .find(|e| Some(e.id) == self.current)
            .cloned()
        else {
            return;
        };
        entry.actions = self.actions.clone(); // Mn exports the live current macro.
        let text = match xml::encode(&entry, &self.entries) {
            Ok(text) => text,
            Err(error) => {
                window.push_notification(error, cx);
                cx.notify();
                return;
            }
        };
        let name = entry
            .name
            .chars()
            .map(|c| {
                if c.is_control() || "<>:\"/\\|?*".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect::<String>();
        let directory = std::path::PathBuf::from(".");
        let prompt = cx.prompt_for_new_path(&directory, Some(&format!("{name}.xml")));
        self.transfer_busy = true;
        let request = Transfer::new();
        let current = request.clone();
        let operation = match request.accept() {
            Ok(operation) => operation,
            Err(error) => {
                self.transfer_busy = false;
                window.push_notification(error, cx);
                return;
            }
        };
        let task = cx.spawn_in(window, async move |this, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(mut path))) => {
                    if path.extension().is_none() {
                        path.set_extension("xml");
                    }
                    if !path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
                    {
                        Some(Err("请使用 .xml 扩展名保存宏文件。".into()))
                    } else {
                        Some(
                            cx.background_executor()
                                .spawn(async move { operation.write_xml(&path, &text) })
                                .await,
                        )
                    }
                }
                Ok(Ok(None)) => None,
                _ => Some(Err("无法打开宏 XML 保存窗口。".into())),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if !this
                    .transfer_task
                    .as_ref()
                    .is_some_and(|pending| pending.request.is_same(&current))
                {
                    return;
                }
                this.transfer_busy = false;
                this.transfer_task = None;
                if let Some(result) = result {
                    window.push_notification(
                        match result {
                            Ok(()) => "已导出本地宏 XML。".to_owned(),
                            Err(error) => error,
                        },
                        cx,
                    );
                }
                cx.notify();
            });
        });
        self.transfer_task = Some(PendingTransfer {
            request,
            _task: task,
        });
        cx.notify();
    }
}
