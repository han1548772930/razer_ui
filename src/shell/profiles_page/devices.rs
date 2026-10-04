//! Profiles module 43: Ga -> $n -> Ua; game tile module 7693.
//! Local metadata is shared with the product workspace. Program discovery,
//! cover art and service-only profile commands await their actual data source.
use super::*;
use crate::{model::LocalizedText, resources, ui::surface::css};
use gpui_kit::component::input::InputEvent;

fn device_text(value: &LocalizedText) -> String {
    value.values.get(&i18n::locale().to_lowercase()).filter(|text| !text.is_empty())
        .or_else(|| value.values.get("en")).cloned().unwrap_or_default()
}

fn profile_choices(device: &crate::model::Device) -> Vec<Choice> {
    let hide_factory = device.profiles.iter().any(|p| p.name == "General")
        && device.profiles.iter().any(|p| p.name == "Factory Default");
    device.profiles.iter().filter(|p| !hide_factory || p.name != "Factory Default")
        .map(|p| Choice::new(&p.id, &p.name)).collect()
}

impl ProfilesPage {
    pub(super) fn device_tiles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.devices.iter().filter_map(|workspace| {
            let device = workspace.read(cx).device(cx);
            // Ga has an independent IOT/subDevices branch; the local model does
            // not supply that branch yet. Ordinary devices need both fields.
            if device.active_profile.is_empty() || device.profiles.is_empty() { return None; }
            let name = device_text(&device.name).to_uppercase();
            let image = resources::dashboard_image(device.product_id, device.edition_id, device.layout_id);
            let target = workspace.clone();
            Some(BaseButton::new(("profiles-device", workspace.entity_id()))
                .accessibility_label(name.clone()).p_0().pt(css(10.))
                .w(css(290.)).h(css(220.)).m(css(10.)).flex_shrink_0()
                .flex().flex_col().items_center().justify_start()
                .bg(rgb(0x111111)).border_2().border_color(gpui_kit::rgba(0))
                .rounded(css(5.)).text_center()
                .hover(|style| style.border_color(gpui_kit::rgba(0x44d62c4d)))
                .active(|style| style.border_color(rgb(0x44d62c)))
                .focus_visible(|style| style.border_color(rgb(0x44d62c)))
                .child(div().w(css(250.)).h(css(140.)).flex_shrink_0()
                    .when_some(image, |view, image| view.child(img(image).size_full().object_fit(ObjectFit::Contain))))
                .child(v_flex().w_full().h(css(70.)).p(css(10.)).flex_shrink_0()
                    .child(div().text_size(css(14.)).truncate().child(name))
                    // editionName is absent from Device. Preserve its space;
                    // edition IDs are not human-readable edition names.
                    .child(div().min_h(css(14.)).text_size(css(12.)).text_color(rgb(0x707070))))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let dialog = cx.new(|cx| DeviceGamesDialog::new(target.clone(), &this.devices, window, cx));
                    this._subscriptions.push(cx.observe(&dialog, |_, _, cx| cx.notify()));
                    this.device_dialog = Some(dialog);
                    cx.notify();
                })).into_any_element())
        }).collect()
    }
}

#[derive(Clone)]
struct KnownGame { name: String, executable: String }
fn game_key(path: &str) -> String { path.replace('/', "\\").to_lowercase() }

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
    games: Vec<KnownGame>,
    add_dialog: Option<Entity<AddGameDialog>>,
    back_from_add: bool,
    _subscriptions: Vec<Subscription>,
}

impl DeviceGamesDialog {
    fn new(workspace: Entity<ProductWorkspace>, devices: &[Entity<ProductWorkspace>], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let device = workspace.read(cx).snapshot(cx);
        let options = profile_choices(&device);
        let index = options.iter().position(|p| p.value == device.active_profile).map(IndexPath::new);
        let profile = cx.new(|cx| SelectState::new(options, index, window, cx));
        let filter = cx.new(|cx| SelectState::new(choices(&FILTER_KEYS), Some(IndexPath::new(0)), window, cx));
        let sort = cx.new(|cx| SelectState::new(choices(&SORT_KEYS), Some(IndexPath::new(0)), window, cx));
        let name = cx.new(|cx| InputState::new(window, cx));
        let mut games = Vec::<KnownGame>::new();
        for entity in devices {
            let owner = entity.read(cx);
            for profile in &owner.device(cx).profiles {
                for (name, executable) in owner.profile_linked_games(&profile.id, cx) {
                    if !games.iter().any(|game| game_key(&game.executable) == game_key(&executable)) {
                        games.push(KnownGame { name, executable });
                    }
                }
            }
        }
        let subscriptions = vec![
            cx.observe(&workspace, |_, _, cx| cx.notify()),
            cx.observe(&profile, |_, _, cx| cx.notify()),
            cx.observe(&filter, |_, _, cx| cx.notify()),
            cx.observe(&sort, |_, _, cx| cx.notify()),
            cx.subscribe_in(&name, window, |this: &mut Self, _, event, window, cx| {
                if this.renaming && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.finish_rename(window, cx);
                }
            }),
        ];
        let focus = cx.focus_handle();
        let return_focus = window.focused(cx);
        focus.focus(window, cx);
        Self { open: true, focus, return_focus, workspace, profile, filter, sort, name,
            renaming: false, menu_open: false, games, add_dialog: None, back_from_add: false, _subscriptions: subscriptions }
    }

    fn selected_profile(&self, cx: &App) -> Option<String> { self.profile.read(cx).selected_value().cloned() }

    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_profile(cx) else { return; };
        let Some(name) = self.workspace.read(cx).device(cx).profiles.iter().find(|p| p.id == id).map(|p| p.name.clone()) else { return; };
        self.renaming = true;
        self.name.update(cx, |input, cx| {
            input.set_value(name, window, cx); input.focus(window, cx); input.select_all(window, cx);
        });
        cx.notify();
    }
    fn finish_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = false;
        if let Some(id) = self.selected_profile(cx) {
            let name = self.name.read(cx).value().to_string();
            self.workspace.update(cx, |workspace, cx| workspace.rename_profile(&id, name, window, cx));
            let options = profile_choices(self.workspace.read(cx).device(cx));
            self.profile.update(cx, |state, cx| {
                state.set_items(options, window, cx); state.set_selected_value(&id, window, cx);
            });
        }
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        if let Some(focus) = self.return_focus.take() { focus.focus(window, cx); }
        cx.notify();
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let add = cx.new(|cx| { let mut dialog = AddGameDialog::new(window, cx); dialog.from_device = true; dialog });
        self._subscriptions.push(cx.observe(&add, |this, _, cx| { this.back_from_add = true; cx.notify(); }));
        self.add_dialog = Some(add);
        cx.notify();
    }

    fn more_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let selected = self.selected_profile(cx).is_some();
        gpui_kit::base::Popover::new("profiles-device-actions")
            .anchor(Anchor::TopLeft).open(self.menu_open)
            .trigger_with(move |_, _, _| BaseButton::new("profiles-device-more")
                .accessibility_label("profile actions").size(css(26.)).mr(css(10.)).p_0()
                .border_1().border_color(rgb(0x222222))
                .hover(|style| style.border_color(rgb(0x5d5d5d)))
                .active(|style| style.border_color(rgb(0x44d62c)))
                .child(img("synapse/profile-more.svg").size(css(20.))).into_any_element())
            .on_open_change(cx.listener(|this, open: &bool, _, cx| { this.menu_open = *open; cx.notify(); }))
            .content(move |_, _, cx| {
                let mut menu = v_flex().min_w(css(155.)).max_w(css(280.)).bg(rgb(0))
                    .border_1().border_color(rgb(0x5d5d5d)).text_size(css(14.)).text_color(rgb(0xcccccc));
                for key in ["ADD", "IMPORT", "divider", "RENAME", "DUPLICATE", "EXPORT", "divider", "DELETE"] {
                    if key == "divider" { menu = menu.child(div().h(css(1.)).my(css(4.)).mx(css(6.)).bg(rgb(0x5d5d5d))); continue; }
                    let owner = owner.clone();
                    // Service commands remain unavailable until their payloads
                    // are implemented for every product; rename preserves them.
                    menu = menu.child(BaseButton::new(key).p_0().px(css(6.)).py(css(5.)).line_height(css(17.))
                        .justify_start().disabled(!selected || key != "RENAME")
                        .styles(|style| style.disabled(|s| s.opacity(0.3)))
                        .hover(|style| style.bg(gpui_kit::rgba(0xffffff1a))).child(i18n::t(key))
                        .on_click(move |_, window, cx| { let _ = owner.update(cx, |this, cx| {
                            this.menu_open = false; this.begin_rename(window, cx);
                        }); }));
                }
                menu.into_any_element()
            }).into_any_element()
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let options = profile_choices(self.workspace.read(cx).device(cx));
        let field = if self.renaming {
            div().on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" { this.renaming = false; this.focus.focus(window, cx); cx.stop_propagation(); cx.notify(); }
            })).child(Input::new(&self.name).appearance(false).w(css(230.)).h(css(27.))
                .p(css(5.)).bg(rgb(0x111111)).border_1().border_color(rgb(0x44d62c))
                .rounded_none().text_size(css(14.)).line_height(css(17.))).into_any_element()
        } else {
            surface::select(&self.profile).id("profiles-device-profile").items(options.clone())
                .accessibility_label(i18n::t("PROFILE")).disabled(options.len() == 1)
                .w(css(230.)).into_any_element()
        };
        let mut right = h_flex().ml_auto().mr(css(18.));
        for (label, state, keys) in [("VIEWS", &self.filter, FILTER_KEYS.as_slice()), ("ORDER", &self.sort, SORT_KEYS.as_slice())] {
            right = right.child(h_flex().gap(css(10.)).ml(css(10.))
                .child(i18n::t(label)).child(surface::select(state).items(choices(keys))
                    .accessibility_label(i18n::t(label)).w(css(160.))));
        }
        // 8844 replaces the add/scan/search block with Ua's profile bar.
        h_flex().relative().w_full().h(css(26.)).mb(css(10.))
            .child(h_flex().absolute().left_0().w(relative(0.5)).h_full()
                .child(div().mx(css(10.)).child(field)).child(self.more_menu(cx)))
            .child(right).into_any_element()
    }

    fn game_tile(&self, game: KnownGame, cx: &mut Context<Self>) -> AnyElement {
        let id = self.selected_profile(cx);
        let workspace = self.workspace.read(cx);
        let linked_profile = workspace.device(cx).profiles.iter().find(|profile|
            workspace.profile_linked_games(&profile.id, cx).iter().any(|(_, path)| game_key(path) == game_key(&game.executable)))
            .map(|profile| (profile.id.clone(), profile.name.clone()));
        let active = linked_profile.as_ref().is_some_and(|(linked, _)| Some(linked) == id.as_ref());
        let busy = linked_profile.is_some() && !active;
        let name = game.name.clone();
        let target = self.workspace.clone();
        BaseButton::new(SharedString::from(format!("profiles-game-{}", game.executable)))
            .accessibility_label(name.clone()).disabled(id.is_none()).selected(active)
            .group("profiles-linked-game").relative().p_0().flex().flex_col().justify_start()
            .w(css(240.)).h(css(190.)).mx(css(5.)).mt(css(10.)).flex_shrink_0()
            .bg(rgb(0x111111)).border_1().rounded(css(5.))
            .when(active, |tile| tile.border_2()).border_color(if active { rgb(0x44d62c) } else { rgb(0x111111) })
            .hover(|style| style.border_color(rgb(0x44d62c)))
            .active(move |style| if active { style.border_1() } else { style.border_2().border_color(rgb(0x44d62c)) })
            .when(busy, |tile| tile.child(div().absolute().left_0().top_0().w_full().h(css(40.))
                .bg(gpui_kit::rgba(0x11111180)).pl(css(40.)).pt(css(12.)).text_size(css(14.)).line_height(css(17.))
                .text_color(rgb(0x44d62c)).truncate().child(linked_profile.as_ref().map(|(_, name)| name.clone()).unwrap_or_default())))
            .child(div().absolute().left(css(10.)).top(css(10.)).size(css(20.)).border_1()
                .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
                .bg(if active { rgb(0x44d62c) } else { rgb(0x111111) })
                .when(active, |check| check.child(img("synapse/check.svg").size_full()))
                .when(busy, |check| check.child(div().absolute().left(css(4.)).top(css(8.)).w(css(10.)).h(css(2.)).bg(rgb(0x44d62c)))))
            .child(div().h(css(120.)).w_full().flex_shrink_0())
            .child(div().h(css(70.)).w_full().p(css(10.)).text_size(css(14.)).line_height(css(19.))
                .text_center().text_color(rgb(0xcccccc)).group_hover("profiles-linked-game", |s| s.text_color(rgb(0x44d62c)))
                .child(div().truncate().child(name)))
            .on_click(move |_, window, cx| { if let Some(id) = &id {
                target.update(cx, |workspace, cx| workspace.link_profile_game(id, game.name.clone(), game.executable.clone(), !active, window, cx));
            } }).into_any_element()
    }

    fn game_tiles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let removed = self.filter.read(cx).selected_value().is_some_and(|value| value == "REMOVED_GAMES");
        let mut games = if removed { Vec::new() } else { self.games.clone() };
        let sort = self.sort.read(cx).selected_value().cloned().unwrap_or_default();
        if matches!(sort.as_str(), "NAME_A_TO_Z" | "NAME_Z_TO_A") {
            games.sort_by_key(|game| game.name.to_lowercase());
            if sort == "NAME_Z_TO_A" { games.reverse(); }
        }
        let mut tiles: Vec<_> = games.into_iter().map(|game| self.game_tile(game, cx)).collect();
        // Ua appends add-new even for the Removed filter; oe differs here.
        tiles.push(BaseButton::new("profiles-device-add-game").p_0().flex().flex_col().justify_start()
            .w(css(240.)).h(css(190.)).mx(css(5.)).mt(css(10.)).flex_shrink_0()
            .border_2().border_dashed().border_color(rgb(0x5d5d5d)).rounded(css(5.))
            .hover(|style| style.border_color(rgb(0x44d62c)))
            .active(|style| style.border_color(rgb(0x44d62c)))
            .child(div().flex().items_center().justify_center().h(css(120.)).w_full().flex_shrink_0()
                .child(img("synapse/dashboard-add.svg").size(css(40.))))
            .child(div().h(css(70.)).w_full().text_center().text_size(css(14.)).line_height(css(16.))
                .child(format!("{} {}\n{}", i18n::t("CLICK_TO_ADD"), i18n::t("GAME_PROGRAM"), i18n::t("DRAG_AND_DROP_HERE"))))
            .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx))).into_any_element());
        tiles
    }
}

impl Render for DeviceGamesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open { return div().into_any_element(); }
        if let Some(dialog) = &self.add_dialog { if dialog.read(cx).open { return dialog.clone().into_any_element(); } }
        let opacity = Presence::new((ElementId::from(("profiles-device", cx.entity_id())), "opacity"), true)
            .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear)).sample(window, cx).progress;
        let progress = Presence::new((ElementId::from(("profiles-device", cx.entity_id())), "position"), true)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Ease)).sample(window, cx).progress;
        let (opacity, progress) = if self.back_from_add { (1., 1.) } else { (opacity, progress) };
        let viewport = window.viewport_size();
        let unit = window.rem_size() / 16.;
        let app_height = viewport.height - unit * 42.;
        let width = unit * popup_width(f32::from(viewport.width / unit), true);
        let target_top = unit * (42. + 110. + 100.);
        let start_top = viewport.height + unit * 110.;
        let title = format!("LINKED GAMES TO {}", device_text(&self.workspace.read(cx).device(cx).product_name)).to_uppercase();
        gpui_kit::base::Dialog::new(cx).focus_handle(self.focus.clone())
            .close_on_backdrop_press(false).on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(div().absolute().left_0().right_0().top(unit * 42.).bottom_0()
                .opacity(opacity).bg(gpui_kit::rgba(0x00000080))
                .child(img("synapse/profiles-glow.svg").absolute().bottom_0()
                    .left((viewport.width - unit * 960.) / 2.).w(unit * 960.).h(unit * 500.)))
            .popup(v_flex().id("profiles-device-games-dialog").absolute().left((viewport.width - width) / 2.)
                .top(start_top + (target_top - start_top) * progress).w(width).h((app_height - unit * 210.).max(px(0.)))
                .bg(rgb(0x222222)).rounded_t(css(5.)).occlude().text_size(css(14.)).line_height(css(17.)).text_color(rgb(0xcccccc))
                .child(h_flex().relative().w_full().h(css(36.)).flex_shrink_0().justify_center()
                    .pt(css(20.)).pb(css(10.)).font_family("RazerF5").text_size(css(16.)).line_height(css(19.))
                    .text_color(rgb(0x999999)).shadow(vec![BoxShadow { color: rgb(0x5d5d5d).into(), offset: point(px(0.), unit), blur_radius: px(0.), spread_radius: px(0.), inset: false }])
                    .child(title).child(BaseButton::new("profiles-device-close").accessibility_label(i18n::t("CLOSE"))
                        .absolute().right_0().top_0().size(css(36.)).p_0().flex().items_center().justify_center()
                        .hover(|style| style.bg(gpui_kit::rgba(0xffffff1a))).active(|style| style.bg(gpui_kit::rgba(0x0000001a)))
                        .child(img("synapse/profiles-close.svg").size(css(20.)))
                        .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))))
                .child(v_flex().flex_1().min_h_0().pt(css(20.)).pl(css(25.)).pb(css(42.))
                    .child(self.toolbar(cx)).child(div().id("profiles-device-games-scroll").flex_1().min_h_0().scrollable_both()
                        .child(h_flex().items_start().flex_wrap().min_w(css(800.)).mr(-css(5.)).pb(css(25.)).children(self.game_tiles(cx))))))
            .into_any_element()
    }
}
