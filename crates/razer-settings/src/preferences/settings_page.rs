//! Current Settings app 720/ho/uo, independently verified on 2026-10-03.
use razer_widgets::scroll::SourceScrollable as _;
#[path = "settings_lighting.rs"]
mod lighting;
use razer_widgets::settings_button::settings_button;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_pages::features::Choice;
use razer_state::AppPreferences;
use razer_state::LANGUAGES;
use razer_widgets::surface;
use razer_widgets::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Synapse,
    General,
    Products,
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
    let layouts = razer_pages::features::keyboard_products::source_preview_layouts(pid);
    if !layouts.is_empty() {
        return layouts;
    }
    if pid != 653 {
        return vec![0];
    }
    keyboard_preview_layouts()
        .iter()
        .map(|layout| layout.layout_id)
        .collect()
}

pub fn preview_layouts_for_edition(pid: u32, edition: u32) -> Vec<u32> {
    if !preview_editions(pid).contains(&edition) {
        return Vec::new();
    }
    razer_pages::features::keyboard_products::source_preview_layouts_for_edition(pid, edition)
        .unwrap_or_else(|| preview_layouts(pid))
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
            "../../../../assets/synapse/products/653/configs/customize-layouts.json"
        ))
        .expect("audited current 653 layouts")
        .layouts
    })
}

pub(crate) fn preview_layout_label(pid: u32, layout: u32) -> String {
    if pid == 653 {
        if let Some(value) = keyboard_preview_layouts()
            .iter()
            .find(|value| value.layout_id == layout)
        {
            return format!("{} · {}", layout, value.layout_name);
        }
    }
    if layout == 0 {
        i18n::t("LOCAL_PRODUCTS_DEFAULT_LAYOUT")
    } else {
        i18n::t("LOCAL_PRODUCTS_LAYOUT_ID").replace("{id}", &layout.to_string())
    }
}

pub enum SettingsEvent {
    Changed,
    Language,
    StartupRequested(StartupSetting),
    DynamicLightingRequested(bool),
    ResetTutorials,
    Preview(u32),
    PreviewVariant(u32, u32, u32),
    ChromaTour,
    AppPicker,
    Modules,
    Dashboard,
    ProfileMigration,
    ReleaseNotes,
    Pairing,
}
/// The two commands in source `Ks` have separate host calls. A local draft is
/// never a receipt that either native command has succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupSetting {
    AutoStart(bool),
    MinimizedOnStartup(bool),
}
pub struct SettingsPage {
    values: AppPreferences,
    saved: AppPreferences,
    page: Page,
    language: Entity<SelectState<Vec<Choice>>>,
    products: Option<Entity<crate::products::ProductsView>>,
    startup_observed: Option<razer_state::LocalStartupDraft>,
    startup_editor: razer_state::LocalStartupDraft,
    tutorial_reset: bool,
    storage_error: Option<String>,
    dynamic_lighting_supported: bool,
    dynamic_lighting_mode: bool,
    dynamic_lighting_switching: bool,
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
        if let Some(products) = &self.products {
            products.update(cx, |products, cx| products.relocalize(window, cx));
        }
        cx.emit(SettingsEvent::Language);
        self.changed(cx);
    }
    pub fn new(
        mut values: AppPreferences,
        _runtime: Entity<super::runtime_page::RuntimePanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Prepare immutable source metadata before rendering the About page.
        let _ = dashboard_source_version();
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
        let dynamic_lighting_mode = saved.dynamic_lighting_draft.unwrap_or(false);
        let mut this = Self {
            saved,
            startup_editor: values.startup_draft.unwrap_or_default(),
            values,
            page: Page::Synapse,
            language,
            products: None,
            startup_observed: None,
            tutorial_reset: false,
            storage_error: None,
            dynamic_lighting_supported: razer_platform::system::supports_dynamic_lighting(),
            dynamic_lighting_mode,
            dynamic_lighting_switching: false,
            subscriptions: vec![],
        };
        this.subscriptions.push(cx.subscribe_in(
            &this.language,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(language)) = event {
                    this.values.language = language.clone();
                    i18n::set_locale(language);
                    if let Some(products) = &this.products {
                        products.update(cx, |products, cx| products.relocalize(window, cx));
                    }
                    cx.emit(SettingsEvent::Language);
                    this.changed(cx);
                }
            },
        ));
        this
    }

    fn select_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        if page == Page::Products && self.products.is_none() {
            // The local catalog visits every product family. It belongs to this
            // optional page, not first-window initialization. Retain it after
            // opening so filtering, selection and scroll survive navigation.
            let products = cx.new(|cx| crate::products::ProductsView::new(window, cx));
            self.subscriptions.push(cx.subscribe(
                &products,
                |_, _, event: &crate::products::OpenProduct, cx| {
                    cx.emit(SettingsEvent::PreviewVariant(
                        event.product_id(),
                        event.edition_id(),
                        event.layout_id(),
                    ));
                },
            ));
            self.products = Some(products);
        }
        self.page = page;
        cx.notify();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }
    /// Called only with an actual host query/readback. It updates the source
    /// editor without emitting a user command or turning a query into a save.
    pub fn observe_startup(
        &mut self,
        value: razer_state::LocalStartupDraft,
        cx: &mut Context<Self>,
    ) {
        self.startup_observed = Some(value);
        self.startup_editor = value;
        cx.notify();
    }
    /// Update only from actual storage/service observations, independently of local intent.
    pub fn observe_dynamic_lighting(
        &mut self,
        dynamic: bool,
        switching: bool,
        cx: &mut Context<Self>,
    ) {
        self.dynamic_lighting_mode = dynamic;
        self.dynamic_lighting_switching = switching;
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
    pub fn mouse_dynamic_tutorial_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.values.mouse_dynamic_tutorial_visible = Some(visible);
        self.changed(cx);
    }
    pub fn mouse_calibration_welcome_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.values.mouse_calibration_welcome_visible = Some(visible);
        self.changed(cx);
    }
    pub fn windows_login_warning_hidden(&mut self, cx: &mut Context<Self>) {
        self.values.windows_login_warning_hide = Some(true);
        self.changed(cx);
    }
    pub fn source_product_preference(
        &self,
        product_id: u32,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        self.values
            .source_product_preferences
            .get(&product_id)
            .map(|raw| serde_json::from_str(raw).map_err(anyhow::Error::from))
            .transpose()
    }
    /// Import only the actual source key's value. This does not create a
    /// Synapse object from our local profile/settings representations.
    pub fn observe_source_product_preference(
        &mut self,
        product_id: u32,
        raw: Option<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        if let Some(raw) = raw {
            let _: serde_json::Value = serde_json::from_str(&raw)?;
            self.values
                .source_product_preferences
                .insert(product_id, raw);
        } else {
            self.values.source_product_preferences.remove(&product_id);
        }
        self.changed(cx);
        Ok(())
    }
    /// The source writes false only when the original synapse_<PID> object
    /// exists. Preserve every sibling field; never invent a missing object.
    pub fn hide_keyboard_mapping_sensitivity_warning(
        &mut self,
        product_id: u32,
        cx: &mut Context<Self>,
    ) -> bool {
        let Ok(Some(mut preference)) = self.source_product_preference(product_id) else {
            return false;
        };
        let Some(object) = preference.as_object_mut() else {
            return false;
        };
        object.insert(
            "showSensitivityWarning".into(),
            serde_json::Value::Bool(false),
        );
        let Ok(raw) = serde_json::to_string(&preference) else {
            return false;
        };
        self.values
            .source_product_preferences
            .insert(product_id, raw);
        self.changed(cx);
        true
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
        let startup = self.startup_editor;
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
                                    settings_check(
                                        "settings-auto-start",
                                        i18n::t("START_SYNAPSE"),
                                        startup.auto_start,
                                        false,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.startup_editor.auto_start =
                                                !this.startup_editor.auto_start;
                                            this.values.startup_draft = Some(this.startup_editor);
                                            cx.emit(SettingsEvent::StartupRequested(
                                                StartupSetting::AutoStart(
                                                    this.startup_editor.auto_start,
                                                ),
                                            ));
                                            this.changed(cx);
                                        },
                                    )),
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
                                            settings_check(
                                                "settings-start-minimized",
                                                i18n::t("MINIMIZE_SYSTRAY"),
                                                startup.start_minimized,
                                                !startup.auto_start,
                                                window,
                                                cx,
                                            )
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    if !this.startup_editor.auto_start {
                                                        return;
                                                    }
                                                    this.startup_editor.start_minimized =
                                                        !this.startup_editor.start_minimized;
                                                    this.values.startup_draft =
                                                        Some(this.startup_editor);
                                                    cx.emit(SettingsEvent::StartupRequested(
                                                        StartupSetting::MinimizedOnStartup(
                                                            this.startup_editor.start_minimized,
                                                        ),
                                                    ));
                                                    this.changed(cx);
                                                }),
                                            ),
                                        ),
                                ),
                        ),
                    )
                    .child(
                        self.panel("NOTIFICATIONS", cx).child(
                            div()
                                .relative()
                                .child(
                                    settings_check(
                                        "settings-notifications",
                                        i18n::t("DISPLAY_NOTIFICATIONS"),
                                        self.values.notifications,
                                        false,
                                        window,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.values.notifications = !this.values.notifications;
                                            this.changed(cx);
                                        },
                                    )),
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
                                .child({
                                    let owner = cx.entity().downgrade();
                                    lighting::content(
                                        Some(self.dynamic_lighting_mode),
                                        self.dynamic_lighting_switching,
                                        move |dynamic, _, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                if this.dynamic_lighting_switching
                                                    || this.dynamic_lighting_mode == dynamic
                                                {
                                                    return;
                                                }
                                                this.dynamic_lighting_mode = dynamic;
                                                this.values.dynamic_lighting_draft = Some(dynamic);
                                                cx.emit(SettingsEvent::DynamicLightingRequested(
                                                    dynamic,
                                                ));
                                                this.changed(cx);
                                            });
                                        },
                                        cx,
                                    )
                                }),
                        )
                    }),
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
                    .child(
                        h_flex().justify_center().mb(surface::css(20.)).child(
                            img("synapse/settings-synapse-logo.svg")
                                .w(surface::css(294.366))
                                .h(surface::css(70.))
                                .object_fit(ObjectFit::Contain),
                        ),
                    )
                    .child(
                        div()
                            .text_center()
                            .mb(surface::css(20.))
                            // Fs uses Dashboard version/buildVersion, not the host version.
                            .child(
                                i18n::t("VERSION")
                                    .replace("{{number}}", dashboard_source_version()),
                            ),
                    )
                    .child(
                        v_flex()
                            .text_center()
                            .mb(surface::css(20.))
                            .child(
                                i18n::t("COPYRIGHT")
                                    .replace("{{year}}", &razer_platform::system::copyright_year()),
                            )
                            .child(i18n::t("TRADEMARK")),
                    ),
            )
            .into_any_element()
    }
}
fn dashboard_source_version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION
        .get_or_init(|| {
            let manifest: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../assets/synapse/manifests/settings-dashboard-manifest.json"
            ))
            .expect("current acquired Dashboard manifest");
            let version = manifest["version"].as_str().expect("Dashboard version");
            let build = manifest["buildVersion"]
                .as_u64()
                .expect("Dashboard buildVersion");
            std::iter::once("4".to_owned())
                .chain(version.split('.').skip(1).map(str::to_owned))
                .chain(std::iter::once(build.to_string()))
                .collect::<Vec<_>>()
                .join(".")
        })
        .as_str()
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
    .aria_toggled(if checked { Toggled::True } else { Toggled::False })
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
                            (
                                Page::Products,
                                "settings-tab-products",
                                "LOCAL_PRODUCTS_TITLE",
                            ),
                        ]
                        .map(|(page, id, label)| {
                            surface::navigation_button(id, i18n::t(label), self.page == page, cx)
                                .role(Role::Tab)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_page(page, window, cx);
                                }))
                        }),
                    ),
            )
            .child(if self.page == Page::Products {
                self.products
                    .as_ref()
                    .expect("product catalog is initialized by page selection")
                    .clone()
                    .into_any_element()
            } else {
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
                                Page::Products => unreachable!(),
                            }),
                    )
                    .into_any_element()
            })
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
