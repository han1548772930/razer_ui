use super::*;

pub(in crate::shell) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    request: MouseDynamicRequested,
}
impl LocalSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

fn merge(saved: &mut Device, profile_id: &str, value: &serde_json::Value) -> anyhow::Result<()> {
    anyhow::ensure!(
        razer_device::mouse_dynamic::capability(saved.product_id).is_some(),
        "Dynamic product has no current task-maker evidence"
    );
    anyhow::ensure!(
        value.is_object(),
        "Dynamic profile setting is not an object"
    );
    let root = saved.source_device_settings.as_ref();
    anyhow::ensure!(
        root.is_none_or(serde_json::Value::is_object),
        "Dynamic source device document is invalid"
    );
    let previous = root
        .and_then(|root| root.get("version"))
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Dynamic source version is invalid"))
        })
        .transpose()?;
    let version = razer_device::mapping_submission::next_source_version(previous)?;
    let profile = saved
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| anyhow::anyhow!("Dynamic profile was removed before local persistence"))?;
    let source = profile
        .source_settings
        .get_or_insert_with(|| serde_json::json!({}));
    anyhow::ensure!(source.is_object(), "Dynamic source profile is invalid");
    source["dynamicSensitivity"] = value.clone();
    saved
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}))["version"] = serde_json::json!(version);
    Ok(())
}

impl AppShell {
    pub(super) fn persist_mouse_dynamic(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseDynamicRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || self.storage_error.is_some() {
            let message = self
                .storage_error
                .clone()
                .unwrap_or_else(|| "Application is closing".into());
            self.finish_mouse_dynamic(
                &workspace,
                &request,
                MouseDynamicCompletion::Failed(message),
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
            self.finish_mouse_dynamic(
                &workspace,
                &request,
                MouseDynamicCompletion::Failed("Dynamic active local profile is absent".into()),
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
            Some(ProductSave::MouseDynamic(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(in crate::shell) fn rebase_mouse_dynamic_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MouseDynamic(dynamic)) = &save.product else {
            return Ok(());
        };
        anyhow::ensure!(
            dynamic
                .workspace
                .read(cx)
                .mouse_dynamic_request_is_current(&dynamic.request, cx),
            "Dynamic profile or connection changed before local persistence"
        );
        let value = dynamic
            .request
            .payload()
            .get("dynamicSensitivity")
            .ok_or_else(|| anyhow::anyhow!("Dynamic local task omits its profile setting"))?;
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| {
                let mut saved = workspace.read(cx).saved_snapshot(cx);
                if workspace.read(cx).identity(cx) == dynamic.identity {
                    merge(&mut saved, &dynamic.profile_id, value)?;
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }
    pub(in crate::shell) fn finish_mouse_dynamic_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !succeeded {
            self.finish_mouse_dynamic(&save.workspace,&save.request,MouseDynamicCompletion::Failed("Dynamic local profile persistence did not succeed; device task was not submitted".into()),cx);
            return;
        }
        self.submit_mouse_dynamic(
            save.workspace,
            save.request,
            Admission::AcceptedWrite,
            window,
            cx,
        );
    }
}

#[cfg(test)]
#[path = "mouse_dynamic_save_tests.rs"]
mod tests;
