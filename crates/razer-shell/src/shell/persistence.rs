//! Workspace composition and the single product follow-up of a local save.
use super::*;
pub(super) struct PreparedSave {
    pub(super) window: AnyWindowHandle,
    pub(super) file: store::WorkspaceFile,
    pub(super) owners: BTreeMap<String, SavedRevisionOwner>,
    pub(super) product: Option<ProductSave>,
    device_draft: Option<store::DeviceSaveDraft>,
}
pub(super) struct SavedRevisionOwner {
    workspace: WeakEntity<ProductWorkspace>,
    connection_revision: u64,
}
impl SavedRevisionOwner {
    fn current(
        &self,
        identity: &str,
        devices: &[Entity<ProductWorkspace>],
        cx: &App,
    ) -> Option<Entity<ProductWorkspace>> {
        let owner = self.workspace.upgrade()?;
        (devices.contains(&owner)
            && owner.read(cx).identity(cx) == identity
            && owner.read(cx).connection_revision() == self.connection_revision)
            .then_some(owner)
    }
}
pub(super) enum ProductSave {
    Leviathan(leviathan::LeviathanSave),
    MousePolling(MousePollingSave),
    MouseInGamePolling(mouse_polling_write::InGameSave),
    MouseDpiStages(MouseDpiStagesSave),
    MouseLegacyDpi(MouseLegacyDpiSave),
    MouseCurrentDpi(MouseCurrentDpiSave),
    MouseDynamic(mouse_dynamic::LocalSave),
    MouseRotation(mouse_rotation::LocalSave),
    ProductLighting(product_lighting::ProductLightingSave),
    MouseCurrentPower(MouseCurrentPowerSave),
    MouseTracking(mouse_tracking::TrackingSave),
    MouseMatcher(mouse_matcher::LocalSave),
    MousePhilips(mouse_philips::PhilipsSave),
    ControllerTrigger(ControllerTriggerSave),
    ControllerLighting(ControllerLightingSave),
    ControllerChroma(ControllerChromaSave),
    SeirenEq(seiren_eq::SeirenEqSave),
    MouseProfile(MouseLegacyProfileSave),
    KeyboardActuation(keyboard_actuation_write::KeyboardActuationSave),
    HanboSettings(hanbo_cooler::LocalSave),
    HanboProfile(hanbo_cooler::ProfileSave),
    HelpHardwareReset(receiver_reset::hardware::LocalSave),
    ReceiverProfile(receiver_reset::local::LocalSave),
    Hyperpolling(hyperpolling_runtime::LocalSave),
    SourceProfileImport(profile_transfer::ImportSave),
}
impl ProductSave {
    fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        match self {
            Self::Leviathan(save) => save.owner(),
            Self::MousePolling(save) => (&save.identity, save.workspace.downgrade()),
            Self::MouseInGamePolling(save) => save.owner(),
            Self::MouseDpiStages(save) => save.owner(),
            Self::MouseLegacyDpi(save) => (&save.identity, save.workspace.downgrade()),
            Self::MouseCurrentDpi(save) => save.owner(),
            Self::MouseDynamic(save) => save.owner(),
            Self::MouseRotation(save) => save.owner(),
            Self::ProductLighting(save) => save.owner(),
            Self::MouseCurrentPower(save) => save.owner(),
            Self::MouseTracking(save) => save.owner(),
            Self::MouseMatcher(save) => save.owner(),
            Self::MousePhilips(save) => save.owner(),
            Self::ControllerTrigger(save) => save.owner(),
            Self::ControllerLighting(save) => save.owner(),
            Self::ControllerChroma(save) => save.owner(),
            Self::SeirenEq(save) => save.owner(),
            Self::MouseProfile(save) => (&save.identity, save.workspace.downgrade()),
            Self::KeyboardActuation(save) => save.owner(),
            Self::HanboSettings(save) => save.owner(),
            Self::HanboProfile(save) => save.owner(),
            Self::HelpHardwareReset(save) => save.owner(),
            Self::ReceiverProfile(save) => save.owner(),
            Self::Hyperpolling(save) => save.owner(),
            Self::SourceProfileImport(save) => save.owner(),
        }
    }
}
impl AppShell {
    /// Shared workspace fields are composed once for every product and save path.
    /// The caller supplies device drafts or saved revisions according to its operation.
    pub(super) fn workspace_file(&self, devices: Vec<Device>, cx: &App) -> store::WorkspaceFile {
        store::WorkspaceFile::new(devices, self.tracking_intro_seen)
            .with_shortcuts(self.shortcuts.read(cx).saved_snapshot())
            .with_retired_shortcuts(self.retired_shortcuts.clone())
            .with_preferences(self.settings.read(cx).snapshot())
            .with_custom_colors(cx.global::<CustomColors>().colors())
            .with_host_tab_order(self.host_tabs.order())
            .with_dashboard(self.dashboard_state.read(cx).snapshot())
            .with_module_services(self.module_catalog.read(cx).service_snapshot())
            .with_macros(self.macro_library.read(cx).snapshot())
    }
}

impl AppShell {
    pub(super) fn finish_local_preference_save(
        &mut self,
        label: &str,
        error: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if let Some(error) = error {
            self.status = format!("{label}本地保存失败：{error}");
            self.cancel_pending_exit(cx);
        } else {
            if self.status == "正在重试本地保存…"
                && self.ui_preferences.read(cx).save_error().is_none()
                && self.chroma_settings.read(cx).save_error().is_none()
                && !self.ui_preferences.read(cx).is_pending()
                && !self.chroma_settings.read(cx).is_pending()
            {
                self.status = "本次本地保存已完成。".into();
            }
            if self.close_requested && self.save_task.is_none() {
                self.continue_save_queue(true, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn local_preference_exit_failed(&mut self, cx: &mut Context<Self>) -> bool {
        let error = self.ui_preferences.read(cx).save_error().map(str::to_owned);
        if let Some(error) = error {
            self.finish_local_preference_save("界面偏好", Some(&error), cx);
            return true;
        }
        let error = self
            .chroma_settings
            .read(cx)
            .save_error()
            .map(str::to_owned);
        if let Some(error) = error {
            self.finish_local_preference_save("Chroma 设置", Some(&error), cx);
            return true;
        }
        false
    }

    pub(super) fn retry_local_preferences(&mut self, cx: &mut Context<Self>) {
        if self.close_requested {
            return;
        }
        let mut requested = self
            .ui_preferences
            .update(cx, |owner, cx| owner.retry_failed(cx))
            > 0;
        if self.chroma_settings.read(cx).save_error().is_some() {
            match self.chroma_settings.update(cx, |owner, cx| owner.retry(cx)) {
                Ok(()) => requested = true,
                Err(error) => {
                    self.finish_local_preference_save("Chroma 设置", Some(&error), cx);
                    return;
                }
            }
        }
        if requested {
            self.status = "正在重试本地保存…".into();
        }
        cx.notify();
    }

    pub(super) fn cancel_pending_exit(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.close_requested) {
            return;
        }
        razer_state::ui_preferences::cancel_exit(cx);
        razer_state::chroma_settings::cancel_exit(cx);
        self.help_system_info.resume_after_cancelled_exit();
        self.runtime
            .update(cx, |runtime, cx| runtime.cancel_exit(cx));
        self.background_exit_task = None;
        self.sync_hanbo_cooler_connections(cx);
        self.resume_global_shortcuts(cx);
        let handle = self.main_window;
        let shell = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = shell.update(cx, |shell, cx| {
                    if shell.close_requested {
                        return;
                    }
                    shell.cancel_keyboard_game_exit(cx);
                    shell.cancel_keyboard_latest_exit(window, cx);
                    for workspace in &shell.devices {
                        let active = matches!(&shell.location, Location::Device(key)
                            if workspace.read(cx).identity(cx) == *key);
                        workspace.update(cx, |workspace, cx| {
                            workspace.cancel_exit(active, window, cx)
                        });
                    }
                    #[cfg(target_os = "windows")]
                    razer_tray::native::show(window);
                });
            });
        });
        cx.notify();
    }

    pub(super) fn prepare_save(
        &self,
        window: AnyWindowHandle,
        file: store::WorkspaceFile,
        product: Option<ProductSave>,
        cx: &App,
    ) -> PreparedSave {
        let device_draft = match &product {
            None => Some(store::DeviceSaveDraft::new(
                self.devices
                    .iter()
                    .map(|owner| owner.read(cx).saved_snapshot(cx))
                    .collect(),
                file.devices.clone(),
            )),
            Some(ProductSave::MouseProfile(profile)) if profile.scroll.is_none() => {
                let baseline = self
                    .devices
                    .iter()
                    .map(|owner| owner.read(cx).saved_snapshot(cx))
                    .collect::<Vec<_>>();
                let mut proposed = baseline.clone();
                if let Some(target) = proposed
                    .iter_mut()
                    .find(|device| device.workspace_identity() == profile.identity)
                {
                    *target = profile.device.clone();
                }
                Some(store::DeviceSaveDraft::new(baseline, proposed))
            }
            _ => None,
        };
        PreparedSave {
            window,
            file,
            owners: self.capture_save_owners(cx),
            product,
            device_draft,
        }
    }

    pub(super) fn prepared_save_owners_current(&self, save: &PreparedSave, cx: &App) -> bool {
        let Some(product) = &save.product else {
            return save.owners.iter().all(|(identity, captured)| {
                if !save
                    .device_draft
                    .as_ref()
                    .is_some_and(|draft| draft.has_changes(identity))
                {
                    return true;
                }
                captured.current(identity, &self.devices, cx).is_some()
            });
        };
        self.product_save_owner_current(&save.owners, product, cx)
    }

    pub(super) fn product_save_owner_current(
        &self,
        owners: &BTreeMap<String, SavedRevisionOwner>,
        product: &ProductSave,
        cx: &App,
    ) -> bool {
        let (identity, bound) = product.owner();
        let Some(captured) = owners
            .get(identity)
            .and_then(|captured| captured.current(identity, &self.devices, cx))
        else {
            return false;
        };
        bound.upgrade().is_some_and(|owner| owner == captured)
    }

    pub(super) fn rebase_queued_device_draft(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> Result<(), String> {
        let latest = self
            .devices
            .iter()
            .map(|owner| owner.read(cx).saved_snapshot(cx))
            .collect::<Vec<_>>();
        save.file.devices = match &save.device_draft {
            Some(draft) => draft.rebase(&latest)?,
            None => latest,
        };
        Ok(())
    }

    pub(super) fn cancel_prepared_save(&mut self, save: PreparedSave, cx: &mut Context<Self>) {
        self.finish_product_save(save.product, false, save.window, cx);
    }

    pub(super) fn finish_product_save(
        &mut self,
        product: Option<ProductSave>,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(product) = product else {
            return;
        };
        match product {
            ProductSave::SourceProfileImport(save) => {
                self.finish_source_profile_import(save, succeeded, window, cx)
            }
            ProductSave::Leviathan(sound) => self.finish_leviathan_save(sound, succeeded, cx),
            ProductSave::MouseInGamePolling(_) => {
                if succeeded {
                    self.status = "游戏内回报率已保存到本机。".into();
                }
            }
            ProductSave::MousePolling(polling) => {
                if succeeded {
                    let owner = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let workspace = polling.workspace.clone();
                        let launched = window.update(cx, |_, window, cx| {
                            owner.update(cx, |this, cx| {
                                this.write_mouse_polling_device(
                                    polling.workspace,
                                    polling.scope,
                                    polling.field,
                                    polling.hz,
                                    window,
                                    cx,
                                );
                            })
                        });
                        if !matches!(launched, Ok(Ok(()))) {
                            workspace.update(cx, |workspace, cx| {
                                workspace.finish_mouse_polling(
                                    polling.scope,
                                    polling.field,
                                    polling.hz,
                                    None,
                                    cx,
                                );
                            });
                        }
                    });
                } else {
                    polling.workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_polling(
                            polling.scope,
                            polling.field,
                            polling.hz,
                            None,
                            cx,
                        );
                    });
                }
            }
            ProductSave::MouseLegacyDpi(dpi) => {
                if succeeded {
                    let owner = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let workspace = dpi.workspace.clone();
                        let request = dpi.request.clone();
                        let launched = window.update(cx, |_, window, cx| {
                            owner.update(cx, |this, cx| {
                                this.submit_legacy_dpi_device(
                                    dpi.workspace,
                                    dpi.request,
                                    window,
                                    cx,
                                );
                            })
                        });
                        if !matches!(launched, Ok(Ok(()))) {
                            workspace.update(cx, |workspace, cx| {
                                workspace.finish_legacy_dpi(&request, cx)
                            });
                        }
                    });
                } else {
                    dpi.workspace.update(cx, |workspace, cx| {
                        workspace.finish_legacy_dpi(&dpi.request, cx)
                    });
                }
            }
            ProductSave::MouseDpiStages(dpi) => {
                self.finish_mouse_dpi_stages_save(dpi, succeeded, window, cx)
            }
            ProductSave::MouseCurrentDpi(dpi) => {
                self.finish_current_dpi_save(dpi, succeeded, window, cx)
            }
            ProductSave::MouseDynamic(save) => {
                self.finish_mouse_dynamic_save(save, succeeded, window, cx)
            }
            ProductSave::MouseRotation(save) => {
                self.finish_mouse_rotation_save(save, succeeded, cx)
            }
            ProductSave::ProductLighting(save) => {
                self.finish_product_lighting_save(save, succeeded, window, cx)
            }
            ProductSave::MouseCurrentPower(power) => {
                self.finish_current_power_save(power, succeeded, window, cx)
            }
            ProductSave::MousePhilips(philips) => {
                self.finish_mouse_philips_save(philips, succeeded, cx)
            }
            ProductSave::MouseTracking(tracking) => {
                self.finish_mouse_tracking_save(tracking, succeeded, cx)
            }
            ProductSave::MouseMatcher(save) => self.finish_mouse_matcher_save(save, succeeded, cx),
            ProductSave::ControllerTrigger(trigger) => {
                self.finish_controller_trigger_save(trigger, succeeded, window, cx)
            }
            ProductSave::ControllerLighting(lighting) => {
                self.finish_controller_lighting_save(lighting, succeeded, window, cx)
            }
            ProductSave::ControllerChroma(chroma) => {
                self.finish_controller_chroma_save(chroma, succeeded, window, cx)
            }
            ProductSave::SeirenEq(eq) => self.finish_seiren_eq_save(eq, succeeded, window, cx),
            ProductSave::MouseProfile(profile) => {
                if profile.scroll.is_some() {
                    self.finish_mouse_scroll_save(profile, succeeded, window, cx);
                } else if succeeded
                    && !self.close_requested
                    && profile.apply
                    && self.devices.contains(&profile.workspace)
                    && profile.workspace.read(cx).identity(cx) == profile.identity
                    && profile.scope.is_some()
                    && profile.workspace.read(cx).mouse_polling_scope(cx) == profile.scope
                {
                    if profile.workspace.read(cx).device(cx).product_id == 182 {
                        self.initialize_current_mouse_profiles(cx);
                        profile.workspace.update(cx, |workspace, cx| {
                            workspace.apply_legacy_performance_profile(
                                profile.scope.expect("checked mouse scope"),
                                cx,
                            );
                        });
                    } else {
                        self.initialize_mouse_scroll_profiles(cx);
                        profile.workspace.update(cx, |workspace, cx| {
                            workspace.apply_legacy_performance_profile(
                                profile.scope.expect("checked mouse scope"),
                                cx,
                            );
                        });
                    }
                }
            }
            ProductSave::KeyboardActuation(control) => {
                self.finish_keyboard_actuation_save(control, succeeded, window, cx)
            }
            ProductSave::HanboSettings(save) => {
                self.finish_hanbo_settings_save(save, succeeded, cx)
            }
            ProductSave::HanboProfile(save) => self.finish_hanbo_profile_save(save, succeeded, cx),
            ProductSave::HelpHardwareReset(save) => {
                self.finish_help_hardware_reset_save(save, succeeded, window, cx)
            }
            ProductSave::ReceiverProfile(save) => {
                self.finish_receiver_profile_save(save, succeeded, window, cx)
            }
            ProductSave::Hyperpolling(save) => {
                self.finish_hyperpolling_save(save, succeeded, window, cx)
            }
        }
    }

    pub(super) fn capture_save_owners(&self, cx: &App) -> BTreeMap<String, SavedRevisionOwner> {
        self.devices
            .iter()
            .map(|owner| {
                (
                    owner.read(cx).identity(cx),
                    SavedRevisionOwner {
                        workspace: owner.downgrade(),
                        connection_revision: owner.read(cx).connection_revision(),
                    },
                )
            })
            .collect()
    }
    pub(super) fn saved_revision_owner(
        &self,
        owners: &BTreeMap<String, SavedRevisionOwner>,
        snapshot: &Device,
        cx: &App,
    ) -> Option<Entity<ProductWorkspace>> {
        let identity = snapshot.workspace_identity();
        owners.get(&identity)?.current(&identity, &self.devices, cx)
    }
}
