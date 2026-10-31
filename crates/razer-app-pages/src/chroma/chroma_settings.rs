//! Current /chroma-app/settings/ 1743: Zo -> mi/vi; source receipts are generated.
//! This entity owns local Chroma intent, not host autostart or device ownership.
use crate::release_notes;
use razer_widgets::keyboard_navigation::Button;
use gpui_kit::base::{Checkbox, CheckboxState};
use gpui_kit::component::{
    select::{SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::chroma_settings::SettingsDraft;
use razer_pages::features::Choice;
use razer_settings::settings_page::{SettingsEvent, SettingsPage};
use razer_state::chroma_settings::{Change, ChromaSettings as SettingsOwner};
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::settings_button as source_button;
use razer_widgets::surface::css;
use razer_widgets::theme::ChromaSettingsColors as Colors;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct Language {
    name: String,
    code: String,
}
#[derive(Deserialize)]
struct Source {
    translations: BTreeMap<String, BTreeMap<String, String>>,
    languages: Vec<Language>,
    #[serde(rename = "tutorialKeys")]
    tutorial_keys: Vec<String>,
    version: String,
}
fn source() -> &'static Source {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE.get_or_init(|| read_chroma_settings_catalog())
}
pub fn text(key: &str) -> String {
    source()
        .translations
        .get(&i18n::locale().to_lowercase())
        .and_then(|t| t.get(key))
        .or_else(|| source().translations["en"].get(key))
        .cloned()
        .unwrap_or_else(|| key.into())
}
fn local(zh: &str, en: &str) -> String {
    if i18n::locale().starts_with("zh") {
        zh.into()
    } else {
        en.into()
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Chroma,
    General,
}
impl Page {
    fn key(self) -> &'static str {
        match self {
            Self::Chroma => "CHROMA_APP",
            Self::General => "GENERAL",
        }
    }
}
pub enum SettingsAction {
    ResetTutorials,
    OpenMigration,
    ReleaseNotes,
}
pub struct ChromaSettings {
    page: Page,
    history: Vec<Page>,
    history_ix: usize,
    focus: FocusHandle,
    window: AnyWindowHandle,
    locale_owner: Entity<SettingsPage>,
    dashboard: Entity<crate::chroma_page::ChromaPage>,
    language: Entity<SelectState<Vec<Choice>>>,
    settings_owner: Entity<SettingsOwner>,
    draft: SettingsDraft,
    error: Option<String>,
    dynamic_supported: bool,
    dynamic_switching: bool,
    dualsense_supported: bool,
    dualsense_present: bool,
    notes: Option<Entity<release_notes::ReleaseNotes>>,
    notes_subscription: Option<Subscription>,
    unsaved: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<SettingsAction> for ChromaSettings {}
impl ChromaSettings {
    pub fn new(
        locale_owner: Entity<SettingsPage>,
        dashboard: Entity<crate::chroma_page::ChromaPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let choices = source()
            .languages
            .iter()
            .map(|l| Choice::new(&l.code, l.name.clone()))
            .collect::<Vec<_>>();
        let language = cx.new(|cx| SelectState::new(choices, None, window, cx));
        language.update(cx, |s, cx| {
            s.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
        });
        let settings_owner = SettingsOwner::acquire(cx);
        let draft = settings_owner.read(cx).preferences().settings().clone();
        let error = settings_owner.read(cx).error().map(str::to_owned);
        let unsaved = settings_owner.read(cx).settings_dirty();
        let subscriptions = vec![
            cx.observe(&settings_owner, |this, owner, cx| {
                let owner = owner.read(cx);
                this.draft = owner.preferences().settings().clone();
                this.unsaved = owner.settings_dirty();
                this.error = owner.error().map(str::to_owned);
                cx.notify();
            }),
            cx.observe(&dashboard, |_, _, cx| cx.notify()),
            cx.subscribe(&locale_owner, |this: &mut Self, _, event, cx| {
                if matches!(event, SettingsEvent::Language) {
                    let handle = this.window;
                    let owner = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.language.update(cx, |s, cx| {
                                    s.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
                                });
                                cx.notify();
                            });
                        });
                    });
                }
            }),
            cx.subscribe(&language, |this: &mut Self, _, event, cx| {
                if let SelectEvent::Confirm(Some(code)) = event {
                    let handle = this.window;
                    let locale = this.locale_owner.clone();
                    let code = code.clone();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            locale.update(cx, |s, cx| s.set_language(&code, window, cx))
                        });
                    });
                }
            }),
        ];
        Self {
            page: Page::Chroma,
            history: vec![Page::Chroma],
            history_ix: 0,
            focus: cx.focus_handle(),
            window: window.window_handle(),
            locale_owner,
            dashboard,
            language,
            settings_owner,
            draft,
            error,
            dynamic_supported: razer_platform::system::supports_dynamic_lighting(),
            dynamic_switching: false,
            dualsense_supported: false,
            dualsense_present: false,
            notes: None,
            notes_subscription: None,
            unsaved,
            _subscriptions: subscriptions,
        }
    }
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        if self
            .notes
            .as_ref()
            .is_some_and(|notes| notes.update(cx, |notes, cx| notes.focus_if_open(window, cx)))
        {
            return;
        }
        self.focus.focus(window, cx);
    }
    pub fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.unsaved {
            self.error = Some(local(
                "本地更改尚未保存。可重试保存，或放弃未保存更改后关闭。",
                "Local changes are unsaved. Retry saving or discard unsaved changes before closing.",
            ));
            cx.notify();
        }
        !self.unsaved
    }
    pub fn bind_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window = window.window_handle();
        self.notes = None;
        self.notes_subscription = None;
        self.language.update(cx, |s, cx| {
            s.set_selected_value(&i18n::locale().to_lowercase(), window, cx)
        });
    }
    fn toggle(
        cx: &Context<Self>,
        action: fn(&mut Self, bool, &mut Context<Self>),
    ) -> impl Fn(CheckboxState, &ClickEvent, &mut Window, &mut App) + 'static {
        let owner = cx.entity().downgrade();
        move |state, _, _, cx| {
            let _ = owner.update(cx, |this, cx| {
                action(this, state == CheckboxState::Checked, cx)
            });
        }
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        self.error = self
            .settings_owner
            .update(cx, |owner, cx| owner.retry(cx))
            .err();
        cx.notify();
    }
    fn change_settings(&mut self, change: Change, cx: &mut Context<Self>) {
        self.error = self
            .settings_owner
            .update(cx, |owner, cx| owner.change(change, cx))
            .err();
        let owner = self.settings_owner.read(cx);
        self.draft = owner.preferences().settings().clone();
        self.unsaved = owner.settings_dirty();
        cx.notify();
    }
    fn discard(&mut self, cx: &mut Context<Self>) {
        self.error = self
            .settings_owner
            .update(cx, |owner, cx| owner.discard_settings(cx))
            .err();
        let owner = self.settings_owner.read(cx);
        self.draft = owner.preferences().settings().clone();
        self.unsaved = owner.settings_dirty();
        cx.notify();
    }
    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        if page == self.page {
            return;
        }
        self.history.truncate(self.history_ix + 1);
        self.history.push(page);
        self.history_ix += 1;
        self.page = page;
        cx.notify();
    }
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let next = if forward {
            self.history_ix.checked_add(1)
        } else {
            self.history_ix.checked_sub(1)
        };
        if let Some(ix) = next.filter(|ix| *ix < self.history.len()) {
            self.history_ix = ix;
            self.page = self.history[ix];
            cx.notify();
        }
    }
    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.unsaved {
            self.error = Some(local(
                "本地更改尚未保存，请先重试保存",
                "Local changes are unsaved. Retry saving before refreshing.",
            ));
            cx.notify();
            return;
        }
        self.error = self
            .settings_owner
            .update(cx, |owner, cx| owner.reload(cx))
            .err();
        cx.notify();
    }
    /// Accept only the current host's WDL/storage and Sensa feature observations.
    /// Local checkbox drafts never supply these service facts.
    pub fn observe(
        &mut self,
        switching: bool,
        dualsense_supported: bool,
        dualsense_present: bool,
        cx: &mut Context<Self>,
    ) {
        self.dynamic_switching = switching;
        self.dualsense_supported = dualsense_supported;
        self.dualsense_present = dualsense_present;
        cx.notify();
    }
    fn reset_tutorials(&mut self, cx: &mut Context<Self>) {
        if self.tutorials_reset(cx) {
            return;
        }
        self.change_settings(Change::ResetTutorials(source().tutorial_keys.clone()), cx);
        cx.emit(SettingsAction::ResetTutorials);
    }
    fn tutorials_reset(&self, cx: &App) -> bool {
        source().tutorial_keys.iter().take(3).all(|key| {
            if key == "isShowIntroductionBanner" {
                self.dashboard.read(cx).introduction_enabled()
            } else {
                self.draft.tutorial_enabled(key)
            }
        })
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let navigation = h_flex().w(relative(0.2)).children(
            [
                ("chroma-settings-back", "BACK", "back", self.history_ix == 0),
                (
                    "chroma-settings-forward",
                    "FORWARD",
                    "forward",
                    self.history_ix + 1 >= self.history.len(),
                ),
                ("chroma-settings-refresh", "REFRESH", "refresh", false),
            ]
            .map(|(id, label, asset, disabled)| {
                Button::new(id)
                    .accessibility_label(text(label))
                    .disabled(disabled)
                    .p_0()
                    .w(css(40.))
                    .h(css(38.))
                    .when(disabled, |b| b.opacity(0.3))
                    .when(!disabled, |b| b.hover(|s| s.bg(Colors::hover())))
                    .focus_visible(|s| s.border_1().border_color(Colors::selected()))
                    .child(
                        img(SharedString::from(format!(
                            "synapse/chroma-settings-{asset}.svg"
                        )))
                        .size(css(20.)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| match asset {
                        "back" => this.step(false, cx),
                        "forward" => this.step(true, cx),
                        _ => this.reload(cx),
                    }))
            }),
        );
        h_flex()
            .h(css(40.))
            .flex_shrink_0()
            .bg(Colors::background())
            .child(navigation)
            .child(
                div()
                    .flex_1()
                    .text_center()
                    .text_size(css(14.))
                    .text_color(Colors::secondary())
                    .child(text("SETTINGS_HEADER").to_uppercase()),
            )
            .child(div().w(relative(0.2)))
            .into_any_element()
    }
    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(css(48.))
            .border_b(css(2.))
            .border_color(Colors::tooltip())
            .flex_shrink_0()
            .justify_center()
            .gap(css(20.))
            .children([Page::Chroma, Page::General].map(|page| {
                let selected = page == self.page;
                Button::new(SharedString::from(format!(
                    "chroma-settings-nav-{}",
                    page.key()
                )))
                .accessibility_label(text(page.key()))
                .selected(selected)
                .px(css(10.))
                .py(css(7.))
                .rounded(css(14.))
                .text_size(css(12.))
                .line_height(css(14.))
                .when(selected, |b| {
                    b.bg(Colors::selected()).text_color(Colors::panel())
                })
                .when(!selected, |b| {
                    b.text_color(Colors::secondary())
                        .hover(|s| s.bg(Colors::hover()).text_color(Colors::text()))
                })
                .focus_visible(|s| s.border_1().border_color(Colors::selected()))
                .child(text(page.key()).to_uppercase())
                .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
            }))
            .into_any_element()
    }
    fn startup(&self, cx: &mut Context<Self>) -> AnyElement {
        let auto = self.draft.auto_start_enabled();
        let minimized = self.draft.start_minimized_enabled();
        panel("AUTO_LAUNCH")
            .child(
                checkbox(
                    "chroma-settings-autostart",
                    text("START_CHROMA_APP"),
                    auto,
                    false,
                )
                .on_change(Self::toggle(cx, |this, value, cx| {
                    this.change_settings(Change::AutoStart(value), cx);
                })),
            )
            .child(
                v_flex()
                    .relative()
                    .pl(css(30.))
                    .child(
                        div()
                            .absolute()
                            .left(css(9.))
                            .top_0()
                            .w(css(1.))
                            .h(css(37.))
                            .bg(Colors::note()),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(css(10.))
                            .top(css(36.))
                            .w(css(13.))
                            .h(css(1.))
                            .bg(Colors::note()),
                    )
                    .child(
                        div()
                            .mb(css(10.))
                            .text_color(Colors::note())
                            .child(text("NOTE_DISABLE_CHROMA_APP")),
                    )
                    .child(
                        checkbox(
                            "chroma-settings-minimized",
                            text("MINIMIZE_SYSTRAY"),
                            minimized,
                            !auto,
                        )
                        .on_change(Self::toggle(cx, |this, value, cx| {
                            if this.draft.auto_start_enabled() {
                                this.change_settings(Change::StartMinimized(value), cx);
                            }
                        })),
                    ),
            )
            .into_any_element()
    }
    fn notifications(&self, cx: &mut Context<Self>) -> AnyElement {
        let tip = format!(
            "{}\n• {}\n• {}\n• {}",
            text("NOTIFICATIONS_TOOLTIP"),
            text("NOTIFICATIONS_TOOLTIP_DESC1"),
            text("NOTIFICATIONS_TOOLTIP_DESC2"),
            text("NOTIFICATIONS_TOOLTIP_DESC3")
        );
        panel("NOTIFICATIONS")
            .child(
                h_flex()
                    .gap(css(10.))
                    .child(
                        checkbox(
                            "chroma-settings-notifications",
                            text("DISPLAY_NOTIFICATIONS"),
                            self.draft.notifications_enabled(),
                            false,
                        )
                        .on_change(Self::toggle(cx, |this, value, cx| {
                            this.change_settings(Change::Notifications(value), cx);
                        })),
                    )
                    .child(help("chroma-settings-notification-help", tip)),
            )
            .into_any_element()
    }
    fn lighting(&self, cx: &mut Context<Self>) -> AnyElement {
        let dynamic = self.draft.dynamic_lighting_enabled();
        panel("DEVICE_LIGHTING")
            .relative()
            .child(
                help(
                    "chroma-settings-lighting-help",
                    text("DEVICE_LIGHTING_TIPS"),
                )
                .absolute()
                .right(css(10.))
                .top(css(10.)),
            )
            .child(
                h_flex()
                    .gap(css(14.))
                    .child(
                        h_flex()
                            .h(css(36.))
                            .p(css(5.))
                            .border_1()
                            .border_color(Colors::border())
                            .rounded(css(18.))
                            .children([(false, "CHROMA_RGB"), (true, "DYNAMIC_LIGHTING")].map(
                                |(value, label)| {
                                    Button::new(SharedString::from(format!(
                                        "chroma-settings-lighting-mode-{label}"
                                    )))
                                    .accessibility_label(text(label))
                                    .selected(dynamic == value)
                                    .disabled(self.dynamic_switching)
                                    .h(css(26.))
                                    .px(css(10.))
                                    .py_0()
                                    .rounded(css(13.))
                                    .text_size(css(12.))
                                    .when(dynamic == value, |b| {
                                        b.bg(Colors::selected()).text_color(Colors::panel())
                                    })
                                    .when(self.dynamic_switching, |b| b.opacity(0.3))
                                    .focus_visible(|s| {
                                        s.border_1().border_color(Colors::selected())
                                    })
                                    .child(text(label))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.dynamic_switching {
                                            // ee invokes the same outer toggle for either half,
                                            // including the already selected mode.
                                            this.change_settings(
                                                Change::DynamicLighting(
                                                    !this.draft.dynamic_lighting_enabled(),
                                                ),
                                                cx,
                                            );
                                        }
                                    }))
                                },
                            )),
                    )
                    .when(self.dynamic_switching, |r| {
                        r.child(spinner::Spinner::new().small())
                    }),
            )
            .child(div().pt(css(20.)).child(text(if dynamic {
                "DYNAMIC_LIGHTING_TITLE"
            } else {
                "CHROMA_RGB_TITLE"
            })))
            .child(
                h_flex()
                    .items_start()
                    .mt(css(10.))
                    .gap(css(10.))
                    .child(
                        img("synapse/chroma-settings-warning.svg")
                            .size(css(14.))
                            .mt(css(3.))
                            .flex_shrink_0(),
                    )
                    .child(text(if dynamic {
                        "DYNAMIC_LIGHTING_MSG"
                    } else {
                        "CHROMA_RGB_MSG"
                    })),
            )
            .child(
                h_flex()
                    .mt(css(20.))
                    .gap(css(20.))
                    .child(
                        img("synapse/chroma-settings-wdl.svg")
                            .size(css(44.))
                            .flex_shrink_0(),
                    )
                    .child(
                        // Current settings/75.496bba91.chunk.js de calls
                        // msSettings("personalization-lighting"). The matching
                        // 75.e743878a.chunk.css .img-text .external supplies
                        // 14px/44px underlined text, green hover and .7 active.
                        Button::new("chroma-settings-open-wdl")
                            .accessibility_label(text("OPEN_WINDOWS_DYNAMIC_LIGHTING"))
                            .p_0()
                            .h(css(44.))
                            .text_size(css(14.))
                            .line_height(css(44.))
                            .text_color(Colors::text())
                            .text_decoration_1()
                            .hover(|style| style.text_color(Colors::selected()))
                            .active(|style| style.opacity(0.7))
                            .focus_visible(|style| {
                                style.border_1().border_color(Colors::selected())
                            })
                            .child(text("OPEN_WINDOWS_DYNAMIC_LIGHTING"))
                            .on_click(|_, _, cx| {
                                cx.open_url("ms-settings:personalization-lighting");
                            }),
                    ),
            )
            .into_any_element()
    }
    fn chroma(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let reset = self.tutorials_reset(cx);
        let left = v_flex()
            .child(self.startup(cx))
            .child(self.notifications(cx))
            .into_any_element();
        let right = v_flex()
            .child(
                panel("TUTORIAL_RESET").child(
                    h_flex()
                        .gap(css(20.))
                        .child(
                            source_button::settings_button(
                                "chroma-settings-reset",
                                text("RESET"),
                                reset,
                                window,
                                cx,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.reset_tutorials(cx))),
                        )
                        .child(div().flex_1().child(text("CHROMA_APP_TUTORIAL_RESET_MSG"))),
                ),
            )
            .child(
                panel("PROFILE_MIGRATION").child(
                    h_flex()
                        .gap(css(20.))
                        .child(
                            source_button::settings_button(
                                "chroma-settings-migration",
                                text("LAUNCH"),
                                false,
                                window,
                                cx,
                            )
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(SettingsAction::OpenMigration)),
                            ),
                        )
                        .child(div().flex_1().child(text("PROFILE_MIGRATION_DESC"))),
                ),
            )
            .when(self.dynamic_supported, |v| v.child(self.lighting(cx)))
            .into_any_element();
        columns(window, left, right)
    }
    /// The shell supplies notes read from its owning host memory store.
    /// Constructing a separate empty store here would lose the producer's content.
    pub fn show_release_notes(
        &mut self,
        notes: Entity<release_notes::ReleaseNotes>,
        cx: &mut Context<Self>,
    ) {
        self.notes_subscription = Some(cx.observe(&notes, |_, _, cx| cx.notify()));
        self.notes = Some(notes);
        cx.notify();
    }
    fn general(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let whats_new = text("RELEASE_PATCH_NOTE_WHATS_NEW");
        let (before, after) = whats_new
            .split_once("{{releasePatchNote}}")
            .unwrap_or((&whats_new, ""));
        let right = v_flex()
            .child(self.keyboard_navigation(window, cx))
            .child(self.about(cx))
            .into_any_element();
        columns(
            window,
            v_flex()
                .child(
                    panel("LANGUAGE")
                        // Ae mounts the plain dropdown-area (230px); it is not
                        // the separate general-setting 188px dropdown.
                        .child(razer_widgets::surface::select(&self.language).w(css(230.))),
                )
                .child(
                    panel("THX_WHAT_NEWS").child(
                        h_flex()
                            .flex_wrap()
                            .child(before.to_owned())
                            .child(
                                Button::new("chroma-settings-release-notes")
                                    .accessibility_label("Release Notes")
                                    .p_0()
                                    .text_size(css(14.))
                                    .text_color(Colors::text())
                                    .text_decoration_1()
                                    .hover(|s| s.text_color(Colors::selected()))
                                    .focus_visible(|s| {
                                        s.border_1().border_color(Colors::selected())
                                    })
                                    .child("Release Notes")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        cx.emit(SettingsAction::ReleaseNotes)
                                    })),
                            )
                            .child(after.to_owned()),
                    ),
                )
                .into_any_element(),
            right,
        )
    }
    fn keyboard_navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Current General owner ga mounts Sa in its right column before About.
        panel_title(i18n::t("CHROMA_SETTINGS.KEYBOARD_NAVIGATION"))
            .child(h_flex().items_center().gap(css(20.))
                .child(source_button::settings_button(
                    "chroma-settings-keyboard-navigation",
                    i18n::t("CHROMA_SETTINGS.VIEW_ALL_SHORTCUTS"), false, window, cx,
                ).on_click(|_, window, cx| razer_widgets::keyboard_navigation::open_guide(window, cx)))
                .child(div().flex_1().child(i18n::t("CHROMA_SETTINGS.KEYBOARD_NAVIGATION_CHROMA_MSG"))))
            .into_any_element()
    }
    fn about(&self, _cx: &mut Context<Self>) -> AnyElement {
        panel("ABOUT")
            .child(
                div().flex().justify_center().mb(css(20.)).child(
                    img("synapse/chroma-settings-logo.svg")
                        .w(css(400.))
                        .h(css(400. * 70. / 392.))
                        .object_fit(ObjectFit::Contain),
                ),
            )
            .child(
                div()
                    .text_center()
                    .mb(css(20.))
                    .text_color(Colors::secondary())
                    .child(text("VERSION").replace("{{number}}", &source().version)),
            )
            .child(
                div().text_center().text_color(Colors::secondary()).child(
                    text("COPYRIGHT")
                        .replace("{{year}}", &chrono::Local::now().format("%Y").to_string()),
                ),
            )
            .when(self.dualsense_supported && self.dualsense_present, |p| {
                p.child(
                    div()
                        .mt(css(20.))
                        .text_center()
                        .text_color(Colors::secondary())
                        .child(text("PS_TRADEMARK")),
                )
            })
            .child(
                div()
                    .mt(css(20.))
                    .mb(css(20.))
                    .text_center()
                    .text_color(Colors::secondary())
                    .child(text("TRADEMARK")),
            )
            .into_any_element()
    }
}
fn panel(title: &str) -> Div {
    panel_title(text(title))
}
fn panel_title(title: String) -> Div {
    v_flex()
        .w(css(600.))
        .my(css(10.))
        .px(css(40.))
        .py(css(30.))
        .rounded(css(5.))
        .bg(Colors::panel())
        .text_size(css(14.))
        .line_height(css(17.))
        .text_color(Colors::text())
        .child(
            div()
                .mb(css(20.))
                .font_family("RazerF5")
                .text_size(css(18.))
                .text_color(Colors::selected())
                .child(title.to_uppercase()),
        )
}
fn columns(_window: &Window, left: AnyElement, right: AnyElement) -> AnyElement {
    h_flex()
        .items_start()
        .flex_wrap()
        .justify_center()
        .max_w(css(1240.))
        .mx_auto()
        .child(
            div()
                .w(css(600.))
                // .main-setting div.widget-col overrides the generic narrow
                // breakpoint's 30px margins back to zero.
                .child(left),
        )
        .child(div().w(css(600.)).child(right))
        .into_any_element()
}
fn checkbox(id: &'static str, label: String, checked: bool, disabled: bool) -> Checkbox {
    Checkbox::new(id)
        .accessibility_label(label.clone())
        .checked(checked)
        .disabled(disabled)
        .group(id)
        .flex()
        .items_start()
        .min_h(css(20.))
        .gap(css(10.))
        .when(disabled, |c| c.opacity(0.3))
        .focus_visible(|s| s.bg(Colors::hover()))
        .child(
            div()
                .size(css(20.))
                .relative()
                .flex_shrink_0()
                .border_1()
                .rounded(css(2.4))
                .border_color(if checked {
                    Colors::selected()
                } else {
                    Colors::checkbox_border()
                })
                .when(checked, |v| {
                    v.bg(Colors::selected()).child(
                        img("synapse/chroma-settings-check.svg")
                            .absolute()
                            .top_0()
                            .left_0()
                            .size(css(20.)),
                    )
                })
                .when(!disabled, |v| {
                    v.group_hover(id, |s| s.border_color(Colors::selected()))
                }),
        )
        .child(
            div()
                .pt(css(2.))
                .text_size(css(14.))
                .line_height(css(17.))
                .child(label),
        )
}
fn help(id: &'static str, tip: String) -> Button {
    Button::new(id)
        .accessibility_label(tip.clone())
        .p_0()
        .size(css(14.))
        .rounded(css(7.))
        .bg(Colors::help())
        .hover(|s| s.bg(Colors::help_hover()))
        .focus_visible(|s| s.border_1().border_color(Colors::selected()))
        .child(img("synapse/chroma-settings-help.svg").size_full())
        .tooltip(move |window, cx| {
            Tooltip::new(tip.clone())
                .max_w(css(300.))
                .text_size(css(14.))
                .line_height(css(16.))
                .bg(Colors::tooltip())
                .border_color(Colors::border())
                .rounded_none()
                .shadow_none()
                .px(css(10.))
                .py(css(8.))
                .build(window, cx)
        })
}
impl Render for ChromaSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::Chroma => self.chroma(window, cx),
            Page::General => self.general(window, cx),
        };
        let content = v_flex()
            .pt(css(10.))
            .px(css(20.))
            .pb(css(20.))
            .child(content)
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    h_flex()
                        .gap(css(10.))
                        .justify_center()
                        .child(
                            div()
                                .id("chroma-settings-storage-error")
                                .text_size(css(12.))
                                .child(error),
                        )
                        .child(
                            Button::new("chroma-settings-retry")
                                .accessibility_label(local("重试保存", "Retry save"))
                                .child(local("重试保存", "Retry save"))
                                .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                        ),
                )
            });
        let content = content.when(self.unsaved, |v| {
            v.child(
                Button::new("chroma-settings-discard")
                    .disabled(self.settings_owner.read(cx).is_pending())
                    .accessibility_label(local(
                        "放弃未保存的本地更改",
                        "Discard unsaved local changes",
                    ))
                    .self_center()
                    .child(local(
                        "放弃未保存的本地更改",
                        "Discard unsaved local changes",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.discard(cx))),
            )
        });
        v_flex()
            .id("chroma-settings")
            .test_support()
            .aria_label(text("SETTINGS_HEADER"))
            .size_full()
            .min_w(css(600.))
            .font_family("Roboto")
            .bg(Colors::background())
            .text_color(Colors::text())
            .track_focus(&self.focus)
            .child(self.toolbar(cx))
            .child(self.navigation(cx))
            .child(
                div()
                    .id("chroma-settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(content),
            )
            .when_some(self.notes.clone(), |v, notes| v.child(notes))
    }
}

// Generated catalog reader: razer-app-pages/chroma_settings_data.
fn read_chroma_settings_catalog() -> Source {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                translations: r.child(decode_1)?,
                languages: r.child(decode_4)?,
                tutorial_keys: r.child(decode_6)?,
                version: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Language>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Language> {
        let value = match r.tag()? {
            0 => Language {
                name: r.child(decode_2)?,
                code: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_CHROMA_SETTINGS_DATA,
        0,
        "d3f99c6d91ff46c4d7a9b3f23ca0b8d138204fa0594949a347a0b18e40267f4f",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod chroma_settings_data_catalog_tests {
    #[test]
    fn packaged_catalog_chroma_settings_data_decodes() {
        let _ = super::read_chroma_settings_catalog();
    }
}
// End generated catalog reader: razer-app-pages/chroma_settings_data.
