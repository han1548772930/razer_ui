use super::*;

pub(in crate::shell) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    request: MouseRotationRequested,
}
impl LocalSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

fn merge(saved: &mut Device, profile_id: &str, setting: Setting) -> anyhow::Result<()> {
    setting.validate()?;
    anyhow::ensure!(
        rotation::capability(saved.product_id).is_some(),
        "Rotation product has no current source task"
    );
    let root = saved.source_device_settings.as_ref();
    anyhow::ensure!(
        root.is_none_or(serde_json::Value::is_object),
        "Rotation source device document is invalid"
    );
    let previous = root
        .and_then(|root| root.get("version"))
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Rotation source version is invalid"))
        })
        .transpose()?;
    let version = razer_device::mapping_submission::next_source_version(previous)?;
    let profile = saved
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| anyhow::anyhow!("Rotation profile was removed before persistence"))?;
    anyhow::ensure!(
        profile
            .source_settings
            .as_ref()
            .is_none_or(serde_json::Value::is_object),
        "Rotation source profile is invalid"
    );
    profile
        .source_settings
        .get_or_insert_with(|| serde_json::json!({}))["rotation"] =
        serde_json::json!({"isEnabled":setting.enabled,"value":setting.angle_degrees});
    saved
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}))["version"] = serde_json::json!(version);
    Ok(())
}

impl AppShell {
    pub(super) fn persist_mouse_rotation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseRotationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || self.storage_error.is_some() {
            let message = self
                .storage_error
                .clone()
                .unwrap_or_else(|| "Application is closing".into());
            self.finish_mouse_rotation(
                &workspace,
                &request,
                MouseRotationCompletion::Failed(message),
                cx,
            );
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        if !device
            .profiles
            .iter()
            .any(|profile| profile.id == device.active_profile)
        {
            self.finish_mouse_rotation(
                &workspace,
                &request,
                MouseRotationCompletion::Failed("Rotation active local profile is absent".into()),
                cx,
            );
            return;
        }
        let save = LocalSave {
            identity: workspace.read(cx).identity(cx),
            profile_id: device.active_profile,
            workspace,
            request,
        };
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
            Some(ProductSave::MouseRotation(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(in crate::shell) fn rebase_mouse_rotation_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MouseRotation(rotation)) = &save.product else {
            return Ok(());
        };
        anyhow::ensure!(
            rotation
                .workspace
                .read(cx)
                .mouse_rotation_request_is_current(&rotation.request, cx),
            "Rotation profile or connection changed before persistence"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| {
                let mut saved = workspace.read(cx).saved_snapshot(cx);
                if workspace.read(cx).identity(cx) == rotation.identity {
                    merge(&mut saved, &rotation.profile_id, rotation.request.setting())?;
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }

    pub(in crate::shell) fn finish_mouse_rotation_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if !succeeded {
            self.finish_mouse_rotation(
                &save.workspace,
                &save.request,
                MouseRotationCompletion::Failed(
                    "Rotation local persistence did not succeed; device task was not submitted"
                        .into(),
                ),
                cx,
            );
            return;
        }
        let current = save.workspace.update(cx, |workspace, cx| {
            workspace.mark_mouse_rotation_persisted(&save.request, cx)
        });
        if !current || self.receiver_macro_exclusive(cx) {
            self.finish_mouse_rotation(
                &save.workspace,
                &save.request,
                MouseRotationCompletion::Cancelled,
                cx,
            );
            return;
        }
        self.submit_mouse_rotation(save.workspace, save.request, Admission::AcceptedWrite, cx);
    }
}

#[cfg(test)]
#[path = "mouse_rotation_save_tests.rs"]
mod tests;
