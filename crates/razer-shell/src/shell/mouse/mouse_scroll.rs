//! Source wheel edits persist through the shared serialized writer before a
//! retained direct/receiver task. Failed writes retain the source editor value.
use super::*;
use razer_device::mouse_scroll::ScrollChange;
use razer_discovery::direct;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_products::MouseScrollRequested;
use serde_json::Value;

pub(super) fn merge_profile(
    mut saved: Device,
    captured: &Device,
    source_profile: &Value,
    changes: &[ScrollChange],
) -> anyhow::Result<Device> {
    anyhow::ensure!(
        source_profile.is_object()
            && source_profile
                .get("scrollWheel")
                .is_some_and(Value::is_object),
        "Scroll source profile has no wheel object"
    );
    let profile = captured
        .profiles
        .iter()
        .find(|profile| profile.id == captured.active_profile)
        .ok_or_else(|| anyhow::anyhow!("Scroll source profile is absent"))?;
    let target = saved
        .profiles
        .iter_mut()
        .find(|target| target.id == profile.id)
        .ok_or_else(|| anyhow::anyhow!("Scroll source profile was removed before persistence"))?;
    let settings = target.source_settings.get_or_insert_with(|| {
        let mut initial = source_profile.clone();
        initial["scrollWheel"] = serde_json::json!({});
        initial["_scrollWheelLocalFieldsV1"] = serde_json::json!([]);
        initial
    });
    anyhow::ensure!(
        settings.is_object(),
        "Scroll profile storage is not an object"
    );
    let wheel = settings
        .as_object_mut()
        .unwrap()
        .entry("scrollWheel")
        .or_insert_with(|| serde_json::json!({}));
    anyhow::ensure!(wheel.is_object(), "Stored scroll wheel is not an object");
    let fields: Vec<&str> = changes
        .iter()
        .flat_map(|change| match change {
            ScrollChange::LegacyMode(_) => &["scrollMode"][..],
            ScrollChange::ModeV2 { .. } => &["scrollMode", "disabledModes"][..],
            ScrollChange::VirtualAcceleration(_) => &["accelerationEnabled"][..],
            ScrollChange::SmartReel(_) => &["smartReelEnabled"][..],
            ScrollChange::VirtualAccelerationLevel(_) => &["accelerationLevel"][..],
            ScrollChange::SmartReelThreshold(_) => &["smartReelLevel"][..],
        })
        .copied()
        .collect();
    anyhow::ensure!(
        !fields.is_empty(),
        "Scroll save has no accepted source tasks"
    );
    // A mode waiting in the source UI's debounce is not yet a storage event.
    // Another switch/slider task must not save that unrelated optimistic value.
    for field in &fields {
        wheel[*field] = source_profile["scrollWheel"]
            .get(*field)
            .ok_or_else(|| anyhow::anyhow!("Accepted scroll field is absent: {field}"))?
            .clone();
    }
    let mut local: std::collections::BTreeSet<String> = settings
        .get("_scrollWheelLocalFieldsV1")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    for field in fields {
        local.insert(field.to_owned());
    }
    settings["_scrollWheelLocalFieldsV1"] = serde_json::json!(local);
    let root = saved
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    anyhow::ensure!(root.is_object(), "Scroll root storage is not an object");
    let previous = root
        .get("version")
        .map(|version| {
            version
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Scroll source version is not an unsigned integer"))
        })
        .transpose()?;
    root["version"] = serde_json::json!(razer_device::mapping_submission::next_source_version(
        previous
    )?);
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device() -> Device {
        let mut device = razer_model::product_preview::registered_preview(226).unwrap();
        device.profiles[0].source_settings = Some(json!({"guid":"source-guid","name":"Original",
            "scrollWheel":{"scrollMode":"Tactile","smartReelEnabled":false},"pollingRate":1000,
            "unknown":{"retained":true}}));
        device.source_device_settings = Some(
            json!({"version":7,"deviceMetadatas":{"serial":{"activeProfileGuid":"source-guid"}}}),
        );
        device
    }

    #[::core::prelude::v1::test]
    fn mouse_scroll_save_rebases_only_its_profile_fields_and_preserves_a_new_selection() {
        let captured = device();
        let mut saved = captured.clone();
        saved.profiles[0].name = "Renamed while saving".into();
        saved.profiles[0].source_settings.as_mut().unwrap()["pollingRate"] = json!(4000);
        let mut other = saved.profiles[0].clone();
        other.id = "second-domain-profile".into();
        saved.active_profile = other.id.clone();
        saved.profiles.push(other);
        let before_other = serde_json::to_value(&saved.profiles[1]).unwrap();
        let source = json!({"scrollWheel":{"scrollMode":"FreeSpin","smartReelEnabled":true},
            "_scrollWheelLocalFieldsV1":["scrollMode","smartReelEnabled"],"pollingRate":125});
        let merged = merge_profile(
            saved,
            &captured,
            &source,
            &[
                ScrollChange::LegacyMode(razer_device::mouse_scroll::ScrollMode::FreeSpin),
                ScrollChange::SmartReel(true),
            ],
        )
        .unwrap();
        assert_eq!(merged.active_profile, "second-domain-profile");
        assert_eq!(merged.profiles[0].name, "Renamed while saving");
        assert_eq!(
            merged.profiles[0].source_settings.as_ref().unwrap()["pollingRate"],
            4000
        );
        assert_eq!(
            merged.profiles[0].source_settings.as_ref().unwrap()["scrollWheel"],
            source["scrollWheel"]
        );
        assert_eq!(
            merged.profiles[0].source_settings.as_ref().unwrap()["unknown"],
            json!({"retained":true})
        );
        assert_eq!(
            serde_json::to_value(&merged.profiles[1]).unwrap(),
            before_other
        );
        let root = merged.source_device_settings.as_ref().unwrap();
        assert_eq!(root["version"], 8);
        assert_eq!(
            root["deviceMetadatas"]["serial"]["activeProfileGuid"],
            "source-guid"
        );
    }

    #[::core::prelude::v1::test]
    fn mouse_scroll_save_rejects_removed_profiles_and_malformed_or_non_integer_storage() {
        let captured = device();
        let source = json!({"scrollWheel":{"scrollMode":"FreeSpin"}});
        for failure in 0..5 {
            let mut saved = captured.clone();
            match failure {
                0 => saved.profiles.clear(),
                1 => saved.profiles[0].source_settings = Some(json!([])),
                2 => saved.source_device_settings = Some(json!([])),
                3 => saved.source_device_settings.as_mut().unwrap()["version"] = json!(1.5),
                4 => saved.source_device_settings.as_mut().unwrap()["version"] = json!("7"),
                _ => unreachable!(),
            }
            assert!(
                merge_profile(
                    saved,
                    &captured,
                    &source,
                    &[ScrollChange::LegacyMode(
                        razer_device::mouse_scroll::ScrollMode::FreeSpin
                    )]
                )
                .is_err()
            );
        }
        assert!(
            merge_profile(
                captured.clone(),
                &captured,
                &json!({"scrollWheel":null}),
                &[ScrollChange::SmartReel(true)]
            )
            .is_err()
        );
    }

    #[::core::prelude::v1::test]
    fn mouse_scroll_save_uses_current_durable_version_and_53bit_wrap() {
        let captured = device();
        let mut saved = captured.clone();
        saved.source_device_settings.as_mut().unwrap()["version"] =
            json!(razer_device::mapping_submission::MAX_SOURCE_VERSION);
        let first = merge_profile(
            saved,
            &captured,
            &json!({"scrollWheel":{"scrollMode":"FreeSpin"}}),
            &[ScrollChange::LegacyMode(
                razer_device::mouse_scroll::ScrollMode::FreeSpin,
            )],
        )
        .unwrap();
        assert_eq!(first.source_device_settings.as_ref().unwrap()["version"], 0);
        let second = merge_profile(
            first,
            &captured,
            &json!({"scrollWheel":{"scrollMode":"Tactile"}}),
            &[ScrollChange::LegacyMode(
                razer_device::mouse_scroll::ScrollMode::Tactile,
            )],
        )
        .unwrap();
        assert_eq!(
            second.source_device_settings.as_ref().unwrap()["version"],
            1
        );
        assert_eq!(
            second.profiles[0].source_settings.as_ref().unwrap()["scrollWheel"]["scrollMode"],
            "Tactile"
        );
    }
    #[::core::prelude::v1::test]
    fn mouse_scroll_save_does_not_commit_a_mode_still_waiting_in_ui_debounce() {
        let captured = device();
        let source = json!({"scrollWheel":{"scrollMode":"FreeSpin","smartReelEnabled":true},
            "_scrollWheelLocalFieldsV1":["scrollMode","smartReelEnabled"]});
        let saved = merge_profile(
            captured.clone(),
            &captured,
            &source,
            &[ScrollChange::SmartReel(true)],
        )
        .unwrap();
        let settings = saved.profiles[0].source_settings.as_ref().unwrap();
        assert_eq!(settings["scrollWheel"]["scrollMode"], "Tactile");
        assert_eq!(settings["scrollWheel"]["smartReelEnabled"], true);
        assert_eq!(
            settings["_scrollWheelLocalFieldsV1"],
            json!(["smartReelEnabled"])
        );
        let mut empty = captured.clone();
        empty.profiles[0].source_settings = None;
        let saved =
            merge_profile(empty, &captured, &source, &[ScrollChange::SmartReel(true)]).unwrap();
        let wheel = &saved.profiles[0].source_settings.as_ref().unwrap()["scrollWheel"];
        assert!(wheel.get("scrollMode").is_none());
        assert_eq!(wheel["smartReelEnabled"], true);
    }
}

impl AppShell {
    pub(super) fn rebase_mouse_scroll_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(profile) = &save.mouse_profile else {
            return Ok(());
        };
        let Some(request) = &profile.scroll else {
            return Ok(());
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == profile.identity {
                    merge_profile(
                        saved,
                        &profile.device,
                        request.profile(),
                        &request.submission().changes,
                    )
                } else {
                    Ok(saved)
                }
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }
    pub(super) fn persist_mouse_scroll(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseScrollRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.storage_error.is_some()
            || !self.devices.contains(&workspace)
            || !workspace.read(cx).mouse_scroll_save_matches(&request, cx)
        {
            if let Some(error) = &self.storage_error {
                self.status = format!("滚轮配置保存失败：{error}");
            }
            request.cancel();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_scroll(&request, cx)
            });
            cx.notify();
            return;
        }
        if request.is_saved() {
            self.submit_mouse_scroll(workspace, request, window, cx);
            return;
        }
        self.persist_legacy_profile(workspace, None, false, Some(request), window, cx);
    }

    pub(super) fn finish_mouse_scroll_save(
        &mut self,
        save: MouseLegacyProfileSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = save.scroll else {
            return;
        };
        if succeeded {
            request.mark_saved();
        }
        if !succeeded {
            request.cancel();
        }
        if !succeeded || self.close_requested || !self.devices.contains(&save.workspace) {
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_scroll(&request, cx)
            });
            return;
        }
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let workspace = save.workspace.clone();
            let guard = request.clone();
            if window
                .update(cx, |_, window, cx| {
                    let _ = owner.update(cx, |owner, cx| {
                        owner.submit_mouse_scroll(save.workspace, request, window, cx)
                    });
                })
                .is_err()
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_mouse_scroll(&guard, cx)
                });
            }
        });
    }

    fn submit_mouse_scroll(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseScrollRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !workspace
                .read(cx)
                .mouse_scroll_request_matches(&request, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_scroll(&request, cx)
            });
            return;
        }
        if !request.is_saved() || !request.begin_device() {
            return;
        }
        let routes: Vec<_> = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && direct::mouse_scroll::available(route)
            })
            .collect();
        if routes.len() != 1 {
            self.status = "滚轮设置已保存；尚无唯一的鼠标或接收器连接，设备未写入。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_scroll(&request, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let revision = self.discovery_revision;
        self.mouse_scroll_workers += 1;
        let workspace = workspace.downgrade();
        cx.spawn_in(window, async move |owner, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Scroll scope canceled before service creation"
                    );
                    let mut client = ServiceClient::spawn()?;
                    client.set_request_guard(guard.guard());
                    let result = direct::mouse_scroll::request(
                        &mut client,
                        &selected,
                        guard.submission().clone(),
                    );
                    let shutdown = if client.is_stopped() {
                        None
                    } else {
                        client.request(ServiceRequest::Shutdown).err()
                    };
                    result.map(|report| (report, shutdown))
                })
                .await;
            let _ = owner.update_in(cx, |owner, _, cx| {
                owner.mouse_scroll_workers = owner.mouse_scroll_workers.saturating_sub(1);
                if let Some(workspace) = workspace.upgrade() {
                    let current = owner.devices.contains(&workspace)
                        && workspace.read(cx).identity(cx) == identity
                        && workspace
                            .read(cx)
                            .mouse_scroll_request_matches(&request, cx)
                        && owner.discovery_revision == revision
                        && owner.device_observations.iter().any(|current| {
                            current.matches(workspace.read(cx).device(cx))
                                && current.physical_product_id() == route.physical_product_id()
                                && current.container().eq_ignore_ascii_case(route.container())
                                && current.hid_node() == route.hid_node()
                                && current.peer_product_id() == route.peer_product_id()
                                && current.connection() == route.connection()
                        });
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_scroll(&request, cx)
                    });
                    if current {
                        owner.status = match result {
                            Ok((_, None)) => "滚轮设置已保存，设备写入及读回已确认。".into(),
                            Ok((_, Some(error))) => {
                                format!("滚轮设备操作已确认；通信进程退出失败：{error:#}")
                            }
                            Err(error) => {
                                format!("滚轮设备状态尚未确认：{error:#}；已保留当前设置。")
                            }
                        };
                    }
                }
                if owner.close_requested && owner.save_task.is_none() {
                    owner.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
