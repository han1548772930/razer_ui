//! Local profile imports use the workspace writer without activating hardware.
use super::*;
use razer_model::{
    macro_library::MacroLibraryFile,
    profile_transfer::{ProfileMacroImport, SourceProfileImport},
};

pub(super) struct ImportSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
    request: SourceProfileImport,
    saved_baseline: Device,
    macros: Option<ProfileMacroImport>,
}
impl ImportSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    fn owner_current(&self, devices: &[Entity<ProductWorkspace>], cx: &App) -> bool {
        devices.contains(&self.workspace)
            && self.workspace.read(cx).identity(cx) == self.identity
            && self.workspace.read(cx).connection_revision() == self.connection_revision
    }
    fn current(
        &self,
        devices: &[Entity<ProductWorkspace>],
        macros: &MacroLibraryFile,
        cx: &App,
    ) -> bool {
        self.owner_current(devices, cx)
            && self.request.matches(self.workspace.read(cx).device(cx))
            && self
                .macros
                .as_ref()
                .is_none_or(|import| import.matches(macros))
    }
}
impl AppShell {
    pub(super) fn persist_source_profile_import(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: SourceProfileImport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let macros = self.macro_library.read(cx).snapshot();
        let macro_import = match request.prepare_macros(&macros) {
            Ok(import) => import,
            Err(error) => {
                let _ = workspace.update(cx, |workspace, cx| {
                    workspace.finish_source_profile_import(
                        &request,
                        Err("The profile import was not saved locally".into()),
                        cx,
                    )
                });
                self.status = error.clone();
                window.push_notification(error, cx);
                cx.notify();
                return;
            }
        };
        let save = ImportSave {
            identity: workspace.read(cx).identity(cx),
            connection_revision: workspace.read(cx).connection_revision(),
            saved_baseline: workspace.read(cx).saved_snapshot(cx),
            workspace,
            request,
            macros: macro_import,
        };
        if self.close_requested || !save.current(&self.devices, &macros, cx) {
            let result = save.workspace.update(cx, |workspace, cx| {
                workspace.finish_source_profile_import(
                    &save.request,
                    Err("The profile import was not saved locally".into()),
                    cx,
                )
            });
            if let Err(error) = result {
                window.push_notification(error, cx);
            }
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|workspace| workspace.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::SourceProfileImport(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_source_profile_import(
        &self,
        prepared: &mut PreparedSave,
        cx: &App,
    ) -> Result<(), String> {
        let Some(ProductSave::SourceProfileImport(save)) = &prepared.product else {
            return Ok(());
        };
        if !save.current(&self.devices, &self.macro_library.read(cx).snapshot(), cx) {
            return Err(
                "The profile or macro import owner changed before local persistence".into(),
            );
        }
        prepared.file.devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).saved_snapshot(cx))
            .collect();
        let target = prepared
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == save.identity)
            .ok_or("Profile import persistence owner is absent")?;
        if target.profiles != save.saved_baseline.profiles
            || target.active_profile != save.saved_baseline.active_profile
            || target.source_device_settings != save.saved_baseline.source_device_settings
        {
            return Err("Profile import conflicts with a newer local save revision".into());
        }
        target.profiles = save.request.candidate().profiles.clone();
        target.source_device_settings = save.request.candidate().source_device_settings.clone();
        target.active_profile = save.request.candidate().active_profile.clone();
        if let Some(import) = &save.macros {
            prepared.file.macros = import.candidate().clone();
        }
        Ok(())
    }

    pub(super) fn finish_source_profile_import(
        &mut self,
        save: ImportSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let receipt = if succeeded {
            (|| {
                if !save.owner_current(&self.devices, cx) {
                    return Err("The profile import device owner changed during persistence".into());
                }
                let device = save
                    .request
                    .merge_receipt(save.workspace.read(cx).device(cx))?;
                let macros = save
                    .macros
                    .as_ref()
                    .map(|import| import.merge_receipt(&self.macro_library.read(cx).snapshot()))
                    .transpose()?;
                Ok((device, macros))
            })()
            .map_err(|error: String| {
                format!(
                    "导入文件已写入本机，但当前配置所有者或状态冲突，已停止后续覆盖：{error}。请重新启动以读取已保存配置。"
                )
            })
        } else {
            Err("The profile import was not saved locally".into())
        };
        // Both documents are validated before either owner is changed. These
        // synchronous commits acknowledge storage only, without native apply.
        let mut result = match receipt {
            Ok((device, macros)) => {
                let result = save.workspace.update(cx, |workspace, cx| {
                    workspace.finish_source_profile_import(&save.request, Ok(device), cx)
                });
                if matches!(&result, Ok(true)) {
                    if let (Some(import), Some(merged)) = (&save.macros, macros) {
                        self.macro_library.update(cx, |library, cx| {
                            library.publish_profile_import(import, merged, cx)
                        });
                    }
                    self.acknowledge_keyboard_latest_profile_collection(save.workspace.clone(), cx);
                }
                result
            }
            Err(error) => save.workspace.update(cx, |workspace, cx| {
                workspace.finish_source_profile_import(&save.request, Err(error), cx)
            }),
        };
        if succeeded && !matches!(&result, Ok(true)) {
            let error = match result {
                Err(error) => error,
                _ => "导入文件已写入本机，但导入请求已变化，已停止后续覆盖。请重新启动以读取已保存配置。".into(),
            };
            self.storage_error = Some(error.clone());
            result = Err(error);
        }
        if let Err(error) = &result {
            self.status = error.clone();
        }
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| match result {
                Ok(true) => save.workspace.update(cx, |workspace, cx| {
                    workspace.refresh_transferred_profiles(window, cx)
                }),
                Err(error) => window.push_notification(error, cx),
                Ok(false) => (),
            });
        });
    }
}
