//! Persist the accepted profile intent before its source hardware task starts.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::mouse_brightness::{BrightnessChange, Submission};
use razer_discovery::direct;
use razer_discovery::discovery::ObservedDevice;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Brightness {
    is_enabled: bool,
    value: u8,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}
impl Brightness {
    fn submission(&self, request: &MouseLightingRequested) -> anyhow::Result<Submission> {
        let submission = Submission::Apply {
            change: BrightnessChange {
                is_enabled: self.is_enabled,
                percent: self.value,
                region_id: 0,
            },
            profile_guid: request
                .profile_guid()
                .context("Brightness request has no active source profile")?
                .into(),
        };
        submission.validate(request.product_id())?;
        Ok(submission)
    }
}

pub(in crate::shell) struct ProductLightingSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    connection_revision: u64,
    route: Option<ObservedDevice>,
    request: MouseLightingRequested,
    brightness: Brightness,
}
impl ProductLightingSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        shell.devices.contains(&self.workspace)
            && self.workspace.read(cx).identity(cx) == self.identity
            && self.workspace.read(cx).connection_revision() == self.connection_revision
            && self.workspace.read(cx).device(cx).active_profile == self.profile_id
            && self
                .workspace
                .read(cx)
                .mouse_lighting_request_matches(&self.request, cx)
            && self.route.as_ref().is_none_or(|route| {
                super::super::device_discovery::retained_connection(
                    std::slice::from_ref(route),
                    &shell.device_observations,
                    self.workspace.read(cx).device(cx),
                ) && direct::mouse_brightness::available(route)
            })
    }
}

fn rebase_brightness(
    device: &mut Device,
    profile_id: &str,
    brightness: &Brightness,
) -> anyhow::Result<()> {
    let profile = device
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .context("Brightness profile was removed before persistence")?;
    let settings = profile
        .source_settings
        .as_mut()
        .and_then(Value::as_object_mut)
        .context("Brightness source profile is not an object")?;
    let target = settings
        .get_mut("brightness")
        .and_then(Value::as_object_mut)
        .context("Current profile brightness is missing")?;
    let Value::Object(fields) = serde_json::to_value(brightness)? else {
        unreachable!("Named brightness fields serialize as an object");
    };
    target.extend(fields);
    let root = device
        .source_device_settings
        .get_or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("Brightness device configuration is not an object")?;
    let previous = root
        .get("version")
        .map(|value| {
            value
                .as_u64()
                .context("Brightness source version is not an unsigned integer")
        })
        .transpose()?;
    root.insert(
        "version".into(),
        razer_device::mapping_submission::next_source_version(previous)?.into(),
    );
    Ok(())
}

impl AppShell {
    pub(super) fn persist_mouse_lighting_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseLightingRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let validation = (|| -> anyhow::Result<Brightness> {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Envelope {
                brightness: Brightness,
            }
            let envelope: Envelope = serde_json::from_value(request.payload().clone())?;
            envelope.brightness.submission(&request)?;
            ensure!(
                !self.close_requested,
                "Brightness request canceled during shutdown"
            );
            ensure!(
                self.storage_error.is_none(),
                "{}",
                self.storage_error
                    .as_deref()
                    .unwrap_or("Storage is unavailable")
            );
            Ok(envelope.brightness)
        })();
        let brightness = match validation {
            Ok(brightness) => brightness,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.complete_mouse_lighting(
                        &request,
                        Err(format!("{error:#}")),
                        window,
                        cx,
                    )
                });
                return;
            }
        };
        let mut routes = self
            .device_observations
            .iter()
            .filter(|route| route.matches(workspace.read(cx).device(cx)));
        let route = routes
            .next()
            .filter(|route| routes.next().is_none() && direct::mouse_brightness::available(route))
            .cloned();
        let save = ProductLightingSave {
            identity: workspace.read(cx).identity(cx),
            profile_id: workspace.read(cx).device(cx).active_profile.clone(),
            connection_revision: workspace.read(cx).connection_revision(),
            route,
            workspace,
            request,
            brightness,
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
            Some(ProductSave::ProductLighting(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(in crate::shell) fn rebase_product_lighting_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::ProductLighting(lighting)) = &save.product else {
            return Ok(());
        };
        ensure!(
            lighting.current(self, cx),
            "Lighting profile owner or physical connection changed before persistence"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).saved_snapshot(cx))
            .collect();
        let index = self
            .devices
            .iter()
            .position(|workspace| workspace.read(cx).identity(cx) == lighting.identity)
            .context("Lighting device was removed before persistence")?;
        rebase_brightness(
            save.file
                .devices
                .get_mut(index)
                .context("Lighting device snapshot is absent")?,
            &lighting.profile_id,
            &lighting.brightness,
        )
    }
    pub(in crate::shell) fn finish_product_lighting_save(
        &mut self,
        save: ProductLightingSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let current = save.current(self, cx);
        if !succeeded || !current {
            let error = if !succeeded {
                "Lighting local profile persistence did not complete"
            } else {
                "Lighting profile owner changed after local persistence"
            };
            if window
                .update(cx, |_, window, cx| {
                    save.workspace.update(cx, |workspace, cx| {
                        workspace.complete_mouse_lighting(
                            &save.request,
                            Err(error.into()),
                            window,
                            cx,
                        )
                    })
                })
                .is_err()
            {
                save.workspace.update(cx, |workspace, cx| {
                    workspace.abandon_mouse_lighting(&save.request, Some(error.into()), cx)
                });
            }
            return;
        }
        // Register the retained device task before the save queue can finish
        // its exit decision. A requested close drains this accepted write.
        self.submit_mouse_lighting_brightness(save, window, cx);
    }
    fn submit_mouse_lighting_brightness(
        &mut self,
        save: ProductLightingSave,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !save.current(self, cx) {
            save.workspace.update(cx, |workspace, cx| {
                workspace.abandon_mouse_lighting(
                    &save.request,
                    Some("Lighting connection owner changed after local persistence".into()),
                    cx,
                )
            });
            return;
        }
        let result = (|| -> anyhow::Result<_> {
            let route = save.route.clone().context("Brightness was saved locally; no unique physical mouse route was present when the request was accepted")?;
            Ok((route, save.brightness.submission(&save.request)?))
        })();
        match result {
            Ok((route, submission)) => self.start_mouse_lighting_brightness(
                save.workspace,
                save.request,
                route,
                submission,
                window,
                cx,
            ),
            Err(error) => {
                let error = format!("{error:#}");
                if window
                    .update(cx, |_, window, cx| {
                        save.workspace.update(cx, |workspace, cx| {
                            workspace.complete_mouse_lighting(
                                &save.request,
                                Err(error.clone()),
                                window,
                                cx,
                            )
                        })
                    })
                    .is_err()
                {
                    save.workspace.update(cx, |workspace, cx| {
                        workspace.abandon_mouse_lighting(&save.request, Some(error), cx)
                    });
                }
            }
        }
    }
}
