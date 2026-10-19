use super::*;
use razer_discovery::discovery::ObservedTransport;
use razer_model::model::Profile;
use razer_pages::features::SeirenEqRequested;

pub(super) enum SeirenEqSave {
    Settings(SeirenEqSettingsSave),
    MixerVersion(SeirenMixerVersionSave),
}
pub(super) struct SeirenEqSettingsSave {
    workspace: Entity<ProductWorkspace>,
    request: SeirenEqRequested,
    identity: String,
    profile: Profile,
}
pub(super) struct SeirenMixerVersionSave {
    workspace: Entity<ProductWorkspace>,
    request: razer_pages::features::SeirenStreamMixerRequested,
    identity: String,
    version: Option<u64>,
    reply: std::sync::mpsc::Sender<Result<u64, String>>,
}
impl SeirenEqSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        match self {
            Self::Settings(save) => (&save.identity, save.workspace.downgrade()),
            Self::MixerVersion(save) => (&save.identity, save.workspace.downgrade()),
        }
    }
}
impl AppShell {
    pub(super) fn persist_seiren_device_version(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: razer_pages::features::SeirenStreamMixerRequested,
        reply: std::sync::mpsc::Sender<Result<u64, String>>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || self.storage_error.is_some()
            || !workspace
                .read(cx)
                .seiren_stream_mixer_request_matches(&request, cx)
        {
            let _ = reply.send(Err(self.storage_error.clone().unwrap_or_else(|| {
                "Mixer device-document save owner is no longer current".into()
            })));
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        let identity = workspace.read(cx).identity(cx);
        self.pending_saves.push_back(self.prepare_save(
            window,
            file,
            Some(ProductSave::SeirenEq(SeirenEqSave::MixerVersion(
                SeirenMixerVersionSave {
                    workspace,
                    request,
                    identity,
                    version: None,
                    reply,
                },
            ))),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn persist_seiren_eq(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: SeirenEqRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !request.persists_local_settings() {
            self.submit_seiren_eq_device(workspace, request, window.window_handle(), cx);
            return;
        }
        if self.storage_error.is_some()
            || !workspace.read(cx).seiren_eq_request_matches(&request, cx)
        {
            if let Some(error) = &self.storage_error {
                self.status = format!("麦克风 EQ 本地保存失败：{error}");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_eq(&request, None, cx)
            });
            if self.storage_error.is_some() {
                self.cancel_pending_exit(cx);
            }
            cx.notify();
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        let Some(profile) = device
            .profiles
            .iter()
            .find(|profile| profile.id == device.active_profile)
            .cloned()
        else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_eq(&request, None, cx)
            });
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::SeirenEq(SeirenEqSave::Settings(
                SeirenEqSettingsSave {
                    workspace,
                    request,
                    identity,
                    profile,
                },
            ))),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_seiren_eq_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::SeirenEq(operation)) = &mut save.product else {
            return Ok(());
        };
        let eq = match operation {
            SeirenEqSave::Settings(eq) => eq,
            SeirenEqSave::MixerVersion(version) => {
                anyhow::ensure!(
                    version.request.is_valid()
                        && self.devices.contains(&version.workspace)
                        && version.workspace.read(cx).identity(cx) == version.identity,
                    "Mixer device-document save owner changed"
                );
                let mut found = false;
                save.file.devices = self
                    .devices
                    .iter()
                    .map(|entity| {
                        let mut device = entity.read(cx).saved_snapshot(cx);
                        if entity.read(cx).identity(cx) == version.identity {
                            found = true;
                            let settings = device
                                .source_device_settings
                                .get_or_insert_with(|| serde_json::json!({}));
                            anyhow::ensure!(
                                settings.is_object(),
                                "Seiren device settings is not an object"
                            );
                            let next = razer_device::seiren_stream_mixer::next_device_version(
                                settings.get("version"),
                            )?;
                            settings["version"] = serde_json::json!(next);
                            version.version = Some(next);
                        }
                        Ok(device)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                anyhow::ensure!(found, "Seiren device-document saved owner is absent");
                return Ok(());
            }
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == eq.identity {
                    if matches!(
                        eq.request.change(),
                        Some(razer_device::seiren_eq::Change::SamplingRate(_))
                    ) {
                        if let Some(rate) = eq.request.settings().pointer("/profile/samplingRate") {
                            saved
                                .source_device_settings
                                .get_or_insert_with(|| serde_json::json!({}))["samplingRate"] =
                                rate.clone();
                        }
                        return saved;
                    }
                    if matches!(
                        eq.request.change(),
                        Some(razer_device::seiren_eq::Change::MonitorVolume(_))
                    ) {
                        if let Some(monitor) = eq.request.settings().pointer("/device/micSideTone")
                        {
                            saved
                                .source_device_settings
                                .get_or_insert_with(|| serde_json::json!({}))["micSideTone"] =
                                monitor.clone();
                        }
                        return saved;
                    }
                    if let Some(profile) = saved
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == eq.profile.id)
                    {
                        let target = profile
                            .source_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        let requested = eq.request.change();
                        if matches!(
                            requested,
                            Some(
                                razer_device::seiren_eq::Change::DialControl(_)
                                    | razer_device::seiren_eq::Change::RestoreDialControl(_)
                            )
                        ) {
                            if let Some(dial) =
                                eq.request.settings().pointer("/profile/dialControl")
                            {
                                if target.get("profile").is_none() {
                                    target["profile"] = serde_json::json!({});
                                }
                                target["profile"]["dialControl"] = dial.clone();
                            }
                            return saved;
                        }
                        let fields: &[&str] = match requested {
                            Some(razer_device::seiren_eq::Change::MicAgc(_)) => &["micAgc"],
                            Some(razer_device::seiren_eq::Change::MonitorVolume(_)) => {
                                &["micSideTone"]
                            }
                            Some(
                                razer_device::seiren_eq::Change::VocalBass(_)
                                | razer_device::seiren_eq::Change::VocalExciter(_)
                                | razer_device::seiren_eq::Change::DeEsser(_),
                            ) => &["eqEffects"],
                            _ => &["parametric", "eqEffects"],
                        };
                        for &field in fields {
                            if let Some(value) =
                                eq.request.settings().pointer(&format!("/device/{field}"))
                            {
                                if target.get("device").is_none() {
                                    target["device"] = serde_json::json!({});
                                }
                                target["device"][field] = value.clone();
                            }
                        }
                    } else {
                        saved.profiles.push(eq.profile.clone());
                    }
                }
                saved
            })
            .collect();
        Ok(())
    }
    pub(super) fn finish_seiren_eq_save(
        &mut self,
        save: SeirenEqSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let save = match save {
            SeirenEqSave::Settings(save) => save,
            SeirenEqSave::MixerVersion(save) => {
                let result = if succeeded {
                    save.version
                        .ok_or_else(|| "Seiren device document has an invalid version".to_owned())
                        .and_then(|version| {
                            let current = self.devices.contains(&save.workspace)
                                && save.workspace.read(cx).identity(cx) == save.identity;
                            if current {
                                // A canceled in-flight save can still have reached disk.
                                // Observe that local version without authorizing native work.
                                save.workspace.update(cx, |workspace, cx| {
                                    workspace.observe_seiren_document_version(version, cx)
                                });
                            }
                            if !current || self.close_requested || !save.request.is_valid() {
                                Err("Seiren mixer owner canceled after local persistence".into())
                            } else {
                                Ok(version)
                            }
                        })
                } else {
                    Err("Seiren device-document local persistence failed".into())
                };
                let _ = save.reply.send(result);
                return;
            }
        };
        if !succeeded || save.request.change().is_none() || self.close_requested {
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_eq(&save.request, None, cx)
            });
            return;
        }
        self.submit_seiren_eq_device(save.workspace, save.request, window, cx);
    }
    fn submit_seiren_eq_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: SeirenEqRequested,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || request.change().is_none()
            || !workspace.read(cx).seiren_eq_request_matches(&request, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_eq(&request, None, cx)
            });
            return;
        }
        let persisted = request.persists_local_settings();
        let routes: Vec<_> = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && matches!(route.transport(), Some(ObservedTransport::Wired))
                    && route.peer_product_id().is_none()
                    && route.product_id() == route.physical_product_id()
                    && razer_device::seiren_eq::capability(route.product_id()).is_some()
            })
            .collect();
        if routes.len() != 1 {
            self.status = if persisted {
                "麦克风设置已保存到本机；当前有线设备连接尚不唯一，设备未写入。"
            } else {
                "当前有线麦克风连接尚不唯一，音量／监听未写入。"
            }
            .into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_eq(&request, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        self.sync_audio_notification_owners(window, cx);
        self.status = "正在确认麦克风 EQ 设备设置…".into();
        cx.notify();
        self.start_seiren_audio_operation(workspace, seiren_audio_operations::Request::Eq(request), Some(route), window, cx);
    }
}
