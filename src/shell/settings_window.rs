//! Current `/settings/` root: source receipts in settings-window-current-evidence.json.
//! Host catalogs are deliberately absent until the service supplies them.
use super::{
    AppShell, display_window,
    settings_page::{SettingsEvent, SettingsPage},
};
use crate::{
    features::Choice,
    i18n,
    ui::{scroll::SourceScrollable as _, surface},
};
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::*;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};
#[path = "settings_window_presentation.rs"]
mod presentation;

#[derive(Deserialize)]
struct Language {
    code: String,
    label: String,
}
#[derive(Deserialize)]
struct SocialLink {
    name: String,
    #[serde(rename = "translationType")]
    translation_type: String,
    link: String,
}
#[derive(Deserialize)]
struct Source {
    nav: Vec<String>,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    languages: Vec<Language>,
    links: BTreeMap<String, String>,
    version: String,
    #[serde(rename = "socialLinks")]
    social_links: Vec<SocialLink>,
    #[serde(rename = "toolbarKeys")]
    toolbar_keys: BTreeMap<String, String>,
}
fn source() -> &'static Source {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE.get_or_init(|| {
        serde_json::from_str(include_str!("settings_window_data.json"))
            .expect("audited settings data")
    })
}
pub(super) fn text(key: &str) -> String {
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

pub(super) struct SettingsWindow {
    selected: String,
    language: Entity<SelectState<Vec<Choice>>>,
    tray_action: Entity<super::settings_systray_action::SystrayActionSelector>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl SettingsWindow {
    fn new(settings: Entity<SettingsPage>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            focus,
            _subscriptions: vec![subscription, language_observer],
        }
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
                div()
                    .flex()
                    .w_full()
                    .child(
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
                    )
                    .child(
                        h_flex().child(
                            div()
                                .id("host-settings-auto-update")
                                .flex_shrink_0()
                                .text_size(surface::css(14.))
                                .text_color(
                                    crate::ui::theme::SettingsWindowColors::installed_action(),
                                )
                                .opacity(0.3)
                                .underline()
                                .mx(surface::css(10.))
                                .child(text("TEXT_AUTO_UPDATE_ENABLED")),
                        ),
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
                    .border_color(crate::ui::theme::SettingsWindowColors::installed_action()),
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
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_center()
                    .mt(surface::css(20.))
                    .mb(surface::css(30.))
                    .children(
                        [
                            "FAQ",
                            "TEXT_TERMS_OF_SERVICE",
                            "TEXT_PRIVACY_POLICY",
                            "TEXT_OPEN_SOURCE_SOFTWARE_NOTICE",
                        ]
                        .into_iter()
                        .flat_map(|key| {
                            let link = gpui_kit::base::Link::new(key)
                                .href(source().links[key].clone())
                                .accessibility_label(text(key))
                                .open_with(|url, _, _, cx| cx.open_url(url))
                                .text_color(cx.theme().muted_foreground)
                                .underline()
                                .cursor_default()
                                .hover(|s| s.text_color(cx.theme().foreground))
                                .focus_visible(|s| s.bg(cx.theme().secondary_hover))
                                .child(text(key))
                                .into_any_element();
                            let mut items = Vec::with_capacity(2);
                            if key != "FAQ" {
                                items.push(
                                    div()
                                        .mx(surface::css(6.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child("|")
                                        .into_any_element(),
                                );
                            }
                            items.push(link);
                            items
                        }),
                    ),
            )
            .child(
                div()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(text("CONNECT_WITH_US").to_uppercase()),
            )
            .child(
                h_flex()
                    .justify_center()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .child(presentation::SocialLink::new(
                        "insider",
                        "Razer Insider",
                        "https://insider.razer.com/index.php",
                    )),
            )
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(24.))
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .children(source().social_links.iter().map(|social| {
                        let asset = if social.name == "ig" {
                            "instagram"
                        } else {
                            &social.name
                        };
                        presentation::SocialLink::new(
                            asset.to_owned(),
                            text(&social.translation_type),
                            social.link.clone(),
                        )
                    })),
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
        v_flex()
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
                h_flex()
                    .justify_center()
                    .gap(surface::css(20.))
                    .h(surface::css(48.))
                    .min_h(surface::css(48.))
                    .border_b_2()
                    .border_color(crate::ui::theme::SettingsWindowColors::navigation_border())
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
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = target.clone();
                            if target != "TEXT_SYSTRAY_SETTING" {
                                this.tray_action
                                    .update(cx, |selector, cx| selector.unmount(cx));
                            }
                            cx.notify();
                        }))
                    })),
            )
            .child(
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
                    ),
            )
    }
}
impl AppShell {
    pub(super) fn open_settings_window(&mut self, quick_panel: bool, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(720.)), cx)),
            // Current tray 2554/he passes these overrides only from its gear.
            // open_or_focus ignores options when a named window already exists.
            // main.js resolves M.minimumWidth/Height from WINDOW_SIZE_DEFAULTS.
            // Only the tray gear overrides those values through the SDK options.
            window_min_size: Some(if quick_panel {
                size(px(1000.), px(768.))
            } else {
                size(px(600.), px(500.))
            }),
            ..TitleBar::window_options()
        };
        if let Err(error) = display_window::open_or_focus(
            cx,
            "razer-settings".into(),
            display_window::WindowPolicy::Different,
            options,
            move |window, cx| cx.new(|cx| SettingsWindow::new(settings, window, cx)),
        ) {
            self.status = error.to_string();
            cx.notify();
        }
    }
}
