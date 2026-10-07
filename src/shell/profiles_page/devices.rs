//! Profiles module 43: Ga -> $n -> Ua; game tile module 7693.
//! Local metadata is shared with the product workspace. Program discovery,
//! cover art and service-only profile commands await their actual data source.
use super::*;
use crate::{model::LocalizedText, resources, ui::surface::css};
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::select::{SelectEvent, SelectItem};

fn device_text(value: &LocalizedText) -> String {
    value
        .values
        .get(&i18n::locale().to_lowercase())
        .filter(|text| !text.is_empty())
        .or_else(|| value.values.get("en"))
        .cloned()
        .unwrap_or_default()
}

fn profile_choices(device: &crate::model::Device) -> Vec<Choice> {
    let hide_factory = device.profiles.iter().any(|p| p.name == "General")
        && device.profiles.iter().any(|p| p.name == "Factory Default");
    device
        .profiles
        .iter()
        .filter(|p| !hide_factory || p.name != "Factory Default")
        .map(|p| Choice::new(&p.id, &p.name))
        .collect()
}

impl ProfilesPage {
    pub(super) fn device_tiles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.devices
            .iter()
            .filter_map(|workspace| {
                let device = workspace.read(cx).device(cx);
                // Ga has an independent IOT/subDevices branch; the local model does
                // not supply that branch yet. Ordinary devices need both fields.
                if device.active_profile.is_empty() || device.profiles.is_empty() {
                    return None;
                }
                let name = device_text(&device.name).to_uppercase();
                let image = resources::dashboard_image(
                    device.product_id,
                    device.edition_id,
                    device.layout_id,
                );
                let target = workspace.clone();
                Some(
                    BaseButton::new(("profiles-device", workspace.entity_id()))
                        .accessibility_label(name.clone())
                        .p_0()
                        .pt(css(10.))
                        .w(css(290.))
                        .h(css(220.))
                        .m(css(10.))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_start()
                        .bg(rgb(0x111111))
                        .border_2()
                        .border_color(gpui_kit::rgba(0))
                        .rounded(css(5.))
                        .text_center()
                        .hover(|style| style.border_color(gpui_kit::rgba(0x44d62c4d)))
                        .active(|style| style.border_color(rgb(0x44d62c)))
                        .focus_visible(|style| style.border_color(rgb(0x44d62c)))
                        .child(div().w(css(250.)).h(css(140.)).flex_shrink_0().when_some(
                            image,
                            |view, image| {
                                view.child(img(image).size_full().object_fit(ObjectFit::Contain))
                            },
                        ))
                        .child(
                            v_flex()
                                .w_full()
                                .h(css(70.))
                                .p(css(10.))
                                .flex_shrink_0()
                                .child(div().text_size(css(14.)).truncate().child(name))
                                // editionName is absent from Device. Preserve its space;
                                // edition IDs are not human-readable edition names.
                                .child(
                                    div()
                                        .min_h(css(14.))
                                        .text_size(css(12.))
                                        .text_color(rgb(0x707070)),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let dialog = cx.new(|cx| {
                                DeviceGamesDialog::new(target.clone(), &this.devices, window, cx)
                            });
                            this._subscriptions
                                .push(cx.observe(&dialog, |_, _, cx| cx.notify()));
                            this.device_dialog = Some(dialog);
                            cx.notify();
                        }))
                        .into_any_element(),
                )
            })
            .collect()
    }
}

#[derive(Clone)]
struct KnownGame {
    name: String,
    executable: String,
}
fn game_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}

pub(super) struct DeviceGamesDialog {
    pub(super) open: bool,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    workspace: Entity<ProductWorkspace>,
    profile: Entity<SelectState<Vec<Choice>>>,
    filter: Entity<SelectState<Vec<Choice>>>,
    sort: Entity<SelectState<Vec<Choice>>>,
    name: Entity<InputState>,
    renaming: bool,
    menu_open: bool,
    delete_request: Option<(String, String)>,
    delete_task: Option<Task<()>>,
    collection_error: Option<String>,
    collection_status: String,
    transfer: Option<Entity<super::transfer::ProfileTransfer>>,
    transfer_subscription: Option<Subscription>,
    transfer_intent: Option<super::transfer::TransferIntent>,
    games: Vec<KnownGame>,
    devices: Vec<Entity<ProductWorkspace>>,
    device_subscriptions: Vec<Subscription>,
    add_dialog: Option<Entity<AddGameDialog>>,
    back_from_add: bool,
    _subscriptions: Vec<Subscription>,
}

impl DeviceGamesDialog {
    fn new(
        workspace: Entity<ProductWorkspace>,
        devices: &[Entity<ProductWorkspace>],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let device = workspace.read(cx).snapshot(cx);
        let options = profile_choices(&device);
        let index = options
            .iter()
            .position(|p| p.value() == &device.active_profile)
            .or_else(|| (!options.is_empty()).then_some(0))
            .map(IndexPath::new);
        let profile = cx.new(|cx| SelectState::new(options, index, window, cx));
        let filter = cx
            .new(|cx| SelectState::new(choices(&FILTER_KEYS), Some(IndexPath::new(0)), window, cx));
        let sort =
            cx.new(|cx| SelectState::new(choices(&SORT_KEYS), Some(IndexPath::new(0)), window, cx));
        let name = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let mut games = Vec::<KnownGame>::new();
        for entity in devices {
            let owner = entity.read(cx);
            for profile in &owner.device(cx).profiles {
                for (name, executable) in owner.profile_linked_games(&profile.id, cx) {
                    if !games
                        .iter()
                        .any(|game| game_key(&game.executable) == game_key(&executable))
                    {
                        games.push(KnownGame { name, executable });
                    }
                }
            }
        }
        let subscriptions = vec![
            cx.observe_in(&workspace, window, |this: &mut Self, _, window, cx| {
                this.sync_profile_choices(window, cx);
            }),
            cx.subscribe_in(&profile, window, |this: &mut Self, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(_)) = event {
                    // Ua/Y changes the assignment target only. It does not
                    // activate a hardware profile when browsing this list.
                    this.delete_request = None;
                    this.delete_task = None;
                    cx.notify();
                }
            }),
            cx.observe(&filter, |_, _, cx| cx.notify()),
            cx.observe(&sort, |_, _, cx| cx.notify()),
            cx.subscribe_in(&name, window, |this: &mut Self, _, event, window, cx| {
                if this.renaming
                    && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                {
                    this.finish_rename(window, cx);
                }
            }),
        ];
        let focus = cx.focus_handle();
        let return_focus = window.focused(cx);
        focus.focus(window, cx);
        let mut dialog = Self {
            open: true,
            focus,
            return_focus,
            workspace,
            profile,
            filter,
            sort,
            name,
            renaming: false,
            menu_open: false,
            delete_request: None,
            delete_task: None,
            collection_error: None,
            collection_status: String::new(),
            transfer: None,
            transfer_subscription: None,
            transfer_intent: None,
            games,
            devices: Vec::new(),
            device_subscriptions: Vec::new(),
            add_dialog: None,
            back_from_add: false,
            _subscriptions: subscriptions,
        };
        dialog.set_devices(devices.to_vec(), cx);
        dialog
    }

    pub(super) fn set_devices(
        &mut self,
        devices: Vec<Entity<ProductWorkspace>>,
        cx: &mut Context<Self>,
    ) {
        self.device_subscriptions = devices
            .iter()
            .map(|device| cx.observe(device, |this, _, cx| this.refresh_known_games(cx)))
            .collect();
        self.devices = devices;
        self.refresh_known_games(cx);
    }

    fn refresh_known_games(&mut self, cx: &mut Context<Self>) {
        // Ua observes the current game catalog. This local adapter only knows
        // explicit profile associations: retain previously observed games when
        // they are unlinked, and incorporate later associations from every
        // currently supplied workspace without inventing an installed catalog.
        for entity in &self.devices {
            let owner = entity.read(cx);
            for profile in &owner.device(cx).profiles {
                for (name, executable) in owner.profile_linked_games(&profile.id, cx) {
                    if let Some(game) = self
                        .games
                        .iter_mut()
                        .find(|game| game_key(&game.executable) == game_key(&executable))
                    {
                        game.name = name;
                    } else {
                        self.games.push(KnownGame { name, executable });
                    }
                }
            }
        }
        cx.notify();
    }

    fn selected_profile(&self, cx: &App) -> Option<String> {
        let id = self.profile.read(cx).selected_value()?;
        self.workspace
            .read(cx)
            .device(cx)
            .profiles
            .iter()
            .any(|profile| &profile.id == id)
            .then(|| id.clone())
    }

    fn sync_profile_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let device = self.workspace.read(cx).device(cx);
        let options = profile_choices(device);
        let previous = self.profile.read(cx).selected_value().cloned();
        let retained = previous
            .as_ref()
            .filter(|id| options.iter().any(|option| option.value() == *id));
        let selected = retained.cloned().or_else(|| {
            options
                .iter()
                .find(|option| option.value() == &device.active_profile)
                .or_else(|| options.first())
                .map(|option| option.value().clone())
        });
        if retained.is_none() {
            // Never submit a name or delayed deletion against a replacement ID.
            self.renaming = false;
            self.delete_request = None;
            self.delete_task = None;
        }
        if let Some((id, name)) = &mut self.delete_request {
            if let Some(profile) = device.profiles.iter().find(|profile| &profile.id == id) {
                *name = profile.name.clone();
            } else {
                self.delete_request = None;
                self.delete_task = None;
            }
        }
        self.profile.update(cx, |state, cx| {
            state.set_items(options, window, cx);
            if let Some(id) = selected {
                state.set_selected_value(&id, window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
        cx.notify();
    }

    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_profile(cx) else {
            return;
        };
        let Some(name) = self
            .workspace
            .read(cx)
            .device(cx)
            .profiles
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
        else {
            return;
        };
        self.renaming = true;
        self.name.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }
    fn finish_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = false;
        if let Some(id) = self.selected_profile(cx) {
            let name = controls::trim_name(&self.name.read(cx).value()).to_string();
            let duplicate = self
                .workspace
                .read(cx)
                .device(cx)
                .profiles
                .iter()
                .any(|profile| profile.name == name);
            // Current 1867/p compares case-sensitively, including this profile.
            if !name.is_empty() && !duplicate {
                self.workspace.update(cx, |workspace, cx| {
                    workspace.rename_profile(&id, name, window, cx)
                });
            }
            let options = profile_choices(self.workspace.read(cx).device(cx));
            self.profile.update(cx, |state, cx| {
                state.set_items(options, window, cx);
                state.set_selected_value(&id, window, cx);
            });
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(transfer) = self.transfer.take() {
            transfer.update(cx, |transfer, cx| transfer.dismiss(cx));
        }
        self.transfer_subscription = None;
        self.delete_task = None;
        self.delete_request = None;
        self.open = false;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_request = None;
        self.delete_task = None;
        let add = cx.new(|cx| {
            let mut dialog = AddGameDialog::new(window, cx);
            dialog.from_device = true;
            dialog
        });
        self._subscriptions.push(cx.observe(&add, |this, _, cx| {
            this.back_from_add = true;
            cx.notify();
        }));
        self.add_dialog = Some(add);
        cx.notify();
    }

    fn profile_action(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_open = false;
        self.delete_request = None;
        self.delete_task = None;
        self.collection_error = None;
        if matches!(action, "IMPORT" | "EXPORT") {
            let mode = if action == "IMPORT" {
                super::transfer::Mode::Import
            } else {
                super::transfer::Mode::Export
            };
            let device = self.workspace.read(cx).device(cx);
            let pid = device.product_id;
            let name = device_text(&device.product_name);
            let profiles = device.profiles.clone();
            let transfer = cx.new(|cx| {
                super::transfer::ProfileTransfer::new(mode, pid, name, profiles, window, cx)
            });
            self.transfer_subscription = Some(cx.subscribe_in(
                &transfer,
                window,
                |this, _, event: &super::transfer::Closed, window, cx| {
                    if let Some(intent) = event.0.clone() {
                        match intent.validate(&this.workspace.read(cx).device(cx).profiles) {
                            Ok(()) => {
                                this.collection_status = intent.description();
                                this.transfer_intent = Some(intent);
                            }
                            Err(error) => this.collection_error = Some(error),
                        }
                    }
                    this.transfer = None;
                    this.transfer_subscription = None;
                    this.focus.focus(window, cx);
                    cx.notify();
                },
            ));
            self.transfer = Some(transfer);
            cx.notify();
            return;
        }
        if action == "RENAME" {
            self.begin_rename(window, cx);
            return;
        }
        let selected = self.selected_profile(cx);
        if action == "DELETE" {
            let Some(id) = selected else { return };
            let Some(name) = self
                .workspace
                .read(cx)
                .device(cx)
                .profiles
                .iter()
                .find(|profile| profile.id == id)
                .map(|profile| profile.name.clone())
            else {
                return;
            };
            // Current Ua waits 100ms after dismissing the more menu.
            self.delete_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.open && this.selected_profile(cx).as_ref() == Some(&id) {
                        this.delete_request = Some((id, name));
                        cx.notify();
                    }
                });
            }));
            cx.notify();
            return;
        }
        let result = self.workspace.update(cx, |workspace, cx| match action {
            "ADD" => workspace.add_local_profile(window, cx),
            "DUPLICATE" => selected
                .as_deref()
                .ok_or_else(|| "未选择有效配置文件".to_string())
                .and_then(|id| workspace.duplicate_local_profile(id, window, cx)),
            _ => Err("此配置操作尚未接入".into()),
        });
        self.finish_collection_change(result, window, cx);
    }

    fn finish_collection_change(
        &mut self,
        result: Result<String, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(id) => {
                let options = profile_choices(self.workspace.read(cx).device(cx));
                self.profile.update(cx, |state, cx| {
                    state.set_items(options, window, cx);
                    state.set_selected_value(&id, window, cx);
                });
                self.collection_status = "配置更改保留为本地草稿；关闭此弹层后可通过顶部未保存配置入口保存，尚未写入设备".into();
                self.collection_error = None;
            }
            Err(error) => self.collection_error = Some(error),
        }
        cx.notify();
    }

    fn confirm_profile_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, _)) = self.delete_request.take() else {
            return;
        };
        self.delete_task = None;
        let result = self.workspace.update(cx, |workspace, cx| {
            workspace.delete_local_profile(&id, window, cx)
        });
        self.finish_collection_change(result, window, cx);
    }

    fn delete_confirmation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (_, name) = self.delete_request.as_ref()?;
        let opacity = Presence::new("profiles-delete-opacity", true)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        Some(
            deferred(
                v_flex()
                    .id("profiles-profile-delete-confirmation")
                    .test_support()
                    .aria_label(format!("{}：{name}", i18n::t("DELETE_PROFILE_TITLE")))
                    .absolute()
                    .left(css(250.))
                    .top(css(42.))
                    .w(css(300.))
                    .p(css(20.))
                    .items_center()
                    .occlude()
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(rgb(0xfd4949))
                    .rounded(css(3.))
                    .opacity(opacity)
                    .text_center()
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.delete_request = None;
                            this.delete_task = None;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .mb(css(10.))
                            .text_color(rgb(0xfd4949))
                            .child(i18n::t("DELETE_PROFILE_TITLE").to_uppercase()),
                    )
                    .child(div().mb(css(10.)).child(i18n::t("DELETE_PROFILE_MSG")))
                    .child(
                        BaseButton::new("profiles-profile-delete-confirm")
                            .accessibility_label(format!("{} {name}", i18n::t("DELETE")))
                            .min_w(css(90.))
                            .h(css(27.))
                            .px(css(5.))
                            .py(css(4.))
                            .bg(rgb(0xfd4949))
                            .text_color(rgb(0x111111))
                            .border_1()
                            .border_color(rgba(0x0000004d))
                            .text_size(css(12.))
                            .line_height(css(14.))
                            .child(i18n::t("DELETE"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_profile_delete(window, cx)
                            })),
                    ),
            )
            .with_priority(250)
            .into_any_element(),
        )
    }

    fn more_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let selected = self.selected_profile(cx).is_some();
        let can_delete = self.workspace.read(cx).device(cx).profiles.len() > 1;
        let trigger = controls::more_button(self.menu_open, window, cx);
        gpui_kit::base::Popover::new("profiles-device-actions")
            .anchor(Anchor::TopLeft)
            .open(self.menu_open)
            .trigger_with(move |_, _, _| trigger.into_any_element())
            .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                this.menu_open = *open;
                if *open {
                    this.delete_request = None;
                    this.delete_task = None;
                }
                cx.notify();
            }))
            .content(move |_, window, cx| {
                let mut menu = v_flex()
                    .min_w(css(155.))
                    .max_w(css(280.))
                    .bg(rgb(0))
                    .border_1()
                    .border_color(rgb(0x5d5d5d))
                    .text_size(css(14.))
                    .text_color(rgb(0xcccccc));
                for key in [
                    "ADD",
                    "IMPORT",
                    "divider",
                    "RENAME",
                    "DUPLICATE",
                    "EXPORT",
                    "divider",
                    "DELETE",
                ] {
                    if key == "divider" {
                        menu =
                            menu.child(div().h(css(1.)).my(css(4.)).mx(css(6.)).bg(rgb(0x5d5d5d)));
                        continue;
                    }
                    let owner = owner.clone();
                    let disabled = (key != "ADD" && !selected) || (key == "DELETE" && !can_delete);
                    let pointer = controls::pointer(
                        (ElementId::from("profiles-profile-menu"), key).into(),
                        window,
                        cx,
                    );
                    let hovered = pointer.read(cx).hovered && !disabled;
                    let background = motion::transition(
                        (ElementId::from("profiles-profile-menu-bg"), key),
                        Hsla::from(rgba(if hovered { 0xffffff1a } else { 0x00000000 })),
                        Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
                        window,
                        cx,
                    );
                    menu = menu.child(
                        controls::track(BaseButton::new(key), &pointer, window)
                            .p_0()
                            .px(css(6.))
                            .py(css(5.))
                            .line_height(css(17.))
                            .justify_start()
                            .disabled(disabled)
                            .styles(|style| style.disabled(|s| s.opacity(0.3)))
                            .bg(background)
                            .child(i18n::t(key))
                            .on_click(move |_, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.profile_action(key, window, cx);
                                });
                            }),
                    );
                }
                menu.into_any_element()
            })
            .into_any_element()
    }

    fn toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let options = profile_choices(self.workspace.read(cx).device(cx));
        let field = if self.renaming {
            div()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        this.renaming = false;
                        this.focus.focus(window, cx);
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(
                    Input::new(&self.name)
                        .appearance(false)
                        .w(css(230.))
                        .h(css(27.))
                        .p(css(5.))
                        .bg(rgb(0x111111))
                        .border_1()
                        .border_color(rgb(0x44d62c))
                        .rounded_none()
                        .text_size(css(14.))
                        .line_height(css(17.)),
                )
                .into_any_element()
        } else {
            surface::select(&self.profile)
                .id("profiles-device-profile")
                .items(options.clone())
                .accessibility_label(i18n::t("PROFILE"))
                .disabled(options.len() == 1)
                .w(css(230.))
                .into_any_element()
        };
        let mut right = h_flex().ml_auto().mr(css(18.));
        for (label, state, keys) in [
            ("VIEWS", &self.filter, FILTER_KEYS.as_slice()),
            ("ORDER", &self.sort, SORT_KEYS.as_slice()),
        ] {
            right = right.child(
                h_flex()
                    .gap(css(10.))
                    .ml(css(10.))
                    .child(i18n::t(label))
                    .child(
                        surface::select(state)
                            .items(choices(keys))
                            .accessibility_label(i18n::t(label))
                            .w(css(160.)),
                    ),
            );
        }
        // 8844 replaces the add/scan/search block with Ua's profile bar.
        h_flex()
            .relative()
            .w_full()
            .h(css(26.))
            .mb(css(10.))
            .child(
                h_flex()
                    .absolute()
                    .left_0()
                    .w(relative(0.5))
                    .h_full()
                    .child(div().mx(css(10.)).child(field))
                    .child(self.more_menu(window, cx)),
            )
            .child(right)
            .children(self.delete_confirmation(window, cx))
            .into_any_element()
    }

    fn game_tile(
        &self,
        game: KnownGame,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = self.selected_profile(cx);
        let workspace = self.workspace.read(cx);
        let linked_profile = workspace
            .device(cx)
            .profiles
            .iter()
            .find(|profile| {
                workspace
                    .profile_linked_games(&profile.id, cx)
                    .iter()
                    .any(|(_, path)| game_key(path) == game_key(&game.executable))
            })
            .map(|profile| (profile.id.clone(), profile.name.clone()));
        let active = linked_profile
            .as_ref()
            .is_some_and(|(linked, _)| Some(linked) == id.as_ref());
        let busy = linked_profile.is_some() && !active;
        let name = game.name.clone();
        let target = self.workspace.clone();
        let tile_id = ElementId::from(SharedString::from(format!(
            "profiles-game-{}",
            game.executable
        )));
        let pointer = controls::pointer(tile_id.clone(), window, cx);
        let hovered = pointer.read(cx).hovered;
        let pressed = pointer.read(cx).pressed;
        controls::track(BaseButton::new(tile_id.clone()), &pointer, window)
            .accessibility_label(name.clone())
            .disabled(id.is_none())
            .selected(active)
            .group("profiles-linked-game")
            .relative()
            .p_0()
            .flex()
            .flex_col()
            .justify_start()
            .w(css(240.))
            .h(css(190.))
            .mx(css(5.))
            .mt(css(10.))
            .flex_shrink_0()
            .bg(rgb(0x111111))
            .border_1()
            .rounded(css(5.))
            .when(active, |tile| tile.border_2())
            .border_color(if active { rgb(0x44d62c) } else { rgb(0x111111) })
            .hover(|style| style.border_color(rgb(0x44d62c)))
            .active(move |style| {
                if active {
                    style.border_1()
                } else {
                    style.border_2().border_color(rgb(0x44d62c))
                }
            })
            .when(busy, |tile| {
                tile.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w_full()
                        .h(css(40.))
                        .bg(gpui_kit::rgba(0x11111180))
                        .rounded_t(css(5.))
                        .child(
                            div()
                                .absolute()
                                .left(css(40.))
                                .top(css(12.))
                                .max_w(relative(0.8))
                                .text_size(css(14.))
                                .line_height(css(17.))
                                .text_color(rgb(0x44d62c))
                                .truncate()
                                .child(
                                    linked_profile
                                        .as_ref()
                                        .map(|(_, name)| name.clone())
                                        .unwrap_or_default(),
                                ),
                        ),
                )
            })
            .child(controls::linked_check(
                tile_id, active, busy, hovered, pressed, window, cx,
            ))
            .child(div().h(css(120.)).w_full().flex_shrink_0())
            .child(
                div()
                    .h(css(70.))
                    .w_full()
                    .p(css(10.))
                    .text_size(css(14.))
                    .line_height(css(19.))
                    .text_center()
                    .text_color(rgb(0xcccccc))
                    .group_hover("profiles-linked-game", |s| s.text_color(rgb(0x44d62c)))
                    .child(div().truncate().child(name)),
            )
            .on_click(move |_, window, cx| {
                if let Some(id) = &id {
                    target.update(cx, |workspace, cx| {
                        workspace.link_profile_game(
                            id,
                            game.name.clone(),
                            game.executable.clone(),
                            !active,
                            window,
                            cx,
                        )
                    });
                }
            })
            .into_any_element()
    }

    fn game_tiles(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let filter = self
            .filter
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default();
        let mut games = if filter == "REMOVED_GAMES" {
            Vec::new()
        } else {
            self.games.clone()
        };
        if filter == "LINKED_GAMES" {
            // Ua/z uses any visible linked device, not just the selected
            // assignment target. Local workspaces supply only known links.
            games.retain(|game| {
                self.devices.iter().any(|entity| {
                    let owner = entity.read(cx);
                    owner.device(cx).profiles.iter().any(|profile| {
                        owner
                            .profile_linked_games(&profile.id, cx)
                            .iter()
                            .any(|(_, path)| game_key(path) == game_key(&game.executable))
                    })
                })
            });
        }
        let sort = self
            .sort
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default();
        if matches!(sort.as_str(), "NAME_A_TO_Z" | "NAME_Z_TO_A") {
            games.sort_by_key(|game| game.name.to_lowercase());
            if sort == "NAME_Z_TO_A" {
                games.reverse();
            }
        }
        let mut tiles: Vec<_> = games
            .into_iter()
            .map(|game| self.game_tile(game, window, cx))
            .collect();
        // Ua appends add-new even for the Removed filter; oe differs here.
        tiles.push(
            controls::linked_add(window, cx)
                .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx)))
                .into_any_element(),
        );
        tiles
    }
}

impl Render for DeviceGamesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        if let Some(dialog) = &self.add_dialog {
            if dialog.read(cx).open {
                return dialog.clone().into_any_element();
            }
        }
        let opacity = Presence::new(
            (
                ElementId::from(("profiles-device", cx.entity_id())),
                "opacity",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
        let progress = Presence::new(
            (
                ElementId::from(("profiles-device", cx.entity_id())),
                "position",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
        let (opacity, progress) = if self.back_from_add {
            (1., 1.)
        } else {
            (opacity, progress)
        };
        let viewport = window.viewport_size();
        let unit = window.rem_size() / 16.;
        let app_height = viewport.height - unit * 42.;
        let width = unit * popup_width(f32::from(viewport.width / unit), true);
        let target_top = unit * (42. + 110. + 100.);
        let start_top = viewport.height + unit * 110.;
        let title = format!(
            "LINKED GAMES TO {}",
            device_text(&self.workspace.read(cx).device(cx).product_name)
        )
        .to_uppercase();
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(unit * 42.)
                    .bottom_0()
                    .opacity(opacity)
                    .bg(gpui_kit::rgba(0x00000080))
                    .child(
                        img("synapse/profiles-glow.svg")
                            .absolute()
                            .bottom_0()
                            .left((viewport.width - unit * 960.) / 2.)
                            .w(unit * 960.)
                            .h(unit * 500.),
                    ),
            )
            .popup(
                v_flex()
                    .id("profiles-device-games-dialog")
                    .test_support()
                    .aria_label(title.clone())
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "escape" && this.delete_request.is_some() {
                            this.delete_request = None;
                            this.delete_task = None;
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
                    .absolute()
                    .left((viewport.width - width) / 2.)
                    .top(start_top + (target_top - start_top) * progress)
                    .w(width)
                    .h((app_height - unit * 210.).max(px(0.)))
                    .bg(rgb(0x222222))
                    .rounded_t(css(5.))
                    .occlude()
                    .text_size(css(14.))
                    .line_height(css(17.))
                    .text_color(rgb(0xcccccc))
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(css(36.))
                            .flex_shrink_0()
                            .text_center()
                            .pt(css(20.))
                            .pb(css(10.))
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .line_height(css(19.))
                            .text_color(rgb(0x999999))
                            .shadow(vec![BoxShadow {
                                color: rgb(0x5d5d5d).into(),
                                offset: point(px(0.), unit),
                                blur_radius: px(0.),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                            .child(title)
                            .child(
                                controls::close_button("profiles-device-close", window, cx)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.close(window, cx)),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .pt(css(20.))
                            .pl(css(25.))
                            .pb(css(42.))
                            .child(self.toolbar(window, cx))
                            .when(!self.collection_status.is_empty(), |view| {
                                view.child(
                                    div()
                                        .id("profiles-local-draft-status")
                                        .test_support()
                                        .text_size(css(12.))
                                        .child(self.collection_status.clone()),
                                )
                            })
                            .when_some(self.collection_error.clone(), |view, error| {
                                view.child(
                                    div()
                                        .id("profiles-local-draft-error")
                                        .test_support()
                                        .aria_label(error.clone())
                                        .text_size(css(12.))
                                        .text_color(rgb(0xfd4949))
                                        .child(error),
                                )
                            })
                            .when(self.transfer_intent.is_some(), |view| {
                                view.child(
                                    BaseButton::new("profiles-transfer-revoke")
                                        .child(if i18n::locale().starts_with("zh") {
                                            "撤销尚未应用的传输请求"
                                        } else {
                                            "Revoke unapplied transfer request"
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.transfer_intent = None;
                                            this.collection_status.clear();
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .id("profiles-device-games-scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .scrollable_both()
                                    .child(
                                        h_flex()
                                            .items_start()
                                            .flex_wrap()
                                            .min_w(css(800.))
                                            .mr(-css(5.))
                                            .pb(css(25.))
                                            .children(self.game_tiles(window, cx)),
                                    ),
                            ),
                    )
                    .children(self.transfer.clone()),
            )
            .into_any_element()
    }
}
