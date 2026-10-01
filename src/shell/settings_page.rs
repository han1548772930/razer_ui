//! Separate Settings app 720/ho/uo, static snapshot retrieved 2026-10-01.
use crate::{
    features::Choice,
    i18n,
    preferences::{AppPreferences, LANGUAGES, RECOMMENDATION_CATEGORIES},
    ui::surface,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    select::{SelectEvent, SelectState},
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
}
pub(super) struct SettingsPage {
    values: AppPreferences,
    saved: AppPreferences,
    page: Page,
    language: Entity<SelectState<Vec<Choice>>>,
    tutorial_reset: bool,
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
            saved: values.clone(),
            values,
            page: Page::Synapse,
            language,
            tutorial_reset: false,
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
    pub(super) fn saved_snapshot(&self) -> AppPreferences {
        self.saved.clone()
    }
    pub(super) fn mark_saved(&mut self, snapshot: AppPreferences, cx: &mut Context<Self>) {
        self.saved = snapshot;
        cx.notify();
    }
    pub(super) fn tutorial_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        self.values.gamer_room_tutorial_seen = seen;
        self.changed(cx);
    }
    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.values = self.saved.clone();
        self.language.update(cx, |state, cx| {
            state.set_selected_value(&self.values.language, window, cx)
        });
        i18n::set_locale(&self.values.language);
        cx.emit(SettingsEvent::Language);
        self.changed(cx);
    }
    fn panel(&self, title: &str, cx: &App) -> Div {
        surface::panel(i18n::t(title), cx)
            .w(surface::css(600.))
            .min_w(surface::css(600.))
            .gap(surface::css(20.))
    }
    fn synapse(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex().items_start().flex_wrap().gap(surface::css(20.)).justify_center()
            .child(v_flex().w(surface::css(600.)).gap(surface::css(20.))
                .child(self.panel("AUTO_LAUNCH",cx)
                    .child(Checkbox::new("settings-auto-start").label(i18n::t("START_SYNAPSE")).checked(false).disabled(true))
                    .child(v_flex().pl(surface::css(30.)).gap_2().child(surface::note(i18n::t("NOTE_DISABLE_SYNAPSE"),cx))
                        .child(Checkbox::new("settings-start-minimized").label(i18n::t("MINIMIZE_SYSTRAY")).checked(false).disabled(true)))
                    .child(surface::note("尚未连接 Synapse 启动服务，当前启动偏好未读取。",cx)))
                .child(self.panel("NOTIFICATIONS",cx)
                    .child(Checkbox::new("settings-notifications").label(i18n::t("DISPLAY_NOTIFICATIONS")).checked(self.values.notifications)
                        .on_click(cx.listener(|this,checked,_,cx| { this.values.notifications=*checked; this.changed(cx); })))
                    .child(surface::note(i18n::t("NOTIFICATIONS_TOOLTIP"),cx)))
                .child(self.recommendations(cx)))
            .child(v_flex().w(surface::css(600.)).gap(surface::css(20.))
                .child(self.panel("TUTORIAL_RESET",cx).child(h_flex().gap(surface::css(20.)).items_start()
                    .child(Button::new("settings-reset-tutorials").label(i18n::t("RESET")).outline().disabled(self.tutorial_reset)
                        .on_click(cx.listener(|this,_,_,cx| { this.tutorial_reset=true; cx.emit(SettingsEvent::ResetTutorials); cx.notify(); })))
                    .child(div().flex_1().child(i18n::t("SYNAPSE_TUTORIAL_RESET_MSG"))))
                    .when(self.tutorial_reset,|this| this.child(surface::note("本地教程已重置。",cx))))
                .child(self.panel("PROFILE_MIGRATION",cx).child(h_flex().items_start().gap(surface::css(20.))
                    .child(Button::new("settings-migration").label(i18n::t("LAUNCH")).outline().on_click(|_,window,cx| {
                        information(window,cx,"配置迁移","迁移需要 Synapse 3 配置读取、转换与设备服务。当前未接入迁移服务，尚未进行导入。");
                    })).child(div().flex_1().child(i18n::t("PROFILE_MIGRATION_DESC")))))
                .child(self.panel("DEVICE_LIGHTING",cx)
                    .child(h_flex().gap_2()
                        .child(Button::new("settings-chroma").label(i18n::t("CHROMA_RGB")).outline().disabled(true))
                        .child(Button::new("settings-wdl").label(i18n::t("DYNAMIC_LIGHTING")).outline().disabled(true)))
                    .child(surface::note("尚未读取灯光控制权，不能切换设备的灯光服务。",cx))
                    .child(h_flex().gap_3().child(img("synapse/settings-wdl.svg").size(surface::css(40.)))
                        .child(Button::new("settings-open-wdl").label(i18n::t("OPEN_WINDOWS_DYNAMIC_LIGHTING")).ghost()
                            .on_click(|_,_,cx| cx.open_url("ms-settings:personalization-lighting"))))))
            .into_any_element()
    }
    fn recommendations(&self, cx: &mut Context<Self>) -> AnyElement {
        self.panel("RECOMMENDATION_HEADER", cx)
            .child(
                surface::SynapseSwitch::new("settings-recommendations")
                    .label(i18n::t("RECOMMENDATION_HEADER"))
                    .checked(self.values.recommendations)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.values.recommendations = *checked;
                        this.changed(cx);
                    })),
            )
            .child(surface::note(i18n::t("RECOMMENDATION_DESC"), cx))
            .child(div().child(i18n::t("DEVICES_HEADER")))
            .child(surface::note(i18n::t("RECOMMENDATION_DEVICES_DESC"), cx))
            .children(RECOMMENDATION_CATEGORIES.iter().map(|(id, label)| {
                let id = *id;
                Checkbox::new(SharedString::from(format!("settings-category-{id}")))
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
            }))
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
                        !self.values.recommendations || self.values.ignored_products.is_empty(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.values.ignored_products.clear();
                        this.changed(cx);
                    })),
            )
            .into_any_element()
    }
    fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex().items_start().flex_wrap().gap(surface::css(20.)).justify_center()
            .child(v_flex().w(surface::css(600.)).gap(surface::css(20.))
                .child(self.panel("LANGUAGE",cx).child(surface::select(&self.language).id("settings-language").w(surface::css(188.))))
                .child(self.panel("THX_WHAT_NEWS",cx)
                    .child(Button::new("settings-release-notes").label("Release Notes").ghost().on_click(|_,window,cx| {
                        information(window,cx,"Release Notes","当前没有读取到已安装 Synapse 版本的发行说明，可在官方支持站点查看。");
                    })).child(surface::external("settings-release-support","Razer Synapse 支持","https://mysupport.razer.com/app/answers/detail/a_id/14644"))))
            .child(self.panel("ABOUT",cx)
                .child(img("synapse/settings-synapse-logo.svg").h(surface::css(120.)).w_full().object_fit(ObjectFit::Contain))
                .child(div().text_center().child(format!("razer_ui {} · 本地重构界面",env!("CARGO_PKG_VERSION"))))
                .child(surface::note("Synapse 服务版本可在“服务连接”中读取。",cx))
                .child(div().text_center().child(i18n::t("TRADEMARK")))
                .children([
                    ("settings-eula","LICENSE_TERMS_EULA","https://assets.razerzone.com/downloads/software/RazerSynapse3EndUserLicenseAgreement.pdf"),
                    ("settings-oss","OPEN_SOURCE_SOFTWARE_NOTICE","https://mysupport.razer.com/app/answers/detail/a_id/15020"),
                    ("settings-terms","TERMS_OF_NOTICE","https://www.razer.com/legal/terms-of-service"),
                    ("settings-privacy","PRIVACY_POLICY","https://www.razer.com/legal/privacy-policy"),
                ].map(|(id,label,url)| surface::external(id,i18n::t(label),url)))
                .child(div().text_center().child(i18n::t("CONNECT_WITH_US")))
                .child(h_flex().flex_wrap().gap_3().justify_center().children([
                    ("settings-insider","Razer Insider","https://insider.razer.com"),
                    ("settings-facebook","Facebook","https://www.facebook.com/razer"),
                    ("settings-instagram","Instagram","https://www.instagram.com/razer"),
                    ("settings-twitter","X","https://www.twitter.com/Razer"),
                    ("settings-youtube","YouTube","https://www.youtube.com/Razer"),
                    ("settings-tiktok","TikTok","https://www.tiktok.com/@razer"),
                    ("settings-twitch","Twitch","https://www.twitch.tv/razer"),
                    ("settings-discord","Discord","https://discord.com/invite/razer"),
                ].map(|(id,label,url)| surface::external(id,label,url)))))
            .into_any_element()
    }
}
fn information(window: &mut Window, cx: &mut App, title: &'static str, body: &'static str) {
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title(title).child(body).footer(
            Button::new("settings-information-close")
                .label("关闭")
                .on_click(|_, window, cx| window.close_dialog(cx)),
        )
    });
}
impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-page")
            .test_support()
            .w_full()
            .gap(surface::css(20.))
            .child(
                h_flex()
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
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.page = page;
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(surface::note(
                "通知和推荐偏好保存在本机；Synapse 启动、灯光控制权及迁移需要相应服务。",
                cx,
            ))
            .child(match self.page {
                Page::Synapse => self.synapse(cx),
                Page::General => self.general(cx),
                Page::Connection => self.runtime.clone().into_any_element(),
            })
            .child(
                h_flex()
                    .gap_3()
                    .justify_end()
                    .child(
                        Button::new("settings-discard")
                            .label("丢弃设置更改")
                            .outline()
                            .disabled(!self.dirty())
                            .on_click(cx.listener(|this, _, window, cx| this.discard(window, cx))),
                    )
                    .child(
                        Button::new("settings-save")
                            .label("保存到本机")
                            .primary()
                            .disabled(!self.dirty())
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Save))),
                    ),
            )
    }
}
