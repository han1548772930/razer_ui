//! Separate Settings app 720/ho/uo, static snapshot retrieved 2026-10-01.
#[path = "settings_lighting.rs"]
mod lighting;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
use crate::{
    features::Choice,
    i18n,
    preferences::{AppPreferences, LANGUAGES, RECOMMENDATION_CATEGORIES},
    ui::{surface, theme},
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    scroll::ScrollableElement as _,
    select::{SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Synapse,
    General,
    Connection,
}
pub(super) enum SettingsEvent {
    Changed,
    Language,
    ResetTutorials,
    Save,
    Preview(u32),
    PreviewModules,
    PreviewHeader,
    ReleaseNotes,
    Pairing,
}
pub(super) struct SettingsPage {
    values: AppPreferences,
    saved: AppPreferences,
    page: Page,
    language: Entity<SelectState<Vec<Choice>>>,
    tutorial_reset: bool,
    saving: bool,
    storage_error: Option<String>,
    dynamic_lighting_supported: bool,
    runtime: Entity<super::runtime_page::RuntimePanel>,
    subscriptions: Vec<Subscription>,
}
impl EventEmitter<SettingsEvent> for SettingsPage {}
impl SettingsPage {
    pub(super) fn new(
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
        let mut this = Self {
            saved,
            values,
            page: Page::Synapse,
            language,
            tutorial_reset: false,
            saving: false,
            storage_error: None,
            dynamic_lighting_supported: crate::backend::system::supports_dynamic_lighting(),
            runtime,
            subscriptions: vec![],
        };
        this.subscriptions
            .push(cx.subscribe(&this.language, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(language)) = event {
                    this.values.language = language.clone();
                    i18n::set_locale(language);
                    cx.emit(SettingsEvent::Language);
                    this.changed(cx);
                }
            }));
        this
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }
    pub(super) fn dirty(&self) -> bool {
        self.values != self.saved
    }
    pub(super) fn snapshot(&self) -> AppPreferences {
        self.values.clone()
    }
    pub(super) fn mark_saved(&mut self, snapshot: AppPreferences, cx: &mut Context<Self>) {
        self.saved = snapshot;
        cx.notify();
    }
    pub(super) fn tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.values.gamer_room_tutorial_seen = seen;
        if seen {
            self.tutorial_reset = false;
        }
        self.changed(cx);
    }
    pub(super) fn tutorial_viewed(&mut self, cx: &mut Context<Self>) {
        self.tutorial_reset = false;
        cx.notify();
    }
    pub(super) fn dashboard_tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.values.dashboard_tutorial_seen = seen;
        if seen {
            self.tutorial_reset = false;
        }
        self.changed(cx);
    }
    // Tutorial dismissal is immediate, independent of the editable preferences.
    pub(super) fn tutorial_snapshot(&self) -> AppPreferences {
        let mut saved = self.saved.clone();
        saved.gamer_room_tutorial_seen = self.values.gamer_room_tutorial_seen;
        saved.dashboard_tutorial_seen = self.values.dashboard_tutorial_seen;
        saved
    }
    pub(super) fn tutorial_pending(&self) -> bool {
        self.values.gamer_room_tutorial_seen != self.saved.gamer_room_tutorial_seen
            || self.values.dashboard_tutorial_seen != self.saved.dashboard_tutorial_seen
    }
    pub(super) fn mark_tutorial_saved(
        &mut self,
        gamer_room_seen: bool,
        dashboard_seen: bool,
        cx: &mut Context<Self>,
    ) {
        self.saved.gamer_room_tutorial_seen = gamer_room_seen;
        self.saved.dashboard_tutorial_seen = dashboard_seen;
        cx.notify();
    }
    pub(super) fn set_persistence_state(
        &mut self,
        saving: bool,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.saving = saving;
        self.storage_error = error;
        cx.notify();
    }
    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seen = self.values.gamer_room_tutorial_seen;
        let dashboard_seen = self.values.dashboard_tutorial_seen;
        self.values = self.saved.clone();
        self.values.gamer_room_tutorial_seen = seen;
        self.values.dashboard_tutorial_seen = dashboard_seen;
        self.language.update(cx, |state, cx| {
            state.set_selected_value(&self.values.language, window, cx)
        });
        i18n::set_locale(&self.values.language);
        cx.emit(SettingsEvent::Language);
        self.changed(cx);
    }
    fn panel(&self, title: &str, cx: &App) -> Div {
        self.panel_with_control(title, div(), cx)
    }
    fn panel_with_control(&self, title: &str, control: impl IntoElement, cx: &App) -> Div {
        // Settings overrides the shared widget title to 18px (720 CSS).
        v_flex()
            .w(surface::css(600.))
            .min_w(surface::css(600.))
            .py(surface::css(surface::WIDGET_PADDING_Y))
            .px(surface::css(surface::WIDGET_PADDING_X))
            .bg(cx.theme().group_box)
            .rounded(surface::css(surface::WIDGET_RADIUS))
            .text_size(surface::css(14.))
            .gap(surface::css(20.))
            .child(
                h_flex()
                    .gap_3()
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
    fn synapse(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .items_start()
            .flex_wrap()
            .gap(surface::css(20.))
            .justify_center()
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .gap(surface::css(20.))
                    .child(
                        self.panel("AUTO_LAUNCH", cx)
                            .child(
                                v_flex()
                                    .child(
                                        Checkbox::new("settings-auto-start")
                                            .label(i18n::t("START_SYNAPSE"))
                                            .checked(false)
                                            .disabled(true),
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
                                                Checkbox::new("settings-start-minimized")
                                                    .label(i18n::t("MINIMIZE_SYSTRAY"))
                                                    .checked(false)
                                                    .disabled(true),
                                            ),
                                    ),
                            )
                            .child(surface::note(
                                "尚未连接 Synapse 启动服务，当前启动偏好未读取。",
                                cx,
                            )),
                    )
                    .child(
                        self.panel("NOTIFICATIONS", cx).child(
                            div()
                                .relative()
                                .child(
                                    Checkbox::new("settings-notifications")
                                        .label(i18n::t("DISPLAY_NOTIFICATIONS"))
                                        .checked(self.values.notifications)
                                        .on_click(cx.listener(|this, checked, _, cx| {
                                            this.values.notifications = *checked;
                                            this.changed(cx);
                                        })),
                                )
                                .child(
                                    Button::new("settings-notifications-help")
                                        .ghost()
                                        .p_0()
                                        .size(surface::css(14.))
                                        .absolute()
                                        .left(surface::css(226.))
                                        .top(surface::css(4.))
                                        .accessibility_label(i18n::t("NOTIFICATIONS"))
                                        .child(img("synapse/onboard-help.svg").size_full())
                                        .tooltip_placement(Placement::Right)
                                        .tooltip(format!(
                                            "{}\n• {}\n• {}\n• {}",
                                            i18n::t("NOTIFICATIONS_TOOLTIP"),
                                            i18n::t("NOTIFICATIONS_TOOLTIP_DESC1"),
                                            i18n::t("NOTIFICATIONS_TOOLTIP_DESC2"),
                                            i18n::t("NOTIFICATIONS_TOOLTIP_DESC3")
                                        )),
                                ),
                        ),
                    )
                    .child(self.recommendations(cx)),
            )
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .gap(surface::css(20.))
                    .child(
                        self.panel("TUTORIAL_RESET", cx)
                            .child(
                                h_flex()
                                    .gap(surface::css(20.))
                                    .items_start()
                                    .child(
                                        Button::new("settings-reset-tutorials")
                                            .label(i18n::t("RESET"))
                                            .outline()
                                            .disabled(self.tutorial_reset)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.tutorial_reset = true;
                                                cx.emit(SettingsEvent::ResetTutorials);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div().flex_1().child(i18n::t("SYNAPSE_TUTORIAL_RESET_MSG")),
                                    ),
                            )
                            .when(self.tutorial_reset, |this| {
                                this.child(surface::note("下次进入相应页面时显示本地教程。", cx))
                            }),
                    )
                    .child(
                        self.panel("PROFILE_MIGRATION", cx).child(
                            h_flex()
                                .items_start()
                                .gap(surface::css(20.))
                                .child(
                                    Button::new("settings-migration")
                                        .label(i18n::t("LAUNCH"))
                                        .outline()
                                        .on_click(|_, window, cx| {
                                            super::profile_migration::open(window, cx);
                                        }),
                                )
                                .child(div().flex_1().child(i18n::t("PROFILE_MIGRATION_DESC"))),
                        ),
                    )
                    .when(self.dynamic_lighting_supported, |column| {
                        column.child(self.panel("DEVICE_LIGHTING", cx).child(lighting::content(
                            None,
                            false,
                            |_, _, _| {},
                            cx,
                        )))
                    }),
            )
            .into_any_element()
    }
    fn recommendations(&self, cx: &mut Context<Self>) -> AnyElement {
        self.panel_with_control(
            "RECOMMENDATION_HEADER",
            surface::SynapseSwitch::new("settings-recommendations")
                .label(i18n::t("RECOMMENDATION_HEADER"))
                .checked(self.values.recommendations)
                .on_change(cx.listener(|this, checked, _, cx| {
                    this.values.recommendations = *checked;
                    this.changed(cx);
                })),
            cx,
        )
        .child(surface::note(i18n::t("RECOMMENDATION_DESC"), cx))
        .child(div().child(i18n::t("DEVICES_HEADER")))
        .child(surface::note(i18n::t("RECOMMENDATION_DEVICES_DESC"), cx))
        .child(
            h_flex()
                .flex_wrap()
                .gap_y_3()
                .children(RECOMMENDATION_CATEGORIES.iter().map(|(id, label)| {
                    let id = *id;
                    Checkbox::new(SharedString::from(format!("settings-category-{id}")))
                        .w_1_2()
                        .label(i18n::t(label))
                        .checked(
                            !self
                                .values
                                .ignored_categories
                                .iter()
                                .any(|ignored| ignored == id),
                        )
                        .disabled(!self.values.recommendations)
                        .on_click(cx.listener(move |this, checked, _, cx| {
                            this.values
                                .ignored_categories
                                .retain(|ignored| ignored != id);
                            if !checked {
                                this.values.ignored_categories.push(id.into());
                            }
                            this.values.ignored_categories.sort();
                            this.changed(cx);
                        }))
                })),
        )
        .child(div().mt_3().child(i18n::t("NEW_RELEASE_AND_DEALS")))
        .child(
            Checkbox::new("settings-new-products")
                .label(i18n::t("NEW_RELEASE_DESC"))
                .checked(self.values.new_products)
                .disabled(!self.values.recommendations)
                .on_click(cx.listener(|this, checked, _, cx| {
                    this.values.new_products = *checked;
                    this.changed(cx);
                })),
        )
        .child(
            Checkbox::new("settings-partner-deals")
                .label(i18n::t("NEW_RELEASE_DESC_2"))
                .checked(self.values.partner_deals)
                .disabled(!self.values.recommendations)
                .on_click(cx.listener(|this, checked, _, cx| {
                    this.values.partner_deals = *checked;
                    this.changed(cx);
                })),
        )
        .child(div().mt_3().child(i18n::t("IGNORE_CATEGORIES_HEADER")))
        .child(surface::note(i18n::t("IGNORE_CATEGORIES_DESC"), cx))
        .child(
            Button::new("settings-reset-categories")
                .label(i18n::t("RESET"))
                .outline()
                .disabled(
                    !self.values.recommendations
                        || (self.values.ignored_products.is_empty()
                            && self.values.owned_products.is_empty()),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.values.ignored_products.clear();
                    this.values.owned_products.clear();
                    this.changed(cx);
                })),
        )
        .into_any_element()
    }
    fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        let whats_new = i18n::t("RELEASE_PATCH_NOTE_WHATS_NEW");
        let (before, after) = whats_new
            .split_once("{{releasePatchNote}}")
            .unwrap_or(("", ""));
        h_flex()
            .items_start()
            .flex_wrap()
            .gap(surface::css(20.))
            .justify_center()
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .gap(surface::css(20.))
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
                        self.panel("THX_WHAT_NEWS", cx)
                            .child(
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
                            )
                            .child(surface::external(
                                "settings-release-support",
                                "Razer Synapse 支持",
                                "https://mysupport.razer.com/app/answers/detail/a_id/14644",
                            )),
                    ),
            )
            .child(self.about(cx))
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
                    .child(v_flex().text_center().mb(surface::css(20.))
                        .child(format!("razer_ui {} · 本地重构界面", env!("CARGO_PKG_VERSION")))
                        .child("Synapse 服务版本可在“服务连接”中读取。"))
                    .child(v_flex().text_center().mb(surface::css(20.))
                        // Attribution year of the audited Settings source snapshot (2026-10-01).
                        .child(i18n::t("COPYRIGHT").replace("{{year}}", "2026"))
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
                surface::panel("本地工作区", cx)
                    .child(surface::note(
                        format!("本地配置位置：{}", crate::store::store_path().display()),
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
                                Button::new("preview-module-pages")
                                    .label("预览模块界面")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::PreviewModules)
                                    })),
                            )
                            .child(
                                Button::new("preview-header-states")
                                    .label("预览顶部状态")
                                    .outline()
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsEvent::PreviewHeader)
                                    })),
                            )
                            .child(
                                Button::new("preview-lighting-settings")
                                    .label("预览灯光设置")
                                    .outline()
                                    .on_click(|_, window, cx| lighting::open_preview(window, cx)),
                            )
                            .child(
                                Button::new("open-pairing-page")
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-page")
            .test_support()
            .size_full()
            .min_h_0()
            .child(
                gpui_kit::base::Tabs::new("settings-navigation")
                    .flex().items_center()
                    .h(surface::css(48.)).flex_shrink_0()
                    .border_b_2().border_color(cx.theme().title_bar)
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
            .child(div().id("settings-scroll").flex_1().min_h_0().overflow_scrollbar()
                .child(v_flex().w_full().min_w(surface::css(660.)).max_w(surface::css(1280.))
                    .mx_auto().px(surface::css(30.)).py(surface::css(20.)).gap(surface::css(20.))
            .child(surface::note(
                "通知和推荐偏好保存在本机；Synapse 启动、灯光控制权及迁移需要相应服务。",
                cx,
            ))
            .child(match self.page {
                Page::Synapse => self.synapse(cx),
                Page::General => self.general(cx),
                Page::Connection => self.connection(cx),
            })))
            .when_some(self.storage_error.clone(), |this, error| this.child(
                div().px_4().py_2().text_color(cx.theme().danger)
                    .child(format!("未能保存设置：{error}"))))
            .child(
                h_flex()
                    .flex_shrink_0().px_4().py_3().border_t_1().border_color(cx.theme().border)
                    .gap_3()
                    .justify_end()
                    .child(
                        Button::new("settings-discard")
                            .label("丢弃设置更改")
                            .outline()
                            .disabled(!self.dirty() || self.saving)
                            .on_click(cx.listener(|this, _, window, cx| this.discard(window, cx))),
                    )
                    .child(
                        Button::new("settings-save")
                            .label(if self.saving { "正在保存…" } else { "保存设置到本机" })
                            .primary()
                            .disabled(!self.dirty() || self.saving || self.storage_error.is_some())
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Save))),
                    ),
            )
    }
}
