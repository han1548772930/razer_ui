//! Current 182 stage intents: local profile persistence before device submission.
use super::*;
use razer_device::mouse_dpi_stages::{DpiStagesDraft, DpiStagesReading};
use razer_discovery::direct;
use razer_model::model::Profile;
use razer_pages::features::mouse_polling::MousePollingScope;

#[path = "mouse_dpi_stages_runtime.rs"]
mod runtime;
#[path = "mouse_dpi_stages_worker.rs"]
mod worker;
pub(super) use runtime::State;

pub(super) struct MouseDpiStagesSave {
    workspace: WeakEntity<ProductWorkspace>,
    scope: MousePollingScope,
    revision: u64,
    draft: DpiStagesDraft,
    identity: String,
    profile: Profile,
}
impl MouseDpiStagesSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::mouse_dpi_stages::DpiStage;

    #[::core::prelude::v1::test]
    fn queued_stage_save_merges_only_requested_sensitivity_and_advances_root_version() {
        let mut saved = razer_model::product_preview::registered_preview(182).unwrap();
        saved.profiles[0].settings = Some(razer_model::settings::ProfileSettings::for_product(182));
        saved.profiles[0].source_settings =
            Some(serde_json::json!({"pollingRate":500,"dpiStages":{"extra":"retain"}}));
        saved.source_device_settings =
            Some(serde_json::json!({"version":4,"deviceSetting":{"enabled":true}}));
        let profile = saved.profiles[0].clone();
        // A different save committed while this DPI request waited.
        saved.profiles[0].settings.as_mut().unwrap().polling_wired = Some(500);
        let draft = DpiStagesDraft {
            enabled: true,
            active_stage: 3,
            stages: [400, 800, 2000, 3200, 6400]
                .into_iter()
                .enumerate()
                .map(|(index, value)| DpiStage {
                    x: value,
                    y: if index == 2 { 2100 } else { value },
                    visible: index == 0 || index == 2,
                    independent: index == 2,
                })
                .collect(),
        };
        assert!(merge_dpi_profile(&mut saved, &profile, &draft));
        let settings = saved.profiles[0].settings.as_ref().unwrap();
        assert_eq!(settings.polling_wired, Some(500));
        assert_eq!(settings.sensitivity.stages[2], [2000, 2100]);
        assert_eq!(settings.sensitivity.active, 2);
        assert!(settings.sensitivity.independent);
        assert_eq!(
            settings
                .sensitivity
                .slots
                .iter()
                .map(|slot| slot.id)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        assert_eq!(
            settings
                .sensitivity
                .slots
                .iter()
                .filter(|slot| slot.enabled)
                .count(),
            2
        );
        let source = saved.profiles[0].source_settings.as_ref().unwrap();
        assert_eq!(source["pollingRate"], 500);
        assert_eq!(source["dpiStages"]["extra"], "retain");
        assert_eq!(source["dpiStages"]["count"], 2);
        assert_eq!(source["dpiStages"]["stages"][4]["x"], 6400);
        assert_eq!(saved.source_device_settings.as_ref().unwrap()["version"], 5);
        assert_eq!(
            saved.source_device_settings.as_ref().unwrap()["deviceSetting"]["enabled"],
            true
        );
        saved.source_device_settings.as_mut().unwrap()["version"] =
            serde_json::json!(razer_device::mapping_submission::MAX_SOURCE_VERSION);
        assert!(merge_dpi_profile(&mut saved, &profile, &draft));
        assert_eq!(saved.source_device_settings.as_ref().unwrap()["version"], 0);
    }

    #[::core::prelude::v1::test]
    fn invalid_stage_save_cannot_resurrect_a_removed_profile_or_partially_update_storage() {
        let mut saved = razer_model::product_preview::registered_preview(182).unwrap();
        saved.profiles[0].settings = Some(razer_model::settings::ProfileSettings::for_product(182));
        let profile = saved.profiles[0].clone();
        let mut draft = DpiStagesDraft {
            enabled: false,
            active_stage: 0,
            stages: vec![],
        };
        let before = serde_json::to_value(&saved).unwrap();
        assert!(!merge_dpi_profile(&mut saved, &profile, &draft));
        assert_eq!(serde_json::to_value(&saved).unwrap(), before);
        draft.active_stage = 1;
        draft.stages = vec![
            DpiStage {
                x: 800,
                y: 800,
                visible: true,
                independent: false
            };
            5
        ];
        saved.profiles.clear();
        assert!(!merge_dpi_profile(&mut saved, &profile, &draft));
        assert!(saved.profiles.is_empty());
    }
}

/// Current 182 module30525/ft updates only dpiStages and the root version.
/// Stable slot IDs are Rust UI identity; all hidden rows and XY flags survive.
fn merge_dpi_profile(saved: &mut Device, profile: &Profile, draft: &DpiStagesDraft) -> bool {
    if razer_device::mouse_dpi_stages::capability(saved.product_id)
        .is_none_or(|cap| razer_device::mouse_dpi_stages::pack(cap, draft).is_err())
        || saved
            .source_device_settings
            .as_ref()
            .is_some_and(|root| !root.is_object())
    {
        return false;
    }
    let Ok(next_version) = razer_device::mapping_submission::next_source_version(
        saved
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get("version"))
            .and_then(serde_json::Value::as_u64),
    ) else {
        return false;
    };
    let Some(destination) = saved.profiles.iter_mut().find(|p| p.id == profile.id) else {
        return false;
    };
    if destination.source_settings.as_ref().is_some_and(|source| {
        !source.is_object()
            || source
                .get("dpiStages")
                .is_some_and(|stages| !stages.is_object())
    }) {
        return false;
    }
    let Some(captured) = profile.settings.as_ref() else {
        return false;
    };
    if captured.sensitivity.slots.len() != draft.stages.len() {
        return false;
    }
    let settings = destination
        .settings
        .get_or_insert_with(|| razer_model::settings::ProfileSettings::for_product(182));
    settings.sensitivity = captured.sensitivity.clone();
    settings.sensitivity.visible = draft.enabled;
    settings.sensitivity.active = draft.active_stage - 1;
    settings.sensitivity.stages = draft
        .stages
        .iter()
        .map(|stage| [u32::from(stage.x), u32::from(stage.y)])
        .collect();
    for (slot, stage) in settings.sensitivity.slots.iter_mut().zip(&draft.stages) {
        slot.enabled = stage.visible;
        slot.independent = stage.independent;
    }
    settings.sensitivity.independent = draft.stages[draft.active_stage - 1].independent;
    let source = destination
        .source_settings
        .get_or_insert_with(|| serde_json::json!({}));
    let stages = source
        .as_object_mut()
        .expect("validated source profile object")
        .entry("dpiStages")
        .or_insert_with(|| serde_json::json!({}));
    stages["stages"] = serde_json::json!(draft.stages);
    stages["active"] = serde_json::json!(draft.active_stage);
    stages["enable"] = serde_json::json!(draft.enabled);
    stages["count"] = serde_json::json!(draft.stages.iter().filter(|row| row.visible).count());
    let root = saved
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    root["version"] = serde_json::json!(next_version);
    true
}

impl AppShell {
    pub(super) fn write_mouse_dpi_stages(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        revision: u64,
        draft: DpiStagesDraft,
        persist_local: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !persist_local {
            self.mouse_dpi_request(workspace, scope, revision, Some(draft), cx);
            return;
        }
        let valid = self.storage_error.is_none()
            && !self.close_requested
            && self.devices.contains(&workspace)
            && workspace
                .read(cx)
                .mouse_dpi_request_matches(scope, revision, &draft, cx);
        let device = workspace.read(cx).snapshot(cx);
        let profile = device
            .profiles
            .iter()
            .find(|profile| profile.id == device.active_profile);
        if !valid || profile.is_none() {
            if let Some(error) = &self.storage_error {
                self.status = format!("DPI 本地保存失败：{error}");
            }
            workspace.update(cx, |owner, cx| {
                owner.finish_mouse_dpi(scope, revision, &draft, None, cx)
            });
            cx.notify();
            return;
        }
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
            Some(ProductSave::MouseDpiStages(MouseDpiStagesSave {
                workspace: workspace.downgrade(),
                scope,
                revision,
                draft,
                identity: workspace.read(cx).identity(cx),
                profile: profile.expect("checked active profile").clone(),
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_mouse_dpi_stages_save(&self, save: &mut PreparedSave, cx: &App) -> bool {
        let Some(ProductSave::MouseDpiStages(dpi)) = &save.product else {
            return true;
        };
        let Some(workspace) = dpi.workspace.upgrade() else {
            return false;
        };
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace.read(cx).mouse_dpi_request_matches(
                dpi.scope,
                dpi.revision,
                &dpi.draft,
                cx,
            )
        {
            return false;
        }
        let mut matched = false;
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == dpi.identity {
                    matched = merge_dpi_profile(&mut saved, &dpi.profile, &dpi.draft);
                }
                saved
            })
            .collect();
        matched
    }

    pub(super) fn finish_mouse_dpi_stages_save(
        &mut self,
        save: MouseDpiStagesSave,
        succeeded: bool,
        _window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = save.workspace.upgrade() else {
            return;
        };
        if !succeeded || self.close_requested {
            workspace.update(cx, |owner, cx| {
                owner.finish_mouse_dpi(save.scope, save.revision, &save.draft, None, cx)
            });
            return;
        }
        self.mouse_dpi_request(workspace, save.scope, save.revision, Some(save.draft), cx);
    }

    pub(super) fn read_mouse_dpi_stages(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        revision: u64,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mouse_dpi_request(workspace, scope, revision, None, cx);
    }

    fn mouse_dpi_request(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        revision: u64,
        draft: Option<DpiStagesDraft>,
        cx: &mut Context<Self>,
    ) {
        let matches = match &draft {
            Some(draft) => workspace
                .read(cx)
                .mouse_dpi_request_matches(scope, revision, draft, cx),
            None => workspace
                .read(cx)
                .mouse_dpi_read_matches(scope, revision, cx),
        };
        if !matches || self.close_requested || !self.devices.contains(&workspace) {
            workspace.update(cx, |owner, cx| match &draft {
                Some(draft) => owner.finish_mouse_dpi(scope, revision, draft, None, cx),
                None => owner.finish_mouse_dpi_read(scope, revision, None, cx),
            });
            return;
        }
        let device = workspace.read(cx).device(cx);
        let valid_scope = match &draft {
            Some(draft) => workspace
                .read(cx)
                .mouse_dpi_write_guard(scope, revision, draft, cx),
            None => workspace.read(cx).mouse_dpi_read_guard(scope, revision, cx),
        };
        let Some(valid_scope) = valid_scope else {
            return;
        };
        let routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(device) && direct::mouse_dpi_stages_available(route))
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            self.status = "DPI 阶段表未发送：需要唯一且仍连接的设备或接收器路径。".into();
            workspace.update(cx, |owner, cx| match &draft {
                Some(draft) => owner.finish_mouse_dpi(scope, revision, draft, None, cx),
                None => owner.finish_mouse_dpi_read(scope, revision, None, cx),
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        self.start_mouse_dpi_stages_operation(
            workspace,
            route,
            runtime::Request {
                scope,
                revision,
                draft,
                valid: valid_scope,
            },
            cx,
        );
    }
}
