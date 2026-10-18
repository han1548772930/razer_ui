//! Local profile transfer dialogs; codecs and disk I/O have lower-layer owners.
use super::*;
use razer_model::profile_transfer::{encode_profile, import_name, valid_name};
use razer_storage::file_transfer::{Operation, Transfer};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

/// The modal owns cancellation independently of the content entity borrow.
/// Closing from a content callback never reenters that borrowed entity.
#[derive(Clone, Default)]
pub(super) struct DialogTransfer(Rc<RefCell<Option<(Transfer, bool)>>>);
impl DialogTransfer {
    fn begin(&self) -> Result<(Transfer, Operation), String> {
        self.cancel();
        let request = Transfer::new();
        let operation = request.accept()?;
        *self.0.borrow_mut() = Some((request.clone(), true));
        Ok((request, operation))
    }
    fn is_current(&self, request: &Transfer) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|(current, _)| current.is_same(request))
    }
    fn accepts_result(&self, request: &Transfer) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|(current, valid)| *valid && current.is_same(request))
    }
    fn finish(&self, request: &Transfer) {
        if self.is_current(request) {
            self.0.borrow_mut().take();
        }
    }
    pub(super) fn cancel(&self) {
        if let Some((request, valid)) = self.0.borrow_mut().as_mut() {
            *valid = false;
            request.cancel();
        }
    }
}

struct ImportDialog {
    target: ProfileTarget,
    pid: u32,
    layout: u32,
    loaded: Option<Profile>,
    name: Entity<InputState>,
    file: Option<String>,
    error: Option<String>,
    busy: bool,
    transfer: DialogTransfer,
    task: Option<Task<()>>,
}
impl ImportDialog {
    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (request, operation) = match self.transfer.begin() {
            Ok(work) => work,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.busy = true;
        self.error = None;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择本地配置 JSON".into()),
        });
        let (pid, layout) = (self.pid, self.layout);
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let result = match picker.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => {
                        let label = path.to_string_lossy().into_owned();
                        Some((
                            label,
                            cx.background_executor()
                                .spawn(async move {
                                    razer_storage::profile_transfer::read_local(
                                        &operation, &path, pid, layout,
                                    )
                                })
                                .await,
                        ))
                    }
                    None => None,
                },
                Ok(Ok(None)) => None,
                _ => Some((String::new(), Err("无法打开文件选择窗口。".into()))),
            };
            _ = view.update_in(cx, |this, window, cx| {
                if !this.transfer.is_current(&request) {
                    return;
                }
                this.transfer.finish(&request);
                this.task = None;
                this.busy = false;
                let Some(workspace) = this.target.current(cx) else {
                    this.loaded = None;
                    this.error = Some("当前配置文件已切换，请重新打开导入。".into());
                    cx.notify();
                    return;
                };
                if let Some((file, result)) = result {
                    this.file = Some(file);
                    match request.check().and(result) {
                        Ok(profile) => {
                            let name =
                                import_name(&workspace.read(cx).device.profiles, &profile.name);
                            this.name
                                .update(cx, |input, cx| input.set_value(name, window, cx));
                            this.loaded = Some(profile);
                            this.error = None;
                        }
                        Err(error) => {
                            this.loaded = None;
                            this.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(mut profile) = self.loaded.clone() else {
            return;
        };
        let Some(workspace) = self.target.current(cx) else {
            self.error = Some("当前配置文件已切换，请重新打开导入。".into());
            cx.notify();
            return;
        };
        let name = self.name.read(cx).value().trim().to_string();
        if !valid_name(&name)
            || workspace
                .read(cx)
                .device
                .profiles
                .iter()
                .any(|profile| profile.name.eq_ignore_ascii_case(&name))
        {
            self.error = Some("请输入 1–32 个字符且未被使用的配置名称。".into());
            cx.notify();
            return;
        }
        profile.name = name;
        dismiss_profile_dialog(&self.target.workspace, window, cx);
        workspace.update(cx, |workspace, cx| {
            workspace.continue_with(Continue::ImportProfile(profile), window, cx)
        });
    }
}
impl Drop for ImportDialog {
    fn drop(&mut self) {
        self.transfer.cancel();
    }
}
impl Render for ImportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = v_flex()
            .id("profile-import-content")
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .child(surface::note(
                "仅支持本项目导出的 .razer-ui-profile.json。导入会新增本地配置，不覆盖已有配置。",
                cx,
            ))
            .child(
                profile_dialog_button(
                    "profile-import-browse",
                    if self.busy {
                        "正在读取…"
                    } else {
                        "选择文件…"
                    },
                    ThxKind::Primary,
                    window,
                    cx,
                )
                .disabled(self.busy)
                .on_click(cx.listener(|this, _, window, cx| this.browse(window, cx))),
            )
            .when_some(self.file.clone(), |body, file| {
                body.child(div().text_xs().child(file))
            })
            .when(self.loaded.is_some(), |body| {
                body.child(div().child("新配置名称"))
                    .child(
                        Input::new(&self.name)
                            .id("profile-import-name")
                            .aria_label("新配置名称")
                            .h(surface::css(27.)),
                    )
                    .child(surface::note(
                        "同名配置已自动添加编号；导入后仍需保存设备配置。",
                        cx,
                    ))
            })
            .when_some(self.error.clone(), |body, error| {
                body.child(
                    div()
                        .id("profile-import-error")
                        .test_support()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            });
        v_flex()
            .id("profile-import-dialog")
            .test_support()
            .size_full()
            .min_h_0()
            .child(content)
            .child(
                profile_dialog_footer(cx)
                    .child(
                        profile_dialog_button(
                            "profile-import-cancel",
                            razer_i18n::t("CANCEL"),
                            ThxKind::Test,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            dismiss_profile_dialog(&this.target.workspace, window, cx)
                        })),
                    )
                    .child(
                        profile_dialog_button(
                            "profile-import-confirm",
                            "导入到本地草稿",
                            ThxKind::Primary,
                            window,
                            cx,
                        )
                        .disabled(self.loaded.is_none() || self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.import(window, cx))),
                    ),
            )
    }
}

struct ExportDialog {
    target: ProfileTarget,
    bytes: Vec<u8>,
    busy: bool,
    result: Option<Result<String, String>>,
    transfer: DialogTransfer,
    task: Option<Task<()>>,
}
impl ExportDialog {
    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (request, operation) = match self.transfer.begin() {
            Ok(work) => work,
            Err(error) => {
                self.result = Some(Err(error));
                cx.notify();
                return;
            }
        };
        self.busy = true;
        self.result = None;
        let directory = PathBuf::from(".");
        let picker = cx.prompt_for_new_path(&directory, Some("profile.razer-ui-profile.json"));
        self.task = Some(cx.spawn_in(window, async move |view, cx| {
            let path = match picker.await {
                Ok(Ok(path)) => Ok(path),
                _ => Err("无法打开保存文件窗口。".to_string()),
            };
            let prepared = view.update_in(cx, |this, _, cx| {
                if !this.transfer.is_current(&request) || this.target.current(cx).is_none() {
                    Err("当前配置文件已切换，请重新打开导出。".to_string())
                } else {
                    path.map(|path| path.map(|path| (path, this.bytes.clone())))
                }
            });
            let result = match prepared {
                Ok(Ok(Some((path, bytes)))) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            razer_storage::profile_transfer::write_local(&operation, &path, &bytes)
                                .map(|_| path.to_string_lossy().into_owned())
                        })
                        .await,
                ),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => Some(Err(error)),
                Err(_) => return,
            };
            _ = view.update_in(cx, |this, _, cx| {
                if !this.transfer.is_current(&request) {
                    return;
                }
                let accepts_result =
                    this.transfer.accepts_result(&request) && this.target.current(cx).is_some();
                this.transfer.finish(&request);
                this.task = None;
                this.busy = false;
                this.result = if accepts_result {
                    result
                } else {
                    Some(Err("当前配置文件已切换，请重新打开导出。".into()))
                };
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Drop for ExportDialog {
    fn drop(&mut self) {
        self.transfer.cancel();
    }
}
impl Render for ExportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex()
            .id("profile-export-content").w_full().flex_1().min_h_0().overflow_y_scroll()
            .p(surface::css(20.)).gap(surface::css(10.))
            .child(surface::note("导出当前本地配置快照，包含按键映射和关联程序路径。此 JSON 供本项目导入，不是 Synapse 配置包。", cx));
        if let Some(result) = &self.result {
            body = body.child(
                div()
                    .id("profile-export-result")
                    .test_support()
                    .text_color(if result.is_ok() {
                        cx.theme().foreground
                    } else {
                        cx.theme().danger
                    })
                    .child(match result {
                        Ok(path) => format!("已导出：{path}"),
                        Err(error) => error.clone(),
                    }),
            );
        }
        v_flex()
            .id("profile-export-dialog")
            .test_support()
            .size_full()
            .min_h_0()
            .child(body)
            .child(
                profile_dialog_footer(cx)
                    // 源码在非导入（即导出）模式的底栏左侧渲染 `.willNotImport`：
                    // `IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT`
                    // = "Linked Games and Chroma Effects will not be exported."
                    .child(
                        div()
                            .id("profile-export-note")
                            .test_support()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(16.))
                            .text_color(cx.theme().muted_foreground)
                            .child(razer_i18n::t_or(
                                "IMPORT_EXPORT_MODAL_WILL_NOT_IMPORT",
                                "不会导出已关联的游戏和 Chroma 幻彩效果.",
                            )),
                    )
                    .child(
                        profile_dialog_button(
                            "profile-export-close",
                            razer_i18n::t("CLOSE"),
                            ThxKind::Test,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            dismiss_profile_dialog(&this.target.workspace, window, cx)
                        })),
                    )
                    .child(
                        profile_dialog_button(
                            "profile-export-save",
                            if self.busy {
                                "正在导出…"
                            } else {
                                "选择导出位置…"
                            },
                            ThxKind::Primary,
                            window,
                            cx,
                        )
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
                    ),
            )
    }
}

impl DeviceWorkspace {
    pub(super) fn open_profile_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = ProfileTarget::new(self, cx);
        let (pid, layout) = (self.device.product_id, self.device.layout_id);
        let name = cx.new(|cx| InputState::new(window, cx));
        let transfer = DialogTransfer::default();
        let view = cx.new(|_| ImportDialog {
            target,
            pid,
            layout,
            loaded: None,
            name,
            file: None,
            error: None,
            busy: false,
            transfer: transfer.clone(),
            task: None,
        });
        self.show_profile_dialog(
            "导入本地配置".to_string(),
            view.into(),
            Some(transfer),
            window,
            cx,
        );
    }
    pub(in crate::features::workspace) fn import_local_profile(
        &mut self,
        mut profile: Profile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        profile.name = import_name(&self.device.profiles, &profile.name);
        let id = next_profile_id(&self.device, &self.saved);
        profile.id = id.clone();
        profile.guid = id.clone();
        self.device.profiles.push(profile);
        self.device.active_profile = id;
        self.refresh_profile_choices(window, cx);
    }
    pub(in crate::features::workspace) fn export_local_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.device.active_profile_obj() else {
            return;
        };
        let bytes = match encode_profile(&self.device, profile) {
            Ok(bytes) => bytes,
            Err(error) => {
                window.push_notification(error, cx);
                return;
            }
        };
        let name = profile.name.clone();
        let target = ProfileTarget::new(self, cx);
        let transfer = DialogTransfer::default();
        let view = cx.new(|_| ExportDialog {
            target,
            bytes,
            busy: false,
            result: None,
            transfer: transfer.clone(),
            task: None,
        });
        self.show_profile_dialog(
            format!("导出本地配置 — {name}"),
            view.into(),
            Some(transfer),
            window,
            cx,
        );
    }
}

#[cfg(test)]
#[path = "profile_transfer_tests.rs"]
mod tests;
