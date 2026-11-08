//! OTFS uses the existing local save queue before its retained mapping publisher.
use super::*;
use anyhow::Context as _;
use razer_pages::features::mouse_products::MouseOtfsRequested;
use serde_json::Value;

pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: MouseOtfsRequested,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

pub(super) fn prepare(save: &mut PreparedSave) -> anyhow::Result<bool> {
    let Some(ProductSave::MouseOtfs(local)) = &mut save.product else {
        return Ok(true);
    };
    anyhow::ensure!(
        local.request.is_valid(),
        "OTFS request owner was replaced before persistence"
    );
    let device = save
        .file
        .devices
        .iter_mut()
        .find(|device| device.workspace_identity() == local.identity)
        .context("OTFS device was removed from the local library")?;
    let product = razer_device::mapping_document::MappingProduct::for_product_id(device.product_id)
        .context("OTFS mapping product is unavailable")?;
    let current = device
        .source_device_settings
        .as_ref()
        .and_then(|settings| settings.get(product.storage_field()))
        .context("OTFS current mapping document is absent")?;
    local.request.rebase(current)?;
    if !local.request.publication_required() {
        return Ok(false);
    }
    apply_projection(device, local.request.document(), product.storage_field())?;
    Ok(true)
}

fn apply_projection(device: &mut Device, document: &Value, field: &str) -> anyhow::Result<()> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Projection {
        active_profile: String,
        profiles: Vec<ProjectedProfile>,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ProjectedProfile {
        guid: String,
        app_engine: Option<Value>,
        default_mappings: Option<Value>,
    }
    let projection: Projection = serde_json::from_value(document.clone())?;
    let projected = projection
        .profiles
        .into_iter()
        .find(|profile| profile.guid == projection.active_profile)
        .context("OTFS compiled active profile is absent")?;
    let profile = device
        .profiles
        .iter_mut()
        .find(|profile| profile.guid == projected.guid)
        .context("OTFS current source profile was removed")?;
    let fields = profile
        .source_settings
        .as_mut()
        .and_then(Value::as_object_mut)
        .context("OTFS source profile is absent")?;
    fields.insert(
        "appEngine".into(),
        projected.app_engine.context("OTFS engine is absent")?,
    );
    fields.insert(
        "defaultMappings".into(),
        projected
            .default_mappings
            .context("OTFS defaults are absent")?,
    );
    device
        .source_device_settings
        .as_mut()
        .and_then(Value::as_object_mut)
        .context("OTFS source document is missing")?
        .insert(field.into(), document.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_projection;
    use serde_json::json;

    #[test]
    fn otfs_projection_updates_only_current_profile_compilation_and_owned_document() {
        for pid in [70, 158] {
            let mut device = razer_model::product_preview::registered_preview(pid).unwrap();
            let first = razer_model::model::Profile {
                id: "a".into(),
                guid: "a".into(),
                name: "A".into(),
                settings: None,
                dpi_stages: None,
                source_settings: Some(
                    json!({"guid":"a","mappings":[{"sentinel":1}],"name":"A","extension":9}),
                ),
            };
            let mut second = first.clone();
            second.id = "b".into();
            second.guid = "b".into();
            second.source_settings.as_mut().unwrap()["guid"] = json!("b");
            device.profiles = vec![first, second];
            device.source_device_settings = Some(json!({"other":42}));
            let before = device.profiles[0].clone();
            let doc = json!({"activeProfile":"b","profiles":[{"guid":"b","appEngine":{"hash":"temporary","mappings":[]},"defaultMappings":{"hash":"default","mappings":[]}}]});
            apply_projection(&mut device, &doc, "owned").unwrap();
            assert_eq!(device.profiles[0], before);
            let current = device.profiles[1].source_settings.as_ref().unwrap();
            assert_eq!(current["mappings"], json!([{"sentinel":1}]));
            assert_eq!(current["extension"], 9);
            assert_eq!(current["appEngine"]["hash"], "temporary");
            assert_eq!(device.source_device_settings.as_ref().unwrap()["other"], 42);
            assert_eq!(
                device.source_device_settings.as_ref().unwrap()["owned"],
                doc
            );
        }
    }
}

impl AppShell {
    pub(super) fn persist_mouse_otfs(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseOtfsRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace)
            || !workspace.read(cx).accepts_mouse_otfs(&request, cx)
        {
            return;
        }
        if self.storage_error.is_some() {
            workspace.update(cx, |owner, cx| {
                owner.finish_mouse_otfs_save(&request, false, cx)
            });
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|owner| owner.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::MouseOtfs(LocalSave {
                workspace,
                identity,
                request,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn finish_mouse_otfs_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        save.workspace.update(cx, |owner, cx| {
            owner.finish_mouse_otfs_save(&save.request, succeeded, cx)
        });
    }
}
