//! Original sensitivity-clutch warning changes only an existing synapse_PID object.
use super::*;
impl AppShell {
    pub(super) fn read_keyboard_sensitivity_preference(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        product_id: u32,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(workspace)
            || workspace.read(cx).device(cx).product_id != product_id
        {
            return;
        }
        let owner = workspace.read(cx).device(cx).clone();
        let result = razer_storage::source_local::path()
            .and_then(|path| {
                razer_storage::source_local::get_raw(&path, &format!("synapse_{product_id}"))
            })
            .and_then(|raw| {
                self.settings.update(cx, |settings, cx| {
                    settings.observe_source_product_preference(product_id, raw, cx)
                })
            })
            .and_then(|_| self.settings.read(cx).source_product_preference(product_id));
        match result {
            Ok(preference) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_keyboard_mapping_sensitivity_warning(&owner, preference, cx)
                });
            }
            Err(error) => {
                eprintln!("[keyboard-clutch] source preference read/parse failed: {error:#}")
            }
        }
    }
    pub(super) fn hide_keyboard_sensitivity_preference(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        product_id: u32,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(workspace)
            || workspace.read(cx).device(cx).product_id != product_id
        {
            return;
        }
        let key = format!("synapse_{product_id}");
        let result = (|| -> anyhow::Result<bool> {
            let path = razer_storage::source_local::path()?;
            let raw = razer_storage::source_local::get_raw(&path, &key)?;
            self.settings.update(cx, |settings, cx| {
                settings.observe_source_product_preference(product_id, raw, cx)
            })?;
            if !self.settings.update(cx, |settings, cx| {
                settings.hide_keyboard_mapping_sensitivity_warning(product_id, cx)
            }) {
                return Ok(false);
            }
            let value = self
                .settings
                .read(cx)
                .source_product_preference(product_id)?
                .ok_or_else(|| {
                    anyhow::anyhow!("source preference disappeared before local write")
                })?;
            razer_storage::source_local::set(&path, &key, &value)?;
            let raw = razer_storage::source_local::get_raw(&path, &key)?;
            self.settings.update(cx, |settings, cx| {
                settings.observe_source_product_preference(product_id, raw, cx)
            })?;
            Ok(true)
        })();
        match result {
            Ok(true) => self.save_auxiliary_preferences(cx),
            Ok(false) => {}
            Err(error) => {
                eprintln!("[keyboard-clutch] source preference write/readback failed: {error:#}")
            }
        }
    }
}
