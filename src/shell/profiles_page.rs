//! Current /synapse/profiles/ module 43: Ba mounts Games (oe) and Devices (Ga).
//! Shared URL-matching constants are not navigation entries. See the scoped
//! AST/CSS receipts in docs/re/profiles-app-audit.json. Service data is deferred;
//! an empty catalog remains empty instead of becoming a list of fixture games.
use crate::{
    features::{Choice, ProductWorkspace},
    i18n,
    ui::{scroll::SourceScrollable as _, surface},
};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{self, Easing, Presence, Transition},
};
use gpui_kit::component::{
    input::{Input, InputState},
    select::SelectState,
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

mod controls;
mod devices;
mod transfer;

const FILTER_KEYS: [&str; 3] = ["ALL_GAMES", "LINKED_GAMES", "REMOVED_GAMES"];
const SORT_KEYS: [&str; 4] = ["NAME_A_TO_Z", "NAME_Z_TO_A", "LAST_PLAYED", "MOST_PLAYED"];

// main CSS <=900 max-width, >=1440 !important width; lazy CSS >=1600
// device override. A bare 1300px or 800px loses the enclosing media query.
fn popup_width(viewport: f32, device: bool) -> f32 {
    if device && viewport >= 1600. {
        1300.
    } else {
        let width = (viewport - 40.)
            .min(if viewport <= 900. { 800. } else { 1050. })
            .max(0.);
        // Only .profiles-link-games cancels choose-a-mat's 800px minimum.
        if device { width.max(800.) } else { width }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProfilesView {
    Games,
    Devices,
}
impl ProfilesView {
    const ALL: [Self; 2] = [Self::Games, Self::Devices];
    fn key(self) -> &'static str {
        match self {
            Self::Games => "GAMES_HEADER",
            Self::Devices => "DEVICE",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Games => "profiles-games",
            Self::Devices => "profiles-devices",
        }
    }
}

pub(super) struct ProfilesPage {
    focus: FocusHandle,
    view: ProfilesView,
    history: Vec<ProfilesView>,
    history_index: usize,
    filter: Entity<SelectState<Vec<Choice>>>,
    sort: Entity<SelectState<Vec<Choice>>>,
    search: Entity<InputState>,
    searching: bool,
    add_dialog: Option<Entity<AddGameDialog>>,
    devices: Vec<Entity<ProductWorkspace>>,
    device_subscriptions: Vec<Subscription>,
    device_dialog: Option<Entity<devices::DeviceGamesDialog>>,
    locale: String,
    _subscriptions: Vec<Subscription>,
}

fn choices(keys: &[&str]) -> Vec<Choice> {
    keys.iter()
        .map(|key| Choice::new(*key, i18n::t(key)))
        .collect()
}

impl ProfilesPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx
            .new(|cx| SelectState::new(choices(&FILTER_KEYS), Some(IndexPath::new(0)), window, cx));
        let sort =
            cx.new(|cx| SelectState::new(choices(&SORT_KEYS), Some(IndexPath::new(0)), window, cx));
        let search = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.observe(&filter, |_, _, cx| cx.notify()),
            cx.observe(&sort, |_, _, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
        ];
        Self {
            focus: cx.focus_handle(),
            view: ProfilesView::Games,
            history: vec![ProfilesView::Games],
            history_index: 0,
            filter,
            sort,
            search,
            searching: false,
            add_dialog: None,
            devices: Vec::new(),
            device_subscriptions: Vec::new(),
            device_dialog: None,
            locale: i18n::locale(),
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn set_devices(
        &mut self,
        devices: Vec<Entity<ProductWorkspace>>,
        cx: &mut Context<Self>,
    ) {
        self.device_subscriptions = devices
            .iter()
            .map(|device| cx.observe(device, |_, _, cx| cx.notify()))
            .collect();
        self.devices = devices;
        if let Some(dialog) = &self.device_dialog {
            dialog.update(cx, |dialog, cx| {
                dialog.set_devices(self.devices.clone(), cx)
            });
        }
        cx.notify();
    }

    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        self.refresh_locale(window, cx);
        cx.notify();
    }

    pub(super) fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = i18n::locale();
        if self.locale == locale {
            return;
        }
        self.locale = locale;
        for (state, keys) in [
            (&self.filter, FILTER_KEYS.as_slice()),
            (&self.sort, SORT_KEYS.as_slice()),
        ] {
            state.update(cx, |state, cx| state.set_items(choices(keys), window, cx));
        }
        cx.notify();
    }

    pub(super) fn has_previous_page(&self) -> bool {
        self.history_index > 0
    }
    pub(super) fn has_next_page(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }
    pub(super) fn history_blocked(&self, cx: &App) -> bool {
        self.add_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.read(cx).open)
            || self
                .device_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.read(cx).open)
    }
    pub(super) fn step_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if forward && self.has_next_page() {
            self.history_index += 1;
        } else if !forward && self.has_previous_page() {
            self.history_index -= 1;
        } else {
            return;
        }
        self.view = self.history[self.history_index];
        self.reset_search(window, cx);
        cx.notify();
    }
    fn set_view(&mut self, view: ProfilesView, window: &mut Window, cx: &mut Context<Self>) {
        if view == self.view {
            return;
        }
        self.view = view;
        self.history.truncate(self.history_index + 1);
        self.history.push(view);
        self.history_index = self.history.len() - 1;
        self.reset_search(window, cx);
        cx.notify();
    }
    fn reset_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.searching = false;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        // Ba replaces the mounted view, so a fresh Games instance resets these.
        for state in [&self.filter, &self.sort] {
            state.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::new(0)), window, cx)
            });
        }
    }
    pub(super) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_dialog = None;
        self.device_dialog = None;
        self.reset_search(window, cx);
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = cx.new(|cx| AddGameDialog::new(window, cx));
        self._subscriptions
            .push(cx.observe(&dialog, |_, _, cx| cx.notify()));
        self.add_dialog = Some(dialog);
        cx.notify();
    }
    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .id("profiles-navs")
            .justify_center()
            .gap(surface::css(20.))
            .children(ProfilesView::ALL.into_iter().map(|view| {
                surface::navigation_button(view.id(), i18n::t(view.key()), view == self.view, cx)
                    .disabled(self.history_blocked(cx))
                    .role(Role::Tab)
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.set_view(view, window, cx)),
                    )
            }))
            .into_any_element()
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.history_blocked(cx);
        let mut left = h_flex().flex_1().min_w_0().gap(surface::css(10.));
        if self.view == ProfilesView::Games {
            left = left
                .child(controls::nav_tip(
                    "profiles-add-tip",
                    "ADD_GAME_AND_PROGRAM",
                    icon_button(
                        "profiles-add",
                        "synapse/profiles-add.svg",
                        "ADD_GAME_AND_PROGRAM",
                        cx,
                    )
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx))),
                ))
                // Original scan is initiateGameScan; no service result is synthesized.
                .child(
                    icon_button(
                        "profiles-scan",
                        "synapse/profiles-scan.svg",
                        "SCAN_FOR_GAMES",
                        cx,
                    )
                    .disabled(true)
                    // The source dispatches `initiateGameScan` here. Keep the
                    // control visible while making the unavailable service
                    // boundary explicit instead of implying a scan ran.
                    .tooltip(|window, cx| {
                        Tooltip::new("Game scan service unavailable").build(window, cx)
                    }),
                );
            left = if self.searching {
                left.child(search_field(
                    "profiles-search-field",
                    &self.search,
                    disabled,
                    Some(Box::new(cx.listener(|this, _, window, cx| {
                        this.searching = false;
                        this.focus.focus(window, cx);
                        cx.notify();
                    }))),
                    cx,
                ))
            } else {
                left.child(controls::nav_tip(
                    "profiles-search-tip",
                    "SEARCH",
                    icon_button(
                        "profiles-search",
                        "synapse/profiles-search.svg",
                        "SEARCH",
                        cx,
                    )
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.searching = true;
                        this.search.update(cx, |input, cx| input.focus(window, cx));
                        cx.notify();
                    })),
                ))
            };
        }
        let mut right = h_flex()
            .flex_1()
            .min_w_0()
            .justify_end()
            // Source m.handleClick excludes navRef (filter/order controls).
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        if self.view == ProfilesView::Games {
            for (id, label, state, keys) in [
                (
                    "profiles-filter",
                    "VIEWS",
                    &self.filter,
                    FILTER_KEYS.as_slice(),
                ),
                ("profiles-sort", "ORDER", &self.sort, SORT_KEYS.as_slice()),
            ] {
                right = right.child(
                    h_flex()
                        .gap(surface::css(10.))
                        .ml(surface::css(10.))
                        .child(div().text_size(surface::css(14.)).child(i18n::t(label)))
                        .child(
                            surface::select(state)
                                .id(id)
                                .items(choices(keys))
                                .accessibility_label(i18n::t(label))
                                .disabled(disabled)
                                .w(surface::css(160.)),
                        ),
                );
            }
        }
        h_flex()
            .id("profiles-main-nav")
            .w_full()
            .flex_shrink_0()
            .p(surface::css(11.))
            // 28px navigation + 11px top/bottom padding + 2px bottom border.
            .h(surface::css(52.))
            .when(disabled, |row| row.opacity(0.3))
            .border_b_2()
            .border_color(cx.theme().title_bar)
            .child(left)
            .child(self.navigation(cx))
            .child(right)
            .into_any_element()
    }
    fn add_tile(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = window.use_keyed_state("profiles-add-hover", cx, |_, _| false);
        let border = motion::transition(
            "profiles-add-border",
            if *state.read(cx) {
                cx.theme().primary
            } else {
                cx.theme().border
            },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        // `.game-tile`: 290x220, body 150, footer 70. The embedded profile
        // assignment surface's 240x190 `.linked-game-tile` is a different class.
        BaseButton::new("profiles-add-game-tile")
            .accessibility_label(i18n::t("ADD_GAME_TITLE"))
            .flex()
            .flex_col()
            .items_stretch()
            .justify_start()
            .p_0()
            .w(surface::css(290.))
            .h(surface::css(220.))
            .m(surface::css(10.))
            .flex_shrink_0()
            .border_2()
            .border_dashed()
            .border_color(border)
            .rounded(surface::css(5.))
            .bg(cx.theme().transparent)
            .focus_visible(|s| s.border_color(cx.theme().primary))
            .on_hover(window.listener_for(&state, |value, hovered, _, cx| {
                *value = *hovered;
                cx.notify();
            }))
            .active(|s| s.border_color(cx.theme().primary))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(surface::css(150.))
                    .flex_shrink_0()
                    .child(img("synapse/dashboard-add.svg").size(surface::css(40.))),
            )
            .child(
                div()
                    .h(surface::css(70.))
                    .flex_shrink_0()
                    .px(surface::css(35.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .text_center()
                    .text_color(cx.theme().foreground)
                    .child(format!(
                        "{} {}\n{}",
                        i18n::t("CLICK_TO_ADD"),
                        i18n::t("GAME_PROGRAM"),
                        i18n::t("DRAG_AND_DROP_HERE")
                    )),
            )
            .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx)))
            .into_any_element()
    }
}

impl Render for ProfilesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let removed = self
            .filter
            .read(cx)
            .selected_value()
            .is_some_and(|s| s == "REMOVED_GAMES");
        let mut grid = h_flex()
            .id("profiles-grid")
            .items_start()
            .flex_wrap()
            .when(self.view == ProfilesView::Games, |grid| {
                grid.min_w(surface::css(900.))
            })
            .px(surface::css(20.))
            .pb(surface::css(80.));
        if self.view == ProfilesView::Devices {
            grid = grid.children(self.device_tiles(cx));
        } else if !removed {
            grid = grid.child(self.add_tile(window, cx));
        }
        v_flex()
            .id("profiles-window")
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.search.read(cx).value().is_empty() {
                        this.searching = false;
                        cx.notify();
                    }
                }),
            )
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family("Roboto")
            .text_size(surface::css(16.))
            .track_focus(&self.focus)
            .child(self.toolbar(cx))
            .child(
                div()
                    .id("profiles-body")
                    .flex_1()
                    .min_h_0()
                    // Games .content-wrapper starts at 50px, overlapping the
                    // 52px absolute toolbar; Devices follows its nav normally.
                    .when(self.view == ProfilesView::Games, |body| {
                        body.mt(-surface::css(2.))
                    })
                    .scrollable_both()
                    .child(grid),
            )
            .children(self.add_dialog.clone())
            .children(self.device_dialog.clone())
    }
}

fn icon_button(id: &'static str, icon: &'static str, label: &'static str, cx: &App) -> BaseButton {
    let hover = icon.replace(".svg", "-hover.svg");
    BaseButton::new(id)
        .accessibility_label(i18n::t(label))
        .size(surface::css(26.))
        .group(id)
        .p_0()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .when(!icon.ends_with("scan.svg"), |button| {
            button.active(|s| s.opacity(0.7))
        })
        .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
        .child(
            div()
                .relative()
                .size(surface::css(20.))
                .child(img(icon).size_full())
                .child(
                    img(SharedString::from(hover))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .opacity(0.)
                        .group_hover(id, |s| s.opacity(1.)),
                ),
        )
}

fn search_field(
    id: &'static str,
    state: &Entity<InputState>,
    disabled: bool,
    on_clear: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    cx: &App,
) -> AnyElement {
    let clear = state.clone();
    div()
        .id(id)
        .relative()
        .w(surface::css(133.))
        .h(surface::css(20.))
        .bg(cx.theme().primary_foreground)
        .border_1()
        .border_color(cx.theme().border)
        .hover(|s| s.border_color(cx.theme().primary))
        .active(|s| s.border_color(cx.theme().primary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            Input::new(state)
                .appearance(false)
                .disabled(disabled)
                .w_full()
                .h_full()
                .p_0()
                .pl(surface::css(27.))
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .bg(cx.theme().primary_foreground)
                .border_0(),
        )
        .child(
            img("synapse/profiles-search-grey.svg")
                .absolute()
                .left(surface::css(3.))
                .top(surface::css(2.))
                .size(surface::css(20.)),
        )
        .when(!state.read(cx).value().is_empty(), |view| {
            view.child(
                icon_button(
                    "profiles-search-clear",
                    "synapse/profiles-clear.svg",
                    "CLEAR",
                    cx,
                )
                .absolute()
                .right_0()
                .top_0()
                .size(surface::css(25.))
                .disabled(disabled)
                .on_click(move |event, window, cx| {
                    clear.update(cx, |state, cx| {
                        state.set_value("", window, cx);
                        state.focus(window, cx);
                    });
                    if let Some(on_clear) = &on_clear {
                        on_clear(event, window, cx);
                    }
                }),
            )
        })
        .into_any_element()
}

// Module 3137 / 5529: the installed-program list is empty until its service
// supplies a response. Search and dismissal are local, scan/add are deferred.
struct AddGameDialog {
    open: bool,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    search: Entity<InputState>,
    searching: bool,
    from_device: bool,
    _search_subscription: Subscription,
}
impl AddGameDialog {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let return_focus = window.focused(cx);
        focus.focus(window, cx);
        let search = cx.new(|cx| InputState::new(window, cx));
        let search_subscription = cx.observe(&search, |_, _, cx| cx.notify());
        Self {
            open: true,
            focus,
            return_focus,
            search,
            searching: false,
            from_device: false,
            _search_subscription: search_subscription,
        }
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
}
impl Render for AddGameDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let opacity = Presence::new(
            (ElementId::from(("profiles-add", cx.entity_id())), "opacity"),
            true,
        )
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
        // Ua mounts both popups and switches them using .no-popup.
        let opacity = if self.from_device { 1. } else { opacity };
        let progress = Presence::new(
            (
                ElementId::from(("profiles-add", cx.entity_id())),
                "position",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
        let progress = if self.from_device { 1. } else { progress };
        let viewport = window.viewport_size();
        // The fixed backdrop uses the application viewport, below the 42px host.
        // Preserve @media conditions: max-width 800 only at <=900px.
        // The device popup is outside .profiles-link-games, and does not match
        // its margin-top 89/min-height 650/top 20 overrides.
        let unit = window.rem_size() / 16.;
        let app_height = viewport.height - unit * 42.;
        let backdrop_top = unit * (42. + if self.from_device { 0. } else { 89. });
        let popup_top = if self.from_device { 100. } else { 20. };
        let container_height = if self.from_device {
            app_height
        } else {
            app_height.max(unit * 650.)
        };
        let top = backdrop_top + unit * (110. + popup_top);
        let start = backdrop_top + unit * 110. + container_height;
        let animated_top = start + (top - start) * progress;
        let css_width = f32::from(viewport.width / unit);
        let width = unit * popup_width(css_width, self.from_device);
        let height = (container_height - unit * (110. + popup_top))
            .min(app_height - unit * 110.)
            .max(px(0.));
        let mut nav = h_flex()
            .w_full()
            .mb(surface::css(10.))
            .gap(surface::css(10.))
            .child(
                icon_button(
                    "profiles-program-refresh",
                    "synapse/profiles-refresh.svg",
                    "REFRESH",
                    cx,
                )
                .disabled(true)
                .tooltip(|window, cx| {
                    Tooltip::new("Installed-program service unavailable").build(window, cx)
                }),
            );
        nav = if self.searching {
            nav.child(search_field(
                "profiles-program-search-field",
                &self.search,
                false,
                None,
                cx,
            ))
        } else {
            nav.child(
                icon_button(
                    "profiles-program-search",
                    "synapse/profiles-search.svg",
                    "SEARCH",
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.searching = true;
                    this.search.update(cx, |input, cx| input.focus(window, cx));
                    cx.notify();
                })),
            )
        };
        nav = nav.child(
            h_flex()
                .flex_1()
                .justify_end()
                .items_end()
                .mr(surface::css(15.))
                .text_size(surface::css(14.))
                .child(i18n::t("STILL_DONT_SEE_YOUR_GAME"))
                .child(
                    BaseButton::new("profiles-program-browse")
                        .disabled(true)
                        .tooltip(|window, cx| {
                            Tooltip::new("Executable browse service unavailable").build(window, cx)
                        })
                        .p_0()
                        .underline()
                        .text_color(rgb(0xffffff))
                        .child(i18n::t("BROWSE")),
                ),
        );
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
                    .top(backdrop_top)
                    .bottom_0()
                    .bg(cx.theme().title_bar.opacity((128. / 255.) * opacity)),
            )
            .popup(
                v_flex()
                    .id("profiles-add-dialog")
                    .absolute()
                    .left((viewport.width - width) / 2.)
                    .top(animated_top)
                    .w(width)
                    .h(height)
                    .pb(surface::css(if self.from_device { 0. } else { 90. }))
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.search.read(cx).value().is_empty() {
                                this.searching = false;
                                cx.notify();
                            }
                        }),
                    )
                    .bg(cx.theme().background)
                    .rounded_t(surface::css(5.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(cx.theme().foreground)
                    .child(
                        div()
                            .relative()
                            .h(surface::css(36.))
                            .flex_shrink_0()
                            .text_center()
                            .overflow_hidden()
                            .pt(surface::css(20.))
                            .pb(surface::css(10.))
                            .px(surface::css(50.))
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .line_height(surface::css(19.))
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n::t("ADD_GAME_TITLE").to_uppercase())
                            .shadow(vec![BoxShadow {
                                color: rgb(0x5d5d5d).into(),
                                offset: point(px(0.), unit),
                                blur_radius: px(0.),
                                spread_radius: px(0.),
                                inset: false,
                            }])
                            .child(
                                controls::close_button("profiles-add-close", window, cx).on_click(
                                    cx.listener(|this, _, window, cx| this.close(window, cx)),
                                ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .pt(surface::css(20.))
                            .pl(surface::css(25.))
                            .pb(surface::css(42.))
                            .child(nav)
                            .child(
                                // 3137/f mounts `.content` with app rows only.
                                // No response means no rows; the unavailable
                                // service is explained on Refresh/Browse.
                                // Its goBack prop is not forwarded to 5529/r.
                                div()
                                    .id("profiles-installed-programs")
                                    .flex_1()
                                    .min_h_0()
                                    .scrollable_y(),
                            ),
                    ),
            )
            .into_any_element()
    }
}
