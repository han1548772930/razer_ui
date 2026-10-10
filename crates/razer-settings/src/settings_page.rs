//! Current Settings app 720/ho/uo, independently verified on 2026-10-03.
use razer_widgets::scroll::SourceScrollable as _;
#[path = "settings_lighting.rs"]
mod lighting;
use razer_widgets::settings_button::settings_button;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
use gpui_kit::component::{
    button::Button,
    select::{SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_pages::features::Choice;
use razer_state::AppPreferences;
use razer_state::LANGUAGES;
use razer_state::RECOMMENDATION_CATEGORIES;
use razer_widgets::surface;
use razer_widgets::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Synapse,
    General,
    Connection,
}

/// Source-catalog editions, or the existing local default when no edition is
/// declared. An unregistered product never becomes a preview device.
pub fn preview_editions(pid: u32) -> Vec<u32> {
    let Some(product) = razer_catalog::registered(pid) else {
        return Vec::new();
    };
    if product.edition_ids().is_empty() {
        vec![0]
    } else {
        product.edition_ids().to_vec()
    }
}

pub fn preview_layouts(pid: u32) -> Vec<u32> {
    if razer_catalog::registered(pid).is_none() {
        return Vec::new();
    }
    if pid != 653 {
        // Other product renderers still own their source default layout. Do
        // not offer a 653 layout to a product with a different input contract.
        return vec![0];
    }
    keyboard_preview_layouts()
        .iter()
        .map(|layout| layout.layout_id)
        .collect()
}

#[derive(serde::Deserialize)]
struct PreviewLayout {
    layout_id: u32,
    layout_name: String,
}

fn keyboard_preview_layouts() -> &'static [PreviewLayout] {
    #[derive(serde::Deserialize)]
    struct Catalog {
        layouts: Vec<PreviewLayout>,
    }
    static LAYOUTS: std::sync::OnceLock<Vec<PreviewLayout>> = std::sync::OnceLock::new();
    LAYOUTS.get_or_init(|| {
        serde_json::from_str::<Catalog>(include_str!(
            "../../../assets/synapse/keyboard-653-customize-layouts.json"
        ))
        .expect("audited current 653 layouts")
        .layouts
    })
}

fn preview_product_choices() -> Vec<Choice> {
    razer_catalog::registry()
        .iter()
        .map(|product| {
            Choice::new(
                product.id().to_string(),
                razer_model::demo::preview_product_label(product.id()),
            )
        })
        .collect()
}

pub enum SettingsEvent {
    Changed,
    Language,
    ResetTutorials,
    Preview(u32),
    PreviewVariant(u32, u32, u32),
    ChromaTour,
    Alexa,
    AppPicker,
    Modules,
    Dashboard,
    ProfileMigration,
    ReleaseNotes,
    Pairing,
}
pub struct SettingsPage {
    values: AppPreferences,
    saved: AppPreferences,
    page: Page,
    language: Entity<SelectState<Vec<Choice>>>,
    preview_product: Entity<SelectState<Vec<Choice>>>,
    preview_edition: Entity<SelectState<Vec<Choice>>>,
    preview_layout: Entity<SelectState<Vec<Choice>>>,
    runtime: Entity<super::runtime_page::RuntimePanel>,
    tutorial_reset: bool,
    storage_error: Option<String>,
    dynamic_lighting_supported: bool,
    subscriptions: Vec<Subscription>,
}
impl EventEmitter<SettingsEvent> for SettingsPage {}
impl SettingsPage {
    pub fn tray_double_click(&self) -> razer_state::TrayDoubleClickAction {
        self.values
            .systray_double_click
            .unwrap_or(razer_state::TrayDoubleClickAction::ShowMenu)
    }
    pub fn set_tray_double_click(
        &mut self,
        value: razer_state::TrayDoubleClickAction,
        cx: &mut Context<Self>,
    ) {
        self.values.systray_double_click = Some(value);
        self.changed(cx);
    }
    /// Shared locale owner for the independent current `/settings/` window.
    pub fn set_language(&mut self, language: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some((code, _)) = LANGUAGES
            .iter()
            .find(|(code, _)| code.eq_ignore_ascii_case(language))
        else {
            return;
        };
        if self.values.language.eq_ignore_ascii_case(code) {
            return;
        }
        self.values.language = (*code).into();
        self.language.update(cx, |state, cx| {
            state.set_selected_value(&self.values.language, window, cx)
        });
        i18n::set_locale(code);
        self.refresh_preview_labels(window, cx);
        cx.emit(SettingsEvent::Language);
        self.changed(cx);
    }
    pub fn new(
        mut values: AppPreferences,
        runtime: Entity<super::runtime_page::RuntimePanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved = values.clone();
        let locale = i18n::locale();
        if let Some((code, _)) = LANGUAGES
            .iter()
            .find(|(code, _)| code.eq_ignore_ascii_case(&locale))
        {
            values.language = (*code).into();
        }
        let language = cx.new(|cx| {
            SelectState::new(
                LANGUAGES
                    .iter()
                    .map(|(code, label)| Choice::new(*code, *label))
                    .collect::<Vec<_>>(),
                None,
                window,
                cx,
            )
        });
        language.update(cx, |state, cx| {
            state.set_selected_value(&values.language, window, cx)
        });
        let preview_product = cx.new(|cx| {
            SelectState::new(
                preview_product_choices(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
            .searchable(true)
        });
        let preview_edition = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let preview_layout = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        let mut this = Self {
            saved,
            values,
            page: Page::Synapse,
            language,
            preview_product,
            preview_edition,
            preview_layout,
            runtime,
            tutorial_reset: false,
            storage_error: None,
            dynamic_lighting_supported: razer_platform::system::supports_dynamic_lighting(),
            subscriptions: vec![],
        };
        this.subscriptions.push(cx.subscribe_in(
            &this.language,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(language)) = event {
                    this.values.language = language.clone();
                    i18n::set_locale(language);
                    this.refresh_preview_labels(window, cx);
                    cx.emit(SettingsEvent::Language);
                    this.changed(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.preview_product,
            window,
            |this, _, event: &SelectEvent<Vec<Choice>>, window, cx| {
                if matches!(event, SelectEvent::Confirm(_)) {
                    this.sync_preview_variant(window, cx);
                }
            },
        ));
        this.subscriptions
            .push(cx.observe(&this.preview_edition, |_, _, cx| cx.notify()));
        this.subscriptions
            .push(cx.observe(&this.preview_layout, |_, _, cx| cx.notify()));
        this.sync_preview_variant(window, cx);
        this
    }
    fn refresh_preview_labels(&self, window: &mut Window, cx: &mut Context<Self>) {
        let selected =
            |state: &Entity<SelectState<Vec<Choice>>>| state.read(cx).selected_value().cloned();
        let product = selected(&self.preview_product);
        let edition = selected(&self.preview_edition);
        let layout = selected(&self.preview_layout);
        self.preview_product.update(cx, |state, cx| {
            state.set_items(preview_product_choices(), window, cx);
            if let Some(product) = product {
                state.set_selected_value(&product, window, cx);
            }
        });
        self.sync_preview_variant(window, cx);
        for (state, selected) in [
            (&self.preview_edition, edition),
            (&self.preview_layout, layout),
        ] {
            if let Some(selected) = selected {
                state.update(cx, |state, cx| {
                    state.set_selected_value(&selected, window, cx);
                });
            }
        }
    }
    fn sync_preview_variant(&self, window: &mut Window, cx: &mut Context<Self>) {
        let pid = self
            .preview_product
            .read(cx)
            .selected_value()
            .and_then(|value| value.parse().ok());
        let choices = |values: Vec<u32>| {
            values
                .into_iter()
                .map(|value| {
                    Choice::new(
                        value.to_string(),
                        if value == 0 {
                            "默认 (0)".into()
                        } else {
                            value.to_string()
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let layouts = if pid == Some(653) {
            keyboard_preview_layouts()
                .iter()
                .map(|layout| {
                    Choice::new(
                        layout.layout_id.to_string(),
                        format!("{} · {}", layout.layout_id, layout.layout_name),
                    )
                })
                .collect()
        } else {
            choices(pid.map(preview_layouts).unwrap_or_default())
        };
        for (state, items) in [
            (
                &self.preview_edition,
                pid.map(|pid| {
                    preview_editions(pid)
                        .into_iter()
                        .map(|edition| {
                            Choice::new(
                                edition.to_string(),
                                razer_model::demo::preview_edition_label(pid, edition),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            ),
            (&self.preview_layout, layouts),
        ] {
            state.update(cx, |state, cx| {
                let first = (!items.is_empty()).then_some(IndexPath::new(0));
                state.set_items(items, window, cx);
                state.set_selected_index(first, window, cx);
            });
        }
        cx.notify();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }
    pub fn dirty(&self) -> bool {
        self.values != self.saved
    }
    pub fn snapshot(&self) -> AppPreferences {
        self.values.clone()
    }
    pub fn mark_saved(&mut self, snapshot: AppPreferences, cx: &mut Context<Self>) {
        self.saved = snapshot;
        cx.notify();
    }
    pub fn tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.values.gamer_room_tutorial_seen = seen;
        if seen {
            self.tutorial_reset = false;
        }
        self.changed(cx);
    }
    pub fn tutorial_viewed(&mut self, cx: &mut Context<Self>) {
        self.tutorial_reset = false;
        cx.notify();
    }
    pub fn profile_migration_icon_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.values.profile_migration_icon_visible = visible;
        self.changed(cx);
    }
    pub fn dashboard_tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.values.dashboard_tutorial_seen = seen;
        if seen {
            self.tutorial_reset = false;
        }
        self.changed(cx);
    }
    pub fn set_persistence_state(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.storage_error = error;
        cx.notify();
    }
    fn panel(&self, title: &str, cx: &App) -> Div {
        self.panel_with_control(title, div(), cx)
    }
    fn panel_with_control(&self, title: &str, control: impl IntoElement, cx: &App) -> Div {
        // Settings overrides the shared widget title to 18px (720 CSS).
        v_flex()
            .w(surface::css(600.))
            .min_w(surface::css(600.))
            .flex_shrink_0()
            .mx_auto()
            .my(surface::css(10.))
            .py(surface::css(surface::WIDGET_PADDING_Y))
            .px(surface::css(surface::WIDGET_PADDING_X))
            .bg(cx.theme().group_box)
            .rounded(surface::css(surface::WIDGET_RADIUS))
            .text_size(surface::css(14.))
            .gap(surface::css(20.))
            .child(
                h_flex()
                    .gap(surface::css(10.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(18.))
                            .text_color(cx.theme().primary)
                            .child(i18n::t(title).to_uppercase()),
                    )
                    .child(control),
            )
    }
    fn synapse(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let startup = self.values.startup_draft.unwrap_or_default();
        let column_margin = settings_column_margin(window);
        h_flex()
            .items_start()
            .flex_wrap()
            .w_full()
            .max_w(surface::css(1240.))
            .mx_auto()
            .justify_center()
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .min_w(surface::css(600.))
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .flex_basis(surface::css(600.))
                    .mx(column_margin)
                    .child(
                        self.panel("AUTO_LAUNCH", cx).child(
                            v_flex()
                                .child(
                                    settings_check("settings-auto-start", i18n::t("START_SYNAPSE"), startup.auto_start, false, window, cx)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let draft = this
                                                .values
                                                .startup_draft
                                                .get_or_insert_with(Default::default);
                                            draft.auto_start = !draft.auto_start;
                                            this.changed(cx);
                                        })),
                                )
                                .child(
                                    v_flex()
                                        .relative()
                                        .pl(surface::css(30.))
                                        .child(
                                            div()
                                                .absolute()
                                                .left(surface::css(9.))
                                                .top_0()
                                                .w(surface::css(1.))
                                                .h(surface::css(37.))
                                                .bg(theme::SettingsColors::tree_note()),
                                        )
                                        .child(
                                            div()
                                                .absolute()
                                                .left(surface::css(10.))
                                                .top(surface::css(36.))
                                                .w(surface::css(13.))
                                                .h(surface::css(1.))
                                                .bg(theme::SettingsColors::tree_note()),
                                        )
                                        .child(
                                            div()
                                                .mb(surface::css(10.))
                                                .text_color(theme::SettingsColors::tree_note())
                                                .line_height(surface::css(17.))
                                                .child(i18n::t("NOTE_DISABLE_SYNAPSE")),
                                        )
                                        .child(
                                            settings_check("settings-start-minimized", i18n::t("MINIMIZE_SYSTRAY"), startup.start_minimized, !startup.auto_start, window, cx)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if !this.values.startup_draft.unwrap_or_default().auto_start {
                                                        return;
                                                    }
                                                    let draft = this
                                                        .values
                                                        .startup_draft
                                                        .get_or_insert_with(Default::default);
                                                    draft.start_minimized = !draft.start_minimized;
                                                    this.changed(cx);
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("settings-startup-local-note")
                                        .test_support()
                                        .mt(surface::css(10.))
                                        .text_size(surface::css(12.))
                                        .text_color(theme::SettingsColors::tree_note())
                                        .aria_label("自动启动为本地设置草稿；系统状态未读取，尚未应用到宿主")
                                        .child("自动启动为本地设置草稿；系统状态未读取，尚未应用到宿主"),
                                ),
                        ),
                    )
                    .child(
                        self.panel("NOTIFICATIONS", cx).child(
                            div()
                                .relative()
                                .child(
                                    settings_check("settings-notifications", i18n::t("DISPLAY_NOTIFICATIONS"), self.values.notifications, false, window, cx)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.values.notifications = !this.values.notifications;
                                            this.changed(cx);
                                        })),
                                )
                                .child(
                                    settings_help(
                                        "settings-notifications-help",
                                        i18n::t("NOTIFICATIONS"),
                                        format!(
                                            "{}\n• {}\n• {}\n• {}",
                                            i18n::t("NOTIFICATIONS_TOOLTIP"),
                                            i18n::t("NOTIFICATIONS_TOOLTIP_DESC1"),
                                            i18n::t("NOTIFICATIONS_TOOLTIP_DESC2"),
                                            i18n::t("NOTIFICATIONS_TOOLTIP_DESC3")
                                        ),
                                    )
                                    .absolute()
                                    .left(surface::css(226.))
                                    .top(surface::css(4.)),
                                ),
                        ),
                    )
                    .child(self.recommendations(window, cx)),
            )
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .min_w(surface::css(600.))
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .flex_basis(surface::css(600.))
                    .mx(column_margin)
                    .child(
                        self.panel("TUTORIAL_RESET", cx).child(
                            h_flex()
                                .gap(surface::css(20.))
                                .items_center()
                                .child(
                                    settings_button(
                                        "settings-reset-tutorials",
                                        i18n::t("RESET"),
                                        self.tutorial_reset,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.tutorial_reset = true;
                                            cx.emit(SettingsEvent::ResetTutorials);
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(div().flex_1().child(i18n::t("SYNAPSE_TUTORIAL_RESET_MSG"))),
                        ),
                    )
                    .child(
                        self.panel("PROFILE_MIGRATION", cx).child(
                            h_flex()
                                .items_center()
                                .gap(surface::css(20.))
                                .child(
                                    settings_button(
                                        "settings-migration",
                                        i18n::t("LAUNCH"),
                                        false,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |_, _, _, cx| {
                                            cx.emit(SettingsEvent::ProfileMigration);
                                        },
                                    )),
                                )
                                .child(div().flex_1().child(i18n::t("PROFILE_MIGRATION_DESC"))),
                        ),
                    )
                    .when(self.dynamic_lighting_supported, |column| {
                        column.child(
                            self.panel("DEVICE_LIGHTING", cx)
                                .relative()
                                .child(
                                    settings_help(
                                        "settings-lighting-help",
                                        i18n::t("DEVICE_LIGHTING"),
                                        i18n::t("DEVICE_LIGHTING_TIPS"),
                                    )
                                    .absolute()
                                    .top(surface::css(10.))
                                    .right(surface::css(10.)),
                                )
                                .child(lighting::content(None, false, |_, _, _| {}, cx)),
                        )
                    }),
            )
            .into_any_element()
    }
    fn recommendations(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.panel_with_control(
            "RECOMMENDATION_HEADER",
            div().relative().top(surface::css(3.)).child(
                surface::SynapseSwitch::new("settings-recommendations")
                    .accessibility_label(i18n::t("RECOMMENDATION_HEADER"))
                    .checked(self.values.recommendations)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.values.recommendations = *checked;
                        this.changed(cx);
                    })),
            ),
            cx,
        )
        .gap_0()
        .child(
            div()
                .mt(surface::css(20.))
                .mb(surface::css(20.))
                .child(i18n::t("RECOMMENDATION_DESC")),
        )
        .child(
            v_flex()
                .opacity(if self.values.recommendations { 1. } else { 0.3 })
                .child(div().child(i18n::t("DEVICES_HEADER").to_uppercase()))
                .child(
                    div()
                        .mt(surface::css(5.))
                        .mb(surface::css(10.))
                        .child(i18n::t("RECOMMENDATION_DEVICES_DESC")),
                )
                .child(
                    // Ia's style_checkGroup is a single vertical column, gap: 10px.
                    v_flex()
                        .gap(surface::css(10.))
                        .mb(surface::css(20.))
                        .children(RECOMMENDATION_CATEGORIES.iter().map(|(id, label)| {
                            let id = *id;
                            settings_check(
                                SharedString::from(format!("settings-category-{id}")),
                                i18n::t(label),
                                !self
                                    .values
                                    .ignored_categories
                                    .iter()
                                    .any(|ignored| ignored == id),
                                !self.values.recommendations,
                                window,
                                cx,
                            )
                            .opacity(1.)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let checked = this
                                        .values
                                        .ignored_categories
                                        .iter()
                                        .any(|ignored| ignored == id);
                                    this.values
                                        .ignored_categories
                                        .retain(|ignored| ignored != id);
                                    if !checked {
                                        this.values.ignored_categories.push(id.into());
                                    }
                                    this.values.ignored_categories.sort();
                                    this.changed(cx);
                                },
                            ))
                        })),
                )
                .child(
                    div()
                        .mb(surface::css(10.))
                        .child(i18n::t("NEW_RELEASE_AND_DEALS").to_uppercase()),
                )
                .child(
                    settings_check(
                        "settings-new-products",
                        i18n::t("NEW_RELEASE_DESC"),
                        self.values.new_products,
                        !self.values.recommendations,
                        window,
                        cx,
                    )
                    .opacity(1.)
                    .mb(surface::css(6.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.values.new_products = !this.values.new_products;
                        this.changed(cx);
                    })),
                )
                .child(
                    settings_check(
                        "settings-partner-deals",
                        i18n::t("NEW_RELEASE_DESC_2"),
                        self.values.partner_deals,
                        !self.values.recommendations,
                        window,
                        cx,
                    )
                    .opacity(1.)
                    .mb(surface::css(20.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.values.partner_deals = !this.values.partner_deals;
                        this.changed(cx);
                    })),
                )
                .child(div().child(i18n::t("IGNORE_CATEGORIES_HEADER").to_uppercase()))
                .child(
                    div()
                        // Source block siblings collapse the title's 10px bottom margin
                        // with this description's 5px top margin to a single 10px gap.
                        .mt(surface::css(10.))
                        .mb(surface::css(10.))
                        .child(i18n::t("IGNORE_CATEGORIES_DESC")),
                )
                .child(
                    settings_button(
                        "settings-reset-categories",
                        i18n::t("RESET"),
                        self.values.ignored_products.is_empty()
                            && self.values.owned_products.is_empty(),
                        window,
                        cx,
                    )
                    .disabled(
                        !self.values.recommendations
                            || (self.values.ignored_products.is_empty()
                                && self.values.owned_products.is_empty()),
                    )
                    .when(!self.values.recommendations, |button| {
                        button.cursor_default().when(
                            !self.values.ignored_products.is_empty()
                                || !self.values.owned_products.is_empty(),
                            |button| button.opacity(1.),
                        )
                    })
                    // Ia's reset uses fit-content with 27px horizontal padding.
                    .min_w_0()
                    .self_start()
                    .px(surface::css(27.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.values.ignored_products.clear();
                        this.values.owned_products.clear();
                        this.changed(cx);
                    })),
                ),
        )
        .into_any_element()
    }
    fn general(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let column_margin = settings_column_margin(window);
        let whats_new = i18n::t("RELEASE_PATCH_NOTE_WHATS_NEW");
        let (before, after) = whats_new
            .split_once("{{releasePatchNote}}")
            .unwrap_or(("", ""));
        h_flex()
            .items_start()
            .flex_wrap()
            .w_full()
            .max_w(surface::css(1240.))
            .mx_auto()
            .justify_center()
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .min_w(surface::css(600.))
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .flex_basis(surface::css(600.))
                    .mx(column_margin)
                    .child(
                        self.panel("LANGUAGE", cx).child(
                            surface::select(&self.language)
                                .id("settings-language")
                                .items(
                                    LANGUAGES
                                        .iter()
                                        .map(|(code, label)| Choice::new(*code, *label))
                                        .collect(),
                                )
                                .w(surface::css(188.)),
                        ),
                    )
                    .child(
                        self.panel("THX_WHAT_NEWS", cx).child(
                            h_flex()
                                .flex_wrap()
                                .line_height(surface::css(17.))
                                .child(before.to_owned())
                                .child(
                                    gpui_kit::base::Button::new("settings-release-notes")
                                        .child("Release Notes")
                                        .text_decoration_1()
                                        .hover(|button| button.text_color(cx.theme().primary))
                                        .focus_visible(|button| {
                                            button.text_color(cx.theme().primary)
                                        })
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.emit(SettingsEvent::ReleaseNotes)
                                        })),
                                )
                                .child(after.to_owned()),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .min_w(surface::css(600.))
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .flex_basis(surface::css(600.))
                    .mx(column_margin)
                    .child(self.about(cx)),
            )
            .into_any_element()
    }
    fn about(&self, cx: &App) -> AnyElement {
        self.panel("ABOUT", cx)
            .child(
                v_flex()
                    .w_full()
                    .text_color(cx.theme().muted_foreground)
                    .line_height(surface::css(17.))
                    // Fs sets max-height:120px; the original SVG's natural height is 70px.
                    .child(h_flex().justify_center().mb(surface::css(20.)).child(
                        img("synapse/settings-synapse-logo.svg")
                            .w(surface::css(294.366)).h(surface::css(70.))
                            .object_fit(ObjectFit::Contain)))
                    .child(div().text_center().mb(surface::css(20.))
                        // Fs uses the Dashboard manifest, not the host or this crate's version.
                        // ["4", ..."0.0.86".split(".").slice(1), 2609221012].join(".")
                        .child(i18n::t("VERSION").replace("{{number}}", "4.0.86.2609221012")))
                    .child(v_flex().text_center().mb(surface::css(20.))
                        .child(i18n::t("COPYRIGHT").replace("{{year}}", &razer_platform::system::copyright_year()))
                        .child(i18n::t("TRADEMARK")))
                    .child(v_flex().mb(surface::css(30.))
                        .child(h_flex().justify_center().flex_wrap().children([
                            ("settings-eula", "LICENSE_TERMS_EULA", "https://assets.razerzone.com/downloads/software/RazerSynapse3EndUserLicenseAgreement.pdf"),
                            ("settings-oss", "OPEN_SOURCE_SOFTWARE_NOTICE", "https://mysupport.razer.com/app/answers/detail/a_id/15020"),
                            ("settings-terms", "TERMS_OF_NOTICE", "https://www.razer.com/legal/terms-of-service"),
                        ].map(|(id, label, url)| h_flex()
                            .child(policy_link(id, i18n::t(label), url, cx))
                            .child(div().px(surface::css(5.)).child("|")))))
                        .child(h_flex().justify_center().child(policy_link(
                            "settings-privacy", i18n::t("PRIVACY_POLICY"),
                            "https://www.razer.com/legal/privacy-policy", cx))))
                    .child(div().text_center().mb(surface::css(20.))
                        .child(i18n::t("CONNECT_WITH_US").to_uppercase()))
                    .child(h_flex().justify_center().mb(surface::css(10.)).child(social_link(
                        "settings-insider", "Razer Insider".into(),
                        "synapse/settings-social-insider.svg", "synapse/settings-social-insider-hover.svg",
                        "https://insider.razer.com", true, cx)))
                    .child(h_flex().flex_wrap().gap(surface::css(24.)).justify_center().mb(surface::css(10.))
                        .children([
                            ("settings-facebook", "FOLLOW_SOCIAL_FACEBOOK", "synapse/settings-social-facebook.svg", "synapse/settings-social-facebook-hover.svg", "https://www.facebook.com/razer"),
                            ("settings-instagram", "FOLLOW_SOCIAL_IG", "synapse/settings-social-instagram.svg", "synapse/settings-social-instagram-hover.svg", "https://www.instagram.com/razer"),
                            ("settings-twitter", "FOLLOW_SOCIAL_TWITTER", "synapse/settings-social-twitter.svg", "synapse/settings-social-twitter-hover.svg", "https://www.twitter.com/Razer"),
                            ("settings-youtube", "FOLLOW_SOCIAL_YOUTUBE", "synapse/settings-social-youtube.svg", "synapse/settings-social-youtube-hover.svg", "https://www.youtube.com/Razer"),
                            ("settings-tiktok", "FOLLOW_SOCIAL_TIKTOK", "synapse/settings-social-tiktok.svg", "synapse/settings-social-tiktok-hover.svg", "https://www.tiktok.com/@razer"),
                            ("settings-twitch", "FOLLOW_SOCIAL_TWITCH", "synapse/settings-social-twitch.svg", "synapse/settings-social-twitch-hover.svg", "https://www.twitch.tv/razer"),
                            ("settings-discord", "FOLLOW_SOCIAL_DISCORD", "synapse/settings-social-discord.svg", "synapse/settings-social-discord-hover.svg", "https://discord.com/invite/razer"),
                        ].map(|(id, label, asset, hover, url)| social_link(id, i18n::t(label).into(), asset, hover, url, false, cx)))),
            )
            .into_any_element()
    }
    fn connection(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_4()
            .child(self.runtime.clone())
            .child(
                surface::panel("本地产品预览", cx)
                    .child(surface::note(format!("全部 {} 个已登记产品均可搜索打开。本地预览保留产品标签页、页面主体和弹层入口；已登记不代表界面已完整复刻。", razer_catalog::registry().len()), cx))
                    .child(h_flex().flex_wrap().items_end().gap_3()
                        .child(v_flex().gap_2().child("产品名称或 ID").child(
                            select::Select::new(&self.preview_product)
                                .id("preview-product-select")
                                .placeholder("搜索产品名称或 ID")
                                .w(surface::css(400.))
                        ))
                        .child(v_flex().gap_2().child("产品版本 (edition)").child(
                            select::Select::new(&self.preview_edition).w_56()
                        ))
                        .child(v_flex().gap_2().child("键盘布局 (layout)").child(
                            select::Select::new(&self.preview_layout).w_56()
                        ))
                        .child(Button::new("preview-registered-product").label("打开产品预览").outline()
                            .disabled(self.preview_product.read(cx).selected_value().is_none()
                                || self.preview_edition.read(cx).selected_value().is_none()
                                || self.preview_layout.read(cx).selected_value().is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                let value = |state: &Entity<SelectState<Vec<Choice>>>| {
                                    state.read(cx).selected_value().and_then(|value| value.parse::<u32>().ok())
                                };
                                if let (Some(pid), Some(edition), Some(layout)) = (
                                    value(&this.preview_product), value(&this.preview_edition), value(&this.preview_layout)
                                ) {
                                    cx.emit(SettingsEvent::PreviewVariant(pid, edition, layout));
                                }
                            }))))
                    .child(surface::note("产品名称和版本名称来自当前官方产品清单，可按中文名、英文名或产品 ID 搜索。未声明版本的产品沿用本地默认值。653 可选择其原始键盘布局，其他产品使用各自默认布局。预览不表示已连接设备，编辑仅保留本地草稿。", cx)),
            )
            .child(
                surface::panel("本地工作区", cx)
                    .children(self.preview_product.read(cx).selected_value().and_then(|v| v.parse::<u32>().ok()).and_then(razer_catalog::registered).map(|product| {
                        v_flex().gap_2().child(surface::note(format!("产品 ID {} · 分类 {} · editions {:?}", product.id(), product.categories().join(", "), product.edition_ids()), cx))
                            .children(product.navigations().iter().map(|navigation| {
                                v_flex().gap_1().child(surface::note(format!("{} · {} · {}{}", navigation.key(), navigation.owner(), navigation.display_mode(), if navigation.is_primary() { " · 默认入口" } else { "" }), cx))
                                    .child(surface::note(format!("{} @ {}\nSHA-256 {}", navigation.source(), navigation.offset(), navigation.source_sha256()), cx))
                                    .children(navigation.pages().iter().map(|page| surface::note(format!("{} · {} · ID {:?} · {} @ {} · {:?}\n{}{}", page.id().product_id(), page.kind().key(), page.source_id(), page.component_kind(), page.offset(), page.adapter_status(), page.component_expression().unwrap_or(""), page.extra_class().map(|c| format!(" · class {c}")).unwrap_or_default()), cx)))
                                    .child(Button::new(SharedString::from(format!("copy-source-navigation-{}",navigation.key()))).label("复制根组件依据").outline().on_click(move |_,_,cx| cx.write_to_clipboard(ClipboardItem::new_string(navigation.reachability().to_string()))))
                            }))
                    }))
                    .child(surface::note(
                        format!("razer_ui {} 本地自动启动草稿尚未应用到系统；灯光控制权尚未读取。", env!("CARGO_PKG_VERSION")),
                        cx,
                    ))
                    .child(surface::note(
                        format!("本地配置位置：{}", razer_storage::store_path().display()),
                        cx,
                    ))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_3()
                            .children(
                                [
                                    (653, "preview-keyboard", "预览 653 键盘"),
                                    (777, "preview-headset", "预览 777 耳机"),
                                ]
                                .map(|(pid, id, label)| {
                                    Button::new(id).label(label).outline().on_click(cx.listener(
                                        move |_, _, _, cx| cx.emit(SettingsEvent::Preview(pid)),
                                    ))
                                }),
                            )
                            .child(
                                Button::new("preview-profile-migration")
                                    .label("打开配置迁移")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| { cx.emit(SettingsEvent::ProfileMigration); })),
                            )
                            .child(
                                Button::new("preview-module-pages")
                                    .label("打开设备和模块")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::Modules)
                                    })),
                            )
                            .children(razer_catalog::AUDITED_MOUSE_MAT_IDS.into_iter().map(
                                |pid| {
                                    let product = razer_catalog::audited_mouse_mat(pid)
                                        .expect("audited mouse mat");
                                    Button::new(SharedString::from(format!(
                                        "preview-product-{pid}"
                                    )))
                                    .label(format!("预览 {}", product.name()))
                                    .outline()
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.emit(SettingsEvent::Preview(pid))
                                    }))
                                },
                            ))
                            .child(
                                Button::new("preview-app-picker")
                                    .label("打开更多应用")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::AppPicker)
                                    })),
                            )
                            .child(
                                Button::new("preview-alexa")
                                    .label("打开 Alexa")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::Alexa)
                                    })),
                            )
                            .child(
                                Button::new("preview-chroma-tour")
                                    .label("打开 Chroma 入门教程")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::ChromaTour)
                                    })),
                            )
                            .child(
                                Button::new("preview-header-states")
                                    .label("打开控制板")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::Dashboard)
                                    })),
                            )
                            .child(
                                Button::new("preview-lighting-settings")
                                    .label("打开设备灯光设置")
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| { this.page = Page::Synapse; cx.notify(); })),
                            )
                            .child(
                                Button::new("preview-keyboard-calibration")
                                    .label("打开磁轴键盘预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(740)))),
                            )
                            .child(
                                Button::new("preview-hue")
                                    .label("打开 Philips Hue 预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(769)))),
                            )
                            .child(
                                Button::new("preview-aether-strip")
                                    .label("打开 Aether 灯带预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(784)))),
                            )
                            .child(
                                Button::new("preview-wireless-argb")
                                    .label("打开无线 ARGB 预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(3884)))),
                            )
                            .child(
                                Button::new("preview-wired-argb")
                                    .label("打开主板与 ARGB 端口预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(3871)))),
                            )
                            .child(
                                Button::new("preview-automation")
                                    .label("打开 Base Station V3 预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(3946)))),
                            )
                            .child(
                                Button::new("open-pairing-page")
                                    .label("打开鼠标底座预览")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Preview(241)))),
                            )
                            .child(
                                Button::new("open-multi-pairing-page")
                                    .label("多设备配对")
                                    .outline()
                                    .on_click(
                                        cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Pairing)),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
// Settings 720: .main-setting .widget .check-item margin:0; original
// 20px indicator, 14px/17px label, 2.4px radius and source tick origins.
fn settings_check(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui_kit::base::Button {
    let label = label.into();
    let mut characters = label.chars();
    let label = match characters.next() {
        Some(first) => {
            SharedString::from(format!("{}{}", first.to_uppercase(), characters.as_str()))
        }
        None => label,
    };
    surface::check_item_with_style(
        id,
        "",
        checked,
        disabled,
        surface::CheckItemStyle {
            unchecked_background: cx.theme().transparent,
            tick_bottom_origin: (0.8, 10.2),
        },
        window,
        cx,
    )
    .m_0()
    .items_start()
    .accessibility_label(label.clone())
    .child(
        div()
            .relative()
            // Source text is inside the 1px indicator border, then top:2px.
            .top(surface::css(3.))
            // Original left:30px is relative to the indicator's inner border.
            .ml(surface::css(11.))
            .max_h(surface::css(20.))
            .flex_shrink_0()
            .whitespace_nowrap()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(cx.theme().foreground)
            .child(label),
    )
    .role(Role::CheckBox)
    .aria_toggled(if checked {
        Toggled::True
    } else {
        Toggled::False
    })
}
fn settings_help(id: &'static str, label: String, text: String) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .accessibility_label(label)
        .size(surface::css(14.))
        .p_0()
        .rounded(surface::css(7.))
        .bg(theme::TooltipColors::help_background())
        .hover(|button| button.bg(theme::SettingsColors::help_hover()))
        .child(img("synapse/onboard-help.svg").size_full())
        .tooltip(move |window, cx| {
            Tooltip::new(text.clone())
                .max_w(surface::css(300.))
                .text_size(surface::css(14.))
                .line_height(surface::css(18.))
                .bg(theme::TooltipColors::background())
                .border_color(theme::TooltipColors::border())
                .rounded_none()
                .shadow_none()
                .px(surface::css(10.))
                .py(surface::css(8.))
                .build(window, cx)
        })
}
fn social_link(
    id: &'static str,
    label: SharedString,
    asset: &'static str,
    hover_asset: &'static str,
    url: &'static str,
    insider: bool,
    cx: &App,
) -> impl IntoElement {
    gpui_kit::base::Link::new(id)
        .href(url)
        .open_with(|url, _, _, cx| cx.open_url(url))
        .group(id)
        .relative()
        .flex_shrink_0()
        .w(surface::css(if insider { 270. } else { 28. }))
        .h(surface::css(if insider { 50. } else { 28. }))
        .accessibility_label(label.clone())
        .when(!insider, |link| {
            link.tooltip(move |window, cx| {
                Tooltip::new(label.clone())
                    .bg(theme::SettingsColors::tooltip_surface())
                    .border_color(theme::stepper_border())
                    .rounded_none()
                    .shadow_none()
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .build(window, cx)
            })
        })
        .cursor_pointer()
        .child(img(asset).size_full().object_fit(ObjectFit::Contain))
        .child(
            img(hover_asset)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Contain)
                .opacity(0.)
                .group_hover(id, |image| image.opacity(1.)),
        )
        .focus_visible(|link| link.bg(cx.theme().secondary_hover))
}
fn settings_column_margin(window: &Window) -> Rems {
    // Current 720 CSS: the last .body-widgets div.widget-col media rule wins
    // over the earlier .main-setting div.widget-col rule at <= 1279 CSS px.
    surface::css(
        if surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        ) {
            30.
        } else {
            0.
        },
    )
}

fn policy_link(id: &'static str, label: String, url: &'static str, cx: &App) -> impl IntoElement {
    gpui_kit::base::Link::new(id)
        .href(url)
        .open_with(|url, _, _, cx| cx.open_url(url))
        .text_color(cx.theme().muted_foreground)
        .cursor_pointer()
        .text_decoration_1()
        .hover(|link| link.text_color(cx.theme().primary))
        .focus_visible(|link| link.text_color(cx.theme().primary))
        .child(label)
}
impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-page")
            .test_support()
            .size_full()
            .min_h_0()
            .font_family("Roboto")
            .text_color(cx.theme().foreground)
            .bg(cx.theme().background)
            .child(
                gpui_kit::base::Tabs::new("settings-navigation")
                    .flex()
                    .items_center()
                    .h(surface::css(48.))
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(cx.theme().title_bar)
                    .tab_group()
                    .justify_center()
                    .gap(surface::css(20.))
                    .children(
                        [
                            (Page::Synapse, "settings-tab-synapse", "SYNAPSE"),
                            (Page::General, "settings-tab-general", "GENERAL"),
                            (Page::Connection, "settings-tab-connection", "服务连接"),
                        ]
                        .map(|(page, id, label)| {
                            surface::navigation_button(id, i18n::t(label), self.page == page, cx)
                                .role(Role::Tab)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.page = page;
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(
                        v_flex()
                            .w_full()
                            .min_w(surface::css(600.))
                            .px(surface::css(20.))
                            .pt(surface::css(10.))
                            .pb(surface::css(20.))
                            .child(match self.page {
                                Page::Synapse => self.synapse(window, cx),
                                Page::General => self.general(window, cx),
                                Page::Connection => self.connection(cx),
                            }),
                    ),
            )
            .when_some(self.storage_error.clone(), |this, error| {
                this.child(
                    div()
                        .px_4()
                        .py_2()
                        .text_color(cx.theme().danger)
                        .child(format!("未能保存设置：{error}")),
                )
            })
    }
}
