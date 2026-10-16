//! Current `/settings/` root: source receipts in settings-window-current-evidence.json.
//! Host catalogs are deliberately absent until the service supplies them.
use crate::settings_page::{SettingsEvent, SettingsPage};
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::*;
use razer_i18n as i18n;
use razer_pages::features::Choice;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};
#[path = "settings_window_presentation.rs"]
pub(crate) mod presentation;

#[derive(Deserialize)]
struct Language {
    code: String,
    label: String,
}
#[derive(Deserialize)]
struct Source {
    nav: Vec<String>,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    languages: Vec<Language>,
    version: String,
    #[serde(rename = "toolbarKeys")]
    toolbar_keys: BTreeMap<String, String>,
}
fn source() -> &'static Source {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE.get_or_init(|| crate::data::settings_window_data!())
}
pub fn text(key: &str) -> String {
    source()
        .translations
        .get(&i18n::locale().to_lowercase())
        .and_then(|strings| strings.get(key))
        .or_else(|| source().translations["en"].get(key))
        .cloned()
        .unwrap_or_else(|| key.into())
}
fn language_choices() -> Vec<Choice> {
    source()
        .languages
        .iter()
        .map(|l| Choice::new(l.code.clone(), l.label.clone()))
        .collect()
}

// Current Settings lazy CSS `.settings .input-label`, used by both General
// and Systray. Appearance only; the existing entities retain interaction.
fn input_label(key: &str) -> Div {
    h_flex()
        .font_weight(FontWeight::BOLD)
        .mb(surface::css(10.))
        .child(text(key).to_uppercase())
}

pub struct SettingsWindow {
    selected: String,
    language: Entity<SelectState<Vec<Choice>>>,
    tray_action: Entity<super::settings_systray_action::SystrayActionSelector>,
    products: Option<Entity<crate::products::ProductsView>>,
    settings: Entity<SettingsPage>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl SettingsWindow {
    pub fn new(
        settings: Entity<SettingsPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title(&text("SETTINGS_HEADER"));
        let language = cx.new(|cx| SelectState::new(language_choices(), None, window, cx));
        language.update(cx, |state, cx| {
            state.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
        });
        let language_observer = cx.subscribe_in(
            &settings,
            window,
            |this: &mut Self, _, event, window, cx| {
                if matches!(event, SettingsEvent::Language) {
                    if let Some(products) = &this.products {
                        products.update(cx, |products, cx| products.relocalize(window, cx));
                    }
                    this.language.update(cx, |state, cx| {
                        state.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
                    });
                    window.set_window_title(&text("SETTINGS_HEADER"));
                    cx.notify();
                }
            },
        );
        let tray_action = cx
            .new(|_| super::settings_systray_action::SystrayActionSelector::new(settings.clone()));
        let owner = settings.clone();
        let subscription = cx.subscribe_in(&language, window, move |_, _, event, window, cx| {
            if let SelectEvent::Confirm(Some(code)) = event {
                settings.update(cx, |settings, cx| settings.set_language(code, window, cx));
                window.set_window_title(&text("SETTINGS_HEADER"));
                cx.notify();
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            selected: source().nav[0].clone(),
            language,
            tray_action,
            products: None,
            settings: owner,
            focus,
            _subscriptions: vec![subscription, language_observer],
        }
    }

    fn open_products(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.products.is_none() {
            let products = cx.new(|cx| crate::products::ProductsView::new(window, cx));
            let settings = self.settings.clone();
            self._subscriptions.push(cx.subscribe(
                &products,
                move |_, _, event: &crate::products::OpenProduct, cx| {
                    settings.update(cx, |_, cx| {
                        cx.emit(SettingsEvent::PreviewVariant(
                            event.product_id(),
                            event.edition_id(),
                            event.layout_id(),
                        ));
                    });
                },
            ));
            self.products = Some(products);
        }
        self.selected = "LOCAL_PRODUCTS_TITLE".into();
        self.tray_action
            .update(cx, |selector, cx| selector.unmount(cx));
        cx.notify();
    }

    fn software(&self, cx: &App) -> AnyElement {
        // 9302/ie initializes installedApps=[]; neither the Rust modules nor
        // the reference package proves anything is installed on this machine.
        v_flex()
            .max_w(surface::css(1220.))
            .w_full()
            .mx_auto()
            .mb(surface::css(20.))
            .child(
                div().flex().w_full().child(
                    div()
                        .id("host-settings-installed-title")
                        .flex_grow(1.)
                        .flex_shrink_0()
                        .flex_basis(auto())
                        .font_family("RazerF5")
                        .text_size(surface::css(22.))
                        .font_weight(FontWeight::LIGHT)
                        .text_color(cx.theme().primary)
                        .child(text("TEXT_INSTALLED").to_uppercase()),
                ),
            )
            .into_any_element()
    }

    fn systray(&self, cx: &App) -> AnyElement {
        // Empty, unavailable host catalog: no fabricated installed app choices,
        // widget checkboxes or launch actions. These are the source's empty rows.
        let launcher = surface::panel(text("TEXT_QUICK_LAUNCHER"), cx)
            .child(input_label("PREVIEW"))
            // x renders the `.apps` list even when L=[]; keep its outline
            // and 60px height rather than collapsing it into a 20px spacer.
            .child(
                div()
                    .id("host-settings-launcher-preview")
                    .w(surface::css(360.))
                    .h(surface::css(60.))
                    .flex_shrink_0()
                    .mb(surface::css(20.))
                    .bg(cx.theme().group_box)
                    .border_1()
                    .border_color(razer_widgets::theme::SettingsWindowColors::installed_action()),
            )
            .child(input_label("ORDER_FROM_LEFT_TO_RIGHT"))
            .children((1..=5).map(|slot| {
                v_flex()
                    .mb(surface::css(if slot == 5 { 0. } else { 20. }))
                    .child(
                        div()
                            .mb(surface::css(5.))
                            .child(format!("{} {slot}", text("SLOT"))),
                    )
                    .child(
                        div()
                            .border_1()
                            .border_color(cx.theme().border)
                            .w(surface::css(260.))
                            .h(surface::css(27.))
                            .pl(surface::css(35.))
                            .pr(surface::css(30.))
                            .line_height(surface::css(26.))
                            .opacity(0.3)
                            .child(text("NONE")),
                    )
            }));
        let general = surface::panel(text("GENERAL"), cx)
            .child(input_label("SYSTRAY_ICON"))
            .child(div().mb(surface::css(10.)).child(text("SYSTRAY_ICON_DESC")))
            .child(self.tray_action.clone());
        h_flex()
            .items_start()
            .flex_wrap()
            .justify_center()
            .gap(surface::css(20.))
            .child(v_flex().w(surface::css(600.)).child(launcher))
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .child(general)
                    .child(surface::panel(text("TEXT_WIDGETS"), cx)),
            )
            .into_any_element()
    }

    fn general(&self, cx: &App) -> AnyElement {
        let about = surface::panel(text("ABOUT"), cx)
            .child(
                div()
                    .mx_auto()
                    .w(surface::css(218.))
                    .h(surface::css(30.))
                    .mb(surface::css(20.))
                    .child(img("synapse/settings-window-logo.svg").size_full()),
            )
            .child(
                div()
                    .text_center()
                    .mb(surface::css(20.))
                    .text_color(cx.theme().muted_foreground)
                    .child(text("VERSION").replace("{{number}}", &source().version)),
            )
            .child(
                div()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        text("COPYRIGHT")
                            .replace("{{year}}", &chrono::Local::now().format("%Y").to_string()),
                    ),
            )
            .child(
                div()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(text("TRADEMARK")),
            );
        h_flex()
            .items_start()
            .flex_wrap()
            .justify_center()
            .gap(surface::css(20.))
            .child(
                v_flex().w(surface::css(600.)).child(
                    surface::panel(text("LANGUAGE"), cx)
                        .child(input_label("LANGUAGE"))
                        .child(
                            surface::select(&self.language)
                                .id("host-settings-language")
                                .items(language_choices())
                                .w(surface::css(188.)),
                        ),
                ),
            )
            .child(v_flex().w(surface::css(600.)).child(about))
            .into_any_element()
    }
}
impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.selected.as_str() {
            "TEXT_SYSTRAY_SETTING" => self.systray(cx),
            "TEXT_GENERAL_SETTING" => self.general(cx),
            _ => self.software(cx),
        };
        let content = v_flex()
            .id("razer-settings")
            .track_focus(&self.focus)
            .size_full()
            .on_key_down(|event, window, _| {
                if event.keystroke.key == "escape" {
                    window.remove_window();
                }
            })
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family("Roboto")
            // The shared 97 stylesheet overrides main's 14px body size with
            // 16px. Widgets retain their own 14px; line-height stays 1.22.
            .text_size(surface::css(16.))
            .line_height(relative(1.22))
            .child(TitleBar::new().child(text("SETTINGS_HEADER")))
            .child(
                h_flex()
                    .h(surface::css(38.))
                    .flex_shrink_0()
                    .child(
                        h_flex()
                            .flex_grow(1.)
                            .flex_shrink(1.)
                            .flex_basis(auto())
                            .child(presentation::toolbar_button(
                                "settings-back",
                                text(&source().toolbar_keys["back"]).into(),
                                "synapse/settings-window-back.svg",
                                true,
                                |_, _, _| {},
                                window,
                                cx,
                            ))
                            .child(presentation::toolbar_button(
                                "settings-forward",
                                text(&source().toolbar_keys["forward"]).into(),
                                "synapse/settings-window-forward.svg",
                                true,
                                |_, _, _| {},
                                window,
                                cx,
                            ))
                            .child(presentation::toolbar_button(
                                "settings-refresh",
                                text(&source().toolbar_keys["refresh"]).into(),
                                "synapse/settings-window-refresh.svg",
                                false,
                                cx.listener(|this, _, _, cx| {
                                    this.selected = source().nav[0].clone();
                                    this.tray_action
                                        .update(cx, |selector, cx| selector.unmount(cx));
                                    cx.notify();
                                }),
                                window,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex_grow(3.)
                            .flex_shrink(1.)
                            .flex_basis(surface::css(340.))
                            .overflow_hidden()
                            .text_center()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(38.))
                            .text_color(cx.theme().muted_foreground)
                            .child(text("SETTINGS_HEADER").to_uppercase()),
                    )
                    .child(
                        div()
                            .flex_grow(0.)
                            .flex_shrink(0.)
                            .flex_basis(relative(0.2)),
                    ),
            )
            .child(
                gpui_kit::base::Tabs::new("host-settings-navigation")
                    .flex()
                    .items_center()
                    .tab_group()
                    .justify_center()
                    .gap(surface::css(20.))
                    .h(surface::css(48.))
                    .min_h(surface::css(48.))
                    .border_b_2()
                    .border_color(razer_widgets::theme::SettingsWindowColors::navigation_border())
                    .py(surface::css(9.))
                    .children(source().nav.iter().map(|key| {
                        let selected = self.selected == *key;
                        let target = key.clone();
                        surface::navigation_button(
                            SharedString::from(format!("host-settings-{key}")),
                            text(key),
                            selected,
                            cx,
                        )
                        .role(Role::Tab)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = target.clone();
                            if target != "TEXT_SYSTRAY_SETTING" {
                                this.tray_action
                                    .update(cx, |selector, cx| selector.unmount(cx));
                            }
                            cx.notify();
                        }))
                    }))
                    .child(
                        surface::navigation_button(
                            "host-settings-products",
                            i18n::t("LOCAL_PRODUCTS_TITLE"),
                            self.selected == "LOCAL_PRODUCTS_TITLE",
                            cx,
                        )
                        .role(Role::Tab)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_products(window, cx);
                        })),
                    ),
            )
            .child(if self.selected == "LOCAL_PRODUCTS_TITLE" {
                self.products
                    .as_ref()
                    .expect("product catalog is initialized by navigation")
                    .clone()
                    .into_any_element()
            } else {
                div()
                    .id("host-settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .scrollable_both()
                    .child(
                        div()
                            .min_w(surface::css(600.))
                            .px(surface::css(20.))
                            .pt(surface::css(10.))
                            .pb(surface::css(20.))
                            .child(body),
                    )
                    .into_any_element()
            });
        image_cache(razer_assets::resource_image_cache())
            .size_full()
            .child(content)
    }
}
