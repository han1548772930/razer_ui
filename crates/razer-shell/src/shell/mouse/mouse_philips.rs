//! Source configuration storage precedes PID 70's hardware apply task.
use super::*;
use razer_device::mouse_philips::{ConfigChange, Setting};
use razer_pages::features::mouse_products::MouseCalibrationRequested;
use serde_json::Value;
enum Mutation {
    Configuration(ConfigChange),
    Measurement(razer_device::mouse_philips::MeasurementSave),
}
pub(super) struct PhilipsSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: MouseCalibrationRequested,
    mutation: Mutation,
    setting: Option<Setting>,
    projection: Option<Value>,
    preparation_error: Option<String>,
}
impl PhilipsSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}
impl AppShell {
    pub(super) fn persist_mouse_philips(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCalibrationRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace) {
            request.cancel();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_philips_local_save(&request, cx);
                workspace.finish_mouse_calibration(&request, None, cx)
            });
            return;
        }
        // The page accepted this local mutation before any hardware task.
        // Its submission record also deduplicates a delayed event after the
        // exit drain has already queued the same save.
        if !workspace.update(cx, |workspace, cx| {
            workspace.begin_mouse_philips_local_save(&request, cx)
        }) {
            return;
        }
        if let Some(error) = self.storage_error.clone() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_philips_local_save(&request, cx);
                workspace.finish_mouse_calibration(&request, Some(error), cx)
            });
            self.cancel_pending_exit(cx);
            return;
        }
        let mutation = if let Some(measurement) = request.measurement_save().cloned() {
            Mutation::Measurement(measurement)
        } else if let Some(configuration) = request.philips().cloned() {
            Mutation::Configuration(configuration)
        } else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_philips_local_save(&request, cx);
                workspace.finish_mouse_calibration(
                    &request,
                    Some("Philips typed configuration is absent".into()),
                    cx,
                )
            });
            return;
        };
        let identity = workspace.read(cx).identity(cx);
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
            Some(ProductSave::MousePhilips(PhilipsSave {
                workspace,
                identity,
                request,
                mutation,
                setting: None,
                projection: None,
                preparation_error: None,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_mouse_philips_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MousePhilips(philips)) = &mut save.product else {
            return Ok(());
        };
        anyhow::ensure!(
            philips
                .workspace
                .read(cx)
                .mouse_philips_local_save_matches(&philips.request, cx),
            "Philips calibration owner changed before local save"
        );
        let mut found = false;
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| {
                let mut device = workspace.read(cx).saved_snapshot(cx);
                if workspace == &philips.workspace
                    && workspace.read(cx).identity(cx) == philips.identity
                {
                    found = true;
                    // Initial migration may have measured data only in the current
                    // in-memory device owner; copy it without overriding saved data.
                    if device
                        .source_device_settings
                        .as_ref()
                        .and_then(|root| root.get("calibration"))
                        .is_none()
                    {
                        if let Some(calibration) = workspace
                            .read(cx)
                            .device(cx)
                            .source_device_settings
                            .as_ref()
                            .and_then(|root| root.get("calibration"))
                        {
                            razer_device::mouse_philips::capture_projection(
                                &mut device.source_device_settings,
                                calibration,
                            )?;
                        }
                    }
                    match &philips.mutation {
                        Mutation::Configuration(configuration) => {
                            configuration
                                .merge_local_settings(&mut device.source_device_settings)?;
                            match configuration.setting(device.source_device_settings.as_ref()) {
                                Ok(setting) => {
                                    philips.setting = Some(setting);
                                    philips.preparation_error = None;
                                }
                                Err(error) => {
                                    philips.setting = None;
                                    philips.preparation_error = Some(format!("{error:#}"));
                                }
                            }
                        }
                        Mutation::Measurement(measurement) => {
                            let prepared =
                                measurement.prepare(device.source_device_settings.as_ref())?;
                            philips.setting = Some(prepared.setting());
                            philips.projection = Some(prepared.projection().clone());
                            device.source_device_settings = Some(prepared.root().clone());
                            philips.preparation_error = None;
                        }
                    }
                    for profile in &mut device.profiles {
                        if let Some(settings) = profile
                            .source_settings
                            .as_mut()
                            .and_then(Value::as_object_mut)
                        {
                            settings.remove("calibration");
                        }
                    }
                }
                Ok(device)
            })
            .collect::<anyhow::Result<_>>()?;
        anyhow::ensure!(found, "Philips calibration device owner is absent");
        Ok(())
    }
    pub(super) fn finish_mouse_philips_save(
        &mut self,
        save: PhilipsSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        save.workspace.update(cx, |workspace, cx| {
            workspace.finish_mouse_philips_local_save(&save.request, cx)
        });
        let current = self.devices.contains(&save.workspace)
            && !self.close_requested
            && save.workspace.read(cx).identity(cx) == save.identity
            && save
                .workspace
                .read(cx)
                .mouse_calibration_request_matches(&save.request, cx);
        if succeeded && current {
            if let Some(projection) = save.projection {
                save.workspace.update(cx, |workspace, cx| {
                    workspace.publish_mouse_philips_measurement_save(&save.request, projection, cx)
                });
            }
            if let Some(setting) = save.setting {
                self.submit_mouse_philips(save.workspace, save.request, setting, cx);
                return;
            }
        }
        let error =
            if !succeeded {
                Some("Philips configuration local write failed; editor values are retained".into())
            } else if current {
                Some(save.preparation_error.unwrap_or_else(|| {
                    "Philips configuration has no prepared hardware setting".into()
                }))
            } else {
                None
            };
        save.workspace.update(cx, |workspace, cx| {
            workspace.finish_mouse_calibration(&save.request, error, cx)
        });
    }
}
