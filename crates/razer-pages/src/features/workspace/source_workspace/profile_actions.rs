//! Local mutations behind independently audited product profile commands.
use super::profile_menu::ProfileAction;
use super::profile_transfer::{
    ProfileTransferMode, SourceProfileTransfer, SourceProfileTransferEvent,
};
use super::*;

pub(super) struct PendingProfileExport {
    request: razer_storage::file_transfer::Transfer,
    _task: Task<()>,
}
impl Drop for PendingProfileExport {
    fn drop(&mut self) {
        self.request.cancel();
    }
}

#[derive(Clone)]
pub(super) struct ProfileConfirmation {
    pub(super) action: ProfileAction,
    pub(super) id: String,
    pub(super) bindings_only: bool,
}

impl SourceProductWorkspace {
    fn import_source_profiles(
        &mut self,
        rows: &[Value],
        macros: &[Value],
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.device.product_id == 1398 {
            // Current1398 buttonList has no disabled hypershift rows and its
            // product module exports no imported-mapping sanitizer. Selected
            // rows therefore reach MW56144 unchanged through either caller.
            return self.request_global_profile_import(rows, cx);
        }
        if !matches!(self.device.product_id, 70 | 182 | 756) {
            return Err("This product requires its own source profile import processor".into());
        }
        if self.exiting || self.profile_import.is_some() {
            return Err("The profile owner is closing or another import is pending".into());
        }
        let ids = self
            .device
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();
        let existing = razer_model::profile_transfer::source_profile_payloads(&self.device, &ids)?;
        let active = self
            .device
            .active_profile_obj()
            .map(|profile| profile.guid.as_str())
            .ok_or("The selected profile is absent")?;
        let (candidate, imported_macros) = match self.device.product_id {
            70 | 182 => {
                let prepared = razer_device::profile_transfer::prepare_mouse_import(
                    rows,
                    macros,
                    razer_device::profile_transfer::MouseImportContext::new(
                        self.device.product_id,
                        &existing,
                        &self.local_mapping_macros,
                        self.device.source_device_settings.as_ref(),
                    )
                    .with_selected_profile(existing.iter().find(|profile| {
                        profile.get("guid").and_then(Value::as_str) == Some(active)
                    })),
                    (0..8192).map(|_| uuid::Uuid::new_v4().to_string()),
                )
                .map_err(|error| error.to_string())?;
                (
                    self.prepare_imported_source_collection(
                        prepared.profiles,
                        prepared.device_settings,
                    )?,
                    prepared.imported_macros,
                )
            }
            756 => {
                let (profiles, macros) = razer_device::profile_transfer::prepare_import_756(
                    rows,
                    macros,
                    &existing,
                    &self.local_mapping_macros,
                    active,
                    self.device.layout_id,
                    || uuid::Uuid::new_v4().to_string(),
                )
                .map_err(|error| error.to_string())?;
                (self.prepare_imported_profile_candidate(&profiles)?, macros)
            }
            _ => unreachable!("validated profile processor"),
        };
        let request =
            razer_model::profile_transfer::SourceProfileImport::new(&self.device, candidate)?
                .with_source_macros(&self.local_mapping_macros, imported_macros);
        self.profile_import = Some(request.clone());
        cx.emit(WorkspaceEvent::SourceProfileImportRequested(request));
        cx.notify();
        Ok(())
    }

    fn prepare_imported_profile_candidate(&self, profiles: &[Value]) -> Result<Device, String> {
        use razer_device::{
            keyboard_latest_mapping as mapping, keyboard_latest_restore as restore,
        };
        let ids = self
            .device
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();
        let mut combined =
            razer_model::profile_transfer::source_profile_payloads(&self.device, &ids)?;
        combined.extend_from_slice(profiles);
        let active = self
            .device
            .active_profile_obj()
            .map(|profile| profile.guid.as_str())
            .ok_or("The selected profile is absent")?;
        let previous = self
            .device
            .source_device_settings
            .as_ref()
            .and_then(|settings| settings.get(restore::STORAGE_FIELD));
        let compiled = mapping::prepare_document_with_device_settings(
            &combined,
            active,
            self.device.layout_id,
            previous,
            self.device.source_device_settings.as_ref(),
            (0..8192).map(|_| uuid::Uuid::new_v4().to_string()),
        )
        .map_err(|e| e.to_string())?;
        restore::ProfileCollection::from_document(&compiled).map_err(|error| error.to_string())?;
        let mut candidate = self.device.clone();
        for profile in profiles {
            let guid = profile["guid"]
                .as_str()
                .ok_or("Imported profile GUID is absent")?
                .to_owned();
            let source_settings = compiled["profiles"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["guid"] == guid))
                .cloned()
                .ok_or("The compiled imported profile is absent")?;
            candidate.profiles.push(Profile {
                id: guid.clone(),
                guid: guid.clone(),
                name: profile["name"]
                    .as_str()
                    .ok_or("Imported profile name is absent")?
                    .into(),
                dpi_stages: None,
                settings: None,
                source_settings: Some(source_settings),
            });
        }
        let settings = candidate
            .source_device_settings
            .get_or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or("Current profile device settings are not an object")?;
        settings.insert(restore::STORAGE_FIELD.into(), compiled);
        Ok(candidate)
    }

    fn prepare_imported_source_collection(
        &self,
        profiles: Vec<Value>,
        device_settings: Value,
    ) -> Result<Device, String> {
        let mut candidate = self.device.clone();
        for source in profiles.into_iter().skip(candidate.profiles.len()) {
            let guid = source
                .get("guid")
                .and_then(Value::as_str)
                .ok_or("Imported profile GUID is absent")?
                .to_owned();
            let name = source
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Imported profile name is absent")?
                .to_owned();
            candidate.profiles.push(Profile {
                id: guid.clone(),
                guid,
                name,
                dpi_stages: None,
                settings: None,
                source_settings: Some(source),
            });
        }
        candidate.source_device_settings = Some(device_settings);
        Ok(candidate)
    }

    pub fn request_global_profile_import(
        &mut self,
        rows: &[Value],
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.exiting || self.profile_import.is_some() {
            return Err("The profile owner is closing or another import is pending".into());
        }
        let ids = self
            .device
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();
        let existing = razer_model::profile_transfer::source_profile_payloads(&self.device, &ids)?;
        let candidate = match self.device.product_id {
            70 | 182 => {
                let selected_profile = self.device.active_profile_obj().and_then(|active| {
                    existing.iter().find(|profile| {
                        profile.get("guid").and_then(Value::as_str) == Some(active.guid.as_str())
                    })
                });
                let prepared = razer_device::profile_transfer::prepare_global_mouse_import(
                    rows,
                    razer_device::profile_transfer::MouseImportContext::new(
                        self.device.product_id,
                        &existing,
                        &self.local_mapping_macros,
                        self.device.source_device_settings.as_ref(),
                    )
                    .with_selected_profile(selected_profile),
                    (0..8192).map(|_| uuid::Uuid::new_v4().to_string()),
                )
                .map_err(|error| error.to_string())?;
                self.prepare_imported_source_collection(
                    prepared.profiles,
                    prepared.device_settings,
                )?
            }
            756 => {
                let (profiles, macros) = razer_device::profile_transfer::prepare_global_import_756(
                    rows,
                    &existing,
                    &self.local_mapping_macros,
                    self.device.layout_id,
                    || uuid::Uuid::new_v4().to_string(),
                )
                .map_err(|error| error.to_string())?;
                if !macros.is_empty() {
                    return Err("The global profile command unexpectedly imported macros".into());
                }
                self.prepare_imported_profile_candidate(&profiles)?
            }
            1398 => {
                let prepared = razer_device::profile_transfer::prepare_global_import_1398(
                    rows,
                    &existing,
                    self.device.source_device_settings.as_ref(),
                    || uuid::Uuid::new_v4().to_string(),
                )
                .map_err(|error| error.to_string())?;
                self.prepare_imported_source_collection(
                    prepared.profiles,
                    prepared.device_settings,
                )?
            }
            _ => {
                return Err(
                    "The current product requires its source profile import processor".into(),
                );
            }
        };
        let mut request =
            razer_model::profile_transfer::SourceProfileImport::new(&self.device, candidate)?;
        if matches!(self.device.product_id, 70 | 182 | 756) {
            request = request.with_source_macros(&self.local_mapping_macros, Vec::new());
        }
        self.profile_import = Some(request.clone());
        cx.emit(WorkspaceEvent::SourceProfileImportRequested(request));
        cx.notify();
        Ok(())
    }

    pub fn finish_source_profile_import(
        &mut self,
        request: &razer_model::profile_transfer::SourceProfileImport,
        receipt: Result<Device, String>,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        if !self
            .profile_import
            .as_ref()
            .is_some_and(|current| current.is_same(request))
        {
            return Ok(false);
        }
        self.profile_import = None;
        match receipt {
            Ok(merged) => {
                self.saved = request.candidate().clone();
                self.device = merged;
                if self.device.product_id == 182
                    && self.device.source_device_settings == self.saved.source_device_settings
                    && self.device.profiles == self.saved.profiles
                {
                    self.mouse_profile_cache_changed = false;
                }
                cx.emit(WorkspaceEvent::Changed);
            }
            Err(error) => {
                cx.notify();
                return Err(error);
            }
        }
        cx.notify();
        Ok(true)
    }

    pub fn source_profile_macros(&self) -> &[Value] {
        &self.local_mapping_macros
    }

    pub fn export_global_profiles(
        &mut self,
        selection: &[razer_model::profile_transfer::SourceExportSelection],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let product = razer_device::profile_transfer::product_target(self.device.product_id)
            .ok_or("The product profile file identity is absent")?;
        let (profiles, macros) = razer_model::profile_transfer::source_export_payloads(
            &self.device,
            selection,
            &self.local_mapping_macros,
            product.export_rule,
        )?;
        self.export_source_profiles(&profiles, &macros, window, cx);
        Ok(())
    }

    fn export_source_profiles(
        &mut self,
        profiles: &[Value],
        macros: &[Value],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use razer_storage::file_transfer::Transfer;
        if self.exiting {
            return;
        }
        let Some(product) = razer_device::profile_transfer::product_target(self.device.product_id)
        else {
            window.push_notification("The product profile file identity is absent", cx);
            return;
        };
        let mut target = serde_json::to_value(product.export_target).expect("typed profile target");
        let export_rule = product.export_rule;
        target["deviceName"] = product
            .device_name
            .unwrap_or_else(|| self.device.product_name.get(&razer_i18n::locale()))
            .into();
        let profiles = profiles.to_vec();
        let macros = macros.to_vec();
        let context = razer_model::profile_transfer::ProfileContext::from_device(&self.device);
        let request = Transfer::new();
        let current = request.clone();
        let operation = match request.accept() {
            Ok(operation) => operation,
            Err(error) => {
                window.push_notification(error, cx);
                return;
            }
        };
        let directory = std::path::PathBuf::from(".");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let filename = format!("profiles - {timestamp}.synapse4");
        let picker = cx.prompt_for_new_path(&directory, Some(&filename));
        let task = cx.spawn_in(window, async move |owner, cx| {
            let path = match picker.await {
                Ok(Ok(path)) => Ok(path),
                _ => Err("Could not open the profile save dialog".to_owned()),
            };
            let prepared = owner.update_in(cx, |this, _, _| {
                if this.exiting
                    || !context.matches(&this.device)
                    || !this
                        .profile_export_task
                        .as_ref()
                        .is_some_and(|pending| pending.request.is_same(&current))
                {
                    current.cancel();
                    return None;
                }
                Some(path)
            });
            let result = match prepared {
                Ok(Some(Ok(Some(path)))) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            operation.check()?;
                            let bytes = razer_device::profile_transfer::encode_with_rule(
                                &target,
                                &profiles,
                                &macros,
                                export_rule,
                            )
                            .map_err(|error| error.to_string())?;
                            razer_storage::profile_transfer::write_source(&operation, &path, &bytes)
                        })
                        .await,
                ),
                Ok(Some(Err(error))) => Some(Err(error)),
                Ok(Some(Ok(None))) => None,
                _ => return,
            };
            let _ = owner.update_in(cx, |this, window, cx| {
                if !this
                    .profile_export_task
                    .as_ref()
                    .is_some_and(|pending| pending.request.is_same(&current))
                {
                    return;
                }
                this.profile_export_task = None;
                if !this.exiting && context.matches(&this.device) {
                    if let Some(Err(error)) = result {
                        window.push_notification(error, cx);
                    }
                }
                cx.notify();
            });
        });
        self.profile_export_task = Some(PendingProfileExport {
            request,
            _task: task,
        });
    }

    fn finish_profile_transfer_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(request) = self.profile_transfer_request.take() {
            request.cancel();
        }
        if self.profile_transfer.take().is_some() {
            self.profile.update(cx, |state, cx| state.focus(window, cx));
        }
        self.profile_transfer_subscription = None;
    }

    pub(super) fn cancel_profile_transfer_requests(&mut self) {
        self.profile_transfer_subscription = None;
        self.profile_transfer = None;
        if let Some(request) = self.profile_transfer_request.take() {
            request.cancel();
        }
        self.profile_export_task = None;
    }

    pub(in crate::features) fn refresh_profile_choices(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.profile.update(cx, |state, cx| {
            state.set_items(
                self.device
                    .profiles
                    .iter()
                    .map(|profile| Choice::new(&profile.id, &profile.name))
                    .collect(),
                window,
                cx,
            );
            state.set_selected_value(&self.device.active_profile, window, cx);
        });
        self.sync_keyboard_mapping_profiles(window, cx);
    }

    pub fn dismiss_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.profile_rename = None;
        self.profile_confirmation = None;
        self.profile_confirm_task = None;
        self.profile_export_task = None;
        let transfer_open = self.profile_transfer.is_some();
        self.finish_profile_transfer_dialog(window, cx);
        let linked_open = self.profile_linked_games.take().is_some();
        if transfer_open || linked_open {
            self.profile.update(cx, |state, cx| state.focus(window, cx));
        }
        self.profile_transfer_subscription = None;
        self.profile_linked_games_subscription = None;
        if let Some(popup) = self.profile_menu.read(cx).delegate().popup.clone() {
            // Popover invokes on_open_change synchronously. Its callback updates
            // this workspace, so dismiss only after our entity borrow is released.
            window.defer(cx, move |window, cx| {
                let _ = popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            });
        }
        cx.notify();
    }

    pub(super) fn finish_profile_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.profile_rename.take() else {
            return;
        };
        let name = self.profile_name.read(cx).value().to_string();
        self.change_profile_metadata(
            &id,
            super::super::product_workspace::ProfileMetadata::Rename(name),
            window,
            cx,
        );
        cx.notify();
    }

    fn open_linked_games(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use super::profile_linked_games::{ProfileLinkedGames, ProfileLinkedGamesEvent};
        let profile_id = self.device.active_profile.clone();
        let dialog = cx.new(|cx| ProfileLinkedGames::new(self.device.clone(), window, cx));
        self.profile_linked_games_subscription = Some(cx.subscribe_in(
            &dialog,
            window,
            move |this, _, event, window, cx| match event {
                ProfileLinkedGamesEvent::Close => this.dismiss_profile_dialog(window, cx),
                ProfileLinkedGamesEvent::Link {
                    name,
                    executable,
                    linked,
                } => {
                    if this.device.active_profile != profile_id {
                        return;
                    }
                    this.change_profile_metadata(
                        &profile_id,
                        super::super::product_workspace::ProfileMetadata::LinkGame {
                            name: name.clone(),
                            executable: executable.clone(),
                            linked: *linked,
                        },
                        window,
                        cx,
                    );
                }
            },
        ));
        self.profile_linked_games = Some(dialog);
        // The source dialog takes focus when it opens, the same way the
        // transfer dialog above does.
        if let Some(dialog) = &self.profile_linked_games {
            dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
        }
    }

    fn create_profile(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        let id = if matches!(self.device.product_id, 182 | 756) {
            uuid::Uuid::new_v4().to_string()
        } else {
            (1..)
                .map(|index| format!("local-profile-{index}"))
                .find(|id| {
                    !self
                        .device
                        .profiles
                        .iter()
                        .chain(&self.saved.profiles)
                        .any(|profile| profile.id == *id || profile.guid == *id)
                })
                .expect("unused local profile identity")
        };
        let mut profile = if duplicate {
            let Some(mut profile) = self.device.active_profile_obj().cloned() else {
                return;
            };
            let base = profile
                .name
                .strip_suffix(')')
                .and_then(|name| name.rsplit_once(" ("))
                .filter(|(_, suffix)| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
                .map(|(base, _)| base.trim_end())
                .unwrap_or(&profile.name);
            profile.name = (0..)
                .map(|index| {
                    if index == 0 {
                        base.to_owned()
                    } else {
                        format!("{base} ({index})")
                    }
                })
                .find(|name| {
                    !self
                        .device
                        .profiles
                        .iter()
                        .any(|profile| profile.name == *name)
                })
                .expect("unused copy name");
            // Game associations live in a separate source store keyed by GUID;
            // ON_DUPLICATE_PROFILE copies settings, not those associations.
            if let Some(settings) = &mut profile.settings {
                settings.linked_games.clear();
            }
            profile
        } else {
            // The current helper uses computerName, with an empty-name fallback.
            let computer_name = std::env::var("COMPUTERNAME").unwrap_or_default();
            let base = if self.device.product_id == 182 {
                mouse_profile_source::default_name()
            } else if computer_name.is_empty() {
                "Default".to_owned()
            } else {
                format!("{computer_name}-Default")
            };
            let name = (0..)
                .map(|index| {
                    if index == 0 {
                        base.clone()
                    } else {
                        format!("{base} {index}")
                    }
                })
                .find(|name| {
                    !self
                        .device
                        .profiles
                        .iter()
                        .any(|profile| profile.name == *name)
                })
                .expect("unused default name");
            mouse_profile_source::default_profile(&self.device, name.clone()).unwrap_or_else(|| {
                Profile {
                    id: String::new(),
                    guid: String::new(),
                    name,
                    dpi_stages: None,
                    settings: None,
                    source_settings: (self.device.product_id
                        == razer_device::hanbo_cooler::PRODUCT_ID)
                        .then(razer_device::hanbo_settings::default_profile_settings),
                }
            })
        };
        profile.id = id.clone();
        profile.guid = id.clone();
        if matches!(self.device.product_id, 182 | 756) {
            if let Some(source) = profile
                .source_settings
                .as_mut()
                .and_then(Value::as_object_mut)
            {
                // ey copies the old profile and overlays the new GUID/name.
                source.insert("guid".into(), serde_json::json!(profile.guid));
                source.insert("name".into(), serde_json::json!(profile.name));
            }
        }
        self.device.profiles.push(profile);
        self.refresh_profile_choices(window, cx);
        if self.device.product_id == 756 {
            // Current50574 Jf/ey commit the collection and selected GUID
            // before26568.loadActiveProfile. Its BLE branch only publishes
            // that local selection; it does not call the slot setter in ty.
            self.select_source_profile_inner(&id, true, true, window, cx);
        } else {
            self.select_profile(&id, window, cx);
        }
    }

    pub(super) fn run_profile_action(
        &mut self,
        action: ProfileAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.profile_switch_enabled() {
            return;
        }
        match action {
            ProfileAction::Add => self.create_profile(false, window, cx),
            ProfileAction::Duplicate => self.create_profile(true, window, cx),
            ProfileAction::Rename => {
                let Some(profile) = self.device.active_profile_obj() else {
                    return;
                };
                let name = profile.name.clone();
                self.profile_rename = Some(profile.id.clone());
                self.profile_name.update(cx, |input, cx| {
                    input.set_value(name, window, cx);
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }
            ProfileAction::Delete | ProfileAction::Reset => {
                let Some(profile) = self.device.active_profile_obj() else {
                    return;
                };
                let id = profile.id.clone();
                let delay = profile_menu::spec(self.device.product_id)
                    .and_then(|spec| spec.confirmation(action))
                    .and_then(|spec| spec["delay_ms"].as_u64())
                    .unwrap_or(100);
                self.profile_confirm_task = Some(cx.spawn_in(window, async move |owner, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(delay))
                        .await;
                    let _ = owner.update_in(cx, |this, window, cx| {
                        if this.device.active_profile == id && this.profile_switch_enabled() {
                            this.profile_confirmation = Some(ProfileConfirmation {
                                action,
                                id,
                                bindings_only: false,
                            });
                            this.profile_confirm_focus.focus(window, cx);
                            cx.notify();
                        }
                    });
                }));
            }
            ProfileAction::Import | ProfileAction::Export => {
                let mode = if action == ProfileAction::Import {
                    ProfileTransferMode::Import
                } else {
                    ProfileTransferMode::Export
                };
                let ids = self
                    .device
                    .profiles
                    .iter()
                    .map(|profile| profile.id.clone())
                    .collect::<Vec<_>>();
                let profiles = match razer_model::profile_transfer::source_profile_payloads(
                    &self.device,
                    &ids,
                ) {
                    Ok(profiles) => profiles,
                    Err(error) => {
                        window.push_notification(error, cx);
                        return;
                    }
                };
                let Some(product) =
                    razer_device::profile_transfer::product_target(self.device.product_id)
                else {
                    window.push_notification("The product profile file identity is absent", cx);
                    return;
                };
                let mut target = serde_json::to_value(product.target)
                    .expect("typed profile compatibility target");
                if self.device.product_id == 756 {
                    let presentation =
                        &razer_device::keyboard_latest_mapping::source()["profile_transfer_target"];
                    target["presetProfiles"] = presentation["presetProfiles"].clone();
                    target["deviceName"] = presentation["deviceName"].clone();
                } else {
                    target["deviceName"] =
                        self.device.product_name.get(&razer_i18n::locale()).into();
                }
                self.dismiss_profile_dialog(window, cx);
                let request = razer_storage::file_transfer::Transfer::new();
                let context =
                    razer_model::profile_transfer::ProfileContext::from_device(&self.device);
                let dialog = cx.new(|cx| {
                    SourceProfileTransfer::new(
                        self.device.product_id,
                        mode,
                        target,
                        request.clone(),
                        profiles,
                        self.local_mapping_macros.clone(),
                        self.device
                            .active_profile_obj()
                            .map(|profile| profile.guid.clone())
                            .unwrap_or_default(),
                        window,
                        cx,
                    )
                });
                self.profile_transfer_subscription = Some(cx.subscribe_in(
                    &dialog,
                    window,
                    move |this, source, event: &SourceProfileTransferEvent, window, cx| {
                        if this.exiting
                            || !context.matches(&this.device)
                            || !this
                                .profile_transfer
                                .as_ref()
                                .is_some_and(|current| current.entity_id() == source.entity_id())
                        {
                            return;
                        }
                        match event {
                            SourceProfileTransferEvent::Close => {
                                this.finish_profile_transfer_dialog(window, cx)
                            }
                            SourceProfileTransferEvent::Import { rows, macros } => {
                                if let Err(error) =
                                    this.import_source_profiles(rows, macros, window, cx)
                                {
                                    window.push_notification(error, cx);
                                }
                            }
                            SourceProfileTransferEvent::Export { profiles, macros } => {
                                this.export_source_profiles(profiles, macros, window, cx)
                            }
                        }
                    },
                ));
                dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
                self.profile_transfer = Some(dialog);
                self.profile_transfer_request = Some(request);
            }
            ProfileAction::LinkedGames | ProfileAction::LinkGames => {
                self.open_linked_games(window, cx)
            }
        }
        cx.notify();
    }

    pub(super) fn confirm_profile_action(
        &mut self,
        confirmation: ProfileConfirmation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.device.active_profile != confirmation.id || !self.profile_switch_enabled() {
            return;
        }
        match confirmation.action {
            ProfileAction::Reset if self.device.product_id == 756 => {
                self.reset_latest_keyboard_profile(confirmation.bindings_only, window, cx);
            }
            ProfileAction::Delete if self.device.profiles.len() > 1 => {
                // Current3893 cM.confirmDel / mI.OCz selects the first remaining
                // GUID and sends it as ON_DELETE_PROFILE.newSelectedProfileGUID.
                let successor = (self.device.product_id == razer_device::hanbo_cooler::PRODUCT_ID)
                    .then(|| {
                        self.device
                            .profiles
                            .iter()
                            .find(|profile| profile.id != confirmation.id)
                            .map(|profile| profile.id.clone())
                    })
                    .flatten();
                if let Err(error) = self.change_profile_collection(
                    super::super::product_workspace::ProfileCollectionAction::Delete {
                        id: confirmation.id,
                        successor,
                    },
                    window,
                    cx,
                ) {
                    self.profile_confirmation = None;
                    window.push_notification(error, cx);
                }
            }
            _ => return,
        }
        cx.notify();
    }
}
