use super::*;
use anyhow::{Context as _, ensure};
use razer_device::{
    keyboard_current_profile::LightingOff, keyboard_settings::BrightnessSubmission,
};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_pages::features::keyboard_products::KeyboardLightingRequested;
use razer_storage::keyboard_lighting::{self, Change};

#[derive(Clone)]
enum Request {
    Brightness {
        generation: u64,
        submission: BrightnessSubmission,
    },
    SwitchOff(KeyboardLightingRequested),
}

pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile: String,
    connection_revision: u64,
    route: ObservedDevice,
    request: Request,
    change: Change,
}

impl LocalSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        if !shell.devices.contains(&self.workspace)
            || self.workspace.read(cx).identity(cx) != self.identity
            || self.workspace.read(cx).connection_revision() != self.connection_revision
            || self.workspace.read(cx).device(cx).active_profile != self.profile
            || !device_discovery::retained_connection(
                std::slice::from_ref(&self.route),
                &shell.device_observations,
                self.workspace.read(cx).device(cx),
            )
        {
            return false;
        }
        match &self.request {
            Request::Brightness {
                generation,
                submission,
            } => self.workspace.read(cx).keyboard_brightness_request_current(
                *generation,
                submission.effective_percent(),
                cx,
            ),
            Request::SwitchOff(request) => self
                .workspace
                .read(cx)
                .accepts_keyboard_lighting(request, cx),
        }
    }
}

impl AppShell {
    pub(super) fn persist_keyboard_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        generation: u64,
        submission: BrightnessSubmission,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace.read(cx).keyboard_brightness_request_current(
            generation,
            submission.effective_percent(),
            cx,
        ) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness(
                    generation,
                    submission.effective_percent(),
                    None,
                    Some("亮度请求的配置或页面已变化".into()),
                    false,
                    cx,
                )
            });
            return;
        }
        let saved = workspace.read(cx).saved_snapshot(cx);
        // An explicit manual write remains available before a local profile
        // library is established. No default profile or persistence is claimed.
        if saved.profiles.is_empty()
            && saved.active_profile.is_empty()
            && workspace.read(cx).device(cx).profiles.is_empty()
            && workspace.read(cx).device(cx).active_profile.is_empty()
        {
            self.refresh_macro_recorder_storage();
            if self.receiver_macro_exclusive(cx) {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_local_keyboard_brightness(
                        generation,
                        submission.effective_percent(),
                        cx,
                    )
                });
                self.status = "设备由宏录制器控制；亮度草稿尚未写入设备或本地配置。".into();
            } else {
                self.submit_keyboard_brightness(
                    workspace,
                    generation,
                    submission,
                    window.window_handle(),
                    cx,
                );
            }
            return;
        }
        self.queue_keyboard_lighting_save(
            workspace,
            Request::Brightness {
                generation,
                submission,
            },
            Change::Brightness(submission),
            window,
            cx,
        );
    }

    pub(super) fn persist_keyboard_switch_off(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: KeyboardLightingRequested,
        setting: LightingOff,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.queue_keyboard_lighting_save(
            workspace,
            Request::SwitchOff(request),
            Change::SwitchOff(setting),
            window,
            cx,
        );
    }

    fn queue_keyboard_lighting_save(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: Request,
        change: Change,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prepared = (|| -> anyhow::Result<LocalSave> {
            ensure!(!self.close_requested, "键盘灯光请求已随关闭取消");
            ensure!(self.storage_error.is_none(), "本地存储尚不可用");
            let device = workspace.read(cx).device(cx);
            let saved = workspace.read(cx).saved_snapshot(cx);
            let mut candidate = saved.clone();
            keyboard_lighting::merge(&mut candidate, &device.active_profile, &change)?;
            let routes = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(device) && direct::keyboard_brightness_available(route)
                })
                .collect::<Vec<_>>();
            let [route] = routes.as_slice() else {
                anyhow::bail!("没有唯一的在线键盘亮度连接");
            };
            if matches!(&request, Request::SwitchOff(_)) {
                ensure!(
                    razer_device::keyboard_settings::power::supports_transport(
                        route.product_id(),
                        route.transport()
                            == Some(razer_discovery::discovery::ObservedTransport::Ble)
                    ),
                    "此传输未挂载键盘自动熄灯状态机"
                );
            }
            Ok(LocalSave {
                identity: workspace.read(cx).identity(cx),
                profile: device.active_profile.clone(),
                connection_revision: workspace.read(cx).connection_revision(),
                route: (**route).clone(),
                workspace: workspace.clone(),
                request: request.clone(),
                change: change.clone(),
            })
        })();
        let save = match prepared {
            Ok(save) => save,
            Err(error) => {
                self.status = format!("键盘灯光设置未保存：{error:#}");
                let error = self.status.clone();
                match request {
                    Request::Brightness {
                        generation,
                        submission,
                    } => workspace.update(cx, |workspace, cx| {
                        workspace.finish_keyboard_brightness(
                            generation,
                            submission.effective_percent(),
                            None,
                            Some(error),
                            true,
                            cx,
                        )
                    }),
                    Request::SwitchOff(request) => {
                        workspace.update(cx, |workspace, cx| {
                            workspace.complete_keyboard_lighting(&request, Err(error), window, cx)
                        });
                    }
                }
                return;
            }
        };
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|device| device.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::KeyboardLighting(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(in crate::shell) fn rebase_keyboard_lighting_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::KeyboardLighting(lighting)) = &save.product else {
            return Ok(());
        };
        ensure!(lighting.current(self, cx), "键盘灯光配置或连接已变化");
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).saved_snapshot(cx))
            .collect();
        let index = self
            .devices
            .iter()
            .position(|workspace| workspace.read(cx).identity(cx) == lighting.identity)
            .context("键盘灯光设备已移除")?;
        keyboard_lighting::merge(
            save.file
                .devices
                .get_mut(index)
                .context("键盘灯光快照不存在")?,
            &lighting.profile,
            &lighting.change,
        )?;
        Ok(())
    }

    pub(in crate::shell) fn finish_keyboard_lighting_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let current = save.current(self, cx);
        if !succeeded || !current {
            let error = if succeeded {
                "键盘配置或连接已变化；未继续写入设备"
            } else {
                "键盘灯光本地保存失败；未继续写入设备"
            }
            .to_owned();
            self.finish_keyboard_lighting_request(save, Err(error), window, cx);
            return;
        }
        match (&save.request, &save.change) {
            (
                Request::Brightness {
                    generation,
                    submission,
                },
                Change::Brightness(_),
            ) => {
                let (generation, submission) = (*generation, *submission);
                match self.keyboard_power_submitted(&save.workspace, submission, cx) {
                    Ok(true) => {
                        self.submit_keyboard_brightness(
                            save.workspace.clone(),
                            generation,
                            submission,
                            window,
                            cx,
                        );
                    }
                    Ok(false) => {
                        save.workspace.update(cx, |workspace, cx| {
                            workspace.finish_local_keyboard_brightness(
                                generation,
                                submission.effective_percent(),
                                cx,
                            )
                        });
                        self.status = "亮度已保存到本地配置；设备由宏录制器控制。".into();
                    }
                    Err(error) => self.finish_keyboard_lighting_request(
                        save,
                        Err(format!("{error:#}")),
                        window,
                        cx,
                    ),
                }
            }
            (Request::SwitchOff(_), Change::SwitchOff(setting)) => {
                let result = self
                    .keyboard_switch_off_committed(&save.workspace, setting, cx)
                    .map_err(|error| format!("{error:#}"));
                self.finish_keyboard_lighting_request(save, result, window, cx);
            }
            _ => unreachable!("Lighting save owns its matching typed request"),
        }
    }

    fn finish_keyboard_lighting_request(
        &mut self,
        save: LocalSave,
        result: Result<(), String>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        match save.request {
            Request::Brightness {
                generation,
                submission,
            } => save.workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness(
                    generation,
                    submission.effective_percent(),
                    None,
                    result.err(),
                    false,
                    cx,
                )
            }),
            Request::SwitchOff(request) => {
                let error = result.as_ref().err().cloned();
                if window
                    .update(cx, |_, window, cx| {
                        save.workspace.update(cx, |workspace, cx| {
                            workspace.complete_keyboard_lighting(
                                &request,
                                result.map(|()| None),
                                window,
                                cx,
                            )
                        })
                    })
                    .is_err()
                {
                    save.workspace.update(cx, |workspace, cx| {
                        workspace.abandon_keyboard_lighting(&request, error, cx)
                    });
                }
            }
        }
        cx.notify();
    }
}
