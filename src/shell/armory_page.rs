//! Current Armory 29770 root / 20540 navigation / 77989 feature hook.
//! See docs/re/armory-default-source.json for scoped source and CSS receipts.
use crate::{
    i18n,
    ui::{scroll::SourceScrollable as _, surface},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::motion::{Easing, Presence, Transition};
use gpui_kit::component::{
    input::{Input, InputEvent, InputState},
    *,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::{path::PathBuf, time::Duration};

mod share_profile;
use share_profile::{ShareClosed, ShareProfileDialog};

fn tr(key: &str) -> String {
    i18n::t(&format!("ARMORY_SOURCE.{key}"))
}

/// Armory module 95889 persists the introduction close under this exact
/// localStorage key. Keep the same default and lifetime in the native shell;
/// this is UI preference state and does not imply an Armory service session.
const INTRO_STORAGE_KEY: &str = "isShowArmoryIntroductionBanner";

fn intro_storage_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("razer_ui")
        .join(format!("{INTRO_STORAGE_KEY}.json"))
}

fn load_intro_visibility() -> bool {
    match std::fs::read_to_string(intro_storage_path()) {
        Ok(value) => serde_json::from_str::<bool>(&value).unwrap_or(true),
        Err(_) => true,
    }
}

fn save_intro_visibility(visible: bool) {
    let path = intro_storage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        serde_json::to_string(&visible).unwrap_or_else(|_| "true".into()),
    );
}

fn armory_option(
    id: &'static str,
    label: String,
    selected: bool,
    checkbox: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let tick_top = Presence::new((id, "tick-top"), selected)
        .transition(
            Transition::new(Duration::from_millis(100))
                .delay(Duration::from_millis(100))
                .easing(Easing::Ease),
        )
        .sample(window, cx)
        .progress;
    let tick_bottom = Presence::new((id, "tick-bottom"), selected)
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .w_full()
        .h(surface::css(27.))
        .pl(surface::css(if checkbox { 15. } else { 5. }))
        .pr(surface::css(5.))
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .flex()
        .items_center()
        .justify_start()
        .gap(surface::css(10.))
        .bg(rgb(0x000000))
        .text_color(if selected && !checkbox {
            cx.theme().primary
        } else {
            cx.theme().foreground
        })
        .hover(|style| style.bg(rgb(0x1a1a1a)))
        .when(checkbox, |button| {
            button.child(
                div()
                    .relative()
                    .size(surface::css(20.))
                    .flex_shrink_0()
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(if selected {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x737373)
                    })
                    .bg(if selected {
                        rgb(0x44d62c)
                    } else {
                        rgb(0x111111)
                    })
                    .when(selected, |view| {
                        view.child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let scale = f32::from(bounds.size.width) / 20.;
                                    // Current checkbox :before/:after and tickTop/tickBottom.
                                    for (x, y, angle, length) in [
                                        (8.6_f32, 16.4_f32, -145_f32, 15.4 * tick_top),
                                        (0.6, 10., -50., 9. * tick_bottom),
                                    ] {
                                        let angle = angle.to_radians();
                                        let start =
                                            bounds.origin + point(px(x * scale), px(y * scale));
                                        let mut path = PathBuilder::stroke(px(3. * scale));
                                        path.move_to(start);
                                        path.line_to(
                                            start
                                                + point(
                                                    px(-angle.sin() * length * scale),
                                                    px(angle.cos() * length * scale),
                                                ),
                                        );
                                        if let Ok(path) = path.build() {
                                            window.paint_path(path, rgb(0x111111));
                                        }
                                    }
                                },
                            )
                            .absolute()
                            .inset_0()
                            .size_full(),
                        )
                    }),
            )
        })
        .child(label)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArmoryTab {
    Spotlight,
    Browse,
    MyDownloads,
    MyUploads,
}
impl ArmoryTab {
    const ALL: [Self; 4] = [
        Self::Spotlight,
        Self::Browse,
        Self::MyDownloads,
        Self::MyUploads,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::Spotlight => "SPOTLIGHT_HEADER",
            Self::Browse => "BROWSE_HEADER",
            Self::MyDownloads => "MY_DOWNLOADS_HEADER",
            Self::MyUploads => "MY_UPLOADS_HEADER",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Spotlight => "armory-spotlight",
            Self::Browse => "armory-browse",
            Self::MyDownloads => "armory-downloads",
            Self::MyUploads => "armory-uploads",
        }
    }
}

pub(super) struct ArmoryPage {
    tab: ArmoryTab,
    focus: FocusHandle,
    history: Vec<ArmoryTab>,
    history_index: usize,
    phase1: bool,
    guest: bool,
    banner_open: bool,
    // The source keeps the filtering bar mounted for Browse and My Downloads
    // even while the service dataset is empty. Keep its local visual state
    // until the service-backed filter actions are connected.
    filter_open: bool,
    sort_open: bool,
    filter_selection: [bool; 9],
    sort_choice: [usize; 4],
    // Source module 68142 keeps a compact search affordance in the left side
    // of the shared navigation. The service query is intentionally not
    // fabricated locally: only the input/debounce shell is retained until the
    // Armory endpoint is connected.
    search: Entity<InputState>,
    search_open: bool,
    search_applied: String,
    search_pending: bool,
    search_generation: u64,
    search_task: Option<Task<()>>,
    share_profile: Option<Entity<ShareProfileDialog>>,
    share_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
impl ArmoryPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx));
        let search_subscription =
            cx.subscribe_in(&search, window, |this: &mut Self, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.schedule_search(window, cx);
                }
            });
        Self {
            // 29770 settles on Browse after 77989 finishes without feature data.
            tab: ArmoryTab::Browse,
            history: vec![ArmoryTab::Browse],
            history_index: 0,
            phase1: false,
            guest: true,
            banner_open: load_intro_visibility(),
            filter_open: false,
            sort_open: false,
            filter_selection: [true, false, false, false, false, false, false, false, false],
            sort_choice: [0; 4],
            search,
            search_open: false,
            search_applied: String::new(),
            search_pending: false,
            search_generation: 0,
            search_task: None,
            share_profile: None,
            share_subscription: None,
            _subscriptions: vec![search_subscription],
            focus: cx.focus_handle(),
        }
    }
    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dialog) = &self.share_profile {
            dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    /// Explicit local-profile entry; the guest Browse grid stays empty without
    /// remote contributions. The snapshot is retained only as a sharing draft.
    pub(super) fn open_share_profile(
        &mut self,
        device: crate::model::Device,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dialog = cx.new(|cx| ShareProfileDialog::new(device, window, cx));
        self.share_subscription =
            Some(
                cx.subscribe_in(&dialog, window, |this, _, _: &ShareClosed, window, cx| {
                    this.share_profile = None;
                    this.share_subscription = None;
                    this.focus.focus(window, cx);
                    cx.notify();
                }),
            );
        self.share_profile = Some(dialog);
        self.focus(window, cx);
    }
    fn visible(&self, tab: ArmoryTab) -> bool {
        (!self.guest || tab != ArmoryTab::MyUploads)
            && (self.phase1 || !matches!(tab, ArmoryTab::MyUploads | ArmoryTab::Spotlight))
    }
    pub(super) fn has_previous_page(&self) -> bool {
        self.history_index > 0
    }
    pub(super) fn has_next_page(&self) -> bool {
        self.history_index + 1 < self.history.len()
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
        self.tab = self.history[self.history_index];
        self.clear_search(window, cx);
        self.filter_open = false;
        self.sort_open = false;
        self.focus(window, cx);
    }
    fn set_tab(&mut self, tab: ArmoryTab, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab == tab || !self.visible(tab) {
            return;
        }
        self.tab = tab;
        self.clear_search(window, cx);
        self.filter_open = false;
        self.sort_open = false;
        self.history.truncate(self.history_index + 1);
        self.history.push(tab);
        self.history_index = self.history.len() - 1;
        cx.notify();
    }
    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_task = None;
        self.search_pending = false;
        self.search_applied.clear();
        self.search_open = false;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
    }
    fn schedule_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_generation = self.search_generation.wrapping_add(1);
        let generation = self.search_generation;
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.search_pending = !query.is_empty();
        self.search_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.search_generation != generation {
                    return;
                }
                this.search_applied = query;
                this.search_pending = false;
                this.search_task = None;
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn search_control(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.search_open {
            let clear = self.search.clone();
            return div()
                .id("armory-search-wrapper")
                .relative()
                .w(surface::css(200.))
                .h(surface::css(26.))
                .bg(cx.theme().primary_foreground)
                .border_1()
                .border_color(cx.theme().border)
                .hover(|style| style.border_color(cx.theme().primary))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Input::new(&self.search)
                        .appearance(false)
                        .w_full()
                        .h_full()
                        .p_0()
                        .pl(surface::css(27.))
                        .pr(surface::css(25.))
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
                .when(!self.search.read(cx).value().is_empty(), |view| {
                    view.child(
                        BaseButton::new("armory-search-clear")
                            .accessibility_label(i18n::t("CLEAR"))
                            .absolute()
                            .right_0()
                            .top_0()
                            .size(surface::css(25.))
                            .p_0()
                            .child(img("synapse/profiles-clear.svg").size(surface::css(20.)))
                            .on_click(move |_, window, cx| {
                                clear.update(cx, |state, cx| state.set_value("", window, cx));
                            }),
                    )
                })
                .into_any_element();
        }
        BaseButton::new("armory-search")
            .accessibility_label(i18n::t("SEARCH"))
            .size(surface::css(26.))
            .p_0()
            .flex()
            .items_center()
            .justify_center()
            .child(img("synapse/profiles-search.svg").size(surface::css(20.)))
            .on_click(cx.listener(|this, _, window, cx| {
                this.search_open = true;
                this.search.update(cx, |state, cx| state.focus(window, cx));
                cx.notify();
            }))
            .into_any_element()
    }
    fn filtering_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Source module 54408 mounts this bar for every view except Spotlight.
        // My uploads is hidden in the settled guest state, but keeping the
        // guard makes the feature-enabled branch follow that source contract.
        if self.tab == ArmoryTab::Spotlight {
            return div().into_any_element();
        }
        let filter_label = i18n::t("FILTER");
        let sort_label = i18n::t("SORT");
        let checked_count = self.filter_selection[1..8]
            .iter()
            .filter(|value| **value)
            .count();
        let filter_summary = if checked_count == 0 {
            i18n::t(if self.filter_selection[0] {
                "ALL_CONNECTED_DEVICES_TEXT"
            } else {
                "VIEW_ALL_ITEMS"
            })
        } else if self.filter_selection[0] || checked_count > 1 {
            // Module 49496 emits this literal rather than a locale key.
            format!("Filtered ({checked_count})")
        } else {
            let keys = [
                "LAPTOPS",
                "MICE",
                "KEYBOARDS",
                "HEADSETS",
                "SPEAKERS",
                "NEVER_DOWNLOADED",
                "ALREADY_DOWNLOADED",
            ];
            let selected = self.filter_selection[1..8]
                .iter()
                .position(|value| *value)
                .unwrap_or(0);
            i18n::t(keys[selected])
        };
        let mut controls = h_flex()
            .items_center()
            .gap(surface::css(18.))
            .px(surface::css(10.));
        // Module 54408 mounts its multi-select filter only for Browse. The
        // icon is module 70017 and the 180px box follows the source CSS.
        if self.tab == ArmoryTab::Browse {
            controls = controls.child(
                h_flex()
                    .items_center()
                    .child(
                        BaseButton::new("armory-filter")
                            .accessibility_label(filter_label.clone())
                            .size(surface::css(27.))
                            .p_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(if self.filter_open {
                                cx.theme().primary
                            } else {
                                cx.theme().transparent
                            })
                            .child(
                                svg()
                                    .path("synapse/armory-filter.svg")
                                    .size(surface::css(24.))
                                    .text_color(if self.filter_open {
                                        rgb(0x44d62c)
                                    } else {
                                        rgb(0xcccccc)
                                    }),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.filter_open = !this.filter_open;
                                this.sort_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        BaseButton::new("armory-filter-label")
                            .accessibility_label(filter_label)
                            .h(surface::css(27.))
                            .w(surface::css(180.))
                            .px(surface::css(6.))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(26.))
                            .text_left()
                            .border_1()
                            .border_color(if self.filter_open {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .bg(rgb(0x222222))
                            .text_color(cx.theme().foreground)
                            .child(filter_summary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.filter_open = !this.filter_open;
                                this.sort_open = false;
                                cx.notify();
                            })),
                    ),
            );
        }
        let mut bar = v_flex()
            .id("armory-filtering-bar")
            .relative()
            .items_end()
            .flex_shrink_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                controls.child(
                    BaseButton::new("armory-sort")
                        .accessibility_label(sort_label)
                        .size(surface::css(27.))
                        .p_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(if self.sort_open {
                            cx.theme().primary
                        } else {
                            cx.theme().transparent
                        })
                        .child(
                            svg()
                                .path("synapse/armory-sort.svg")
                                .size(surface::css(24.))
                                .text_color(if self.sort_open {
                                    rgb(0x44d62c)
                                } else {
                                    rgb(0xcccccc)
                                }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sort_open = !this.sort_open;
                            this.filter_open = false;
                            cx.notify();
                        })),
                ),
            );
        if self.tab == ArmoryTab::Browse && self.filter_open {
            let filter_items = [
                ("armory-filter-all", "ALL_CONNECTED_DEVICES_TEXT"),
                ("armory-filter-laptop", "LAPTOPS"),
                ("armory-filter-mice", "MICE"),
                ("armory-filter-keyboards", "KEYBOARDS"),
                ("armory-filter-headsets", "HEADSETS"),
                ("armory-filter-speakers", "SPEAKERS"),
                ("armory-filter-never-downloaded", "NEVER_DOWNLOADED"),
                ("armory-filter-already-downloaded", "ALREADY_DOWNLOADED"),
                ("armory-filter-all-items", "VIEW_ALL_ITEMS"),
            ];
            let mut options = v_flex()
                .id("armory-filter-options")
                .absolute()
                .top(surface::css(28.))
                .left(surface::css(29.))
                .w(surface::css(230.))
                .py(surface::css(4.))
                .bg(rgb(0x000000))
                .border_1()
                .border_color(rgb(0x5d5d5d));
            for (index, (id, key)) in filter_items.into_iter().enumerate() {
                if index == 1 || index == 6 || index == 8 {
                    options = options.child(
                        div()
                            .h(surface::css(1.))
                            .mx(surface::css(6.))
                            .my(surface::css(4.))
                            .bg(rgb(0x5d5d5d)),
                    );
                }
                let label = i18n::t(key);
                options = options.child(
                    armory_option(id, label, self.filter_selection[index], true, window, cx)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if index == 0 {
                                this.filter_selection[0] = !this.filter_selection[0];
                                this.filter_selection[8] =
                                    !this.filter_selection[..8].iter().any(|value| *value);
                            } else if index == 8 {
                                this.filter_selection = [false; 9];
                                this.filter_selection[8] = true;
                            } else {
                                this.filter_selection[index] = !this.filter_selection[index];
                                if index == 6 && this.filter_selection[index] {
                                    this.filter_selection[7] = false;
                                } else if index == 7 && this.filter_selection[index] {
                                    this.filter_selection[6] = false;
                                }
                                this.filter_selection[8] =
                                    !this.filter_selection[..8].iter().any(|value| *value);
                            }
                            this.filter_open = false;
                            cx.notify();
                        })),
                );
            }
            bar = bar.child(options);
        } else if self.sort_open {
            let sort_items = [
                ("armory-sort-likes", "MOST_LIKES"),
                ("armory-sort-downloads", "MOST_DOWNLOADS"),
                ("armory-sort-latest", "LATEST"),
                ("armory-sort-title", "TITLE"),
                ("armory-sort-creator", "CREATOR_NAME"),
                ("armory-sort-trending", "TEXT_TRENDING"),
            ];
            let mut options = v_flex()
                .id("armory-sort-options")
                .absolute()
                .top(surface::css(28.))
                .right(surface::css(-6.))
                .w(surface::css(181.))
                .bg(rgb(0x000000))
                .border_1()
                .border_color(rgb(0x5d5d5d));
            for (index, (id, key)) in sort_items.into_iter().enumerate() {
                let label = i18n::t(key);
                options = options.child(
                    armory_option(
                        id,
                        label,
                        self.sort_choice[self.tab as usize] == index,
                        false,
                        window,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sort_choice[this.tab as usize] = index;
                        this.sort_open = false;
                        cx.notify();
                    })),
                );
            }
            bar = bar.child(options);
        }
        bar.into_any_element()
    }
    fn banner(&self, cx: &mut Context<Self>) -> AnyElement {
        // 458 CSS: image cover/center, 20px 30px 0 margin, min-width 1220.
        div()
            .relative()
            .mt(surface::css(20.))
            .mx(surface::css(30.))
            .min_w(surface::css(1220.))
            .max_h(surface::css(150.))
            .rounded(surface::css(5.))
            .overflow_hidden()
            .child(
                img("synapse/armory-introduction.png")
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                v_flex()
                    .relative()
                    .items_center()
                    .justify_center()
                    .p(surface::css(20.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(24.))
                            .line_height(surface::css(30.))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(rgb(0x44d62c))
                            .text_center()
                            .mb(surface::css(15.))
                            // All source capabilities false -> isExchangeEnabled true.
                            .child(tr("EXCHANGE_GET_STARTED").to_uppercase()),
                    )
                    .children(
                        ["AI_MACRO_DESC", "MACRO_USAGE_DESC", "ASSIGN_MACRO_DESC"].map(|key| {
                            div()
                                .font_family("Roboto")
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(tr(key))
                        }),
                    ),
            )
            .child(
                BaseButton::new("armory-banner-close")
                    .absolute()
                    .right(surface::css(10.))
                    .top(surface::css(10.))
                    .p_0()
                    .size(surface::css(24.))
                    .accessibility_label(tr("CLOSE"))
                    .child(img("synapse/armory-banner-close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.banner_open = false;
                        save_intro_visibility(false);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
impl Render for ArmoryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex().w_full();
        if self.tab == ArmoryTab::Browse && self.banner_open {
            body = body.child(self.banner(cx));
        }
        // 86024 returns null for an empty service dataset. The former paragraphs
        // containing source keys and implementation notes were not product UI.
        v_flex()
            .id("armory-window")
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.filter_open = false;
                    this.sort_open = false;
                    if this.search.read(cx).value().is_empty() {
                        this.search_open = false;
                    }
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.filter_open = false;
                    this.sort_open = false;
                    if this.search_open {
                        this.clear_search(window, cx);
                    }
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(
                h_flex()
                    .w_full()
                    .h(surface::css(48.))
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(rgb(0))
                    .child(surface::nav_left().child(self.search_control(cx)))
                    .child(
                        h_flex()
                            .flex_grow(1.)
                            .flex_shrink_0()
                            .justify_center()
                            .gap(surface::css(20.))
                            .children(
                                ArmoryTab::ALL
                                    .into_iter()
                                    .filter(|tab| self.visible(*tab))
                                    .map(|tab| {
                                        surface::navigation_button(
                                            tab.id(),
                                            tr(tab.key()),
                                            self.tab == tab,
                                            cx,
                                        )
                                        .role(Role::Tab)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.set_tab(tab, window, cx)
                                            }),
                                        )
                                    }),
                            ),
                    )
                    .child(surface::nav_right().child(self.filtering_bar(window, cx))),
            )
            .child(
                div()
                    .id("armory-body")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(body),
            )
            .children(self.share_profile.clone())
    }
}
