//! Local mutations behind independently audited product profile commands.
use super::profile_menu::ProfileAction;
use super::profile_transfer::{
    ProfileTransferMode, SourceProfileTransfer, SourceProfileTransferEvent,
};
use super::*;

#[derive(Clone)]
pub(super) struct ProfileConfirmation {
    pub(super) action: ProfileAction,
    pub(super) id: String,
    pub(super) bindings_only: bool,
}

impl SourceProductWorkspace {
    fn import_source_profiles(&mut self,rows:&[Value],macros:&[Value],window:&mut Window,cx:&mut Context<Self>)->Result<(),String> {
        use razer_device::{keyboard_latest_mapping as mapping,keyboard_latest_restore as restore};
        let existing=self.device.profiles.iter().map(|profile| {
            let mut value=profile.source_settings.clone().unwrap_or(serde_json::json!({}));
            value["guid"]=profile.guid.clone().into();value["name"]=profile.name.clone().into();value
        }).collect::<Vec<_>>();
        let active=self.device.active_profile_obj().map(|profile|profile.guid.as_str()).ok_or("The selected profile is absent")?;
        let (profiles,imported_macros)=razer_device::profile_transfer::prepare_import_756(rows,macros,&existing,
            &self.local_mapping_macros,active,self.device.layout_id,||uuid::Uuid::new_v4().to_string()).map_err(|e|e.to_string())?;
        // Compile a complete candidate before altering the source store. This
        // checks all imported mapping branches and retains the active GUID.
        let mut combined=existing;combined.extend(profiles.clone());
        let previous=self.device.source_device_settings.as_ref().and_then(|settings|settings.get(restore::STORAGE_FIELD));
        let compiled=mapping::prepare_document_with_device_settings(&combined,active,self.device.layout_id,previous,
            self.device.source_device_settings.as_ref(),(0..8192).map(|_|uuid::Uuid::new_v4().to_string())).map_err(|e|e.to_string())?;
        for profile in profiles {
            let guid=profile["guid"].as_str().ok_or("Imported profile GUID is absent")?.to_owned();
            self.device.profiles.push(Profile{id:guid.clone(),guid:guid.clone(),name:profile["name"].as_str()
                .ok_or("Imported profile name is absent")?.into(),dpi_stages:None,settings:None,
                source_settings:compiled["profiles"].as_array().and_then(|rows|rows.iter().find(|row|row["guid"]==guid)).cloned()});
        }
        self.device.source_device_settings.get_or_insert_with(||serde_json::json!({}))[restore::STORAGE_FIELD]=compiled;
        self.local_mapping_macros.extend(imported_macros.clone());
        self.sync_local_mapping_macros(window,cx);
        self.refresh_profile_choices(window,cx);
        if !imported_macros.is_empty(){cx.emit(WorkspaceEvent::ProfileMacrosImported(imported_macros));}
        // MW ay appends and caches; it never invokes loadActiveProfile.
        cx.emit(WorkspaceEvent::Changed);cx.notify();Ok(())
    }

    fn export_source_profiles(&mut self,profiles:&[Value],macros:&[Value],window:&mut Window,cx:&mut Context<Self>) {
        let target=razer_device::keyboard_latest_mapping::source()["profile_transfer_target"].clone();
        let bytes=match razer_device::profile_transfer::encode(&target,profiles,macros) {
            Ok(bytes)=>bytes,Err(error)=>{window.push_notification(error.to_string(),cx);return;}
        };
        let directory=std::env::current_dir().unwrap_or_else(|_|std::path::PathBuf::from("."));
        let timestamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_millis()).unwrap_or(0);
        let filename=format!("profiles - {timestamp}.synapse4");
        let picker=cx.prompt_for_new_path(&directory,Some(&filename));
        self.profile_export_task=Some(cx.spawn_in(window,async move|owner,cx|{
            let result=match picker.await {
                Ok(Ok(Some(path)))=>Some(cx.background_executor().spawn(async move{
                    use std::io::Write as _;
                    let mut file=std::fs::File::create(&path).map_err(|e|e.to_string())?;
                    file.write_all(&bytes).and_then(|_|file.flush()).map_err(|e|e.to_string())
                }).await),Ok(Ok(None))=>None,_=>Some(Err("Could not open the profile save dialog".to_owned())),
            };
            let _=owner.update_in(cx,|this,window,cx|{
                this.profile_export_task=None;
                if let Some(Err(error))=result {window.push_notification(error,cx);}
                cx.notify();
            });
        }));
    }

    pub(super) fn refresh_profile_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let transfer_open = self.profile_transfer.take().is_some();
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
        } else { (1..)
            .map(|index| format!("local-profile-{index}"))
            .find(|id| {
                !self
                    .device
                    .profiles
                    .iter()
                    .chain(&self.saved.profiles)
                    .any(|profile| profile.id == *id || profile.guid == *id)
            })
            .expect("unused local profile identity") };
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
            mouse_profile_source::default_profile(&self.device, name.clone()).unwrap_or_else(|| Profile {
                id: String::new(),
                guid: String::new(),
                name,
                dpi_stages: None,
                settings: None,
                source_settings: None,
            })
        };
        profile.id = id.clone();
        profile.guid = id.clone();
        if matches!(self.device.product_id, 182 | 756) {
            if let Some(source) = profile.source_settings.as_mut().and_then(Value::as_object_mut) {
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
                            this.profile_confirmation = Some(ProfileConfirmation { action, id, bindings_only:false });
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
                let dialog = cx.new(|cx| {
                    let target=if self.device.product_id==756 {
                        razer_device::keyboard_latest_mapping::source()["profile_transfer_target"].clone()
                    } else {
                        serde_json::json!({"productId":self.device.product_id,"deviceName":self.device.product_name,
                            "category":serde_json::to_value(self.device.category).unwrap_or(Value::Null)})
                    };
                    let profiles=self.device.profiles.iter().map(|profile| {
                        let mut source=profile.source_settings.clone().unwrap_or(serde_json::json!({}));
                        source["name"]=profile.name.clone().into();source["guid"]=profile.guid.clone().into();source
                    }).collect();
                    SourceProfileTransfer::new(
                        self.device.product_id,
                        mode,
                        target,
                        profiles,
                        self.local_mapping_macros.clone(),
                        self.device.active_profile_obj().map(|profile|profile.guid.clone()).unwrap_or_default(),
                        window,
                        cx,
                    )
                });
                self.profile_transfer_subscription = Some(cx.subscribe_in(
                    &dialog,
                    window,
                    |this, _, event: &SourceProfileTransferEvent, window, cx| {
                        match event {
                            SourceProfileTransferEvent::Close=>this.dismiss_profile_dialog(window,cx),
                            SourceProfileTransferEvent::Import{rows,macros}=>{
                                if let Err(error)=this.import_source_profiles(rows,macros,window,cx){window.push_notification(error,cx);}
                            }
                            SourceProfileTransferEvent::Export{profiles,macros}=>this.export_source_profiles(profiles,macros,window,cx),
                        }
                    },
                ));
                dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
                self.profile_transfer = Some(dialog);
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
            ProfileAction::Reset if self.device.product_id==756=>{
                self.reset_latest_keyboard_profile(confirmation.bindings_only,window,cx);
            }
            ProfileAction::Delete if self.device.profiles.len() > 1 => {
                if let Err(error) = self.change_profile_collection(
                    super::super::product_workspace::ProfileCollectionAction::Delete(
                        confirmation.id,
                    ),
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
