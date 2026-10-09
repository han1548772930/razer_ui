//! Local mutations behind independently audited product profile commands.
use super::profile_menu::ProfileAction;
use super::profile_transfer::{
    ProfileTransferMode, SourceProfileTransfer, SourceProfileTransferClosed,
};
use super::*;

#[derive(Clone)]
pub(super) struct ProfileConfirmation {
    pub(super) action: ProfileAction,
    pub(super) id: String,
}

impl SourceProductWorkspace {
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
        let id = (1..)
            .map(|index| format!("local-profile-{index}"))
            .find(|id| {
                !self
                    .device
                    .profiles
                    .iter()
                    .chain(&self.saved.profiles)
                    .any(|profile| profile.id == *id || profile.guid == *id)
            })
            .expect("unused local profile identity");
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
            let base = if computer_name.is_empty() {
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
            Profile {
                id: String::new(),
                guid: String::new(),
                name,
                dpi_stages: None,
                settings: None,
                source_settings: None,
            }
        };
        profile.id = id.clone();
        profile.guid = id.clone();
        self.device.profiles.push(profile);
        self.refresh_profile_choices(window, cx);
        self.select_profile(&id, window, cx);
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
            ProfileAction::Delete => {
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
                            this.profile_confirmation = Some(ProfileConfirmation { action, id });
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
                    SourceProfileTransfer::new(
                        self.device.product_id,
                        mode,
                        self.device.profiles.clone(),
                        self.device.active_profile.clone(),
                        window,
                        cx,
                    )
                });
                self.profile_transfer_subscription = Some(cx.subscribe_in(
                    &dialog,
                    window,
                    |this, _, _: &SourceProfileTransferClosed, window, cx| {
                        this.dismiss_profile_dialog(window, cx);
                    },
                ));
                dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
                self.profile_transfer = Some(dialog);
            }
            ProfileAction::Share | ProfileAction::Reshare => cx.emit(WorkspaceEvent::ShareProfile),
            ProfileAction::LinkedGames | ProfileAction::LinkGames => {
                self.open_linked_games(window, cx)
            }
            // All eight independently audited accessory roots hide this entry.
            // A later family needs its own reset payload and confirmation audit.
            ProfileAction::Reset => return,
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
