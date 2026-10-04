//! Current Profiles 1255 embedded root: fixed profile, game tiles and program browser.
//! Local executable metadata never starts, scans or monitors a program.
use crate::{
    features::Choice,
    i18n,
    model::Device,
    ui::{scroll::SourceScrollable as _, surface},
};
use gpui_kit::base::{
    Button as BaseButton,
    motion::{Easing, Presence, Transition},
};
use gpui_kit::component::{
    input::{Input, InputState},
    select::SelectState,
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{path::PathBuf, time::Duration};

const FILTERS: [&str; 3] = ["ALL_GAMES", "LINKED_GAMES", "REMOVED_GAMES"];
const SORTS: [&str; 4] = ["NAME_A_TO_Z", "NAME_Z_TO_A", "LAST_PLAYED", "MOST_PLAYED"];

pub(super) enum ProfileLinkedGamesEvent {
    Close,
    Link {
        name: String,
        executable: String,
        linked: bool,
    },
}

#[derive(Clone)]
struct Game {
    name: String,
    executable: String,
    profile: Option<(String, String)>,
}

fn path_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}
fn options(keys: &[&str]) -> Vec<Choice> {
    keys.iter()
        .map(|key| Choice::new(*key, i18n::t(key)))
        .collect()
}
fn css(value: f32) -> Rems {
    surface::css(value)
}

pub(super) struct ProfileLinkedGames {
    focus: FocusHandle,
    profile_id: String,
    profile_name: String,
    games: Vec<Game>,
    filter: Entity<SelectState<Vec<Choice>>>,
    sort: Entity<SelectState<Vec<Choice>>>,
    search: Entity<InputState>,
    searching: bool,
    adding: bool,
    returned_from_add: bool,
    busy: bool,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<ProfileLinkedGamesEvent> for ProfileLinkedGames {}

impl ProfileLinkedGames {
    pub(super) fn new(device: Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let profile_id = device.active_profile.clone();
        let profile_name = device
            .active_profile_obj()
            .map(|profile| profile.name.clone())
            .unwrap_or_default();
        let mut games = Vec::<Game>::new();
        for profile in &device.profiles {
            if let Some(settings) = &profile.settings {
                for game in &settings.linked_games {
                    if !games
                        .iter()
                        .any(|known| path_key(&known.executable) == path_key(&game.executable))
                    {
                        games.push(Game {
                            name: game.name.clone(),
                            executable: game.executable.clone(),
                            profile: Some((profile.id.clone(), profile.name.clone())),
                        });
                    }
                }
            }
        }
        let filter =
            cx.new(|cx| SelectState::new(options(&FILTERS), Some(IndexPath::new(0)), window, cx));
        let sort =
            cx.new(|cx| SelectState::new(options(&SORTS), Some(IndexPath::new(0)), window, cx));
        let search = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.observe(&filter, |_, _, cx| cx.notify()),
            cx.observe(&sort, |_, _, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
        ];
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            focus,
            profile_id,
            profile_name,
            games,
            filter,
            sort,
            search,
            searching: false,
            adding: false,
            returned_from_add: false,
            busy: false,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.adding {
            self.adding = false;
            self.returned_from_add = true;
            self.clear_search(window, cx);
            self.focus.focus(window, cx);
        } else {
            cx.emit(ProfileLinkedGamesEvent::Close);
        }
        cx.notify();
    }
    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.searching = false;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
    }
    fn open_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.adding = true;
        self.error = None;
        self.clear_search(window, cx);
        cx.notify();
    }

    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select an .exe file".into()),
        });
        cx.spawn_in(window, async move |owner, cx| {
            let result = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next().map(Ok),
                Ok(Ok(None)) => None,
                _ => Some(Err("Unable to open the file browser.".to_owned())),
            };
            let result = match result {
                Some(Ok(path)) => Some(
                    cx.background_executor()
                        .spawn(async move { read_game_path(path) })
                        .await,
                ),
                Some(Err(error)) => Some(Err(error)),
                None => None,
            };
            let _ =
                owner.update_in(cx, |this, window, cx| {
                    this.busy = false;
                    match result {
                        Some(Ok(game)) => {
                            if !this.games.iter().any(|known| {
                                path_key(&known.executable) == path_key(&game.executable)
                            }) {
                                this.games.push(game);
                            }
                            this.adding = false;
                            this.returned_from_add = true;
                            this.clear_search(window, cx);
                            this.focus.focus(window, cx);
                        }
                        Some(Err(error)) => this.error = Some(error),
                        None => {}
                    }
                    cx.notify();
                });
        })
        .detach();
        cx.notify();
    }

    fn toggle_game(&mut self, executable: &str, cx: &mut Context<Self>) {
        let Some(game) = self
            .games
            .iter_mut()
            .find(|game| path_key(&game.executable) == path_key(executable))
        else {
            return;
        };
        let linked = !game
            .profile
            .as_ref()
            .is_some_and(|(id, _)| id == &self.profile_id);
        game.profile = linked.then(|| (self.profile_id.clone(), self.profile_name.clone()));
        cx.emit(ProfileLinkedGamesEvent::Link {
            name: game.name.clone(),
            executable: game.executable.clone(),
            linked,
        });
        cx.notify();
    }

    fn icon(&self, id: &'static str, image: &'static str, label: &'static str) -> BaseButton {
        BaseButton::new(id)
            .accessibility_label(i18n::t(label))
            .p_0()
            .size(css(26.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .hover(|s| s.bg(gpui_kit::rgba(0xffffff1a)))
            .active(|s| s.opacity(0.7))
            .child(img(image).size(css(20.)))
    }

    fn search_control(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.searching {
            return self
                .icon(
                    "profile-games-search",
                    "synapse/profiles-search.svg",
                    "SEARCH",
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.searching = true;
                    this.search.update(cx, |state, cx| state.focus(window, cx));
                    cx.notify();
                }))
                .into_any_element();
        }
        div()
            .relative()
            .w(css(133.))
            .h(css(20.))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .bg(rgb(0x111111))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Input::new(&self.search)
                    .appearance(false)
                    .w_full()
                    .h_full()
                    .p_0()
                    .pl(css(27.))
                    .pr(css(20.))
                    .bg(rgb(0x111111))
                    .border_0()
                    .text_size(css(14.))
                    .line_height(css(17.)),
            )
            .child(
                img("synapse/profiles-search-grey.svg")
                    .absolute()
                    .left(css(3.))
                    .top(css(2.))
                    .size(css(20.)),
            )
            .when(!self.search.read(cx).value().is_empty(), |view| {
                view.child(
                    self.icon(
                        "profile-games-search-clear",
                        "synapse/profiles-clear.svg",
                        "CLEAR",
                    )
                    .absolute()
                    .top_0()
                    .right_0()
                    .size(css(20.))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_search(window, cx);
                        cx.notify();
                    })),
                )
            })
            .into_any_element()
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut bar = h_flex().w_full().h(css(26.)).mb(css(10.)).gap(css(10.));
        if !self.adding {
            bar = bar.child(
                self.icon(
                    "profile-games-add",
                    "synapse/profiles-add.svg",
                    "ADD_GAME_TITLE",
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_add(window, cx))),
            );
        }
        bar = bar
            .child(
                self.icon(
                    "profile-games-refresh",
                    if self.adding {
                        "synapse/profiles-refresh.svg"
                    } else {
                        "synapse/profiles-scan.svg"
                    },
                    "REFRESH",
                )
                .disabled(true)
                .tooltip(|window, cx| {
                    Tooltip::new("Game discovery service unavailable").build(window, cx)
                }),
            )
            .child(self.search_control(cx));
        if self.adding {
            bar = bar.child(
                h_flex()
                    .flex_1()
                    .justify_end()
                    .mr(css(15.))
                    .gap(css(10.))
                    .child(i18n::t("STILL_DONT_SEE_YOUR_GAME"))
                    .child(
                        BaseButton::new("profile-games-browse")
                            .p_0()
                            .disabled(self.busy)
                            .child(i18n::t("BROWSE"))
                            .on_click(cx.listener(|this, _, window, cx| this.browse(window, cx))),
                    ),
            );
        } else {
            let mut right = h_flex().ml_auto().mr(css(18.));
            for (label, state, keys) in [
                ("VIEWS", &self.filter, FILTERS.as_slice()),
                ("ORDER", &self.sort, SORTS.as_slice()),
            ] {
                right = right.child(
                    h_flex()
                        .gap(css(10.))
                        .ml(css(10.))
                        .child(i18n::t(label))
                        .child(
                            surface::select(state)
                                .items(options(keys))
                                .accessibility_label(i18n::t(label))
                                .w(css(160.)),
                        ),
                );
            }
            bar = bar.child(right);
        }
        bar.into_any_element()
    }

    fn tile(&self, game: Game, cx: &mut Context<Self>) -> AnyElement {
        let active = game
            .profile
            .as_ref()
            .is_some_and(|(id, _)| id == &self.profile_id);
        let busy = game.profile.is_some() && !active;
        let executable = game.executable.clone();
        BaseButton::new(SharedString::from(format!(
            "profile-game-{}",
            game.executable
        )))
        .accessibility_label(game.name.clone())
        .selected(active)
        .group("profile-game-tile")
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
        .hover(|s| s.border_color(rgb(0x44d62c)))
        .when(busy, |tile| {
            tile.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(css(40.))
                    .bg(gpui_kit::rgba(0x11111180))
                    .pl(css(40.))
                    .pt(css(12.))
                    .text_color(rgb(0x44d62c))
                    .truncate()
                    .child(
                        game.profile
                            .as_ref()
                            .map(|(_, name)| name.clone())
                            .unwrap_or_default(),
                    ),
            )
        })
        .child(
            div()
                .absolute()
                .left(css(10.))
                .top(css(10.))
                .size(css(20.))
                .border_1()
                .border_color(if active { rgb(0x44d62c) } else { rgb(0x5d5d5d) })
                .bg(if active { rgb(0x44d62c) } else { rgb(0x111111) })
                .when(active, |check| {
                    check.child(
                        div()
                            .size_full()
                            .text_center()
                            .text_size(css(16.))
                            .text_color(rgb(0x111111))
                            .child("✓"),
                    )
                })
                .when(busy, |check| {
                    check.child(
                        div()
                            .absolute()
                            .left(css(4.))
                            .top(css(8.))
                            .w(css(10.))
                            .h(css(2.))
                            .bg(rgb(0x44d62c)),
                    )
                }),
        )
        .child(div().w_full().h(css(120.)).flex_shrink_0())
        .child(
            div()
                .w_full()
                .h(css(70.))
                .p(css(10.))
                .text_center()
                .text_size(css(14.))
                .line_height(css(19.))
                .group_hover("profile-game-tile", |s| s.text_color(rgb(0x44d62c)))
                .child(div().truncate().child(game.name)),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_game(&executable, cx)))
        .into_any_element()
    }

    fn tiles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let filter = self
            .filter
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_default();
        let search = self.search.read(cx).value().trim().to_lowercase();
        let mut games: Vec<_> = self
            .games
            .iter()
            .filter(|game| {
                filter != "REMOVED_GAMES"
                    && (filter != "LINKED_GAMES" || game.profile.is_some())
                    && game.name.to_lowercase().contains(&search)
            })
            .cloned()
            .collect();
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
        let mut tiles: Vec<_> = games.into_iter().map(|game| self.tile(game, cx)).collect();
        tiles.push(crate::ui::game_tile::add_new_tile(
            "profile-games-add-tile",
            format!("{} {}", i18n::t("CLICK_TO_ADD"), i18n::t("GAME_PROGRAM")),
            cx.listener(|this, _, window, cx| this.open_add(window, cx)),
            cx,
        ));
        tiles
    }
}

fn read_game_path(path: PathBuf) -> Result<Game, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Unable to read the selected file: {error}"))?;
    if !canonical.is_file()
        || !canonical
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err("Select an .exe file.".into());
    }
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if name.is_empty() {
        return Err("The selected file has no program name.".into());
    }
    Ok(Game {
        name,
        executable: canonical.to_string_lossy().into_owned(),
        profile: None,
    })
}

impl Render for ProfileLinkedGames {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opacity = Presence::new(
            (
                ElementId::from(("profile-linked-games", cx.entity_id())),
                "opacity",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Linear))
        .sample(window, cx)
        .progress;
        let progress = Presence::new(
            (
                ElementId::from(("profile-linked-games", cx.entity_id())),
                "position",
            ),
            true,
        )
        .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
        let (opacity, progress) = if self.adding || self.returned_from_add {
            (1., 1.)
        } else {
            (opacity, progress)
        };
        let viewport = window.viewport_size();
        let unit = window.rem_size() / 16.;
        let viewport_width = f32::from(viewport.width / unit);
        let width = unit
            * if viewport_width >= 1600. && !self.adding {
                1300.
            } else {
                (viewport_width - 40.)
                    .min(if viewport_width <= 900. { 800. } else { 1050. })
                    .max(0.)
            };
        let top = unit * (42. + 110. + 100.);
        let start = viewport.height + unit * 110.;
        let title = if self.adding {
            i18n::t("ADD_GAME_TITLE").to_uppercase()
        } else {
            format!("{} {}", i18n::t("LINKED_GAMES_TO"), self.profile_name).to_uppercase()
        };
        let mut body = v_flex()
            .flex_1()
            .min_h_0()
            .pt(css(20.))
            .pl(css(25.))
            .pb(css(42.))
            .child(self.toolbar(cx));
        if self.adding {
            body = body.child(
                div()
                    .id("profile-installed-applications")
                    .flex_1()
                    .min_h_0()
                    .scrollable_y(),
            );
        } else {
            body = body.child(
                div()
                    .id("profile-game-tiles")
                    .flex_1()
                    .min_h_0()
                    .scrollable_y()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .mr(css(-5.))
                            .pb(css(25.))
                            .children(self.tiles(cx)),
                    ),
            );
        }
        body = body.when_some(self.error.clone(), |body, error| {
            body.child(div().pr(css(25.)).text_color(rgb(0xfd4949)).child(error))
        });
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
                    .when(!self.adding, |view| {
                        view.child(
                            img("synapse/profiles-glow.svg")
                                .absolute()
                                .bottom_0()
                                .left((viewport.width - unit * 960.) / 2.)
                                .w(unit * 960.)
                                .h(unit * 500.),
                        )
                    }),
            )
            .popup(
                v_flex()
                    .id("source-profile-linked-games")
                    .absolute()
                    .left((viewport.width - width) / 2.)
                    .top(start + (top - start) * progress)
                    .w(width)
                    .h((viewport.height - top).max(px(0.)))
                    .bg(rgb(0x222222))
                    .rounded_t(css(5.))
                    .occlude()
                    .text_size(css(14.))
                    .line_height(css(17.))
                    .text_color(rgb(0xcccccc))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.search.read(cx).value().is_empty() {
                                this.searching = false;
                                cx.notify();
                            }
                        }),
                    )
                    .child(
                        h_flex()
                            .relative()
                            .w_full()
                            .h(css(36.))
                            .flex_shrink_0()
                            .justify_center()
                            .pt(css(20.))
                            .pb(css(10.))
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .line_height(css(19.))
                            .text_color(rgb(0x999999))
                            .child(title)
                            .child(
                                BaseButton::new("profile-games-close")
                                    .accessibility_label(i18n::t("CLOSE"))
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .size(css(36.))
                                    .p_0()
                                    .hover(|s| s.bg(gpui_kit::rgba(0xffffff1a)))
                                    .child(img("synapse/profiles-close.svg").size(css(20.)))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.close(window, cx)),
                                    ),
                            ),
                    )
                    .child(body),
            )
            .into_any_element()
    }
}
